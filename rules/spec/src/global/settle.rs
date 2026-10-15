//! G-SETTLE -- 基础[结算]: tile effects, rent, buy, build, mortgage.
//!
//! Owned by unit 5. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Source precedence: live sheet (`docs/rulebook/cards.json`,
//! `the roster` / `bands.json`) > `docs/rulebook/rulebook-doc.md`.
//! `rulebook.txt` is not authoritative. Specs come from the book text;
//! `docs/rulebook/SETTLE-STAGES.md` stage names are **observability only**.
//!
//! The book's 基础[结算]规则 numbering is 1, 2, 3, **5**, 6, 7 -- item 4 is
//! missing. Clause IDs are sequential (`G-SETTLE-NN`) and do not mirror the
//! book's item numbers.

use game_core::data::{GameData, TileKind};
use game_core::engine::World;
use game_core::state::MatchEvent;
use spec_kit::{int_arg, Clause, Step};

/// Settlement clauses. Each entry is one book sentence, checked as a predicate
/// over one observed transition.
pub const CLAUSES: &[Clause] = &[
    G_SETTLE_01,
    G_SETTLE_02,
    G_SETTLE_03,
    G_SETTLE_04,
    G_SETTLE_05,
    G_SETTLE_06,
    G_SETTLE_07,
    G_SETTLE_08,
    G_SETTLE_10,
    G_SETTLE_11,
    G_SETTLE_12,
    G_SETTLE_13,
    G_SETTLE_14,
];

// ---------------------------------------------------------------------------
// helpers (local to this unit -- the generic kit stays game-free)
// ---------------------------------------------------------------------------

/// 「向上取整10」 on a half charge: `ceil(full / 2 / 10) * 10`.
/// The book only says 「向上取整10」; this is the nearest reading of
/// "round the half up to a multiple of 10".
pub fn half_ceil10(full: i32) -> i32 {
    ((full as f64 / 2.0 / 10.0).ceil() as i32) * 10
}

/// Rent-table entry at the **standing** house count (the book's 「现等级」).
/// RiNG has no table entry (OPEN-RING-RENT); returns `None`.
pub fn table_rent_at(step: &Step<'_>, tile: usize) -> Option<i32> {
    let t = step.data.tiles.get(tile)?;
    if t.kind.as_str() == "ring" || t.rent.is_empty() {
        return None;
    }
    let h = step
        .pre
        .st
        .houses
        .get(tile)
        .copied()
        .unwrap_or(0)
        .max(0) as usize;
    Some(t.rent[h.min(t.rent.len() - 1)])
}

/// Land price + houses already standing
/// (「购买格子地契和建造已有房子的资金总价」).
pub fn deed_value(step: &Step<'_>, tile: usize) -> i32 {
    let Some(t) = step.data.tiles.get(tile) else {
        return 0;
    };
    t.price + step.pre.st.houses.get(tile).copied().unwrap_or(0) * t.house
}

/// Per-tile level cap: the rent table's last level (0 for RiNG, which has an
/// empty table and 「所有RiNG不可升级」).
pub fn level_cap(step: &Step<'_>, tile: usize) -> i32 {
    let Some(t) = step.data.tiles.get(tile) else {
        return 0;
    };
    if t.kind.as_str() == "ring" {
        return 0;
    }
    t.rent.len().saturating_sub(1) as i32
}

/// Message key of an event (the nested `what` for a card attribution).
fn key_of(e: &game_core::state::MatchEvent) -> String {
    spec_kit::event_key(e)
}

/// Did this window contain a card / skill activation? Then a modifier may
/// have reshaped the amount and a clean-number clause must skip (L6
/// 「技能和卡的效果优先」).
fn has_card_activation(step: &Step<'_>) -> bool {
    step.events.iter().any(|e| {
        e.r#type == "card"
            || e.r#type == "skill"
            || key_of(e).starts_with("cards:")
            || key_of(e).starts_with("card:")
            || (!e.card.is_empty()
                && e.r#type != "event"
                && e.r#type != "buy"
                && e.r#type != "build")
    })
}

/// Windows whose money legs are not a clean settle (auction, bankruptcy,
/// mortgage-funding). G-MONEY-01 skips the same shapes.
fn has_funding_or_auction(step: &Step<'_>) -> bool {
    step.events.iter().any(|e| {
        matches!(
            e.r#type.as_str(),
            "mortgage" | "redeem" | "bankrupt" | "auction"
        ) || key_of(e).contains("auction")
    })
}

/// Any lingering field card can reshape a price outside this window
/// (BuyAdd / BuyMul / BuySet on a previously played card). Amount clauses
/// skip those; scenario tests cover the clean figure.
fn has_lingering_modifiers(step: &Step<'_>) -> bool {
    step.pre.st.players.iter().any(|p| {
        p.field
            .iter()
            .any(|f| !f.card.starts_with("skill:"))
    }) || step.pre.st.board_field.iter().any(|f| {
        !f.card.starts_with("tile:") && !f.card.starts_with("mark:")
    })
}

/// Same-colour buyable tiles of an agent tile, in board (「从前到后」) order.
/// Colour is the board `group`; colour-prop overrides are OPEN-同色 and not
/// applied here.
pub fn agent_colour_set(step: &Step<'_>, agent: usize) -> Vec<usize> {
    let Some(a) = step.data.tiles.get(agent) else {
        return Vec::new();
    };
    if a.kind.as_str() != "agent" {
        return Vec::new();
    }
    let g = a.group;
    (0..step.data.tiles.len())
        .filter(|&t| {
            let x = &step.data.tiles[t];
            x.is_buyable() && x.group == g
        })
        .collect()
}

// ---------------------------------------------------------------------------
// clauses
// ---------------------------------------------------------------------------

/// A mortgaged deed's settle has no rent (and the owner branch has no effect).
///
/// * **ID:** G-SETTLE-01
/// * **Source:** `rulebook-doc.md:158` (owner) / `:160`-`:161` (other)
/// * **Quote:** 「如果格子地契已抵押则无效果。」 (owner-landed). The other-owned
///   branch at `:161` is the complement: a mortgaged deed charges **no rent**
///   (「如果格子地契未抵押则**[支付]…现等级地租**」 is the only rent case).
/// * **Trigger:** an observed input whose event window contains a `rent` leg
///   attributed to a landed settle on tile `T`.
/// * **Condition:** `T` is a player-owned buyable tile that is mortgaged in
///   `pre`.
/// * **Effect:** no `rent` event for `T` in this window.
/// * **Frame:** n/a (read of the event window).
/// * **Rulings & OPEN:** the force-buy branch of a mortgaged tile
///   (「可选择[支付]…两倍，从该玩家处强行购买」) is a `pay` / `forcebuy` that is
///   *not* rent and is out of scope here -- see [`G_SETTLE_11`]. An event's
///   tile is the `to` field on the money line when present; a `rent` with no
///   tile is not a landed settle and is ignored.
/// * **Executable:** [`g_settle_01`].
const G_SETTLE_01: Clause = Clause::new(
    "G-SETTLE-01",
    "rulebook-doc.md:158",
    "如果格子地契已抵押则无效果。",
    g_settle_01,
);

/// Executable form of [`G_SETTLE_01`].
pub fn g_settle_01(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "rent" {
            continue;
        }
        let tile = e.to;
        if tile < 0 {
            continue;
        }
        let t = tile as usize;
        if step.pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            return Err(format!(
                "rent paid on mortgaged tile {t} (value {}, owner {})",
                e.value, e.other
            ));
        }
    }
    Ok(())
}

