//! Unit `general`: reference specs FROM the clause docs only.
//!
//! Source of truth: `docs/rulebook/spec/cards/general.md` +
//! `docs/rulebook/cards.json` keys `通用:*`. Never derived from engine behaviour.
//!
//! Effect-log notes (host model limits, not book re-reads):
//! * `Op::add_to_deck` / `Op::to_discard` mutate piles but push **no** `Effect`
//!   (there is no `AddToDeck` / `ToDiscard` variant). Those halves are checked
//!   on post-state (pile membership / counts).
//! * `Op::set_dest` writes `World::dest` only (no `Effect`). The dest is part
//!   of the body's contract and is checked on post-state.
//! * `Op::play_doubled` is a protocol stub returning `0`, and `ctx::n(0, v)`
//!   therefore doubles every tagged-0 number. Specs of clauses whose printed
//!   amount is read through `ctx::n(0, …)` are written from the book (not from
//!   that stub); the corresponding harnesses are BLOCKED-HOST until the stub
//!   defaults to "no doubling".
//!
//! OPEN items are proved only as far as the text determines; see the unit table
//! in `docs/rulebook/spec/kani/general.md`.

use crate::world::{Effect, Snapshot, MAX_CARDS, MAX_PLAYERS};

// ===========================================================================
// C-GEN-recruit-01
// 「[手]：[消耗]1500资金，抽1张卡。」
// Trigger: the [手] effect, played from hand.
// Condition: the user has at least 1500 money (shortfall is a bankruptcy path
//            -- a different clause).
// Effect: money falls by exactly 1500; exactly one card is drawn.
// Frame: only the user's money / hand / draw move. (The played card entering
//        the discard is the ordinary non-[持续] play protocol, outside the body.)
// OPEN-GEN-recruit-once: 「1回限定」 is not defined; not contracted.
// ===========================================================================

/// Exactly the two body effects, in order: pay 1500, draw 1.
/// Narration is not in the frame.
pub fn recruit_01_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 2
        && matches!(effects[0], Effect::PayMoney { player: p, n } if p == player && n == 1500)
        && matches!(effects[1], Effect::Draw { player: p, n } if p == player && n == 1)
}

/// Post: user's money is pre - 1500 (pre >= 1500 by the clause condition).
pub fn recruit_01_post_ok(post: &Snapshot, player: i32, pre_money: i32) -> bool {
    post.money[player as usize] == pre_money - 1500
}

/// MUTANT: the reference wants a 1400 spend.
pub fn recruit_01_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 2
        && matches!(effects[0], Effect::PayMoney { player: p, n } if p == player && n == 1400)
        && matches!(effects[1], Effect::Draw { player: p, n } if p == player && n == 1)
}

// ===========================================================================
// C-GEN-great-01 / 02 / 03
// 「依次进行以下操作：1. [移除]此卡；2. 将一张“PERFECT“加入抽卡区并洗切；
//   3. [获得]2000资金。」
// 01: the card leaves every zone (dest = Banished).
// 02: exactly one 通用:[衍生]PERFECT is in the user's draw pile.
// 03: user's money rises by exactly 2000.
// ===========================================================================

/// Dest constant for 「[移除]」 (`Dest::Banished`).
pub const DEST_BANISHED: i32 = 3;

/// Exactly one logged effect: the +2000 gain.
/// (add_to_deck / set_dest are unlogged in the host model; see module docs.)
pub fn great_01_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 2000)
}

/// Post 01: dest is Banished. Post 02: deck holds exactly one more PERFECT
/// (count may include a pre-existing one; the delta is +1). Post 03: money +2000.
pub fn great_post_ok(
    post_dest: i32,
    pre_deck_perfect: i32,
    post_deck_perfect: i32,
    pre_money: i32,
    post_money: i32,
) -> bool {
    post_dest == DEST_BANISHED
        && post_deck_perfect == pre_deck_perfect + 1
        && post_money == pre_money + 2000
}

