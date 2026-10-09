//! Commit-reveal fairness for match entropy (`docs/FAIRNESS.md`).
//!
//! The scheme, in one paragraph: the host server draws a secret 256-bit
//! **seed** and 256-bit **salt** and publishes a SHA-256 **commitment** over
//! them (plus the engine bundle, the ruleset and the canonical room settings)
//! *before* anyone plays. Every human client then contributes a 256-bit
//! **nonce**; the match RNG is seeded from `SHA-256(seed ‖ sorted nonces)`.
//! Because the server's seed is fixed (committed) before the nonces arrive,
//! the server cannot adapt its seed to player entropy; because the nonces are
//! mixed in, the server cannot know the match stream even though it chose the
//! seed. After the match the openings (seed, salt, nonces) are revealed in
//! the sealed record and anyone can re-check the commitment and re-derive the
//! stream the match ran on.
//!
//! Everything in this module is pure hashing over explicit bytes: the server,
//! the browser engine and the tests all run the same recipe. The encodings are
//! pinned byte-for-byte below -- change the recipe, bump the version tags.

use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::net::RoomMember;
use crate::record::RecordFile;
use crate::scoring::ScoreWeights;
use crate::MatchMode;

/// Version tag inside every hash domain string below. Bump when a recipe
/// changes; an opening produced by another recipe simply fails verification.
pub const FAIR_VERSION: u32 = 1;

/// Domain tag for the commitment hash.
pub const DOM_COMMIT: &str = "bd-fair-commit-v1";
/// Domain tag for the match-seed derivation.
pub const DOM_SEED: &str = "bd-fair-seed-v1";
/// Domain tag for the second stream (`Match`'s `live_rng`) derived from the
/// match seed.
pub const DOM_LIVE: &str = "bd-fair-live-v1";

// ---------------------------------------------------------------- bytes / hex

/// 32 bytes as lower-case hex.
pub fn hex32(b: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

/// Parse 64 lower-case (or upper-case) hex chars into 32 bytes.
pub fn unhex32(s: &str) -> Result<[u8; 32], String> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return Err(format!("want 64 hex chars, got {}", b.len()));
    }
    let mut out = [0u8; 32];
    let nib = |c: u8| -> Result<u8, String> {
        match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'A'..=b'F' => Ok(c - b'A' + 10),
            _ => Err(format!("bad hex char {:?}", c as char)),
        }
    };
    for i in 0..32 {
        out[i] = nib(b[i * 2])? << 4 | nib(b[i * 2 + 1])?;
    }
    Ok(out)
}

fn sha256_hex(parts: &[&[u8]]) -> String {
    let mut h = sha2::Sha256::new();
    for p in parts {
        h.update(p);
    }
    hex32(&{
        let d = h.finalize();
        let mut o = [0u8; 32];
        o.copy_from_slice(&d);
        o
    })
}

// ---------------------------------------------------------------- settings

/// The gameplay-affecting room settings a commitment binds. Deliberately
/// narrow: cosmetics (theme, room name, max players) cannot change a dice
/// roll and are not covered -- `docs/FAIRNESS.md` "what the commit does not
/// cover".
///
/// Canonical encoding (the exact string the hash sees):
/// `bd-fair-settings-v1\n<mode>\n<step_bits>\n<weights_json>\n<canon_member>\n...`
/// where `step_bits` is the `f32` step's **bit pattern** as decimal (float
/// formatting never enters), `weights_json` is `serde_json` of
/// [`ScoreWeights`] in declaration order, and each seat is
/// `<id>,<player>,<bot 0|1>,<mentality>` in seat order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonSettings {
    pub mode: MatchMode,
    /// The tick quantum's f32 bits (never the decimal string).
    pub step_bits: u32,
    pub weights: ScoreWeights,
    pub members: Vec<CanonMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonMember {
    pub id: i32,
    pub player: String,
    pub bot: bool,
    pub mentality: i32,
}

impl CanonSettings {
    pub fn new(mode: MatchMode, step: f32, weights: &ScoreWeights, members: &[RoomMember]) -> Self {
        Self {
            mode,
            step_bits: step.to_bits(),
            weights: weights.clone(),
            members: members.iter().map(CanonMember::from).collect(),
        }
    }

    /// The canonical string the commitment hashes (see the type docs).
    pub fn canon(&self) -> String {
        let w = serde_json::to_string(&self.weights).unwrap_or_else(|_| "{}".into());
        let mut s = String::new();
        s.push_str("bd-fair-settings-v1\n");
        s.push_str(&format!("{}\n", self.mode as i32));
        s.push_str(&format!("{}\n", self.step_bits));
        s.push_str(&w);
        s.push('\n');
        for m in &self.members {
            s.push_str(&format!(
                "{},{},{},{}\n",
                m.id,
                m.player,
                if m.bot { 1 } else { 0 },
                m.mentality
            ));
        }
        s
    }
}

