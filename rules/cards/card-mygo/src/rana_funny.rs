//! `MyGO:（乐奈）有趣的女人` -- C# `CardRanaFunny` (MatchHost.cs:6792-6854):
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（乐奈）有趣的女人`）:
//! > （乐奈）有趣的女人：
//! > 将此卡置于当前格子上，每当有人经过且未在其上[触发结算]时为其增加一个奇迹水晶，当奇迹水晶总数为5或以上时使下一个经过的你以外的玩家选择失去一个“抹茶芭菲”或强制停下并[触发结算]，如果强制停下则此卡洗入弃牌堆。 由此卡效果导致[触发结算]时需支付资金减半
//!
//! plant this card on the current tile; passers-by who do not settle there
//! grow a miracle crystal on it, and at 5+ the next foreign passer is trapped.

use card_sdk::abi::{HookKind, MoveKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const RANA_FUNNY: CardDef = CardDef::new(
    "MyGO:（乐奈）有趣的女人",
    &[
        On::Play(None, rana_funny, ""),
        On::Hook(&[HookKind::PassTile], None, pass_tile, ""),
    ],
);

const ID: &str = "MyGO:（乐奈）有趣的女人";

fn rana_funny(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「将此卡置于当前格子上」 -- C# `H.PlaceFromPlay(c, c.Seat, pos)`.
    ctx::set_dest(ctx::Dest::Field);
    // 规则书: 「将此卡置于当前格子上」 -- bound to where the player is. The
    // `PassTile` hook still filters on the slot below; a `card_tile()` read would
    // let it use `Card.Tile` directly, which is the smaller follow-up.
    ctx::place_card_on(
        player_id,
        ctx::player_pos(player_id),
        ID,
        &Msg::new(key!("rana_funny_note")),
    );
    // The instance carries its tile (`place_card_on` stamps it); `self_tile()`
    // reads it back. A `slot` scratch would miss a `place_raw` / `place_on_tile`
    // arrangement that never ran this play body.
    ctx::log(
        player_id,
        &Msg::new(key!("rana_funny_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CardRanaFunny.PassTile` -- a passer who does not settle here grows a
/// miracle crystal on the card; at 5+ crystals a foreign passer is trapped.
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::PassTile || !ctx::is_placed() {
        return Ok(());
    }
    // 规则书: 「将此卡置于当前格子上」 -- the instance carries its tile;
    // `self_tile()` reads it back (works for a `place_on_tile` arrangement too).
    let tile = ctx::self_tile().unwrap_or(-1);
    if tile < 0 || trigger::tile() != tile {
        return Ok(());
    }
    // C# `m.Teleport` never grows a crystal (`t.Move` flags).
    if trigger::move_kind() == Some(MoveKind::Teleport) {
        return Ok(());
    }
    // 规则书: 「未在其上[触发结算]」 -- C# grows a crystal only when
    // `m.Remaining > 0` (the walker is passing through); the last tile of the
    // walk (`m.Remaining <= 0`) is where they stop to settle, so it is not a
    // pass.
    if trigger::move_remaining() <= 0 {
        return Ok(());
    }
    let who = trigger::player_id();
    // 规则书: 「当奇迹水晶总数为5或以上时使下一个经过的你以外的玩家选择失去一个"抹茶芭菲"
    // 或强制停下并[触发结算]」 -- C# `Crystals >= 5 && m.Seat != User` branches
    // into `Trap` instead of growing a crystal.
    if ctx::crystals() >= 5 && who != player_id {
        trap(player_id, who, tile)?;
        return Ok(());
    }
    // 规则书: 「每当有人经过且未在其上[触发结算]时为其增加一个奇迹水晶」
    ctx::add_crystals(1, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("rana_funny_crystal")).player_id("who", who),
    );
    Ok(())
}

/// C# `CardRanaFunny.Trap` -- the 5+ crystal trap on a foreign passer: lose a
/// 抹茶芭菲 or be forced to stop here and settle at half pay.
fn trap(owner: i32, who: i32, tile: i32) -> card_sdk::Asked {
    // 规则书: 「选择失去一个"抹茶芭菲"或强制停下并[触发结算]」 -- C# `H.AskYes`
    // only when the passer holds a 抹茶芭菲 (`aiYes` prefers losing it)?; with
    // none in hand the forced stop is the only option.
    if ctx::tok(who, "抹茶芭菲") > 0 {
        let lose = ctx::ask_yes(
            who,
            &Msg::new(key!("rana_funny_trap_title")),
            &Msg::new(key!("rana_funny_trap_ask")),
        )?;
        if lose {
            // 规则书: 「失去一个"抹茶芭菲"」 -- C# `H.AddTok(who, "抹茶芭菲", -1)`
            // (default `max = int.MaxValue`).
            ctx::add_tok(who, "抹茶芭菲", -1, i32::MAX)?;
            ctx::log(
                owner,
                &Msg::new(key!("rana_funny_parfait")).player_id("who", who),
            );
            return Ok(());
        }
    }
    // 规则书: 「强制停下并[触发结算]」 -- a [强制停下] is an abnormal movement
    // effect; it passes the C# `H.WithCard(User, H.AbnormalGate(a))` gate so
    // [反击] cards (安可) get their window. `ctx::gate` returns false when the
    // effect is guarded, the mover is immune, or they are [不可阻挡].
    if !ctx::gate(who, card_sdk::abi::AbKind::Stop) {
        return Ok(());
    }
    // 规则书: 「由此卡效果导致[触发结算]时需支付资金减半」 -- `set_stop_at` is
    // the forced stop from this `PassTile` hook (the walk settles at the stop
    // tile); the pay factor is milli-units (500 = x0.5). The C# multiplies the
    // in-flight factor; this sets the x0.5 value the walk starts from.
    ctx::plan::set_stop_at(tile);
    ctx::plan::set_resolve(true);
    ctx::plan::set_pay_factor(500);
    ctx::log(
        owner,
        &Msg::new(key!("rana_funny_stop"))
            .player_id("who", who)
            .tile("tile", tile),
    );
    ctx::set_transfer_to_dest(owner, ctx::Dest::Graveyard);
    Ok(())
}
