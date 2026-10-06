//! `skill:CiRCLE:后勤人员的努力`
//!
//! 规则书（band sheet, CiRCLE）:
//! > （1）只能在卡组中加入"通用"卡（2）在你打出的"通用"卡生效时，可额外弃一张牌，
//! > 将打出的那张卡[手]效果中的一个数字变为两倍（除将牌加入卡组的效果以外）。
//!
//! （2） is the press: it rides the play of a 「通用」 card, so it is offered as
//! that card's effect runs. 「[手]效果中的一个数字」 is `ctx::n(k, value)` -- the
//! play's own numbers are already tagged for a doubling to target.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, CardPile};
use card_sdk::{key, CardDef, Msg, On};

pub const CIRCLE_STAFF: CardDef = CardDef::new(
    "skill:CiRCLE:后勤人员的努力",
    &[On::Hook(&[HookKind::CardPlayed], mine, on_played)],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （2）「在你打出的"通用"卡生效时，可额外弃一张牌，将打出的那张卡[手]效果中的
/// 一个数字变为两倍」.
fn on_played(player_id: i32) -> card_sdk::Asked {
    let Some(id) = ctx::trigger::cards().into_iter().next() else {
        return Ok(());
    };
    // 「"通用"卡」 -- the general pool, by the card's own prefix.
    if !id.starts_with("G:") && !id.starts_with("通用") {
        return Ok(());
    }
    if ctx::hand_size(player_id) < 1 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("circle_staff_title")),
        &Msg::new(key!("circle_staff_ask")).card("card", &id),
    )? {
        return Ok(());
    }
    // 「可额外弃一张牌」
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    let Some(dump) = hand.into_iter().next() else {
        return Ok(());
    };
    ctx::discard_from_hand(player_id, &dump);
    // 「将打出的那张卡[手]效果中的一个数字变为两倍」 -- `ctx::n(k, value)` reads
    // the doubling from the play context; `k` is the number's index in the card's
    // own text, so the player names which one.
    let k = ctx::ask_number(
        player_id,
        &Msg::new(key!("circle_staff_title")),
        &Msg::new(key!("circle_staff_which")),
        1,
        9,
    )?;
    ctx::set_play_doubled(k);
    ctx::log(
        player_id,
        &Msg::new(key!("circle_staff_doubled"))
            .card("card", &id)
            .card("dump", &dump),
    );
    Ok(())
}

// （1）「只能在卡组中加入"通用"卡」 -- a deck-building restriction. There is no
// deck list to edit and no per-card 「通用」 flag to filter on, so nothing here
// can enforce it.
// TODO(规则书)（1）: 「只能在卡组中加入"通用"卡」 -- a deck-construction rule with
//   no vocabulary: the engine has no deck list and no 「通用」 tag on a card to
//   filter a build step on.
//
// TODO(规则书)（2）: 「（除将牌加入卡组的效果以外）」 -- a per-number carve-out the
//   cards do not tag, so the player can name a number the clause excludes.
