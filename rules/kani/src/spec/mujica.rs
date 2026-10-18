//! Unit `mujica`: reference specs FROM the clause docs only.
//!
//! Sources (verbatim quotes in `docs/rulebook/spec/cards/mujica.md` and
//! `docs/rulebook/cards.json` `Mujica:*`). Never derived from engine behaviour.
//!
//! Classification shorthand used by the proofs module and the results table
//! (`docs/rulebook/spec/kani/mujica.md`): PROVEN bodies are proved here;
//! PROTOCOL-LEVEL / OPEN / BLOCKED-HOST rows are listed in that table and
//! have no executable spec obligation beyond the pure arithmetic that the
//! text does determine.

use crate::world::{Effect, Snapshot, MAX_PLAYERS};

// ===========================================================================
// C-MUJICA-黑色生日
// 「每名你以外的资金不多于1000的玩家支付你800资金，
//  每名你以外的资金严格在1000以上的玩家支付你两次200资金。」
// 「不多于1000」 is inclusive (TEST-FINDINGS §6 sheet-superseded).
// 「严格在1000以上」 is exclusive of 1000.
// ===========================================================================

/// Bracket for one other player at resolution time.
/// `true` = low bracket (pays 800), `false` = high bracket (pays 200 twice).
pub fn black_bday_is_low_bracket(money: i32) -> bool {
    money <= 1000
}

/// Exact transfers for one solvent other-player in the low bracket.
/// The body also latches the bracket into a slot (`SetState` ×2: set then
/// clear) — those writes sit alongside the single PayTo.
pub fn black_bday_low_effects(effects: &[Effect], from: i32, to: i32) -> bool {
    let mut pays = 0;
    for e in effects {
        match e {
            Effect::PayTo { from: f, to: t, n } => {
                if *f != from || *t != to || *n != 800 {
                    return false;
                }
                pays += 1;
            }
            Effect::SetState { .. } => {}
            _ => return false,
        }
    }
    pays == 1
}

/// Exact transfers for one solvent other-player in the high bracket.
/// Same latch `SetState` writes as the low bracket.
pub fn black_bday_high_effects(effects: &[Effect], from: i32, to: i32) -> bool {
    let mut pays = 0;
    for e in effects {
        match e {
            Effect::PayTo { from: f, to: t, n } => {
                if *f != from || *t != to || *n != 200 {
                    return false;
                }
                pays += 1;
            }
            Effect::SetState { .. } => {}
            _ => return false,
        }
    }
    pays == 2
}

/// Full-play log over the ordered `others` list (solvent payers only).
/// `low[i]` is the resolution-time bracket of `others[i]`.
pub fn black_bday_log_ok(effects: &[Effect], user: i32, others: &[i32], low: &[bool]) -> bool {
    let mut i = 0usize;
    for (k, &p) in others.iter().enumerate() {
        if low[k] {
            if i >= effects.len() {
                return false;
            }
            if !matches!(effects[i], Effect::PayTo { from: f, to: t, n } if f == p && t == user && n == 800) {
                return false;
            }
            i += 1;
        } else {
            if i + 1 >= effects.len() {
                return false;
            }
            for _ in 0..2 {
                if !matches!(effects[i], Effect::PayTo { from: f, to: t, n } if f == p && t == user && n == 200) {
                    return false;
                }
                i += 1;
            }
        }
    }
    i == effects.len()
}

/// Post money for one low-bracket solvent payer and the user.
pub fn black_bday_low_post_ok(post: &Snapshot, from: i32, to: i32, pre_from: i32, pre_to: i32) -> bool {
    post.money[from as usize] == pre_from - 800 && post.money[to as usize] == pre_to + 800
}

/// Post money for one high-bracket solvent payer and the user (total 400).
pub fn black_bday_high_post_ok(post: &Snapshot, from: i32, to: i32, pre_from: i32, pre_to: i32) -> bool {
    post.money[from as usize] == pre_from - 400 && post.money[to as usize] == pre_to + 400
}

/// MUTANT: low bracket pays 801 (latch `SetState` writes ignored).
pub fn black_bday_low_effects_mutant(effects: &[Effect], from: i32, to: i32) -> bool {
    let mut pays = 0;
    for e in effects {
        match e {
            Effect::PayTo { from: f, to: t, n } => {
                if *f != from || *t != to || *n != 801 {
                    return false;
                }
                pays += 1;
            }
            Effect::SetState { .. } => {}
            _ => return false,
        }
    }
    pays == 1
}

