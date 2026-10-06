//! `R:（纱夜）弹奏弹奏弹奏，继续弹奏` -- C# `CardSayoPlay` (MatchHost.cs:10746-10779).
//!
//! 规则书（docs/rulebook/cards.json, id `R:（纱夜）弹奏弹奏弹奏，继续弹奏`）:
//! > （纱夜）弹奏弹奏弹奏，继续弹奏：
//! > [反击] 时机合适时打出，打出时视为使用一次此卡使用者的技能。
//!
//! Counteraction on your own move roll. The C# realises 「视为使用一次此卡使用者的技能」
//! as H.AnnounceSkill plus SkillSayo's roll bump (+1 or +2, no fire cost); only
//! the bump is expressible here (see the TODOs).

use card_sdk::abi::{ChainKind, MoveKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const SAYO_PLAY: CardDef = CardDef::new(
    "R:（纱夜）弹奏弹奏弹奏，继续弹奏",
    &[On::Counteract(&[ChainKind::MoveRoll], can_counteract, counteract)],
);

/// 规则书: 「[反击] 时机合适时打出」 -- counteraction-only (C# `Normal => false`).
fn can_counteract(player_id: i32) -> bool {
    // 规则书: 「[反击] 时机合适时打出」 -- C# window is the player's own move roll.
    if trigger::kind() != TriggerKind::MoveRoll || trigger::player_id() != player_id {
        return false;
    }
    // C# `!t.Move.Teleport` -- a teleport roll is not 「时机合适」. (The old
    // `TeleportWalk` check folded in here: it is a teleport now.)
    if trigger::move_kind() != Some(MoveKind::Walk) {
        return false;
    }
    trigger::move_roll().is_some()
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 「打出时视为使用一次此卡使用者的技能」 -- the user's bound skill is the
    // `skill:<character>:<name>` field card `bind_skills` placed, so `play_card`
    // runs its `On::Play` entry and the `skillUsed` trigger fires with it.
    if let Some(skill) = ctx::placed_cards(player_id)
        .into_iter()
        .find(|c| c.starts_with("skill:"))
    {
        ctx::log(
            player_id,
            &Msg::new(key!("sayo_play_skill")).card("card", &skill),
        );
        ctx::play_card(&skill, player_id)?;
    }
    let Some(before) = trigger::move_roll() else {
        return Ok(());
    };
    let n = match ctx::ask_pick(
        player_id,
        &Msg::new(key!("sayo_play_ask_title")).card("card", "R:（纱夜）弹奏弹奏弹奏，继续弹奏"),
        &Msg::new(key!("sayo_play_ask_text")),
        &[
            Msg::new(key!("sayo_play_plus_one")),
            Msg::new(key!("sayo_play_plus_two")),
        ],
    )? {
        0 => 1,
        _ => 2,
    };
    // 规则书: 「打出时视为使用一次此卡使用者的技能」-- the C# body bumps the move.
    trigger::set_move_roll(before + n);
    ctx::log(
        player_id,
        &Msg::new(key!("sayo_play_applied"))
            .player_id("who", player_id)
            .card("card", "R:（纱夜）弹奏弹奏弹奏，继续弹奏")
            .i("n", n as i64)
            .i("total", (before + n) as i64),
    );
    Ok(())
}
