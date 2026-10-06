//! `skill:若宫伊芙:天下统一`
//!
//! 规则书（skill sheet, 若宫伊芙）:
//! > （1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得若宫伊芙
//! > 的（2）技能
//! > （2）当你因任意原因投掷前可选择将自己Y个正面[P✽P粉丝]变反并为此次投掷结果增加
//! > Yd4；如果自己是Pastel✽Palettes角色则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正，
//! > 否则将所有Pastel✽Palettes角色的1个反面[P✽P粉丝]变正
//!
//! （1）'s grant is the skill rule placed on the grantee's field (see
//! [`super::aya_with`]).
//!
//! （2） 「因任意原因投掷前」 is every roll, not just the move dice -- `RollAfter`
//! is the moment the face exists, and 「为此次投掷结果增加Yd4」 adds Yd4 to it.

use card_sdk::abi::HookKind;
use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

const FANS_UP: &str = "P✽P粉丝(正)";
const FANS_DOWN: &str = "P✽P粉丝(反)";

pub const EVE_UNIFY: CardDef = CardDef::new("skill:若宫伊芙:天下统一", &[
    On::Hook(&[HookKind::DeckAtGameStart], |_| true, at_start),
    On::Hook(&[HookKind::RollAfter], mine, on_roll)]);

fn mine(player_id: i32) -> bool {
    if card_sdk::ctx::skill_blocked(player_id, "Pastel✽Palettes") {
        return false;
    }
    ctx::trigger::player_id() == player_id
}

/// （1）「游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得若宫伊芙的
/// （2）技能」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    ctx::add_tok(player_id, FANS_UP, 5, i32::MAX);
    for p in 0..ctx::player_count() {
        if p == player_id || ctx::player_out(p) || ctx::in_band(p, "Pastel✽Palettes") {
            continue;
        }
        ctx::place_card(p, "skill:若宫伊芙:天下统一", &Msg::new(key!("eve_unify_granted")));
    }
    Ok(())
}

/// （2）「将自己Y个正面[P✽P粉丝]变反并为此次投掷结果增加Yd4」.
fn on_roll(player_id: i32) -> card_sdk::Asked {
    if ctx::fixed_roll().is_some() {
        return Ok(());
    }
    let up = ctx::tok(player_id, FANS_UP);
    if up < 1 {
        return Ok(());
    }
    let y = ctx::ask_number(
        player_id,
        &Msg::new(key!("eve_unify_title")),
        &Msg::new(key!("eve_unify_ask")),
        0,
        up,
    )?;
    if y < 1 {
        return Ok(());
    }
    let mut y = y;
    ctx::add_tok(player_id, FANS_UP, -y, i32::MAX);
    ctx::add_tok(player_id, FANS_DOWN, y, i32::MAX);
    // 「为此次投掷结果增加Yd4」
    let add = ctx::roll(player_id, y, 4);
    let before = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
    ctx::trigger::set_move_roll(before + add);
    if ctx::in_band(player_id, "Pastel✽Palettes") {
        for p in 0..ctx::player_count() {
            if y <= 0 {
                break;
            }
            if p == player_id || ctx::player_out(p) || ctx::in_band(p, "Pastel✽Palettes") {
                continue;
            }
            let down = ctx::tok(p, FANS_DOWN).min(y);
            if down > 0 {
                ctx::add_tok(p, FANS_DOWN, -down, i32::MAX);
                ctx::add_tok(p, FANS_UP, down, i32::MAX);
                y -= down;
            }
        }
    } else {
        for p in 0..ctx::player_count() {
            if ctx::player_out(p) || !ctx::in_band(p, "Pastel✽Palettes") {
                continue;
            }
            if ctx::tok(p, FANS_DOWN) > 0 {
                ctx::add_tok(p, FANS_DOWN, -1, i32::MAX);
                ctx::add_tok(p, FANS_UP, 1, i32::MAX);
            }
        }
    }
    ctx::log(player_id, &Msg::new(key!("eve_unify_done")).i("n", add as i64));
    Ok(())
}