//! `Mor:（NNM）稍微努力了一下` -- C# `CardNanamiEffort` (MatchHost.cs:4805-4899): spend
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（NNM）稍微努力了一下`）:
//! > （NNM）稍微努力了一下：
//! > 弃置手中x枚角色标记，发动以下效果中的一个：
//!
//! > （1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张
//!
//! > （2）获得x次经过CiRCLE时的资金奖励
//!
//! > （3）将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的
//! > （2）技能，此次技能不受数量或轮数限制
//!
//! > （4）当奇迹水晶耗尽时，将此卡置入弃牌堆
//!
//! x character tokens for one of three effects.

use card_sdk::ctx::{self, CardPile};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "Mor:（NNM）稍微努力了一下";
/// The counter names 「角色标记:*」.
const TOKEN_PREFIX: &str = "角色标记:";

pub const NANAMI_EFFORT: CardDef = CardDef::new(
    "Mor:（NNM）稍微努力了一下",
    &[
        // One Play entry for both contexts (the engine dispatches only the
        // first): the hand body (spend x 角色标记, pick one of (1)/(2)/(3)),
        // or the placed (3) press (spend 1 crystal to fire the owner's (2)
        // skill). The gate admits whenever either branch is available.
        On::Play("", Some(cant_play), play),
        On::AtEnd("", None, discard_down_to_five),
    ],
);

/// Combined gate: from hand the x-标记 body needs at least one 「角色标记:*」;
/// once placed the (3) press needs a crystal and a bound owner skill.
fn cant_play(player_id: i32) -> Option<Msg> {
    if !ctx::is_placed() {
        let names = ctx::tok_names(player_id, TOKEN_PREFIX);
        let total: i32 = names.iter().map(|n| ctx::tok(player_id, n)).sum();
        if total > 0 {
            return None;
        }
        return Some(Msg::new(key!("nanami_effort_no_tok")));
    }
    if ctx::crystals() < 1 {
        return Some(Msg::new(key!("nanami_effort_no_crystal")));
    }
    if owner_skill(player_id).is_none() {
        return Some(Msg::new(key!("nanami_effort_no_skill")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    if ctx::is_placed() {
        return use_skill(player_id);
    }
    nanami_effort(player_id)
}

fn nanami_effort(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「弃置手中x枚角色标记，发动以下效果中的一个」 -- the tokens are the
    // `角色标记:*` counters; x is chosen from what the player actually holds, then
    // they are spent and one effect is picked.
    let names = ctx::tok_names(player_id, TOKEN_PREFIX);
    let total: i32 = names.iter().map(|n| ctx::tok(player_id, n)).sum();
    if total <= 0 {
        ctx::log(
            player_id,
            &Msg::new(key!("nanami_effort_no_tok")).player_id("who", player_id),
        );
        return Ok(());
    }
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("nanami_effort_how_many")),
        &Msg::new(key!("nanami_effort_how_many_text")),
        1,
        total,
    )?;
    // 「弃置…x枚」 -- spend them lowest-index first until x is gone.
    let mut left = x;
    for name in &names {
        if left <= 0 {
            break;
        }
        let take = ctx::tok(player_id, name).min(left);
        if take > 0 {
            ctx::add_tok(player_id, name, -take, i32::MAX)?;
            left -= take;
        }
    }
    // 「发动以下效果中的一个」
    let k = ctx::ask_pick(
        player_id,
        &Msg::new(key!("nanami_effort_pick_title")),
        &Msg::new(key!("nanami_effort_pick_text")),
        &[
            Msg::new(key!("nanami_effort_effect_1")),
            Msg::new(key!("nanami_effort_effect_2")),
            Msg::new(key!("nanami_effort_effect_3")),
        ],
    )?;
    match k {
        // (1) 「回合结束时抽x张卡（可超过上限）」 -- scheduled, runs in `effect_draw`.
        0 => effect_draw(player_id, x)?,
        // (2) Sheet 2026-10-06 新卡组卡 G7: 「获得x次经过CiRCLE时的资金奖励」
        // -- x times the CiRCLE pass *money* reward, not a flat x*2000. The
        // default reward is 2000; a modified one (Morfonica's 1000/1500/2000
        // cycle) should scale this too.
        // TODO(规则书): query the live CiRCLE money reward (it is rewritten by
        //   `circleAffected` listeners) instead of hard-coding 2000.
        1 => {
            let got = x * 2000;
            ctx::gain(
                player_id,
                got,
                &Msg::new(key!("nanami_effort_money")).i("n", got as i64),
            )?;
        }
        // (3) 「将此卡放置在场上并为其放置x个奇迹水晶」 -- placed with x crystals
        // on it; (4) is its exhaustion.
        2 => {
            ctx::set_dest(ctx::Dest::Field);
            ctx::place_card(player_id, ID, &Msg::new(key!("nanami_effort_note")));
            ctx::add_crystals(x, 0)?;
            ctx::log(
                player_id,
                &Msg::new(key!("nanami_effort_placed")).i("n", x as i64),
            );
        }
        _ => {}
    }
    // （3）「你可以移除一个奇迹水晶视为发动你的（2）技能，此次技能不受数量或轮数
    // 限制」 -- a press. The owner's skill is the `skill:<character>:<name>` field
    // card `bind_skills` placed, so `placed_cards` finds it and `play_card` runs
    // its `On::Play` entry.
    Ok(())
}

/// 规则书（4）: 「当奇迹水晶耗尽时，将此卡置入弃牌堆」 -- the exhaustion half of
/// effect (3). Nothing drains the crystals yet (that is the skill hook above),
/// but the branch is what runs when they hit 0.
#[allow(dead_code)]
fn exhausted(_player_id: i32) {
    ctx::set_dest(ctx::Dest::Graveyard);
}

/// Effect (1) 「抽x张卡（可超过上限），回合结束后将手牌弃置到五张」 -- C# case 0 of
/// the play's `H.AskPick` (`MatchHost.cs:4874-4878`). The discard-down half is
/// C# `H._turnCtx.AtEnd.Add(() => H.DiscardDownTo(i, 5, CardName))` =
/// `ctx::before_turn_end` (C# `AtEnd`, pre-wear-off) queuing the `On::AtEnd`
/// body below. Unreachable until the token-list query lands (the play folds
/// first); written now so the queued scheduling is already the right hook.
#[allow(dead_code)]
fn effect_draw(player_id: i32, x: i32) -> card_sdk::Asked {
    // 规则书（1）: 「抽x张卡（可超过上限）」 -- 「可超过上限」 is the hand limit
    // itself, so lift it for the draw and put it back (the limit is keyed state,
    // which is what `NoLimitFx` was an attachment for).
    let cap = ctx::state::get(player_id, card_sdk::abi::state_key::HAND_LIMIT);
    ctx::state::set(player_id, card_sdk::abi::state_key::HAND_LIMIT, i32::MAX);
    ctx::draw(player_id, x)?;
    ctx::state::set(player_id, card_sdk::abi::state_key::HAND_LIMIT, cap);
    // 规则书（1）: 「回合结束后将手牌弃置到五张」 -- C#
    // `H._turnCtx.AtEnd.Add(() => H.DiscardDownTo(i, 5, CardName))`.
    ctx::before_turn_end(player_id);
    Ok(())
}

/// 规则书（1）: 「回合结束后将手牌弃置到五张」 -- C# `H._turnCtx.AtEnd.Add(() =>
/// H.DiscardDownTo(i, 5, CardName))`. Scheduled by the play's effect (1) via
/// `ctx::before_turn_end(player_id)`.
fn discard_down_to_five(player_id: i32) -> card_sdk::Asked {
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    let mut extra = hand.len() as i32 - 5;
    for card in &hand {
        if extra <= 0 {
            break;
        }
        if ctx::discard_from_hand(player_id, card) {
            extra -= 1;
        }
    }
    if extra < 0 {
        return Ok(());
    }
    ctx::log(
        player_id,
        &Msg::new(key!("nanami_effort_discard_down")).player_id("who", player_id),
    );
    Ok(())
}

/// （3）「移除一个奇迹水晶视为发动你的（2）技能」 -- the placed-press branch.
fn owner_skill(player_id: i32) -> Option<alloc::string::String> {
    ctx::placed_cards(player_id)
        .into_iter()
        .find(|c| c.starts_with("skill:"))
}

fn use_skill(player_id: i32) -> card_sdk::Asked {
    if ctx::crystals() < 1 {
        return Ok(());
    }
    let Some(id) = owner_skill(player_id) else {
        return Ok(());
    };
    ctx::add_crystals(-1, 0)?;
    // 「此次技能不受数量或轮数限制」 -- the press is a plain `play_card`, so the
    // skill's own gate is what runs; nothing here counts a use.
    ctx::log(
        player_id,
        &Msg::new(key!("nanami_effort_skill")).card("card", &id),
    );
    ctx::play_card(&id, player_id)?;
    Ok(())
}
