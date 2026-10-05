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

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const HAGUMI_MARKS: CardDef = CardDef::new("HHW:（育美）", &[
    On::Play(play),
]);

fn play(player_id: i32) {
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    // 规则书（1）[手]: 「投掷2次4d20」 -- C# `H.Roll(i, 4, 20, ...)`.
    let a = ctx::roll(player_id, 4, 20);
    let b = ctx::roll(player_id, 4, 20);
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
    let tile = ctx::ask_tile(player_id, &title, &text, &tiles);
    let chosen = rolls[tiles.iter().position(|&t| t == tile).unwrap_or(0)];
    // 规则书（1）[手]: 「若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记」
    // -- the board has 60 tiles, so a total above 60 wrapped past the end of it.
    if chosen > n {
        ctx::gain(player_id, 1000, &Msg::new(key!("hagumi_marks_over_why")));
        ctx::draw(player_id, 1);
        return;
    }
    // 规则书（1）[手]: 「将一个育美标记放置到投掷结果之一的格子上」
    let note = Msg::new(key!("hagumi_marks_mark_note")).n("money", 2000);
    ctx::add_mark(tile, player_id, key!("hagumi_marks_mark"), &note);
    ctx::log(player_id, &Msg::new(key!("hagumi_marks_placed")).tile("tile", tile));
    // 规则书（1）: 「你经过育美标记时可在那格强制停下并获得2000资金，然后移除该标记」
    // -- C# `HagumiMarkFx.PassTile` / `Stop` (an `H.ExtraOf` attachment).
    // TODO(规则书)（1）: the optional force-stop (`m.Stopped = true` behind
    // `H.AbnormalGate`, gated on `m.Remaining > 0 && !m.Teleport`) has no
    // move-mutation API; without it the gain / mark-removal never runs.
    // TODO(规则书)（2）[反击]: 「此卡可在你回合外收到资金的回合结束时打出，当前回合内你
    // 每获得过一次资金，此卡的投掷次数+1」 -- needs a [反击] window at the turn end
    // of a turn in which the holder gained money off-turn (`PayAfter` / `TurnEndAfter`
    // are hook-only and open no reaction window; the C# `CardHagumiMarks` has no
    // `React` body either).
}