impl From<&RoomMember> for CanonMember {
    fn from(m: &RoomMember) -> Self {
        Self {
            id: m.id,
            player: m.player.clone(),
            bot: m.bot,
            mentality: m.mentality as i32,
        }
    }
}

// ---------------------------------------------------------------- openings

/// One player's entropy contribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonceEntry {
    pub member: i32,
    /// 32 bytes, lower-case hex.
    pub nonce: String,
}

/// The reveal: everything the commitment opened. Lives in the sealed record's
/// header (`docs/REPLAY.md` §4); the server holds the openings in its
/// server-side record log from match creation and they cross the network to a
/// client only inside that sealed record, only after the match, only to
/// participants (`docs/FAIRNESS.md` "threat model").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Fairness {
    /// [`FAIR_VERSION`] of the recipe that produced these openings.
    pub v: u32,
    /// The SHA-256 commitment (hex), shown in the match UI at start.
    pub commit: String,
    /// The server's secret 256-bit seed (hex). Empty while the match runs.
    pub seed: String,
    /// The server's secret 256-bit salt (hex). Empty while the match runs.
    pub salt: String,
    /// Human player nonces in **ascending member id** order. Bots contribute
    /// none; a missing nonce is simply absent. Public after the game.
    pub nonces: Vec<NonceEntry>,
    /// The canonical room settings string the commitment hashed.
    pub settings: String,
}

impl Default for Fairness {
    fn default() -> Self {
        Self {
            v: FAIR_VERSION,
            commit: String::new(),
            seed: String::new(),
            salt: String::new(),
            nonces: vec![],
            settings: String::new(),
        }
    }
}

// ---------------------------------------------------------------- recipes

/// `commit = SHA-256("bd-fair-commit-v1\n" ‖ seed_hex ‖ "\n" ‖ salt_hex ‖
/// "\n" ‖ bundle ‖ "\n" ‖ ruleset_sha256 ‖ "\n" ‖ settings ‖ "\n")` -- all
/// parts hex / plain text, newline-separated, settings last (it is the only
/// multi-line part and it ends with a newline of its own; the trailing
/// separator is a second one, kept so every field has a separator after it).
pub fn commit_hex(
    seed: &[u8; 32],
    salt: &[u8; 32],
    bundle: &str,
    ruleset_sha256: &str,
    settings: &str,
) -> String {
    sha256_hex(&[
        format!("{DOM_COMMIT}\n").as_bytes(),
        format!("{}\n", hex32(seed)).as_bytes(),
        format!("{}\n", hex32(salt)).as_bytes(),
        format!("{bundle}\n").as_bytes(),
        format!("{ruleset_sha256}\n").as_bytes(),
        format!("{settings}\n").as_bytes(),
    ])
}

/// The match seed: `SHA-256("bd-fair-seed-v1\n" ‖ seed_hex ‖ "\n" ‖
/// "<member>\n<nonce_hex>\n" ...)`, nonces sorted by ascending member id.
/// Entries with a duplicate member id are a corrupt input (rejected by
/// [`derive_match_seed`]; this raw form assumes the list is already clean).
pub fn derive_match_seed(seed: &[u8; 32], nonces: &[(i32, [u8; 32])]) -> [u8; 32] {
    let mut list: Vec<&(i32, [u8; 32])> = nonces.iter().collect();
    list.sort_by_key(|(m, _)| *m);
    let mut parts: Vec<Vec<u8>> = vec![format!("{DOM_SEED}\n").into(), format!("{}\n", hex32(seed)).into()];
    for (m, n) in list {
        parts.push(format!("{m}\n{}\n", hex32(n)).into());
    }
    let refs: Vec<&[u8]> = parts.iter().map(|p| p.as_slice()).collect();
    unhex32(&sha256_hex(&refs)).expect("sha256 is 32 bytes")
}