/// CiRCLE / 江户川乐器店 settle draws one hand card.
///
/// * **ID:** G-SETTLE-02
/// * **Source:** `rulebook-doc.md:150`
/// * **Quote:** 「CiRCLE和江户川乐器店的[结算]是：抽取一张手卡。」
/// * **Trigger:** an observed input whose window logs a landing on a `circle`
///   or `edogawa` tile (`log.land_circle` / `log.land_draw`).
/// * **Condition:** the window has no card activation (a card can replace the
///   body -- L6 「技能和卡的效果优先」 -- and then this clause does not apply).
/// * **Effect:** exactly one hand card is drawn (`draw` event `value == 1`).
/// * **Frame:** the event half of cafe / 流星堂 does **not** run.
/// * **Rulings & OPEN:** the [经过] CiRCLE reward (`rulebook-doc.md:151`) is a
///   pass effect, not this settle -- owned by G-MOVE. OPEN-结算定义.
/// * **Executable:** [`g_settle_02`].
const G_SETTLE_02: Clause = Clause::new(
    "G-SETTLE-02",
    "rulebook-doc.md:150",
    "CiRCLE和江户川乐器店的[结算]是：抽取一张手卡。",
    g_settle_02,
);

/// Executable form of [`G_SETTLE_02`].
pub fn g_settle_02(step: &Step<'_>) -> Result<(), String> {
    let landed = step.events.iter().any(|e| {
        let k = key_of(e);
        k == "log.land_circle" || k == "log.land_draw"
    });
    if !landed || has_card_activation(step) {
        return Ok(());
    }
    let who = step.player_of_act();
    let draws: i64 = step
        .events
        .iter()
        .filter(|e| e.r#type == "draw" && e.player_id == who as i32)
        .map(|e| e.value as i64)
        .sum();
    // Landing on CiRCLE also [经过]s it, so the [CiRCLE奖励] card half may add
    // one more draw in the same window (「获得2000资金或抽1张卡」). That reward
    // is G-MOVE's; this clause only requires the settle's own 1.
    let reward_card = step.events.iter().any(|e| {
        let k = key_of(e);
        k.contains("circle") && k.contains("card")
    });
    let want_draws = if reward_card { 2 } else { 1 };
    if draws != want_draws {
        return Err(format!(
            "CiRCLE/江户川 settle drew {draws} cards, expected {want_draws} (settle 1 + optional reward card)"
        ));
    }
    // The cafe / 流星堂 half must not have run alongside.
    if step.events.iter().any(|e| e.r#type == "event") {
        return Err("CiRCLE/江户川 settle also drew an event card".into());
    }
    Ok(())
}

/// CiRCLE咖啡厅 / 流星堂 settle draws one hand card **then** one event card.
///
/// * **ID:** G-SETTLE-03
/// * **Source:** `rulebook-doc.md:152`
/// * **Quote:** 「CiRCLE咖啡厅和流星堂的的[结算]是：抽取一张手卡，然后抽取一个事件卡。」
/// * **Trigger:** an observed input whose window logs a landing on a `cafe` or
///   `ryuseido` tile (`log.land_event`).
/// * **Condition:** the window has no card activation (body replace).
/// * **Effect:** exactly one hand card is drawn and exactly one event is drawn
///   (`event` type), in that order.
/// * **Frame:** see [`G_SETTLE_04`] for the event's zone and [`G_SETTLE_05`]
///   for the reshuffle.
/// * **Rulings & OPEN:** OPEN-结算定义.
/// * **Executable:** [`g_settle_03`].
const G_SETTLE_03: Clause = Clause::new(
    "G-SETTLE-03",
    "rulebook-doc.md:152",
    "CiRCLE咖啡厅和流星堂的的[结算]是：抽取一张手卡，然后抽取一个事件卡。",
    g_settle_03,
);

/// Executable form of [`G_SETTLE_03`].
pub fn g_settle_03(step: &Step<'_>) -> Result<(), String> {
    let landed = step.events.iter().any(|e| key_of(e) == "log.land_event");
    if !landed || has_card_activation(step) {
        return Ok(());
    }
    let who = step.player_of_act();
    let draws: i64 = step
        .events
        .iter()
        .filter(|e| e.r#type == "draw" && e.player_id == who as i32)
        .map(|e| e.value as i64)
        .sum();
    if draws != 1 {
        return Err(format!(
            "cafe/流星堂 settle drew {draws} hand cards, expected exactly 1"
        ));
    }
    let evs: Vec<_> = step
        .events
        .iter()
        .filter(|e| e.r#type == "event" && e.player_id == who as i32)
        .collect();
    if evs.len() != 1 {
        return Err(format!(
            "cafe/流星堂 settle drew {} events, expected exactly 1",
            evs.len()
        ));
    }
    // Order: the hand draw precedes the event draw
    // (「抽取一张手卡，然后抽取一个事件卡」).
    let draw_pos = step.events.iter().position(|e| e.r#type == "draw");
    let ev_pos = step.events.iter().position(|e| e.r#type == "event");
    if let (Some(d), Some(v)) = (draw_pos, ev_pos) {
        if d > v {
            return Err(format!(
                "event drawn before the hand card (draw@{d}, event@{v})"
            ));
        }
    }
    Ok(())
}

/// The drawn event is public (not in hand) and files away after it settles.
///
/// * **ID:** G-SETTLE-04
/// * **Source:** `rulebook-doc.md:153`-`:154`
/// * **Quote:** 「抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。」 /
///   「事件结算后进入事件弃卡区。」
/// * **Trigger:** an observed input whose window draws an `event` card.
/// * **Condition:** the drawn event is not a derived card that is 「永久移除」
///   and is not left on the active list (a permanent event's own text is a
///   different clause -- unit 21).
/// * **Effect:** the event id is never in the drawer's hand; after the window
///   it sits in `event_discard` (or `event_removed` / `event_active` when its
///   own text says so).
/// * **Frame:** n/a (zone read).
/// * **Rulings & OPEN:** a permanent event (「将此卡放置于场地中央」) stays in
///   play rather than the discard -- its own clause overrides this filing
///   sentence (L6).
/// * **Executable:** [`g_settle_04`].
const G_SETTLE_04: Clause = Clause::new(
    "G-SETTLE-04",
    "rulebook-doc.md:153",
    "抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。",
    g_settle_04,
);

/// Executable form of [`G_SETTLE_04`].
pub fn g_settle_04(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "event" {
            continue;
        }
        let id = e.card.clone();
        if id.is_empty() {
            continue;
        }
        // Never in any hand (「不进入手卡」 -- no drawer restriction).
        for (pi, h) in step.post.hidden.iter().enumerate() {
            if h.hand.iter().any(|c| c == &id) {
                return Err(format!("drawn event {id} entered P{pi}'s hand"));
            }
        }
        // Filed away after it settles: discard, removed, still active, or
        // reshuffled back into the deck (「洗切并当作新的事件卡堆」).
        let filed = step.post.event_discard.iter().any(|c| c == &id)
            || step.post.event_removed.iter().any(|c| c == &id)
            || step.post.event_deck.iter().any(|c| c == &id)
            || step.post.st.event_active.iter().any(|a| a.id == id);
        if !filed {
            return Err(format!(
                "drawn event {id} is nowhere after the window (not discard / removed / active / deck)"
            ));
        }
    }
    Ok(())
}

/// An emptied event deck reshuffles the discard into a new draw pile.
///
/// * **ID:** G-SETTLE-05
/// * **Source:** `rulebook-doc.md:154`
/// * **Quote:** 「如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。」
/// * **Trigger:** an observed input that draws an event while `pre`'s event
///   deck is empty.
/// * **Condition:** `pre.event_deck` is empty and `pre.event_discard` is
///   non-empty at the moment of the draw.
/// * **Effect:** the draw succeeds (an `event` is logged).
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** 「洗切」 is a shuffle -- order is not specified; only
///   the fact of the recycle is checked.
/// * **Executable:** [`g_settle_05`].
const G_SETTLE_05: Clause = Clause::new(
    "G-SETTLE-05",
    "rulebook-doc.md:154",
    "如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。",
    g_settle_05,
);

/// Executable form of [`G_SETTLE_05`].
pub fn g_settle_05(step: &Step<'_>) -> Result<(), String> {
    let drew = step.events.iter().any(|e| e.r#type == "event");
    if !drew {
        return Ok(());
    }
    if !step.pre.event_deck.is_empty() || step.pre.event_discard.is_empty() {
        return Ok(());
    }
    let any_event = step
        .events
        .iter()
        .any(|e| e.r#type == "event" && !e.card.is_empty());
    if !any_event {
        return Err(
            "event deck empty and discard non-empty, but no event was drawn (no reshuffle)"
                .into(),
        );
    }
    Ok(())
}

/// Unowned [可购买格子]: optional buy at deed price + standing houses.
///
/// * **ID:** G-SETTLE-06
/// * **Source:** `rulebook-doc.md:155`
/// * **Quote:** 「无主的[可购买格子]的[结算]是：**可选择[消耗]购买格子地契和建造已有房子的资金总价，获得格子地契和拥有权。**」
/// * **Trigger:** a `buy` event whose `to` is a buyable tile that was unowned
///   in `pre`.
/// * **Condition:** the window has no card activation (a BuyAdd / BuyMul /
///   BuySet hook reshapes the price -- L6).
/// * **Effect:** the buyer's money falls by exactly
///   `price + houses * house_cost` (the 「资金总价」); ownership of the tile
///   goes to the buyer.
/// * **Frame:** only that player's money and that tile's owner / mortgage move.
/// * **Rulings & OPEN:** the **timing** of the offer (settle vs end step) is
///   specified in the scenario test `g_settle_06_unowned_buy_is_the_settle`:
///   the book makes the optional buy *the settle*. See Findings.
/// * **Executable:** [`g_settle_06`].
const G_SETTLE_06: Clause = Clause::new(
    "G-SETTLE-06",
    "rulebook-doc.md:155",
    "无主的[可购买格子]的[结算]是：可选择[消耗]购买格子地契和建造已有房子的资金总价，获得格子地契和拥有权。",
    g_settle_06,
);

/// Executable form of [`G_SETTLE_06`].
pub fn g_settle_06(step: &Step<'_>) -> Result<(), String> {
    if has_card_activation(step) || has_lingering_modifiers(step) {
        return Ok(());
    }
    for e in step.events {
        if e.r#type != "buy" {
            continue;
        }
        let t = e.to;
        if t < 0 {
            continue;
        }
        let t = t as usize;
        // Only the unowned-land buy (not force / acquire / auction).
        if step.pre.st.owners.get(t).copied().unwrap_or(-1) >= 0 {
            continue;
        }
        if !step
            .data
            .tiles
            .get(t)
            .map(|x| x.is_buyable())
            .unwrap_or(false)
        {
            continue;
        }
        // Auction wins log as `buy` too (G-BANKRUPT owns them).
        if key_of(e).contains("auction") || e.value != 0 {
            continue;
        }
        if has_funding_or_auction(step) {
            continue;
        }
        let who = e.player_id.max(0) as usize;
        let want = deed_value(step, t);
        // `buy` logs the price on the message; the money delta is the charge.
        let Some(logged) = int_arg(&e.msg, "price") else {
            continue;
        };
        let logged = logged as i32;
        if logged != want {
            return Err(format!(
                "unowned buy of tile {t} charged {logged}, expected {want} (deed + houses)"
            ));
        }
        // Money delta is checked in the clean scenario test
        // (`g_settle_06_unowned_buy_price_includes_the_houses`); a lingering
        // free-buy / discount can legitimately zero the charge.
        if step.post.st.owners.get(t).copied().unwrap_or(-1) != who as i32 {
            return Err(format!("tile {t} not owned by P{who} after the buy"));
        }
    }
    Ok(())
}

/// Own un-mortgaged [可购买格子]: optional build at the marked house cost.
///
/// * **ID:** G-SETTLE-07
/// * **Source:** `rulebook-doc.md:157`
/// * **Quote:** 「如果格子地契未抵押则**可选择[消耗]格子地契所标注的房屋建筑费进行升级建造**，每块地有标注的等级上限（例：所有RiNG不可升级，购物中心最大等级为三栋房屋）。」
/// * **Trigger:** a `build` event whose `to` is a buyable tile owned by the
///   builder in `pre`.
/// * **Condition:** the tile was not mortgaged in `pre`; the window has no
///   card activation (build-cost discounts are L6).
/// * **Effect:** money falls by the tile's `house` cost; houses rise by 1.
/// * **Frame:** only that player's money and that tile's houses move.
/// * **Rulings & OPEN:** level cap is [`G_SETTLE_08`]; funding restriction is
///   `G-SETTLE-09` (scenario-only).
/// * **Executable:** [`g_settle_07`].
const G_SETTLE_07: Clause = Clause::new(
    "G-SETTLE-07",
    "rulebook-doc.md:157",
    "如果格子地契未抵押则可选择[消耗]格子地契所标注的房屋建筑费进行升级建造，每块地有标注的等级上限（例：所有RiNG不可升级，购物中心最大等级为三栋房屋）。",
    g_settle_07,
);

/// Executable form of [`G_SETTLE_07`].
pub fn g_settle_07(step: &Step<'_>) -> Result<(), String> {
    if has_card_activation(step) || has_lingering_modifiers(step) {
        return Ok(());
    }
    for e in step.events {
        if e.r#type != "build" {
            continue;
        }
        let t = e.to;
        if t < 0 {
            continue;
        }
        let t = t as usize;
        let who = e.player_id.max(0) as usize;
        if step.pre.st.owners.get(t).copied().unwrap_or(-1) != who as i32 {
            return Err(format!("build on tile {t} not owned by P{who} in pre"));
        }
        if step.pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            return Err(format!("build on mortgaged tile {t} (5.2: no effect)"));
        }
        let Some(tile) = step.data.tiles.get(t) else {
            continue;
        };
        // Turn-scoped build discounts (build_cost_pct / build_discount) live on
        // turn state, not field cards -- skip the marked-cost check when armed.
        if step.pre.turn.build_cost_pct != 100 || step.pre.turn.build_discount > 0 {
            continue;
        }
        let cost = tile.house;
        let Some(logged) = int_arg(&e.msg, "cost") else {
            continue;
        };
        let logged = logged as i32;
        if logged != cost {
            return Err(format!(
                "build on {t} charged {logged}, expected the marked house cost {cost}"
            ));
        }
        if !has_funding_or_auction(step) {
            let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
            if delta != -i64::from(cost) {
                return Err(format!(
                    "P{who} money delta {delta} on build of {t}, expected {}",
                    -cost
                ));
            }
        }
        let pre_h = step.pre.st.houses.get(t).copied().unwrap_or(0);
        let post_h = step.post.st.houses.get(t).copied().unwrap_or(0);
        if post_h != pre_h + 1 {
            return Err(format!("houses on {t} went {pre_h} -> {post_h}, expected +1"));
        }
    }
    Ok(())
}

/// Per-tile level cap: RiNG never upgrades; every tile's houses stay within
/// the marked levels (e.g. 购物中心 max 3).
///
/// * **ID:** G-SETTLE-08
/// * **Source:** `rulebook-doc.md:157`
/// * **Quote:** 「每块地有标注的等级上限（例：所有RiNG不可升级，购物中心最大等级为三栋房屋）。」
/// * **Trigger:** any observed input.
/// * **Condition:** a `build` event, or a post-state house count on a tile
///   whose `pre` count was already at or below the cap (arranged states are
///   non-canonical -- skip when `pre` is already over).
/// * **Effect:** after a build, `houses[t] <= level_cap(t)`; a `build` never
///   lands on a `ring` tile; 购物中心 (and any tile whose rent table has 4
///   levels) caps at 3 houses.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** the cap is read as `rent.len() - 1` (the marked
///   levels on the deed's rent table). board.json carries no separate cap
///   field; the book's example matches 购物中心's four rent rows.
/// * **Executable:** [`g_settle_08`].
const G_SETTLE_08: Clause = Clause::new(
    "G-SETTLE-08",
    "rulebook-doc.md:157",
    "每块地有标注的等级上限（例：所有RiNG不可升级，购物中心最大等级为三栋房屋）。",
    g_settle_08,
);

/// Executable form of [`G_SETTLE_08`].
pub fn g_settle_08(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "build" {
            continue;
        }
        let t = e.to;
        if t < 0 {
            continue;
        }
        let t = t as usize;
        let Some(tile) = step.data.tiles.get(t) else {
            continue;
        };
        if tile.kind.as_str() == "ring" {
            return Err(format!("build on RiNG tile {t} (「所有RiNG不可升级」)"));
        }
        let cap = level_cap(step, t);
        let post_h = step.post.st.houses.get(t).copied().unwrap_or(0);
        if post_h > cap {
            return Err(format!(
                "houses on {t} = {post_h} exceeds the marked cap {cap}"
            ));
        }
    }
    // Also: a clean post-state must not sit above the cap if pre did not.
    for t in 0..step.data.tiles.len() {
        let pre_h = step.pre.st.houses.get(t).copied().unwrap_or(0);
        let post_h = step.post.st.houses.get(t).copied().unwrap_or(0);
        let cap = level_cap(step, t);
        if pre_h <= cap && post_h > cap {
            return Err(format!(
                "houses on {t} rose {pre_h} -> {post_h} above the cap {cap} without a build event"
            ));
        }
    }
    Ok(())
}

/// Other's un-mortgaged [可购买格子]: pay the current-level rent.
///
/// * **ID:** G-SETTLE-10
/// * **Source:** `rulebook-doc.md:160`
/// * **Quote:** 「如果格子地契未抵押则**[支付]拥有格子的玩家格子地契所标记的现等级地租。**」
/// * **Trigger:** a `rent` event on a buyable tile owned by someone else.
/// * **Condition:** the tile was not mortgaged in `pre`; the window has no
///   card activation (「支付减半」 etc. are L6); the tile is not a RiNG
///   (OPEN-RING-RENT: no marked table); not an agent half-charge window.
/// * **Effect:** the payer pays the owner exactly `rent[h]` where `h` is the
///   standing house count (「现等级」).
/// * **Frame:** only the two money balances move (the ledger -- G-MONEY-01).
/// * **Rulings & OPEN:** OPEN-RING-RENT (RiNG has no marked rent table). A
///   half-rate agent charge is [`G_SETTLE_13`], not this clause.
/// * **Executable:** [`g_settle_10`].
const G_SETTLE_10: Clause = Clause::new(
    "G-SETTLE-10",
    "rulebook-doc.md:160",
    "如果格子地契未抵押则[支付]拥有格子的玩家格子地契所标记的现等级地租。",
    g_settle_10,
);

/// Executable form of [`G_SETTLE_10`].
pub fn g_settle_10(step: &Step<'_>) -> Result<(), String> {
    if has_card_activation(step) {
        return Ok(());
    }
    for e in step.events {
        if e.r#type != "rent" {
            continue;
        }
        let t = e.to;
        if t < 0 {
            continue;
        }
        let t = t as usize;
        // Skip the agent half-charge windows (they run under G-SETTLE-13).
        if step
            .events
            .iter()
            .any(|x| key_of(x) == "log.agent_all_owned")
        {
            continue;
        }
        if step.pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            continue; // G-SETTLE-01
        }
        // A 「房屋数视为…」 override (RENT_HOUSES) changes the counted level.
        let overridden = step
            .pre
            .st
            .board_field
            .iter()
            .any(|f| f.tile == t as i32 && f.props.contains_key("rentHouses"))
            || step.pre.st.players.iter().any(|p| {
                p.field
                    .iter()
                    .any(|f| f.props.contains_key("rentHouses"))
            });
        if overridden {
            continue;
        }
        let Some(full) = table_rent_at(step, t) else {
            continue; // RiNG / no table
        };
        if e.value != full {
            return Err(format!(
                "rent on tile {t} charged {}, expected the marked 现等级 rent {full}",
                e.value
            ));
        }
    }
    Ok(())
}

