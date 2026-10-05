//! `R:live前的准备` -- C# `CardBeforeLive` (MatchHost.cs:10673-10709): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:live前的准备`）:
//! > live前的准备：
//! > [反击] 经过江户川乐器店时可打出此卡，使自己在江户川乐器店强制停下并触发结算
//!
//! on passing 江户川乐器店: force yourself to stop there and settle.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const BEFORE_LIVE: CardDef = CardDef {
    id: "R:live前的准备",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// 规则书[反击]: 「[反击] 经过江户川乐器店时可打出此卡」
fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「经过江户川乐器店时」 -- C# `t.Kind == "pass" && t.Seat == seat &&
    // H.Name(t.Tile) == "江户川乐器店"`.
    if trigger::kind() != TriggerKind::Pass || trigger::seat() != seat {
        return false;
    }
    let tile = trigger::tile();
    let shop = ctx::tile_named("江户川乐器店");
    // TODO(规则书)[反击]: the C# also requires `t.Move.Remaining > 0` (still mid-move);
    // the pass trigger carries no remaining-steps count.
    tile >= 0 && tile == shop
}

fn react(seat: i32) {
    // 规则书[反击]: 「使自己在江户川乐器店强制停下并触发结算」
    ctx::log(
        seat,
        &Msg::new(key!("before_live_stop"))
            .seat("who", seat)
            .tile("tile", ctx::tile_named("江户川乐器店")),
    );
    // TODO(规则书)[反击]: 「强制停下并触发结算」 -- needs the move-stop hook (C#
    // `CardBeforeLive.React`: `m.Stopped = true; m.Resolve = true` behind
    // `H.AbnormalGate`), so the current move stops on 江户川乐器店 and settles there.
}