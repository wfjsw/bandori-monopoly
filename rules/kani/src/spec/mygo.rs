//! Unit `mygo`: reference specs FROM the clause docs only.
//!
//! Source of truth: `docs/rulebook/spec/cards/mygo.md` and the verbatim
//! `docs/rulebook/cards.json` text. Never derived from engine behaviour.
//!
//! Clause ids are `C-MYGO-<slug>-<n>`. Each pure function is one clause's
//! Trigger / Condition / Effect / Frame as a predicate over
//! `(pre, effect-log, post)`.

use crate::world::{Effect, Snapshot, MAX_PLAYERS};

// ===========================================================================
// C-MYGO-rinpuu-01
// 「使自己获得一层[停留]」
// Card: `MyGO:轮符雨` On::Play.
// Trigger: a play of this card. Condition: none.
// Effect: the user gains exactly one layer of [停留].
// Frame: only `stay:P{user}` moves (the effect log is exactly one GiveStay).
// ===========================================================================

/// Exact effect log of the play body: one `GiveStay { player, n: 1 }`.
/// `ctx::before_turn_end` is a scheduling op (protocol stub) and logs nothing;
/// `ctx::log` is narration, not in the frame.
pub fn rinpuu_01_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GiveStay {
                player: p,
                n
            } if p == player && n == 1
        )
}

/// Post: user's stay rose by exactly 1; every other snapshot field unchanged.
pub fn rinpuu_01_post_ok(pre: &Snapshot, post: &Snapshot, player: i32) -> bool {
    let p = player as usize;
    post.stay[p] == pre.stay[p].saturating_add(1)
        && crate::world::arr4_eq(post.money, pre.money)
        && crate::world::arr4_eq(post.pos, pre.pos)
        && crate::world::arr4_eq(post.charge, pre.charge)
        && crate::world::arr8b_eq(post.hand, pre.hand)
        && crate::world::arr8b_eq(post.discard, pre.discard)
        && post.pay_amount == pre.pay_amount
        && (0..MAX_PLAYERS).all(|i| i == p || post.stay[i] == pre.stay[i])
}

/// MUTANT: demands two stay layers instead of one. A correct card must FAIL
/// this spec (`*_mutant_is_detected` shows the disagreement is reachable).
pub fn rinpuu_01_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GiveStay {
                player: p,
                n
            } if p == player && n == 2
        )
}

// ===========================================================================
// C-MYGO-journey-01
// 「[手] 将此卡放置于当前格子上并为其放置4个奇迹水晶」
// Card: `MyGO:哪怕这旅程没有终点` On::Play.
// Trigger: the [手] play. Condition: none beyond a legal play from hand.
// Effect: the card is placed on the current tile with exactly 4 [奇迹水晶].
// Frame: only the user's hand/field zones move (PlaceCard + AddCounter). Host
// note: crystals live on the instance (`field[last_uid]` / `self_crystals`),
// and `set_crystals` logs one `AddCounter`.
// ===========================================================================

/// Dest::Field discriminant (`card_sdk::ctx::Dest::Field`). `place_card_at`
/// logs this as `PlaceCard { dest }`; `place_card_on` instead logs the landing
/// **tile index** (see `journey_01_log_ok`).
pub const DEST_FIELD: i32 = 2;

/// Exact effect log of the play body: `PlaceCard` on the current tile then
/// `AddCounter(4)`. Host note: `place_card_on` logs `PlaceCard { dest }` with
/// `dest` = the landing tile index (not `Dest::Field`); `set_crystals(4)` is
/// `set_counter_self`, which logs one `AddCounter { n: 4 }` on the instance.
/// Narration is not the frame.
pub fn journey_01_log_ok(effects: &[Effect], player: i32, card: u8, tile: i32) -> bool {
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::PlaceCard {
                player: p,
                dest,
                card: c
            } if p == player && dest == tile && c == card
        )
        && matches!(
            effects[1],
            Effect::AddCounter {
                owner: p,
                n,
                ..
            } if p == player && n == 4
        )
}

