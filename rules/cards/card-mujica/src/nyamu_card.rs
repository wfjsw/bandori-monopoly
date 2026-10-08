//! `Mujica:（喵梦）` -- C# `CardNyamuCard` (MatchHost.cs:6080-6116): placed card
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（喵梦）`）:
//! > （喵梦）
//! >  
//! > （1）将此卡放置于场上 
//!
//! > （2）
//! > 1. 每当此卡将从正面翻至背面时可选择失去2个火罐。
//! > 2. 每当此卡将从背面翻至正面时此卡拥有者抽一张卡。
//!
//! whose flip face-down may cost 2 fire, flip face-up draws 1.

use card_sdk::{ctx, key, CardDef, Msg, On};

const ID: &str = "Mujica:（喵梦）";
/// `card_face_down` as of the last check, so a flip is a change.
const WAS_DOWN: &str = "nyamu.wasDown";

pub const NYAMU_CARD: CardDef = CardDef::new(
    "Mujica:（喵梦）",
    &[
        On::Play(None, nyamu_card, ""),
        On::Hook(&[card_sdk::abi::HookKind::TurnEnd], None, watch_flips, card_sdk::pre::MINE),
    ],
)
    .legacy(&[(1, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「每当此卡将从背面翻至正面时此卡拥有者抽一张卡」.
fn watch_flips(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() {
        return Ok(());
    }
    let now = ctx::self_face_down();
    let was = ctx::slot(player_id, WAS_DOWN) != 0;
    ctx::set_slot(player_id, WAS_DOWN, now as i32);
    if was && !now {
        ctx::draw(player_id, 1)?;
        ctx::log(player_id, &Msg::new(key!("nyamu_flipped_up")));
    }
    Ok(())
}

fn nyamu_card(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「将此卡放置于场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "Mujica:（喵梦）", &Msg::new(key!("nyamu_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("nyamu_placed")).player_id("who", player_id),
    );
    // TODO(规则书)[judgement]（2）1: 「每当此卡将从正面翻至背面时可选择失去2个火罐」
    //   -- the clause does not say what losing the 2 fire pots buys. C#
    //   `CardNyamuCard.Flipped(down: true)` runs `H.AskYes` + `H.SpendFire(Seat, 2)`
    //   and then stops there, so the C# is no more specific. Read here as "pay 2
    //   fire pots to keep the card face-up"; without that reading the spend is
    //   pure loss and the clause would never be taken.
    // （2）2「每当此卡将从背面翻至正面时此卡拥有者抽一张卡」 -- observed by polling
    // the face-down flag at the turn end (`Fx.Flipped` does not exist).
    Ok(())
}
