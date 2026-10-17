//! `通用:[衍生]FEVER!` -- C# `CardFever`: stays in play, money to the owner +X.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:[衍生]FEVER!`）:
//! > [衍生]FEVER!：
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]。
//! > [持续]：
//!
//! > （1）[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）。
//!
//! > （2）[拥有者]回合开始时将此卡放入[使用者]弃卡区。
//!

use card_sdk::abi::HookKind;
use ruleset_core::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "通用:[衍生]FEVER!";

pub const FEVER: CardDef = CardDef::new(
    "通用:[衍生]FEVER!",
    &[
        On::Play("", None, fever),
        // C# `CardFever.PaySplitAdd` (value modifier) / `CardFever.TurnStart` (event
        // reaction) -- field hooks, not [反击]. Mixed kinds split (ABI v73);
        // bodies split to match the variant (Filter = pure integer, Action =
        // event reaction).
        On::Filter(&[HookKind::PaySplitAdd], "card.placed", None, pay_add),
        On::Action(&[HookKind::TurnStart], "card.placed", None, turn_start),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"FEVER!"#)
        .raw_name(r#"[衍生]FEVER!"#)
        .derived(true)
        .tags(&[r#"手"#, r#"持续"#, r#"衍生"#])
        .text(r#"[手]：
将此卡放置在[使用者]的[场地]。
[持续]：
（1）[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）。
（2）[拥有者]回合开始时将此卡放入[使用者]弃卡区。"#)
        .cell(r#"A6"#)
        .fields(&[("group", r#"通用"#), ("owner", "")]));

fn fever(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("fever_note")));
    // C# `H.PlaceFromPlay(c).Mem["x"] = c.N(0, 600)` -- the printed 600 is the
    // card's number, read through `ctx::n` in `x_of` (the C# stores the played
    // value in `Mem["x"]`; equivalent while `PlayCtx.Doubled` is unported).
    // 规则书（1）[持续] runs in `counteract` at `paySplitAdd`; 规则书（2）[持续] at `turnStart`.
    Ok(())
}

/// 规则书（1）: 「X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）」 -- 600
/// minus 200 per **face-up** card on the owner's field, this card included
/// (「每拥有一张卡」 counts every card the owner has in play), free to go below 0.
fn x_of(player_id: i32) -> i32 {
    let n = ctx::field_instances(player_id)
        .into_iter()
        .filter(|(uid, _)| !ctx::is_face_down_at(*uid))
        .count() as i32;
    ctx::n(0, 600) - 200 * n
}

/// 规则书（1）[持续]: 「[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，
/// [拥有者]场上每拥有一张卡则X降低200（可小于0）」 -- C# `CardFever.PaySplitAdd`
/// (`p.to == Player && p.amount > 0` -> `p.amount = Math.Max(0, p.amount + x)`).
/// `t.target` is the payee (a gain is the bank paying the owner) and `value` the
/// pending amount. Early-returns that skipped the log still return `value`
/// unchanged without logging.
fn pay_add(player_id: i32, value: i32) -> i32 {
    if trigger::target() != player_id || value <= 0 {
        return value;
    }
    let x = x_of(player_id);
    let out = (value + x).max(0);
    // C# `p.Note("FEVER! " + ((x >= 0) ? "+" : "") + x)`.
    ctx::log(player_id, &Msg::new(key!("fever_pay")).i("x", x as i64));
    out
}

/// 规则书（2）[持续]: 「[拥有者]回合开始时将此卡放入[使用者]弃卡区」
fn turn_start(player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() != player_id {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("fever_unplaced")).player_id("who", player_id),
    );
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn fever_kani(player_id: i32) -> card_sdk::Asked {
    fever(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_add_kani(player_id: i32, value: i32) -> i32 {
    pay_add(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_start_kani(player_id: i32) -> card_sdk::Asked {
    turn_start(player_id)
}