/// MUTANT: the reference wants +2500.
pub fn great_01_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 2500)
}

// ===========================================================================
// C-GEN-perfect-01 / 02 / 03
// 「依次进行以下操作：1. [移除]此卡；2. 将一张“FEVER!“加入弃卡区；
//   3. [获得]3000资金。」
// 01: dest = Banished. 02: one 通用:[衍生]FEVER! enters the discard.
// 03: money +3000.
// ===========================================================================

/// Exactly one logged effect: the +3000 gain.
pub fn perfect_01_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 3000)
}

/// Post: dest Banished, discard holds one more FEVER!, money +3000.
pub fn perfect_post_ok(
    post_dest: i32,
    pre_discard_fever: i32,
    post_discard_fever: i32,
    pre_money: i32,
    post_money: i32,
) -> bool {
    post_dest == DEST_BANISHED
        && post_discard_fever == pre_discard_fever + 1
        && post_money == pre_money + 3000
}

/// MUTANT: the reference wants +2500.
pub fn perfect_01_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 2500)
}

// ===========================================================================
// C-GEN-fever-01
// 「[手]：将此卡放置在[使用者]的[场地]。」
// Effect: a field instance sits on the user's field (owner == user == player).
// ===========================================================================

/// Dest constant for 「[场地]」 (`Dest::Field`).
pub const DEST_FIELD: i32 = 2;

/// Exactly one logged effect: PlaceCard on Field.
pub fn fever_01_log_ok(effects: &[Effect], player: i32, this_card: u8) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::PlaceCard { player: p, dest, card } if p == player && dest == DEST_FIELD && card == this_card
        )
}

/// MUTANT: the reference wants dest = Graveyard.
pub fn fever_01_log_ok_mutant(effects: &[Effect], player: i32, this_card: u8) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::PlaceCard { player: p, dest, card } if p == player && dest == 0 && card == this_card
        )
}

// ===========================================================================
// C-GEN-fever-02 / 03
// 「[持续]（1）[拥有者]被[支付]或[获得]资金时将金额额外提高X；
//   X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）。」
// 02: any [获得] to the owner while the card is on their field.
// 03: any [支付] to the owner (a pure transfer) -- the transferred amount is
//     raised by X.
// OPEN / BLOCKED: the body reads the printed 600 through `ctx::n(0, 600)`;
// the `play_doubled` host stub returns 0, which `n(0, …)` treats as "double
// the 0th number". The BOOK value is 600. See the unit table.
// ===========================================================================

/// X = 600 - 200 * n, free to go below 0. `n` = number of face-up cards on
/// the owner's field, this instance included (「场上每拥有一张卡」).
pub fn fever_x(n_cards: i32) -> i32 {
    600 - 200 * n_cards
}

/// Post pay-amount the clause demands (floor 0).
/// Book: 「将金额额外提高X」 -- amount becomes max(amount + X, 0).
/// (The body also floors at 0; the book's 「提高」 with a negative X is a
/// lower, still floored by the C#-equivalent `max(0, …)` the source uses.
/// The printed text does not name a floor; the source's `Math.Max(0, …)` is
/// the standing reading -- OPEN-GEN-fever-floor if the sheet later says else.)
pub fn fever_02_amount(pre_amount: i32, x: i32) -> i32 {
    (pre_amount + x).max(0)
}

/// Exactly one logged effect: the pay-amount rewrite.
pub fn fever_02_log_ok(effects: &[Effect], expected_to: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::SetPayAmount { to } if to == expected_to)
}

/// MUTANT of [`fever_x`]: off-by-one high.
pub fn fever_x_mutant(n_cards: i32) -> i32 {
    fever_x(n_cards) + 1
}

// ===========================================================================
// C-GEN-fever-04
// 「[持续]（2）[拥有者]回合开始时将此卡放入[使用者]弃卡区。」
// Effect: the instance leaves the field; dest becomes Graveyard (弃卡区).
// Note: the destination is the **user**'s discard when owner != user
// (normal play: same seat).
// ===========================================================================

