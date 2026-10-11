//! Unit `ppp`: reference specs FROM the clause docs only.
//!
//! Spec source of truth: `docs/rulebook/spec/cards/ppp.md` and
//! `docs/rulebook/cards.json`. Never derive a spec from engine behaviour.
//!
//! Each pure function is one clause's Trigger / Condition / Effect / Frame
//! as a predicate over (pre + effect-log + post).

use crate::intern::{self, NameId};
use crate::world::{Effect, Snapshot, World, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Shared vocabulary
// ---------------------------------------------------------------------------

/// 「[移除]」 = leave every private / public zone (not the discard).
/// Expressed by `ctx::set_dest(Dest::Banished)`.
pub const DEST_BANISHED: i32 = 3;
/// `ctx::Dest::Field`.
pub const DEST_FIELD: i32 = 2;

pub const CARD_POPIPA: &str = "PPP:Popipa";
pub const CARD_PIPOPA: &str = "PPP:[衍生]Pipopa";
pub const CARD_POPIPAPAPIPOPA: &str = "PPP:[衍生]Popipapapipopa";

pub fn interned(name: &str) -> NameId {
    intern::intern(name)
}

/// Count of `name` in the world's draw pile.
pub fn deck_count_named(w: &World, name: &str) -> i32 {
    let id = interned(name).0;
    let mut i = 0usize;
    let mut n = 0i32;
    while i < w.deck_n as usize {
        if w.deck_pile[i] == id {
            n += 1;
        }
        i += 1;
    }
    n
}

// ===========================================================================
// C-PPP-popipa-01
// 「获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]」
// Effect: (a) money +1000; (b) one PPP:[衍生]Pipopa enters the draw pile;
//         (c) exactly one card is drawn; (d) PPP:Popipa is [移除].
// Frame: user money / hand / draw / discard only.
// ===========================================================================

/// Exact effect log the clause allows, in order.
/// Deck insert is a pile mutation (no typed Effect arm); the banish is the
/// played-card destination (`w.dest`).
pub fn popipa_effects_ok(effects: &[Effect], player: i32) -> bool {
    if effects.len() != 2 {
        return false;
    }
    matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 1000)
        && matches!(effects[1], Effect::Draw { player: p, n } if p == player && n == 1)
}

/// Post-state: money +1000, one new Pipopa in the draw pile, hand grew by
/// the draw, and the played card's destination is [移除].
pub fn popipa_post_ok(
    pre_money: i32,
    pre_deck_pipopa: i32,
    pre_hand_n: i32,
    post_money: i32,
    deck_pipopa: i32,
    hand_n: i32,
    dest: i32,
) -> bool {
    post_money == pre_money + 1000
        && deck_pipopa == pre_deck_pipopa + 1
        && hand_n == pre_hand_n + 1
        && dest == DEST_BANISHED
}

/// Mutant: reference demands +1100 money. Card must FAIL to match.
pub fn popipa_effects_ok_mutant(effects: &[Effect], player: i32) -> bool {
    if effects.len() != 2 {
        return false;
    }
    matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 1100)
        && matches!(effects[1], Effect::Draw { player: p, n } if p == player && n == 1)
}

// ===========================================================================
// C-PPP-pipopa-01
// 「获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]」
// Effect: (a) money +1000; (b) one PPP:[衍生]Popipapapipopa on the user's
//         field; (c) this card [移除].
// Frame: money / field / played-card zones.
// ===========================================================================

/// `card` is the host slot used for the placed Popipapapipopa (unknown PPP
/// names map to slot 2 under the Kani intern facade).
pub fn pipopa_effects_ok(effects: &[Effect], player: i32, popipapapipopa_slot: u8) -> bool {
    if effects.len() != 2 {
        return false;
    }
    matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 1000)
        && matches!(
            effects[1],
            Effect::PlaceCard {
                player: p,
                dest: DEST_FIELD,
                card
            } if p == player && card == popipapapipopa_slot
        )
}

pub fn pipopa_post_ok(pre_money: i32, post_money: i32, dest: i32, field_has_new: bool) -> bool {
    post_money == pre_money + 1000 && dest == DEST_BANISHED && field_has_new
}

/// Mutant: reference wants a 2000 gain.
pub fn pipopa_effects_ok_mutant(effects: &[Effect], player: i32, popipapapipopa_slot: u8) -> bool {
    if effects.len() != 2 {
        return false;
    }
    matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 2000)
        && matches!(
            effects[1],
            Effect::PlaceCard {
                player: p,
                dest: DEST_FIELD,
                card
            } if p == player && card == popipapapipopa_slot
        )
}

// ===========================================================================
// C-PPP-bang-dream-01
// 「为[使用者]的团卡添加一个[奇迹水晶]」
// Effect: that instance's crystal count rises by exactly 1.
// Condition: the user has a band-skill field instance (团卡).
// ===========================================================================

/// Exact log for the crystal leg (host models band crystals as `band_crystals`
/// and/or `AddCounter` on the band-skill instance).
pub fn bang_dream_01_crystals_after(pre: i32) -> i32 {
    pre + 1
}

// ===========================================================================
// C-PPP-signpost-02
// 「回合结束时[失去]1000资金」
// Effect: the owner's money falls by exactly 1000 ([失去] = delete money).
// ===========================================================================

pub fn signpost_02_effects_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::PayMoney { player: p, n } if p == player && n == 1000
        )
}

