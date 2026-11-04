//!
//! 规则书（docs/rulebook/cards.json, id `PPP:[衍生]拍卖撤下来了`）:
//! > [衍生]拍卖撤下来了：
//! > [特]：
//! > 此卡加入手牌时将此卡放置在自身场上，每回合开始时将自身场上的“仓库里的Random Star”[移除]（如果有，然后失去540资金）
//! > [持续]：此卡拥有者火罐上限加1且不受任何其他效果影响。
//!
//! auto-placed when it enters the hand; on the owner's own turn start it eats a
//! placed 仓库里的Random Star, and it raises the owner's fire-pot cap.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, CardPile, TriggerKind};
use card_sdk::{key, Action, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

const ID: &str = "PPP:[衍生]拍卖撤下来了";
const STAR: &str = "PPP:仓库里的Random Star";

pub const AUCTION_PULLED: CardDef = CardDef::new(
    "PPP:[衍生]拍卖撤下来了",
    // 规则书: 「此卡加入手牌时」 -- a draw (`Drawn`) and any uncounted hand
    // arrival (`HandChanged`, `counted == false`) both place it.
    &[On::Action(Action::Drawn("", None, hook)),
    On::Action(Action::HandChanged("", None, hook)),
    On::Action(Action::TurnStart("", None, hook))],
).props(&[(card_sdk::abi::prop::EST_COST, 540), (card_sdk::abi::prop::IMMUNE, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"拍卖撤下来了"#)
        .raw_name(r#"[衍生]拍卖撤下来了"#)
        .derived(true)
        .tags(&[r#"持续"#, r#"特"#, r#"衍生"#])
        .text(CardText::of(&[
            sec(r#"[特]："#, &[
                para(r#"此卡加入手牌时将此卡放置在自身场上，每回合开始时将自身场上的“仓库里的Random Star”[移除]（如果有，然后失去540资金）"#),
            ]),
            sec(r#"[持续]："#, &[
                para(r#"此卡拥有者火罐上限加1且不受任何其他效果影响。"#),
            ]),
        ]))
        // sheet cell B13
        .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]));

fn hook(player_id: i32) -> card_sdk::Asked {
    match trigger::kind() {
        TriggerKind::Drawn | TriggerKind::HandChanged => {
            if !trigger::card_is(ID) {
                return Ok(());
            }
            // 规则书: 「此卡加入手牌时将此卡放置在自身场上」 -- an uncounted
            // arrival only (`counted` arrivals are the draw routine's `drawn`).
            if trigger::kind() == TriggerKind::HandChanged
                && trigger::zone().map_or(true, |z| z.counted)
            {
                return Ok(());
            }
            if !ctx::take_card(player_id, ID, CardPile::Hand) {
                return Ok(());
            }
            ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Effect);
            ctx::place_card(player_id, ID, &Msg::new(key!("auction_pulled_note")));
            // TODO(ABI)（此卡拥有者火罐上限加1）: no leave-field hook point -- the cap is written on the
            // seat row and never reverted when this card leaves play.
            bandori_std::fire::add_max(player_id, 1);
            ctx::log(
                player_id,
                &Msg::new(key!("auction_pulled_placed")).player_id("who", player_id),
            );
            ctx::set_self_immune(true);
        }
        TriggerKind::TurnStart => {
            // TODO(规则书)（每回合开始时）:
            if trigger::player_id() != player_id || !ctx::is_placed() {
                return Ok(());
            }
            // 规则书[特]: 「每回合开始时将自身场上的“仓库里的Random Star”[移除]
            //   （如果有，然后失去540资金）」
            let field = ctx::cards_in(player_id, CardPile::Field);
            if !field.iter().any(|c| c == STAR) {
                return Ok(());
            }
            // `take_card(Field, ..)` drops it without sending it anywhere.
            if ctx::take_card(player_id, STAR, CardPile::Field) {
                ctx::log(
                    player_id,
                    &Msg::new(key!("auction_pulled_removed")).player_id("who", player_id),
                );
                ctx::pay(player_id, 540, &Msg::new(key!("auction_pulled_paid")))?;
            }
        }
        _ => {}
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn hook_kani(player_id: i32) -> card_sdk::Asked {
    hook(player_id)
}