/// Dest constant for 「弃卡区」 (`Dest::Graveyard`).
pub const DEST_GRAVEYARD: i32 = 0;

/// Exactly zero logged effects (set_dest is unlogged); post dest is Graveyard.
pub fn fever_04_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// MUTANT: the reference wants dest = Banished.
pub fn fever_04_dest_ok_mutant(post_dest: i32) -> bool {
    post_dest == DEST_BANISHED
}

// ===========================================================================
// C-GEN-budokan-01
// 「将X设为2000÷“[使用者]以外的[存活]玩家数量”向上取整10，
//   Y设为[存活]玩家数量减1。[指定][使用者]以外的所有玩家，
//   被[指定]的玩家[支付][使用者]X资金。」
// X is a pure function of alive_others. Y is printed and unused
// (OPEN-GEN-budokan-Y). The [指定] gate is a protocol stub under Kani
// (`Op::target` returns 0) -- the transfer loop is PROTOCOL-LEVEL.
// ===========================================================================

/// `X = ceil_to_10(2000 / alive_others)`; 2000/3 → 670.
/// 「向上取整10」 = round **up** to a multiple of 10.
/// `alive_others == 0` is the empty-body case (the source returns early).
pub fn budokan_x(alive_others: i32) -> i32 {
    if alive_others <= 0 {
        0
    } else {
        ((2000 + 10 * alive_others - 1) / (10 * alive_others)) * 10
    }
}

/// One transfer of X from `from` to the user is exactly what the book demands
/// for one designated player.
pub fn budokan_transfer_ok(effects: &[Effect], from: i32, to: i32, x: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::PayTo { from: f, to: t, n } if f == from && t == to && n == x)
}

/// MUTANT of [`budokan_x`]: floor-to-10 instead of ceil-to-10.
pub fn budokan_x_mutant(alive_others: i32) -> i32 {
    if alive_others <= 0 {
        0
    } else {
        (2000 / (10 * alive_others)) * 10
    }
}

// ===========================================================================
// C-GEN-rain-01
// 「投掷2d2并记录结果为X。[指定]X名玩家（其中必须包括[使用者]），
//   被[指定]的玩家获得一层[停留]。」
// X = sum of 2d2 (in 2..=4). The user is a mandatory target. The [指定]
// pipeline is a protocol stub under Kani (`Op::target`) -- the designation
// loop is PROTOCOL-LEVEL. The stay grant itself is modeled.
// ===========================================================================

/// Minimum / maximum of 2d2.
pub const RAIN_X_LO: i32 = 2;
pub const RAIN_X_HI: i32 = 4;

/// Exactly the stay grants, one `GiveStay { n: 1 }` per designated player.
/// Order follows the body's `targets` vector (user first).
pub fn rain_01_log_ok(effects: &[Effect], targets: &[i32]) -> bool {
    if effects.len() != targets.len() {
        return false;
    }
    let mut i = 0usize;
    while i < targets.len() {
        if !matches!(effects[i], Effect::GiveStay { player: p, n } if p == targets[i] && n == 1) {
            return false;
        }
        i += 1;
    }
    true
}

/// MUTANT: the reference wants GiveStay n = 2.
pub fn rain_01_log_ok_mutant(effects: &[Effect], targets: &[i32]) -> bool {
    if effects.len() != targets.len() {
        return false;
    }
    let mut i = 0usize;
    while i < targets.len() {
        if !matches!(effects[i], Effect::GiveStay { player: p, n } if p == targets[i] && n == 2) {
            return false;
        }
        i += 1;
    }
    true
}

