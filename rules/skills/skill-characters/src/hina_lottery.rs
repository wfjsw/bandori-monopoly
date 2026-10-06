//! `skill:冰川日菜:日菜抽中的大奖`
//!
//! 规则书（skill sheet, 冰川日菜）:
//! > （1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得冰川日菜
//! > 的（2）技能
//! > （2）每回合开始时必须进行1次投掷1d4，根据最后投掷的结果在下回合开始前获得
//! > 以下角色的（2）技能：结果为1是丸山彩；结果为2是白鹭千圣；结果为3是若宫伊芙；
//! > 结果为4是大和麻弥。
//!
//! （1）'s grant is the skill rule placed on the grantee's field (see
//! [`super::aya_with`]).
//!
//! （2） is a *rotating* grant: the 1d4 picks whose (2) the roller holds until
//! the next turn start. Placing that character's skill rule for the duration
//! and unplacing it at the next turn start is the same shape as （1）, with an
//! expiry -- which is what the keyed state's `expires` is for.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

const FANS_UP: &str = "P✽P粉丝(正)";
/// Which character's (2) this player is currently holding, or `""`.
const BORROWED: &str = "skill.hinaLottery.borrowed";

/// The four the clause names, in the order the 1d4 picks them.
const POOL: [&str; 4] = [
    "skill:丸山彩:With~",
    "skill:白鹭千圣:保持坦率的你",
    "skill:若宫伊芙:天下统一",
    "skill:大和麻弥:朝阳照耀的片刻",
];

pub const HINA_LOTTERY: CardDef = CardDef::new(
    "skill:冰川日菜:日菜抽中的大奖",
    &[
        On::Hook(&[HookKind::DeckAtGameStart], |_| true, at_start),
        On::Hook(&[HookKind::TurnStartBefore], mine, roll),
        On::Hook(&[HookKind::TurnStart], mine, expire),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （1）「游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得冰川日菜
/// 的（2）技能」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    ctx::add_tok(player_id, FANS_UP, 5, i32::MAX);
    for p in 0..ctx::player_count() {
        if p == player_id || ctx::player_out(p) || ctx::in_band(p, "Pastel✽Palettes") {
            continue;
        }
        ctx::place_card(
            p,
            "skill:冰川日菜:日菜抽中的大奖",
            &Msg::new(key!("hina_lottery_granted")),
        );
    }
    Ok(())
}

/// （2）「每回合开始时必须进行1次投掷1d4，根据最后投掷的结果在下回合开始前获得
/// 以下角色的（2）技能」.
fn roll(player_id: i32) -> card_sdk::Asked {
    // 「在下回合开始前」 -- last round's borrow comes off first.
    expire(player_id)?;
    let n = ctx::roll(player_id, 1, 4);
    let Some(id) = POOL.get((n - 1).max(0) as usize) else {
        return Ok(());
    };
    ctx::place_card(player_id, id, &Msg::new(key!("hina_lottery_note")));
    state::set(player_id, BORROWED, (n - 1) as i32);
    ctx::log(
        player_id,
        &Msg::new(key!("hina_lottery_rolled")).i("n", n as i64),
    );
    Ok(())
}

/// 「在下回合开始前」 -- the borrow wears off.
fn expire(player_id: i32) -> card_sdk::Asked {
    let k = state::get(player_id, BORROWED);
    if k < 0 {
        return Ok(());
    }
    if let Some(id) = POOL.get(k as usize) {
        if let Some(uid) = ctx::find_card(player_id, id) {
            ctx::unplace_at(uid);
        }
    }
    state::set(player_id, BORROWED, -1);
    Ok(())
}
