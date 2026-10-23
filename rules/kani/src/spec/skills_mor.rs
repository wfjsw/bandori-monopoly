//! Unit `skills_mor`: reference specs FROM the clause docs only.
//!
//! Sources: `docs/rulebook/spec/cards/mor.md` sections "Character skills" and
//! "Band skill"; the live sheet text in the roster / `the roster`.
//! Never derived from engine behaviour. Quotes are copied into each clause's
//! section below.
//!
//! Classification tags used in this file:
//! * **PROVEN candidate** — straight-line hook/play body the proofs drive.
//! * **PROTOCOL-LEVEL** — multi-turn windows, settle chains, plan scheduling,
//!   borrowed-skill dispatch, mark-namespace internals the book does not define.
//! * **OPEN-…** — text undefined (see the OPEN index in `mor.md`).
//! * **BLOCKED-HOST** — the body reaches an unmodeled / mis-modeled host op.

use crate::world::Effect;

// ===========================================================================
// C-MOR-band-01  `skill:Morfonica:振翅高飞的练习曲` (1)
// 「（1）当你一次性失去2000以上资金时，抽一张卡。」
//
// Trigger: a single loss of ≥ 2000 money (「一次性」 = one loss event;
// 「2000以上」 is inclusive of 2000).
// Condition: the seat carries the Morfonica band skill (host pre
// `actor == owner`, i.e. the trigger is this seat's own money move) and the
// move is a **loss** (the player is not the payee).
// Effect: exactly one draw.
// Frame: the player's hand / draw / discard and money (money is unchanged by
// the draw itself).
// Classification: **PROVEN candidate** (PayAfter hook, no prompt).
// ===========================================================================

/// The clause's trigger+condition residual over the hook payload.
///
/// `value` is the size of the one money move; `target` is the payee
/// (`-1` / bank when the money leaves to the bank). The player is losing
/// money iff they are **not** the payee of their own move.
///
/// Book: 「当你一次性失去2000以上资金时」 — loss, size ≥ 2000 (inclusive).
pub fn band_01_should_draw(value: i32, target: i32, player: i32) -> bool {
    value >= 2000 && target != player
}

/// Exact effect-log: one `Draw{player, 1}`. Narration is not in the frame.
pub fn band_01_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::Draw { player: p, n: 1 } if p == player)
}

/// MUTANT of [`band_01_should_draw`]: fires at 1999 (off-by-one low).
/// A correct body must NOT match this.
pub fn band_01_should_draw_mutant(value: i32, target: i32, player: i32) -> bool {
    value >= 1999 && target != player
}

// ===========================================================================
// C-MOR-band-02  `skill:Morfonica:振翅高飞的练习曲` (2)
// 「（2）[CiRCLE奖励]选择[获得]资金时根据选择[获得]资金次数设资金量为1000，1500，2000的循环」
//
// Trigger: a [CiRCLE奖励] choice of the money option.
// Condition: the seat carries the Morfonica band skill; the choice is the
// money half (`REWARD_MONEY`).
// Effect: the amount is 1000 / 1500 / 2000 cycling by the count of money
// rewards taken so far (1st = 1000, 2nd = 1500, 3rd = 2000, 4th = 1000, …);
// the reward is replaced by that fixed sum (the original payout is skipped).
// Frame: the player's money (the per-player taken counter is skill-internal).
// Classification: **PROVEN candidate** for the cycle + gain. The
// 「替换奖励」 cancel leg is `trigger::set_cancelled()`; see the host note in
// `proofs/skills_mor.rs` (Kani `trig_set_cancelled` currently writes `a0 != 0`
// with `a0 == 0`, so the flag cannot be observed — **BLOCKED-HOST**).
// ===========================================================================

/// Money-option discriminant (`card_sdk::abi::REWARD_MONEY`).
/// Book: 「[CiRCLE奖励]选择[获得]资金」 — only the money half cycles.
pub const BAND_02_REWARD_MONEY: i32 = 0;

/// True when the CiRCLE reward pick is the money option.
pub fn band_02_is_money_option(value: i32) -> bool {
    value == BAND_02_REWARD_MONEY
}

/// The cycling amount. `taken` is the number of money rewards already taken
/// **before** this one (0-based). Book: 「1000，1500，2000的循环」 with
/// 1st = 1000 → `taken % 3 == 0`.
pub fn band_02_amount(taken: i32) -> i32 {
    match taken.rem_euclid(3) {
        0 => 1000,
        1 => 1500,
        _ => 2000,
    }
}

