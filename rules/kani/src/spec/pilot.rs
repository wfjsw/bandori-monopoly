//! Pilot reference specs (C-PP-aya-haruka-01/02, PPP tae_sound).
//!
//! Moved from `spec.rs` when the per-unit package landed. New specs go in
//! `spec/<unit>.rs`, not here.

//! Reference specifications written FROM the 规则书 clause docs only.
//!
//! Never derived from engine behaviour. Each function is the clause's
//! Trigger / Condition / Effect / Frame as an executable predicate on
//! pre + effect-log + post.
//!
//! Sources (verbatim quotes in `docs/rulebook/spec/cards/*.md` and
//! `docs/rulebook/cards.json`).

use crate::world::{Effect, Snapshot, MAX_CARDS};

// ===========================================================================
// C-PP-aya-haruka-02
// 「（1）如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]或[支付]时
//  将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。」
// ===========================================================================

/// `X` = 100, or 200 once the owner holds >= 10 [P✽P粉丝] (either face).
pub fn aya_haruka_02_x(fans_pos: i32, fans_neg: i32) -> i32 {
    if fans_pos + fans_neg >= 10 {
        200
    } else {
        100
    }
}

/// Owner is the poorest living player. Book: 「资金数是所有存活玩家中最少」.
/// Ties are OPEN (OPEN-ties); the engine treats "every living player has at
/// least as much" as the gate. This spec uses the same reading so the proof
/// pins that reading as the clause's executable meaning, not as engine drift.
pub fn is_poorest(money: &[i32; 4], out: &[bool; 4], n: i32, owner: i32) -> bool {
    let me = money[owner as usize];
    for p in 0..n {
        let p = p as usize;
        if p == owner as usize || out[p] {
            continue;
        }
        if money[p] < me {
            return false;
        }
    }
    true
}

/// The post `pay_amount` the clause demands, given the pre amount.
pub fn aya_haruka_02_amount(pre_amount: i32, x: i32) -> i32 {
    (pre_amount - x).max(0)
}

/// Exact effect-log the clause allows. Logs (`ctx::log`) are narration and
/// are not part of the frame: only the payment-amount rewrite is an effect.
pub fn aya_haruka_02_log_ok(effects: &[Effect], expected_to: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::SetPayAmount { to } if to == expected_to)
}

// ===========================================================================
// PPP:（多惠）寻找更好的声音
// 「传送到"江户川乐器店"并获得一个[火罐]」
// ===========================================================================

/// Spec: teleport the player to the tile named 「江户川乐器店」, then +1 [火罐].
/// Frame: exactly those two effects, in that order. Nothing else.
pub fn tae_sound_effects_ok(effects: &[Effect], player: i32, shop: i32) -> bool {
    if effects.len() != 2 {
        return false;
    }
    matches!(effects[0], Effect::Teleport { player: p, tile } if p == player && tile == shop)
        && matches!(effects[1], Effect::GainFire { player: p, n } if p == player && n == 1)
}

/// Post-state the clause demands.
pub fn tae_sound_post_ok(post: &Snapshot, player: i32, shop: i32, pre_fire: i32) -> bool {
    post.pos[player as usize] == shop
        && post.fire[player as usize] == pre_fire + 1
}

// ===========================================================================
// C-PP-aya-haruka-01
// 「[手]：将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。」
// Condition: discard holds at least one card.
// Effect: card is on the user's field; exactly one discard card is in hand.
// Frame: user's field / hand / discard only.
// ===========================================================================

