//! G-STATUS -- [停留] / [晕眩] / [除外] / [不可阻挡] / [无法移动] and layers.
//!
//! Owned by unit 3. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Source precedence: live sheet (`docs/rulebook/cards.json`,
//! `the roster` / `bands.json`) > `docs/rulebook/rulebook-doc.md`.
//! `rulebook.txt` is not authoritative.
//!
//! The book spells 眩晕 and 晕眩 interchangeably; both mean the same status
//! (engine keys `stun` / `stunStart`).

use game_core::engine::World;
use game_core::state::MatchEvent;
use spec_kit::{Clause, Step, StepAct};

/// Status clauses. Each entry is one book sentence, checked as a predicate
/// over one observed transition.
pub const CLAUSES: &[Clause] = &[
    G_STATUS_01,
    G_STATUS_02,
    G_STATUS_03,
    G_STATUS_04,
    G_STATUS_05,
    G_STATUS_06,
    G_STATUS_07,
    G_STATUS_08,
    G_STATUS_09,
    G_STATUS_10,
];

// ---------------------------------------------------------------------------
// shared helpers (local to this file; the kit has no status vocabulary)
// ---------------------------------------------------------------------------

const STAY: &str = game_core::state::key::STAY;
const STUN: &str = game_core::state::key::STUN;
const STUN_START: &str = game_core::state::key::STUN_START;
const EXILE: &str = game_core::state::key::EXILE;
const EXILE_TO: &str = game_core::state::key::EXILE_TO;
const UNSTOPPABLE: &str = game_core::state::key::UNSTOPPABLE;

fn layers(w: &game_core::engine::World, i: usize, key: &str) -> i32 {
    w.st.players.get(i).map(|p| p.state_get(key)).unwrap_or(0)
}

/// [晕眩] layers, counting the timed `stunStart` variant the engine uses for
/// layers that must survive the current turn end.
fn stun_layers(w: &game_core::engine::World, i: usize) -> i32 {
    layers(w, i, STUN) + layers(w, i, STUN_START)
}

/// [无法移动] holds when any of the three statuses is on. [不可阻挡] only
/// cancels the [停留] half (item 14: 「无视已存在的[停留]」).
fn cannot_move(w: &game_core::engine::World, i: usize) -> bool {
    let stay = layers(w, i, STAY);
    let unstoppable = layers(w, i, UNSTOPPABLE) > 0;
    (stay > 0 && !unstoppable) || stun_layers(w, i) > 0 || layers(w, i, EXILE) > 0
}

/// Message key of a logged event (`log.*`), or the bare key.
fn msg_key(e: &MatchEvent) -> &str {
    let k = e.msg.key();
    k.strip_prefix("log.").unwrap_or(k)
}

/// Turn started for `who` in this window (`log.turn` / `log.turn_extra`).
fn turn_started(events: &[MatchEvent], who: usize) -> bool {
    let w = who as i32;
    events.iter().any(|e| {
        e.r#type == "turn" && e.player_id == w && matches!(msg_key(e), "turn" | "turn_extra")
    })
}

/// The player's turn ended in this window: a voluntary `end`, the auto-skip
/// lines that run `end_turn`, or a later `turn` line taking over.
fn turn_ended(events: &[MatchEvent], who: usize) -> bool {
    let w = who as i32;
    for e in events {
        let k = msg_key(e);
        if e.player_id == w && matches!(k, "end_turn" | "stunned_skip" | "exiled_skip") {
            return true;
        }
    }
    if turn_started(events, who) {
        let mut seen_who = false;
        for e in events {
            if e.r#type != "turn" {
                continue;
            }
            if !matches!(msg_key(e), "turn" | "turn_extra") {
                continue;
            }
            if e.player_id == w {
                seen_who = true;
            } else if seen_who {
                return true;
            }
        }
    }
    false
}

/// How many times this window logged a grant of `key` layers to `who`.
fn grants(events: &[MatchEvent], who: usize, key: &str) -> i32 {
    let want = match key {
        STAY => "outcome.stay",
        STUN | STUN_START => "outcome.stun",
        EXILE => "outcome.exile",
        _ => return 0,
    };
    let w = who as i32;
    events
        .iter()
        .filter(|e| {
            if msg_key(e) != want {
                return false;
            }
            // `log.outcome.stay` / `stun` carry `target`; `log.outcome.exile`
            // names the target as `player_id`. Accept either slot.
            let target = spec_kit::player_arg(&e.msg, "target").unwrap_or(e.player_id);
            target == w
        })
        .count() as i32
}

/// An explicit status-[清除] line. Deliberately narrow: a generic
/// `embers_clear` (mark wipe) must not disable the layer-decay checks.
fn status_cleared(events: &[MatchEvent]) -> bool {
    events.iter().any(|e| {
        let k = msg_key(e);
        k == "outcome.clear" || k == "outcome.purge" || k == "status.clear"
    })
}

