//! Whole-`World` snapshot and diff, plus the mapping from observed field
//! paths onto `footprints.json` surface keys.
//!
//! Black-box: only `game_core`'s public `World` / `MatchState` is read.

use std::collections::BTreeMap;

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
        m.insert(format!("character:P{i}"), p.character.clone());
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
                    "{}|uid={}|tile={}|crystals={}|immune={}|down={}|band={}",
                    c.card, c.uid, c.tile, c.crystals, c.immune, c.face_down, c.band_skill
                ),
            );
            m.insert(format!("crystals:P{i}:{}", c.card), c.crystals.to_string());
            for (k, v) in &c.props {
                m.insert(format!("fprop:P{i}:{}:{k}", c.card), v.to_string());
            }
        }
        for (h, c) in w.hidden.get(i).map(|h| h.hand.clone()).unwrap_or_default().iter().enumerate() {
            m.insert(format!("hand:P{i}:{h}"), c.clone());
        }
        for (h, c) in w.hidden.get(i).map(|h| h.draw.clone()).unwrap_or_default().iter().enumerate() {
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
        if t < w.st.tile_colors.len() {
            m.insert(format!("tcolor:T{t}"), w.st.tile_colors[t].to_string());
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
                "{}|uid={}|tile={}|crystals={}|immune={}|down={}",
                c.card, c.uid, c.tile, c.crystals, c.immune, c.face_down
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
    m.insert("turn.buy_discount".into(), t.buy_discount.to_string());
    m.insert("turn.paid_in_settle".into(), t.paid_in_settle.to_string());
    m.insert("turn.free_buy".into(), t.free_buy.to_string());
    m.insert("turn.raze_on_buy".into(), t.raze_on_buy.to_string());
    m.insert("turn.turn_start_pos".into(), format!("{:?}", t.turn_start_pos));
    m.insert("turn.turn_snap".into(), format!("{:?}", t.turn_snap));
    m.insert("turn.turn_rolls".into(), format!("{:?}", t.turn_rolls));
    m.insert("turn.build_discount".into(), t.build_discount.to_string());
    m.insert(
        "turn.build_discount_layers".into(),
        t.build_discount_layers.to_string(),
    );
    m.insert("turn.build_cost_pct".into(), t.build_cost_pct.to_string());

    m.insert("extra_turns".into(), format!("{:?}", w.extra_turns));
    m.insert("ring_bonus".into(), w.ring_bonus.to_string());
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
///
/// The allowlist is documented in `docs/rulebook/COVERAGE.md` under
/// "Unintended interactions".
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
    // Engine status ticks: stay / stun / stunStart lose a layer at a turn
    // boundary and the item is rewritten with its `expires` marker. A rule
    // *clearing* or *adding* a layer still shows up as a value change with a
    // stable `expires`, so only the tick shape is ignored (see
    // [`is_engine_tick`]).
    if let Some(rest) = path.strip_prefix("state:P") {
        let key = rest.split(':').nth(1).unwrap_or("");
        // Movement scratch the engine writes on every walk.
        if key == "lastWalk" {
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
    let keys: std::collections::BTreeSet<&String> =
        before.map.keys().chain(after.map.keys()).collect();
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

/// Map an observed path onto a `footprints.json` surface key. Returns `None`
/// for paths that are not a rule-facing surface (pure layout / identity).
pub fn surface_of(path: &str) -> Option<&'static str> {
    let p = path;
    if p.starts_with("money:P") {
        return Some("money");
    }
    if p.starts_with("pos:P") {
        return Some("path.position");
    }
    if p.starts_with("next_steps:P") {
        return Some("roll.set");
    }
    if p.starts_with("owner:T") {
        return Some("tile.owner");
    }
    if p.starts_with("houses:T") {
        return Some("tile.houses");
    }
    if p.starts_with("mortgaged:T") {
        return Some("tile.mortgaged");
    }
    if p.starts_with("ember:T") {
        return Some("tile.effect");
    }
    if p.starts_with("tcolor:T") {
        return Some("tile.colour");
    }
    if p.starts_with("mark:") {
        return Some("marks");
    }
    if p.starts_with("hand:P") {
        return Some("hand");
    }
    if p.starts_with("draw:P") {
        return Some("draw.pile");
    }
    if p.starts_with("discard:P") {
        return Some("discard.pile");
    }
    if p.starts_with("field:P") || p.starts_with("bfield:") {
        return Some("field");
    }
    if p.starts_with("crystals:P") {
        return Some("crystals");
    }
    if p.starts_with("fprop:") || p.starts_with("bprop:") {
        // Tile-instance props: a card that bends a tile writes these.
        if p.contains(":noReward") {
            return Some("circle.reward");
        }
        if p.contains(":rentFactor") || p.contains(":payFactor") {
            return Some("tile.rent");
        }
        if p.contains(":buyDiscount") || p.contains(":freeBuy") || p.contains(":razeOnBuy") {
            return Some("tile.buy");
        }
        if p.contains(":noBuild") {
            return Some("build");
        }
        return Some("tile.effect");
    }
    if p.starts_with("state:P") {
        let key = p.rsplit(':').next().unwrap_or("");
        return Some(match key {
            "fire" => "fire",
            "stay" => "status.stay",
            "stun" | "stunStart" => "status.stun",
            "exile" | "exileTo" => "status.exile",
            "handLimit" | "startHand" => "hand.limit",
            "noHand" | "unstoppable" | "noBuild" | "noCircleReward" => "status.block",
            "built" | "bought" | "redeemed" | "mortgaged" => "state",
            _ => "state",
        });
    }
    if p.starts_with("token:P") {
        // A `skillBlock:*` token is the skill-suppression marker; everything
        // else (P✽P fans, water-jelly marks, …) is a fan / mark counter.
        if p.contains("skillBlock") {
            return Some("skills.blocked");
        }
        return Some("fans");
    }
    if p.starts_with("turn.") {
        return Some(match p {
            "turn.extra" => "turn.extra",
            "turn.buy_discount" | "turn.free_buy" | "turn.raze_on_buy" => "tile.buy",
            "turn.build_discount" | "turn.build_discount_layers" | "turn.build_cost_pct" => {
                "build"
            }
            "turn.fixed_roll" | "turn.main_steps" => "roll.result",
            "turn.abnormal" => "abnormal",
            "turn.no_money_loss" => "money",
            "turn.extreme" => "effects",
            "turn.play_from_hand" => "card.play",
            "turn.paid_in_settle" => "money",
            _ => "state",
        });
    }
    if p == "scheduled" {
        // A rule's own turn-end callback queue. The rule scheduling itself
        // (On::AtEnd) is its effect mechanism, not a foreign write.
        return Some("effects");
    }
    if p == "extra_turns" {
        return Some("turn.extra");
    }
    if p == "ring_bonus" {
        return Some("tile.rent");
    }
    if p == "targeted" {
        return Some("targeting");
    }
    if p.starts_with("event_") {
        return Some("event");
    }
    if p == "leftovers" || p == "out_count" {
        return Some("bankrupt");
    }
    // Identity / layout that no rule should be writing, but that also is not
    // a named surface. Flag as `state` so a stray write is still visible.
    if p.starts_with("character:P") || p == "board_owner" {
        return None;
    }
    if p.starts_with("roll:P") || p.starts_with("assets:P") {
        return None;
    }
    if p.starts_with("bankrupt:P") || p.starts_with("left:P") {
        return Some("bankrupt");
    }
    Some("state")
}

/// Surfaces touched by a delta list.
pub fn surfaces_of(deltas: &[Delta]) -> std::collections::BTreeSet<&'static str> {
    deltas.iter().filter_map(|d| surface_of(&d.path)).collect()
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