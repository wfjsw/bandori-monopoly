//! `HHW:（育美）` -- C# `CardHagumiMarks` (MatchHost.cs:4293-4313).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（育美）`）:
//! > （育美）
//! > （1）[手] 投掷2次4d20并将一个育美标记放置到投掷结果之一的格子上，你经过育美标记时可在那格强制停下并获得2000资金，然后移除该标记；若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记。
//! > （2）[反击] 此卡可在你回合外收到资金的回合结束时打出，当前回合内你每获得过一次资金，此卡的投掷次数+1
//!
//! Roll 4d20 twice, offer the (distinct) resulting tiles, place a mark on the
//! chosen one. The prompt's options come from dice rolled inside the effect;
//! replay re-rolls them identically on every run.

use card_sdk::{ctx, key, CardDef, Msg};

pub const HAGUMI_MARKS: CardDef = CardDef {
    id: "HHW:（育美）",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    // 规则书（1）[手]: 「投掷2次4d20」 -- C# `H.Roll(i, 4, 20, ...)`.
    let a = ctx::roll(seat, 4, 20);
    let b = ctx::roll(seat, 4, 20);
    // 规则书（1）[手]: 「将一个育美标记放置到投掷结果之一的格子上」 -- the two rolls
    // may land on the same tile; the C# `.Distinct()` keeps the first roll then.
    let mut tiles = vec![(a - 1).rem_euclid(n)];
    let mut rolls = vec![a];
    let t2 = (b - 1).rem_euclid(n);
    if t2 != tiles[0] {
        tiles.push(t2);
        rolls.push(b);
    }
    let title = Msg::new(key!("hagumi_marks_ask_title"));
    let text = Msg::new(key!("hagumi_marks_ask_text"));
    let tile = ctx::ask_tile(seat, &title, &text, &tiles);
    let chosen = rolls[tiles.iter().position(|&t| t == tile).unwrap_or(0)];
    // 规则书（1）[手]: 「若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记」
    // -- the board has 60 tiles, so a total above 60 wrapped past the end of it.
    if chosen > n {
        ctx::gain(seat, 1000, &Msg::new(key!("hagumi_marks_over_why")));
        ctx::draw(seat, 1);
        return;
    }
    // 规则书（1）[手]: 「将一个育美标记放置到投掷结果之一的格子上」
    let note = Msg::new(key!("hagumi_marks_mark_note")).n("money", 2000);
    ctx::add_mark(tile, seat, key!("hagumi_marks_mark"), &note);
    ctx::log(seat, &Msg::new(key!("hagumi_marks_placed")).tile("tile", tile));
    // TODO(规则书)（1）: 「你经过育美标记时可在那格强制停下并获得2000资金，然后移除该
    // 标记」 -- needs the Fx.PassTile hook (C# `HagumiMarkFx.PassTile` / `Stop`) to
    // ask, force-stop and pay 2,000 when the owner passes the mark.
    // TODO(规则书)（2）[反击]: 「此卡可在你回合外收到资金的回合结束时打出，当前回合内你
    // 每获得过一次资金，此卡的投掷次数+1」 -- needs the reaction window on gaining
    // money outside your turn (the ABI's TriggerKind has only Roll / MoveRoll).
}