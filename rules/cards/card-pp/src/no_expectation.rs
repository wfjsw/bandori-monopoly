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
//! deck, draw 1. The [特] immunity and the whole [持续] block need hooks the
//! ABI lacks (TODOs below).

use card_sdk::{ctx, key, CardDef, Msg};

pub const NO_EXPECTATION: CardDef = CardDef {
    id: "PP:不要背负期待",
    play: Some(no_expectation),
    can_react: None,
    react: None,
    why_not: None,
};

fn no_expectation(seat: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "PP:不要背负期待", &Msg::new(key!("no_expectation_note")));
    // 规则书[手]: 「并将1张“共鸣”加入卡组」
    ctx::add_to_deck(seat, "PP:[衍生]共鸣", true);
    ctx::log(
        seat,
        &Msg::new(key!("no_expectation_added")).seat("who", seat).card("card", "PP:[衍生]共鸣"),
    );
    // 规则书[手]: 「然后抽1张牌」
    ctx::draw(seat, 1);
    // TODO(规则书): [特] 「此卡不受任何其他效果影响」 -- needs the C# `Card.Immune`
    // flag so other effects skip this field card.
    // TODO(规则书): [持续]（1）「非回合开始时进行投掷的投掷结果减少2（如果是移动投掷则最终结果
    // 最小为0，如果为0则此次移动不[结算]）；手卡上限数量减1」 -- needs the Fx.RollAfter
    // hook (C# `Card.RollAfter(MoveCtx)`) and Fx.HandLimitDelta
    // (C# `Card.HandLimitDelta`).
    // TODO(规则书): [持续]（2）「此卡放入[场地]或每次[拥有者]弃卡区洗入抽卡区时对[拥有者]生效2次：
    // [支付]资金时金额提高100；[收取]资金时金额减少100（最低0）」 -- needs the
    // Fx.Reshuffled hook (C# `Card.Reshuffled`) for the re-stack and the
    // Fx.PayAdd hook (C# `Card.PayAdd`) to bend payments both ways.
}