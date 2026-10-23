//! Board **event card (事件卡) rules** -- what an event does when it is drawn,
//! while it is active, and when it expires.
//!
//! Standardized the same way card, skill and tile rules are: one [`CardDef`] per
//! event, bound to the **neutral board owner** ([`docs/EVENTS.md`]) for as long
//! as the event is in play. The engine keeps only the generic mechanics -- the
//! deck, the draw, the public reveal, the active list and the filing away --
//! so a body is a list of citations over `ctx` primitives.
//!
//! | id | file | kind | sheet |
//! |---|---|---|---|
//! | `event:对邦` | `duobang.rs` | one-shot | 中立事件 A2 |
//! | `event:弦卷集团地产开发` | `tsurumaki_estate.rs` | one-shot | A3 |
//! | `event:很噜的感觉` | `lulu.rs` | one-shot | A4 |
//! | `event:前往哈比内尔王国旅游` | `habinel.rs` | one-shot | A5 |
//! | `event:这只手我不会放开` | `this_hand.rs` | one-shot | A6 |
//! | `event:PICO灵魂交换` | `pico_swap.rs` | one-shot | A7 |
//! | `event:协助CiRCLE重建` | `circle_rebuild.rs` | active | A8 |
//! | `event:前场队还是后场队？` | `front_or_back.rs` | one-shot | A9 |
//! | `event:EX任务挑战` | `ex_quest.rs` | one-shot | A10 |
//! | `event:发送熊饼表情` | `bear_cookie.rs` | one-shot | A11 |
//! | `event:飞鸟山之战` | `asukayama.rs` | one-shot | A12 |
//! | `event:幻觉来了` | `hallucination.rs` | active | A13 |
//! | `event:卡池BUG` | `pool_bug.rs` | active | A14 |
//! | `event:A！A！O！` | `a_a_o.rs` | active | A15 |
//! | `event:麻里奈小姐的礼物箱` | `marina_box.rs` | active | A16 |
//! | `event:元祖！邦多利酱` | `bangdream_chan.rs` | active | A17 |
//! | `event:超燃甩头` | `heat_head.rs` | active | A18 |
//! | `event:意外的对邦` | `surprise_duel.rs` | active | A19 |
//! | `event:Forbidden Moca` | `forbidden_moca.rs` | active | A20 |
//! | `event:上学时间` | `school_time.rs` | one-shot | A21 |
//! | `event:泪水的含义` | `tears.rs` | one-shot | A22 |
//! | `event:Kizuna Music` | `kizuna_music.rs` | one-shot | A23 |
//! | `event:冲榜` | `chart_rush.rs` | derived one-shot | A24 |
//! | `event:修复公告` | `fix_note.rs` | active | A25 |
//! | `event:迷子的追逐` | `lost_chase.rs` | derived one-shot | A26 |
//! | `event:让我来结束一切` | `end_it_all.rs` | derived, permanent | A27 |
//! | `event:一切不会结束` | `never_ends.rs` | derived one-shot | A28 |
//! | `event:火种燃尽之后会怎么样呢？` | `embers.rs` | active | A29 |
//!
//! Every body's prose lives on the [`card_sdk::meta::CardMeta`] beside the rule
//! (name / text / cell / derived, byte-identical to the sheet's 中立事件 tab) and
//! is quoted in the file header, exactly like a card (`docs/CARDS.md`). Gaps in
//! the text, and places the engine falls short of it, are `TODO(规则书)`.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

mod util;

pub mod a_a_o;
pub mod asukayama;
pub mod bangdream_chan;
pub mod bear_cookie;
pub mod chart_rush;
pub mod circle_rebuild;
pub mod duobang;
pub mod embers;
pub mod end_it_all;
pub mod ex_quest;
pub mod fix_note;
pub mod forbidden_moca;
pub mod front_or_back;
pub mod habinel;
pub mod hallucination;
pub mod heat_head;
pub mod kizuna_music;
pub mod lost_chase;
pub mod lulu;
pub mod marina_box;
pub mod never_ends;
pub mod pico_swap;
pub mod pool_bug;
pub mod school_time;
pub mod surprise_duel;
pub mod tears;
pub mod this_hand;
pub mod tsurumaki_estate;

