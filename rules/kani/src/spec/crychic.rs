//! Unit `crychic`: reference specs FROM the clause docs only.
//!
//! Quotes live in `docs/rulebook/spec/cards/crychic.md` and the live sheet
//! (`docs/rulebook/cards.json`). Never derive a spec from engine behaviour.
//!
//! Classification notes (see `docs/rulebook/spec/kani/crychic.md`):
//! * PROTOCOL-LEVEL / OPEN clauses carry a `// CLASS:` line and no body.
//! * Host dual-store: `give_stay`/`spend_charge` write `World::stay`/`fire`,
//!   while `state::{get,set,add}` on `state_key::{STAY,CHARGE}` write
//!   `player_keys`. Specs name the store the *body* uses; the book's word is
//!   the same.

use crate::world::Effect;

// ---------------------------------------------------------------------------
// C-CRYCHIC-soyoreturn-02 — 「（2）[手] 获得500资金」
// ---------------------------------------------------------------------------

/// Exactly one effect: the owner gains 500.
pub fn soyoreturn_02_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n: 500 } if p == player
        )
}

/// MUTANT: off-by-one amount (501) — must NOT match the body.
pub fn soyoreturn_02_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n: 501 } if p == player
        )
}

/// Money delta the clause demands.
pub fn soyoreturn_02_amount() -> i32 {
    500
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-vocalist-01 — 「一次性向其他玩家支付至少5000资金时，免除此次支付。」
// CLASS: [反击] window is PROTOCOL-LEVEL; this is the leaf body IFF.
// ---------------------------------------------------------------------------

/// Waiving the payment = rewrite the pending pay amount to 0.
pub fn vocalist_01_log_ok(effects: &[Effect]) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::SetPayAmount { to: 0 })
}

/// MUTANT: waive to 1 instead of 0 (still a rewrite, wrong target).
pub fn vocalist_01_log_ok_mutant(effects: &[Effect]) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::SetPayAmount { to: 1 })
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-grab-01 — 「（1）获得一层[停留]。」
// ---------------------------------------------------------------------------

/// Play body also arms the PASSER latch (`state::set(PASSER, -1)`).
/// `passer_key` is the interned `want_to_grab_passer` NameId.
pub fn grab_01_log_ok(effects: &[Effect], player: i32, passer_key: u16) -> bool {
    effects.len() == 2
        && matches!(effects[0], Effect::GiveStay { player: p, n: 1 } if p == player)
        && matches!(
            effects[1],
            Effect::SetState { player: p, key, n: -1 } if p == player && key == passer_key
        )
}

/// MUTANT: two stay layers instead of one.
pub fn grab_01_log_ok_mutant(effects: &[Effect], player: i32, passer_key: u16) -> bool {
    effects.len() == 2
        && matches!(effects[0], Effect::GiveStay { player: p, n: 2 } if p == player)
        && matches!(
            effects[1],
            Effect::SetState { player: p, key, n: -1 } if p == player && key == passer_key
        )
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-debut-02 — 「回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶。」
// ---------------------------------------------------------------------------

/// Only the stun is in the effect log (`add_group_pool` is unlogged in the
/// Kani host). The crystal is checked as `post.group_pool == pre+1`.
pub fn debut_02_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::GiveStun { player: p, n: 1 } if p == player)
}

/// MUTANT: stay instead of stun (wrong status).
pub fn debut_02_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::GiveStay { player: p, n: 1 } if p == player)
}

/// Band-skill crystal delta the clause demands.
pub fn debut_02_crystal_delta() -> i32 {
    1
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-ifonly-01 — 「[手] 移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金」
// ---------------------------------------------------------------------------

/// X = the crystals removed (any X >= 0; host clamps the pool at 0..=99).
pub fn ifonly_01_amount(x: i32) -> i32 {
    2000 + 500 * x
}

/// Play body: drain crystals (unlogged) + gain + place the card on field as
/// [持续]. `dest` is `Dest::Field` (2); `card_slot` of a non-pilot id is 2.
pub fn ifonly_01_log_ok(effects: &[Effect], player: i32, x: i32) -> bool {
    let expected = ifonly_01_amount(x);
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n } if p == player && n == expected
        )
        && matches!(
            effects[1],
            Effect::PlaceCard { player: p, dest: 2, .. } if p == player
        )
}

