//! `Sumimi:#L12` -- C# `CardL12` (MatchHost.cs:11621-11682): pull a Sumimi card
//!
//! 规则书（the roster, id `Sumimi:#L12`）:
//! > [手] 从弃牌堆或抽牌堆选择一张“Sumimi”卡加入手牌并将此卡置于自身场上； [持续] （1）[拥有者]手卡上限数量减1。（2）每当你消耗火罐时，为此卡添加一个[奇迹水晶] （3）当此卡上拥有6个[奇迹水晶]时，将此卡返回手牌。
//!
//! from discard/draw, place this card, hand limit -1, crystals on fire spent.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger, CardPile};
use bandori_std::state_key;
use card_sdk::abi::prop;
use card_sdk::ctx::NameId;
use card_sdk::{it, items, para, sec, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const L12: CardDef = CardDef::new(
    "Sumimi:#L12",
    &[
        On::Play("", None, l12),
        // 规则书[持续]（2）: 「每当你消耗火罐时，为此卡添加一个[奇迹水晶]」
        On::Action(Action::MarkerSpent("", Some(fire_spent_guard), fire_spent)),
        // 规则书[持续]（3）: 「当此卡上拥有6个[奇迹水晶]时，将此卡返回手牌。」 -- a
        // state trigger on the counter, so a crystal put here by another card
        // counts as much as this card's own fire spend.
        On::Action(Action::CounterChanged(
            "card.placed && counter_is('crystals') && card.counter('crystals') >= 6",
            Some(back_guard),
            back)),
    ],
)
.meta(&card_sdk::meta::CardMeta::new()
        // sheet cell L12 -- the name line is blank; `#L12` is the name.
        .name("#L12")
        .raw_name("")
        .tags(&[r#"手"#, r#"持续"#])
        .text(CardText::of(&[
            sec("", &[
                para(r#"[手] 从弃牌堆或抽牌堆选择一张“Sumimi”卡加入手牌并将此卡置于自身场上； [持续]"#),
                items(&[
                    it(r#"[拥有者]手卡上限数量减1。"#),
                    it(r#"每当你消耗火罐时，为此卡添加一个[奇迹水晶]"#),
                    it(r#"当此卡上拥有6个[奇迹水晶]时，将此卡返回手牌。"#),
                ]),
            ]),
        ]))
        .fields(&[("group", r#"Sumimi"#), ("owner", "")])) // 规则书[持续]（1）: 「[拥有者]手卡上限数量减1。」 -- the `handLimitDelta`
// property (C# `Card.HandLimitDelta`), continuous while this instance sits on
// the field.
.props(&[(prop::HAND_LIMIT_DELTA, -1)]);

const ID: &str = "Sumimi:#L12";

/// C# `CardL12.Pool` -- distinct ids in the discard then the draw pile that
/// belong to band `Sumimi`. The data key prefix is the band
/// (`H.Db.Card(id)?.group == "Sumimi"`). The copy being played is already on
/// the field by now, so another `#L12` in the discard is a legal pick and no
/// self-exclusion is needed.
fn pool(player_id: i32) -> Vec<NameId> {
    let mut out: Vec<NameId> = Vec::new();
    for pile in [CardPile::Graveyard, CardPile::Deck] {
        for id in ctx::cards_in(player_id, pile) {
            if id.as_str().starts_with("Sumimi:") && !out.iter().any(|x| *x == id) {
                out.push(id);
            }
        }
    }
    out
}

fn l12(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡置于自身场上」 -- C# `H.PlaceFromPlay(c)`.
    // TODO(规则书)（从弃牌堆或抽牌堆选择一张“Sumimi”卡加入手牌并将此卡置于自身场上）:
    // 书里先选卡加入手牌、再把此卡放场上 -- 我们先放置再询问（询问会暂停，结算顺序
    // 因此与所写相反）。
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("l12_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("l12_placed")).player_id("who", player_id),
    );
    // 规则书[手]: 「从弃牌堆或抽牌堆选择一张“Sumimi”卡加入手牌」 -- C# `CardL12.Pool`
    // lists `hidden.discard.Concat(hidden.draw)` ids with `band == "Sumimi"` and
    // `H.AskCard` picks one to `H.AddToHand`.
    let pool = pool(player_id);
    if pool.is_empty() {
        return Ok(());
    }
    let picks: Vec<&str> = pool.iter().map(|s| s.as_str()).collect();
    let i = ctx::ask_card(
        player_id,
        &Msg::new(key!("l12_title")),
        &Msg::new(key!("l12_search")),
        &picks,
    )?;
    let id = pool[i];
    // C# `hidden.discard.Remove(text)`, else `hidden.draw.Remove(text)` +
    // `H.Shuffle(hidden.draw)`. `take_card` pulls one copy out of the pile
    // without moving it; `add_to_dest` then files it.
    if !ctx::take_card(player_id, id.as_str(), CardPile::Graveyard) {
        ctx::take_card(player_id, id.as_str(), CardPile::Deck);
        // TODO(规则书)（从弃牌堆或抽牌堆选择一张“Sumimi”卡加入手牌）: 书里不说选
        // 完是否重洗 -- 我们照 C# `H.Shuffle` 把剩下的抽牌堆重洗。
        ctx::shuffle_into_deck(player_id, false, false);
    }
    ctx::add_to_dest(player_id, id.as_str(), ctx::Dest::Hand);
    ctx::log(
        player_id,
        &Msg::new(key!("l12_taken"))
            .player_id("who", player_id)
            .card("card", id.as_str()),
    );
    // 规则书[持续]（1）: 「[拥有者]手卡上限数量减1。」 -- the continuous delta
    // declared on the `CardDef` above (`props(&[(prop::HAND_LIMIT_DELTA, -1)])`), stamped at
    // placement, gone with the card. No state write: that would double-count.
    // C# `NoteText` shows the crystal count / hand-limit note; CardDef has no
    // NoteText hook.
    Ok(())
}

/// 规则书[持续]（2）: 「每当你消耗火罐时，为此卡添加一个[奇迹水晶]」 -- C#
/// `CardL12.FireSpent`.
/// Guard for [`fire_spent`]: only a fire-pot spend counts (`markerSpent` is
/// every named counter of many) and only this seat's. `false` means the card
/// is not activated at all.
fn fire_spent_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && matches!(
            trigger::name().as_str(),
            bandori_std::fire::KEY | bandori_std::fire::TEMP_KEY
        )
}

// TODO(规则书)（每当你消耗火罐时，为此卡添加一个[奇迹水晶]）: 书里的“消耗” -- 现在
// regime 的「每回合结束时失去1火罐 / 退出状态2时失去所有」也走 `spend_pool`，失去
// 同样会加一个晶。
fn fire_spent(player_id: i32) -> card_sdk::Asked {
    // 规则书[持续]（2）: 「每当你消耗火罐时，为此卡添加一个[奇迹水晶]」 -- one
    // crystal per pot spent (`t.value` is how many pots went). The 「每当…时」
    // can also be read as one per spend; the C# counts per pot.
    let n = trigger::value().max(0);
    if n == 0 {
        return Ok(());
    }
    // 规则书: 「为此卡添加一个[奇迹水晶]」
    ctx::add_crystals(n, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("l12_fire_spent"))
            .player_id("who", player_id)
            .i("n", n as i64),
    );
    Ok(())
}

/// 规则书[持续]（3）: 「当此卡上拥有6个[奇迹水晶]时，将此卡返回手牌。」 -- run by
/// the `counterChanged` entry above.
fn back(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Hand);
    ctx::log(
        player_id,
        &Msg::new(key!("l12_back")).player_id("who", player_id),
    );
    Ok(())
}

/// Residual the condition cannot express: the changed card is this instance
/// (the raise carries the card id, not the uid) and the counter is `crystals`.
fn back_guard(_player_id: i32) -> bool {
    ctx::is_placed() && bandori_std::crystals_changed_of(ID)
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn l12_kani(player_id: i32) -> card_sdk::Asked {
    l12(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn fire_spent_kani(player_id: i32) -> card_sdk::Asked {
    fire_spent(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn back_kani(player_id: i32) -> card_sdk::Asked {
    back(player_id)
}
