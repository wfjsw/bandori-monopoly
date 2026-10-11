//! Unit `mor`: reference specs FROM the clause docs only.
//!
//! Sources: `docs/rulebook/spec/cards/mor.md`, `docs/rulebook/cards.json`.
//! Never derive a spec from engine behaviour. Quotes are copied into each
//! clause's module doc.

use crate::world::{Effect, Snapshot, MAX_PLAYERS};

// ===========================================================================
// C-MOR-rui-demon-01 / -02  `Mor:（Rui）正论恶魔`
// 「向全场玩家支付100资金（该数值不可被任何效果影响），之后全场玩家向你支付300资金（可被影响）。
//  在此卡结算过程中，每当你的技能被触发后，立即将x设置为5」
// ===========================================================================

/// Book: 「向全场玩家支付100…之后全场玩家向你支付300」. 「全场玩家」 is
/// every other living seat (clause doc). The 100 leg is face-value (immunity
/// is a pipeline property the clause pins as face value when unmodified).
pub fn rui_demon_01_effects_ok(
    effects: &[Effect],
    player: i32,
    n_players: i32,
    out: &[bool; MAX_PLAYERS],
) -> bool {
    // Build the expected other-living list in seat order (host `others` order).
    let mut others = [0i32; MAX_PLAYERS];
    let mut n_others = 0usize;
    for p in 0..n_players {
        let p = p as usize;
        if (p as i32) != player && !out[p] {
            others[n_others] = p as i32;
            n_others += 1;
        }
    }
    // Per other: PayTo(player→o, 100), SetState(X=5),
    // then PayTo(o→player, 300), SetState(X=5), then a final SetState(X=5).
    // X's key id is engine-side; match SetState { player, n: 5 } only.
    let want_len = n_others * 4 + 1;
    if effects.len() != want_len {
        return false;
    }
    let mut i = 0usize;
    for k in 0..n_others {
        let o = others[k];
        match effects[i] {
            Effect::PayTo { from, to, n } if from == player && to == o && n == 100 => {}
            _ => return false,
        }
        i += 1;
        match effects[i] {
            Effect::SetState { player: p, n, .. } if p == player && n == 5 => {}
            _ => return false,
        }
        i += 1;
    }
    for k in 0..n_others {
        let o = others[k];
        match effects[i] {
            Effect::PayTo { from, to, n } if from == o && to == player && n == 300 => {}
            _ => return false,
        }
        i += 1;
        match effects[i] {
            Effect::SetState { player: p, n, .. } if p == player && n == 5 => {}
            _ => return false,
        }
        i += 1;
    }
    match effects[i] {
        Effect::SetState { player: p, n, .. } if p == player && n == 5 => {}
        _ => return false,
    }
    true
}

/// Post money: every other living seat loses 100 and gains 300 (net +200 to
/// the user from each); the user pays 100 per other and receives 300 per other.
/// `pay_to` floors at 0 / saturates, so the post is the host's arithmetic; the
/// clause pins the face-value legs. We check the **face-value** deltas when no
/// floor/saturate fires (pre money large enough).
pub fn rui_demon_01_post_ok(
    post: &Snapshot,
    pre: &Snapshot,
    player: i32,
    n_players: i32,
    out: &[bool; MAX_PLAYERS],
) -> bool {
    let p = player as usize;
    let mut want_user = pre.money[p];
    for q in 0..n_players {
        let q = q as usize;
        if (q as i32) == player || out[q] {
            continue;
        }
        want_user = want_user - 100 + 300;
        // Other: -100 + 300, floored at 0 on the 100 leg then +300.
        // The host does (m-100).max(0) then +300.
        let m0 = pre.money[q];
        let want_other = (m0 - 100).max(0) + 300;
        if post.money[q] != want_other {
            return false;
        }
    }
    // User's 100-outs also floor: (m-100).max(0) per leg, then +300 per leg.
    // Recompute leg-by-leg in seat order to match the body.
    let mut m = pre.money[p];
    for q in 0..n_players {
        let q = q as usize;
        if (q as i32) == player || out[q] {
            continue;
        }
        m = (m - 100).max(0);
    }
    for q in 0..n_players {
        let q = q as usize;
        if (q as i32) == player || out[q] {
            continue;
        }
        m = m.saturating_add(300);
    }
    let _ = want_user;
    post.money[p] == m
}

