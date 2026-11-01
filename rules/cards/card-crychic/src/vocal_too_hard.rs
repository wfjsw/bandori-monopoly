//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:主唱太拼命了`）:
//! > 主唱太拼命了：
//! > [反击] 一次性向其他玩家支付至少5000资金时，免除此次支付。
//!
//! that cancels one 5,000+ payment to another player.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const VOCAL_TOO_HARD: CardDef = CardDef::new(
    "CRYCHIC:主唱太拼命了",
    &[On::Counteract(
        &[ChainKind::Effect],
        // 规则书: 「[反击] 一次性向其他玩家支付至少5000资金时」 -- a payment to
        // another player (`target != owner`; the bank is `target < 0`), and
        // 「一次性」 excludes a [分摊] share (`pay_total_stage`).
        // TODO(规则书)（支付至少5000资金）: 门槛应是实际支付额 -- the `effect`
        //   窗的 `value` 是修正前的宣告额 (CROSS-TESTS C20)。
        // TODO(ABI)（一次性…支付）: a 「[分摊]」 command raises the `effect` window
        //   only on its legs, never on the command total (`pay_total_stages`
        //   runs the `payCombined*` stages alone), so with `pay_total_stage`
        //   this counter never answers a split total at all -- it is dead for
        //   exactly the payments 「一次性」 is meant to keep. Needs an
        //   `effect`-shaped window on `payTotalBefore` (G02 spec 2.20).
        "actor == owner && effect.has(Pay) && target >= 0 && target != owner && pay_total_stage && value >= 5000",
        None,
        counteract,
    )
    .counter_reason(key!("vocal_too_hard_when"))],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"主唱太拼命了"#)
        .raw_name(r#"主唱太拼命了"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(
            r#"[反击] 一次性向其他玩家支付至少5000资金时，免除此次支付。"#,
        ))
        // sheet cell M10
        .fields(&[("group", r#"CRYCHIC"#), ("owner", "")]),
);

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「免除此次支付」
    trigger::set_pay_amount(0);
    ctx::log(
        player_id,
        &Msg::new(key!("vocal_too_hard_note")).player_id("who", player_id),
    );
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
