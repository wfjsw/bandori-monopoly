//! `Sumimi:#L12` -- C# `CardL12` (MatchHost.cs:11621-11682): pull a Sumimi card
//!
//! 规则书（data/cards.json, id `Sumimi:#L12`）:
//! > [手] 从弃牌堆或抽牌堆选择一张“Sumimi”卡加入手牌并将此卡置于自身场上； [持续] （1）[拥有者]手卡上限数量减1。（2）每当你消耗火罐时，为此卡添加一个[奇迹水晶] （3）当此卡上拥有6个[奇迹水晶]时，将此卡返回手牌。
//!
//! from discard/draw, place this card, hand limit -1, crystals on fire spent.
//! Not in docs/rulebook/cards.json -- translated from the C# class and the card text.

use alloc::vec::Vec;

use alloc::string::String;
use card_sdk::abi::{prop, state_key};
use card_sdk::ctx::{self, trigger, CardPile};
use card_sdk::{key, CardDef, Msg, On};

pub const L12: CardDef = CardDef::new(
    "Sumimi:#L12",
    &[
        On::Play(None, l12, ""),
        On::Hook(&[card_sdk::abi::HookKind::FireSpent], Some(fire_spent_guard), fire_spent, ""),
    ],
)
// 规则书[持续]（1）: 「[拥有者]手卡上限数量减1。」 -- the `handLimitDelta`
// property (C# `Card.HandLimitDelta`), continuous while this instance sits on
// the field.
.props(&[(prop::HAND_LIMIT_DELTA, -1)]);

const ID: &str = "Sumimi:#L12";

/// C# `CardL12.Pool` -- distinct ids in the discard then the draw pile that
/// belong to band `Sumimi`, minus this card. The data key prefix is the band
/// (`H.Db.Card(id)?.band == "Sumimi"`).
fn pool(player_id: i32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for pile in [CardPile::Discard, CardPile::Deck] {
        for id in ctx::cards_in(player_id, pile) {
            if id != ID && id.starts_with("Sumimi:") && !out.iter().any(|x| *x == id) {
                out.push(id);
            }
        }
    }
    out
}

fn l12(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡置于自身场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("l12_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("l12_placed")).player_id("who", player_id),
    );
    // 规则书[手]: 「从弃牌堆或抽牌堆选择一张“Sumimi”卡加入手牌」 -- C# `CardL12.Pool`
    // lists `hidden.discard.Concat(hidden.draw)` ids with `band == "Sumimi"` (minus
    // this card) and `H.AskCard` picks one to `H.AddToHand`.
    let pool = pool(player_id);
    if pool.is_empty() {
        return Ok(());
    }
    let picks: Vec<&str> = pool.iter().map(|s| s.as_str()).collect();
    let i = ctx::ask_card(
        player_id,
        &Msg::new(key!("l12_title")),
        &Msg::new(key!("l12_search")),
        &picks,
    )?;
    let id = pool[i].clone();
    // C# `hidden.discard.Remove(text)`, else `hidden.draw.Remove(text)` +
    // `H.Shuffle(hidden.draw)`. `take_card` pulls one copy out of the pile
    // without moving it; `add_to_hand` then files it. (The reshuffle of the
    // remaining draw pile is cosmetic and not expressible here.)
    if !ctx::take_card(player_id, CardPile::Discard, &id) {
        ctx::take_card(player_id, CardPile::Deck, &id);
    }
    ctx::add_to_hand(player_id, &id);
    ctx::log(
        player_id,
        &Msg::new(key!("l12_taken"))
            .player_id("who", player_id)
            .card("card", &id),
    );
    // 规则书[持续]（1）: 「[拥有者]手卡上限数量减1。」 -- the continuous delta
    // declared on the `CardDef` above (`props(&[(prop::HAND_LIMIT_DELTA, -1)])`), stamped at
    // placement, gone with the card. No state write: that would double-count.
    // C# `NoteText` shows the crystal count / hand-limit note; CardDef has no
    // NoteText hook.
    Ok(())
}

/// 规则书[持续]（2）: 「每当你消耗火罐时，为此卡添加一个[奇迹水晶]」 -- C#
/// `CardL12.FireSpent`.
/// Pure guard for [`fire_spent`] -- the activation gate. `false`
/// means the card is not activated at all.
fn fire_spent_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id
}

fn fire_spent(player_id: i32) -> card_sdk::Asked {
    // One crystal per fire spent (`t.value` is how many pots went).
    let n = trigger::value().max(0);
    if n == 0 {
        return Ok(());
    }
    ctx::add_crystals(n, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("l12_fire_spent"))
            .player_id("who", player_id)
            .i("n", n as i64),
    );
    // 规则书[持续]（3）: 「当此卡上拥有6个[奇迹水晶]时，将此卡返回手牌。」
    if ctx::crystals() >= 6 {
        ctx::set_dest(ctx::Dest::Hand);
        ctx::log(
            player_id,
            &Msg::new(key!("l12_back")).player_id("who", player_id),
        );
    }
    Ok(())
}