/// MUTANT of [`rui_demon_01_effects_ok`]: demands a 200 leg instead of 100.
pub fn rui_demon_01_effects_ok_mutant(
    effects: &[Effect],
    player: i32,
    n_players: i32,
    out: &[bool; MAX_PLAYERS],
) -> bool {
    let mut others = [0i32; MAX_PLAYERS];
    let mut n_others = 0usize;
    for p in 0..n_players {
        let p = p as usize;
        if (p as i32) != player && !out[p] {
            others[n_others] = p as i32;
            n_others += 1;
        }
    }
    let want_len = n_others * 4 + 1;
    if effects.len() != want_len || n_others == 0 {
        return false;
    }
    // First leg must be 200 (wrong).
    matches!(
        effects[0],
        Effect::PayTo { from, to, n } if from == player && to == others[0] && n == 200
    )
}

// ===========================================================================
// C-MOR-xiaobai-01  `Mor:（小白）`
// 「[反击]当你将要向其他玩家支付时打出此卡，此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金」
// ===========================================================================

/// Body effect: the pending payment's target is rewritten to -1 (pure loss;
/// recorded on the trigger, not the effect log) and the intended payee loses
/// `amount / 2` (integer half). Exactly one effect: `PayMoney(payee, amount/2)`.
pub fn xiaobai_01_effects_ok(effects: &[Effect], payee: i32, amount: i32) -> bool {
    let half = amount / 2;
    effects.len() == 1
        && matches!(effects[0], Effect::PayMoney { player, n } if player == payee && n == half)
}

/// Post: payee money decreased by `amount/2` (floored at 0); the counteruser's
/// money is untouched by this body (the original payment rewrite is the
/// engine's pending-pay leg, not this body).
pub fn xiaobai_01_post_ok(post: &Snapshot, pre: &Snapshot, payee: i32, amount: i32) -> bool {
    let half = amount / 2;
    let p = payee as usize;
    post.money[p] == (pre.money[p] - half).max(0)
}

/// MUTANT: demands the full amount, not the half.
pub fn xiaobai_01_effects_ok_mutant(effects: &[Effect], payee: i32, amount: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::PayMoney { player, n } if player == payee && n == amount)
}

// ===========================================================================
// C-MOR-rainbow-01 / -02  `Mor:秘密与青春的虹彩`
// 「（1）当你向学妹或同级生支付时，打出此卡，此次支付金额减半。」
// 「（2）当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍。」
// Ruling 2026-10-06: grades come from a static normalised table.
// Rounding: the clause pins integer half / integer 1.5× (140 → 210).
// The body additionally CeilTo-10s; that is a candidate finding (see proofs).
// ===========================================================================

/// Clause pin: integer half (toward zero for positives = `amount / 2`).
pub fn rainbow_01_amount(amount: i32) -> i32 {
    amount / 2
}

/// Clause pin: integer 1.5× (`amount * 3 / 2`).
pub fn rainbow_02_amount(amount: i32) -> i32 {
    amount * 3 / 2
}

/// Exactly one effect: `SetPayAmount { to: expected }`.
pub fn rainbow_log_ok(effects: &[Effect], expected: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::SetPayAmount { to } if to == expected)
}

/// MUTANT of half: off-by-one high.
pub fn rainbow_01_amount_mutant(amount: i32) -> i32 {
    amount / 2 + 1
}

/// MUTANT of 1.5×: off-by-one high.
pub fn rainbow_02_amount_mutant(amount: i32) -> i32 {
    amount * 3 / 2 + 1
}

