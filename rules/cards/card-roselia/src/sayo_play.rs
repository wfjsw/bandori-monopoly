//! `R:（纱夜）弹奏弹奏弹奏，继续弹奏` -- C# `CardSayoPlay` (MatchHost.cs:10746-10779).
//!
//! 规则书（docs/rulebook/cards.json, id `R:（纱夜）弹奏弹奏弹奏，继续弹奏`）:
//! > （纱夜）弹奏弹奏弹奏，继续弹奏：
//! > [反击] 时机合适时打出，打出时视为使用一次此卡使用者的技能。
//!
//! Counteraction after your non-teleport main distance is determined, before
//! moving (same timing as Sayo's skill; the printed card text is unchanged).
//! The C# realises 「视为使用一次此卡使用者的技能」
//! as H.AnnounceSkill plus SkillSayo's extension (+1 or +2, no fire cost).

use card_sdk::abi::{prop, ChainKind, MoveKind, TriggerKind};
use card_sdk::ctx::{self, plan, state, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const SAYO_PLAY: CardDef = CardDef::new(
    "R:（纱夜）弹奏弹奏弹奏，继续弹奏",
    &[
        On::Counteract(
            &[ChainKind::MoveBefore],
            "actor == owner && move.kind == Walk",
            Some(legacy_can_counteract),
            counteract,
        ),
        On::Play("", Some(counter_only), no_play),
    ],
)
.props(&[(prop::COUNTERACT_GROUP, 1)])
.legacy(&[(0, legacy_can_counteract)]);

fn counter_only(_player_id: i32) -> Option<Msg> {
    Some(Msg::new(key!("sayo_play_timing")))
}

fn no_play(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

/// 规则书: 「[反击] 时机合适时打出」 -- counteraction-only (C# `Normal => false`).
fn legacy_can_counteract(player_id: i32) -> bool {
    if trigger::kind() != TriggerKind::MoveBefore || trigger::player_id() != player_id {
        return false;
    }
    // C# `!t.Move.Teleport` -- a teleport roll is not 「时机合适」. (The old
    // `TeleportWalk` check folded in here: it is a teleport now.)
    if trigger::move_kind() != Some(MoveKind::Walk) {
        return false;
    }
    trigger::move_is_main()
        && state::get(player_id, "skill.sayoThorns.used") != ctx::turn_key()
        && !ctx::skill_blocked(player_id, "")
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // Announce the user's character skill. Running its press body again would
    // add a second paid skill offer before this card's free extension.
    if let Some(skill) = ctx::character_skill(player_id) {
        ctx::log(
            player_id,
            &Msg::new(key!("sayo_play_skill")).card("card", &skill),
        );
    }
    let before = trigger::value().max(0);
    let extra = (trigger::move_total() - trigger::value().max(0)).max(0);
    let total = before + extra;
    let landing =
        |steps: i32| (trigger::tile() + steps * trigger::move_dir()).rem_euclid(ctx::tile_count());
    let n = match ctx::ask_pick(
        player_id,
        &Msg::new(key!("sayo_play_ask_title")).card("card", "R:（纱夜）弹奏弹奏弹奏，继续弹奏"),
        &Msg::new(key!("sayo_play_ask_text"))
            .i("n", total as i64)
            .tile("tile", landing(total)),
        &[
            Msg::new(key!("sayo_play_plus_one")).tile("tile", landing(total + 1)),
            Msg::new(key!("sayo_play_plus_two")).tile("tile", landing(total + 2)),
        ],
    )? {
        0 => 1,
        _ => 2,
    };
    // 规则书: 「打出时视为使用一次此卡使用者的技能」-- the C# body bumps the move.
    state::set(player_id, "skill.sayoThorns.used", ctx::turn_key());
    plan::set_steps(before + n);
    ctx::log(
        player_id,
        &Msg::new(key!("sayo_play_applied"))
            .player_id("who", player_id)
            .card("card", "R:（纱夜）弹奏弹奏弹奏，继续弹奏")
            .i("n", n as i64)
            .i("total", (total + n) as i64),
    );
    Ok(())
}
