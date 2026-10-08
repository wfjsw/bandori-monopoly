//! `PPP:（香澄）大家我都喜欢哦` -- C# `CardKasumiLoveAll` (MatchHost.cs:9037-9084):
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（香澄）大家我都喜欢哦`）:
//! > （香澄）大家我都喜欢哦：
//! > [手]：
//! > 将此卡放置在“星之鼓动山丘”上。
//! > [持续]：
//! > 其他玩家[经过]且[移动终点]不为此卡所在格子时那名玩家在此卡所在格子[强制停下]并将此卡放入[使用者]弃卡区且为[使用者]的团卡添加一个[奇迹水晶]，那名玩家此次[结算]如果[支付]地租则地租只算作原本的一半。
//!
//! park on 星之鼓动山丘; anyone who passes it is force-stopped there at half rent.

use card_sdk::abi::{HookKind, MoveKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const KASUMI_LOVE_ALL: CardDef = CardDef::new(
    "PPP:（香澄）大家我都喜欢哦",
    &[
        On::Play(None, play, ""),
        On::Hook(&[HookKind::PassTile], None, pass_tile, ""),
    ],
);

/// Where the card's tile is written down (C# `Tile` on the placed card).

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在“星之鼓动山丘”上」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // H.TileNamed("星之鼓动山丘"))`.
    let tile = ctx::tile_named("星之鼓动山丘");
    ctx::set_dest(ctx::Dest::Field);
    // 规则书: 「将此卡放置在“星之鼓动山丘”上」 -- bound to that tile, so the
    // `PassTile` hook below can ask 「此卡所在格子」.
    ctx::place_card_on(
        player_id,
        tile,
        "PPP:（香澄）大家我都喜欢哦",
        &Msg::new(key!("kasumi_love_all_note")),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("kasumi_love_all_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// 规则书[持续]: 「其他玩家[经过]且[移动终点]不为此卡所在格子时那名玩家在此卡所在格子
/// [强制停下]并将此卡放入[使用者]弃卡区且为[使用者]的团卡添加一个[奇迹水晶]，那名玩家
/// 此次[结算]如果[支付]地租则地租只算作原本的一半。」
/// -- C# `CardKasumiLoveAll.PassTile` -> `Stop`.
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::PassTile || !ctx::is_placed() {
        return Ok(());
    }
    let tile = ctx::self_tile().unwrap_or(-1);
    if tile < 0 || trigger::tile() != tile {
        return Ok(());
    }
    // 规则书[持续]: 「[强制停下]」 is an abnormal movement effect -- it passes
    // the C# `H.WithCard(User, H.AbnormalGate(a))` gate so [反击] cards (安可)
    // get their window. `ctx::gate` returns false when the effect is guarded,
    // the mover is immune, or they are [不可阻挡] (「可选择受到的…[强制停下]…
    // 效果是否生效」).
    let mover = trigger::player_id();
    // 规则书[持续]: 「其他玩家[经过]」 -- other than the [使用者] (the placer).
    // C# `m.Seat == User` skips the placer: their own card does not stop them.
    if mover == player_id {
        return Ok(());
    }
    // C# `m.Remaining <= 0` -- 「[移动终点]不为此卡所在格子」: only a still-walking
    // pass is intercepted (the walk would otherwise end here).
    if trigger::move_remaining() <= 0 {
        return Ok(());
    }
    // C# `m.Teleport` -- a teleport does not walk past the tile.
    if trigger::move_kind() == Some(MoveKind::Teleport) {
        return Ok(());
    }
    if !ctx::gate(mover, card_sdk::abi::AbKind::Stop) {
        return Ok(());
    }
    // C# `m.Stopped = true; m.Resolve = true; m.RentFactor *= 0.5`
    // (MatchHost.cs:9063-9078). `set_stop_at` is the forced stop from this
    // `PassTile` hook: the walk settles at the stop tile.
    ctx::plan::set_stop_at(tile);
    ctx::plan::set_resolve(true);
    // 规则书[持续]: 「那名玩家此次[结算]如果[支付]地租则地租只算作原本的一半」
    // -- milli-units: 500 = x0.5.
    ctx::plan::set_rent_factor(500);
    ctx::log(
        player_id,
        &Msg::new(key!("kasumi_love_all_stop"))
            .player_id("who", mover)
            .tile("tile", tile),
    );
    // 规则书[持续]: 「将此卡放入[使用者]弃卡区」 -- C# `H.Unplace(this, "discard",
    // "有人在这里停下了")`.
    ctx::set_dest(ctx::Dest::Graveyard);
    // 规则书[持续]: 「为[使用者]的团卡添加一个[奇迹水晶]」 -- C# `H.AddBandCrystals(user, 1, ...)`.
    if !ctx::player_out(player_id) {
        ctx::add_band_crystals(player_id, 1, i32::MAX);
    }
    Ok(())
}
