//! `PP:不要背负期待` -- C# `CardNoExpectation`: stay in play, 「共鸣」 joins the
//!
//! 规则书（docs/rulebook/cards.json, id `PP:不要背负期待`）:
//! > 不要背负期待：
//! > [特]：
//! > 此卡不受任何其他效果影响。
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]并将1张“共鸣”加入卡组，然后抽1张牌。
//! > [持续]：
//! >
//! > （1）以下效果对[拥有者]持续生效：非回合开始时进行投掷的投掷结果减少2（如果是移动投掷则最终结果最小为0，如果为0则此次移动不[结算]）；手卡上限数量减1。
//! >
//! > （2）以下效果此卡放入[场地]或每次[拥有者]弃卡区洗入抽卡区时对[拥有者]生效2次：[支付]资金时金额提高100；[收取]资金时金额减少100（最低0）。
//!
//! deck, draw 1. The roll shave and the pay bends live in the Fx hooks;
//! the reshuffle stack growth runs on `Reshuffled`.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const NO_EXPECTATION: CardDef = CardDef::new("PP:不要背负期待", &[
    On::Play(no_expectation),
    On::Hook(&[TriggerKind::RollAfter, TriggerKind::PayAdd], hook),
    On::Hook(&[TriggerKind::Reshuffled], reshuffled),
]);

/// C# `Mem["stacks"]` -- how many times the pay bend is stacked (2 on place,
/// +2 per reshuffle).
const SLOT_STACKS: &str = "no_expectation_stacks";

fn stacks(player_id: i32) -> i32 {
    ctx::slot(player_id, SLOT_STACKS)
}

fn no_expectation(player_id: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PP:不要背负期待", &Msg::new(key!("no_expectation_note")));
    // 规则书[持续]（2）: 「此卡放入[场地]……对[拥有者]生效2次」 -- C#
    // `H.PlaceFromPlay(c).Mem["stacks"] = 2`.
    ctx::set_slot(player_id, SLOT_STACKS, 2);
    // 规则书[手]: 「并将1张“共鸣”加入卡组」
    ctx::add_to_deck(player_id, "PP:[衍生]共鸣", true);
    ctx::log(
        player_id,
        &Msg::new(key!("no_expectation_added")).player_id("who", player_id).card("card", "PP:[衍生]共鸣"),
    );
    // 规则书[手]: 「然后抽1张牌」
    ctx::draw(player_id, 1);
    // TODO(规则书): [特] 「此卡不受任何其他效果影响」 -- needs the C# `Card.Immune`
    // flag so other effects skip this field card.
}

/// C# `CardNoExpectation.RollAfter` / `PayAdd` -- both [持续] halves that the
/// hook surface can express.
fn hook(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    match trigger::kind() {
        // 规则书[持续]（1）: 「非回合开始时进行投掷的投掷结果减少2」 -- C#
        // `CardNoExpectation.RollAfter` (`m.Roll = max(0, m.Roll - 2)`).
        // On a `rollAfter` trigger the roll is `value()`; `set_move_roll` is the
        // write-back the engine lands on `m.Roll`.
        TriggerKind::RollAfter => {
            if trigger::player_id() != player_id {
                return;
            }
            let roll = trigger::move_roll().unwrap_or(trigger::value());
            if roll <= 0 {
                return;
            }
            let cut = (roll - 2).max(0);
            trigger::set_move_roll(cut);
            ctx::log(
                player_id,
                &Msg::new(key!("no_expectation_roll")).i("n", cut as i64),
            );
            // 规则书[持续]（1）: 「如果为0则此次移动不[结算]」 -- C# `m.Resolve = false`.
            if cut == 0 {
                ctx::plan::set_resolve(false);
                ctx::log(
                    player_id,
                    &Msg::new(key!("no_expectation_no_resolve")).player_id("who", player_id),
                );
            }
            // The hand-limit half still needs Fx.HandLimitDelta (C#
            // `Card.HandLimitDelta` returning -1).
        }
        // 规则书[持续]（2）: 「[支付]资金时金额提高100；[收取]资金时金额减少100（最低0）」
        // -- C# `CardNoExpectation.PayAdd`, once per stack.
        TriggerKind::PayAdd => {
            let n = stacks(player_id);
            if n <= 0 {
                return;
            }
            let from = trigger::player_id();
            let to = trigger::target();
            let amount = trigger::value();
            if amount <= 0 {
                return;
            }
            let bump = 100 * n;
            if from == player_id && to >= 0 && to != player_id {
                // paying someone else: amount up
                trigger::set_pay_amount(amount + bump);
            } else if to == player_id && from >= 0 && from != player_id {
                // being paid by someone else: amount down
                trigger::set_pay_amount((amount - bump).max(0));
            }
        }
        _ => {}
    }
}

/// C# `CardNoExpectation.Reshuffled` -- each time the owner's discard pile is
/// shuffled back into the draw pile, the pay/gain bend gains 2 more stacks.
/// 规则书[持续]（2）: 「每次[拥有者]弃卡区洗入抽卡区时对[拥有者]生效2次」
fn reshuffled(player_id: i32) {
    if !ctx::is_placed(player_id) || trigger::player_id() != player_id {
        return;
    }
    ctx::inc_slot(player_id, SLOT_STACKS, 2);
}