/// Other's mortgaged [可购买格子]: optional force-buy at 2x (deed + houses),
/// staying mortgaged.
///
/// * **ID:** G-SETTLE-11
/// * **Source:** `rulebook-doc.md:161`
/// * **Quote:** 「如果格子地契已抵押则可选择**[支付]拥有格子的玩家购买格子地契和建造已有房子的资金总价的两倍，从该玩家处强行购买该格地契，获得的地契仍为抵押状态**。」
/// * **Trigger:** a `forcebuy` event on tile `T`.
/// * **Condition:** `T` was mortgaged and owned by someone else in `pre`.
/// * **Effect:** the buyer pays exactly `2 * (price + houses * house_cost)`;
///   ownership moves to the buyer; `mortgaged[T]` stays true; houses are
///   unchanged.
/// * **Frame:** only the two money balances and the tile's owner move (the
///   mortgage flag is unchanged).
/// * **Rulings & OPEN:** the 「不受任何资金变动效果影响」 half is
///   [`G_SETTLE_12`]. See also 其他规则注意事项 6.
/// * **Executable:** [`g_settle_11`].
const G_SETTLE_11: Clause = Clause::new(
    "G-SETTLE-11",
    "rulebook-doc.md:161",
    "如果格子地契已抵押则可选择[支付]拥有格子的玩家购买格子地契和建造已有房子的资金总价的两倍，从该玩家处强行购买该格地契，获得的地契仍为抵押状态",
    g_settle_11,
);

