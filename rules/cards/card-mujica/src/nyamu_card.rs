//! `Mujica:（喵梦）` -- C# `CardNyamuCard` (MatchHost.cs:6080-6116): placed card
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（喵梦）`）:
//! > （喵梦）
//! >  
//! > （1）将此卡放置于场上
//!
//! > （2）
//! > 1. 每当此卡将从正面翻至背面时可选择失去2个火罐。
//! > 2. 每当此卡将从背面翻至正面时此卡拥有者抽一张卡。
//!
//! Neither flip clause is wired to a real flip trigger yet: the face state is
//! only sampled at the owner's TurnEnd, so the face-down cost is never offered
//! and a flip-up while the card was face-down is not seen.

use card_sdk::{ctx, key, Action, CardDef, Msg, On};
use card_sdk::{it, it_sub, CardText};

const ID: &str = "Mujica:（喵梦）";
/// `card_face_down` as of the last check, so a flip is a change.
const WAS_DOWN: &str = "nyamu.wasDown";

pub const NYAMU_CARD: CardDef = CardDef::new(
    "Mujica:（喵梦）",
    &[
        On::Play("", None, nyamu_card),
        // 规则书: 「每当此卡将从背面翻至正面时此卡拥有者抽一张卡」
        On::Action(Action::TurnEnd(
            "actor == owner && card.placed",
            None,
            watch_flips,
        )),
    ],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name("")
        .raw_name(r#"（喵梦）"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"将此卡放置于场上"#),
            it_sub(
                r#""#,
                &[
                    r#"每当此卡将从正面翻至背面时可选择失去2个火罐。"#,
                    r#"每当此卡将从背面翻至正面时此卡拥有者抽一张卡。"#,
                ],
            ),
        ]))
        // sheet cell J15
        .fields(&[("group", r#"Ave Mujica"#), ("owner", r#"祐天寺若麦"#)]),
);

/// Flip watcher -- `WAS_DOWN` is the face state at the previous sample.
///
/// 规则书: 「每当此卡将从背面翻至正面时此卡拥有者抽一张卡」
///
/// TODO(ABI) face-flip hook: no per-card "about to flip" / "flipped" trigger
/// exists, so a flip is only sampled from the face-down flag at the owner's
/// TurnEnd -- and a face-down placed card's `On::Action` hook is skipped by the
/// dispatch, so the sample never runs while the card is down (`WAS_DOWN` stays
/// 0) and the draw below never fires in the ordinary path. Needs a hook that
/// runs at the flip, face-down card included.
fn watch_flips(player_id: i32) -> card_sdk::Asked {
    let now = ctx::self_face_down();
    let was = ctx::slot(player_id, WAS_DOWN) != 0;
    ctx::set_slot(player_id, WAS_DOWN, now as i32);
    if was && !now {
        // 规则书: 「每当此卡将从背面翻至正面时此卡拥有者抽一张卡」
        ctx::draw(player_id, 1)?;
        ctx::log(player_id, &Msg::new(key!("nyamu_flipped_up")));
    }
    Ok(())
}

fn nyamu_card(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「将此卡放置于场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "Mujica:（喵梦）", &Msg::new(key!("nyamu_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("nyamu_placed")).player_id("who", player_id),
    );
    // 规则书: 「每当此卡将从正面翻至背面时可选择失去2个火罐」
    // TODO(规则书)[judgement]（每当此卡将从正面翻至背面时可选择失去2个火罐）: 翻至背面时可选失去 2 个火罐 -- 这里既不询问也不扣罐；C# `CardNyamuCard.Flipped(down: true)` 跑 `H.AskYes` + `H.SpendFire(Seat, 2)` 后也停在原地。书中没写这 2 罐换来什么，读作「付 2 罐让卡保持正面」，否则纯亏。
    // 这是翻面前的触发，TurnEnd 采样太晚 -- 见 [`watch_flips`]；那里也记了 (2)2 的采样缺口。
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn nyamu_card_kani(player_id: i32) -> card_sdk::Asked {
    nyamu_card(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn watch_flips_kani(player_id: i32) -> card_sdk::Asked {
    watch_flips(player_id)
}