/// MUTANT: demands two PlaceCard effects.
pub fn journey_01_log_ok_mutant(effects: &[Effect], player: i32, card: u8, tile: i32) -> bool {
    let _ = tile;
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::PlaceCard {
                player: p,
                ..
            } if p == player
        )
        && matches!(
            effects[1],
            Effect::PlaceCard {
                player: p,
                ..
            } if p == player
        )
}

// ===========================================================================
// C-MYGO-haneoka-02/03/04/05
// 「若出目至少为10则在当前格子免费加盖一层房屋，若出目至少为15，则额外抽一张卡，
//  至少为20，则将此卡放置在自己场上…若小于10，此卡放入弃牌堆且视为此卡未生效」
// Card: `MyGO:羽丘的不可思议女孩` On::Play (gated on own tile).
// Thresholds inclusive (sheet 2026-10-06). `add_house` is free (no money).
// The `<10` branch's 「放入弃牌堆」 is the play pipeline's default dest, not a
// body effect — the body itself logs nothing on that branch.
// ===========================================================================

/// Effects demanded for a face `r` on an owned, unmortgaged tile.
/// r < 10: empty log (未生效; discard is the play pipeline).
/// r >= 10: SetHouses (free +1). r >= 15: also Draw 1. r >= 20: also PlaceCard.
pub fn haneoka_effects_ok(effects: &[Effect], player: i32, tile: i32, r: i32, card: u8) -> bool {
    if r < 10 {
        return effects.is_empty();
    }
    let mut i = 0usize;
    // house
    if i >= effects.len() {
        return false;
    }
    if !matches!(effects[i], Effect::SetHouses { tile: t, n } if t == tile && n >= 1) {
        return false;
    }
    i += 1;
    if r >= 15 {
        if i >= effects.len() {
            return false;
        }
        if !matches!(effects[i], Effect::Draw { player: p, n } if p == player && n == 1) {
            return false;
        }
        i += 1;
    }
    if r >= 20 {
        if i >= effects.len() {
            return false;
        }
        if !matches!(effects[i], Effect::PlaceCard { player: p, dest, card: c }
            if p == player && dest == DEST_FIELD && c == card)
        {
            return false;
        }
        i += 1;
    }
    i == effects.len()
}

/// MUTANT: for r >= 10 demands an extra Draw even on the house-only band.
pub fn haneoka_effects_ok_mutant(
    effects: &[Effect],
    player: i32,
    tile: i32,
    r: i32,
    card: u8,
) -> bool {
    if r < 10 {
        // mutant: ineffective branch should have an effect (wrong)
        return effects.len() == 1;
    }
    haneoka_effects_ok(effects, player, tile, r, card) && {
        // mutant wants an extra Draw always
        effects
            .iter()
            .any(|e| matches!(e, Effect::Draw { player: p, .. } if *p == player))
            || r < 15
    }
}

// ===========================================================================
// C-MYGO-rainday-01
// 「将此卡放置于自己场上并为其放置5个奇迹水晶」
// Card: `MyGO:那天的雨` On::Play (the play body also runs the -02 rain; that
// leg is a separate clause and is asserted by `rainday_02` below).
// ===========================================================================

/// The placement leg: exactly one PlaceCard(Field). Crystals are a counter
/// write (`set_crystals(5)`) with no Effect; checked via the key map.
pub fn rainday_01_place_ok(effects: &[Effect], player: i32, card: u8) -> bool {
    effects.first().is_some_and(|e| {
        matches!(
            *e,
            Effect::PlaceCard {
                player: p,
                dest,
                card: c
            } if p == player && dest == DEST_FIELD && c == card
        )
    })
}

/// MUTANT: demands 4 crystals instead of 5 (checked by the proof via key map).
pub fn rainday_01_crystals_mutant() -> i32 {
    4
}