/// The public `MatchState.match_id`, hashed out of the derived seed so no
/// seed bits travel in a view. Same shape the legacy u64 path produces
/// (`((x as i32) & 0x7FFF_FFFF) | 1`), different source.
pub fn match_id(match_seed: &[u8; 32]) -> i32 {
    let d = unhex32(&sha256_hex(&[
        b"bd-fair-id-v1
",
        format!("{}
", hex32(match_seed)).as_bytes(),
    ]))
    .expect("sha256 is 32 bytes");
    let id = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
    ((id as i32) & 0x7FFF_FFFF) | 1
}

/// The second stream's key: `SHA-256("bd-fair-live-v1\n" ‖ match_seed_hex ‖
/// "\n")`. `Match` keeps one game stream and one live-cosmetics stream
/// (`live_rng`); they must not share a key.
pub fn live_seed(match_seed: &[u8; 32]) -> [u8; 32] {
    unhex32(&sha256_hex(&[
        format!("{DOM_LIVE}\n").as_bytes(),
        format!("{}\n", hex32(match_seed)).as_bytes(),
    ]))
    .expect("sha256 is 32 bytes")
}

// ---------------------------------------------------------------- verify

/// One checked step of [`verify`]. The whole check passes when no step is
/// `false`; a step whose `ok` is true but whose `note` is set is a caveat
/// (e.g. a partial record whose initial RNG cannot be pinned).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyStep {
    /// Machine name: `commit` | `settings` | `derived_seed` | `initial_rng` |
    /// `bundle` | `ruleset` | `replay`.
    pub step: String,
    pub ok: bool,
    /// Human-readable reason (pass or fail). Empty when there is nothing to
    /// say beyond ok/fail.
    pub note: String,
}

/// The full verdict of one record's fairness check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyReport {
    pub ok: bool,
    /// False when the record carries no fairness material at all (written
    /// before the scheme) -- then `ok` is false and `steps` explains why.
    pub present: bool,
    pub steps: Vec<VerifyStep>,
}

impl VerifyReport {
    fn push(&mut self, step: &str, ok: bool, note: impl Into<String>) {
        self.steps.push(VerifyStep {
            step: step.into(),
            ok,
            note: note.into(),
        });
        if !ok {
            self.ok = false;
        }
    }
}

/// Hash-only verification of one record's fairness material. Engine-free: it
/// runs the same in the server tests, in the browser's current build and in
/// the record's own archived bundle. The engine-dependent half (the initial
/// RNG state and the replay itself) is [`verify_replay`].
///
/// Checks, in order:
/// 1. `fair` is present and `v` is the version this recipe speaks.
/// 2. The canonical settings string round-trips: it must equal what
///    [`CanonSettings`] rebuilt from the record's own header + setup hashes
///    (when the record carries a seeded setup; snapshot records skip this).
/// 3. `commit` recomputes from `seed`/`salt`/`settings` and the record's
///    engine stamp (bundle id + ruleset sha).
/// 4. The derived match seed equals `body.init.seed256` (seeded records).
pub fn verify(file: &RecordFile) -> VerifyReport {
    let mut r = VerifyReport {
        ok: true,
        present: false,
        steps: vec![],
    };
    let Some(fair) = file.header.fair.as_ref() else {
        r.push("commit", false, "record carries no fairness material");
        return r;
    };
    r.present = true;
    if fair.v != FAIR_VERSION {
        r.push(
            "commit",
            false,
            format!("fairness recipe v{} != v{}", fair.v, FAIR_VERSION),
        );
        return r;
    }
    if fair.commit.is_empty() || fair.seed.is_empty() || fair.salt.is_empty() {
        r.push("commit", false, "openings missing (seed/salt/commit)");
        return r;
    }
    let (seed, salt) = match (unhex32(&fair.seed), unhex32(&fair.salt)) {
        (Ok(s), Ok(t)) => (s, t),
        (e, t) => {
            let _ = t;
            r.push(
                "commit",
                false,
                format!("openings not 32 bytes: {:?}", e.map(|_| ())),
            );
            return r;
        }
    };

    // -- settings --------------------------------------------------------
    // Rebuild the canonical string from the record itself and require it to
    // match what the server hashed. A snapshot record has no seeded setup to
    // rebuild members from; take the stored string then (the commit still
    // covers it).
    match &file.body.init {
        crate::record::Init::Seed(setup) => {
            let want = CanonSettings::new(
                file.header.mode,
                file.header.step,
                &setup.weights,
                &setup.members,
            )
            .canon();
            if want != fair.settings {
                r.push("settings", false, "canonical settings do not match the record");
            } else {
                r.push("settings", true, "");
            }
        }
        crate::record::Init::Snapshot { .. } => {
            if fair.settings.is_empty() {
                r.push("settings", false, "settings string missing");
            } else {
                r.push("settings", true, "snapshot record: settings taken on trust");
            }
        }
    }

    // -- commit ----------------------------------------------------------
    let got = commit_hex(
        &seed,
        &salt,
        &file.header.engine.bundle,
        &file.header.engine.ruleset_sha256,
        &fair.settings,
    );
    if got != fair.commit {
        r.push(
            "commit",
            false,
            format!("commitment {} != recomputed {}", fair.commit, got),
        );
    } else {
        r.push("commit", true, "");
    }

    // -- bundle / ruleset presence ---------------------------------------
    if file.header.engine.bundle.is_empty() {
        r.push("bundle", false, "record names no engine bundle");
    } else {
        r.push("bundle", true, &file.header.engine.bundle);
    }
    if file.header.engine.ruleset_sha256.is_empty() {
        r.push("ruleset", false, "record names no ruleset hash");
    } else {
        r.push("ruleset", true, &file.header.engine.ruleset_sha256);
    }

    // -- derived seed ----------------------------------------------------
    let mut nonces: Vec<(i32, [u8; 32])> = Vec::new();
    for n in &fair.nonces {
        match unhex32(&n.nonce) {
            Ok(b) => {
                if nonces.iter().any(|(m, _)| *m == n.member) {
                    r.push("derived_seed", false, format!("member {} nonce duplicated", n.member));
                    return r;
                }
                nonces.push((n.member, b));
            }
            Err(e) => {
                r.push("derived_seed", false, format!("member {}: {e}", n.member));
                return r;
            }
        }
    }
    let derived = derive_match_seed(&seed, &nonces);
    match &file.body.init {
        crate::record::Init::Seed(setup) => match setup.seed256 {
            Some(s) if s.0 == derived => r.push("derived_seed", true, ""),
            Some(s) => r.push(
                "derived_seed",
                false,
                format!("setup seed {} != derived {}", hex32(&s.0), hex32(&derived)),
            ),
            None => r.push(
                "derived_seed",
                false,
                "seeded record carries no 256-bit seed (legacy record)",
            ),
        },
        crate::record::Init::Snapshot { .. } => {
            r.push(
                "derived_seed",
                true,
                "snapshot record: derived seed not applicable",
            );
        }
    }
    r
}