// ===========================================================================
// C-MUJICA-心の雨
// 「使你前方20格内的所有玩家各自骰1d20」
// 「骰点小于等于12的玩家获得一层[停留]，小于等于3的玩家变为获得2层[停留]。」
// 「若未能使任何玩家获得[停留]，自身获得一层[晕眩]并获得1000资金」
// 「变为」 = the gain is 2 layers, not 1+2 (OPEN-misread avoided).
// 「前方」 = the 20 tiles clockwise ahead; the user's own tile is not 「前方」.
// ===========================================================================

/// Stay layers demanded by one d20 result. `≤3` → 2 (replaces the 1 of `≤12`).
pub fn heart_rain_stay_layers(roll: i32) -> i32 {
    if roll <= 3 {
        2
    } else if roll <= 12 {
        1
    } else {
        0
    }
}

/// True when the player stands on one of the 20 tiles clockwise ahead of `me`.
/// `forward` is the clockwise tile distance (0 = same tile, not 「前方」).
pub fn heart_rain_in_window(forward: i32) -> bool {
    (1..=20).contains(&forward)
}

/// Fallback effects when no in-window player gained any [停留] layer.
pub fn heart_rain_fallback_effects(effects: &[Effect], user: i32) -> bool {
    effects.len() == 2
        && matches!(effects[0], Effect::GiveStun { player: p, n } if p == user && n == 1)
        && matches!(effects[1], Effect::GainMoney { player: p, n } if p == user && n == 1000)
}

/// Stay effects for one in-window player whose roll granted `layers > 0`.
pub fn heart_rain_stay_effect(effects: &[Effect], player: i32, layers: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GiveStay { player: p, n } if p == player && n == layers)
}

/// MUTANT: `≤3` stacks 1+2 = 3 layers.
pub fn heart_rain_stay_layers_mutant(roll: i32) -> i32 {
    if roll <= 3 {
        3
    } else if roll <= 12 {
        1
    } else {
        0
    }
}

// ===========================================================================
// C-MUJICA-骰子已经掷下
// 「将此卡放置于自身场上」
// 「回合结束后放入弃牌堆」
// ===========================================================================

/// Play body: place this card on the user's field (Dest::Field).
pub fn dice_cast_place_effects(effects: &[Effect], player: i32, card: u8) -> bool {
    // PlaceCard + the once-per-turn slot latch (SetState) + the no-counteract
    // latch (SetState) are the play's records. Narration is not in the frame.
    let mut placed = false;
    let mut slot_writes = 0;
    for e in effects {
        match e {
            Effect::PlaceCard {
                player: p,
                dest: 2,
                card: c,
            } if *p == player && *c == card => placed = true,
            Effect::SetState { .. } => slot_writes += 1,
            _ => return false,
        }
    }
    placed && slot_writes >= 1
}

/// Turn-end body: dest becomes Graveyard (discard). Under the model this is
/// `set_dest(Graveyard)` + a narration line; the host's `UnplaceCard` is the
/// discard motion and is PROTOCOL-LEVEL (see unit table).
pub fn dice_cast_turn_end_sets_graveyard(dest_after: i32) -> bool {
    dest_after == 1 // Dest::Graveyard
}

// ===========================================================================
// C-MUJICA-燃尽前的线香花火
// 「将此卡放置于场上并放置2个奇迹水晶（上限2）」
// 「你的回合结束后自动移除一个奇迹水晶并使你获得一个额外回合」
// 「最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]。」
// OPEN-花火额外回合: does the last crystal also grant the extra turn?
// OPEN-花火眩晕掉层: turn-end layer tick vs 「立刻获得2层」.
// ===========================================================================

/// Play: place on field + exactly 2 crystals (cap 2).
pub fn sparkler_place_effects(effects: &[Effect], player: i32, card: u8) -> bool {
    let mut placed = false;
    let mut crystals = 0;
    for e in effects {
        match e {
            Effect::PlaceCard {
                player: p,
                dest: 2,
                card: c,
            } if *p == player && *c == card => placed = true,
            Effect::AddCounter { n, .. } => {
                if *n != 2 {
                    return false;
                }
                crystals += 1;
            }
            _ => return false,
        }
    }
    placed && crystals == 1
}

/// Last-crystal body: the user gains **2** [眩晕] layers, immediately.
/// 「置入弃牌堆」 is expressed by the body as `set_dest(Graveyard)`; the host
/// records that as the `dest` field and does **not** push an `UnplaceCard`
/// effect (see unit table — discard motion itself is PROTOCOL-LEVEL).
pub fn sparkler_last_crystal_effects(effects: &[Effect], user: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GiveStun { player: p, n } if p == user && n == 2)
}