/// Exact effect-log for one cycle step:
/// 1. the taken counter advances by one (`SetState`);
/// 2. the player gains the cycling amount (`GainMoney`).
/// The book's frame names only money; the counter write is the cycle's own
/// bookkeeping and is therefore part of the body's effect log (not narration).
pub fn band_02_log_ok(effects: &[Effect], player: i32, taken: i32) -> bool {
    if effects.len() != 2 {
        return false;
    }
    let amount = band_02_amount(taken);
    matches!(
        effects[0],
        Effect::SetState { player: p, n, .. } if p == player && n == taken + 1
    ) && matches!(
        effects[1],
        Effect::GainMoney { player: p, n } if p == player && n == amount
    )
}

/// MUTANT of [`band_02_amount`]: cycle rotated by one (`(taken+1) % 3`).
pub fn band_02_amount_mutant(taken: i32) -> i32 {
    band_02_amount(taken + 1)
}

// ===========================================================================
// C-MOR-touko-01  `skill:桐谷透子:天上天下，唯我独尊` (1)
// 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
//
// Trigger: [经过] CiRCLE (the Pass hook onto a CiRCLE tile).
// Condition: the seat's character is 桐谷透子 (host pre
// `actor == owner && is_start(tile.id)`).
// Effect: one fire pot is gained, at most up to the cap of 1.
// Frame: the fire counter.
// Classification: **PROVEN candidate** for the request (`GainFire{player,1}`).
// The 「上限1」 cap is host-side (`game_core::gain_charge` honours `charge_max`);
// the Kani host's `Op::gain_charge` does not — the at-cap leg is
// **BLOCKED-HOST** (host-model gap), not a body-vs-book finding.
// ===========================================================================

/// Fire-pot cap from the quote (「上限1」).
pub const TOUKO_01_FIRE_MAX: i32 = 1;

/// Exact effect-log when the gain applies (below cap): one `GainFire{player,1}`.
pub fn touko_01_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::GainFire { player: p, n: 1 } if p == player)
}

/// Book post when below the cap: fire increases by exactly one.
pub fn touko_01_post_below_cap(post_fire: i32, pre_fire: i32) -> bool {
    post_fire == pre_fire + 1
}

/// Book post when at the cap: no change (the host refuses the gain).
/// Reserved for when the Kani host models the cap; see the host note above.
pub fn touko_01_post_at_cap(post_fire: i32, pre_fire: i32) -> bool {
    post_fire == pre_fire
}

/// MUTANT: demands two fire pots.
pub fn touko_01_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::GainFire { player: p, n: 2 } if p == player)
}

// ===========================================================================
// C-MOR-touko-02  `skill:桐谷透子:天上天下，唯我独尊` (2)
// 「（2）一回合一次，在你的经营阶段，你可以消耗一个火罐并指定你的一个地块，
//  指定前后各一格范围内（不包括该格子本身）的所有其他玩家支付你X，X为此地的
//  地租的一半/指定玩家数（向上取整百）。」
//
// Classification: **BLOCKED-HOST: `rent_of`**. The body reaches
// `ctx::rent_of(tile)` (unmodeled in `host.rs::dispatch`, see KANI-COVERAGE).
// OPEN-地租档: which rent step counts as 「此地的地租」 is not spelled out;
// the clause doc pins the no-house rent. Prompt + once-per-turn latch are
// secondary; they do not un-block the rent read. No harness — do not fake.
// ===========================================================================

/// Pinned arithmetic of X, given the (no-house) rent and the designated
/// player count. Book: 「此地的地租的一半/指定玩家数（向上取整百）」.
/// Kept as a pure function so the arithmetic is ready when `rent_of` lands.
pub fn touko_02_x(rent: i32, n: i32) -> i32 {
    if n <= 0 {
        return 0;
    }
    let half = rent / 2;
    let share = half / n;
    if share <= 0 {
        0
    } else {
        // 「向上取整百」 -- round up to a multiple of 100.
        ((share + 99) / 100) * 100
    }
}

// ===========================================================================
// C-MOR-nanami-01  `skill:广町七深:这是很普通的事吧？` (1)
// 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限2）」
//
// Same shape as C-MOR-touko-01 with cap 2.
// Classification: **PROVEN candidate** (request); at-cap leg **BLOCKED-HOST**
// (same Kani `gain_charge` gap).
// ===========================================================================

pub const NANAMI_01_FIRE_MAX: i32 = 2;

pub fn nanami_01_log_ok(effects: &[Effect], player: i32) -> bool {
    touko_01_log_ok(effects, player)
}