// ===========================================================================
// C-MOR-garden-01  `Mor:（筑紫）迷茫的庭园`
// 「获得100资金。」
// (The play body continues with 1d6 teleport / settle choice / immunity;
//  those are separate clauses. This pin is the gain leg.)
// ===========================================================================

/// The gain is the **first** effect of the play.
pub fn garden_01_first_effect_ok(effects: &[Effect], player: i32) -> bool {
    !effects.is_empty()
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 100)
}

/// Full play post (clauses 01–03, Kani-expressible legs): money +100, then
/// the user is at `tile_steps_ahead(chosen, 1)`. The settle choice is a prompt;
/// a skip must not move money from the landing.
pub fn garden_gain_post_ok(post: &Snapshot, pre: &Snapshot, player: i32) -> bool {
    post.money[player as usize] == pre.money[player as usize] + 100
}

// ===========================================================================
// C-MOR-courage-01 / -02 / -03  `Mor:勇气展翅高飞之时`
// 「掷骰3d20，结果对应序号格子的所有者向你支付该地块的购买价格+地块已有房子的建造价格总额的一半，
//  若为地产商地块，获得1000资金」
// Ruling 2026-10-06: land price counts in full; only the house total is halved.
// Integer half of the house total (even halves of odd house costs) is not
// specified; the clause uses the engine's integer half.
// ===========================================================================

/// 3d20 sum → 0-based tile index (result N names board tile N, index N-1).
pub fn courage_tile(roll: i32, n_tiles: i32) -> i32 {
    if n_tiles <= 0 {
        0
    } else {
        (roll - 1).rem_euclid(n_tiles)
    }
}

/// Amount the owner pays the user: `land + house_total / 2`.
/// `buy_price = land + houses * house_cost`, so `house_total = buy_price - land`.
pub fn courage_01_amount(land: i32, buy_price: i32) -> i32 {
    let house_total = buy_price - land;
    land + house_total / 2
}

/// Agent tile: the user gains 1000 and the owner-payment branch does not run.
pub fn courage_02_effects_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 1000)
}

/// Unowned (or self-owned / owner out): no money moves.
pub fn courage_03_effects_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// Owned by another living player: exactly one `PayTo(owner → user, amount)`.
pub fn courage_01_effects_ok(effects: &[Effect], owner: i32, player: i32, amount: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::PayTo { from, to, n } if from == owner && to == player && n == amount)
}

/// MUTANT of the pay amount: off-by-one high.
pub fn courage_01_amount_mutant(land: i32, buy_price: i32) -> i32 {
    courage_01_amount(land, buy_price) + 1
}

// ===========================================================================
// C-MOR-band-01 / -02  `skill:Morfonica:振翅高飞的练习曲`
// 「（1）当你一次性失去2000以上资金时，抽一张卡。」
// 「（2）[CiRCLE奖励]选择[获得]资金时根据选择[获得]资金次数设资金量为1000，1500，2000的循环」
// 「2000以上」 is inclusive. The cycle is per player.
// ===========================================================================

/// True when a single loss of `value` from this player draws one card.
/// `value >= 2000` and the player is the one losing (`target != player` means
/// the money left the player; the body skips when `target == player`).
pub fn band_01_draws(loss_from: i32, player: i32, value: i32) -> bool {
    loss_from != player && value >= 2000
}

/// Exactly one effect: `Draw { player, n: 1 }`.
pub fn band_01_effects_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::Draw { player: p, n } if p == player && n == 1)
}

/// 1st money-reward = 1000, 2nd = 1500, 3rd = 2000, 4th = 1000, …
pub fn band_02_amount(taken: i32) -> i32 {
    match taken.rem_euclid(3) {
        0 => 1000,
        1 => 1500,
        _ => 2000,
    }
}

/// Effects: `GainMoney(player, amount)` then the reward is cancelled (the
/// cancel is a trigger flag, not an effect).
pub fn band_02_effects_ok(effects: &[Effect], player: i32, amount: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == amount)
}

