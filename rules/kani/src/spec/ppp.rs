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
pub const CARD_BANG_DREAM: &str = "PPP:Bang Dream!";
pub const CARD_AUCTION_OFF: &str = "PPP:[衍生]拍卖撤下来了";
pub const CARD_STAR_BEAT: &str = "PPP:STAR BEAT!";
pub const CARD_DOOR: &str = "PPP:Tomorrow's Door";

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

// ===========================================================================
// C-PPP-bang-dream-02
// 「然后[传送]至任意[使用者]拥有的格子且可选择盖房」
// Effect: the user is [传送] to a tile they own. 「可选择盖房」 is optional.
// Frame: the user's pos (and optionally a house) plus the played-card zones.
// ===========================================================================

/// Post-pos must equal the picked owned-tile index. Any pick among the
/// player's owned tiles is legal (「任意」).
pub fn bang_dream_02_pos_is_owned(pos: i32, owned: &[i32], n_owned: usize) -> bool {
    let mut i = 0;
    while i < n_owned {
        if owned[i] == pos {
            return true;
        }
        i += 1;
    }
    false
}

/// Exactly one Teleport on this leg (the optional house is a separate
/// prompt and is not required).
pub fn bang_dream_02_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::Teleport { player: p, .. } if p == player)
}

/// Mutant: reference wants a GainMoney(1) side-effect the book does not name.
pub fn bang_dream_02_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 2
        && matches!(effects[0], Effect::Teleport { player: p, .. } if p == player)
        && matches!(effects[1], Effect::GainMoney { player: p, n } if p == player && n == 1)
}

// ===========================================================================
// C-PPP-popipapapipopa-c2
// 「消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）。」
// Effect: spending k crystals reduces the money movement by 150*k,
//         floored at 0; those k crystals are removed.
// ===========================================================================

/// Post pay_amount after spending `k` crystals on a spend of `amount`.
pub fn popipapapipopa_c2_amount_after(amount: i32, k: i32) -> i32 {
    (amount - 150 * k).max(0)
}

/// True iff the log is exactly "crystals -k then pay-amount rewrite".
/// `k == 0` is the decline branch: empty log.
pub fn popipapapipopa_c2_log_ok(
    effects: &[Effect],
    owner: i32,
    key: u16,
    k: i32,
    expected_to: i32,
) -> bool {
    if k <= 0 {
        return effects.is_empty();
    }
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::AddCounter { owner: o, key: kk, n } if o == owner && kk == key && n == -k
        )
        && matches!(effects[1], Effect::SetPayAmount { to } if to == expected_to)
}

/// Mutant: reference wants a 150-per-crystal cut but floors at 1 (not 0).
pub fn popipapapipopa_c2_amount_after_mutant(amount: i32, k: i32) -> i32 {
    (amount - 150 * k).max(1)
}

// ===========================================================================
// C-PPP-auction-off-01
// 「此卡加入手牌时将此卡放置在自身场上」
// Trigger: this card enters a hand.
// Effect: it is immediately placed on that player's field.
// ===========================================================================

/// Mutant: reference wants placement on the discard instead of the field.
pub fn auction_off_01_effects_ok_mutant(effects: &[Effect], player: i32, this_slot: u8) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::PlaceCard {
                player: p,
                dest: DEST_BANISHED,
                card
            } if p == player && card == this_slot
        )
}

// ===========================================================================
// C-PPP-auction-off-c1
// 「此卡拥有者火罐上限加1」
// Effect: max(火罐) = baseline + 1 while the card is present.
// (Host models the continuous +1 as a one-shot `add_fire_max` at placement.)
// ===========================================================================

pub fn auction_off_c1_fire_max_after(pre: i32) -> i32 {
    pre + 1
}

// ===========================================================================
// C-PPP-star-beat-01
// 「[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的格子上
//   进行一次盖房，随后减少1层”」
// Effect: the user gains 1 layer of that status.
// Engine key: `star_beat.layers` (the book does not name a key).
// ===========================================================================

pub const STAR_BEAT_LAYERS_KEY: &str = "star_beat.layers";

pub fn star_beat_01_layers_after(pre: i32) -> i32 {
    pre + 1
}

/// Mutant: reference wants +2 layers.
pub fn star_beat_01_layers_after_mutant(pre: i32) -> i32 {
    pre + 2
}

// ===========================================================================
// C-PPP-door-01
// 「此卡指定的序列（从前往后）为“流星堂”，“花咲川女子学院”，“Space”，
//   “Live House Galaxy”，“武道馆”，“RiNG 1”，“RiNG 2”，“RiNG 3”，“RiNG 4”，
//   “大阪中之岛公园”」
// Pure sequence definition. Board indices are 0-based slots of TILE_NAMES.
// ===========================================================================

/// The printed sequence, as 0-based board slots (`ir_fill::TILE_NAMES`).
/// 流星堂=45, 花咲川女子学院=9, Space=42, Live House Galaxy=50,
/// 武道馆=40, RiNG 1=8, RiNG 2=16, RiNG 3=36, RiNG 4=53,
/// 大阪中之岛公园=24.
pub const DOOR_SEQUENCE: [i32; 10] = [45, 9, 42, 50, 40, 8, 16, 36, 53, 24];

/// Ordered names, for a human-readable const check.
pub const DOOR_SEQUENCE_NAMES: [&str; 10] = [
    "流星堂",
    "花咲川女子学院",
    "Space",
    "Live House Galaxy",
    "武道馆",
    "RiNG 1",
    "RiNG 2",
    "RiNG 3",
    "RiNG 4",
    "大阪中之岛公园",
];