// ===========================================================================
// C-GEN-party-01 / 02 / 03
// 「所有其他玩家可选择[消耗]500资金，你消耗500资金，
//   将X设为因此卡[消耗]资金的玩家数量加1。根据X进行以下操作之一：
//   1. X至少为2则[使用者]投掷Xd20，如果结果至少为35则[使用者][获得]3000资金
//      且其他因此卡[消耗]资金的玩家[获得]1500资金；
//   2. X等于1则[使用者]的本回合结束后获得一个额外回合。」
// X = (others who spent) + 1  (user ruling 2026-10-06: 「加1」 supplies the
// user; X does not count the user in the base).
// 「至少为35」 is inclusive (sheet A12).
// give_extra_turn is a protocol stub (returns 0, no effect) -- branch 03 is
// PROTOCOL-LEVEL for the extra-turn half.
// ===========================================================================

/// X = number of other spenders + 1.
pub fn party_x(other_spenders: i32) -> i32 {
    other_spenders + 1
}

/// Branch 02 payout is reachable only when the Xd20 sum is >= 35 (inclusive).
pub fn party_pays_out(roll_sum: i32) -> bool {
    roll_sum >= 35
}

/// Exactly the money flow of a successful branch-02: user pays 500, each
/// other spender pays 500 (those two phases come first), then on a hit the
/// user gains 3000 and each other spender gains 1500.
/// `spent` lists the other spenders, in the order the body paid them.
pub fn party_02_log_ok(effects: &[Effect], user: i32, spent: &[i32], hit: bool) -> bool {
    let n_spent = spent.len();
    // 1. optional other pays, 2. user pay, 3. optional payout gains
    let pays = n_spent + 1;
    let gains = if hit { 1 + n_spent } else { 0 };
    if effects.len() != pays + gains {
        return false;
    }
    let mut i = 0usize;
    while i < n_spent {
        if !matches!(effects[i], Effect::PayMoney { player: p, n } if p == spent[i] && n == 500) {
            return false;
        }
        i += 1;
    }
    if !matches!(effects[n_spent], Effect::PayMoney { player: p, n } if p == user && n == 500) {
        return false;
    }
    if hit {
        if !matches!(effects[pays], Effect::GainMoney { player: p, n } if p == user && n == 3000) {
            return false;
        }
        let mut j = 0usize;
        while j < n_spent {
            if !matches!(effects[pays + 1 + j], Effect::GainMoney { player: p, n }
                if p == spent[j] && n == 1500)
            {
                return false;
            }
            j += 1;
        }
    }
    true
}

/// MUTANT of [`party_pays_out`]: exclusive 35.
pub fn party_pays_out_mutant(roll_sum: i32) -> bool {
    roll_sum > 35
}

// ===========================================================================
// C-GEN-harvest-01
// 「[手]：立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]。」
// The main-move rewrite + settle is engine scheduling (PROTOCOL-LEVEL). The
// step-count bound is a pure contract: the chosen integer is in 1..=6.
// The body also reads the printed 6 through `ctx::n(0, 6)` -- same
// `play_doubled` stub issue as fever (BLOCKED-HOST for the printed bound).
// ===========================================================================

/// Every legal step count of 「移动1到6以内的任意整数」.
pub fn harvest_steps_ok(steps: i32) -> bool {
    (1..=6).contains(&steps)
}

/// MUTANT: off-by-one high bound.
pub fn harvest_steps_ok_mutant(steps: i32) -> bool {
    (1..=7).contains(&steps)
}

// ===========================================================================
// C-GEN-tsugu-01..04
// 「投掷3d10并根据结果依次进行以下操作：
//   1. 结果至少为22则抽1张卡；
//   2. 结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到
//      “bandori车站”并不[结算]，且可选择购买任意无主的[可购买格子]；
//   3. 结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）；
//   4. 结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]
//      改为[传送]到“bandori车站”并[结算]。」
// Branches are 「依次」 and stack (≥28 also implies ≥26 and ≥22).
// OPEN-GEN-tsugu-teleport-main, OPEN-GEN-tsugu-3-order.
// The move / buy halves are PROTOCOL-LEVEL (plan stubs). The draw half (01)
// is modeled.
// ===========================================================================

