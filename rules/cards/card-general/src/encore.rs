//! `通用:安可` -- C# `CardEncore` (MatchHost.cs:2388-2408): [反击] negate an
//!
//! 规则书（docs/rulebook/cards.json, id `通用:安可`）:
//! > 安可：
//! > [手]：
//! > [反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果。
//!
//! [异常移动效果] aimed at the user.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{AbKind, ChainKind};
use card_sdk::{key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const ENCORE: CardDef = CardDef::new(
    "通用:安可",
    &[On::Counteract(
        &[ChainKind::Effect],
        // 规则书[反击]: 「[使用者]即将因任何原因受到[异常移动效果]影响时」
        // C# `CardEncore.CanCounteract`: `t.Kind == "abnormal" && t.Target == seat`.
        // 「[异常移动效果]」 is the abnormal family (传送/停留/晕眩/除外/强制移动/
        // 强制停下/反方向 -- CROSS-TESTS 47-52), carried here as the chain's
        // `abnormal` effect entry -- not just any effect aimed at the user: the
        // money pipeline declares a `pay` effect on every [支付]/[获得] and must
        // not open this window.
        "target == owner && effect.has(Abnormal)",
        None,
        counteract).counter_reason(key!("encore_when"))],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"安可"#)
        .raw_name(r#"安可"#)
        .tags(&[r#"手"#, r#"反击"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"[反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果。"#),
            ]),
        ]))
        // sheet cell A13
        .fields(&[("group", r#"通用"#), ("owner", "")]),
);

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「无效此次[异常移动效果]和其导致的所有效果」
    // C# `c.Trigger.Cancelled = true` then 「安可：这次…无效」.
    trigger::set_cancelled();
    ctx::log(
        player_id,
        &Msg::new(key!("encore_negate")).player_id("who", player_id),
    );
    // TODO(规则书)[judgement](ABI): the C# log names the abnormal
    //   (`AbName(c.Trigger.Ab?.Kind)`) -- the clause under-specifies; narrowed to
    //   the generic [异常移动效果] wording. `trigger::abnormal_kind()` is `None`
    //   inside an `Effect` window (it only matches Abnormal/AbnormalGuard kinds),
    //   so the kind would have to come from the chain's `effect::kind` entry (cf.
    //   `own_stage.rs`), and `Msg` has no localized AbKind-name argument anyway.
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