/// Dest flag after the last-crystal body: Graveyard (0).
/// Wire values (`card_sdk::ctx::Dest`): Graveyard=0, Hand=1, Field=2, Banished=3.
pub fn sparkler_last_crystal_dest(dest_after: i32) -> bool {
    dest_after == 0
}

/// Turn-end burn body: one crystal removed. The extra-turn grant is a
/// protocol stub in the host (`give_extra_turn` returns 0 with no Effect) —
/// see BLOCKED/PROTOCOL row for the grant itself.
pub fn sparkler_burn_effects(effects: &[Effect], _user: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::AddCounter { n, .. } if n == -1)
}

/// MUTANT: last crystal grants 1 stun, not 2.
pub fn sparkler_last_crystal_effects_mutant(effects: &[Effect], user: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GiveStun { player: p, n } if p == user && n == 1)
}

// ===========================================================================
// C-MUJICA-人偶的箱庭
// 「向你支付X*20资金，X为该玩家正常移动到你所在格子所需的移动数。」
// OPEN-触发结算 / OPEN-箱庭0步.
// 「正常移动到你所在格子」 = the clockwise walk length (the board's only forward).
// ===========================================================================

/// Pay branch amount: `X * 20`, X = clockwise tiles from the payer to the user.
pub fn doll_garden_pay_amount(x: i32) -> i32 {
    x * 20
}

/// Exactly one transfer of `X*20` from the payer to the user.
pub fn doll_garden_pay_effects(effects: &[Effect], from: i32, to: i32, x: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::PayTo { from: f, to: t, n } if f == from && t == to && n == x * 20)
}

/// MUTANT: pays X*20 + 10.
pub fn doll_garden_pay_amount_mutant(x: i32) -> i32 {
    x * 20 + 10
}

// ===========================================================================
// C-MUJICA-祥，移动
// 「强制一名玩家向你选择的方向移动3格并[触发结算]」
// OPEN-触发结算. 「移动3格」 is a fixed 3-tile walk, not a roll.
// 「触发结算时进行的支付价格减半」 — OPEN-取整 on odd prices.
// ===========================================================================

/// Forced walk length is exactly 3 tiles.
pub fn saki_move_steps() -> i32 {
    3
}

/// Destination after a 3-tile walk in `dir` (1 = clockwise, -1 = counter).
/// `tile_count` is the board size (60).
pub fn saki_move_dest(from: i32, dir: i32, tile_count: i32) -> i32 {
    let d = if dir >= 0 { 3 } else { -3 };
    let n = tile_count;
    (((from + d) % n) + n) % n
}

/// Teleport-style effect log for the forced walk (the walk itself is a
/// move-plan; under the model the body records `Teleport` / `MoveUnits`).
pub fn saki_move_walk_effects(effects: &[Effect], target: i32, dest: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::Teleport { player: p, tile } if p == target && tile == dest)
}

/// Halve a settle payment. Even prices: exact half. Odd: OPEN-取整; this
/// spec uses floor (the engine's even-rent contract uses 320 → 160).
pub fn saki_move_halve_pay(amount: i32) -> i32 {
    amount / 2
}

/// MUTANT: walk is 4 tiles.
pub fn saki_move_dest_mutant(from: i32, dir: i32, tile_count: i32) -> i32 {
    let d = if dir >= 0 { 4 } else { -4 };
    let n = tile_count;
    (((from + d) % n) + n) % n
}

// ===========================================================================
// C-MUJICA-（祥子）斩断留恋，忘却一切
// 「抵押一张你拥有且未抵押的最贵地契」
// 「并弃置一张手牌（若无手牌则弃掉下一张抽到的牌）」
// 「立刻传送至任意可购买或已拥有的格子并触发结算，作为你的主要移动。」
// OPEN-触发结算. 「最贵」 = printed purchase price.
// ===========================================================================

/// The most expensive un-mortgaged deed among `owned` (tile indices) given
/// each tile's printed price. Ties: lowest tile index wins (deterministic).
pub fn sakiko_most_expensive_tile(
    owned: &[i32],
    mortgaged: &[bool],
    prices: &[i32],
    n_tiles: i32,
) -> i32 {
    let mut best = -1;
    let mut best_price = -1;
    for &t in owned {
        if t < 0 || t >= n_tiles {
            continue;
        }
        let ti = t as usize;
        if mortgaged[ti] {
            continue;
        }
        let pr = prices[ti];
        if pr > best_price {
            best_price = pr;
            best = t;
        }
    }
    best
}