/// Every entry of the sequence exists on the board under the printed name.
pub fn door_sequence_names_on_board() -> bool {
    let mut i = 0;
    while i < 10 {
        let want = DOOR_SEQUENCE_NAMES[i];
        let mut found = false;
        let mut t = 0;
        while t < 60 {
            if crate::ir_fill::TILE_NAMES[t].1 == want
                && crate::ir_fill::TILE_NAMES[t].0 == DOOR_SEQUENCE[i]
            {
                found = true;
            }
            t += 1;
        }
        if !found {
            return false;
        }
        i += 1;
    }
    true
}

/// Mutant: reference wants a 9-entry sequence (drop the last stop).
pub const DOOR_SEQUENCE_MUTANT_LEN: usize = 9;

// ===========================================================================
// C-PPP-wait-aoki-01
// 「将此卡放置在[使用者]的[场地]。」
// Effect: the card is on the user's field (not in hand/draw/discard).
// OPEN-场上: 「[场地]」 is the player's field.
// ===========================================================================

pub const CARD_WAIT_AOKI: &str = "PPP:（有咲）等等等一下";

pub fn wait_aoki_01_effects_ok(effects: &[Effect], player: i32, this_slot: u8) -> bool {
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

/// Mutant: reference wants the card on the discard instead of the field.
pub fn wait_aoki_01_effects_ok_mutant(effects: &[Effect], player: i32, this_slot: u8) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::PlaceCard {
                player: p,
                dest: DEST_BANISHED,
                card
            } if p == player && card == this_slot
        )
}

// ===========================================================================
// C-PPP-random-star-01
// 「将此卡放置自身场上并将弃牌堆和手牌洗入卡组，然后将一张“拍卖撤下来了”
//   放置在卡组底端」
// (a) this card on the owner's field;
// (b) discard + hand shuffled into the draw pile (multiset union) —
//     BLOCKED-HOST: `Op::shuffle_into_deck` is a silent 0;
// (c) exactly one `PPP:[衍生]拍卖撤下来了` at the draw-pile bottom.
// ===========================================================================

pub const CARD_RANDOM_STAR: &str = "PPP:仓库里的Random Star";
pub const CARD_AUCTION_OFF_NAME: &str = "PPP:[衍生]拍卖撤下来了";

/// Place + deck-bottom insert, exactly. The sweep leg is separate
/// (BLOCKED-HOST) and is not asserted here.
pub fn random_star_01_effects_ok(
    effects: &[Effect],
    player: i32,
    this_slot: u8,
    auction_slot: u8,
) -> bool {
    // PlaceCard(this) is the only typed effect the host models on this body
    // (`add_to_deck_at` is a pile mutation with no Effect arm).
    if effects.len() != 1 {
        return false;
    }
    let _ = auction_slot;
    matches!(
        effects[0],
        Effect::PlaceCard {
            player: p,
            dest: DEST_FIELD,
            card
        } if p == player && card == this_slot
    )
}

/// Post: one new auction card at the deck bottom (last element of the
/// top-first pile).
pub fn random_star_01_deck_bottom_ok(deck_n: i32, last_card: u16, auction_id: u16) -> bool {
    deck_n >= 1 && last_card == auction_id
}

/// Mutant: reference wants two auction cards on the deck bottom.
pub fn random_star_01_deck_bottom_ok_mutant(
    deck_n: i32,
    last_two_equal: bool,
    auction_id: u16,
    last_card: u16,
) -> bool {
    deck_n >= 2 && last_two_equal && last_card == auction_id
}

// ===========================================================================
// C-PPP-star-beat-03 (sticker leg)
// 「1. 获得2个星星贴纸，本回合的主要移动改为[传送]到(45×…) mod 60格并结算」
// Stickers +2 on option 1. The teleport/settle is PROTOCOL-LEVEL (`card_move`).
// ===========================================================================

pub const TOK_STAR_STICKER: &str = "星星贴纸";

pub fn star_beat_03_stickers_ok(
    effects: &[Effect],
    player: i32,
    key: u16,
    picked_teleport: bool,
) -> bool {
    if picked_teleport {
        effects.iter().any(|e| {
            matches!(e, Effect::AddToken { player: p, key: k, n }
                if *p == player && *k == key && *n == 2)
        })
    } else {
        !effects
            .iter()
            .any(|e| matches!(e, Effect::AddToken { player: p, key: k, .. } if *p == player && *k == key))
    }
}

/// Destination formula: `(45 * (F + 1)) mod 60` where `F` counts living
/// players whose money decimal form contains the digit 5.
pub fn star_beat_03_dest(f: i32) -> i32 {
    (45 * (f + 1)).rem_euclid(60)
}

/// Number of living players whose money contains digit 5.
pub fn money_has_digit5(x: i32) -> bool {
    let mut v = x.unsigned_abs();
    if v == 0 {
        return false;
    }
    while v > 0 {
        if v % 10 == 5 {
            return true;
        }
        v /= 10;
    }
    false
}

pub fn count_f(money: &[i32; 4], out: &[bool; 4], n: i32) -> i32 {
    let mut f = 0;
    let mut p = 0;
    while p < n as usize && p < 4 {
        if !out[p] && money_has_digit5(money[p]) {
            f += 1;
        }
        p += 1;
    }
    f
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