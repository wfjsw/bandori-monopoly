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

use card_sdk::{ctx, key, CardDef, Msg};

pub const KASUMI_LOVE_ALL: CardDef = CardDef {
    id: "PPP:（香澄）大家我都喜欢哦",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    // 规则书[手]: 「将此卡放置在“星之鼓动山丘”上」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // H.TileNamed("星之鼓动山丘"))`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        seat,
        "PPP:（香澄）大家我都喜欢哦",
        &Msg::new(key!("kasumi_love_all_note")),
    );
    ctx::log(
        seat,
        &Msg::new(key!("kasumi_love_all_placed")).seat("who", seat),
    );
    // TODO(ABI): 「将此卡放置在“星之鼓动山丘”上」 -- the placement is bound to that
    //   tile (C# `H.PlaceFromPlay(c, owner, tile)`), not the seat's field; the ABI's
    //   `place_card` only parks the card at a seat (same gap as `kokoro_circle`).
    // TODO(规则书)[持续]: 「其他玩家[经过]且[移动终点]不为此卡所在格子时那名玩家在此卡
    //   所在格子[强制停下]并将此卡放入[使用者]弃卡区且为[使用者]的团卡添加一个[奇迹水晶]，
    //   那名玩家此次[结算]如果[支付]地租则地租只算作原本的一半。」
    //   -- needs the Fx.PassTile hook (C# `CardKasumiLoveAll.PassTile` -> `Stop`:
    //   `m.Remaining > 0 && !m.Teleport`), the AbnormalGate forced-stop gate
    //   (C# `Abnormal{Kind = "stop"}` -> `m.Stopped = true; m.Resolve = true`), a
    //   rent-halving pay factor (`m.RentFactor *= 0.5`), then `H.Unplace(this,
    //   "discard", ...)` and `H.AddBandCrystals(user, 1, ...)`.
}