//! Unit `ras`: reference specs FROM the clause docs only.
//!
//! Sources: `docs/rulebook/spec/cards/ras.md` + `docs/rulebook/cards.json`.
//! Never derived from engine behaviour. Each function is one clause's
//! Trigger / Condition / Effect / Frame as a predicate on pre + log + post.
//!
//! OPEN items are named in the module docs of each clause group and recorded
//! in `docs/rulebook/spec/kani/ras.md`. Prove only what the text determines.

use crate::world::{Effect, Snapshot, MAX_PLAYERS};

// ===========================================================================
// C-RAS-unstoppable-card-03
// 「投掷1d6mod6，根据结果1-6分别传送至…。本次传送不触发结算，视为你的主要移动。
//  且若骰点为1-3获得2000资金，若为4-6则获得1000资金。」
//
// Clause 03 only: the money half. The teleport half (01/02) needs
// `Op::tile_named` for the six destination names and the move-plan
// (`plan::set_teleport_to` / `set_resolve` / `card_move`) — see the unit
// table (BLOCKED-HOST / PROTOCOL-LEVEL).
// OPEN-unstoppable-mod: face 6 is `6 mod 6 = 0` but the body says 「结果1-6」;
// this spec uses the printed 1-6 branches.
// ===========================================================================

/// Money the clause demands for a d6 face `r` (1..=6). Faces 1-3 → 2000;
/// faces 4-6 → 1000.
pub fn unstoppable_card_03_money(r: i32) -> i32 {
    if r <= 3 {
        2000
    } else {
        1000
    }
}

/// Mutant: off-by-one on the high band (1100 instead of 1000).
pub fn unstoppable_card_03_money_mutant(r: i32) -> i32 {
    if r <= 3 {
        2000
    } else {
        1100
    }
}

/// Exact effect log for the money half, when the player is still in.
pub fn unstoppable_card_03_log_ok(effects: &[Effect], player: i32, expected: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n } if p == player && n == expected
        )
}

/// Out of the game: the clause's money half is a no-op (the body guards
/// `!player_out`). The teleport half is BLOCKED-HOST and contributes nothing.
pub fn unstoppable_card_03_out_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ===========================================================================
// C-RAS-riot-02 / C-RAS-riot-03
// 「[反击] 当你被其他人的卡效果影响时打出此卡，所有玩家将所有手牌放至弃牌堆，
//  并抽等量的卡，你额外抽1张卡。」
//
// The [反击] *window* is PROTOCOL-LEVEL (engine chain). The **body** is a
// pure hand-cycle and is proved here by invoking `On::Counteract` directly.
// The counter card's own spend-to-discard is engine-side (counteract cost),
// not part of this body.
// ===========================================================================

/// One player's contribution to the cycle: hand size `h` is discarded, then
/// `h` cards are drawn; the holder draws `h + 1`.
pub fn riot_draw_count(hand_len: i32, is_holder: bool) -> i32 {
    hand_len + if is_holder { 1 } else { 0 }
}

/// Effect log of the cycle, in body order: every living player's hand is
/// discarded (player order, hand order), then every living player draws
/// (player order). `hands[p]` is the pre hand of player `p` as a list of
/// card slots in `cards_in` order; `out[p]` marks players already out.
pub fn riot_cycle_log_ok(
    effects: &[Effect],
    hands: &[[u8; 4]; MAX_PLAYERS],
    hand_ns: &[usize; MAX_PLAYERS],
    out: &[bool; MAX_PLAYERS],
    n_players: i32,
    holder: i32,
) -> bool {
    let mut i = 0usize;
    // 1. all discards, player order
    for p in 0..n_players {
        let p = p as usize;
        if out[p] {
            continue;
        }
        for k in 0..hand_ns[p] {
            match effects.get(i) {
                Some(Effect::DiscardFromHand { player, card }) => {
                    if *player != p as i32 || *card != hands[p][k] {
                        return false;
                    }
                }
                _ => return false,
            }
            i += 1;
        }
    }
    // 2. all draws, player order
    for p in 0..n_players {
        let p = p as usize;
        if out[p] {
            continue;
        }
        let n = riot_draw_count(hand_ns[p] as i32, p as i32 == holder);
        if n <= 0 {
            continue;
        }
        match effects.get(i) {
            Some(Effect::Draw { player, n: dn }) => {
                if *player != p as i32 || *dn != n {
                    return false;
                }
            }
            _ => return false,
        }
        i += 1;
    }
    i == effects.len()
}

/// Frame: money / positions / fire untouched by the cycle itself.
pub fn riot_cycle_frame_ok(pre: &Snapshot, post: &Snapshot) -> bool {
    pre.money == post.money && pre.pos == post.pos && pre.fire == post.fire
}

/// Mutant: the holder draws one *fewer* (no 「你额外抽1张卡」).
pub fn riot_draw_count_mutant(hand_len: i32, is_holder: bool) -> i32 {
    let _ = is_holder;
    hand_len
}

// ===========================================================================
// C-RAS-pareo-01
// 「获得2个PAREO标记，然后视为你的房屋总数增加且可选择移除任意你拥有的格子上的一层房屋」
//
// Clause 01: the mark half. Isolation for the pure-mark proof: the optional
// house-removal offer is declined / has no eligible tile, and the holder is
// not 鳰原令王那 (so 「视为你的房屋总数增加」 has no listener).
// The printed cap 「上限3」 lives on the 鳰原令王那 skill text and on
// `ctx::add_tok(..., max=3)`; the body *requests* +2 and names max 3.
// OPEN: the card text itself does not print a cap.
// ===========================================================================

/// Marks gained by the card's request: always 2, subject to the host's max
/// clamp (max=3 named by the body). Sub-cap region (`pre <= 1`) the grant is
/// exactly 2.
pub fn pareo_01_mark_delta() -> i32 {
    2
}

/// Exact log for the mark half alone: one `AddToken` of +2 PAREO.
pub fn pareo_01_log_ok(effects: &[Effect], player: i32, pareo_key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddToken { player: p, key, n } if p == player && key == pareo_key && n == 2
        )
}

/// Frame for the mark half: nothing but the mark moved.
pub fn pareo_01_frame_ok(pre: &Snapshot, post: &Snapshot) -> bool {
    pre.money == post.money
        && pre.pos == post.pos
        && pre.fire == post.fire
        && pre.hand == post.hand
        && pre.discard == post.discard
}

/// Mutant: +3 marks instead of +2.
pub fn pareo_01_log_ok_mutant(effects: &[Effect], player: i32, pareo_key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddToken { player: p, key, n } if p == player && key == pareo_key && n == 3
        )
}