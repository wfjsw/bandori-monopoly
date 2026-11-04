//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（MASKING）CRUSH ON THE DRUM!!!`）:
//! > （MASKING）CRUSH ON THE DRUM!!!：
//! > [特]：
//! > 打出此卡的回合任何[使用者]使用的卡包含添加骰效果的卡不受任何其他效果影响。
//! > [手]：
//! > 移动阶段前打出此卡，本回合主要移动掷骰额外添加Xd20，X为你弃牌堆的卡数
//!
//! [手] +Xd20 from the discard pile and [特] immunity for dice-adding cards.

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const CRUSH_DRUM: CardDef = CardDef::new(
    "RAS:（MASKING）CRUSH ON THE DRUM!!!",
    &[On::Play("", Some(cant_play), play)],
)
// 规则书[特]: 「打出此卡的回合任何[使用者]使用的卡包含添加骰效果的卡不受
// 任何其他效果影响」 -- the play raises the turn's dice immunity (the engine
// reads it before the play's own `card` window).
.props(&[
    (card_sdk::abi::prop::GRANTS_DICE_IMMUNITY, 1),
    (card_sdk::abi::prop::ADDS_DICE, 1),
])
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"CRUSH ON THE DRUM!!!"#)
        .raw_name(r#"（MASKING）CRUSH ON THE DRUM!!!"#)
        .tags(&[r#"手"#, r#"特"#])
        .text(CardText::of(&[
            sec(
                r#"[特]："#,
                &[para(
                    r#"打出此卡的回合任何[使用者]使用的卡包含添加骰效果的卡不受任何其他效果影响。"#,
                )],
            ),
            sec(
                r#"[手]："#,
                &[para(
                    r#"移动阶段前打出此卡，本回合主要移动掷骰额外添加Xd20，X为你弃牌堆的卡数"#,
                )],
            ),
        ]))
        // sheet cell H14
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", r#"佐藤益木"#)]),
);

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[手]: 「移动阶段前打出此卡」 -- refuses the play once the main move is spent.
    rules_status::move_blocked(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    let count = ctx::discard_size(player_id);
    if count > 0 {
        ctx::plan::add_extra_dice(count, 20, "（CRUSH ON THE DRUM!!!）");
    }
    ctx::log(
        player_id,
        &Msg::new(key!("crush_drum_roll")).i("n", count as i64),
    );
    // TODO(规则书)[judgement](ABI): `OPEN-crush-immune` scope -- settle-time
    // effects after the play and later plays.
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