// ===========================================================================
// C-MYGO-rainday-02 (play leg)
// 「打出此卡时及你的每回合开始时投掷1d10并按地产商格子顺序使（除"东京外"的）
//  第n个地产商对应的颜色格子及这些格子相邻格子上的所有玩家获得一层[停留]」
// After the placement leg, the body rolls 1d10 and grants +1 stay to every
// living player standing on a tile of the n-th agent's colour **or adjacent**.
// The agent table (excluding 东京外) is the book's board order; the body's
// AGENTS list is the implementation's reading. Spec: every living player whose
// `pos` is in the rolled zone gets exactly one GiveStay; nobody else does.
// OPEN-rainday-d10: face > 10 is unreachable on a plain 1d10 (dead text).
// ===========================================================================

/// All GiveStay effects after the PlaceCard must be n==1 and cover exactly the
/// players in `zone` (the rolled colour+adjacent set).
pub fn rainday_02_stay_ok(effects: &[Effect], after: usize, zone: &[bool]) -> bool {
    let mut seen = 0;
    for e in &effects[after..] {
        match e {
            Effect::GiveStay { player: p, n } => {
                let p = *p as usize;
                if *n != 1 || p >= zone.len() || !zone[p] {
                    return false;
                }
                seen += 1;
            }
            _ => return false,
        }
    }
    // every zone player must appear exactly once
    let expected = zone.iter().filter(|&&b| b).count();
    seen == expected
}

/// MUTANT: demands a stay for a player outside the zone.
pub fn rainday_02_stay_ok_mutant(effects: &[Effect], after: usize, zone: &[bool]) -> bool {
    let _ = zone;
    effects[after..]
        .iter()
        .any(|e| matches!(e, Effect::GiveStay { .. }))
        && {
            // wrong: require a stay even when the zone is empty
            true
        }
}

// ===========================================================================
// C-MYGO-hitoshizuku-01/02/03
// 「清除场上所有[停留]与[眩晕]效果，所有玩家因本效果每清除一种效果则支付此卡使用者
//  1000资金，若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金。」
// Ruling 2026-10-06 (TEST-FINDINGS §6): 「每清除一种效果」 is per effect TYPE
// per AFFECTED PLAYER (stay=1 + stun=1 → 2000 for one player with both).
// Spec money: each affected player P pays `1000 * kinds(P)` to the user;
// the user additionally gains `1000 * kinds(user)` (clause -03).
//
// The body (`card-mygo/src/hitoshizuku.rs`) implements exactly that: `cleared`
// is a per-player `(j, n)` list and `transfer(j, user, 1000 * n)` / the
// self-gain uses that player's own `n`. The older "body charges 1000 * GLOBAL
// kinds" note is **stale** (pre-ruling snapshot); the book and the body agree.
// `hitoshizuku_body_global_pay_ok` is kept as the *mutant* reading.
// ===========================================================================

/// kinds(P) = number of effect types cleared on P (0..=2).
pub fn hitoshizuku_pay_amount(kinds: i32) -> i32 {
    1000 * kinds
}

/// Spec: after the clear leg, every living player `j != user` with `kinds(j)>0`
/// has a PayTo { from: j, to: user, n: 1000*kinds(j) }; the user's own extra
/// gain is `GainMoney { player: user, n: 1000*kinds(user) }` when kinds(user)>0.
/// The clear leg itself is a sequence of GiveStay / GiveStun / SetState.
/// Bounded while (slice iterators are an SSA cliff).
pub fn hitoshizuku_money_ok(effects: &[Effect], user: i32, kinds: &[i32], n_players: i32) -> bool {
    let mut pays = 0;
    let mut gains = 0;
    let mut i = 0usize;
    let len = effects.len().min(24);
    while i < len {
        match &effects[i] {
            Effect::PayTo { from, to, n } => {
                if *to != user {
                    return false;
                }
                let j = *from as usize;
                if j >= kinds.len() {
                    return false;
                }
                if *n != hitoshizuku_pay_amount(kinds[j]) {
                    return false;
                }
                pays += 1;
            }
            Effect::GainMoney { player: p, n } => {
                if *p != user {
                    return false;
                }
                if *n != 1000 * kinds[user as usize] {
                    return false;
                }
                gains += 1;
            }
            _ => {}
        }
        i += 1;
    }
    let mut expected_pays = 0;
    let mut j = 0usize;
    let np = n_players.max(0) as usize;
    while j < np && j < kinds.len() {
        if j != user as usize && kinds[j] > 0 {
            expected_pays += 1;
        }
        j += 1;
    }
    let expected_gains = if kinds[user as usize] > 0 { 1 } else { 0 };
    pays == expected_pays && gains == expected_gains
}

