//! Whole-[`World`] snapshot and diff, plus a frame condition.
//!
//! Generalised from `crates/game-rules/tests/q4/snap.rs` (which stays
//! untouched): the footprints `surface_of` mapping is game-specific and does
//! not live here. What stays is the stable `path -> value` view of every
//! observable world field, the bookkeeping filter, and [`frame`].
//!
//! Black-box: only `game_core`'s public `World` is read.

use std::collections::{BTreeMap, BTreeSet};

use game_core::engine::World;
use game_core::state::BOARD_OWNER;

/// A flattened view of every observable world field, `path -> value`.
///
/// Paths are stable strings so two snapshots can be diffed with a plain
/// `BTreeMap` compare. Volatile bookkeeping (event tail, rng stream, prompt
/// ids, previews) is still captured -- the *diff* filter drops it via
/// [`is_bookkeeping`].
#[derive(Clone, Debug, Default)]
pub struct Snap {
    pub map: BTreeMap<String, String>,
}

impl Snap {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.map.get(k).map(String::as_str)
    }
}

/// Capture the whole world.
pub fn snapshot(w: &World) -> Snap {
    let mut m: BTreeMap<String, String> = BTreeMap::new();

    // ---- players ----------------------------------------------------------
    for (i, p) in w.st.players.iter().enumerate() {
        m.insert(format!("money:P{i}"), p.money.to_string());
        m.insert(format!("pos:P{i}"), p.pos.to_string());
        m.insert(format!("roll:P{i}"), p.roll.to_string());
        m.insert(format!("out:P{i}"), p.out_order.to_string());
        m.insert(format!("bankrupt:P{i}"), p.bankrupt.to_string());
        m.insert(format!("left:P{i}"), p.left.to_string());
        m.insert(format!("assets:P{i}"), p.assets.to_string());
        m.insert(format!("persona:P{i}"), p.persona.clone());
        m.insert(format!("hand_n:P{i}"), p.hand.to_string());
        m.insert(format!("draw_n:P{i}"), p.draw.to_string());
        m.insert(format!("discard_pub:P{i}"), p.discard.join("|"));

        for (k, v) in &p.state {
            m.insert(
                format!("state:P{i}:{k}"),
                format!("{}|{}|{}|{:?}", v.value, v.min, v.max, v.expires),
            );
        }
        for (t, c) in p.tokens.iter().enumerate() {
            m.insert(format!("token:P{i}:{t}:{}", c.name), c.value.to_string());
        }
        for (f, c) in p.field.iter().enumerate() {
            m.insert(
                format!("field:P{i}:{f}"),
                format!(
                    "{}|uid={}|tile={}|pool={}|immune={}|down={}|group={}",
                    c.card, c.uid, c.tile, c.counters.values().sum::<i32>(), c.immune, c.face_down, c.group_skill
                ),
            );
            m.insert(format!("pool:P{i}:{}", c.card), c.counters.values().sum::<i32>().to_string());
            for (k, v) in &c.props {
                m.insert(format!("fprop:P{i}:{}:{k}", c.card), v.to_string());
            }
        }
        for (h, c) in w
            .hidden
            .get(i)
            .map(|h| h.hand.clone())
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            m.insert(format!("hand:P{i}:{h}"), c.clone());
        }
        for (h, c) in w
            .hidden
            .get(i)
            .map(|h| h.draw.clone())
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            m.insert(format!("draw:P{i}:{h}"), c.clone());
        }
        for (h, c) in w
            .hidden
            .get(i)
            .map(|h| h.discard.clone())
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            m.insert(format!("discard:P{i}:{h}"), c.clone());
        }
        if let Some(ns) = w.hidden.get(i).and_then(|h| h.next_steps) {
            m.insert(format!("next_steps:P{i}"), ns.to_string());
        } else {
            m.insert(format!("next_steps:P{i}"), "-".into());
        }
    }

    // ---- board ------------------------------------------------------------
    let n = w.st.owners.len();
    for t in 0..n {
        m.insert(format!("owner:T{t}"), w.st.owners[t].to_string());
        m.insert(format!("houses:T{t}"), w.st.houses[t].to_string());
        m.insert(format!("mortgaged:T{t}"), w.st.mortgaged[t].to_string());
        if t < w.st.embers.len() {
            m.insert(format!("ember:T{t}"), w.st.embers[t].to_string());
        }
    }
    for (i, mk) in w.st.marks.iter().enumerate() {
        m.insert(
            format!("mark:{i}"),
            format!(
                "tile={}|kind={}|owner={}|count={}|card={}",
                mk.tile, mk.kind, mk.owner, mk.count, mk.card
            ),
        );
    }

    // ---- board field (tile rule instances) --------------------------------
    for (i, c) in w.st.board_field.iter().enumerate() {
        m.insert(
            format!("bfield:{i}"),
            format!(
                "{}|uid={}|tile={}|pool={}|immune={}|down={}",
                c.card, c.uid, c.tile, c.counters.values().sum::<i32>(), c.immune, c.face_down
            ),
        );
        for (k, v) in &c.props {
            m.insert(format!("bprop:{i}:{k}"), v.to_string());
        }
    }

    // ---- turn ctx / world bookkeeping we care about -----------------------
    let t = &w.turn;
    m.insert("turn.player".into(), t.player_id.to_string());
    m.insert("turn.extra".into(), t.extra.to_string());
    m.insert("turn.main_moved".into(), t.main_moved.to_string());
    m.insert("turn.played".into(), t.played.join("|"));
    m.insert("turn.no_money_loss".into(), format!("{:?}", t.no_money_loss));
    m.insert("turn.fixed_roll".into(), format!("{:?}", t.fixed_roll));
    m.insert("turn.main_steps".into(), t.main_steps.to_string());
    m.insert("turn.abnormal".into(), format!("{:?}", t.abnormal));
    m.insert("turn.extreme".into(), t.extreme.to_string());
    m.insert("turn.play_from_hand".into(), t.play_from_hand.to_string());
    m.insert("turn.paid_in_settle".into(), t.paid_in_settle.to_string());
    m.insert("turn.turn_start_pos".into(), format!("{:?}", t.turn_start_pos));
    m.insert("turn.turn_snap".into(), format!("{:?}", t.turn_snap));
    m.insert("turn.turn_rolls".into(), format!("{:?}", t.turn_rolls));
    m.insert("turn.build_discount".into(), t.build_discount.to_string());
    m.insert(
        "turn.build_discount_layers".into(),
        t.build_discount_layers.to_string(),
    );
    m.insert("turn.build_cost_pct".into(), t.build_cost_pct.to_string());
    m.insert(
        "turn.lingering".into(),
        t.lingering
            .iter()
            .map(|l| format!("{}@{}:{}:{:?}", l.card, l.owner, l.expires, l.props))
            .collect::<Vec<_>>()
            .join("|"),
    );

    m.insert("extra_turns".into(), format!("{:?}", w.extra_turns));
    m.insert("rule_param_bonus".into(), format!("{:?}", w.rule_param_bonus));
    m.insert("out_count".into(), w.out_count.to_string());
    m.insert("targeted".into(), format!("{:?}", w.targeted));
    m.insert(
        "scheduled".into(),
        w.scheduled
            .iter()
            .map(|s| format!("{}@{}:{}:{}", s.card, s.owner, s.target, s.uid))
            .collect::<Vec<_>>()
            .join("|"),
    );
    m.insert("next_turn_pending".into(), w.next_turn_pending.to_string());
    m.insert(
        "leftovers".into(),
        format!("{:?}", w.leftovers.iter().collect::<Vec<_>>()),
    );

    // Event-deck piles (not the log).
    m.insert("event_deck".into(), w.event_deck.join("|"));
    m.insert("event_discard".into(), w.event_discard.join("|"));
    m.insert("event_removed".into(), w.event_removed.join("|"));
    m.insert("st.event_active".into(), format!("{:?}", w.st.event_active));

    // Board-owner seat marker (always -1; keeps the key space complete).
    m.insert("board_owner".into(), BOARD_OWNER.to_string());

    Snap { map: m }
}