pub fn signpost_02_post_ok(pre_money: i32, post_money: i32) -> bool {
    post_money == pre_money - 1000
}

/// Mutant: reference wants a 500 loss.
pub fn signpost_02_effects_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::PayMoney { player: p, n } if p == player && n == 500
        )
}

// ===========================================================================
// C-PPP-auction-off-01
// 「此卡加入手牌时将此卡放置在自身场上」
// Trigger: this card enters a hand.
// Effect: it is immediately placed on that player's field.
// ===========================================================================

pub fn auction_off_01_effects_ok(effects: &[Effect], player: i32, this_slot: u8) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::PlaceCard {
                player: p,
                dest: DEST_FIELD,
                card
            } if p == player && card == this_slot
        )
}

// ===========================================================================
// C-PPP-popipapapipopa-c1
// 「此卡拥有者每次[经过]…时为此卡添加1个[奇迹水晶]（上限10个）」
// Effect: this card's crystals rise by exactly 1, never past 10.
// Condition: card on owner's field; crystal count < 10.
// ===========================================================================

pub fn popipapapipopa_c1_crystals_after(pre: i32) -> i32 {
    if pre >= 10 {
        pre
    } else {
        pre + 1
    }
}

/// True iff the log matches "add 1 crystal when under the cap, else nothing".
pub fn popipapapipopa_c1_log_ok(effects: &[Effect], owner: i32, key: u16, pre_crystals: i32) -> bool {
    let after = popipapapipopa_c1_crystals_after(pre_crystals);
    if after == pre_crystals {
        // cap reached: no effect
        return effects.is_empty();
    }
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter { owner: o, key: k, n } if o == owner && k == key && n == 1
        )
}

// ===========================================================================
// C-PPP-kasumi-skill-01
// 「其他玩家“星之鼓动山丘”上[结算]时获得一个[火罐]（初始0，上限1）」
// Effect: 香澄's [火罐] rises by exactly 1, never past 1; initial 0.
// ===========================================================================

pub fn kasumi_skill_01_fire_after(pre: i32) -> i32 {
    if pre >= 1 {
        pre
    } else {
        pre + 1
    }
}

pub fn kasumi_skill_01_effects_ok(effects: &[Effect], player: i32, pre: i32) -> bool {
    let after = kasumi_skill_01_fire_after(pre);
    if after == pre {
        return effects.is_empty();
    }
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainFire { player: p, n } if p == player && n == 1
        )
}

// ===========================================================================
// C-PPP-band-01a
// 「[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸」
// Effect: 星星贴纸 +1 per such pass.
// ===========================================================================

pub const BAND_STICKER: &str = "星星贴纸";
/// Board indices of #1 / #16 / #31 / #46 (「#N格」 = index N-1).
pub const BAND_CORNERS: [i32; 4] = [0, 15, 30, 45];

pub fn is_band_corner(tile: i32) -> bool {
    let mut i = 0;
    while i < 4 {
        if BAND_CORNERS[i] == tile {
            return true;
        }
        i += 1;
    }
    false
}

pub fn band_01a_effects_ok(effects: &[Effect], player: i32, key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddToken { player: p, key: k, n } if p == player && k == key && n == 1
        )
}

// ===========================================================================
// C-PPP-band-01b
// 「随后可使用2个星星贴纸为此卡添加1个[奇迹水晶]」
// Effect: 2 stickers spent; this card gains exactly 1 [奇迹水晶].
// ===========================================================================

pub fn band_01b_effects_ok(effects: &[Effect], player: i32, tok: u16, crystal_key: u16) -> bool {
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::AddToken { player: p, key: k, n } if p == player && k == tok && n == -2
        )
        && matches!(
            effects[1],
            Effect::AddCounter { owner: p, key: k, n } if p == player && k == crystal_key && n == 1
        )
}

// ===========================================================================
// C-PPP-band-01c
// 「随后可使用此卡的2个[奇迹水晶]抽1张卡或[获得]2000资金
//  （使用此卡[奇迹水晶]的效果为1回合1次）」
// Effect: 2 crystals spent; either Draw 1 or GainMoney 2000. Once per turn.
// ===========================================================================

pub fn band_01c_draw_ok(effects: &[Effect], player: i32, crystal_key: u16) -> bool {
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::AddCounter { owner: p, key: k, n } if p == player && k == crystal_key && n == -2
        )
        && matches!(effects[1], Effect::Draw { player: p, n } if p == player && n == 1)
}

pub fn band_01c_cash_ok(effects: &[Effect], player: i32, crystal_key: u16) -> bool {
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::AddCounter { owner: p, key: k, n } if p == player && k == crystal_key && n == -2
        )
        && matches!(effects[1], Effect::GainMoney { player: p, n } if p == player && n == 2000)
}

// ---------------------------------------------------------------------------
// Tiny helpers
// ---------------------------------------------------------------------------

pub fn players_ok(n: i32) -> bool {
    (2..=MAX_PLAYERS as i32).contains(&n)
}

/// Frame: other players' money untouched.
pub fn frame_others_money(pre: &Snapshot, post: &Snapshot, me: i32) -> bool {
    let mut p = 0;
    while p < MAX_PLAYERS {
        if p as i32 != me && pre.money[p] != post.money[p] {
            return false;
        }
        p += 1;
    }
    true
}