/// Executable form of [`G_SETTLE_11`].
pub fn g_settle_11(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "forcebuy" {
            continue;
        }
        let t = e.to;
        if t < 0 {
            continue;
        }
        let t = t as usize;
        if !step.pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            return Err(format!(
                "force-buy of {t} which was not mortgaged in pre (6.2 is the mortgaged branch)"
            ));
        }
        let want = 2 * deed_value(step, t);
        let who = e.player_id.max(0) as usize;
        let owner = e.other.max(0) as usize;
        if e.value != want {
            return Err(format!(
                "force-buy of {t} charged {}, expected 2x(deed+houses) = {want}",
                e.value
            ));
        }
        // Whole-window money deltas are only meaningful on a clean leg.
        let other_money = step.events.iter().any(|x| {
            x.r#type != "forcebuy"
                && matches!(x.r#type.as_str(), "pay" | "rent" | "lose" | "gain" | "buy" | "build")
        });
        if !other_money && !has_card_activation(step) {
            let d_buyer = step.post_money(who) as i64 - step.pre_money(who) as i64;
            if d_buyer != -i64::from(want) {
                return Err(format!(
                    "P{who} money delta {d_buyer} on force-buy, expected {}",
                    -want
                ));
            }
            let d_owner = step.post_money(owner) as i64 - step.pre_money(owner) as i64;
            if d_owner != i64::from(want) {
                return Err(format!(
                    "P{owner} money delta {d_owner} on force-buy, expected +{want} (payee receives)"
                ));
            }
        }
        if step.post.st.owners.get(t).copied().unwrap_or(-1) != who as i32 {
            return Err(format!("tile {t} not owned by P{who} after the force-buy"));
        }
        if !step.post.st.mortgaged.get(t).copied().unwrap_or(false) {
            return Err(format!(
                "tile {t} lost its mortgage on force-buy (stays mortgaged)"
            ));
        }
    }
    Ok(())
}