/// Mortgage effect: `SetMortgaged { tile, on: true }`.
pub fn sakiko_mortgage_effect(effects: &[Effect], tile: i32) -> bool {
    !effects.is_empty()
        && matches!(effects[0], Effect::SetMortgaged { tile: t, on: true } if t == tile)
}

/// Discard-one-hand effect (hand non-empty branch).
pub fn sakiko_discard_one_effect(effects: &[Effect], player: i32) -> bool {
    effects.iter().any(|e| {
        matches!(e, Effect::DiscardFromHand { player: p, .. } if *p == player)
    })
}

// ===========================================================================
// C-MUJICA-欢迎来到ave mujica的世界
// 「（1）转换任意一名玩家的状态（若指定了不存在状态2的玩家则无效果）」
// 「状态」 = the band skill's 状态1 / 状态2 pair (`skillState` 1 ↔ 2).
// 「任意一名玩家」 includes the user.
// 「不存在状态2的玩家则无效果」 is a required no-op.
// ===========================================================================

/// Toggle: 1 → 2, 2 → 1. A player without 状态2 (skillState never 1 or 2)
/// must be a no-op.
pub fn welcome_next_state(current: i32, has_state2: bool) -> i32 {
    if !has_state2 {
        current // no-op
    } else if current == 1 {
        2
    } else if current == 2 {
        1
    } else {
        current
    }
}

/// Toggle effect: exactly one `SetState` on the designated player's
/// `skillState` key to the toggled value.
pub fn welcome_toggle_effects(effects: &[Effect], player: i32, to: i32, skill_state_key: u16) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::SetState { player: p, key, n } if p == player && key == skill_state_key && n == to)
}

/// Required no-op when the target has no 状态2.
pub fn welcome_no_state2_is_noop(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// MUTANT: toggle always flips 1↔2 even without 状态2.
pub fn welcome_next_state_mutant(current: i32, has_state2: bool) -> i32 {
    let _ = has_state2;
    if current == 1 {
        2
    } else if current == 2 {
        1
    } else {
        2
    }
}

// ===========================================================================
// C-MUJICA-（喵梦）
// 「（1）将此卡放置于场上」
// ===========================================================================

pub fn nyamu_place_effects(effects: &[Effect], player: i32, card: u8) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::PlaceCard { player: p, dest: 2, card: c } if p == player && c == card)
}

// ===========================================================================
// C-MUJICA-（海铃）
// 「（1）将此卡放置于在此卡使用者下一名行动的玩家场上」
// ===========================================================================

/// Destination player is the next actor after the user in seat order.
pub fn umiri_next_actor(user: i32, n_players: i32) -> i32 {
    (user + 1) % n_players.max(1)
}

/// Place on the next actor's field. The body also writes the `umiri_user`
/// latch (`SetState`).
pub fn umiri_place_effects(effects: &[Effect], holder: i32, card: u8) -> bool {
    let mut placed = false;
    for e in effects {
        match e {
            Effect::PlaceCard {
                player: p,
                dest: 2,
                card: c,
            } if *p == holder && *c == card => placed = true,
            Effect::SetState { .. } => {}
            _ => return false,
        }
    }
    placed
}

// ===========================================================================
// C-MUJICA-（睦/mortis）表演的本能
// 「此卡打出时效果为场上其他玩家打出的上一张卡
//  （不计入效果带有放置于场上的卡）」
// With no prior copyable card the play is inert (empty effect log).
// ===========================================================================

pub fn mortis_inert_is_noop(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ===========================================================================
// Shared frame helpers
// ===========================================================================

/// Money frame: only `user` and `others` may move; every other seat is equal.
pub fn money_frame_ok(
    pre: &Snapshot,
    post: &Snapshot,
    user: i32,
    others: &[i32],
    n: i32,
) -> bool {
    for p in 0..n.min(MAX_PLAYERS as i32) {
        let touched = p == user || others.contains(&p);
        if !touched && pre.money[p as usize] != post.money[p as usize] {
            return false;
        }
    }
    true
}

/// Stay-frame: only `touched` seats may change stay/stun.
pub fn stay_frame_ok(pre: &Snapshot, post: &Snapshot, touched: &[i32], n: i32) -> bool {
    for p in 0..n.min(MAX_PLAYERS as i32) {
        if !touched.contains(&p) && pre.stay[p as usize] != post.stay[p as usize] {
            return false;
        }
    }
    true
}