/// Paths the engine is allowed to move as pure bookkeeping of an action
/// (event ids, clocks, previews, derived mirrors). Anything else is a real
/// state change a rule must own.
pub fn is_bookkeeping(path: &str) -> bool {
    // Derived mirrors recomputed by every tick.
    if path.starts_with("roll:P")
        || path.starts_with("assets:P")
        || path.starts_with("hand_n:P")
        || path.starts_with("draw_n:P")
        || path.starts_with("discard_pub:P")
    {
        return true;
    }
    // Movement scratch the engine writes on every walk, plus the
    // abnormal move hit latch (set when an abnormal move lands, cleared when
    // the next main move completes) -- engine bookkeeping, not a card's
    // frame-visible effect. Cards read it; the frame condition does not.
    if let Some(rest) = path.strip_prefix("state:P") {
        let key = rest.split(':').nth(1).unwrap_or("");
        if key == "lastWalk" || key == "abnormalHit" {
            return true;
        }
    }
    matches!(
        path,
        // Event-deck reshuffles are game flow (turn start draws an event).
        "event_deck" | "event_discard" | "event_removed" | "st.event_active"
            // Turn bookkeeping that grows because *we* acted.
            | "turn.played"
            | "turn.player"
            | "turn.main_moved"
            | "turn.main_steps"
            | "turn.turn_rolls"
            | "turn.turn_snap"
            | "turn.turn_start_pos"
            | "turn.play_from_hand"
            | "next_turn_pending"
    )
}