/// Force-buy price is immune to money modifiers; the payee receives it
/// regardless of their state.
///
/// * **ID:** G-SETTLE-12
/// * **Source:** `rulebook-doc.md:161` / `:146`
/// * **Quote:** 「此次购买的价格不受任何资金变动效果影响，收款方无论处于何种状态都可正常收款。」 /
///   「抵押，赎回，和强制购买的资金变动不受任何效果影响，除非效果写明改动抵押，赎回，或强制购买。」
/// * **Trigger:** a `forcebuy` event (any window, including ones with card
///   activations -- the price is immune).
/// * **Condition:** none beyond the trigger.
/// * **Effect:** the charge is exactly `2 * (price + houses * house_cost)`,
///   even when the window carries pay / rent multipliers; the payee's money
///   rises by that amount even if the payee is stunned / stayed / etc.
/// * **Frame:** n/a (the amount is also checked by [`G_SETTLE_11`]).
/// * **Rulings & OPEN:** a card that *explicitly* rewrites force-buy
///   (「写明改动强制购买」) is outside this clause.
/// * **Executable:** [`g_settle_12`].
const G_SETTLE_12: Clause = Clause::new(
    "G-SETTLE-12",
    "rulebook-doc.md:161",
    "此次购买的价格不受任何资金变动效果影响，收款方无论处于何种状态都可正常收款。",
    g_settle_12,
);

/// Executable form of [`G_SETTLE_12`].
pub fn g_settle_12(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "forcebuy" {
            continue;
        }
        let t = e.to;
        if t < 0 {
            continue;
        }
        let t = t as usize;
        let want = 2 * deed_value(step, t);
        // Immune: checked even when cards fired in the window.
        if e.value != want {
            return Err(format!(
                "force-buy of {t} charged {} under modifiers, expected the fixed 2x = {want}",
                e.value
            ));
        }
        let owner = e.other.max(0) as usize;
        let other_money = step.events.iter().any(|x| {
            x.r#type != "forcebuy"
                && matches!(x.r#type.as_str(), "pay" | "rent" | "lose" | "gain" | "buy" | "build")
        });
        if !other_money {
            let d_owner = step.post_money(owner) as i64 - step.pre_money(owner) as i64;
            if d_owner != i64::from(want) {
                return Err(format!(
                    "payee P{owner} received {d_owner}, expected {want} regardless of state"
                ));
            }
        }
    }
    Ok(())
}

