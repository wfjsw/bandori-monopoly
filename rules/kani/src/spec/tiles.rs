//! Unit `tiles`: reference specs FROM the clause docs only.
//!
//! Sources (verbatim quotes live in the module docs of each spec block):
//! * `docs/rulebook/rulebook-doc.md` 「基础[结算]规则」 (lines 148–162)
//! * `docs/rulebook/spec/global/settle.md` (`G-SETTLE-*`)
//! * `docs/rulebook/spec/global/money.md` (`G-MONEY-*`) where the money figure
//!   is part of the settle (buy / rent / force-buy / agent half).
//!
//! Never derived from engine behaviour. Each function is the clause's
//! Trigger / Condition / Effect / Frame as an executable predicate on
//! pre + effect-log + post.
//!
//! ## Host-model notes (not spec — see `docs/rulebook/spec/kani/tiles.md`)
//!
//! Several settle primitives (`offer_buy` / `offer_build` / `pay_rent` /
//! `offer_force_buy` / `agent_offer` / `draw_event`) and the move-trigger
//! readers (`trig_move_main` / `trig_move_from` / …) are PROTOCOL-LEVEL stubs
//! or unmodeled `Op`s in `rules/kani/src/host.rs`. Specs below are still
//! written from the book; harnesses that cannot evaluate them yet are listed
//! BLOCKED-HOST / PROTOCOL-LEVEL in the unit table rather than weakened.

use crate::world::Effect;

// ===========================================================================
// G-SETTLE-02 — CiRCLE / 江户川乐器店
// 「CiRCLE和江户川乐器店的[结算]是：抽取一张手卡。」
// (`rulebook-doc.md:150`, settle.md G-SETTLE-02)
// ===========================================================================

/// Exactly one hand-card draw for `player`, and nothing else.
/// Narration (`ctx::log`) is not in the frame.
pub fn g_settle_02_draw_one_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::Draw { player: p, n: 1 } if p == player)
}

/// MUTANT: the reference wrongly wants two cards. Used to show the iff is
/// non-vacuous (the body draws one, not two).
pub fn g_settle_02_draw_one_ok_mutant_two(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::Draw { player: p, n: 2 } if p == player)
}

/// MUTANT: the reference wrongly attributes the draw to another seat.
pub fn g_settle_02_draw_one_ok_mutant_wrong_player(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::Draw { player: p, n: 1 } if p != player)
}

// ===========================================================================
// G-SETTLE-03 — CiRCLE咖啡厅 / 流星堂
// 「CiRCLE咖啡厅和流星堂的的[结算]是：抽取一张手卡，然后抽取一个事件卡。」
// (`rulebook-doc.md:152`, settle.md G-SETTLE-03)
//
// G-SETTLE-04 / 05 (event publicity, event-discard, reshuffle) are the
// engine's `H.DrawEvent` half — see settle.md; they are not this body's
// effects. Under the Kani host `draw_event` is a protocol stub (returns 0,
// logs no effect), so the *observable* half is the hand draw alone.
// ===========================================================================

/// Observable half of G-SETTLE-03: exactly one hand-card draw, then the
/// event draw (unobservable under the current host stub).
pub fn g_settle_03_hand_draw_ok(effects: &[Effect], player: i32) -> bool {
    g_settle_02_draw_one_ok(effects, player)
}

/// Full book shape of G-SETTLE-03 (hand draw + event draw), for when the
/// host models `draw_event`. `event_drawn` is the post-count of event draws
/// the host recorded (currently always 0).
pub fn g_settle_03_full_ok(effects: &[Effect], player: i32, event_drawn: i32) -> bool {
    g_settle_02_draw_one_ok(effects, player) && event_drawn == 1
}

// ===========================================================================
// G-SETTLE-01 / G-SETTLE-06 / 07 / 08 / 09 / 10 / 11 / 12 — property / RiNG
// (`rulebook-doc.md:155-161`)
//
// The body (`rules/tiles/src/property.rs`) is a four-way branch on
// (owner, mortgaged, move_is_main). Each leg is an engine primitive
// (`offer_buy` / `offer_build` / `pay_rent` / `offer_force_buy`). The money
// figures live in `G-MONEY-07/09/11/12`.
// ===========================================================================

/// Which leg of 基础[结算] 3/5/6 fires. `owner < 0` = unowned;
/// `owner == player` = own deed; else other's. `main` is the turn's
/// [主要移动] (the body *announces* on main and defers the offer — see
/// settle.md Findings LESS).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PropertyLeg {
    /// 3. unowned → optional buy of `price + houses * house`.
    UnownedBuy,
    /// 5.1 own + un-mortgaged → optional build of `house`, under the cap.
    OwnBuild,
    /// 5.2 own + mortgaged → no effect.
    OwnMortgagedNoop,
    /// 6.1 other's + un-mortgaged → pay rent[h] to the owner.
    OtherRent,
    /// 6.2 other's + mortgaged → optional force-buy of `2*(price+houses*house)`,
    /// deed stays mortgaged, amount immune to modifiers.
    OtherForceBuy,
}

/// Pure branch predicate from the quote. `main` is *not* part of the leg —
/// the book makes the optional buy/build *the settle* regardless of move kind.
/// (The engine's main-landing announce-only path is the LESS finding.)
pub fn g_settle_property_leg(owner: i32, player: i32, mortgaged: bool) -> PropertyLeg {
    if owner < 0 {
        PropertyLeg::UnownedBuy
    } else if owner != player {
        if mortgaged {
            PropertyLeg::OtherForceBuy
        } else {
            PropertyLeg::OtherRent
        }
    } else if mortgaged {
        PropertyLeg::OwnMortgagedNoop
    } else {
        PropertyLeg::OwnBuild
    }
}

