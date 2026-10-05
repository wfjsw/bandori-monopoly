//! `PPP:（沙绫）总有一天要给这片天空命名` -- C# `CardSaayaSky` (MatchHost.cs:9199-9240):
//! park on 山吹面包房; after the user passes it, +1 fire and the card hops to 3d20.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（沙绫）总有一天要给这片天空命名`）:
//! > （沙绫）总有一天要给这片天空命名：
//! >
//! > （1）将此卡放置在山吹面包房
//! >
//! > （2）[使用者][经过]此卡后在回合结束后获得1个[火罐]，然后投掷3d20将此卡放置在投掷结果的格子上
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const SAAYA_SKY: CardDef = CardDef {
    id: "PPP:（沙绫）总有一天要给这片天空命名",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    // 规则书（1）: 「将此卡放置在山吹面包房」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // H.TileNamed("山吹面包房"))`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        seat,
        "PPP:（沙绫）总有一天要给这片天空命名",
        &Msg::new(key!("saaya_sky_note")),
    );
    ctx::log(seat, &Msg::new(key!("saaya_sky_placed")).seat("who", seat));
    // TODO(ABI)（1）: 「将此卡放置在山吹面包房」 -- the placement is bound to that
    //   tile (C# `H.PlaceFromPlay(c, owner, tile)`), not the seat's field; the ABI's
    //   `place_card` only parks the card at a seat.
    // TODO(规则书)（2）: 「[使用者][经过]此卡后在回合结束后获得1个[火罐]，然后投掷3d20
    //   将此卡放置在投掷结果的格子上」 -- needs the Fx.PassTile hook to set the
    //   per-card `_passed` flag (C# `CardSaayaSky.PassTile`), the Fx.TurnEndAfter
    //   hook (C# `CardSaayaSky.TurnEndAfter`) to `ctx::gain_fire(seat, 1, ...)`,
    //   `ctx::roll(seat, 3, 20)`, and tile re-placement of this card
    //   (`Tile = (num - 1) % tiles`; note the C# result is the *tile index* of the
    //   roll minus one, not a ring hop from anywhere).
}