/// A status item losing one layer to the engine's own turn-boundary tick.
/// Snapshots store `value|min|max|expires`, so the tick is visible as the
/// value dropping and `expires` flipping to `Some(TurnEnd)` / `Some(TurnStart)`.
pub fn is_engine_tick(path: &str, before: &str, after: &str) -> bool {
    if !path.starts_with("state:P") {
        return false;
    }
    let key = path.split(':').nth(2).unwrap_or("");
    if !matches!(key, "stay" | "stun" | "stunStart" | "exile") {
        return false;
    }
    let parse = |s: &str| -> Option<(i32, bool)> {
        let v: i32 = s.split('|').next()?.parse().ok()?;
        let exp = s.contains("TurnEnd") || s.contains("TurnStart");
        Some((v, exp))
    };
    match (parse(before), parse(after)) {
        (Some((b, _)), Some((a, e))) => a <= b && (a < b || e),
        _ => false,
    }
}

/// A change outside the engine allowlist.
#[derive(Clone, Debug)]
pub struct Delta {
    pub path: String,
    pub before: String,
    pub after: String,
}

/// Diff two snapshots. Bookkeeping paths and engine status ticks are dropped
/// unless `keep_bookkeeping`.
pub fn diff(before: &Snap, after: &Snap, keep_bookkeeping: bool) -> Vec<Delta> {
    let mut out = Vec::new();
    let keys: BTreeSet<&String> = before.map.keys().chain(after.map.keys()).collect();
    for k in keys {
        let b = before.map.get(k).map(String::as_str).unwrap_or("<absent>");
        let a = after.map.get(k).map(String::as_str).unwrap_or("<absent>");
        if b == a {
            continue;
        }
        if !keep_bookkeeping && (is_bookkeeping(k) || is_engine_tick(k, b, a)) {
            continue;
        }
        out.push(Delta {
            path: k.clone(),
            before: b.to_string(),
            after: a.to_string(),
        });
    }
    out
}

/// Frame condition: every non-bookkeeping changed path must sit under one of
/// `allowed_prefixes`. Reports every offender.
///
/// A clause's "Frame" field names the paths its effect may touch; this is the
/// executable form of that sentence. Prefix match is `path == prefix` or
/// `path.starts_with(prefix)` -- pass `"money:P0"` or `"hand:P0"` (exact) or
/// `"money:"` (every player).
///
/// ```
/// # use spec_kit::snap::frame;
/// # fn demo(pre: &game_core::engine::World, post: &game_core::engine::World) {
/// frame(pre, post, &["money:P0", "hand:P0", "draw:P0"]).expect("frame");
/// # }
/// ```
pub fn frame(
    pre: &World,
    post: &World,
    allowed_prefixes: &[&str],
) -> Result<(), String> {
    let a = snapshot(pre);
    let b = snapshot(post);
    let deltas = diff(&a, &b, false);
    let bad: Vec<String> = deltas
        .iter()
        .filter(|d| {
            !allowed_prefixes
                .iter()
                .any(|p| d.path == *p || d.path.starts_with(p))
        })
        .map(|d| format!("{}: {} -> {}", d.path, d.before, d.after))
        .collect();
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "frame: {} path(s) changed outside {:?}: {}",
            bad.len(),
            allowed_prefixes,
            bad.join("; ")
        ))
    }
}

/// Render a compact one-line summary of a delta list (for findings).
pub fn render_deltas(deltas: &[Delta], limit: usize) -> String {
    deltas
        .iter()
        .take(limit)
        .map(|d| format!("{}: {} -> {}", d.path, d.before, d.after))
        .collect::<Vec<_>>()
        .join("; ")
}