/// MUTANT: 2000+500*(X+1) — off-by-one on the crystal count.
pub fn ifonly_01_log_ok_mutant(effects: &[Effect], player: i32, x: i32) -> bool {
    let expected = ifonly_01_amount(x + 1);
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n } if p == player && n == expected
        )
        && matches!(
            effects[1],
            Effect::PlaceCard { player: p, dest: 2, .. } if p == player
        )
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-ifonly-02 — 「[持续] 若你的手牌大于等于7，此卡立即置入弃牌堆。」
// ---------------------------------------------------------------------------

/// The card must leave the field into the discard. Observed as an
/// `UnplaceCard` effect (explicit `unplace`) or a field-count drop (the
/// `set_dest(Graveyard)` commit — world state only, not an in-run effect).
pub fn ifonly_02_log_ok(effects: &[Effect], uid: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::UnplaceCard { uid: u } if u == uid)
}

/// MUTANT: nothing happens (empty log) — must NOT match a correct move.
pub fn ifonly_02_log_ok_mutant(effects: &[Effect], _uid: i32) -> bool {
    effects.is_empty()
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-debut-01 — 「打出此卡后，本回合内你的资金不会下降…」
// CLASS: the standing no-drop claim is engine scheduling (set_no_money_loss
// is a protocol stub). The play body only arms it + schedules at_turn_end.
// ---------------------------------------------------------------------------

/// Play body: narration only + two scheduling ops (both protocol stubs that
/// do not push effects). Zero effects is the leaf's exact log.
pub fn debut_01_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-soyoreturn-01 — [特] draw-play, two branches.
// CLASS: draw-triggered [特] is a multi-step arrange; the branch bodies are
// covered as leafs below where the text determines them.
// ---------------------------------------------------------------------------

/// Branch 2 leaf: 「获得1000资金并将此卡加入手牌」 — the card is already in
/// hand from the draw, so the body only gains 1000 (see source comment).
pub fn soyoreturn_01_branch2_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n: 1000 } if p == player
        )
}

// ---------------------------------------------------------------------------
// CLASS markers for clauses this unit does not emit harnesses for
// ---------------------------------------------------------------------------
//
// C-CRYCHIC-destiny-01/02     OPEN-收款 + cross-turn window          → OPEN / PROTOCOL-LEVEL
// C-CRYCHIC-elegant-01        deferred draw at next turn end         → PROTOCOL-LEVEL
// C-CRYCHIC-karaoke-01        dice-loop + walk-as-main-move          → PROTOCOL-LEVEL
// C-CRYCHIC-human-01/02       X write-down + roll latch + turn start → mixed; see proofs
// C-CRYCHIC-grab-02/03        OPEN-触发结算                          → OPEN
// C-CRYCHIC-haruhikage-01     OPEN-春日影抽6                         → OPEN
// C-CRYCHIC-haruhikage-02     [反击] borrows a Crychic skill         → PROTOCOL-LEVEL
// C-CRYCHIC-myown-01/02       OPEN-触发结算 + abnormal-move set      → OPEN
// C-CRYCHIC-tomoricry-01      OPEN-两个X差值                         → OPEN
// C-CRYCHIC-tomoricry-02/03   cross-band transform / MyGO swap       → PROTOCOL-LEVEL
// C-CRYCHIC-mutsumifeel-01    choice prompt (branch bodies are 02/03) → covered by branches
// C-CRYCHIC-mutsumifeel-02    mid-play band-(2) invoke               → PROTOCOL-LEVEL
// C-CRYCHIC-mutsumifeel-03    shuffle + move-from-CiRCLE             → PROTOCOL-LEVEL
// C-CRYCHIC-sakilead-01/02/03 multi-player walk + settle order       → PROTOCOL-LEVEL
// C-CRYCHIC-rikireroll-01     re-roll loop + turn-roll history       → PROTOCOL-LEVEL