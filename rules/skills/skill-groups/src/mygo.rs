//! `skill:MyGO!!!!!:迷途之星`
//!
//! 规则书（band sheet, MyGO!!!!!）:
//! > （1）开局时投掷3d20，并取出目作为你本局游戏的起始点 （2）若移动掷骰出目为16
//! > 及以上，为此卡添加一个[奇迹水晶]（上限1）。你的回合中，可于移动掷骰前选择移动
//! > 1格以替代移动掷骰并移除一个[奇迹水晶] （3）每次你的卡在未生效的情况下进入弃牌
//! > 堆时，为此卡添加一个可超出上限的[奇迹水晶]（最多超出2个）；你的回合中，可移除
//! > 此卡的两个[奇迹水晶]以抽一张卡。
//!
//! 「为此卡添加一个[奇迹水晶]」 is a *band-card* crystal -- `add_card_crystals`
//! on this rule's own id -- not the player's counter.

use bandori_std::state_key; use card_sdk::abi::{ActionKind};
use bandori_std::ctx::{self, plan, state};
use card_sdk::{Action, CardDef, key, Msg, On};

const ID: &str = "skill:MyGO!!!!!:迷途之星";
/// 「可超出上限的[奇迹水晶]（最多超出2个）」 -- overflow allowance.
const OVER: &str = "skill.mygo.over";

pub const MYGO: CardDef = CardDef::new(
    "skill:MyGO!!!!!:迷途之星",
    &[
        // （1）「开局时投掷3d20，并取出目作为你本局游戏的起始点」 -- a start
        // position: the before-match-start point decides those.
        On::Action(Action::DeckBeforeGame("", "", None, at_start)),
        On::Action(Action::DiceRollAfter("actor == owner && move.roll >= 16", "", None, after_roll)),
        On::Action(Action::Discarded(card_sdk::pre::MINE, "", None, on_discarded)),
        // One Play entry for both press effects (the engine dispatches only
        // the first). 规则书（2）/（3） give two independent 「可」 abilities
        // (move-1 before the move roll for 1 crystal; draw 1 for 2 crystals)
        // rather than one press with two modes, so each press activates one of
        // them and the player presses again for the other. The gate admits
        // whenever either is available; the body lists the available options
        // and skips the prompt when only one is.
        On::Play("", "", Some(cant_play), press),
    ],
);

/// （1）「开局时投掷3d20，并取出目作为你本局游戏的起始点」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    let roll = ctx::roll(player_id, 3, 20);
    let start = (roll % ctx::tile_count().max(1) + ctx::tile_count()) % ctx::tile_count().max(1);
    state::set(player_id, "skill.mygo.start", start);
    ctx::teleport_to(player_id, start);
    ctx::log(
        player_id,
        &Msg::new(key!("mygo_start"))
            .i("n", roll as i64)
            .tile("tile", start),
    );
    Ok(())
}

/// （2）「若移动掷骰出目为16及以上，为此卡添加一个[奇迹水晶]（上限1）」.
fn after_roll(player_id: i32) -> card_sdk::Asked {
    ctx::add_crystals(1, 1)?;
    ctx::log(player_id, &Msg::new(key!("mygo_crystal")));
    Ok(())
}

/// （3）「每次你的卡在未生效的情况下进入弃牌堆时，为此卡添加一个可超出上限的
/// [奇迹水晶]（最多超出2个）」.
fn on_discarded(player_id: i32) -> card_sdk::Asked {
    // 「上限1」 + 「最多超出2个」 -- held total never exceeds 3. Each ineffective
    // discard adds one uncapped crystal while under that ceiling. The `OVER`
    // counter records how many over-cap grants have landed (audit / display).
    let have = ctx::crystals();
    let max = 1 + 2;
    if have >= max {
        return Ok(());
    }
    // `OVER` = how far above the base cap of 1 the post-add count sits.
    state::set(player_id, OVER, have.min(2));
    ctx::add_crystals(1, i32::MAX)?;
    Ok(())
}

/// Which press effects are available right now, in rulebook order: the
/// (2) move-1 and the (3) draw-1. `ctx::is_placed` is true for the skill
/// press, so the instance's crystal pool is the one being spent.
fn available(player_id: i32) -> [bool; 2] {
    if bandori_std::ctx::skill_blocked(player_id, "") || !is_my_turn(player_id) {
        return [false, false];
    }
    let c = ctx::crystals();
    [c >= 1, c >= 2]
}

