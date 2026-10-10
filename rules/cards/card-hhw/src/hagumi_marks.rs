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
//!
//! The mid-walk stop rides the `PassTile` hook: the C# `HagumiMarkFx` is an
//! `H.ExtraOf` player attachment, and the hook surface only dispatches to
//! *placed* cards, so the play body places this card as the `HagumiMarkFx`
//! stand-in (same pattern as `HHW:爱心义演`'s `CharityFx`).

use card_sdk::abi::{ChainKind, HookKind, MarkFilter, MoveKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "HHW:（育美）";

pub const HAGUMI_MARKS: CardDef = CardDef::new(
    "HHW:（育美）",
    &[
        On::Play("", None, play),
        On::Hook(
            &[HookKind::PassTile],
            "actor == owner && card.placed && move.kind != Teleport && tile.id >= 0",
            None,
            hook,
        ),
        On::Counteract(
            &[ChainKind::EndTurnAfter],
            // 「你回合外」 is `actor != owner`. 「收到资金」 -- at least one gain
            // this turn -- is `gains_this_turn(owner) > 0`, the engine's
            // per-turn gain counter (rulebook 支付阶段 7 「资金变动」). It counts
            // every money-in whatever the cause (C# `H.GainR` covers 「获得」 and
            // payments landing in this player's favour alike), so a pure-hand
            // (2) sees the count too -- nothing needs to be placed.
            "actor != owner && gains_this_turn(owner) > 0",
            None,
            counteract2,
        ),
    ],
);

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）[手]: 「投掷2次4d20」
    rolls(player_id, 2)
}

/// The shared (1) body: roll `times` lots of 4d20, place a mark on one of the
/// distinct results. (2) calls this with a boosted 「投掷次数」.
fn rolls(player_id: i32, times: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    // 规则书（1）[手]: 「投掷2次4d20」 -- C# `H.Roll(i, 4, 20, ...)`. (2) adds
    // 「此卡的投掷次数+1」 per gain, so `times` is the count of 4d20 rolls.
    let mut tiles = alloc::vec::Vec::new();
    let mut rolls = alloc::vec::Vec::new();
    for _ in 0..times.max(1) {
        let r = ctx::roll(player_id, 4, 20);
        let t = (r - 1).rem_euclid(n);
        // 规则书（1）[手]: 「将一个育美标记放置到投掷结果之一的格子上」 -- the rolls
        // may land on the same tile; the C# `.Distinct()` keeps the first roll then.
        if !tiles.contains(&t) {
            tiles.push(t);
            rolls.push(r);
        }
    }
    let title = Msg::new(key!("hagumi_marks_ask_title"));
    let text = Msg::new(key!("hagumi_marks_ask_text"));
    let tile = ctx::ask_tile(player_id, &title, &text, &tiles)?;
    let chosen = rolls[tiles.iter().position(|&t| t == tile).unwrap_or(0)];
    // 规则书（1）[手]: 「若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记」
    // -- the board has 60 tiles, so a total above 60 wrapped past the end of it.
    if chosen > n {
        ctx::gain(player_id, 1000, &Msg::new(key!("hagumi_marks_over_why")))?;
        ctx::draw(player_id, 1)?;
        return Ok(());
    }
    // 规则书（1）[手]: 「将一个育美标记放置到投掷结果之一的格子上」 -- one row per
    // mark (`place_mark_new`, the old `add_mark` semantics).
    let note = Msg::new(key!("hagumi_marks_mark_note")).n("money", 2000);
    ctx::place_mark(
        tile,
        key!("hagumi_marks_mark"),
        "",
        player_id,
        ctx::self_uid(),
        1,
        &note, card_sdk::abi::Stack::Fresh);
    ctx::log(
        player_id,
        &Msg::new(key!("hagumi_marks_placed")).tile("tile", tile),
    );
    // 规则书（1）: 「你经过育美标记时可在那格强制停下并获得2000资金，然后移除该标记」
    // -- C# `HagumiMarkFx.PassTile` / `Stop` (an `H.ExtraOf` attachment). The
    // hook surface only dispatches to placed cards, so this placement stands in
    // for the player attachment (same pattern as `HHW:爱心义演`); the `PassTile`
    // hook below is the stop body.
    if !ctx::is_placed() {
        ctx::set_dest(ctx::Dest::Field);
        ctx::place_card(player_id, ID, &Msg::new(key!("hagumi_marks_note")));
    }
    Ok(())
}