/// 3d10 sum is in 3..=30.
pub const TSUGU_SUM_LO: i32 = 3;
pub const TSUGU_SUM_HI: i32 = 30;

/// Branch predicates, in the printed order.
pub fn tsugu_ge22(sum: i32) -> bool {
    sum >= 22
}
pub fn tsugu_ge26(sum: i32) -> bool {
    sum >= 26
}
pub fn tsugu_ge28(sum: i32) -> bool {
    sum >= 28
}
pub fn tsugu_lt26(sum: i32) -> bool {
    sum < 26
}

/// Exactly one draw when sum >= 22.
pub fn tsugu_01_log_ok(effects: &[Effect], player: i32, sum: i32) -> bool {
    if sum >= 22 {
        effects.len() == 1
            && matches!(effects[0], Effect::Draw { player: p, n } if p == player && n == 1)
    } else {
        effects.is_empty()
    }
}

/// MUTANT of [`tsugu_ge22`]: exclusive 22.
pub fn tsugu_ge22_mutant(sum: i32) -> bool {
    sum > 22
}

// ===========================================================================
// C-GEN-marina-01 / exclusive
// 「[反击][使用者][经过]#1格子且#1格子受到其他效果影响时：
//   [使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果。」
// Counteraction window = PROTOCOL-LEVEL. The leaf body's contract is that the
// #1 extras are suppressed; the shipped body only logs (source TODO: missing
// move-tag ABI) -- FAILS(LESS) / BLOCKED-HOST. Exclusive gate is the name
// carrying 「[月岛麻里奈]」.
// OPEN-GEN-marina-extra.
// ===========================================================================

