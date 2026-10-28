//! `PP:[丸山彩]憧憬的前方` -- C# `CardAyaLonging` (MatchHost.cs:7995-8063):
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[丸山彩]憧憬的前方`）:
//! > [丸山彩]憧憬的前方：
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。
//! > [持续]：
//!
//! > （1）如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]或[支付]时将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。
//!
//! > （2）[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）。
//!
//! stay in play, take a card back from the discard, and shave the owner's
//! payments -- X while poorest, 1500 on resonance.
//! The discard pick runs in `Play`; the [持续]（1） shave is a `PaySplitAdd`
//! amount filter and (2) a `PaySplitMulBefore` action.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, FilterKind};
use card_sdk::ctx::NameId;
use card_sdk::{it, items, para, sec, CardText};
use card_sdk::{key, Action, CardDef, Filter, Msg, On, PayAddValue};
use skill_groups::fans;

pub const AYA_LONGING: CardDef = CardDef::new(
    "PP:[丸山彩]憧憬的前方",
    &[
        On::Play("", None, aya_longing),
        // 规则书[持续]（1）: the passive X shave is a pure amount modifier, so it
        // rides `PaySplitAdd` (支付阶段 2) where it belongs and composes with
        // sibling add modifiers. No prompt, so no purity problem.
        On::Filter(Filter::PaySplitAdd("card.placed", None, shave_x_filter)),
        // 规则书[持续]（2）: 「[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）」 -- the
        // 共鸣 cost makes this half interactive (`try_resonance` prompts), so it
        // cannot be a pure Filter. It rides `Action::PaySplitMulBefore` (v96),
        // the writable stage between the `PaySplitAdd` fold and the `PaySplitMul`
        // fold (规则书 支付阶段 3 「支付计算后」). Sibling Add modifiers no longer
        // see the 1500 already gone; the shave lands on the composed Add figure.
        // (1)'s pure `PaySplitAdd` filter stays above it -- two subtractions
        // under a local floor are order-independent, so the X-then-1500 reading
        // is preserved.
        //
        // Reading pinned here: the 1500 shave runs **after** the whole
        // `PaySplitAdd` fold (base + sibling Add, then −1500 with a local floor).
        // 规则书 does not pin floor-vs-add order among same-stage ± clauses; see
        // the sibling-add regression test.
        // TODO(规则书): local `.max(0)` per clause vs a final floor after the
        //   whole add/sub stage is still unnamed by the book (the file's existing
        //   order-independence claim covers X-then-1500, not add-then-floor).
        // TODO(规则书)（[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0））: a fixed
        //   −1500 is a 支付阶段 2 「支付计算时」 modifier, but the 共鸣 prompt needs
        //   an interactive stage-2 hook and there is none (no `PaySplitAddBefore`),
        //   so it lands at `PaySplitMulBefore` (stage 3) after the whole stage-2
        //   fold; a sibling stage-2 add therefore composes differently.
        // TODO(规则书)（[共鸣][反击][消耗]或[支付]时）: `PaySplitMulBefore` runs per
        //   settled leg, so a split payment prompts, spends one 共鸣 and shaves
        //   1500 per leg -- the sheet does not say per command or per leg.
        On::Action(Action::PaySplitMulBefore("actor == owner && value > 0", None, pay_res).counter_reason(key!("aya_longing_when"))),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"憧憬的前方"#)
        .raw_name(r#"[丸山彩]憧憬的前方"#)
        .tags(&[r#"手"#, r#"反击"#, r#"持续"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。"#),
            ]),
            sec(r#"[持续]："#, &[
                items(&[
                    it(r#"如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]或[支付]时将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。"#),
                    it(r#"[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）。"#),
                ]),
            ]),
        ]))
        // sheet cell D16
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", r#"丸山彩"#)]));

/// C# `CardAyaLonging.X` -- 100, or 200 once the owner holds 10+ [P✽P粉丝].
// 规则书[持续]（1）: 「X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100」
fn shave_x(player_id: i32) -> i32 {
    let fans = fans::up(player_id) + fans::down(player_id);
    if fans < 10 {
        100
    } else {
        200
    }
}

/// C# `CardAyaLonging.Poorest` -- every alive player has at least as much money.
// 规则书[持续]（1）: 「如果[拥有者]的资金数是所有存活玩家中最少」
// TODO(规则书)（如果[拥有者]的资金数是所有存活玩家中最少）: the sheet does not say
//   whether a money tie counts as 「最少」 (OPEN-ties) -- we count it (non-strict `>=`).
fn poorest(player_id: i32) -> bool {
    let me = ctx::money_of(player_id);
    (0..ctx::player_count()).all(|p| p == player_id || ctx::player_out(p) || ctx::money_of(p) >= me)
}

fn aya_longing(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "PP:[丸山彩]憧憬的前方",
        &Msg::new(key!("aya_longing_note")),
    );
    // 规则书[手]: 「并将弃卡区中的一张卡加入手卡」
    // C# `H._hidden[i].discard.Distinct()` + `H.AskCard` + `H._hidden[i].discard.Remove`
    // + `H.AddToHand`.
    let mut ids: Vec<NameId> = Vec::new();
    for c in ctx::cards_in(player_id, ctx::CardPile::Graveyard) {
        if !ids.contains(&c) {
            ids.push(c);
        }
    }
    if ids.is_empty() {
        return Ok(());
    }
    let refs: Vec<&str> = ids.iter().map(|c| c.as_str()).collect();
    let pick = ctx::ask_card(
        player_id,
        &Msg::new(key!("aya_longing_title")),
        &Msg::new(key!("aya_longing_ask")),
        &refs,
    )?;
    let id = ids.swap_remove(pick.min(ids.len() - 1));
    if ctx::take_card(player_id, id.as_str(), ctx::CardPile::Graveyard) {
        ctx::add_to_dest(player_id, id.as_str(), ctx::Dest::Hand);
        ctx::log(
            player_id,
            &Msg::new(key!("aya_longing_back"))
                .player_id("who", player_id)
                .card("card", id.as_str()),
        );
    }
    Ok(())
}

/// 规则书[持续]（1）: while the owner is the poorest alive player, every
/// [消耗]/[支付] drops by X (floor 0). Pure `PaySplitAdd` Filter -- returns the
/// new figure, the host folds it into the leg amount.
fn shave_x_filter(player_id: i32, mut v: PayAddValue) -> PayAddValue {
    // 「[拥有者][消耗]或[支付]时」 -- the owner is the payer/spender.
    // TODO(规则书)（（最低0））: the reduction floors at 0 -- we skip entirely when the
    //   running figure is already ≤0, so a figure an earlier counteraction drove
    //   negative neither floors to 0 nor loses X here.
    if v.value <= 0 || trigger::player_id() != player_id || !poorest(player_id) {
        return PayAddValue {
            value: v.value,
            ..v
        };
    }
    let x = shave_x(player_id);
    let out = (v.value - x).max(0);
    ctx::log(
        player_id,
        &Msg::new(key!("aya_longing_shave")).i("n", x as i64),
    );
    PayAddValue { value: out, ..v }
}

/// 规则书[持续]（2）: 「[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）」 --
/// the 共鸣 cost is interactive, so this half stays an Action on
/// `PaySplitMulBefore` (the PaySplitMul input).
fn pay_res(player_id: i32) -> card_sdk::Asked {
    if trigger::value() <= 0 {
        return Ok(());
    }
    // TODO(规则书)（[共鸣][反击][消耗]或[支付]时）: 「[共鸣]」 is undefined by the book
    //   (OPEN-共鸣) -- we require paying the 共鸣 cost before the −1500 can apply.
    if crate::resonance::try_resonance(player_id)? {
        trigger::set_pay_amount((trigger::value() - 1500).max(0));
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn aya_longing_kani(player_id: i32) -> card_sdk::Asked {
    aya_longing(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn shave_x_filter_kani(player_id: i32, v: card_sdk::PayAddValue) -> card_sdk::PayAddValue {
    shave_x_filter(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_res_kani(player_id: i32) -> card_sdk::Asked {
    pay_res(player_id)
}