/// Spec for the play body when the discard is non-empty at pre:
/// 1. place this card on the user's field (dest = Field);
/// 2. move exactly one pre-existing discard card into hand.
/// Prompt: any pick among the distinct discard cards is legal; the chosen
/// one is the one that moves.
pub fn aya_haruka_01_effects_ok(
    effects: &[Effect],
    player: i32,
    this_card: u8,
    picked: u8,
    pre_discard: &[bool; MAX_CARDS],
) -> bool {
    if effects.len() != 3 {
        return false;
    }
    // 1. PlaceCard on Field (dest 2 = Dest::Field)
    if !matches!(
        effects[0],
        Effect::PlaceCard {
            player: p,
            dest: 2,
            card
        } if p == player && card == this_card
    ) {
        return false;
    }
    // 2. TakeFromPile(Discard, picked) then 3. AddToHand(picked)
    if !matches!(
        effects[1],
        Effect::TakeFromPile {
            player: p,
            pile: 1,
            card
        } if p == player && card == picked
    ) {
        return false;
    }
    if !matches!(
        effects[2],
        Effect::AddToHand {
            player: p,
            card
        } if p == player && card == picked
    ) {
        return false;
    }
    pre_discard[picked as usize]
}

/// When the discard is empty at pre the book's second half has no legal
/// target: the first half (「将此卡放置在[使用者]的[场地]」) still applies and
/// nothing else does.
pub fn aya_haruka_01_empty_discard_ok(effects: &[Effect], player: i32, this_card: u8) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::PlaceCard {
                player: p,
                dest: 2,
                card
            } if p == player && card == this_card
        )
}

/// Post: exactly one pre-discard card is now in hand and gone from discard.
pub fn aya_haruka_01_post_ok(
    post: &Snapshot,
    _player: i32,
    picked: u8,
    pre_discard: &[bool; MAX_CARDS],
) -> bool {
    let mut moved = 0;
    for i in 0..MAX_CARDS {
        let was = pre_discard[i];
        let now_d = post.discard[i];
        let now_h = post.hand[i];
        if was && !now_d && now_h {
            moved += 1;
            if i != picked as usize {
                return false;
            }
        } else if was && i == picked as usize {
            // picked must have moved
            return false;
        } else if !was && now_h && i != 1 {
            // no extra hand cards appeared
            // (resonance in hand is a separate bit; ignore id 1)
            return false;
        }
    }
    moved == 1
}

// ===========================================================================
// MUTANTS -- deliberately wrong specs. A correct card must FAIL these.
// Used by `#[kani::proof] #[kani::should_panic]` harnesses to show the
// proofs are not vacuous.
// ===========================================================================

/// MUTANT of [`aya_haruka_02_x`]: off-by-one high (X+1).
pub fn aya_haruka_02_x_mutant(fans_pos: i32, fans_neg: i32) -> i32 {
    aya_haruka_02_x(fans_pos, fans_neg) + 1
}

/// MUTANT of [`tae_sound_effects_ok`]: demands an extra GainFire.
pub fn tae_sound_effects_ok_mutant(effects: &[Effect], player: i32, shop: i32) -> bool {
    if effects.len() != 3 {
        return false;
    }
    matches!(effects[0], Effect::Teleport { player: p, tile } if p == player && tile == shop)
        && matches!(effects[1], Effect::GainFire { player: p, n } if p == player && n == 1)
        && matches!(effects[2], Effect::GainFire { player: p, n } if p == player && n == 1)
}

/// MUTANT of [`aya_haruka_01_effects_ok`]: demands two cards move to hand.
pub fn aya_haruka_01_effects_ok_mutant(
    effects: &[Effect],
    player: i32,
    _this_card: u8,
    picked: u8,
    pre_discard: &[bool; MAX_CARDS],
) -> bool {
    if effects.len() != 4 {
        return false;
    }
    let _ = picked;
    let _ = pre_discard;
    matches!(effects[0], Effect::PlaceCard { player: p, .. } if p == player)
        && matches!(effects[1], Effect::TakeFromPile { player: p, pile: 1, .. } if p == player)
        && matches!(effects[2], Effect::AddToHand { player: p, .. } if p == player)
        && matches!(effects[3], Effect::AddToHand { player: p, .. } if p == player)
}