/// MUTANT: cycle step off by one.
pub fn band_02_amount_mutant(taken: i32) -> i32 {
    band_02_amount(taken + 1)
}

// ===========================================================================
// C-MOR-toko-swap-01  `Mor:（toko）`
// 「选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换，
//  地契+房屋价格更低的玩家向更高的玩家支付差价，互换后将你得到的地契免费赎回」
// ===========================================================================

/// Sum of deed price + built-house cost on a tile.
pub fn toko_tile_value(deed_price: i32, houses: i32, house_cost: i32) -> i32 {
    deed_price + houses * house_cost
}

/// Difference paid from the lower-valued side to the higher.
pub fn toko_diff(a: i32, b: i32) -> i32 {
    if a >= b {
        a - b
    } else {
        b - a
    }
}

/// Spec: swap owners of the two mortgaged same-group tiles; the lower-valued
/// owner pays the higher the difference; the deed the user **receives** is
/// un-mortgaged for free; the deed given away stays mortgaged.
pub fn toko_swap_effects_ok(
    effects: &[Effect],
    user: i32,
    give_tile: i32,
    get_tile: i32,
    give_owner: i32,
    get_owner: i32,
    diff: i32,
    user_gets_lower: bool,
) -> bool {
    // Order the body produces is not pinned by the book beyond the listed
    // legs; require exactly these multiset legs.
    if effects.len() != 4 {
        return false;
    }
    let mut saw_set_a = false;
    let mut saw_set_b = false;
    let mut saw_pay = false;
    let mut saw_unmort = false;
    for e in effects {
        match e {
            Effect::SetOwner { tile, player } => {
                if *tile == give_tile && *player == get_owner {
                    saw_set_a = true;
                } else if *tile == get_tile && *player == give_owner {
                    saw_set_b = true;
                } else {
                    return false;
                }
            }
            Effect::PayTo { from, to, n } => {
                let (want_from, want_to) = if user_gets_lower {
                    (get_owner, give_owner)
                } else {
                    (give_owner, get_owner)
                };
                if *from == want_from && *to == want_to && *n == diff {
                    saw_pay = true;
                } else {
                    return false;
                }
            }
            Effect::SetMortgaged { tile, on } => {
                if *tile == get_tile && !*on {
                    saw_unmort = true;
                } else {
                    return false;
                }
            }
            _ => return false,
        }
    }
    let _ = user;
    saw_set_a && saw_set_b && saw_pay && saw_unmort
}

// ===========================================================================
// C-MOR-noble-02  `Mor:高贵的微蓝` (mark leg only; OPEN-触发结算)
// 「如果此地块属于你：1.立刻进行一次触发结算。2.在该地块上放置一个标记，有标记时此地块不能被指定。」
// ===========================================================================

/// The mark leg: exactly one `PlaceMark` on the user's tile. The forced
/// settle (1) is OPEN-触发结算 and is not pinned here.
pub fn noble_02_mark_ok(effects: &[Effect], tile: i32) -> bool {
    effects.iter().any(|e| matches!(e, Effect::PlaceMark { tile: t, .. } if *t == tile))
}
// ===========================================================================
// C-MOR-courage-03 post  `Mor:勇气展翅高飞之时`
// 「结果对应序号格子的所有者向你支付…」 -- unowned branch: nobody pays.
// Host note: `is_agent` / `tile_price` / `buy_price` use 1-based
// `board::tile_slot` while the body passes the 0-based roll index; and
// `buy_price` returns land price only (houses ignored). courage-01/02 are
// BLOCKED-HOST on those ops. courage-03 is the empty-effect branch, which
// holds whenever the agent check does not misfire (see proofs isolation).
// ===========================================================================

/// Unowned / self-owned / owner-out: no money moves; effect log empty.
pub fn courage_03_post_ok(post: &Snapshot, pre: &Snapshot) -> bool {
    post.money == pre.money
}