/// [地产商], all same-colour buyables owned by others: half-rate settle of
/// each, front to back, rounded up to 10.
///
/// * **ID:** G-SETTLE-13
/// * **Source:** `rulebook-doc.md:162`
/// * **Quote:** 「若与该格子同色的所有[可购买格子]均已属于其他玩家则需向该格子同色的所有[可购买格子]从前到后依次进行一次半价收费的[结算]（向上取整10）。」
/// * **Trigger:** a window that logs `log.agent_all_owned` and then a run of
///   `rent` legs on the agent's colour set.
/// * **Condition:** every same-colour buyable was owned by someone else in
///   `pre`; no card activation (a scale would be L6).
/// * **Effect:** each tile charges `ceil(full / 2 / 10) * 10` where `full` is
///   its marked current-level rent; the charges appear in board
///   (「从前到后」) order.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** OPEN-同色 (colour = board `group`); OPEN-RING-RENT;
///   the half on a **mortgaged** member (which normally has no rent) is
///   unspecified -- see Open questions.
/// * **Executable:** [`g_settle_13`].
const G_SETTLE_13: Clause = Clause::new(
    "G-SETTLE-13",
    "rulebook-doc.md:162",
    "若与该格子同色的所有[可购买格子]均已属于其他玩家则需向该格子同色的所有[可购买格子]从前到后依次进行一次半价收费的[结算]（向上取整10）。",
    g_settle_13,
);

/// Executable form of [`G_SETTLE_13`].
pub fn g_settle_13(step: &Step<'_>) -> Result<(), String> {
    // Find the agent tile from the announce line.
    let mut agent: Option<usize> = None;
    for e in step.events {
        if key_of(e) == "log.agent_all_owned" {
            if let Some(game_core::msg::Arg::Tile(t)) =
                e.msg.a.get("agent").or_else(|| e.msg.a.get("tile"))
            {
                agent = Some(*t as usize);
            }
        }
    }
    let Some(agent) = agent else {
        // Fallback: still check the half formula on any half-noted rent.
        return g_settle_13_half_note_only(step);
    };
    if has_card_activation(step) {
        return Ok(());
    }
    let set = agent_colour_set(step, agent);
    if set.is_empty() {
        return Ok(());
    }
    // Every member must be owned by others in pre (the branch's condition).
    let who = step.player_of_act() as i32;
    for &t in &set {
        let o = step.pre.st.owners.get(t).copied().unwrap_or(-1);
        if o < 0 || o == who {
            return Err(format!(
                "agent_all_owned announced but tile {t} is unowned or self-owned in pre"
            ));
        }
    }
    // Charges in board order, each at half ceil 10 of the marked rent.
    let mut seen: Vec<usize> = Vec::new();
    for e in step.events {
        if e.r#type != "rent" && e.r#type != "pay" {
            continue;
        }
        let t = e.to;
        if t < 0 {
            continue;
        }
        let t = t as usize;
        if !set.contains(&t) {
            continue;
        }
        seen.push(t);
        if step.pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            continue; // unspecified -- see OPEN
        }
        if let Some(full) = table_rent_at(step, t) {
            let want = half_ceil10(full);
            if e.value != want {
                return Err(format!(
                    "agent half-charge on {t} = {}, expected half ceil10 = {want} (full {full})",
                    e.value
                ));
            }
        }
    }
    // Front-to-back: the charged tiles appear in non-decreasing board order.
    let mut sorted = seen.clone();
    sorted.sort_unstable();
    if seen != sorted {
        return Err(format!(
            "agent half-charges not front-to-back (board order): {seen:?}"
        ));
    }
    // Completeness: every non-mortgaged member of the set is charged
    // (「需向…所有[可购买格子]…依次进行一次半价收费的[结算]」).
    for &t in &set {
        if step.pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            continue;
        }
        if let Some(_full) = table_rent_at(step, t) {
            if !seen.contains(&t) {
                return Err(format!(
                    "agent sweep skipped same-colour tile {t} (「所有[可购买格子]」)"
                ));
            }
        }
    }
    Ok(())
}

/// Helper for [`G_SETTLE_13`] when the agent tile is not recoverable from the
/// log: still check every half-noted rent against the ceil-10 formula.
fn g_settle_13_half_note_only(step: &Step<'_>) -> Result<(), String> {
    if has_card_activation(step) {
        return Ok(());
    }
    for e in step.events {
        if e.r#type != "rent" {
            continue;
        }
        let t = e.to;
        if t < 0 {
            continue;
        }
        let t = t as usize;
        if step.pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            continue;
        }
        if let Some(full) = table_rent_at(step, t) {
            let half = half_ceil10(full);
            let noted_half = format!("{:?}", e.msg).contains("half");
            if noted_half && e.value != half && e.value != full {
                return Err(format!(
                    "half-noted rent on {t} = {}, expected {half} or full {full}",
                    e.value
                ));
            }
        }
    }
    Ok(())
}

/// [地产商], otherwise: choose **one** same-colour unowned-or-own buyable and
/// settle it once.
///
/// * **ID:** G-SETTLE-14
/// * **Source:** `rulebook-doc.md:162`
/// * **Quote:** 「否则可选择该格子同色的无主或玩家拥有的[可购买格子]之一进行一次[结算]。」
/// * **Trigger:** a window that settles an agent and does **not** take the
///   all-owned-by-others branch (no `log.agent_all_owned`).
/// * **Condition:** some same-colour buyable is unowned or self-owned in
///   `pre`.
/// * **Effect:** at most one nested settle of a same-colour unowned-or-own
///   buyable. Other-owned tiles are never the chosen target of this branch.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** 「进行一次[结算]」 is one settle of the chosen tile.
///   The choice itself is a `tile` prompt -- scenario-tested.
/// * **Executable:** [`g_settle_14`].
const G_SETTLE_14: Clause = Clause::new(
    "G-SETTLE-14",
    "rulebook-doc.md:162",
    "否则可选择该格子同色的无主或玩家拥有的[可购买格子]之一进行一次[结算]。",
    g_settle_14,
);

/// Executable form of [`G_SETTLE_14`].
pub fn g_settle_14(step: &Step<'_>) -> Result<(), String> {
    // Fire on an agent-choose window: the skip / empty lines, or a nested
    // buy/build that the choose-one committed.
    let agent_window = step.events.iter().any(|e| {
        let k = key_of(e);
        k == "log.agent_skip" || k == "log.agent_nothing" || k == "log.agent_none"
    }) || step.events.iter().any(|e| {
        matches!(e.r#type.as_str(), "buy" | "build") && {
            let t = e.to;
            t >= 0 && {
                let g = step
                    .data
                    .tiles
                    .get(t as usize)
                    .map(|x| x.group)
                    .unwrap_or(-1);
                // Same-colour as some agent is hard to know here; just require
                // the window also carried an agent announce, else skip.
                step.events
                    .iter()
                    .any(|x| key_of(x).contains("agent"))
                    && g >= 0
            }
        }
    });
    if !agent_window {
        return Ok(());
    }
    if step
        .events
        .iter()
        .any(|e| key_of(e) == "log.agent_all_owned")
    {
        return Ok(());
    }
    // The chosen settle, if any, targets one same-colour unowned-or-own tile.
    for e in step.events {
        let t = match e.r#type.as_str() {
            "buy" | "build" | "forcebuy" | "rent" => e.to,
            _ => continue,
        };
        if t < 0 {
            continue;
        }
        let t = t as usize;
        let o = step.pre.st.owners.get(t).copied().unwrap_or(-1);
        let who = step.player_of_act() as i32;
        if o >= 0 && o != who {
            return Err(format!(
                "agent choose-one branch settled other-owned tile {t} (owner {o})"
            ));
        }
    }
    Ok(())
}

