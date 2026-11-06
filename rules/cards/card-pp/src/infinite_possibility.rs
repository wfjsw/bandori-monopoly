//!
//! 规则书（docs/rulebook/cards.json, id `PP:[大和麻弥]可能性为∞`）:
//! > [大和麻弥]可能性为∞：
//! > [特]：
//! > 观看卡组并观看到此卡时将此卡展示给所有玩家并将其放置在自己[场上]（次效果优先于其他后续效果，比如观看卡组后将一张牌加入手牌）。
//! > [持续]：
//!
//! > （1）[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4则为此卡添加1个[奇迹水晶]，否则移除此卡的[奇迹水晶]并为[拥有者]的Pastel✽Palettes乐队卡添加1个[奇迹水晶]。
//!
//! > （2）[共鸣]交换[拥有者]的抽卡区和弃卡区。
//!
//! The `On::Play` below is the [持续]（2） press, refused while the card is not
//! placed (`can_swap`); the crystal growth runs on the `Drew` hook.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ActionKind;
use card_sdk::{it, items, para, sec, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const INFINITE_POSSIBILITY: CardDef = CardDef::new(
    "PP:[大和麻弥]可能性为∞",
    &[
        // 规则书[持续]（2）: 「[共鸣]交换[拥有者]的抽卡区和弃卡区」 -- the one press.
        On::Play("", Some(can_swap), swap),
        // 规则书[特]: 「观看卡组并观看到此卡时…」 -- the turned card itself, in
        // whatever look (own or an opponent's, private or public).
        On::Action(Action::Inspected("", None, seen)),
        // 规则书[持续]（1）: 「[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4」 --
        // 「抽卡动作」 is the instruction, so the point is the batch's end.
        On::Action(Action::DrawAfter(
            "actor == owner && card.placed && value > 0",
            None,
            draw_after,
        )),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"可能性为∞"#)
        .raw_name(r#"[大和麻弥]可能性为∞"#)
        .tags(&[r#"持续"#, r#"特"#])
        .text(CardText::of(&[
            sec(r#"[特]："#, &[
                para(r#"观看卡组并观看到此卡时将此卡展示给所有玩家并将其放置在自己[场上]（次效果优先于其他后续效果，比如观看卡组后将一张牌加入手牌）。"#),
            ]),
            sec(r#"[持续]："#, &[
                items(&[
                    it(r#"[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4则为此卡添加1个[奇迹水晶]，否则移除此卡的[奇迹水晶]并为[拥有者]的Pastel✽Palettes乐队卡添加1个[奇迹水晶]。"#),
                    it(r#"[共鸣]交换[拥有者]的抽卡区和弃卡区。"#),
                ]),
            ]),
        ]))
        // sheet cell D17
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", r#"大和麻弥"#)]));

/// 规则书[特]: 「观看卡组并观看到此卡时将此卡展示给所有玩家并将其放置在自己[场上]
/// （次效果优先于其他后续效果，比如观看卡组后将一张牌加入手牌）」 -- the turn-up
/// is what fires this, so the placement happens before the look continues: the
/// card leaves the frame (it is no longer kingside the deck) and is shown on
/// the way.
pub fn seen(player_id: i32) -> card_sdk::Asked {
    let Some(id) = ctx::trigger::cards().first().copied() else {
        return Ok(());
    };
    ctx::reveal_card(
        player_id,
        id.as_str(),
        &Msg::new(key!("infinite_possibility_seen")),
    );
    if ctx::take_card(player_id, id.as_str(), ctx::CardPile::Inspection) {
        ctx::place_card(
            player_id,
            id.as_str(),
            &Msg::new(key!("infinite_possibility_note")),
        );
    }
    Ok(())
}

/// 规则书[持续]（1）: 「[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4则为此卡
/// 添加1个[奇迹水晶]，否则移除此卡的[奇迹水晶]并为[拥有者]的Pastel✽Palettes乐队卡添加1个 [奇迹水晶]」
fn draw_after(player_id: i32) -> card_sdk::Asked {
    if ctx::crystals() < 4 {
        ctx::add_crystals(1, 4)?;
    } else {
        let c = ctx::crystals();
        ctx::add_crystals(-c, 0)?;
        ctx::add_group_crystals(player_id, 1, i32::MAX);
        ctx::log(
            player_id,
            &Msg::new(key!("infinite_possibility_full")).player_id("who", player_id),
        );
    }
    Ok(())
}

// 规则书[持续]（2）: 「[共鸣]交换[拥有者]的抽卡区和弃卡区」 -- a press. The
// resonance cost is the same `try_resonance` every other [共鸣] card uses; the
// swap takes each card out of its pile before filing it into the other one.

/// 规则书[持续]（2）: 「[共鸣]交换[拥有者]的抽卡区和弃卡区」.
fn can_swap(player_id: i32) -> Option<Msg> {
    if !ctx::is_placed() {
        return Some(Msg::new(key!("infinite_possibility_not_placed")));
    }
    None
}

pub(crate) fn swap(player_id: i32) -> card_sdk::Asked {
    if !crate::resonance::try_resonance(player_id)? {
        return Ok(());
    }
    let deck = ctx::cards_in(player_id, ctx::CardPile::Deck);
    let disc = ctx::cards_in(player_id, ctx::CardPile::Graveyard);
    // An exchange, not a copy: each card leaves its pile first. The discard is
    // emptied into the deck before the deck is emptied, so the draw pile never
    // runs dry mid-swap (an empty draw pile reshuffles the discard back in).
    // The deck half files with the create-only `add_to_dest(Graveyard)` --
    // `to_discard` would first consume a same-named hand copy.
    for c in &disc {
        ctx::take_card(player_id, c.as_str(), ctx::CardPile::Graveyard);
    }
    for c in &disc {
        ctx::add_to_dest(player_id, c.as_str(), ctx::Dest::DeckTop, false);
    }
    for c in &deck {
        ctx::take_card(player_id, c.as_str(), ctx::CardPile::Deck);
    }
    for c in &deck {
        ctx::add_to_dest(player_id, c.as_str(), ctx::Dest::Graveyard, false);
    }
    ctx::log(player_id, &Msg::new(key!("infinite_possibility_swapped")));
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn swap_kani(player_id: i32) -> card_sdk::Asked {
    swap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn draw_after_kani(player_id: i32) -> card_sdk::Asked {
    draw_after(player_id)
}