/// A card body / effect ran in this window and may have granted layers
/// without a `log.outcome.*` line (the hostfns `give_stay` path does not
/// log). Exact decay is only required on a quiet window.
fn busy_window(events: &[MatchEvent]) -> bool {
    events.iter().any(|e| {
        e.r#type == "card" || matches!(msg_key(e), "effect_applied" | "card_effect")
    })
}

/// Did this window log a [停留] skip for `who`?
fn stay_skip(events: &[MatchEvent], who: usize) -> bool {
    events
        .iter()
        .any(|e| e.r#type == "status" && e.player_id == who as i32 && msg_key(e) == "stay_skip")
}

/// A `"card"` activation by `who` of the given trigger kind ran in this window.
fn activated(step: &Step<'_>, who: usize, kind: &str) -> bool {
    let w = who as i32;
    step.events
        .iter()
        .any(|e| e.r#type == "card" && e.player_id == w && (kind.is_empty() || e.kind == kind))
}

/// Act was `who` pressing a skill.
fn act_is_skill(step: &Step<'_>, who: usize) -> bool {
    matches!(step.act, StepAct::Act { msg, .. } if msg.act == "skill" && step.player_of_act() == who)
}

/// Card id `who` tried to play from hand, if this act is a play.
fn act_played_card(step: &Step<'_>, who: usize) -> Option<String> {
    match step.act {
        StepAct::Act { msg, .. } if msg.act == "play" && step.player_of_act() == who => {
            Some(msg.card.clone())
        }
        _ => None,
    }
}

/// Card-text exception 「可在眩晕时打出」 / 「可在晕眩时打出」.
fn playable_stunned(step: &Step<'_>, card: &str) -> bool {
    step.data.card(card).is_some_and(|c| text_allows_stunned_use(&c.text))
}

/// Does `text` print a stun-use waiver? The book spells 眩晕 and 晕眩
/// interchangeably and the sheet brackets the status (「[晕眩]」 / 「[眩晕]」).
fn text_allows_stunned_use(text: &str) -> bool {
    for stun in ["晕眩", "眩晕"] {
        for form in [
            format!("在[{stun}]状态下使用"),
            format!("在{stun}状态下使用"),
            format!("在[{stun}]时使用"),
            format!("可在{stun}时打出"),
            format!("可在眩晕时打出"),
            format!("可在晕眩时打出"),
        ] {
            if text.contains(&form) {
                return true;
            }
        }
        // 「此技能可以正常使用或在[晕眩]状态下使用」 -- the bracketed form.
        if text.contains(&format!("[{stun}]状态下使用")) || text.contains(&format!("[{stun}]时使用"))
        {
            return true;
        }
    }
    false
}

/// Skill-text exception 「在[晕眩]状态下使用」 / 「可在眩晕时使用」.
fn skill_ok_stunned(step: &Step<'_>, skill: &str) -> bool {
    if let Some(c) = step.data.card(skill) {
        if text_allows_stunned_use(&c.text) {
            return true;
        }
    }
    if let Some(rest) = skill.strip_prefix("skill:") {
        if let Some((owner, _)) = rest.split_once(':') {
            if let Some(c) = step.data.persona(owner) {
                if text_allows_stunned_use(&c.text) {
                    return true;
                }
            }
            if let Some(b) = step.data.groups.iter().find(|b| b.name == owner) {
                if text_allows_stunned_use(&b.text) {
                    return true;
                }
            }
        }
    }
    false
}

/// Money-movement event types this area watches.
fn is_money_leg(e: &MatchEvent) -> bool {
    matches!(e.r#type.as_str(), "pay" | "rent" | "gain" | "lose")
}

/// Skip windows the book's status gates cannot close over (bankrupt / auction).
fn skip_money_window(events: &[MatchEvent]) -> bool {
    events
        .iter()
        .any(|e| e.r#type == "bankrupt" || e.r#type == "auction")
}



// ---------------------------------------------------------------------------
// G-STATUS-01 -- layer counts
// ---------------------------------------------------------------------------

/// Stay / stun / exile layers are never negative.
///
/// * **ID:** G-STATUS-01
/// * **Source:** `rulebook-doc.md:96-98`
/// * **Quote:** 「[停留]：…重复受到该效果时可叠加，玩家的每回合结束时移除一层。」
///   「[晕眩]：…重复受到该效果时可叠加，玩家的每回合结束时移除一层。」
///   「[除外]：…该效果生效时无法重复获得该效果，玩家的每回合开始前移除一层。」
/// * **Trigger:** every observed input, on the settled `post` world.
/// * **Condition:** the engine's `stay` / `stun` / `stunStart` / `exile`
///   counters.
/// * **Effect:** every such counter is `>= 0`.
/// * **Frame:** n/a (pure read).
/// * **Rulings & OPEN:** "可叠加" + "移除一层" makes the counters layer counts;
///   a layer count that went negative would mean a decay ran one too many
///   times. `stunStart` is the engine's [晕眩] variant that expires at turn
///   start (`Tick::TurnStart`); the book only names one [晕眩], so the mapping
///   is engine-side. What each status *forbids* is G-STATUS-02+.
/// * **Executable:** [`g_status_01`].
const G_STATUS_01: Clause = Clause::new(
    "G-STATUS-01",
    "rulebook-doc.md:96-98",
    "[停留]/[晕眩]：重复受到该效果时可叠加，玩家的每回合结束时移除一层。[除外]：该效果生效时无法重复获得该效果，玩家的每回合开始前移除一层。",
    g_status_01,
);

/// Executable form of [`G_STATUS_01`].
pub fn g_status_01(step: &Step<'_>) -> Result<(), String> {
    verify_layers_nonneg(step.post)
}

// ---------------------------------------------------------------------------
// G-STATUS-02 -- [无法移动]
// ---------------------------------------------------------------------------

/// [无法移动]: main-move destination is the current tile with no settle, and
/// non-[传送] movement effects cannot be used.
///
/// * **ID:** G-STATUS-02
/// * **Source:** `rulebook-doc.md:95`
/// * **Quote:** 「[无法移动]：处于该状态时[主要移动]的[移动终点]视为当前所在格子且不[结算]。无法使用非[传送]的移动效果（如果因效果随机到非[传送]的移动效果则抵消该效果）。」
/// * **Trigger:** a `roll` (the [主要移动]) by a player who is [无法移动],
///   or a non-[传送] movement effect targeting them.
/// * **Condition:** in `pre`, the player has [停留] (unless [不可阻挡]),
///   [晕眩], or [除外] -- any of which grants [无法移动] (items 11-13).
/// * **Effect:** the [主要移动] does not leave the current tile and does not
///   [结算]; a non-[传送] movement effect is cancelled.
/// * **Frame:** n/a (the move is a no-op).
/// * **Rulings & OPEN:** the 行动阶段 rows phrase the same fact as a skip
///   (row 7 「停留效果发动：跳过主要移动阶段」). A refused `roll` / a skipped
///   move phase is the observable form of 「[移动终点]视为当前所在格子且不
///   [结算]」. [传送] is *not* blocked by this item (the second sentence only
///   bars non-[传送] effects).
/// * **Executable:** [`g_status_02`].
const G_STATUS_02: Clause = Clause::new(
    "G-STATUS-02",
    "rulebook-doc.md:95",
    "[无法移动]：处于该状态时[主要移动]的[移动终点]视为当前所在格子且不[结算]。无法使用非[传送]的移动效果（如果因效果随机到非[传送]的移动效果则抵消该效果）。",
    g_status_02,
);

/// Executable form of [`G_STATUS_02`].
///
/// Fires when the player is [无法移动] and either (a) the engine holds the
/// skip latch (`skip_move`), or (b) the status is [晕眩] / [除外] (present
/// tense, even mid-运营). A [停留] gained *mid-turn* does not raise the latch
/// (TEST-FINDINGS §6 / glossary 51, OPEN-停留-中途) and is left alone here.
pub fn g_status_02(step: &Step<'_>) -> Result<(), String> {
    let who = step.player_of_act();
    let is_roll = matches!(step.act, StepAct::Act { msg, .. } if msg.act == "roll");
    verify_main_move_skipped(step.pre, step.post, step.events, who, is_roll)
}

// ---------------------------------------------------------------------------
// G-STATUS-03 -- [停留]
// ---------------------------------------------------------------------------

/// [停留] grants [无法移动], stacks, and loses one layer at end of the
/// player's own turn.
///
/// * **ID:** G-STATUS-03
/// * **Source:** `rulebook-doc.md:96`
/// * **Quote:** 「[停留]：处于该状态时[无法移动]。重复受到该效果时可叠加，玩家的每回合结束时移除一层。」
/// * **Trigger:** any act; on turn end of the layer's owner.
/// * **Condition:** `stay` layers on that player.
/// * **Effect:** while `stay > 0` the player is [无法移动]; repeated grants
///   add layers; at end of their own turn one layer is removed.
/// * **Frame:** only `state:P*:stay` moves on a pure decay.
/// * **Rulings & OPEN:** 行动阶段 row 7 (「停留效果发动：跳过主要移动阶段」)
///   is the skip form of the [无法移动] consequence (see G-STATUS-02). A
///   grant logged in the same window as the decay may offset the -1 (the
///   engine's turnEndAfter 「[停留] they grant survives this turn's wear-off」).
///   A [清除] that zeros layers is a different clause (card text).
/// * **Executable:** [`g_status_03`].
const G_STATUS_03: Clause = Clause::new(
    "G-STATUS-03",
    "rulebook-doc.md:96",
    "[停留]：处于该状态时[无法移动]。重复受到该效果时可叠加，玩家的每回合结束时移除一层。",
    g_status_03,
);

/// Executable form of [`G_STATUS_03`].
pub fn g_status_03(step: &Step<'_>) -> Result<(), String> {
    for i in 0..step.pre.st.players.len() {
        verify_stay_turn_end(step.pre, step.post, step.events, i)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-STATUS-04 -- [晕眩] body
// ---------------------------------------------------------------------------

/// [晕眩] body: [无法移动], no [主动] skills, no hand-card [手] effects, no
/// paying or receiving.
///
/// * **ID:** G-STATUS-04
/// * **Source:** `rulebook-doc.md:97`
/// * **Quote:** 「[晕眩]：处于该状态时[无法移动]，无法使用[主动]技能和手卡的[手]效果，无法收付款。」
/// * **Trigger:** a skill press or hand-card play by a stunned player, or a
///   money leg to / from one.
/// * **Condition:** `stun + stunStart > 0` on that player in `pre`.
/// * **Effect:** the skill / [手] play does not run; no money moves to or from
///   them (the pay is blocked).
/// * **Frame:** n/a (the act is a no-op).
/// * **Rulings & OPEN:** cards that print 「可在眩晕时打出」 (e.g. MyGO:壱雫空)
///   and skills that print 「在[晕眩]状态下使用」 are card-text exceptions and
///   are out of scope of this global clause. OPEN-晕眩-自得 (TEST-FINDINGS §6.9):
///   does 「无法收付款」 also block a card's own 「获得」 to the player that same
///   card body just stunned? Engine now lets the gain land.
/// * **Executable:** [`g_status_04`].
const G_STATUS_04: Clause = Clause::new(
    "G-STATUS-04",
    "rulebook-doc.md:97",
    "[晕眩]：处于该状态时[无法移动]，无法使用[主动]技能和手卡的[手]效果，无法收付款。",
    g_status_04,
);

/// Executable form of [`G_STATUS_04`].
pub fn g_status_04(step: &Step<'_>) -> Result<(), String> {
    let who = step.player_of_act();
    let stunned_pre = stun_layers(step.pre, who) > 0;
    if stunned_pre {
        if act_is_skill(step, who) {
            let skill = match step.act {
                StepAct::Act { msg, .. } => msg.card.clone(),
                _ => String::new(),
            };
            if !skill_ok_stunned(step, &skill) && activated(step, who, "skill") {
                return Err(format!(
                    "G-STATUS-04: stunned player {who} used [主动] skill {skill:?}"
                ));
            }
        }
        if let Some(card) = act_played_card(step, who) {
            if !playable_stunned(step, &card) && activated(step, who, "play") {
                return Err(format!(
                    "G-STATUS-04: stunned player {who} played hand card {card:?} ([手])"
                ));
            }
        }
    }
    for i in 0..step.pre.st.players.len() {
        let pre_stunned = stun_layers(step.pre, i) > 0;
        if pre_stunned {
            verify_no_money_status(step.pre, step.post, step.events, i, MoneyGate::Stun)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-STATUS-05 -- [晕眩] stacking, decay, and the 行动阶段 skip
// ---------------------------------------------------------------------------

/// [晕眩] stacks, loses one layer at end of the player's turn, and skips
/// 运营 and the main move (行动阶段 row 4).
///
/// * **ID:** G-STATUS-05
/// * **Source:** `rulebook-doc.md:97` + `rulebook-doc.md:204`
/// * **Quote:** 「重复受到该效果时可叠加，玩家的每回合结束时移除一层。」
///   「眩晕效果发动：跳过经营和主要移动阶段」
/// * **Trigger:** end of the stunned player's turn; their turn start while
///   stunned.
/// * **Condition:** `stun` layers.
/// * **Effect:** one layer removed at end of their own turn; their turn skips
///   运营/出牌 and 主要移动 (the whole active window).
/// * **Frame:** only `state:P*:stun` / `stunStart` move on a pure decay.
/// * **Rulings & OPEN:** the book's 眩晕 = 晕眩. `stunStart` is the engine's
///   timed variant (expires at turn start); same non-negativity and same
///   skip. The auto-skip is logged as `log.stunned_skip`.
/// * **Executable:** [`g_status_05`].
const G_STATUS_05: Clause = Clause::new(
    "G-STATUS-05",
    "rulebook-doc.md:97",
    "[晕眩]：重复受到该效果时可叠加，玩家的每回合结束时移除一层。眩晕效果发动：跳过经营和主要移动阶段",
    g_status_05,
);

/// Executable form of [`G_STATUS_05`].
pub fn g_status_05(step: &Step<'_>) -> Result<(), String> {
    for i in 0..step.pre.st.players.len() {
        verify_stun_turn_end(step.pre, step.post, step.events, i)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-STATUS-06 -- [除外] body
// ---------------------------------------------------------------------------

/// [除外] body: [无法移动], cannot be [指定], no [主动]/[手], no pay/receive;
/// the pawn is not on any tile.
///
/// * **ID:** G-STATUS-06
/// * **Source:** `rulebook-doc.md:98`
/// * **Quote:** 「[除外]：处于该状态时[无法移动]，不能被[指定]，无法使用[主动]技能和手卡的[手]效果，无法收付款；玩家棋子不失为存在于任何格子上。」
/// * **Trigger:** a skill press / hand play by an exiled player, a [指定]
///   targeting them, or a money leg to / from them.
/// * **Condition:** `exile > 0` on that player in `pre`.
/// * **Effect:** the skill / [手] play does not run; they are not a legal
///   [指定] target; no money moves to or from them; the pawn counts as on no
///   tile.
/// * **Frame:** n/a (the act is a no-op).
/// * **Rulings & OPEN:** 「不失为」 is the book's spelling of 「不视为」. The
///   「不能被[指定]」 half is a target-legality rule the engine surfaces as a
///   refused pick -- scenario-tested; the per-act half is the skill / [手] /
///   money gate. The engine routes tile income of an exiled player to a
///   redirect (无路矢); the general 「无法收付款」 still says they do not
///   receive it.
/// * **Executable:** [`g_status_06`].
const G_STATUS_06: Clause = Clause::new(
    "G-STATUS-06",
    "rulebook-doc.md:98",
    "[除外]：处于该状态时[无法移动]，不能被[指定]，无法使用[主动]技能和手卡的[手]效果，无法收付款；玩家棋子不失为存在于任何格子上。",
    g_status_06,
);

/// Executable form of [`G_STATUS_06`].
pub fn g_status_06(step: &Step<'_>) -> Result<(), String> {
    let who = step.player_of_act();
    if layers(step.pre, who, EXILE) > 0 {
        if act_is_skill(step, who) && activated(step, who, "skill") {
            return Err(format!(
                "G-STATUS-06: exiled player {who} used a [主动] skill"
            ));
        }
        if let Some(card) = act_played_card(step, who) {
            if activated(step, who, "play") {
                return Err(format!(
                    "G-STATUS-06: exiled player {who} played hand card {card:?} ([手])"
                ));
            }
        }
    }
    for i in 0..step.pre.st.players.len() {
        let pre_exiled = layers(step.pre, i, EXILE) > 0;
        if pre_exiled {
            verify_no_money_status(step.pre, step.post, step.events, i, MoneyGate::Exile)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-STATUS-07 -- [除外] does not re-stack; decays before turn start
// ---------------------------------------------------------------------------

/// [除外] cannot be re-acquired while active; one layer is removed before the
/// player's turn start.
///
/// * **ID:** G-STATUS-07
/// * **Source:** `rulebook-doc.md:98` + `rulebook-doc.md:201`
/// * **Quote:** 「该效果生效时无法重复获得该效果，玩家的每回合开始前移除一层。」
///   「负面效果：移除一层[除外]」
/// * **Trigger:** a grant of [除外] while already exiled; the player's own
///   turn start.
/// * **Condition:** `exile > 0` in `pre`.
/// * **Effect:** a second grant does not raise the counter; at their turn
///   start (before the turn proper) one layer is removed.
/// * **Frame:** only `state:P*:exile` / `exileTo` move on a pure decay.
/// * **Rulings & OPEN:** one effect may still grant several layers at once
///   (MyGO:无路矢 「获得2层[除外]」) -- 「无法重复获得」 bars a *second*
///   acquisition while the first is active, not a multi-layer first grant.
///   A grant after the decay leaves 0 is a fresh acquisition.
/// * **Executable:** [`g_status_07`].
const G_STATUS_07: Clause = Clause::new(
    "G-STATUS-07",
    "rulebook-doc.md:98",
    "[除外]：该效果生效时无法重复获得该效果，玩家的每回合开始前移除一层。",
    g_status_07,
);

/// Executable form of [`G_STATUS_07`].
pub fn g_status_07(step: &Step<'_>) -> Result<(), String> {
    for i in 0..step.pre.st.players.len() {
        verify_exile_turn_start(step.pre, step.post, step.events, i)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-STATUS-08 -- [除外] stops skills and [持续]; return [传送]
// ---------------------------------------------------------------------------

/// While [除外], player skills and [场地] [持续] effects stop; the granting
/// effect names the return [传送] tile.
///
/// * **ID:** G-STATUS-08
/// * **Source:** `rulebook-doc.md:98`
/// * **Quote:** 「处于该状态时玩家技能和[场地]上的[持续]效果停止生效。获得该状态的效果会写明状态结束后在玩家回合开始时要[传送]到的格子。」
/// * **Trigger:** any window in which an exiled player would fire a skill /
///   [持续] hook, and the turn start where their [除外] ticks out.
/// * **Condition:** `exile > 0` in `pre` (or ticking to 0 at this turn start).
/// * **Effect:** no player-skill or tile-[持续] body runs on their behalf;
///   when the last layer ticks out, they [传送] to the tile the granting
///   effect wrote (`exileTo`).
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** the book states a convention on the *granting*
///   effect's text (it must name the return tile), not a constraint the
///   engine can fail; the checkable half is the return [传送] on expiry. A
///   grant with `exileTo < 0` has no named tile -- OPEN-除外-归位 for whether
///   that is legal.
/// * **Executable:** [`g_status_08`].
const G_STATUS_08: Clause = Clause::new(
    "G-STATUS-08",
    "rulebook-doc.md:98",
    "处于该状态时玩家技能和[场地]上的[持续]效果停止生效。获得该状态的效果会写明状态结束后在玩家回合开始时要[传送]到的格子。",
    g_status_08,
);

/// Executable form of [`G_STATUS_08`].
pub fn g_status_08(step: &Step<'_>) -> Result<(), String> {
    for i in 0..step.pre.st.players.len() {
        verify_exile_return(step.pre, step.post, step.events, i)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-STATUS-09 -- [不可阻挡]
// ---------------------------------------------------------------------------

/// [不可阻挡]: ignores existing [停留]; gains no new [晕眩] / [除外] layers;
/// existing layers still decay; may choose whether [传送] / [强制移动] /
/// [强制停下] apply.
///
/// * **ID:** G-STATUS-09
/// * **Source:** `rulebook-doc.md:99`
/// * **Quote:** 「[不可阻挡]：处于该状态时无视已存在的[停留]，[晕眩]效果也不会获得新的层数；无法获得新的[除外]层数。[停留]，[晕眩]，[除外]在适当时机依旧掉层。处于该状态时可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效。」
/// * **Trigger:** any act while `unstoppable > 0`.
/// * **Condition:** `unstoppable > 0` in `pre`.
/// * **Effect:** existing [停留] does not skip the move; no new [晕眩] or
///   [除外] layers are gained; existing layers still tick at their book
///   timing; [传送]/[强制移动]/[强制停下] are optional to the receiver.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** the choice half (「可选择…是否生效」) is a player
///   decision -- not a fixed post-condition, so this clause checks the
///   *prohibition* half (no new layers) and the *non-skip* half. Layer decay
///   is G-STATUS-03 / 05 / 07 (「依旧掉层」).
/// * **Executable:** [`g_status_09`].
const G_STATUS_09: Clause = Clause::new(
    "G-STATUS-09",
    "rulebook-doc.md:99",
    "[不可阻挡]：处于该状态时无视已存在的[停留]，[晕眩]效果也不会获得新的层数；无法获得新的[除外]层数。[停留]，[晕眩]，[除外]在适当时机依旧掉层。",
    g_status_09,
);

/// Executable form of [`G_STATUS_09`].
pub fn g_status_09(step: &Step<'_>) -> Result<(), String> {
    for i in 0..step.pre.st.players.len() {
        verify_unstoppable(step.pre, step.post, step.events, i)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-STATUS-10 -- [异常移动效果] set
// ---------------------------------------------------------------------------

/// [异常移动效果] is exactly {传送, 停留, 晕眩, 除外, 强制移动, 强制停下,
/// reverse-direction movement}.
///
/// * **ID:** G-STATUS-10
/// * **Source:** `rulebook-doc.md:100`
/// * **Quote:** 「[异常移动效果]：包括[传送]，[停留]，[晕眩]，[除外]，[强制移动]，[强制停下]，反方向移动。」
/// * **Trigger:** any window that names an [异常移动效果] (e.g. a card that
///   says 「不受异常移动效果影响」 or 「受到[除外]以外的异常移动效果」).
/// * **Condition:** n/a -- this is a set-membership definition.
/// * **Effect:** the set is exactly the seven named members. Nothing outside
///   it is an [异常移动效果]; every named member is one.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** the book uses 「包括」 (includes), which could be
///   open-ended. Read with the closed list that follows and with R:选择自己的
///   舞台's 「[除外]以外的异常移动效果」 (which only makes sense as a closed
///   set), the set is exactly these seven. OPEN-异常移动效果-封闭 if a later
///   sheet adds members.
/// * **Executable:** [`g_status_10`].
const G_STATUS_10: Clause = Clause::new(
    "G-STATUS-10",
    "rulebook-doc.md:100",
    "[异常移动效果]：包括[传送]，[停留]，[晕眩]，[除外]，[强制移动]，[强制停下]，反方向移动。",
    g_status_10,
);

/// The seven members of [异常移动效果], in the book's order.
pub const ABNORMAL_MOVE_EFFECTS: [&str; 7] = [
    "[传送]",
    "[停留]",
    "[晕眩]",
    "[除外]",
    "[强制移动]",
    "[强制停下]",
    "反方向移动",
];

/// Executable form of [`G_STATUS_10`].
///
/// This is a definition, not a transition constraint: the checkable fact is
/// that the named set is the one the book lists. A transition can only violate
/// it by treating some other effect as an [异常移动效果] (or excluding a named
/// one), which is card-text work. The predicate holds unless
/// [`ABNORMAL_MOVE_EFFECTS`] has drifted from the book's seven.
pub fn g_status_10(_step: &Step<'_>) -> Result<(), String> {
    const EXPECTED: [&str; 7] = [
        "[传送]",
        "[停留]",
        "[晕眩]",
        "[除外]",
        "[强制移动]",
        "[强制停下]",
        "反方向移动",
    ];
    if ABNORMAL_MOVE_EFFECTS != EXPECTED {
        return Err(format!(
            "G-STATUS-10: [异常移动效果] set drifted: {ABNORMAL_MOVE_EFFECTS:?}"
        ));
    }
    Ok(())
}


// ---------------------------------------------------------------------------
// Pure verifiers (iff). Clauses delegate to these; `g_status.rs` drives them
// and applies mutants. Signature is (pre, post, events, ...) so a test can
// build a triple without a live `Step`.
// ---------------------------------------------------------------------------

/// G-STATUS-01: every layer counter in `post` is `>= 0`.
pub fn verify_layers_nonneg(post: &World) -> Result<(), String> {
    const KEYS: [&str; 4] = [STAY, STUN, STUN_START, EXILE];
    for (i, p) in post.st.players.iter().enumerate() {
        for k in KEYS {
            let v = p.state_get(k);
            if v < 0 {
                return Err(format!("player {i} {k}={v} is negative"));
            }
        }
    }
    Ok(())
}

/// G-STATUS-02: a [主要移动] `roll` by a [无法移动] player (skip latch or
/// hard stun/exile) stays on the current tile and does not settle.
pub fn verify_main_move_skipped(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
    is_roll: bool,
) -> Result<(), String> {
    if !cannot_move(pre, who) {
        return Ok(());
    }
    let hard = stun_layers(pre, who) > 0 || layers(pre, who, EXILE) > 0;
    if !hard && !pre.st.skip_move {
        return Ok(()); // OPEN-停留-中途
    }
    if !is_roll {
        return Ok(());
    }
    let pre_pos = pre.st.players[who].pos;
    let post_pos = post.st.players[who].pos;
    if post_pos != pre_pos {
        return Err(format!(
            "G-STATUS-02: player {who} is [无法移动] but [主要移动] landed on {post_pos} (起点 {pre_pos})"
        ));
    }
    for e in events {
        let k = msg_key(e);
        if k.starts_with("land")
            || k.starts_with("settle")
            || k == "settle"
            || k == "settleAfter"
            || k == "settleBefore"
            || k == "settleBody"
        {
            return Err(format!(
                "G-STATUS-02: player {who} is [无法移动] but the move settled ({k})"
            ));
        }
    }
    Ok(())
}

/// G-STATUS-03: [停留] loses exactly one layer at the owner's turn end (quiet
/// window), never stacks illegally, and is unchanged otherwise.
pub fn verify_stay_turn_end(pre: &World, post: &World, events: &[MatchEvent], who: usize) -> Result<(), String> {
    let before = layers(pre, who, STAY);
    let after = layers(post, who, STAY);
    let g = grants(events, who, STAY);
    if !turn_ended(events, who) {
        // Condition false: this rule must be a no-op on the counter. A shrink
        // here would be a decay firing off its tick (LESS/WRONG).
        if after < before && g == 0 && !status_cleared(events) {
            return Err(format!(
                "G-STATUS-03: player {who} [停留] shrank off its tick ({before} -> {after})"
            ));
        }
        return Ok(());
    }
    if status_cleared(events) || busy_window(events) || g > 0 {
        // `g` is a grant *line* count, not a layer total (`give_stay(p, 2)`
        // logs once). Do not invent a bound from it. Exact decay is the
        // quiet-window case below; the clean scenarios pin multi-layer grants.
        return Ok(());
    }
    // 「玩家的每回合结束时移除一层」 -- exactly one layer off, floor 0.
    let expected = (before - 1).max(0);
    if after != expected {
        return Err(format!(
            "G-STATUS-03: player {who} [停留] {before} -> {after} over their turn end (expected {expected})"
        ));
    }
    Ok(())
}

/// G-STATUS-04 / 06 money half: no pay/rent/gain/lose leg to or from a
/// stunned / exiled seat (per-leg amount, not net).
/// Which status's 「无法收付款」 gate a caller is checking.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MoneyGate {
    /// [晕眩] (`stun` / `stunStart`).
    Stun,
    /// [除外] (`exile`).
    Exile,
}

pub fn verify_no_money_status(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
    gate: MoneyGate,
) -> Result<(), String> {
    // The gate is derived from `pre` -- the caller cannot hide a leg by
    // passing a false flag.
    let gated = match gate {
        MoneyGate::Stun => stun_layers(pre, who) > 0,
        MoneyGate::Exile => layers(pre, who, EXILE) > 0,
    };
    let _ = post;
    if !gated {
        return Ok(());
    }
    if skip_money_window(events) {
        return Ok(());
    }
    for e in events {
        if !is_money_leg(e) {
            continue;
        }
        let amount = e.value as i64;
        if amount == 0 {
            continue;
        }
        for s in [e.player_id, e.other] {
            if s < 0 || s as usize != who {
                continue;
            }
            if e.r#type == "gain"
                && (grants(events, who, STUN) > 0 || grants(events, who, EXILE) > 0)
            {
                continue; // OPEN-晕眩-自得
            }
            let side = if e.player_id == who as i32 && e.r#type != "gain" {
                "payer"
            } else {
                "payee"
            };
            let id = match gate {
                MoneyGate::Stun => "G-STATUS-04",
                MoneyGate::Exile => "G-STATUS-06",
            };
            return Err(format!(
                "{id}: player {who} {side} on a {} leg of {amount} while gated",
                e.r#type
            ));
        }
    }
    Ok(())
}

/// G-STATUS-05: [晕眩] loses exactly one layer at the owner's turn end (quiet),
/// and a turn that starts while still stunned auto-skips (row 4).
pub fn verify_stun_turn_end(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
) -> Result<(), String> {
    let before = layers(pre, who, STUN);
    let after = layers(post, who, STUN);
    let g = grants(events, who, STUN);
    if turn_ended(events, who) && !status_cleared(events) && !busy_window(events) && g == 0 {
        let expected = (before - 1).max(0);
        if after != expected {
            return Err(format!(
                "G-STATUS-05: player {who} [晕眩] {before} -> {after} over their turn end (expected {expected})"
            ));
        }
    }
    let stunned_at_start = stun_layers(pre, who) > 0;
    if turn_started(events, who) && stunned_at_start {
        let skipped = events.iter().any(|e| {
            e.r#type == "status" && e.player_id == who as i32 && msg_key(e) == "stunned_skip"
        });
        if !skipped && stun_layers(post, who) > 0 {
            return Err(format!(
                "G-STATUS-05: stunned player {who} started a turn without the 运营/主要移动 skip (row 4)"
            ));
        }
    }
    Ok(())
}

/// G-STATUS-07: [除外] never re-stacks while active; loses exactly one layer
/// before the owner's turn start (quiet).
pub fn verify_exile_turn_start(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
) -> Result<(), String> {
    let before = layers(pre, who, EXILE);
    let after = layers(post, who, EXILE);
    let g = grants(events, who, EXILE);
    if before > 0 && g > 0 {
        return Err(format!(
            "G-STATUS-07: player {who} re-acquired [除外] while active (pre {before}, post {after}, grant-lines {g})"
        ));
    }
    if turn_started(events, who) && !status_cleared(events) && !busy_window(events) && g == 0 {
        let expected = (before - 1).max(0);
        if after != expected {
            return Err(format!(
                "G-STATUS-07: player {who} [除外] {before} -> {after} over their turn start (expected {expected})"
            ));
        }
    }
    Ok(())
}

/// G-STATUS-08: when [除外] ticks out at turn start, the pawn lands on the
/// named return tile (`exileTo` present and `>= 0`).
pub fn verify_exile_return(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
) -> Result<(), String> {
    let before = layers(pre, who, EXILE);
    let after = layers(post, who, EXILE);
    if before == 0 || !turn_started(events, who) || after != 0 {
        return Ok(());
    }
    let has_to = pre
        .st
        .players
        .get(who)
        .is_some_and(|p| p.state.contains_key(EXILE_TO));
    let to = layers(pre, who, EXILE_TO);
    if !has_to || to < 0 {
        return Ok(()); // OPEN-除外-归位
    }
    let pos = post.st.players.get(who).map(|p| p.pos).unwrap_or(-1);
    if pos != to {
        return Err(format!(
            "G-STATUS-08: player {who} [除外] expired but landed on {pos}, expected the named tile {to}"
        ));
    }
    Ok(())
}

/// G-STATUS-09: while [不可阻挡], no new [晕眩] / [除外] layers and no [停留]
/// skip.
pub fn verify_unstoppable(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
) -> Result<(), String> {
    if layers(pre, who, UNSTOPPABLE) <= 0 {
        return Ok(());
    }
    for (key, label) in [(STUN, "[晕眩]"), (STUN_START, "[晕眩]"), (EXILE, "[除外]")] {
        let b = layers(pre, who, key);
        let a = layers(post, who, key);
        let g = grants(events, who, key);
        if g > 0 {
            return Err(format!(
                "G-STATUS-09: unstoppable player {who} gained {label} (pre {b}, post {a}, grant-lines {g})"
            ));
        }
        if a > b {
            return Err(format!(
                "G-STATUS-09: unstoppable player {who} gained {label} layers ({b} -> {a})"
            ));
        }
    }
    if stay_skip(events, who) {
        return Err(format!(
            "G-STATUS-09: unstoppable player {who} was skipped for [停留]"
        ));
    }
    Ok(())
}

