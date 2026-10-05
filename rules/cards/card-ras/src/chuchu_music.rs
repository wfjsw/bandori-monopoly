//! `RAS:（chuchu）演奏我的音乐吧` -- C# `CardChuchuMusic` (MatchHost.cs:9905-10002):
//! park on another player; their buys pay the user and pass the card along.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（chuchu）演奏我的音乐吧`）:
//! > （chuchu）演奏我的音乐吧：
//! >  将此卡放置于你以外的一名玩家场上并为其添加3个奇迹水晶，那名玩家的每个回合结束时失去一个；场上存在此卡的玩家下次购买地契时，[使用者]获得100资金，将此卡移至除[使用者]外行动序列下一名玩家的场上并将奇迹水晶补充至3个；此卡进入弃牌堆前每触发一次该效果，此卡获得资金时额外获得100（上限500）。此卡奇迹水晶为0时，放入[使用者]的弃牌堆并使[使用者]抽一张卡。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const CHUCHU_MUSIC: CardDef = CardDef {
    id: "RAS:（chuchu）演奏我的音乐吧",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardChuchuMusic.WhyNot`: 「没有别的玩家」 when `H.Others(seat)` is empty.
fn why_not(seat: i32) -> Option<Msg> {
    if ctx::others(seat).is_empty() {
        return Some(Msg::new(key!("chuchu_music_no_others")));
    }
    None
}

fn play(seat: i32) {
    let others = ctx::others(seat);
    // C# `WhyNot` refuses the play with no other player alive; the gate is now
    // `why_not` above.
    if others.is_empty() {
        return;
    }
    // 规则书: 「将此卡放置于你以外的一名玩家场上」 -- C# `H.PickTarget` over
    // `H.Others(seat)` then `H.PlaceFromPlay(c, r.index, -1, 3)`.
    // C# `H.PickTarget` also runs the `H.Target` targeting gate (Untargetable
    // check) after the seat prompt -- no such gate in the vocabulary yet.
    let who = ctx::ask_seat(
        seat,
        &Msg::new(key!("chuchu_music_title")),
        &Msg::new(key!("chuchu_music_ask")),
        &others,
    );
    // 规则书: 「将此卡放置于你以外的一名玩家场上」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        who,
        "RAS:（chuchu）演奏我的音乐吧",
        &Msg::new(key!("chuchu_music_note")),
    );
    ctx::log(
        seat,
        &Msg::new(key!("chuchu_music_placed")).seat("who", seat).seat("target", who),
    );
    // TODO(规则书): 「并为其添加3个奇迹水晶」 -- the placement's crystal charge
    // needs card_crystals on a placed card (`H.PlaceFromPlay(c, r.index, -1, 3)`).
    // TODO(规则书): 「那名玩家的每个回合结束时失去一个」 -- needs the Fx.TurnEnd
    // decay (C# `DecayCard.TurnEnd` / `DecayOn => Seat`).
    // TODO(规则书): 「场上存在此卡的玩家下次购买地契时，[使用者]获得100资金，将此卡移至除[使用者]外行动序列下一名玩家的场上并将奇迹水晶补充至3个」
    // -- needs the Fx.Bought hook (C# `CardChuchuMusic.Bought` / `Move`), the
    // per-card crystal refill, and a card that travels between seats. The next
    // seat in action order is `(Seat + i) % seat_count()` skipping the user and
    // outed seats (C# loop in `Move`).
    // TODO(规则书): 「此卡进入弃牌堆前每触发一次该效果，此卡获得资金时额外获得100（上限500）」
    // -- needs per-card `Mem["hits"]` (C# `Pay => Math.Min(500, 100 + 100 * Hits)`).
    // TODO(规则书): 「此卡奇迹水晶为0时，放入[使用者]的弃牌堆并使[使用者]抽一张卡」
    // -- needs the crystal-empty branch (C# `DecayCard.Empty` -> `H.Unplace` +
    // `H.DrawR(user, 1)`).
}
