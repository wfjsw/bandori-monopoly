//! `event:麻里奈小姐的礼物箱` -- 事件卡「麻里奈小姐的礼物箱」（中立事件 A16）.
//!
//! 事件文本（data/events.json, id `麻里奈小姐的礼物箱`）:
//! > 将此卡放置于场地中央，抽到的玩家的第3回合开始时放入事件弃牌。所有玩家[经过]CiRCLE时可[消耗]一次500资金，抽到的玩家投掷一次1d10（不受任何其他效果影响），如果投掷结果至少为6，那名玩家[获得]1200资金。

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

use crate::util::{expire, keep, roll};

const ID: &str = "麻里奈小姐的礼物箱";

/// The player who drew the event, on the instance's props.
const DRAWER: &str = "drawer";
/// How many of the drawer's own turn starts have passed.
const TURNS: &str = "turns";

pub const MARINA_BOX: CardDef = CardDef::new(
    "event:麻里奈小姐的礼物箱",
    &[
        On::Play("", None, play),
        // 「所有玩家[经过]CiRCLE时」 -- the tile is the condition now (the
        // body-top `!is_circle` early-out). The `always` guard is deleted: it
        // prefetched every pass of every tile for a no-op.
        On::Hook(&[HookKind::PassTile], "is_circle(tile.id)", None, on_pass),
        On::Hook(&[HookKind::TurnStartBefore], "", None, on_turn_start),
    ],
);

/// 规则书: 「将此卡放置于场地中央，抽到的玩家的第3回合开始时放入事件弃牌」
/// -- keep the event and remember who drew it; the turn-start hook below counts
/// their turns and files it away on the third.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::set_prop(DRAWER, player_id);
    ctx::set_prop(TURNS, 0);
    ctx::log(
        player_id,
        &Msg::new("log.event.marina_box_on").player_id("who", player_id),
    );
    Ok(())
}

/// 规则书: 「所有玩家[经过]CiRCLE时可[消耗]一次500资金，抽到的玩家投掷一次
/// 1d10（不受任何其他效果影响），如果投掷结果至少为6，那名玩家[获得]1200资金」
/// -- the passer is asked whether to spend; only a 「yes」 pays 500 and rolls.
/// The CiRCLE tile is the entry's condition (`is_circle(tile.id)`).
fn on_pass(_player_id: i32) -> card_sdk::Asked {
    let who = trigger::player_id();
    if who < 0 {
        return Ok(());
    }
    // 「可[消耗]一次500资金」 -- optional, a player decision. The ask is the
    // missing `passTile` prompt surface filled in: the passer is prompted here.
    // 「可」 means the spend is opt-in, so 「不消耗」 is the default (the choice
    // prompt's fallback is index 0). The two behaviour tests answer 「消耗」
    // explicitly; `drain()` (fallback) therefore declines.
    if ctx::money_of(who) < 500 {
        return Ok(());
    }
    let pay = ctx::ask_pick(
        who,
        &Msg::new("log.event.marina_box_title"),
        &Msg::new("log.event.marina_box_ask"),
        &[
            Msg::new("log.event.marina_box_no"),
            Msg::new("log.event.marina_box_yes"),
        ],
    )?;
    if pay != 1 {
        return Ok(());
    }
    ctx::pay(who, 500, &Msg::new("log.event.marina_box_pay"))?;
    let drawer = ctx::prop(DRAWER);
    // 「抽到的玩家投掷一次1d10（不受任何其他效果影响）」 -- a bare `ctx::roll`,
    // which raises no [反击] window and consults no dice modifiers.
    let face = roll(drawer, 1, 10);
    if face >= 6 {
        ctx::gain(who, 1200, &Msg::new("log.event.marina_box_win"))?;
    }
    Ok(())
}

/// 规则书: 「抽到的玩家的第3回合开始时放入事件弃牌」 -- the drawer is always
/// mid-turn when the card is drawn (it is drawn from a landing), so their
/// current turn is already the 1st. The 3rd turn start is the **2nd** one after
/// the draw: count drawer turn starts and expire on the second.
fn on_turn_start(_player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() != ctx::prop(DRAWER) {
        return Ok(());
    }
    let n = ctx::prop(TURNS) + 1;
    ctx::set_prop(TURNS, n);
    // The draw happens mid-turn-1, so turn starts 1 and 2 are the drawer's 2nd
    // and 3rd turns. 「第3回合开始时」 = `n >= 2`.
    if n >= 2 {
        expire(ID);
    }
    Ok(())
}