/// G-MONEY-11 / G-SETTLE-06: buy price = deed price + standing houses * house cost.
pub fn buy_total(price: i32, houses: i32, house_cost: i32) -> i32 {
    price + houses * house_cost
}

/// G-MONEY-07 / G-SETTLE-11: force-buy price = 2 × (price + houses * house).
pub fn force_buy_total(price: i32, houses: i32, house_cost: i32) -> i32 {
    2 * buy_total(price, houses, house_cost)
}

/// G-MONEY-09 / G-SETTLE-10: rent is the marked current-level table entry.
/// `rent_table[h]` is 「现等级地租」. OPEN-RING-RENT: RiNG has an empty table.
pub fn rent_at_level(rent_table: &[i32], houses: i32) -> Option<i32> {
    if houses < 0 {
        return None;
    }
    rent_table.get(houses as usize).copied()
}

/// G-SETTLE-08: level cap = marked rent-table length − 1 (「每块地有标注的
/// 等级上限」). RiNG's table is empty → cap 0 (「所有RiNG不可升级」).
pub fn level_cap(rent_table: &[i32]) -> i32 {
    (rent_table.len() as i32) - 1
}

/// G-SETTLE-07 / G-MONEY-12: build costs the marked 「房屋建筑费」 and raises
/// houses by 1, only while `houses < level_cap`.
pub fn build_ok(pre_houses: i32, rent_table: &[i32], mortgaged: bool) -> bool {
    !mortgaged && pre_houses < level_cap(rent_table)
}

/// G-SETTLE-01: a mortgaged deed charges **no rent**.
pub fn g_settle_01_no_rent_on_mortgaged(mortgaged: bool) -> bool {
    mortgaged
}

// ===========================================================================
// G-SETTLE-13 / 14 — [地产商]
// 「若与该格子同色的所有[可购买格子]均已属于其他玩家则需向该格子同色的所有
//  [可购买格子]从前到后依次进行一次半价收费的[结算]（向上取整10），
//  否则可选择该格子同色的无主或玩家拥有的[可购买格子]之一进行一次[结算]。」
// (`rulebook-doc.md:162`)
// ===========================================================================

/// The agent's branch. `same_owned_by_other[i]` is true when same-colour
/// buyable `i` is owned by someone other than the mover. `same` is in board
/// (「从前到后」) order.
pub fn g_settle_13_all_owned_by_other(owners: &[i32], player: i32) -> bool {
    !owners.is_empty() && owners.iter().all(|&o| o >= 0 && o != player)
}

/// G-MONEY-10: half charge, 「向上取整10」 → `ceil(full / 2 / 10) * 10`.
/// 301 → 160 (not banker's rounding). `full` is the marked current-level rent.
pub fn agent_half_charge(full: i32) -> i32 {
    if full <= 0 {
        return 0;
    }
    // ceil(full / 2 / 10) * 10 == ceil(full / 20) * 10
    let half = (full + 1) / 2; // ceil(full/2)
    let tens = (half + 9) / 10; // ceil(half/10)
    tens * 10
}

/// G-SETTLE-14: the choose-one branch may settle at most one same-colour
/// buyable that is unowned or self-owned. Other-owned tiles are never legal
/// targets of this branch.
pub fn g_settle_14_legal_target(owner: i32, player: i32) -> bool {
    owner < 0 || owner == player
}

// ===========================================================================
// [CiRCLE奖励] — `rulebook-doc.md:151` + 专有名词
// 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]。」
// 「[CiRCLE奖励]：[获得]2000资金或抽1张卡。」
// (settle.md: owned by G-MOVE for the [经过] gate; the reward itself is here)
// ===========================================================================

/// The reward constant (专有名词).
pub const CIRCLE_MONEY: i32 = 2000;

/// Pass gate: 「[经过]CiRCLE且[移动起点]不为CiRCLE」.
/// `start < 0` = no move in flight (a card calling the reward outright) —
/// the clause does not bar it (reading recorded in `rules/tiles/src/circle.rs`).
pub fn circle_pass_pays(at: i32, self_tile: i32, mover: i32, exile: i32, start: i32, start_is_start: bool, at_is_start: bool) -> bool {
    at == self_tile
        && mover >= 0
        && exile <= 0
        && !(start >= 0 && start_is_start && at_is_start)
}

/// Money half: exactly one `GainMoney` of 2000 to the mover.
pub fn circle_reward_money_ok(effects: &[Effect], mover: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == mover && n == CIRCLE_MONEY)
}

/// Card half: exactly one `Draw` of 1 to the mover.
pub fn circle_reward_card_ok(effects: &[Effect], mover: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::Draw { player: p, n: 1 } if p == mover)
}

/// `pick == 0` is the money option, `pick == 1` the card option (the body's
/// `REWARD_MONEY` / `REWARD_CARD`).
pub fn circle_reward_effects_ok(effects: &[Effect], mover: i32, pick: i32) -> bool {
    if pick == 0 {
        circle_reward_money_ok(effects, mover)
    } else {
        circle_reward_card_ok(effects, mover)
    }
}

// ===========================================================================
// Shared helpers
// ===========================================================================

/// Frame: money of every seat outside `a`/`b` is unchanged; used by rent /
/// force-buy / buy legs (G-MONEY-01 「only named players change」).
pub fn frame_others_money_unchanged(pre: &[i32; 4], post: &[i32; 4], a: i32, b: i32, n: i32) -> bool {
    for i in 0..n {
        let i = i as usize;
        if (i as i32) != a && (i as i32) != b && pre[i] != post[i] {
            return false;
        }
    }
    true
}
