//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:羽丘的不可思议女孩`）:
//! > 羽丘的不可思议女孩：
//! >  位于属于自己的格子上时，投掷1d20，若出目至少为10则在当前格子免费加盖一层房屋，若出目至少为15，则额外抽一张卡，至少为20，则将此卡放置在自己场上，在后续任何时刻可将其置入弃牌堆并抵消一次任意付款。若小于10，此卡放入弃牌堆且视为此卡未生效
//!
//! Sheet 2026-10-06: every threshold is inclusive (「至少为」), and the
//! ineffective branch is 「小于10」. So 10 builds, 15 draws, 20 places.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{roll_source, ActionKind, RollPurpose};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Msg, On};

pub const HANEOKA: CardDef = CardDef::new(
    "MyGO:羽丘的不可思议女孩",
    &[
        On::Play("", Some(cant_play), haneoka),
        //
        // Rides `PaySplitChooseBefore` (v78), restoring the rulebook PaySplitChoose position:
        // the interactive `ask_yes` + `set_dest` + `set_pay_amount(0)` offer sits
        // immediately before the pure `PaySplitChoose` filter stage.
        On::Action(Action::PaySplitChooseBefore("actor == owner && card.placed && value > 0", None, pay_choose)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"羽丘的不可思议女孩"#)
        .raw_name(r#"羽丘的不可思议女孩"#)
        .tags(&[])
        .text(CardText::para(r#"位于属于自己的格子上时，投掷1d20，若出目至少为10则在当前格子免费加盖一层房屋，若出目至少为15，则额外抽一张卡，至少为20，则将此卡放置在自己场上，在后续任何时刻可将其置入弃牌堆并抵消一次任意付款。若小于10，此卡放入弃牌堆且视为此卡未生效"#))
        // sheet cell I7
        .fields(&[("group", r#"MyGO!!!!!"#), ("owner", "")]));

const ID: &str = "MyGO:羽丘的不可思议女孩";

fn cant_play(player_id: i32) -> Option<Msg> {
    let pos = ctx::player_pos(player_id);
    if ctx::tile_owner(pos) != player_id {
        return Some(Msg::new(key!("haneoka_blocked_own")));
    }
    None
}

fn haneoka(player_id: i32) -> card_sdk::Asked {
    let pos = ctx::player_pos(player_id);
    // 规则书: 「位于属于自己的格子上时」
    if ctx::tile_owner(pos) != player_id {
        return Ok(());
    }
    // It also honours a forced extreme (「以理论最大值或最小值结算」).
    let r = ctx::roll(player_id, 1, 20, RollPurpose::Effect, roll_source::CARD)?;
    if r < 10 {
        // 规则书 (sheet 2026-10-06): 「若小于10，此卡放入弃牌堆且视为此卡未生效」
        ctx::log(
            player_id,
            &Msg::new(key!("haneoka_no_effect"))
                .card("card", ID)
                .i("roll", r as i64),
        );
        // TODO(规则书)[judgement]: 「视为此卡未生效」 -- the clause names a state without saying
        // what observes it. And what counts a use that this would suppress?
        return Ok(());
    }
    if r >= 10 {
        // 规则书 (sheet 2026-10-06): 「若出目至少为10则在当前格子免费加盖一层房屋」
        // The build gate is the engine's own `WhyNotBuildOn` (`ctx::can_build_on`:
        // own + buyable, and no NO_BUILD / NO_BUILD_ABOVE / mortgaged / house-cap
        // veto). Only an actual layer rise is announced -- the count is clamped,
        // so a tile that cannot take another house leaves it unchanged.
        let before = ctx::houses_of(pos);
        let after = if ctx::can_build_on(player_id, pos) {
            ctx::add_house(pos, 1)
        } else {
            before
        };
        if after > before {
            ctx::log(
                player_id,
                &Msg::new(key!("haneoka_house"))
                    .player_id("who", player_id)
                    .tile("tile", pos)
                    .i("n", after as i64)
                    .card("card", ID),
            );
        } else {
            ctx::log(
                player_id,
                &Msg::new(key!("haneoka_no_house"))
                    .tile("tile", pos)
                    .card("card", ID),
            );
        }
    }
    if r >= 15 {
        // 规则书 (sheet 2026-10-06): 「若出目至少为15，则额外抽一张卡」
        ctx::draw(player_id, 1)?;
    }
    if r >= 20 {
        // 规则书 (sheet 2026-10-06): 「至少为20，则将此卡放置在自己场上」;
        // the PaySplitChoose hook below offers 「在后续任何时刻可将其置入弃牌堆并
        // 抵消一次任意付款」.
        ctx::set_dest(ctx::Dest::Field);
        ctx::place_card(
            player_id,
            ID,
            &Msg::new(key!("haneoka_placed"))
                .card("card", ID)
                .player_id("who", player_id),
        );
    }
    Ok(())
}

fn pay_choose(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value();
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("haneoka_cancel_title")).card("card", ID),
        &Msg::new(key!("haneoka_cancel_ask"))
            .n("money", amount as i64)
            .card("card", ID),
    )? {
        return Ok(());
    }
    if !ctx::is_placed() {
        return Ok(());
    }
    trigger::set_pay_amount(0);
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("haneoka_cancelled"))
            .card("card", ID)
            .n("money", amount as i64),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn haneoka_kani(player_id: i32) -> card_sdk::Asked {
    haneoka(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_choose_kani(player_id: i32) -> card_sdk::Asked {
    pay_choose(player_id)
}