// ===========================================================================
// C-MOR-toko-swap-01 no-pair no-op  `Mor:（toko）`
// 「选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换…」
// When no such pair exists the body logs and returns; the effect log is empty.
// ===========================================================================

/// No swappable pair: the play produces no effects (the cant_play gate would
/// also refuse; the body itself early-returns with a narration only).
pub fn toko_no_pair_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ===========================================================================
// C-MOR-starry-01 / -02 / -05 / -06  `Mor:蝴蝶飞舞的星月夜`
// 「（1）支付X次1000的的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]」
// 「此卡没有[奇迹水晶]时加入弃牌堆」
// 「（2）…此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]。」
// 「（3）此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金。」
// OPEN-X: the book gives no range; the engine offers 1..=5.
// 「场地中央」 is not a board slot; the card is a field card of the user.
// ===========================================================================

/// X ∈ 1..=max, `max = clamp(money/1000, 1, 5)` (engine offer; OPEN-X).
pub fn starry_01_x_in_range(x: i32, money: i32) -> bool {
    let max = (money / 1000).clamp(1, 5);
    (1..=max).contains(&x)
}

/// The user pays `X × 1000`.
pub fn starry_01_amount(x: i32) -> i32 {
    x * 1000
}

/// Exactly three effects in order: PayMoney(user, 1000·X), PlaceCard(user,
/// Field), AddCounter(user, crystals, X). `set_dest(Field)` is folded into
/// the PlaceCard dest (2 = Dest::Field). Counter owner is the trigger seat
/// (the play's player under a normal play).
pub fn starry_01_effects_ok(
    effects: &[Effect],
    player: i32,
    x: i32,
) -> bool {
    if effects.len() != 3 {
        return false;
    }
    matches!(
        effects[0],
        Effect::PayMoney { player: p, n } if p == player && n == 1000 * x
    ) && matches!(
        effects[1],
        Effect::PlaceCard { player: p, dest, .. } if p == player && dest == 2
    ) && matches!(
        effects[2],
        Effect::AddCounter { owner, n, .. } if owner == player && n == x
    )
}

/// Post money: the pay floors at 0.
pub fn starry_01_post_ok(post: &Snapshot, pre: &Snapshot, player: i32, x: i32) -> bool {
    let p = player as usize;
    post.money[p] == (pre.money[p] - 1000 * x).max(0)
}

/// MUTANT of the pay amount: off-by-one high.
pub fn starry_01_amount_mutant(x: i32) -> i32 {
    x * 1000 + 1
}

/// starry-06: every crystal removal pays the owner `1000 · removed`.
/// Exactly one `GainMoney` (the empty-discard dest write is not an effect).
pub fn starry_06_effects_ok(effects: &[Effect], owner: i32, removed: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player, n } if player == owner && n == 1000 * removed)
}

/// MUTANT of the payout: 500 per crystal.
pub fn starry_06_effects_ok_mutant(effects: &[Effect], owner: i32, removed: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player, n } if player == owner && n == 500 * removed)
}

/// starry-02: empty crystals → the card is sent to the discard. Under Kani the
/// body only writes `dest = Graveyard` (0); the engine's post-body unplace is
/// outside the effect log. Pin: dest becomes 0 and (with -06) the gain ran.
pub fn starry_02_dest_is_graveyard(dest: i32) -> bool {
    dest == 0
}

/// starry-05: owner passes CiRCLE → one crystal is removed.
/// Exactly one `AddCounter(owner, n: -1)`.
pub fn starry_05_effects_ok(effects: &[Effect], owner: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::AddCounter { owner: o, n, .. } if o == owner && n == -1)
}

// ===========================================================================
// C-MOR-nnm-02 / -03 / -04 / -05  `Mor:（NNM）稍微努力了一下`
// 「弃置手中x枚角色标记，发动以下效果中的一个」
// 「（1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张」
// 「（2）获得x次经过CiRCLE时的资金奖励」
// 「（3）将此卡放置在场上并放置x个奇迹水晶…」
// 「（4）当奇迹水晶耗尽时，将此卡置入弃牌堆」
// OPEN-角色标记 / OPEN-多分支选择.
// Sheet 2026-10-06 新卡组卡 G7 rewords (2) from 「x*2000资金」 to x times the
// current CiRCLE money reward (Morfonica cycle 1000/1500/2000).
// ===========================================================================