/// MUTANT / stale reading: every living player pays `1000 * global_kinds`.
/// Kept so `hitoshizuku_global_pay_mutant_is_detected` can pin that the body
/// does **not** take this arm when per-player kinds differ.
pub fn hitoshizuku_body_global_pay_ok(
    effects: &[Effect],
    user: i32,
    global_kinds: i32,
    n_players: i32,
) -> bool {
    let each = 1000 * global_kinds;
    let mut pays = 0;
    let mut i = 0usize;
    let len = effects.len().min(24);
    while i < len {
        if let Effect::PayTo { to, n, .. } = &effects[i] {
            if *to != user || *n != each {
                return false;
            }
            pays += 1;
        }
        i += 1;
    }
    let mut expected = 0;
    let mut j = 0usize;
    let np = n_players.max(0) as usize;
    while j < np {
        if j != user as usize {
            expected += 1;
        }
        j += 1;
    }
    pays == expected
}

// ===========================================================================
// C-MYGO-tomori-card-01/02
// 「（1）获得两层仅能被自然流失效果移除的[停留]，将此卡放置于场上并将所有手卡
//  置入弃牌堆，在此卡上放置X+1个[奇迹水晶]，X为你弃置的手牌数」
// Card: `MyGO:（灯）不再迷茫` On::Play.
// Isolation: empty hand at pre (X=0 ⇒ crystals=1) so the `cards_in` /
// `discard_from_hand` String loop is a no-op (the prompt pilot's memcmp cliff).
// The general X>0 case is the same shape plus X DiscardFromHand effects.
// ===========================================================================

/// Exact effect log when the hand is empty: GiveStay(user, 2), PlaceCard(Field),
/// then AddCounter(1). `inc_slot("lockedStay", 2)` is a slot write (no Effect);
/// `set_crystals(1)` is `set_counter_self` and logs the AddCounter.
pub fn tomori_card_02_empty_hand_log_ok(effects: &[Effect], player: i32, card: u8) -> bool {
    effects.len() == 3
        && matches!(effects[0], Effect::GiveStay { player: p, n } if p == player && n == 2)
        && matches!(
            effects[1],
            Effect::PlaceCard {
                player: p,
                dest,
                card: c
            } if p == player && dest == DEST_FIELD && c == card
        )
        && matches!(
            effects[2],
            Effect::AddCounter {
                owner: p,
                n,
                ..
            } if p == player && n == 1
        )
}

/// MUTANT: demands 1 stay layer instead of 2.
pub fn tomori_card_02_empty_hand_log_ok_mutant(effects: &[Effect], player: i32, card: u8) -> bool {
    effects.len() == 3
        && matches!(effects[0], Effect::GiveStay { player: p, n } if p == player && n == 1)
        && matches!(
            effects[1],
            Effect::PlaceCard {
                player: p,
                dest,
                card: c
            } if p == player && dest == DEST_FIELD && c == card
        )
        && matches!(
            effects[2],
            Effect::AddCounter {
                owner: p,
                ..
            } if p == player
        )
}

/// Crystals demanded when X cards were discarded: exactly X + 1.
pub fn tomori_card_02_crystals(x: i32) -> i32 {
    x + 1
}