/// The engine half of the check: rebuild the match from the derived seed,
/// demand its initial RNG state equal [`crate::rng::Rng::from_seed256`] of
/// that seed (and the live stream equal [`live_seed`]), then replay the whole
/// input log and require every checkpoint to hold (`docs/REPLAY.md` §4).
///
/// The caller must pass the engine that matches the record's bundle -- in the
/// browser that is the archived bundle the replay loader picked, so an old
/// record is checked by the very engine that wrote it. Records without
/// fairness material fail in [`verify`] before reaching here.
pub fn verify_replay(
    file: &RecordFile,
    data: std::sync::Arc<crate::data::GameData>,
    rules: std::sync::Arc<dyn crate::engine::CardRules>,
) -> VerifyReport {
    let mut r = verify(file);
    if !r.present {
        return r;
    }
    // Only a seeded record has an initial RNG state to pin.
    let crate::record::Init::Seed(setup) = &file.body.init else {
        r.push(
            "initial_rng",
            true,
            "snapshot record: initial RNG state not verifiable",
        );
        return r;
    };
    let Some(fair) = file.header.fair.as_ref() else {
        return r;
    };
    let Ok(seed) = unhex32(&fair.seed) else {
        return r;
    };
    let nonces: Vec<(i32, [u8; 32])> = fair
        .nonces
        .iter()
        .filter_map(|n| unhex32(&n.nonce).ok().map(|b| (n.member, b)))
        .collect();
    let derived = derive_match_seed(&seed, &nonces);

    // The match the derived seed builds must be keyed by exactly that seed,
    // and the live stream by `live_seed(derived)` -- the cross-check between
    // this recipe and `Match::new_seeded`.
    let m = crate::engine::Match::new_seeded(
        data.clone(),
        rules.clone(),
        &setup.members,
        crate::rng::Seed256(derived),
        file.header.mode,
        setup.weights,
    );
    let (game_key, live_key) = m.seed256s();
    let mut rng_ok = true;
    match game_key {
        Some(k) if k == derived => r.push("initial_rng", true, ""),
        Some(k) => {
            rng_ok = false;
            r.push(
                "initial_rng",
                false,
                format!(
                    "match rng keyed by {} != derived {}",
                    hex32(&k),
                    hex32(&derived)
                ),
            );
        }
        None => {
            rng_ok = false;
            r.push("initial_rng", false, "match rng is not the ChaCha stream");
        }
    }
    match live_key {
        Some(k) if k == live_seed(&derived) => {}
        got => {
            if rng_ok {
                r.push("initial_rng", false, format!("live stream mismatch: {got:?}"));
            }
        }
    }

    // Replay the input log through the documented construction and check the
    // checkpoints. `from_body` re-derives the same match from `setup`.
    let mut rep = match crate::record::Replayer::from_body(
        data,
        rules,
        file.header.clone(),
        file.body.clone(),
    ) {
        Ok(x) => x,
        Err(e) => {
            r.push("replay", false, format!("replayer: {e}"));
            return r;
        }
    };
    let mut guard = 0u32;
    while !rep.status().ended {
        rep.step_ticks(1);
        guard += 1;
        if guard > 5_000_000 {
            r.push("replay", false, "replay did not terminate");
            return r;
        }
    }
    if rep.status().diverged {
        let why = rep
            .first_mismatch()
            .map(|m| format!("{}: want {} got {}", m.field, m.want, m.got))
            .unwrap_or_else(|| "diverged".into());
        r.push("replay", false, why);
    } else {
        r.push("replay", true, "");
    }
    r
}