pub fn nanami_01_post_below_cap(post_fire: i32, pre_fire: i32) -> bool {
    touko_01_post_below_cap(post_fire, pre_fire)
}

pub fn nanami_01_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::GainFire { player: p, n: 2 } if p == player)
}

// ===========================================================================
// C-MOR-nanami-02 / -03
// 「（2）当有玩家在使用角色技能时，你可以支付一个火罐得到一个该角色的标记…」
// 「（3）在时机合适时候，你可以使用手中的角色标记，视为使用该角色的技能…」
//
// Classification: **PROTOCOL-LEVEL / documented-only**.
// (2) needs a foreign character-skill press in the same window plus a
// mark-namespace (OPEN-角色标记) and a 3-turn passive CD (OPEN-被动CD).
// (3) is borrowed-skill dispatch (OPEN-借用技能, OPEN-时机合适); `play_card`
// is a protocol stub that returns 0 under Kani. No fake harness.
// ===========================================================================

// ===========================================================================
// C-MOR-mashiro-01 / -02  `skill:仓田真白:向后全速前进`
// 「（1）[主动移动]时移动掷骰变为2d20（2）反方向移动」
//
// Classification: **PROTOCOL-LEVEL**. Both halves are `On::RollPlan` and only
// write plan fields (`plan::set_base_dice(2,20)` / `plan::set_reverse(true)`).
// Every `Op::set_*` plan arm in `host.rs` is a protocol stub returning 0, so
// the effect is not observable in the Kani world — the real semantics are
// engine move-plan scheduling (`KANI.md` §6). OPEN-主动移动: [主动移动] vs
// [主要移动] spelling. No harness — do not fake a proof over stub fields.
// ===========================================================================

// ===========================================================================
// C-MOR-rui-crit-01  `skill:八潮瑠唯:正论暴击` (1)
// 「（1）如果你的（2）技能或角色卡被复制，当复制者使x重置为0时，对方失去该技能（2）」
//
// Classification: **OPEN-复制 / documented-only**. The copy mechanism is not
// defined in the rulebook body. No harness.
// ===========================================================================

// ===========================================================================
// C-MOR-rui-crit-02  `skill:八潮瑠唯:正论暴击` (2)
// 「（抵押或赎回以外）当你即将获得或失去资金（在所有其他资金量改动效果生效后并
//  结果不等于0）时，roll1d20，若结果小于等于X，将X重置为0，此次获得资金翻倍或
//  无需失去资金（收款或付款方同样受到影响），若结果大于X，X+1。X初始为0」
//
// Trigger: about to gain or lose a non-zero amount of money (not mortgage /
// redeem), after all other money modifiers.
// Condition: the seat's character is 八潮瑠唯 and this seat is on either side
// of the move (OPEN-双方影响: 「收款或付款方同样受到影响」).
// Effect: 1d20 vs X (starts at 0). On ≤ X: X := 0 and the gain doubles (or
// the loss is cancelled). On > X: X := X+1.
// Frame: the money of the affected seats and the X counter.
// Classification: **PROVEN candidate** — straight-line hook over a symbolic
// 1d20 (`ctx::roll` → symbolic die). The transfer-rewriting of the *other*
// leg of a two-sided pay is OPEN-双方影响 and is not pinned beyond the
// pay-amount rewrite this body issues.
// ===========================================================================

/// X starts at 0.
pub const RUI_CRIT_02_X0: i32 = 0;

/// Post X after a 1d20 roll `n` against current X.
/// Book: on ≤ X reset to 0; on > X increment by 1.
pub fn rui_crit_02_x_after(x: i32, n: i32) -> i32 {
    if n <= x {
        0
    } else {
        x + 1
    }
}

/// Post pay-amount after a hit (`n <= x`).
/// Book: 「此次获得资金翻倍或无需失去资金」 — a gain (`amount > 0`) doubles;
/// a loss (`amount < 0`) is cancelled to 0. Zero never reaches the body.
pub fn rui_crit_02_amount_on_hit(amount: i32) -> i32 {
    if amount > 0 {
        amount * 2
    } else {
        0
    }
}

/// Post pay-amount after a miss (`n > x`): unchanged.
pub fn rui_crit_02_amount_on_miss(amount: i32) -> i32 {
    amount
}

/// Exact effect-log on a hit: X := 0, then the pay-amount rewrite.
/// `ctx::roll` / `ctx::effect` / `ctx::log` are not in the frame.
pub fn rui_crit_02_log_hit_ok(effects: &[Effect], player: i32, after: i32) -> bool {
    effects.len() == 2
        && matches!(effects[0], Effect::SetState { player: p, n: 0, .. } if p == player)
        && matches!(effects[1], Effect::SetPayAmount { to } if to == after)
}

