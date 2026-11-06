//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Sumimi不会解散哦`）:
//! > Sumimi不会解散哦：
//! >  当你本回合未进行过抵押/赎回操作且资金不含有相同数字时可打出，进行一次可使你的资金变动为拥有相同数字的传送，视为你的主要移动；若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果
//!
//! settle-teleport that must leave money with a repeated digit, else fully
//! revert (`world_restore`, armed by [`no_breakup`] and run by [`revert`]).

use alloc::vec::Vec;

use card_sdk::abi::ActionKind;
use card_sdk::CardText;
use card_sdk::{ctx, key, Action, CardDef, Msg, On};

pub const NO_BREAKUP: CardDef = CardDef::new(
    "Sumimi:Sumimi不会解散哦",
    &[On::Play(
        // The repeated-digit clause stays in the residual guard (not `condition`):
        // a play gate must not trade its specific refusal
        // (`no_breakup_repeated`) for the generic `err.play_pre` a pre-reject
        // surfaces (GUARDS.md §4.4).
        "",
        Some(cant_play),
        no_breakup),
        // 规则书: 「若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消
        // 所有受到的效果」 -- the shot's own settle resolves and is judged here.
        // The entry cond must hold on **both** passes of its drive: the
        // collection and the re-run after the terminal's request (where the
        // restore has put the pre-shot state back). `ARMED` is set before the
        // snapshot precisely so the restore re-arms it on the resume pass; the
        // guards below add the live-seat / digit test.
        On::Action(Action::TileResolved(
            "slot('latch:nb_armed') > 0 && settle.tile >= 0",
            Some(failed_shot),
            revert,
        )),
        On::Action(Action::TileResolved(
            "slot('latch:nb_armed') > 0 && settle.tile >= 0",
            Some(kept_shot),
            settle_after,
        )),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Sumimi不会解散哦"#)
        .raw_name(r#"Sumimi不会解散哦"#)
        .tags(&[])
        .text(CardText::para(r#"当你本回合未进行过抵押/赎回操作且资金不含有相同数字时可打出，进行一次可使你的资金变动为拥有相同数字的传送，视为你的主要移动；若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果"#))
        // sheet cell L3
        .fields(&[("group", r#"Sumimi"#), ("owner", "")]));

/// The armed marker, and the only shot key that is read: the **first** of the
/// two snapshot keys a play takes (see [`no_breakup`]); the shot's own key is
/// `BASE + 1`. A `latch:` slot is load-bearing twice over: the settle that
/// collects the terminals runs inside the play body's own host request, so the
/// marker has to cross onto live (`adopt_turn_policy`) before the collection,
/// and the revert's restore writes the pre-shot player state back, so the
/// marker has to be inside the snapshot or the terminal's re-run would fall
/// through its entry cond. Both come out of one key written before the
/// checkpoint.
const BASE: &str = "latch:nb_base";
/// The armed latch proper (guards the terminals; [`BASE`] carries the key).
const ARMED: &str = "latch:nb_armed";
/// The key of the last shot a terminal finished. A re-run of the play body
/// re-consumes the same snapshot answers; without this it would arm the shot
/// again after the terminal already restored and dropped it, and the next
/// settle of the match would fire the card a second time.
const JUDGED: &str = "latch:nb_judged";

const ID: &str = "Sumimi:Sumimi不会解散哦";

/// 规则书: 「资金不含有相同数字」 (the gate); 「拥有相同数字」 reuses it though the target reading is
/// open -- see the TODO at the `good` predicate.
fn repeated_digits(money: i32) -> bool {
    let mut x = money.unsigned_abs();
    let mut seen: u16 = 0;
    if x == 0 {
        return false;
    }
    while x > 0 {
        let d = (x % 10) as u16;
        if seen & (1 << d) != 0 {
            return true;
        }
        seen |= 1 << d;
        x /= 10;
    }
    false
}

/// TODO(规则书)（进行一次可使你的资金变动为拥有相同数字的传送）: 规则书: 「进行一次可使你的资金变动为拥有相同数字的传送」 -- the
/// destination set the body asks from. Dice rent is not modelled either (it
/// cannot be predicted).
fn predict(player_id: i32, t: i32) -> i32 {
    let money = ctx::money_of(player_id);
    let owner = ctx::tile_owner(t);
    if owner >= 0 && owner != player_id && !ctx::player_out(owner) && !ctx::mortgaged_of(t) {
        money - ctx::rent_of(t)
    } else {
        money
    }
}

/// The repeated-digit refusal is the specific `no_breakup_repeated`, so it lives
/// here, not in `condition`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「当你本回合未进行过抵押/赎回操作」 -- both actions record
    // themselves and clear at the turn end, so the gate is just those records.
    // Checked first: the sheet puts this before the digit condition.
    if ctx::state::get(player_id, "mortgaged") != 0 || ctx::state::get(player_id, "redeemed") != 0 {
        return Some(Msg::new(key!("no_breakup_mortgaged")));
    }
    if repeated_digits(ctx::money_of(player_id)) {
        return Some(Msg::new(key!("no_breakup_repeated")));
    }
    rules_status::move_blocked(player_id)
}

fn no_breakup(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「回到原处并取消所有受到的效果」 -- the checkpoint that the failure
    // terminal restores. The body re-runs from the top after every request, so
    // the checkpoint is taken with `world_snapshot`, a **request**: its answer
    // is the same key on every replay and the snapshot table lives on the host.
    //
    // Two of them, back to back. The shot's bookkeeping is player state, and
    // the restore writes the state back -- so the bookkeeping must be *inside*
    // the snapshot the terminal restores, but its own key cannot be known
    // before the request that produces it. The first key (`BASE`) is therefore
    // taken, written to the slot, and a second snapshot is taken over it; that
    // second snapshot is the checkpoint. Nothing allocates between the two, so
    // its key is `BASE + 1`; both are dropped by the terminal.
    ctx::world_snapshot_drop(ctx::slot(player_id, JUDGED))?;
    ctx::set_slot(player_id, ARMED, 1);
    let base = ctx::world_snapshot()?;
    ctx::set_slot(player_id, BASE, base);
    let checkpoint = ctx::world_snapshot()?;
    if ctx::slot(player_id, JUDGED) != checkpoint {
        // Not this shot yet. A re-run after the terminal already judged and
        // restored it re-consumes the same answers and leaves the marker down,
        // so a later settle does not run the card again.
    } else {
        ctx::set_slot(player_id, ARMED, 0);
    }
    // The shot's own settle resolves while this body runs, so the card has to
    // be in play for its `tileResolved` terminals to be live; either terminal
    // sends it on (dest `Graveyard`).
    ctx::set_dest(ctx::Dest::Field);
    // Only on the first pass: the body re-runs after every host request,
    // including the terminal's restore, and a second `place_card` would put a
    // second copy of the card on the field.
    if ctx::self_tile().is_none() {
        ctx::place_card(player_id, ID, &Msg::new(key!("no_breakup_note")));
    }
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    let pos = ctx::player_pos(player_id);
    // Every square is an option, in the fixed `0..n` order. The reply is an
    // index into the list, and a replay recomputes it from a world the settle
    // has moved on (the pawn, the money), so the list itself must not shift:
    // the squares [拥有相同数字] is predicted for are labelled, not filtered
    // out.
    let mut opts: Vec<ctx::TileOption> = Vec::new();
    for t in 0..n {
        // TODO(规则书)（拥有相同数字）: 「拥有相同数字」 is undefined -- all
        //   digits equal (1111) or any repeated digit is open
        //   (OPEN-C-SUMIMI-nobreakup-digits); we reuse the gate's any-repeat
        //   predicate.
        let label = if t != pos && repeated_digits(predict(player_id, t)) {
            Msg::new(key!("no_breakup_same_digits"))
        } else {
            Msg::new("")
        };
        opts.push(ctx::TileOption { tile: t, label });
    }
    let answered = ctx::ask_tile_index(
        player_id,
        &Msg::new(key!("no_breakup_title")),
        &Msg::new(key!("no_breakup_ask")),
        &opts,
    )?;
    let to = opts.get(answered).map_or(pos.max(0), |o| o.tile);
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_settles(true);
    // Re-issued on every replay; the identical plan makes the journal replay
    // its answer instead of running the move (and the settle's rent) again.
    ctx::run_planned_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("no_breakup_moved"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    Ok(())
}

/// 「未能使资金变为拥有相同数字」 -- the shot failed on a live seat.
fn failed_shot(player_id: i32) -> bool {
    ctx::slot(player_id, ARMED) > 0
        && !ctx::player_out(player_id)
        && !repeated_digits(ctx::money_of(player_id))
}

/// The success terminal: the same predicate inverted.
fn kept_shot(player_id: i32) -> bool {
    ctx::slot(player_id, ARMED) > 0 && !failed_shot(player_id)
}

/// Finish a shot: disarm, and mark the checkpoint judged so a re-run of the
/// play body does not arm it again. Both terminals do this **after** their
/// requests: the drive re-runs the body from the top after a host request, and
/// the entry's guard is `slot(ARMED) > 0` -- disarming before the pause would
/// make the resume pass fall through without finishing the body.
fn finish(player_id: i32, checkpoint: i32) {
    ctx::set_slot(player_id, ARMED, 0);
    ctx::set_slot(player_id, JUDGED, checkpoint);
}

/// 规则书: 「回到原处并取消所有受到的效果」 -- the restore is a request, so it
/// lands on the host world and replays as a skip; the body's replay does not
/// teleport again (the move's own request replays too), so the restored square
/// stands.
fn revert(player_id: i32) -> card_sdk::Asked {
    // `BASE` and `BASE + 1` are the two keys the play took; the restore brings
    // them (and the armed latch) back, so the re-runs of this body read the
    // same pair and the same checkpoint every pass.
    let base = ctx::slot(player_id, BASE);
    let checkpoint = base + 1;
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::world_restore(checkpoint)?;
    ctx::world_snapshot_drop(base)?;
    ctx::world_snapshot_drop(checkpoint)?;
    finish(player_id, checkpoint);
    ctx::log(
        player_id,
        &Msg::new(key!("no_breakup_reverted")).player_id("who", player_id),
    );
    Ok(())
}

fn settle_after(player_id: i32) -> card_sdk::Asked {
    let base = ctx::slot(player_id, BASE);
    let checkpoint = base + 1;
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::world_snapshot_drop(base)?;
    ctx::world_snapshot_drop(checkpoint)?;
    finish(player_id, checkpoint);
    Ok(())
}

//
// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn no_breakup_kani(player_id: i32) -> card_sdk::Asked {
    no_breakup(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn revert_kani(player_id: i32) -> card_sdk::Asked {
    revert(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn settle_after_kani(player_id: i32) -> card_sdk::Asked {
    settle_after(player_id)
}