/// nnm-02: the user draws x cards. The end-of-turn discard-down is a
/// separate AtEnd entry (PROTOCOL-LEVEL scheduling).
pub fn nnm_02_effects_ok(effects: &[Effect], player: i32, x: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::Draw { player: p, n } if p == player && n == x)
}

/// nnm-03 clause pin: x times the *current* CiRCLE money reward.
pub fn nnm_03_amount(x: i32, reward: i32) -> i32 {
    x * reward
}

/// nnm-03 as the body hard-codes it (TODO(规则书) in the source): x * 2000.
pub fn nnm_03_amount_body(x: i32) -> i32 {
    x * 2000
}

/// Exactly one `GainMoney(player, amount)`.
pub fn nnm_03_effects_ok(effects: &[Effect], player: i32, amount: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == amount)
}

/// nnm-04: the card is placed with x crystals.
/// PlaceCard(user, Field) + AddCounter(user, crystals, x).
pub fn nnm_04_effects_ok(effects: &[Effect], player: i32, x: i32) -> bool {
    if effects.len() != 2 {
        return false;
    }
    matches!(
        effects[0],
        Effect::PlaceCard { player: p, dest, .. } if p == player && dest == 2
    ) && matches!(
        effects[1],
        Effect::AddCounter { owner, n, .. } if owner == player && n == x
    )
}

/// nnm-05 book pin: when the crystal count hits 0 the card is put into the
/// discard. The engine's discard is `set_dest(Graveyard)` (+ unplace); under
/// Kani the dest write is the observable. `true` when the body requested it.
pub fn nnm_05_wants_discard(dest: i32) -> bool {
    dest == 0
}

/// nnm-05 FAILS(LESS) witness predicate: empty crystals leave the card on
/// the field — no `UnplaceCard` and dest unchanged by this body.
pub fn nnm_05_body_leaves_card(effects: &[Effect], dest: i32, pre_dest: i32) -> bool {
    !effects.iter().any(|e| matches!(e, Effect::UnplaceCard { .. })) && dest == pre_dest
}

// ===========================================================================
// C-MOR-tritone-01 / -02  `Mor:迷茫之蝶们的三全音`
// 「[反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，
//  此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金」
// OPEN-三全音收款人: the payee of the payback is not named (bank vs original
// payee). The clause pins the amount and the discard, not the payee.
// ===========================================================================

/// tritone-01 body effects (counteraction window is PROTOCOL-LEVEL; this is
/// the admitted body): GainMoney(user, amount), PlaceCard(user, Field),
/// SetState(user, tritone_owed, amount). `set_crystals(3)` is a counter write
/// with **no** effect-log entry under the Kani host.
pub fn tritone_01_effects_ok(effects: &[Effect], player: i32, amount: i32) -> bool {
    if effects.len() != 3 {
        return false;
    }
    matches!(
        effects[0],
        Effect::GainMoney { player: p, n } if p == player && n == amount
    ) && matches!(
        effects[1],
        Effect::PlaceCard { player: p, dest, .. } if p == player && dest == 2
    ) && matches!(
        effects[2],
        Effect::SetState { player: p, n, .. } if p == player && n == amount
    )
}

/// After the body the card holds exactly 3 crystals (the counter write).
pub fn tritone_01_crystals_ok(crystals: i32) -> bool {
    crystals == 3
}

