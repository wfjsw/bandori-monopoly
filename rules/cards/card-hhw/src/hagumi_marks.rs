//! `HHW:（育美）` -- C# `CardHagumiMarks` (MatchHost.cs:4293-4313).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（育美）`）:
//! > （育美）We Can☆Hurray!
//! > （1）[手] 投掷2次4d20并将一个育美标记放置到投掷结果之一的格子上，你经过育美标记时可在那格强制停下并获得2000资金，然后移除该标记；若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记。
//! > （2）[反击] 此卡可在你回合外收到资金的回合结束时打出，当前回合内你每获得过一次资金，此卡的投掷次数+1
//!
//! Roll 4d20 twice, offer the (distinct) resulting tiles, place a mark on the
//! chosen one -- or, when the chosen result is above 60, gain 1000 and draw a
//! card instead of placing. The prompt's options come from dice rolled inside
//! the effect; replay re-rolls them identically on every run.
//!
//! The mid-walk stop rides the `PassTile` hook: the C# `HagumiMarkFx` is an
//! `H.ExtraOf` player attachment, and the hook surface only dispatches to
//! *placed* cards, so the play body places this card as the `HagumiMarkFx`
//! stand-in (same pattern as `HHW:爱心义演`'s `CharityFx`).

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{roll_source, ActionKind, ChainKind, MarkFilter, MoveKind, RollPurpose};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "HHW:（育美）";

pub const HAGUMI_MARKS: CardDef = CardDef::new(
    "HHW:（育美）",
    &[
        On::Play("", None, play),
        // 规则书（1）: 「你经过育美标记时可在那格强制停下」 -- the offer exists
        // only when one of this player's 育美标记 sits on the tile. Mark count
        // is a derived scan, residual (docs/GUARDS.md §4.2).
        // No `move.kind != Teleport` here: the move table raises 经过 at a
        // teleport's destination (docs/rulebook/rulebook-doc.md 「[传送]后在终点
        // 触发[经过]」), and the stop itself is moot for an endpoint (`remaining`
        // is 0 there, so `hook` skips `set_stop_at`).
        On::Action(Action::PassTile("actor == owner && card.placed && tile.id >= 0", Some(has_mark_here), hook)),
        On::Counteract(
            &[ChainKind::EndTurnAfter],
            // 「你回合外」 is `actor != owner`. 「收到资金」 -- at least one gain
            // this turn -- is `gains_this_turn(owner) > 0`, the engine's
            // per-turn gain counter (rulebook 支付阶段 7 「资金变动」). A pure-hand
            // (2) sees the count too -- nothing needs to be placed.
            // TODO(规则书)（你回合外收到资金的回合结束时）: 「收到资金」 is any money
            // in -- the counter only counts `gain_fixed` / `gain_money` / the
            // payment pipeline, so a raw credit (bankruptcy hand-over, private
            // sale, build refund, mortgage) does not offer the (2).
            "actor != owner && gains_this_turn(owner) > 0",
            None,
            counteract2),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"We Can☆Hurray!"#)
        .raw_name(r#"（育美）We Can☆Hurray!"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"[手] 投掷2次4d20并将一个育美标记放置到投掷结果之一的格子上，你经过育美标记时可在那格强制停下并获得2000资金，然后移除该标记；若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记。"#),
            it(r#"[反击] 此卡可在你回合外收到资金的回合结束时打出，当前回合内你每获得过一次资金，此卡的投掷次数+1"#),
        ]))
        // sheet cell F14
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", r#"北泽育美"#)]));

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
    // Each option is one *roll* `(tile, roll)`; identical pairs collapse (C#
    // `.Distinct()`), but two different results on one tile stay two options --
    // 「若选择的投掷结果大于60」 is about the chosen roll, not the tile.
    let mut options: alloc::vec::Vec<(i32, i32)> = alloc::vec::Vec::new();
    for _ in 0..times.max(1) {
        let r = ctx::roll(player_id, 4, 20, RollPurpose::Effect, roll_source::CARD)?;
        let t = (r - 1).rem_euclid(n);
        if !options.contains(&(t, r)) {
            options.push((t, r));
        }
    }
    let title = Msg::new(key!("hagumi_marks_ask_title"));
    let text = Msg::new(key!("hagumi_marks_ask_text"));
    // 「将一个育美标记放置到投掷结果之一的格子上」 -- one labelled option per
    // roll; the answer is the option index, so the chosen roll survives even when
    // another option sits on the same tile.
    let opts: alloc::vec::Vec<Msg> = options
        .iter()
        .map(|&(t, r)| {
            Msg::new(key!("hagumi_marks_opt"))
                .tile("tile", t)
                .i("roll", r as i64)
        })
        .collect();
    let i = ctx::ask_pick(player_id, &title, &text, &opts)?;
    let (tile, chosen) = options[i.min(options.len() - 1)];
    // 规则书（1）[手]: 「若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记」
    // -- the book's literal 60 (not the tile count: a custom board may differ).
    if chosen > 60 {
        ctx::gain(player_id, 1000, &Msg::new(key!("hagumi_marks_over_why")))?;
        ctx::draw(player_id, 1)?;
        return Ok(());
    }
    // 规则书（1）: 「你经过育美标记时可在那格强制停下并获得2000资金，然后移除该标记」
    // -- C# `HagumiMarkFx.PassTile` / `Stop` (an `H.ExtraOf` attachment). The
    // hook surface only dispatches to placed cards, so this placement stands in
    // for the player attachment (same pattern as `HHW:爱心义演`); the `PassTile`
    // hook below is the stop body. Place the stand-in *before* the mark so
    // `self_uid()` is its instance and the mark dies with it.
    if !ctx::is_placed() {
        ctx::set_dest(ctx::Dest::Field);
        ctx::place_card(player_id, ID, &Msg::new(key!("hagumi_marks_note")));
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
        &note,
        card_sdk::abi::Stack::Fresh,
    );
    ctx::log(
        player_id,
        &Msg::new(key!("hagumi_marks_placed")).tile("tile", tile),
    );
    Ok(())
}