/// Combined gate: admit whenever either press effect is available. The (2)
/// move-1 costs 1 crystal and the (3) draw costs 2, so 1 crystal is already
/// enough for the move half.
fn cant_play(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    // 规则书（`PPP:Returns` [持续]（3））: 「使用因此卡获得的团卡的主动效果时需要
    // 支付1星星贴纸」 -- the sticker is a cost of activation on the borrowed copy.
    if let Some(why) = crate::copy_sticker_gate(player_id) {
        return Some(why);
    }
    if !is_my_turn(player_id) {
        return Some(Msg::new(key!("mygo_not_your_turn")));
    }
    if ctx::crystals() < 1 {
        return Some(Msg::new(key!("mygo_no_crystal")));
    }
    None
}

/// One press = one effect. The rulebook's （2） and （3） are two independent
/// 「可」 abilities (different windows and costs), so when both are available
/// the player picks which this press activates; a single available option is
/// taken without prompting. The player can press again for the other.
fn press(player_id: i32) -> card_sdk::Asked {
    let avail = available(player_id);
    let step = avail[0];
    let draw = avail[1];
    if !step && !draw {
        return Ok(());
    }
    if step && !draw {
        return step_one(player_id);
    }
    if draw && !step {
        return draw_two(player_id);
    }
    let k = ctx::ask_pick(
        player_id,
        &Msg::new(key!("mygo_press_title")),
        &Msg::new(key!("mygo_press_which")),
        &[
            Msg::new(key!("mygo_press_step")),
            Msg::new(key!("mygo_press_draw")),
        ],
    )?;
    if k == 0 {
        step_one(player_id)
    } else {
        draw_two(player_id)
    }
}

/// （2）「你的回合中，可于移动掷骰前选择移动1格以替代移动掷骰并移除一个[奇迹水晶]」.
fn step_one(player_id: i32) -> card_sdk::Asked {
    crate::spend_copy_sticker(player_id)?;
    if ctx::crystals() < 1 {
        return Ok(());
    }
    ctx::add_crystals(-1, i32::MAX)?;
    // 规则书（2）: 「移动1格以替代移动掷骰」 -- the shaped 1-step move replaces
    // the roll and runs now (`ctx::card_move`).
    plan::clear_dice();
    plan::set_steps(1);
    ctx::card_move(player_id);
    ctx::log(player_id, &Msg::new(key!("mygo_step")));
    Ok(())
}

/// （3）「你的回合中，可移除此卡的两个[奇迹水晶]以抽一张卡」.
fn draw_two(player_id: i32) -> card_sdk::Asked {
    crate::spend_copy_sticker(player_id)?;
    if ctx::crystals() < 2 {
        return Ok(());
    }
    ctx::add_crystals(-2, i32::MAX)?;
    ctx::draw(player_id, 1)?;
    ctx::log(player_id, &Msg::new(key!("mygo_draw")));
    Ok(())
}

fn is_my_turn(player_id: i32) -> bool {
    ctx::turn_player() == player_id
}

// （1）「取出目作为你本局游戏的起始点」 -- the opening places everyone on the
// board's own start, so the per-player start is a plain position write at the
// same moment (`teleport_to`).

/// Roster band row for this band skill (ABI v63).
pub const BAND: bandori_std::roster::GroupDef =
bandori_std::roster::GroupDef::new(r#"MyGO!!!!!"#)
    .short_name(r#"MyGO"#)
    .color(r#"#3388BB"#)
    .skill(r#"迷途之星"#)
    .text(r#"（1）开局时投掷3d20，并取出目作为你本局游戏的起始点 （2）若移动掷骰出目为16及以上，为此卡添加一个[奇迹水晶]（上限1）。你的回合中，可于移动掷骰前选择移动1格以替代移动掷骰并移除一个[奇迹水晶] （3）每次你的卡在未生效的情况下进入弃牌堆时，为此卡添加一个可超出上限的[奇迹水晶] （最多超出2个）；你的回合中，可移除此卡的两个[奇迹水晶]以抽一张卡。"#)
    .logo(r#"045"#);

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn at_start_kani(player_id: i32) -> card_sdk::Asked {
    at_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn after_roll_kani(player_id: i32) -> card_sdk::Asked {
    after_roll(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_discarded_kani(player_id: i32) -> card_sdk::Asked {
    on_discarded(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn press_kani(player_id: i32) -> card_sdk::Asked {
    press(player_id)
}