/// Exact effect-log on a miss: only X := X+1.
pub fn rui_crit_02_log_miss_ok(effects: &[Effect], player: i32, x_after: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::SetState { player: p, n, .. } if p == player && n == x_after)
}

/// MUTANT of [`rui_crit_02_x_after`]: hit threshold is `n < x` (strict),
/// so `n == x` is treated as a miss.
pub fn rui_crit_02_x_after_mutant(x: i32, n: i32) -> i32 {
    if n < x {
        0
    } else {
        x + 1
    }
}

// ===========================================================================
// C-MOR-tsukushi-01  `skill:二叶筑紫:交给班长吧` (1)
// 「（1）当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色进行一次该动作，
//  发送给对方一个X（占位）标记并记录获得因此效果获得资金的数量，若为抽卡效果则记录
//  2200。单轮结算（比如登上武道馆的结算）中此效果最多触发一次」
//
// Trigger: a gain of money (or a draw) by the user.
// Condition: the skill has not already fired in this settle chain
// (PROTOCOL — 「单轮结算」 is one settle chain; not Kani-expressible).
// Effect (money leg, the pinned arithmetic): the action is redirected to
// another living player; that player gets an X mark; the amount is recorded
// (draw records 2200).
// Classification: **PROVEN candidate** for the money-redirect arithmetic of
// a single hook body (prompts explored). The X-mark namespace is
// **OPEN-X标记**; the once-per-settle-chain latch is **PROTOCOL-LEVEL**.
// ===========================================================================

/// Recorded figure for a draw. Book: 「若为抽卡效果则记录2200」.
pub const TSUKUSHI_01_DRAW_RECORD: i32 = 2200;

/// Exact effect-log when the user accepts the redirect of a money gain of
/// `amount` to `who`:
/// 1. the original gain is zeroed (`SetPayAmount{0}`);
/// 2. `who` receives one X mark (`AddToken`);
/// 3. the amount is recorded on the skill owner (`SetState`);
/// 4. `who` gains `amount` (`GainMoney`).
/// Mark key is OPEN-X标记 — match the token by player and count only.
pub fn tsukushi_01_log_ok(effects: &[Effect], player: i32, who: i32, amount: i32) -> bool {
    if effects.len() != 4 {
        return false;
    }
    matches!(effects[0], Effect::SetPayAmount { to: 0 })
        && matches!(
            effects[1],
            Effect::AddToken { player: p, n: 1, .. } if p == who
        )
        && matches!(
            effects[2],
            Effect::SetState { player: p, n, .. } if p == player && n == amount
        )
        && matches!(
            effects[3],
            Effect::GainMoney { player: p, n } if p == who && n == amount
        )
}

/// Declining the prompt: no effects (the original gain stays).
pub fn tsukushi_01_decline_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// MUTANT: forwards 2× the amount.
pub fn tsukushi_01_log_ok_mutant(effects: &[Effect], player: i32, who: i32, amount: i32) -> bool {
    if effects.len() != 4 {
        return false;
    }
    matches!(effects[0], Effect::SetPayAmount { to: 0 })
        && matches!(
            effects[1],
            Effect::AddToken { player: p, n: 1, .. } if p == who
        )
        && matches!(
            effects[2],
            Effect::SetState { player: p, n, .. } if p == player && n == amount
        )
        && matches!(
            effects[3],
            Effect::GainMoney { player: p, n } if p == who && n == amount * 2
        )
}

// ===========================================================================
// C-MOR-tsukushi-02  `skill:二叶筑紫:交给班长吧` (2)
// 「（2）当有角色获得金钱或抽卡时，你可以将对方的X标记翻面并代替其进行一次该动作。
//  以此效果获得的金钱数量不能超过记录数字+300，超出部分由原主获得。此效果对"抽牌"
//  动作发动时必须翻面记录数字为2200及以上的标记。」
//
// Pinned arithmetic: the user takes `min(amount, recorded+300)`; the excess
// (`amount - taken`) returns to the original player. Draw leg requires
// `recorded ≥ 2200` (PROTOCOL for how a draw is tagged `isDraw`).
// Classification: **PROVEN candidate** for the split arithmetic; mark flip
// is OPEN-X标记; the draw-tag gate is PROTOCOL-LEVEL.
// ===========================================================================