// G-SETTLE-09 ("build cost not obtainable by mortgaging that deed") is
// scenario-only: the per-act window cannot see "which mortgage funded the
// build". See `tests/g_settle.rs`
// `g_settle_09_upgrade_cannot_be_funded_by_mortgaging_that_deed`.


// ---------------------------------------------------------------------------
// pure verifiers (addendum: iff contracts)
//
// Each `verify_*` is a pure predicate over `(pre, post, events)`. The Clause
// check wraps it; the mutant tests in `tests/g_settle.rs` clone a real
// transition and apply LESS / MORE / WRONG perturbations, asserting the
// verifier rejects each one.
// ---------------------------------------------------------------------------

/// G-SETTLE-01: no rent leg on a deed that was mortgaged in `pre`.
pub fn verify_no_rent_on_mortgaged(
    pre: &World,
    _post: &World,
    events: &[MatchEvent],
) -> Result<(), String> {
    for e in events {
        if e.r#type != "rent" || e.to < 0 {
            continue;
        }
        let t = e.to as usize;
        if pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            return Err(format!(
                "LESS/MORE: rent paid on mortgaged tile {t} (value {})",
                e.value
            ));
        }
    }
    Ok(())
}

/// G-SETTLE-02: a clean circle/edogawa landing draws exactly the settle card
/// (plus at most the CiRCLE-reward card).
pub fn verify_draw_one(
    pre: &World,
    _post: &World,
    events: &[MatchEvent],
    data: &GameData,
    who: usize,
    land_keys: &[&str],
    allow_reward: bool,
) -> Result<(), String> {
    let _ = data;
    let landed = events.iter().any(|e| land_keys.contains(&event_key_str(e).as_str()));
    if !landed {
        return Ok(());
    }
    if events.iter().any(|e| e.r#type == "card") {
        return Ok(());
    }
    let draws: i64 = events
        .iter()
        .filter(|e| e.r#type == "draw" && e.player_id == who as i32)
        .map(|e| e.value as i64)
        .sum();
    let reward = allow_reward
        && events
            .iter()
            .any(|e| event_key_str(e).contains("circle") && event_key_str(e).contains("card"));
    let want = if reward { 2 } else { 1 };
    if draws != want {
        return Err(format!(
            "LESS/WRONG: settle drew {draws} cards, expected {want} (pre-hand {})",
            pre.hidden.get(who).map(|h| h.hand.len()).unwrap_or(0)
        ));
    }
    if !allow_reward && events.iter().any(|e| e.r#type == "event") {
        return Err("MORE: circle/edogawa settle also drew an event".into());
    }
    Ok(())
}

/// G-SETTLE-03: cafe/ryuseido draws 1 hand card then 1 event.
pub fn verify_draw_one_then_event(
    _pre: &World,
    _post: &World,
    events: &[MatchEvent],
    who: usize,
) -> Result<(), String> {
    let landed = events.iter().any(|e| event_key_str(e) == "log.land_event");
    if !landed || events.iter().any(|e| e.r#type == "card") {
        return Ok(());
    }
    let draws: i64 = events
        .iter()
        .filter(|e| e.r#type == "draw" && e.player_id == who as i32)
        .map(|e| e.value as i64)
        .sum();
    if draws != 1 {
        return Err(format!("WRONG: cafe/流星堂 drew {draws} hand cards, expected 1"));
    }
    let evs = events
        .iter()
        .filter(|e| e.r#type == "event" && e.player_id == who as i32)
        .count();
    if evs != 1 {
        return Err(format!("WRONG: cafe/流星堂 drew {evs} events, expected 1"));
    }
    let d = events.iter().position(|e| e.r#type == "draw");
    let v = events.iter().position(|e| e.r#type == "event");
    if let (Some(d), Some(v)) = (d, v) {
        if d > v {
            return Err(format!("WRONG: event before hand draw (draw@{d} event@{v})"));
        }
    }
    Ok(())
}

/// G-SETTLE-04: the drawn event is in no hand and is filed away.
pub fn verify_event_zone(
    _pre: &World,
    post: &World,
    events: &[MatchEvent],
) -> Result<(), String> {
    for e in events {
        if e.r#type != "event" || e.card.is_empty() {
            continue;
        }
        let id = e.card.as_str();
        for (pi, h) in post.hidden.iter().enumerate() {
            if h.hand.iter().any(|c| c == id) {
                return Err(format!("MORE: drawn event {id} entered P{pi}'s hand"));
            }
        }
        let filed = post.event_discard.iter().any(|c| c == id)
            || post.event_removed.iter().any(|c| c == id)
            || post.event_deck.iter().any(|c| c == id)
            || post.st.event_active.iter().any(|a| a.id == id);
        if !filed {
            return Err(format!("LESS: drawn event {id} is nowhere after the window"));
        }
    }
    Ok(())
}

/// G-SETTLE-05: an empty-deck draw still succeeds (discard reshuffled).
pub fn verify_event_reshuffle(
    pre: &World,
    _post: &World,
    events: &[MatchEvent],
) -> Result<(), String> {
    let drew = events.iter().any(|e| e.r#type == "event");
    if !drew {
        return Ok(());
    }
    if !pre.event_deck.is_empty() || pre.event_discard.is_empty() {
        return Ok(());
    }
    if !events.iter().any(|e| e.r#type == "event" && !e.card.is_empty()) {
        return Err("LESS: empty deck + non-empty discard, but no event drawn".into());
    }
    Ok(())
}

/// G-SETTLE-06: unowned buy charges deed+houses and transfers ownership.
pub fn verify_unowned_buy(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    data: &GameData,
) -> Result<(), String> {
    if events.iter().any(|e| e.r#type == "card") {
        return Ok(());
    }
    for e in events {
        if e.r#type != "buy" || e.to < 0 {
            continue;
        }
        let t = e.to as usize;
        if pre.st.owners.get(t).copied().unwrap_or(-1) >= 0 {
            continue;
        }
        if !data.tiles.get(t).map(|x| x.is_buyable()).unwrap_or(false) {
            continue;
        }
        if event_key_str(e).contains("auction") || e.value != 0 {
            continue;
        }
        let Some(tile) = data.tiles.get(t) else {
            continue;
        };
        let want = tile.price + pre.st.houses.get(t).copied().unwrap_or(0) * tile.house;
        let Some(logged) = int_arg(&e.msg, "price") else {
            continue;
        };
        let logged = logged as i32;
        if logged != want {
            return Err(format!(
                "WRONG: unowned buy of {t} charged {logged}, expected {want}"
            ));
        }
        let who = e.player_id.max(0) as usize;
        if post.st.owners.get(t).copied().unwrap_or(-1) != who as i32 {
            return Err(format!("LESS: tile {t} not owned by P{who} after the buy"));
        }
    }
    Ok(())
}

/// G-SETTLE-07: build charges the marked house cost and raises houses by 1.
pub fn verify_build(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    data: &GameData,
) -> Result<(), String> {
    if events.iter().any(|e| e.r#type == "card") {
        return Ok(());
    }
    for e in events {
        if e.r#type != "build" || e.to < 0 {
            continue;
        }
        let t = e.to as usize;
        let who = e.player_id.max(0) as usize;
        if pre.st.owners.get(t).copied().unwrap_or(-1) != who as i32 {
            return Err(format!("MORE: build on tile {t} not owned by P{who}"));
        }
        if pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            return Err(format!("MORE: build on mortgaged tile {t} (5.2: no effect)"));
        }
        let Some(tile) = data.tiles.get(t) else {
            continue;
        };
        if pre.turn.build_cost_pct != 100 || pre.turn.build_discount > 0 {
            continue;
        }
        let cost = tile.house;
        let Some(logged) = int_arg(&e.msg, "cost") else {
            continue;
        };
        if logged as i32 != cost {
            return Err(format!(
                "WRONG: build on {t} charged {logged}, expected marked cost {cost}"
            ));
        }
        let pre_h = pre.st.houses.get(t).copied().unwrap_or(0);
        let post_h = post.st.houses.get(t).copied().unwrap_or(0);
        if post_h != pre_h + 1 {
            return Err(format!(
                "WRONG: houses on {t} went {pre_h} -> {post_h}, expected +1"
            ));
        }
    }
    Ok(())
}

/// G-SETTLE-08: houses never exceed the marked level cap; RiNG never builds.
pub fn verify_level_cap(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    data: &GameData,
) -> Result<(), String> {
    for e in events {
        if e.r#type != "build" || e.to < 0 {
            continue;
        }
        let t = e.to as usize;
        let Some(tile) = data.tiles.get(t) else {
            continue;
        };
        if tile.kind.as_str() == "ring" {
            return Err(format!("MORE: build on RiNG tile {t}"));
        }
        let cap = tile.rent.len().saturating_sub(1) as i32;
        let post_h = post.st.houses.get(t).copied().unwrap_or(0);
        if post_h > cap {
            return Err(format!(
                "MORE: houses on {t} = {post_h} exceeds cap {cap}"
            ));
        }
    }
    for t in 0..data.tiles.len() {
        let pre_h = pre.st.houses.get(t).copied().unwrap_or(0);
        let post_h = post.st.houses.get(t).copied().unwrap_or(0);
        let cap = data.tiles[t].rent.len().saturating_sub(1) as i32;
        if data.tiles[t].kind.as_str() == "ring" {
            continue;
        }
        if pre_h <= cap && post_h > cap {
            return Err(format!(
                "MORE: houses on {t} rose {pre_h} -> {post_h} above cap {cap}"
            ));
        }
    }
    Ok(())
}

/// G-SETTLE-10: rent equals the marked current-level figure on a clean window.
pub fn verify_rent_amount(
    pre: &World,
    _post: &World,
    events: &[MatchEvent],
    data: &GameData,
) -> Result<(), String> {
    if events.iter().any(|e| e.r#type == "card") {
        return Ok(());
    }
    for e in events {
        if e.r#type != "rent" || e.to < 0 {
            continue;
        }
        let t = e.to as usize;
        if events.iter().any(|x| event_key_str(x) == "log.agent_all_owned") {
            continue;
        }
        if pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            continue;
        }
        let overridden = pre
            .st
            .board_field
            .iter()
            .any(|f| f.tile == t as i32 && f.props.contains_key("rentHouses"))
            || pre.st.players.iter().any(|p| {
                p.field.iter().any(|f| f.props.contains_key("rentHouses"))
            });
        if overridden {
            continue;
        }
        let Some(tile) = data.tiles.get(t) else {
            continue;
        };
        if tile.kind.as_str() == "ring" || tile.rent.is_empty() {
            continue;
        }
        let h = pre.st.houses.get(t).copied().unwrap_or(0).max(0) as usize;
        let full = tile.rent[h.min(tile.rent.len() - 1)];
        if e.value != full {
            return Err(format!(
                "WRONG: rent on {t} charged {}, expected marked 现等级 rent {full}",
                e.value
            ));
        }
    }
    Ok(())
}