/// What the browser needs to show: pass/fail plus one line per step.
pub fn verify_summary(rep: &VerifyReport) -> String {
    let mut s = String::new();
    for st in &rep.steps {
        s.push_str(if st.ok { "PASS " } else { "FAIL " });
        s.push_str(&st.step);
        if !st.note.is_empty() {
            s.push_str(": ");
            s.push_str(&st.note);
        }
        s.push('\n');
    }
    s
}

/// The `RoomInfo`-facing half of a match's fairness state: everything a
/// client may see **during** the match. The openings are deliberately not
/// here (`docs/FAIRNESS.md`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FairPublic {
    /// The commitment (hex). Copyable in the match UI.
    pub commit: String,
    /// The canonical room settings string the commitment hashed.
    pub settings: String,
    /// True while the nonce window is still open (the match has not been
    /// created yet).
    pub collecting: bool,
}

/// Match-mode canonicalization used by [`CanonSettings`]: the mode the room
/// will start as.
pub fn mode_of(ranked: bool) -> MatchMode {
    if ranked {
        MatchMode::Ranked
    } else {
        MatchMode::Casual
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::RoomMember;
    use crate::scoring::ScoreWeights;
    use crate::state::BotMentality;

    fn seed(b: u8) -> [u8; 32] {
        [b; 32]
    }

    fn members() -> Vec<RoomMember> {
        vec![
            RoomMember {
                id: 0,
                player: "alice".into(),
                bot: false,
                mentality: BotMentality::Standard,
                ..Default::default()
            },
            RoomMember {
                id: 1,
                player: "bob".into(),
                bot: true,
                mentality: BotMentality::Standard,
                ..Default::default()
            },
        ]
    }

    #[test]
    fn commit_round_trip() {
        let s = CanonSettings::new(MatchMode::Casual, 0.05, &ScoreWeights::default(), &members());
        let c = commit_hex(&seed(1), &seed(2), "bundle-x", "rules-y", &s.canon());
        assert_eq!(c.len(), 64);
        assert_eq!(
            c,
            commit_hex(&seed(1), &seed(2), "bundle-x", "rules-y", &s.canon())
        );
        assert_ne!(c, commit_hex(&seed(1), &seed(3), "bundle-x", "rules-y", &s.canon()));
    }

    #[test]
    fn derive_is_order_independent_and_nonce_sensitive() {
        let a = derive_match_seed(&seed(1), &[(1, seed(9)), (0, seed(8))]);
        let b = derive_match_seed(&seed(1), &[(0, seed(8)), (1, seed(9))]);
        assert_eq!(a, b);
        assert_ne!(a, derive_match_seed(&seed(1), &[(0, seed(8)), (1, seed(7))]));
        assert_ne!(a, derive_match_seed(&seed(2), &[(0, seed(8)), (1, seed(9))]));
        // A missing nonce is simply absent -- a different list is a
        // different seed.
        assert_ne!(a, derive_match_seed(&seed(1), &[(0, seed(8))]));
    }

    #[test]
    fn settings_canon_stable() {
        let s = CanonSettings::new(MatchMode::Casual, 0.05, &ScoreWeights::default(), &members());
        let c = s.canon();
        assert!(
            c.starts_with(&format!("bd-fair-settings-v1
{}
", MatchMode::Casual as i32)),
            "{c}"
        );
        assert!(c.contains("alice"));
        assert_eq!(
            c,
            CanonSettings::new(MatchMode::Casual, 0.05, &ScoreWeights::default(), &members()).canon()
        );
    }
}