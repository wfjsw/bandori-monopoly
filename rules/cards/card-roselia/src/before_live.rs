//! `R:live前的准备` -- C# `CardBeforeLive` (MatchHost.cs:10673-10709): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:live前的准备`）:
//! > live前的准备：
//! > [反击] 经过江户川乐器店时可打出此卡，使自己在江户川乐器店强制停下并触发结算
//!
//! on passing 江户川乐器店: force yourself to stop there and settle.

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const BEFORE_LIVE: CardDef = CardDef::new(
    "R:live前的准备",
    &[On::Counteract(&[ChainKind::Pass], can_counteract, counteract)],
);

/// 规则书[反击]: 「[反击] 经过江户川乐器店时可打出此卡」
fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「经过江户川乐器店时」 -- C# `t.Kind == "pass" && t.Seat == seat &&
    // H.Name(t.Tile) == "江户川乐器店"`.
    if trigger::kind() != TriggerKind::Pass || trigger::player_id() != player_id {
        return false;
    }
    let tile = trigger::tile();
    let shop = ctx::tile_named("江户川乐器店");
    // 规则书[反击]: the C# also requires `t.Move.Remaining > 0` (still mid-move).
    if trigger::move_remaining() <= 0 {
        return false;
    }
    tile >= 0 && tile == shop
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「使自己在江户川乐器店强制停下并触发结算」
    ctx::log(
        player_id,
        &Msg::new(key!("before_live_stop"))
            .player_id("who", player_id)
            .tile("tile", ctx::tile_named("江户川乐器店")),
    );
    // 规则书[反击]: 「强制停下并触发结算」 -- C# `CardBeforeLive.Counteract`:
    // `m.Stopped = true; m.Resolve = true` behind `H.AbnormalGate`.
    if !ctx::gate(trigger::player_id(), card_sdk::abi::AbKind::Stop) {
        return Ok(());
    }
    let shop = ctx::tile_named("江户川乐器店");
    if shop >= 0 {
        ctx::plan::set_stop_at(shop);
        ctx::plan::set_resolve(true);
    }
    Ok(())
}
