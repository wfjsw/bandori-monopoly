//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:无法将视线移开`）:
//! > 无法将视线移开：
//! >  使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制移动1~4以内的任意步数并[触发结算] （此卡可作为[反击]在有玩家对你使用[反击]后立即使用），若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动
//!
//! force-move the player who [反击]'d you this turn 1-4 tiles with settle.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, ChainKind};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const CANT_LOOK_AWAY: CardDef = CardDef::new(
    "Mujica:无法将视线移开",
    &[
        On::Play("", None, play),
        // 规则书: 「（此卡可作为[反击]在有玩家对你使用[反击]后立即使用）」 --
        // the timing is a counter used *against you*. The engine's counter link
        // carries `target` = the countered seat and `actor` = the counter's
        // declarer (see `build_round`), so `target == owner && actor != owner`
        // is exactly 「有玩家对你使用[反击]」. `Counteracted` is the named wire
        // kind; the counter-to-a-counter window also presents the link as the
        // counter's own `card` / `CardUsed` kind, so those two counter-link
        // selectors are declared as well.
        On::Counteract(
            &[ChainKind::Counteracted, ChainKind::Card, ActionKind::CardUsed.as_trigger()],
            "target == owner && actor != owner",
            None,
            counteract).counter_reason(key!("cant_look_away_when")),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"无法将视线移开"#)
        .raw_name(r#"无法将视线移开"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制移动1~4以内的任意步数并[触发结算] （此卡可作为[反击]在有玩家对你使用[反击]后立即使用），若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动"#))
        // sheet cell J7
        .fields(&[("group", r#"Ave Mujica"#), ("owner", "")]));

fn play(_player_id: i32) -> card_sdk::Asked {
    // 规则书: 「使用者」 is the caster, which a hand-play reaches through the
    // trigger (`player_id` is the owner). The hand path is no longer dead:
    // the history names the turn's counterers (design G06 §2.8).
    run(trigger::player_id())?;
    Ok(())
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // The counter itself is cast by the answering seat (`player_id`).
    run(player_id)?;
    Ok(())
}

/// Seats already moved by this play (bit per seat), so a re-run past a
/// suspended settle resumes instead of moving everyone again.
const DONE: &str = "cantLookDone";
/// The raise this progress belongs to; a new play has a new uid.
const UID: &str = "cantLookUid";

/// `caster` is the [使用者] the clause keys on.
fn run(caster: i32) -> card_sdk::Asked {
    // A body that suspends on a settle (the self pick's main move, a buy) is
    // restarted from the top. Keep what already moved for this raise and drop
    // it on a new one.
    // The raise's own id (`seq` is the chain position and 0 on every bare play
    // trigger, so two plays in a turn would share it).
    let uid = trigger::uid();
    if ctx::slot(caster, UID) != uid {
        ctx::set_slot(caster, UID, uid);
        ctx::set_slot(caster, DONE, 0);
    }
    let mut done = ctx::slot(caster, DONE);
    // 规则书: 「使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制
    // 移动1~4以内的任意步数」 -- the target set is the turn's counteraction
    // history (design G06 §5.6), not only the link this counter answers.
    let mut targets: Vec<i32> = ctx::countered_by_this_turn(caster);
    targets.retain(|&s| (0..31).contains(&s) && done & (1 << s) == 0);
    // 规则书: 「若使用者在自身回合内选择了使用者自己进行强制移动，则视为其
    // 主要移动」 -- the caster may join the set, on their own turn only.
    if ctx::turn_player() == caster
        && !targets.contains(&caster)
        && (caster >= 31 || done & (1 << caster) == 0)
    {
        targets.push(caster);
    }
    if targets.is_empty() {
        return Ok(());
    }
    while !targets.is_empty() {
        let who = ctx::ask_player(
            caster,
            &Msg::new(key!("cant_look_away_title")),
            &Msg::new(key!("cant_look_away_pick")),
            &targets,
        )?;
        targets.retain(|&s| s != who);
        if ctx::player_out(who) {
            continue;
        }
        let n = ctx::ask_number(
            caster,
            &Msg::new(key!("cant_look_away_title")),
            &Msg::new(key!("cant_look_away_steps")).player_id("who", who),
            1,
            4,
        )?;
        let forward = ctx::ask_choice(
            caster,
            &Msg::new(key!("cant_look_away_dir_title")),
            &Msg::new(key!("cant_look_away_dir_ask")),
            &[
                Msg::new(key!("cant_look_away_forward")),
                Msg::new(key!("cant_look_away_backward")),
            ],
        )? == 0;
        // 「强制移动」; the self pick is the main move (`set_main_override`).
        ctx::plan::set_forced(true);
        ctx::plan::set_main_override(who == caster);
        ctx::plan::set_steps(n);
        ctx::plan::set_reverse(!forward);
        ctx::plan::set_settles(true);
        ctx::log(
            caster,
            &Msg::new(key!("cant_look_away_ordered"))
                .player_id("who", who)
                .i("n", n as i64),
        );
        done |= 1 << who;
        ctx::set_slot(caster, DONE, done);
        ctx::run_planned_move(who);
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