/// tritone-02 single turn-end tick. `pre_crystals` is the count before the
/// tick; one crystal is removed. While any remain the tick is decay-only
/// (exactly `AddCounter(owner, -1)`). When the count hits 0 the card is
/// discarded (`dest = Graveyard`) and the owner pays back `owed`.
/// OPEN-三全音收款人: the body `pay`s (bank); the clause pins the amount only.
pub fn tritone_02_tick_effects_ok(
    effects: &[Effect],
    owner: i32,
    pre_crystals: i32,
    owed: i32,
) -> bool {
    if pre_crystals > 1 {
        return effects.len() == 1
            && matches!(effects[0], Effect::AddCounter { owner: o, n, .. } if o == owner && n == -1);
    }
    if effects.len() != 2 {
        return false;
    }
    matches!(
        effects[0],
        Effect::AddCounter { owner: o, n, .. } if o == owner && n == -1
    ) && matches!(
        effects[1],
        Effect::PayMoney { player, n } if player == owner && n == owed
    )
}

/// MUTANT of the tick payback: pays owed+1.
pub fn tritone_02_tick_effects_ok_mutant(
    effects: &[Effect],
    owner: i32,
    pre_crystals: i32,
    owed: i32,
) -> bool {
    if pre_crystals > 1 {
        return false;
    }
    effects.len() == 2
        && matches!(
            effects[1],
            Effect::PayMoney { player, n } if player == owner && n == owed + 1
        )
}

// ===========================================================================
// C-MOR-noble-01 / -02  `Mor:高贵的微蓝`
// 「当你位于一块地契价值大于等于2200的地块时，可以打出此卡」
// 「如果此地块属于你：1.立刻进行一次触发结算。2.在该地块上放置一个标记…」
// OPEN-触发结算: the forced settle is not defined; only the mark is pinned.
// Host note: `tile_price(pos)` uses 1-based `board::tile_slot` while `pos` is
// 0-based — noble-01's price gate is BLOCKED-HOST.
// ===========================================================================

/// noble-01 gate: deed (land) price ≥ 2200.
pub fn noble_01_gate_ok(price: i32) -> bool {
    price >= 2200
}

/// noble-02, tile not yours: no effects (the body logs and returns).
pub fn noble_02_not_yours_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// noble-02, tile is yours: only the mark leg (forced settle is
/// OPEN-触发结算 / a `card_settle_at` protocol stub).
pub fn noble_02_effects_ok(effects: &[Effect], tile: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::PlaceMark { tile: t, .. } if t == tile)
}

// ===========================================================================
// C-MOR-again-02  `Mor:再次牵起手来` [持续]
// 「[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区。」
// The [反击] window of -01 is PROTOCOL-LEVEL. This is the PayAt hook body.
// OPEN-取消范围: one-sided on the owner (the clause pins the owner's side).
// ===========================================================================

/// Cancel the pending payment (`SetPayAmount { to: 0 }`) and send the card to
/// the discard (`dest = Graveyard`; the unplace is engine post). Exactly one
/// effect: the pay-amount rewrite to 0.
pub fn again_02_effects_ok(effects: &[Effect]) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::SetPayAmount { to } if to == 0)
}

/// MUTANT: cancels to 1, not 0.
pub fn again_02_effects_ok_mutant(effects: &[Effect]) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::SetPayAmount { to } if to == 1)
}

// ===========================================================================
// C-MOR-wing-01  `Mor:纯真振翅`
// 「传送到移动方向20格后（不触发结算）」
// OPEN-移动方向: 「移动方向」 is the turn's planned direction. Host note:
// `plan::dir()` (`Op::move_dir`) is a protocol stub that returns 0, so the
// teleport lands on the current tile — BLOCKED-HOST on `Op::move_dir`.
// The follow-on roll (wing-02) is engine scheduling (PROTOCOL-LEVEL).
// ===========================================================================

/// Destination = `pos + 20·dir` on the 60-tile ring.
pub fn wing_01_dest(pos: i32, dir: i32) -> i32 {
    (pos + 20 * dir).rem_euclid(60)
}

/// Exactly one effect: `Teleport(user, dest)`. No settle / no money.
pub fn wing_01_effects_ok(effects: &[Effect], player: i32, dest: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::Teleport { player: p, tile } if p == player && tile == dest)
}
