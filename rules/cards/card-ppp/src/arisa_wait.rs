//! `PPP:（有咲）等等等一下` -- C# `CardArisaWait` (MatchHost.cs:9085-9188): the
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（有咲）等等等一下`）:
//! > （有咲）等等等一下：
//! > [手]：
//! >  将此卡放置在[使用者]的[场地]。
//! > [持续]：
//! >
//! > （1）任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。
//! >
//! > （2）事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。
//! >
//! [手] only places the card on the field; the [持续] clauses are the
//! `IEventDefer` / crystal hooks the ABI does not carry yet (TODO in source).

use card_sdk::{ctx, key, CardDef, Msg};

pub const ARISA_WAIT: CardDef = CardDef {
    id: "PPP:（有咲）等等等一下",
    play: Some(arisa_wait),
    can_react: None,
    react: None,
    why_not: None,
};

fn arisa_wait(seat: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]。」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "PPP:（有咲）等等等一下", &Msg::new(key!("arisa_wait_note")));
    ctx::log(seat, &Msg::new(key!("arisa_wait_placed")).seat("who", seat));
    // TODO(规则书)[持续]（1）: 「任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。」
    //   -- needs the Fx.Resolved persistent hook and a per-field-card crystal
    //   counter (C# `Card.Crystals` / `card_crystals`; the ABI only has
    //   `band_crystals` and seat tokens, not per-card crystals).
    // TODO(规则书)[持续]（2）: 「事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，
    //   那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。」
    //   -- needs the `IEventDefer` interface (C# `Defer` / `Resolved`), the
    //   Fx.TurnStart hook, and the H.ResolveEvent / H.DrawEvent event routines
    //   (plus `Card.Detach` to retire deferred events when this card leaves).
}