/// C# `HagumiMarkFx.PassTile` / `Stop` (MatchHost.cs:4314-4384).
/// Residual guard for [`hook`]: one of this player's 育美标记 is on the tile.
/// C# `H.CountMarks(t, "育美标记", Seat) > 0`.
fn has_mark_here(player_id: i32) -> bool {
    ctx::count_marks(trigger::tile(), &mark_filter(player_id)) > 0
}

/// Applicability is the entry's condition + `has_mark_here`.
fn hook(player_id: i32) -> card_sdk::Asked {
    let t = trigger::tile();
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
    // TODO(规则书)（可在那格强制停下并获得2000资金）: the book attaches the 2000
    // and the removal to the pass unconditionally -- a refused gate here skips
    // both (C# `H.AbnormalGate`), and the sheet carries no such carve-out.
    if !ctx::gate(trigger::player_id(), bandori_std::kind::abnormal::Stop) {
        return Ok(());
    }
    // C# `m.Remaining > 0 && !m.Teleport` -- only a mid-walk tile can be stopped
    // (a teleport's remaining is 0, so it arrives here for the gain/removal only).
    if trigger::move_remaining() > 0 {
        // C# `m.Stopped = true` -- the walk settles at the stop tile.
        ctx::plan::set_stop_at(t);
    }
    // 规则书（1）: 「获得2000资金，然后移除该标记」 -- C# `H.GainR(Seat, 2000,
    // "育美标记")` first, then one mark is spent (`bump_mark`: 「该标记」 is one
    // mark, not every row that happens to share the tile).
    ctx::gain(player_id, 2000, &Msg::new(key!("hagumi_marks_gained")))?;
    ctx::bump_mark(t, &mark_filter(player_id), -1);
    ctx::log(
        player_id,
        &Msg::new(key!("hagumi_marks_stop_done"))
            .player_id("who", player_id)
            .tile("tile", t),
    );
    // Stand-in cleanup: when the owner has no marks left the attachment is
    // spent (C# `HagumiMarkFx` just goes quiet; the field-card stand-in files
    // itself away).
    // TODO(规则书)（你经过育美标记时可在那格强制停下并获得2000资金）: the cleanup
    // files only the answering copy -- should a second stand-in of this card sit
    // on the field, it stays behind inert (there is no per-instance dest write).
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
    MarkFilter::any()
        .kind(key!("hagumi_marks_mark"))
        .owner(owner)
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
    // TODO(规则书)（此卡的投掷次数+1）: 「投掷次数+1」 is read as one more whole 4d20
    // roll -- OPEN-HHW-HAGUMI-TIME (+1 die vs +1 to each roll), not ruled.
    rolls(player_id, 2 + gains)
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
pub fn hook_kani(player_id: i32) -> card_sdk::Asked {
    hook(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract2_kani(player_id: i32) -> card_sdk::Asked {
    counteract2(player_id)
}
