//! `HHW:（花音）Wacha Mocha 啪嗒进行曲` -- C# `CardKanonMarch` (MatchHost.cs:4234-4291).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（花音）Wacha Mocha 啪嗒进行曲`）:
//! > （花音）Wacha Mocha 啪嗒进行曲：[场]花音每次倒走获得一个水母标记，当水母标记到达9个时可以清除所有标记传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后置入弃牌堆。
//!

use alloc::vec::Vec;

use card_sdk::abi::{HookKind, MoveKind};
use card_sdk::{ctx, key, CardDef, On, Msg};

const ID: &str = "HHW:（花音）Wacha Mocha 啪嗒进行曲";
/// 「水母标记」.
const JELLY: &str = "水母标记";

pub const KANON_MARCH: CardDef = CardDef::new("HHW:（花音）Wacha Mocha 啪嗒进行曲", &[
    On::Play(None, play),
    On::Hook(&[HookKind::Pass], mine, on_pass),
    On::Play(Some(can_jump), jump)]);

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「[场]」 -- a field card; C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "HHW:（花音）Wacha Mocha 啪嗒进行曲", &Msg::new(key!("kanon_march_note")));
    ctx::log(player_id, &Msg::new(key!("kanon_march_placed")).player_id("who", player_id));
    ctx::add_tok(player_id, JELLY, 0, 9);
    Ok(())
}

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「花音每次倒走获得一个水母标记」 -- C# `CardKanonMarch.Arrive`
/// (`m.Reverse && !m.Teleport`), which is a backward walk that is not a teleport.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::move_kind() == Some(MoveKind::Teleport) {
        return Ok(());
    }
    if ctx::trigger::move_dir() >= 0 {
        return Ok(());
    }
    ctx::add_tok(player_id, JELLY, 1, 9);
    ctx::log(player_id, &Msg::new(key!("kanon_march_mark")).i("n", ctx::tok(player_id, JELLY) as i64));
    Ok(())
}

/// 「当水母标记到达9个时可以清除所有标记传送到#4水族馆或者 #30弦卷豪宅，视为本次
/// 主要移动(喊出呼诶诶～!)，然后置入弃牌堆」.
fn can_jump(player_id: i32) -> Option<Msg> {
    if ctx::tok(player_id, JELLY) < 9 {
        return Some(Msg::new(key!("kanon_march_need")));
    }
    None
}

fn jump(player_id: i32) -> card_sdk::Asked {
    if ctx::tok(player_id, JELLY) < 9 {
        return Ok(());
    }
    let a = ctx::tile_named("水族馆");
    let b = ctx::tile_named("弦卷豪宅");
    let mut pool: Vec<i32> = Vec::new();
    if a >= 0 { pool.push(a); }
    if b >= 0 { pool.push(b); }
    if pool.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_tile(
        player_id,
        &Msg::new(key!("kanon_march_title")),
        &Msg::new(key!("kanon_march_where")),
        &pool,
    )?;
    // 「清除所有标记…然后置入弃牌堆」
    ctx::set_tok(player_id, JELLY, 0);
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(pick);
    ctx::plan::set_resolve(true);
    ctx::unplace_self();
    ctx::to_discard(player_id, ID);
    ctx::log(player_id, &Msg::new(key!("kanon_march_jump")).tile("tile", pick));
    Ok(())
}