/// Split of one redirected gain. Book: 「不能超过记录数字+300，超出部分由原主获得」.
pub fn tsukushi_02_split(amount: i32, recorded: i32) -> (i32, i32) {
    let cap = recorded + 300;
    let mine = if amount < cap { amount } else { cap };
    let theirs = amount - mine;
    (mine, theirs)
}

/// The draw-claim gate. Book: 「对"抽牌"动作发动时必须翻面记录数字为2200及以上的标记」.
pub fn tsukushi_02_draw_gate_ok(recorded: i32, is_draw: bool) -> bool {
    !is_draw || recorded >= TSUKUSHI_01_DRAW_RECORD
}

// ===========================================================================
// C-MOR-tsukushi-03  `skill:二叶筑紫:交给班长吧` (3)
// 「（3）当场上除你之外的玩家都拥有一枚翻面后的X标记时，移除所有X标记，你获得
//  移除标记数量*800的资金（每次发动（3）技能时单个标记的收益减200，最低单标记收益200）。」
//
// Trigger: every other living player holds a flipped X mark (skill press).
// Effect: all X marks are removed and the user gains `n × per_mark`, where
// `per_mark` starts at 800 and drops by 200 on every later (3) fire, floor 200.
// Classification: **PROVEN candidate** for the `n × per_mark` arithmetic and
// the rate decay of a single Play body. OPEN-X标记 for the mark namespace;
// the 「每次发动」 decay counter is engine-side state the body owns.
// ===========================================================================

/// First per-mark rate. Book: 「移除标记数量*800」.
pub const TSUKUSHI_03_RATE0: i32 = 800;

/// Per-step decay, floor. Book: 「每次发动…减200，最低单标记收益200」.
pub const TSUKUSHI_03_DECAY: i32 = 200;
pub const TSUKUSHI_03_FLOOR: i32 = 200;

/// Per-mark rate for this (3) fire, given the rate at the previous fire.
/// First fire uses 800 (the body seeds RATE=800 at TurnStartBefore).
pub fn tsukushi_03_rate(pre_rate: i32) -> i32 {
    if pre_rate <= 0 {
        TSUKUSHI_03_RATE0
    } else if pre_rate < TSUKUSHI_03_FLOOR {
        TSUKUSHI_03_FLOOR
    } else {
        pre_rate
    }
}

/// Rate after this (3) fire. Book: drop by 200, floor 200.
pub fn tsukushi_03_rate_after(pre_rate: i32) -> i32 {
    let r = tsukushi_03_rate(pre_rate);
    let next = r - TSUKUSHI_03_DECAY;
    if next < TSUKUSHI_03_FLOOR {
        TSUKUSHI_03_FLOOR
    } else {
        next
    }
}

/// Total the user gains: removed-mark count × this fire's rate.
pub fn tsukushi_03_gain(n_marks: i32, pre_rate: i32) -> i32 {
    n_marks * tsukushi_03_rate(pre_rate)
}

/// Exact effect-log of one (3) charge: two token-clears per holder (UP and
/// DOWN counts written to 0 as negative deltas), then `GainMoney`, then the
/// decayed rate `SetState`. Mark keys are OPEN-X标记 — match by player and
/// delta. `holders` is `(seat, up_count, down_count)` for every seat that
/// held either face.
pub fn tsukushi_03_log_ok(
    effects: &[Effect],
    player: i32,
    holders: &[(i32, i32, i32)],
    n_marks: i32,
    pre_rate: i32,
) -> bool {
    // holders: (seat, up_count, down_count)
    let gain = tsukushi_03_gain(n_marks, pre_rate);
    let rate_after = tsukushi_03_rate_after(pre_rate);
    if effects.len() != holders.len() * 2 + 2 {
        return false;
    }
    let mut i = 0usize;
    for &(h, up, down) in holders {
        match effects[i] {
            Effect::AddToken { player: p, n, .. } if p == h && n == -up => {}
            _ => return false,
        }
        i += 1;
        match effects[i] {
            Effect::AddToken { player: p, n, .. } if p == h && n == -down => {}
            _ => return false,
        }
        i += 1;
    }
    matches!(
        effects[i],
        Effect::GainMoney { player: p, n } if p == player && n == gain
    ) && matches!(
        effects[i + 1],
        Effect::SetState { player: p, n, .. } if p == player && n == rate_after
    )
}

/// MUTANT of the rate decay: floor at 0 instead of 200.
pub fn tsukushi_03_rate_after_mutant(pre_rate: i32) -> i32 {
    let r = tsukushi_03_rate(pre_rate);
    let next = r - TSUKUSHI_03_DECAY;
    if next < 0 {
        0
    } else {
        next
    }
}