use a_a_o::A_A_O;
use asukayama::ASUKAYAMA;
use bangdream_chan::BANGDREAM_CHAN;
use bear_cookie::BEAR_COOKIE;
use chart_rush::CHART_RUSH;
use circle_rebuild::CIRCLE_REBUILD;
use duobang::DUI_BANG;
use embers::EMBERS;
use end_it_all::END_IT_ALL;
use ex_quest::EX_QUEST;
use fix_note::FIX_NOTE;
use forbidden_moca::FORBIDDEN_MOCA;
use front_or_back::FRONT_OR_BACK;
use habinel::HABINEL;
use hallucination::HALLUCINATION;
use heat_head::HEAT_HEAD;
use kizuna_music::KIZUNA_MUSIC;
use lost_chase::LOST_CHASE;
use lulu::LULU;
use marina_box::MARINA_BOX;
use never_ends::NEVER_ENDS;
use pico_swap::PICO_SWAP;
use pool_bug::POOL_BUG;
use school_time::SCHOOL_TIME;
use surprise_duel::SURPRISE_DUEL;
use tears::TEARS;
use this_hand::THIS_HAND;
use tsurumaki_estate::TSURUMAKI_ESTATE;

/// Every event rule, in sheet / data-file order. The shipped module
/// (`card-all`) concatenates this table with the card, skill and tile ones.
pub static CARDS: &[card_sdk::CardDef] = &[
    DUI_BANG,
    TSURUMAKI_ESTATE,
    LULU,
    HABINEL,
    THIS_HAND,
    PICO_SWAP,
    CIRCLE_REBUILD,
    FRONT_OR_BACK,
    EX_QUEST,
    BEAR_COOKIE,
    ASUKAYAMA,
    HALLUCINATION,
    POOL_BUG,
    A_A_O,
    MARINA_BOX,
    BANGDREAM_CHAN,
    HEAT_HEAD,
    SURPRISE_DUEL,
    FORBIDDEN_MOCA,
    SCHOOL_TIME,
    TEARS,
    KIZUNA_MUSIC,
    CHART_RUSH,
    FIX_NOTE,
    LOST_CHASE,
    END_IT_ALL,
    NEVER_ENDS,
    EMBERS,
];

/// Canonical data-file order for [`meta_rows`] (`RosterList::ordinals`).
/// Keys are the bare event ids (the `event:` rule-id body).
pub const ORDINALS: card_sdk::meta::OrdinalTable = &[
    (r#"对邦"#, 0),
    (r#"弦卷集团地产开发"#, 1),
    (r#"很噜的感觉"#, 2),
    (r#"前往哈比内尔王国旅游"#, 3),
    (r#"这只手我不会放开"#, 4),
    (r#"PICO灵魂交换"#, 5),
    (r#"协助CiRCLE重建"#, 6),
    (r#"前场队还是后场队？"#, 7),
    (r#"EX任务挑战"#, 8),
    (r#"发送熊饼表情"#, 9),
    (r#"飞鸟山之战"#, 10),
    (r#"幻觉来了"#, 11),
    (r#"卡池BUG"#, 12),
    (r#"A！A！O！"#, 13),
    (r#"麻里奈小姐的礼物箱"#, 14),
    (r#"元祖！邦多利酱"#, 15),
    (r#"超燃甩头"#, 16),
    (r#"意外的对邦"#, 17),
    (r#"Forbidden Moca"#, 18),
    (r#"上学时间"#, 19),
    (r#"泪水的含义"#, 20),
    (r#"Kizuna Music"#, 21),
    (r#"冲榜"#, 22),
    (r#"修复公告"#, 23),
    (r#"迷子的追逐"#, 24),
    (r#"让我来结束一切"#, 25),
    (r#"一切不会结束"#, 26),
    (r#"火种燃尽之后会怎么样呢？"#, 27),
];

/// Wire rows for `ruleset_metadata` (`kind = "event"`).
///
/// Prose lives on each [`CardDef`]'s [`card_sdk::meta::CardMeta`]; the row id is
/// the **bare** event id (no `event:` prefix) so it matches `EventData::id` and
/// the `display.json` `events` map.
pub fn meta_rows() -> alloc::vec::Vec<card_sdk::meta::MetaRow> {
    let mut rows = alloc::vec::Vec::new();
    for c in CARDS {
        let Some(m) = c.meta else { continue };
        let bare = c.id.strip_prefix("event:").unwrap_or(c.id);
        // `to_row` tags `kind = "card"`; `RosterList` overwrites it to `"event"`.
        rows.push(m.to_row(bare, 0));
    }
    rows
}