/// The suppression the book demands of the leaf body: the #1 pass/settle runs
/// without the tile's extra effects. Expressed as "the body must leave a
/// suppression mark the engine can honour"; the current host has no such op,
/// so the predicate is the empty-log form of a body that at least must not
/// pay / move / stay anything.
pub fn marina_01_body_quiet(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ===========================================================================
// C-GEN-parking-01 / 02 / 03
// 「[手]：将此卡放置于“Space”格子上，将其上的房屋转移至其他你拥有的格子上
//   （每个格子因此效果最多获得1层）。」
// 「[持续]：（1）此卡所在格子的[结算]改为回合结束后获得一层[停留]。
//   （2）位于此卡所在格子上的玩家无法使用角色及乐队技能。」
// Exclusive: name carries 「[都筑诗船]」.
// 01 house transfer has OPEN-GEN-parking-houses (excess houses).
// 02 settle-replace is engine scheduling. 03 skill-block is a gate.
// ===========================================================================

/// Space tile (「Space」格子). Board index of Space -- the sheet names the
/// tile 「Space」; board.json index is 42 per the clause doc (engine index).
pub const TILE_SPACE: i32 = 42;

/// Each destination tile of the house move gains at most +1 level from this
/// effect (「每个格子因此效果最多获得1层」).
pub fn parking_house_delta_ok(delta: i32) -> bool {
    delta <= 1
}

/// MUTANT: the reference allows +2.
pub fn parking_house_delta_ok_mutant(delta: i32) -> bool {
    delta <= 2
}

// ===========================================================================
// C-GEN-encore-01
// 「[反击][使用者]即将因任何原因受到[异常移动效果]影响时：
//   无效此次[异常移动效果]和其导致的所有效果。」
// Counteraction window = PROTOCOL-LEVEL. The leaf body must negate the
// incoming abnormal (and everything it would cause). The body calls
// `trigger::negate_effect()` -- modeled as `trig_negate`.
// ===========================================================================

/// The leaf body's only contracted action is the negation; narration is not
/// in the frame. Zero `Effect`s and `trig_negate` set.
pub fn encore_01_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ===========================================================================
// C-GEN-net-01 / 02
// 「[反击]1张手卡的[手]效果或事件卡的效果生效前：根据卡和效果的类型依次进行
//   以下操作：1. 手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]；
//   2. 手卡的[手]效果且没有[指定]目标则抵消其所有的效果；
//   3. 事件卡的效果手牌则则抵消其所有的效果。」
// Counteraction window = PROTOCOL-LEVEL. Branch 03 is garbled
// (OPEN-GEN-net-event) and not contracted. OPEN-GEN-net-which-target.
// ===========================================================================

/// Branch 02: every effect of the targeted [手] is negated.
/// Branch 01: exactly one designation is cancelled (not all).
/// Both are chain-protocol; the predicates below only pin the counts.
pub fn net_02_negates_all(n_target_effects: i32) -> bool {
    n_target_effects >= 1
}

/// Branch 01: exactly one of `n_designations` is cancelled.
pub fn net_01_cancels_one(n_designations: i32, n_cancelled: i32) -> bool {
    n_designations >= 1 && n_cancelled == 1
}

// ===========================================================================
// C-GEN-clearcp-01..04
// 「（1）[使用者]使用此卡后的下2回合开始时，此卡在格子上添加的[CP点]及其产物
//   将在相邻的没有[CP点]的格子添加1个[CP点]。」
// 「[手]：在任意一个没有角色和[CP点]的格子上添加1个[CP点]并在自己[场上]添加
//   6个[CP点]。在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]
//   1个[CP点]，[获得]800资金。」
// 01: one tile [CP点] + six on-card [CP点].
// 02: settle on a CP tile spends one tile CP and one on-card CP; settler gains 800.
// 03: each of the next two turn starts: spread (PROTOCOL-LEVEL multi-turn).
// 04: at on-card count 0 the card leaves the field.
// Printed 「移除格子上的个[CP点]」 omits the numeral; read as **1**.
// OPEN-GEN-clearcp-neighbour, OPEN-GEN-clearcp-window.
// ===========================================================================

/// The play grants six on-card CP (「自己[场上]添加6个[CP点]」).
pub const CLEARCP_START: i32 = 6;

/// One settle spends exactly 1 tile CP + 1 on-card CP and pays the settler 800.
pub fn clearcp_02_effects_ok(effects: &[Effect], settler: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == settler && n == 800)
}

/// After one settle the on-card count is start - 1.
pub fn clearcp_after_settle(n: i32) -> bool {
    n == CLEARCP_START - 1
}

/// When the on-card count hits 0 the card leaves the field (user ruling
/// 2026-10-07).
pub fn clearcp_graveyards_at_zero(n: i32) -> bool {
    n == 0
}

/// MUTANT: the reference wants 1000 paid on settle.
pub fn clearcp_02_effects_ok_mutant(effects: &[Effect], settler: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == settler && n == 1000)
}

// ===========================================================================
// Shared helpers
// ===========================================================================

/// True when the snapshot's contracted money cell equals `expected`.
pub fn money_is(post: &Snapshot, player: i32, expected: i32) -> bool {
    post.money[player as usize] == expected
}

/// Frame: every other player's money is unchanged.
pub fn others_money_unchanged(
    pre: &Snapshot,
    post: &Snapshot,
    n: i32,
    except: i32,
) -> bool {
    let mut p = 0i32;
    while p < n && p < MAX_PLAYERS as i32 {
        if p != except && pre.money[p as usize] != post.money[p as usize] {
            return false;
        }
        p += 1;
    }
    true
}

/// Frame: the two card-slot bitsets are unchanged (used when the clause does
/// not move cards through the bitset views).
pub fn bits_unchanged(pre: &Snapshot, post: &Snapshot) -> bool {
    let mut i = 0usize;
    while i < MAX_CARDS {
        if pre.hand[i] != post.hand[i] || pre.discard[i] != post.discard[i] {
            return false;
        }
        i += 1;
    }
    true
}