//! `skill:白鹭千圣:保持坦率的你`
//!
//! 规则书（skill sheet, 白鹭千圣）:
//! > （1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得白鹭千圣
//! > 的（2）技能
//! > （2）自己每次收取资金时可选择将自己Y个正面[P✽P粉丝]变反，此次获得的分摊前数量
//! > 增加Y×100；如果自己是Pastel✽Palettes角色则将Y个其他乐队玩家拥有的反面[P✽P粉丝]
//! > 变正，否则将所有Pastel✽Palettes角色的1个反面[P✽P粉丝]变正
//!
//! （1）'s grant is the skill rule placed on the grantee's field (see
//! [`super::aya_with`]). （2） is 丸山彩's (2) with the sign flipped: a *gain*
//! goes up rather than a payment going down, and the fan tail is identical.

use card_sdk::abi::HookKind;
use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

const FANS_UP: &str = "P✽P粉丝(正)";
const FANS_DOWN: &str = "P✽P粉丝(反)";

pub const CHISATO_FRANK: CardDef = CardDef::new("skill:白鹭千圣:保持坦率的你", &[
    On::Hook(&[HookKind::DeckAtGameStart], |_| true, at_start),
    On::Hook(&[HookKind::PayChoose], mine, on_gain)]);

fn mine(player_id: i32) -> bool {
    if card_sdk::ctx::skill_blocked(player_id, "Pastel✽Palettes") {
        return false;
    }
    ctx::trigger::player_id() == player_id
}

/// （1）「游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得白鹭千圣的
/// （2）技能」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    ctx::add_tok(player_id, FANS_UP, 5, i32::MAX);
    for p in 0..ctx::player_count() {
        if p == player_id || ctx::player_out(p) || ctx::in_band(p, "Pastel✽Palettes") {
            continue;
        }
        ctx::place_card(p, "skill:白鹭千圣:保持坦率的你", &Msg::new(key!("chisato_frank_granted")));
    }
    Ok(())
}

/// （2）「自己每次收取资金时可选择将自己Y个正面[P✽P粉丝]变反，此次获得的分摊前
/// 数量增加Y×100」 -- the gain is `to == me` and the figure is the amount.
fn on_gain(player_id: i32) -> card_sdk::Asked {
    let amount = ctx::trigger::value();
    if amount <= 0 || ctx::trigger::target() != player_id {
        return Ok(());
    }
    let up = ctx::tok(player_id, FANS_UP);
    if up < 1 {
        return Ok(());
    }
    let mut y = ctx::ask_number(
        player_id,
        &Msg::new(key!("chisato_frank_title")),
        &Msg::new(key!("chisato_frank_ask")).i("n", amount as i64),
        0,
        up,
    )?;
    if y < 1 {
        return Ok(());
    }
    ctx::add_tok(player_id, FANS_UP, -y, i32::MAX);
    ctx::add_tok(player_id, FANS_DOWN, y, i32::MAX);
    // 「此次获得的分摊前数量增加Y×100」
    ctx::trigger::set_pay_amount(amount + y * 100);
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
    ctx::log(player_id, &Msg::new(key!("chisato_frank_done")).i("n", y as i64));
    Ok(())
}