//! `PPP:（有咲）等等等一下` -- C# `CardArisaWait` (MatchHost.cs:9085-9188): the
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（有咲）等等等一下`）:
//! > （有咲）等等等一下： 
//! > [手]：
//! >  将此卡放置在[使用者]的[场地]。
//! > [持续]：
//!
//! > （1）任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。
//!
//! > （2）事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。
//!

use card_sdk::abi::HookKind;
use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "PPP:（有咲）等等等一下";

pub const ARISA_WAIT: CardDef = CardDef::new(
    "PPP:（有咲）等等等一下",
    &[
        On::Play("", None, arisa_wait),
        On::Hook(&[HookKind::EventAfter], "", Some(event_after_guard), event_after),
    ],
);

fn arisa_wait(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]。」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("arisa_wait_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("arisa_wait_placed")).player_id("who", player_id),
    );
    // TODO(规则书)[持续]（2）: 「事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，
    //   那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。」
    //   -- needs the `IEventDefer` interface (C# `Defer` / `Resolved`), the
    //   Fx.TurnStart hook, and the H.ResolveEvent / H.DrawEvent event routines
    //   (plus `Card.Detach` to retire deferred events when this card leaves).
    //   When Defer lands, the `by != Title` filter of (1) must be restored so
    //   this card's own deferred releases do not bank a crystal.
    Ok(())
}

/// `IEventDefer.Resolved` (C# `CardArisaWait.Resolved`) -- any event that
/// settles banks a crystal on this card.
/// Pure guard for [`event_after`] -- the activation gate. `false`
/// means the card is not activated at all.
fn event_after_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn event_after(player_id: i32) -> card_sdk::Asked {
    // 规则书[持续]（1）: 「任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。」
    // C# `by != Title` filters out this card's own deferred releases; while
    // Defer (2) is held there are none, so every `eventAfter` is foreign.
    ctx::add_crystals(1, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("arisa_wait_crystal"))
            .player_id("who", player_id)
            .i("n", ctx::crystals() as i64),
    );
    Ok(())
}