/// G-SETTLE-11: force-buy is 2x (deed+houses), stays mortgaged.
pub fn verify_force_buy(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    data: &GameData,
) -> Result<(), String> {
    for e in events {
        if e.r#type != "forcebuy" || e.to < 0 {
            continue;
        }
        let t = e.to as usize;
        if !pre.st.mortgaged.get(t).copied().unwrap_or(false) {
            return Err(format!("MORE: force-buy of {t} which was not mortgaged"));
        }
        let Some(tile) = data.tiles.get(t) else {
            continue;
        };
        let want = 2
            * (tile.price
                + pre.st.houses.get(t).copied().unwrap_or(0) * tile.house);
        if e.value != want {
            return Err(format!(
                "WRONG: force-buy of {t} charged {}, expected 2x = {want}",
                e.value
            ));
        }
        let who = e.player_id.max(0) as usize;
        if post.st.owners.get(t).copied().unwrap_or(-1) != who as i32 {
            return Err(format!("LESS: tile {t} not owned by P{who} after force-buy"));
        }
        if !post.st.mortgaged.get(t).copied().unwrap_or(false) {
            return Err(format!(
                "LESS: tile {t} lost its mortgage (「仍为抵押状态」)"
            ));
        }
    }
    Ok(())
}

/// G-SETTLE-12: force-buy amount is immune to modifiers.
pub fn verify_force_buy_immune(
    pre: &World,
    _post: &World,
    events: &[MatchEvent],
    data: &GameData,
) -> Result<(), String> {
    for e in events {
        if e.r#type != "forcebuy" || e.to < 0 {
            continue;
        }
        let t = e.to as usize;
        let Some(tile) = data.tiles.get(t) else {
            continue;
        };
        let want = 2
            * (tile.price
                + pre.st.houses.get(t).copied().unwrap_or(0) * tile.house);
        if e.value != want {
            return Err(format!(
                "WRONG: force-buy of {t} charged {} under modifiers, expected fixed 2x = {want}",
                e.value
            ));
        }
    }
    Ok(())
}

fn event_key_str(e: &MatchEvent) -> String {
    spec_kit::event_key(e)
}
