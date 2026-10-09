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

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "skill:MyGO!!!!!:迷途之星";
/// 「可超出上限的[奇迹水晶]（最多超出2个）」 -- overflow allowance.
const OVER: &str = "skill.mygo.over";

pub const MYGO: CardDef = CardDef::new(
    "skill:MyGO!!!!!:迷途之星",
    &[
        // （1）「开局时投掷3d20，并取出目作为你本局游戏的起始点」 -- a start
        // position: the before-match-start point decides those.
        On::Hook(&[HookKind::DeckBeforeGame], "", None, at_start),
        On::Hook(&[HookKind::RollAfter], card_sdk::pre::MINE, None, after_roll),
        On::Hook(&[HookKind::Discarded], card_sdk::pre::MINE, None, on_discarded),
        // One Play entry for both press effects (the engine dispatches only
        // the first). 规则书（2）/（3） give two independent 「可」 abilities
        // (move-1 before the move roll for 1 crystal; draw 1 for 2 crystals)
        // rather than one press with two modes, so each press activates one of
        // them and the player presses again for the other. The gate admits
        // whenever either is available; the body lists the available options
        // and skips the prompt when only one is.
        On::Play("", Some(cant_play), press),
    ],
)
    .legacy(&[(1, legacy_mine), (2, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

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
    if ctx::trigger::move_roll().unwrap_or(0) < 16 {
        return Ok(());
    }
    ctx::add_crystals(1, 1)?;
    ctx::log(player_id, &Msg::new(key!("mygo_crystal")));
    Ok(())
}

/// （3）「每次你的卡在未生效的情况下进入弃牌堆时，为此卡添加一个可超出上限的
/// [奇迹水晶]（最多超出2个）」.
fn on_discarded(player_id: i32) -> card_sdk::Asked {
    let have = ctx::crystals();
    let room = 1 + state::get(player_id, OVER).min(2);
    if have >= room {
        return Ok(());
    }
    state::set(player_id, OVER, state::get(player_id, OVER) + 1);
    ctx::add_crystals(1, i32::MAX)?;
    Ok(())
}

/// Which press effects are available right now, in rulebook order: the
/// (2) move-1 and the (3) draw-1. `ctx::is_placed` is true for the skill
/// press, so the instance's crystal pool is the one being spent.
fn available(player_id: i32) -> [bool; 2] {
    if card_sdk::ctx::skill_blocked(player_id, "") || !is_my_turn(player_id) {
        return [false, false];
    }
    let c = ctx::crystals();
    [c >= 1, c >= 2]
}

/// Combined gate: admit whenever either press effect is available. The (2)
/// move-1 costs 1 crystal and the (3) draw costs 2, so 1 crystal is already
/// enough for the move half.
fn cant_play(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
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
