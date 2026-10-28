//! `AG:（鸫）微小的『能做到』的事` -- C# `CardTsugumiCan` (MatchHost.cs:1818-1901):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（鸫）微小的『能做到』的事`）:
//! > （鸫）微小的『能做到』的事：打出此卡时，选择其中一个效果发动：
//! > （1）【反击】时机合适时打出此卡，使任意结果只有数字区间的效果以理论最大值或最小值结算，除@Tsugu ycm以外无法直接改变骰子点数（ex：可以对YOLO生效但无法对无论是何种颜色的夕阳生效）
//! > （2）视为打出一张@Tsugu ycm
//!
//! choose one at play time: play a @Tsugu ycm, or (as a counteraction) force a
//! range-card's dice to max/min. Only the chosen effect runs. No whole-card
//! play gate: (1) needs no main move, and the nested (2) ycm gates only its
//! own teleport arm (LOY-TSUGU-01).

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::{it, items, para, sec, CardText};
use card_sdk::{key, CardDef, Msg, On};

pub const TSUGUMI_CAN: CardDef = CardDef::new(
    "AG:（鸫）微小的『能做到』的事",
    &[
        On::Play("", None, play),
        On::Counteract(&[ChainKind::Card], "", Some(can_counteract), counteract).counter_reason(key!("tsugumi_can_when")),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"微小的『能做到』的事"#)
        .raw_name(r#"（鸫）微小的『能做到』的事"#)
        .tags(&[r#"反击"#])
        .text(CardText::of(&[
            sec("", &[
                para(r#"打出此卡时，选择其中一个效果发动："#),
                items(&[
                    it(r#"【反击】时机合适时打出此卡，使任意结果只有数字区间的效果以理论最大值或最小值结算，除@Tsugu ycm以外无法直接改变骰子点数（ex：可以对YOLO生效但无法对无论是何种颜色的夕阳生效）"#),
                    it(r#"视为打出一张@Tsugu ycm"#),
                ]),
            ]),
        ]))
        // sheet cell C16
        .fields(&[("group", r#"Afterglow"#), ("owner", r#"羽泽鸫"#)]));

fn can_counteract(player_id: i32) -> bool {
    // 规则书（1）【反击】: 「时机合适时打出此卡，使任意结果只有数字区间的效果以理论最大值或最小值结算」
    // -- C# counteracts while another `RangeCard` play is resolving
    // (`t.Kind == "card" && t.Play.Def.RangeCard && t.Play.Extreme == 0`).
    let _ = player_id;
    // 规则书（1）: 「除@Tsugu ycm以外无法直接改变骰子点数」 -- ycm is the one
    // exception, i.e. the card whose pips MAY be forced, so a ycm play is not
    // excluded here (OPEN-@Tsugu reads the sheet mention as garbled); a future
    // RangeCard filter must keep letting ycm through.
    // TODO(规则书)[judgement](ABI)（使任意结果只有数字区间的效果以理论最大值或最小值结算）:
    // the clause limits the counter to numeric-interval effects (YOLO yes,
    // 「无论是何种颜色的夕阳」 no) and to a play with no extreme yet armed, but
    // the trigger vocabulary has no `t.Play.Def.RangeCard` / `t.Play.Extreme`,
    // so every `card` play falls through to `true` (BACKLOG ABI-04).
    true
}

/// Per-play slot: which printed effect was chosen. 1-based so an unset slot
/// reads 0; the card's own play body reads it. The engine runs this card's own
/// follow-up (`counteract`) **before** the play body, so the choice is asked
/// there and only the (2) arm plays anything in `play`.
const PICK: &str = "tsugumi_can_pick";

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「打出此卡时，选择其中一个效果发动」 -- `counteract` asked and
    // stored the pick before this body; (1) already ran its force there, so only
    // (2) does work here.
    if ctx::slot(player_id, PICK) == 2 {
        // 规则书（2）: 「视为打出一张@Tsugu ycm」 -- C# `Ycm.Play(c)` on a shared
        // `CardTsuguYcm` instance (id `通用:@Tsugu ycm`).
        ctx::log(
            player_id,
            &Msg::new(key!("tsugumi_can_ycm")).player_id("who", player_id),
        );
        ctx::play_card("通用:@Tsugu ycm", player_id)?;
    }
    Ok(())
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「打出此卡时，选择其中一个效果发动」 -- the whole card is one of
    // two effects. The engine's own-play follow-up reaches this body before the
    // play body runs (it used to run both (1) and (2) unconditionally); a
    // declared counteraction against another card reaches it directly, so the
    // choice is asked here for both. Stored per play: `play` plays the ycm only
    // on (2).
    // The own-play dispatch hands over this card's own play trigger (`t.Card`
    // is this card); a declared counteraction hands over the **answered**
    // card's trigger instead.
    let own_play = trigger::card_is("AG:（鸫）微小的『能做到』的事");
    ctx::set_slot(player_id, PICK, 0);
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("tsugumi_can_title")),
        &Msg::new(key!("tsugumi_can_choose")),
        &[
            Msg::new(key!("tsugumi_can_choice_counter")),
            Msg::new(key!("tsugumi_can_choice_ycm")),
        ],
    )?;
    ctx::set_slot(player_id, PICK, pick as i32 + 1);
    if pick == 1 {
        // 规则书（2）: 「视为打出一张@Tsugu ycm」 -- on an own play the body
        // below runs next and plays it; a counteraction has no body of its own,
        // so it plays the ycm here instead of forcing the answered play.
        if !own_play {
            ctx::log(
                player_id,
                &Msg::new(key!("tsugumi_can_ycm")).player_id("who", player_id),
            );
            ctx::play_card("通用:@Tsugu ycm", player_id)?;
        }
        return Ok(());
    }
    // 规则书（1）【反击】: 「使任意结果只有数字区间的效果以理论最大值或最小值结算」
    // -- C# asks max/min and writes `target.Extreme = 1 / -1`.
    let extreme = ctx::ask_pick(
        player_id,
        &Msg::new(key!("tsugumi_can_title")),
        &Msg::new(key!("tsugumi_can_ask")),
        &[
            Msg::new(key!("tsugumi_can_max")),
            Msg::new(key!("tsugumi_can_min")),
        ],
    )?;
    // 规则书（1）: 「以理论最大值或最小值结算」 -- C# `target.Extreme = 1 / -1`,
    // so the play being counteracted to settles its number ranges at the theoretical
    // extreme the picker named.
    ctx::set_extreme(if extreme == 0 { 1 } else { -1 });
    ctx::log(
        player_id,
        &Msg::new(key!("tsugumi_can_forced")).player_id("who", player_id),
    );
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

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
