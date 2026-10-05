//! `PPP:（香澄）大家我都喜欢哦` -- C# `CardKasumiLoveAll` (MatchHost.cs:9037-9084):
//! park on 星之鼓动山丘; anyone who passes it is force-stopped there at half rent.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（香澄）大家我都喜欢哦`）:
//! > （香澄）大家我都喜欢哦：
//! > [手]：
//! > 将此卡放置在“星之鼓动山丘”上。
//! > [持续]：
//! > 其他玩家[经过]且[移动终点]不为此卡所在格子时那名玩家在此卡所在格子[强制停下]并将此卡放入[使用者]弃卡区且为[使用者]的团卡添加一个[奇迹水晶]，那名玩家此次[结算]如果[支付]地租则地租只算作原本的一半。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::abi::MoveKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const KASUMI_LOVE_ALL: CardDef = CardDef::new("PPP:（香澄）大家我都喜欢哦", &[
    On::Play(play),
    On::Hook(&[TriggerKind::PassTile], pass_tile),
]);

/// Where the card's tile is written down (C# `Tile` on the placed card).
const SLOT_TILE: &str = "kasumi_love_all_tile";

fn play(player_id: i32) {
    // 规则书[手]: 「将此卡放置在“星之鼓动山丘”上」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // H.TileNamed("星之鼓动山丘"))`.
    let tile = ctx::tile_named("星之鼓动山丘");
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "PPP:（香澄）大家我都喜欢哦",
        &Msg::new(key!("kasumi_love_all_note")),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("kasumi_love_all_placed")).player_id("who", player_id),
    );
    // TODO(ABI): 「将此卡放置在“星之鼓动山丘”上」 -- the placement is bound to that
    //   tile (C# `H.PlaceFromPlay(c, owner, tile)`), not the player's field; the ABI's
    //   `place_card` only parks the card at a player (same gap as `kokoro_circle`).
    //   The tile is remembered in `SLOT_TILE` so the hook below can key on it.
    if tile >= 0 {
        ctx::set_slot(player_id, SLOT_TILE, tile);
    }
}

/// 规则书[持续]: 「其他玩家[经过]且[移动终点]不为此卡所在格子时那名玩家在此卡所在格子
/// [强制停下]并将此卡放入[使用者]弃卡区且为[使用者]的团卡添加一个[奇迹水晶]，那名玩家
/// 此次[结算]如果[支付]地租则地租只算作原本的一半。」
/// -- C# `CardKasumiLoveAll.PassTile` -> `Stop`.
fn pass_tile(player_id: i32) {
    if trigger::kind() != TriggerKind::PassTile || !ctx::is_placed(player_id) {
        return;
    }
    let tile = ctx::slot(player_id, SLOT_TILE);
    if tile < 0 || trigger::tile() != tile {
        return;
    }
    // C# `m.Seat == User` -- the card's own placer is not stopped by it.
    if trigger::player_id() == player_id {
        return;
    }
    // C# `m.Remaining <= 0` -- 「[移动终点]不为此卡所在格子」: only a still-walking
    // pass is intercepted (the walk would otherwise end here).
    if trigger::move_remaining() <= 0 {
        return;
    }
    // C# `m.Teleport` -- a teleport does not walk past the tile.
    if trigger::move_kind() == Some(MoveKind::Teleport) {
        return;
    }
    // TODO(规则书)[持续]: the `H.AbnormalGate` stop guard (C# `Abnormal{Kind = "stop"}`
    //   -> `m.Stopped = true; m.Resolve = true`) is still held -- a blocker with a
    //   「不可阻挡」-style bypass is not in the vocabulary. The stop + rent shaping
    //   itself is written below.
    // C# `m.Stopped = true; m.Resolve = true; m.RentFactor *= 0.5`.
    ctx::plan::set_stop_at(tile);
    ctx::plan::set_resolve(true);
    // 规则书[持续]: 「那名玩家此次[结算]如果[支付]地租则地租只算作原本的一半」
    // -- milli-units: 500 = x0.5.
    ctx::plan::set_rent_factor(500);
    let mover = trigger::player_id();
    ctx::log(
        player_id,
        &Msg::new(key!("kasumi_love_all_stop"))
            .player_id("who", mover)
            .tile("tile", tile),
    );
    // 规则书[持续]: 「将此卡放入[使用者]弃卡区」 -- C# `H.Unplace(this, "discard",
    // "有人在这里停下了")`.
    ctx::unplace_card(player_id);
    ctx::to_discard(player_id, "PPP:（香澄）大家我都喜欢哦");
    // 规则书[持续]: 「为[使用者]的团卡添加一个[奇迹水晶]」 -- C# `H.AddBandCrystals(user, 1, ...)`.
    if !ctx::player_out(player_id) {
        ctx::add_band_crystals(player_id, 1, i32::MAX);
    }
}
