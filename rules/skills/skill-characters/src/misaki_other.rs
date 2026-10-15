//! `skill:奥泽美咲:另一个我`
//!
//! 规则书（skill sheet, 奥泽美咲）:
//! > （1）每次经过"弦卷集团"（#29）格子后获得2火罐（初始0，上限2），若你在传送时
//! > CiRCLE位于你正向移动的移动起点和移动终点之间，则你下次经过"弦卷集团"前可将
//! > "弦卷集团"格子额外视为CiRCLE。
//! > （2）你的回合开始时，你可以消耗一个火罐，使你的移动掷骰额外获得一个1d10，
//! > 且你的移动视为[传送]。
//! > （3）使用火罐进行移动掷骰后，触发结算前，你可以消耗1火罐，获得一层在你的下
//! > 回合开始前移除的[除外]。
//!
//! （1）'s second half is a *standing re-classification* of one tile: 「弦卷集团」
//! counts as CiRCLE for this player until it is next passed. That is a per-player
//! tile re-classification in spirit but the axis here is tile *kind*, not colour
//! -- and the vocabulary has a colour override, not a kind one. See the TODO below.
//!
//! （2） 「你的移动视为[传送]」 is the plan's kind; 「额外获得一个1d10」 rides on
//! the dice table.
//!
//! （3） 「获得一层在你的下回合开始前移除的[除外]」 is `give_exile` with
//! `expires: TurnStart`.

use card_sdk::abi::{state_key, HookKind};
use ruleset_core::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

const GROUP: &str = "skill.misakiOther.group";

pub const MISAKI_OTHER: CardDef = CardDef::new(
    "skill:奥泽美咲:另一个我",
    &[
        On::Play("", Some(can_use_exile), use_exile),
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Hook(&[HookKind::Pass], "actor == owner && tile.id == tile_named('弦卷集团')", None, on_pass),
        On::Hook(
            &[HookKind::TurnStartBefore],
            "actor == owner && fire(owner) >= 1",
            None,
            at_turn_start,
        ),
    ],
)
    .silent(&[1]);

/// 「初始0，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 0, 2);
    Ok(())
}

/// （1）「每次经过"弦卷集团"（#29）格子后获得2火罐」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    rules_status::fire::gain(player_id, 2, &Msg::new(key!("misaki_other_gain")))?;
    state::set(player_id, GROUP, 0);
    Ok(())
}

/// （2）「你的回合开始时，你可以消耗一个火罐，使你的移动掷骰额外获得一个1d10，
/// 且你的移动视为[传送]」.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    // `fire(owner) >= 1` is the pre (「你可以消耗一个火罐」).
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("misaki_other_title")),
        &Msg::new(key!("misaki_other_ask")),
    )? {
        return Ok(());
    }
    if !rules_status::fire::spend(player_id, 1, &Msg::new(key!("misaki_other_spend")))? {
        return Ok(());
    }
    plan::add_extra_dice(1, 10, "另一个我");
    plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    ctx::log(player_id, &Msg::new(key!("misaki_other_dice")));
    Ok(())
}

/// （3） 「你可以消耗1火罐」.
fn can_use_exile(player_id: i32) -> Option<Msg> {
    if ruleset_core::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("misaki_other_no_fire")));
    }
    None
}

/// （3）「获得一层在你的下回合开始前移除的[除外]」 -- the exile wears off at the
/// next turn start, which is what `expires: TurnStart` says.
fn use_exile(player_id: i32) -> card_sdk::Asked {
    if !rules_status::fire::spend(player_id, 1, &Msg::new(key!("misaki_other_spend")))? {
        return Ok(());
    }
    rules_status::exile::send(player_id, 1, -1);
    ctx::log(player_id, &Msg::new(key!("misaki_other_exile")));
    Ok(())
}

// `GROUP` is the latch for （1）'s 「下次经过"弦卷集团"前可将"弦卷集团"格子额外视为
// CiRCLE」. The vocabulary has a *colour* override but not a tile-*kind* one, so
// the re-classification cannot be expressed yet.
// TODO(规则书)[judgement]: （1）「下次经过"弦卷集团"前可将"弦卷集团"格子额外视为CiRCLE」 --
//   the clause under-specifies -- it needs a per-player tile-kind override (the
//   colour override is the wrong axis), and what 「额外视为CiRCLE」 pays on top
//   of the tile's own effect is not stated.
#[allow(dead_code)]
fn _group(player_id: i32) -> i32 {
    state::get(player_id, GROUP)
}

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: ruleset_core::roster::CharacterDef =
ruleset_core::roster::CharacterDef::new(r#"奥泽美咲"#)
    .display(r#"奥泽美咲"#)
    .band(r#"Hello, Happy World!"#)
    .color(r#"#006699"#)
    .skill(r#"另一个我"#)
    .text(r#"（1）每次经过“弦卷集团”（#29）格子后获得2火罐（初始0，上限2），若你在传送时CiRCLE位于你正向移动的移动起点和移动终点之间，则你下次经过“弦卷集团”前可将“弦卷集团”格子额外视为CiRCLE。（2）你的回合开始时，你可以消耗一个火罐，使你的移动掷骰额外获得一个1d10，且你的移动视为[传送]。（3）使用火罐进行移动掷骰后，触发结算前，你可以消耗1火罐，获得一层在你的下回合开始前移除的[除外]。"#)
    .cn_id(r#"015"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"（未命名）"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn use_exile_kani(player_id: i32) -> card_sdk::Asked {
    use_exile(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_start(player_id)
}
