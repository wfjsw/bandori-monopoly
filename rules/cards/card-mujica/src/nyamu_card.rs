//! `Mujica:（喵梦）` -- C# `CardNyamuCard` (MatchHost.cs:6080-6116): placed card
//! whose flip face-down may cost 2 fire, flip face-up draws 1.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（喵梦）`）:
//! > （喵梦）
//! >
//! > （1）将此卡放置于场上
//! >
//! > （2）
//! > 1. 每当此卡将从正面翻至背面时可选择失去2个火罐。
//! > 2. 每当此卡将从背面翻至正面时此卡拥有者抽一张卡。
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const NYAMU_CARD: CardDef = CardDef::new("Mujica:（喵梦）", &[
    On::Play(nyamu_card),
]);

fn nyamu_card(player_id: i32) {
    // 规则书（1）: 「将此卡放置于场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "Mujica:（喵梦）", &Msg::new(key!("nyamu_note")));
    ctx::log(player_id, &Msg::new(key!("nyamu_placed")).player_id("who", player_id));
    // TODO(规则书)（2）1: 「每当此卡将从正面翻至背面时可选择失去2个火罐」
    // -- needs the Fx.Flipped hook (C# `CardNyamuCard.Flipped(down: true)` ->`
    // `H.AskYes` + `H.SpendFire(Seat, 2)`). The flip source itself (a tile or
    // card effect that turns placed cards face-down) is engine-side.
    // The discard body is expressible once the hook exists:
    //   if ctx::fire(player) >= 2 && ctx::ask_yes(...) {
    //       // C# `H.SpendFire(Seat, 2, ...)`.
    //       ctx::spend_fire(player, 2, &Msg::new(key!("nyamu_note")));
    //   }
    // TODO(规则书)（2）2: 「每当此卡将从背面翻至正面时此卡拥有者抽一张卡」
    // -- needs the same Fx.Flipped hook (C# `Flipped(down: false)` ->
    // `H.DrawR(Seat, 1, ...)`), i.e. `ctx::draw(player_id, 1)`.
}