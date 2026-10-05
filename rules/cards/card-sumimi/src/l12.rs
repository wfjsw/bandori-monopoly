//! `Sumimi:#L12` -- C# `CardL12` (MatchHost.cs:11621-11682): pull a Sumimi card
//!
//! 规则书（data/cards.json, id `Sumimi:#L12`）:
//! > [手] 从弃牌堆或抽牌堆选择一张“Sumimi”卡加入手牌并将此卡置于自身场上； [持续] （1）[拥有者]手卡上限数量减1。（2）每当你消耗火罐时，为此卡添加一个[奇迹水晶] （3）当此卡上拥有6个[奇迹水晶]时，将此卡返回手牌。
//!
//! from discard/draw, place this card, hand limit -1, crystals on fire spent.
//! Not in docs/rulebook/cards.json -- translated from the C# class and the card text.

use card_sdk::{ctx, key, CardDef, Msg};

pub const L12: CardDef = CardDef {
    id: "Sumimi:#L12",
    play: Some(l12),
    can_react: None,
    react: None,
    why_not: None,
};

fn l12(seat: i32) {
    // 规则书[手]: 「从弃牌堆或抽牌堆选择一张“Sumimi”卡加入手牌」 -- C# `CardL12.Pool`
    // lists `hidden.discard.Concat(hidden.draw)` ids with `band == "Sumimi"` (minus
    // this card) and `H.AskCard` picks one to `H.AddToHand`.
    // TODO(规则书)[手]: needs a way to **enumerate** the discard / draw piles and
    // pull a specific id out of them into hand. `hand_count` / `discard_count` /
    // `deck_count` / `discard_size` are on the ABI now (per-id counts, pile sizes),
    // but there is still no id list and no remove-from-pile op -- `add_to_hand`
    // only inserts a fresh copy. `ask_card` cannot offer ids the card cannot
    // enumerate.
    ctx::log(seat, &Msg::new(key!("l12_search")).seat("who", seat));
    // 规则书[手]: 「将此卡置于自身场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "Sumimi:#L12", &Msg::new(key!("l12_note")));
    ctx::log(seat, &Msg::new(key!("l12_placed")).seat("who", seat));
    // TODO(规则书)[持续]（1）: 「[拥有者]手卡上限数量减1。」 -- needs the
    // Fx.HandLimitDelta hook (C# `CardL12.HandLimitDelta` returns -1 for the owner).
    // TODO(规则书)[持续]（2）: 「每当你消耗火罐时，为此卡添加一个[奇迹水晶]」 -- needs
    // the Fx.FireSpent hook (C# `CardL12.FireSpent`) and a per-field-card crystal
    // counter (C# `Card.Crystals`; the ABI only has `band_crystals` and seat tokens).
    // TODO(规则书)[持续]（3）: 「当此卡上拥有6个[奇迹水晶]时，将此卡返回手牌。」 -- same
    // counter, then `H.Unplace(this, "hand", ...)` (C# `CardL12.FireSpent`).
    // C# `NoteText` shows the crystal count / hand-limit note; CardDef has no
    // NoteText hook.
}