/// C# `HagumiMarkFx.PassTile` / `Stop` (MatchHost.cs:4314-4384).
fn hook(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && card.placed && move.kind != Teleport && tile.id >= 0`
    // is the pre. C# `H.CountMarks(t, "育美标记", Seat) <= 0` -- no mark on
    // this tile -- is a derived-list residual.
    let t = trigger::tile();
    // C# `H.CountMarks(t, "育美标记", Seat) <= 0` -- no mark on this tile.
    if ctx::count_marks(t, &mark_filter(player_id)) <= 0 {
        return Ok(());
    }
    // 规则书（1）: 「可在那格强制停下并获得2000资金」 -- C# `Stop`:
    // `H.AskYes(..., aiYes: true)` then `m.Stopped = true` (behind
    // `H.AbnormalGate`), remove the mark, `H.GainR(Seat, 2000, ...)`.
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("hagumi_marks_stop_title")),
        &Msg::new(key!("hagumi_marks_stop_ask")).tile("tile", t),
    )? {
        return Ok(());
    }
    // 规则书（1）: 「[强制停下]」 -- behind `H.AbnormalGate`: a guarded or
    // unstoppable mover is not stopped, and the mark is not spent.
    if !ctx::gate(trigger::player_id(), card_sdk::abi::AbKind::Stop) {
        return Ok(());
    }
    // C# `m.Remaining > 0 && !m.Teleport` -- force-stop only mid-walk.
    if trigger::move_remaining() > 0 {
        // C# `m.Stopped = true` -- the walk settles at the stop tile.
        ctx::plan::set_stop_at(t);
    }
    // 规则书（1）: 「然后移除该标记」 -- C# decrements the mark count and drops
    // the mark when it reaches 0.
    ctx::remove_marks(t, &mark_filter(player_id));
    // 规则书（1）: 「获得2000资金」 -- C# `H.GainR(Seat, 2000, "育美标记")`.
    ctx::gain(player_id, 2000, &Msg::new(key!("hagumi_marks_gained")))?;
    ctx::log(
        player_id,
        &Msg::new(key!("hagumi_marks_stop_done"))
            .player_id("who", player_id)
            .tile("tile", t),
    );
    // Stand-in cleanup: when the owner has no marks left the attachment is
    // spent (C# `HagumiMarkFx` just goes quiet; the field-card stand-in files
    // itself away).
    if !any_marks(player_id) {
        ctx::set_dest(ctx::Dest::Graveyard);
    }
    // TODO(规则书)（1）[judgement]: C# `HagumiMarkFx.PassTile` also has a circle-tile case
    //   (`H.Tile(t)?.kind == "circle"` -> `All(n)`: remove every 育美标记 and
    //   gain 1,000 per mark 「经过起点」). Not in the rulebook text, so held.
    Ok(())
}

/// The 育美标记 filter: this kind, owned by `player_id`.
fn mark_filter(owner: i32) -> MarkFilter {
    MarkFilter::any().kind(key!("hagumi_marks_mark")).owner(owner)
}

/// Does `player_id` still own any 育美标记 on the board?
fn any_marks(player_id: i32) -> bool {
    let n = ctx::tile_count();
    (0..n).any(|t| ctx::count_marks(t, &mark_filter(player_id)) > 0)
}

/// 规则书（2）: 「当前回合内你每获得过一次资金，此卡的投掷次数+1」 -- the (1)
/// body with 2 + gains rolls of 4d20 instead of 2.
fn counteract2(player_id: i32) -> card_sdk::Asked {
    let gains = ctx::gains_this_turn(player_id);
    // 规则书（2）: 「此卡的投掷次数+1」 per gain, on top of (1)'s 「投掷2次」.
    rolls(player_id, 2 + gains)
}
