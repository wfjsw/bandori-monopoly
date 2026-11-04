//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Sumimi不会解散哦`）:
//! > Sumimi不会解散哦：
//! >  当你本回合未进行过抵押/赎回操作且资金不含有相同数字时可打出，进行一次可使你的资金变动为拥有相同数字的传送，视为你的主要移动；若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果
//!
//! settle-teleport that must leave money with a repeated digit, else fully
//! revert (the revert half is not wired -- see the TODO R3 below).

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
        On::Action(Action::TileResolved(
            "slot('nb_shot') > 0 && tile.id >= 0 && settle.tile == tile.id",
            Some(failed_shot),
            revert,
        )),
        On::Action(Action::TileResolved(
            "slot('nb_shot') > 0 && tile.id >= 0 && settle.tile == tile.id",
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

/// The armed shot: the snapshot key of the play that is still unresolved
/// (0 = none). A slot, not an instance prop: the `tileResolved` hook is a
/// different body from the play.
const SLOT_SHOT: &str = "nb_shot";

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
    // terminal restores. Taken before the ask; the key rides the slot so the
    // `tileResolved` body can still name it.
    let key = ctx::world_snapshot()?;
    ctx::set_slot(player_id, SLOT_SHOT, key);
    let n = ctx::tile_count();
    let pos = ctx::player_pos(player_id);
    if n <= 0 {
        return Ok(());
    }
    let mut all: Vec<i32> = Vec::new();
    let mut good: Vec<i32> = Vec::new();
    for t in 0..n {
        if t == pos {
            continue;
        }
        all.push(t);
        // TODO(规则书)（拥有相同数字）: 「拥有相同数字」 is undefined -- all
        //   digits equal (1111) or any repeated digit is open
        //   (OPEN-C-SUMIMI-nobreakup-digits); we reuse the gate's any-repeat
        //   predicate.
        if repeated_digits(predict(player_id, t)) {
            good.push(t);
        }
    }
    if all.is_empty() {
        return Ok(());
    }
    let picks: &[i32] = if good.is_empty() { &all } else { &good };
    let to = ctx::ask_tile(
        player_id,
        &Msg::new(key!("no_breakup_title")),
        &Msg::new(key!("no_breakup_ask")),
        picks,
    )?;
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_settles(true);
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
    !ctx::player_out(player_id) && !repeated_digits(ctx::money_of(player_id))
}

/// The success terminal: the same predicate inverted.
fn kept_shot(player_id: i32) -> bool {
    !failed_shot(player_id)
}

/// Clear the slot and answer its key; only one terminal runs.
fn take_key(player_id: i32) -> i32 {
    let key = ctx::slot(player_id, SLOT_SHOT);
    ctx::set_slot(player_id, SLOT_SHOT, 0);
    key
}

/// 规则书: 「回到原处并取消所有受到的效果」 -- absolute restore, then drop the
/// checkpoint.
fn revert(player_id: i32) -> card_sdk::Asked {
    let key = take_key(player_id);
    ctx::world_restore(key)?;
    ctx::world_snapshot_drop(key)?;
    ctx::log(
        player_id,
        &Msg::new(key!("no_breakup_reverted")).player_id("who", player_id),
    );
    Ok(())
}

fn settle_after(player_id: i32) -> card_sdk::Asked {
    ctx::world_snapshot_drop(take_key(player_id))?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn no_breakup_kani(player_id: i32) -> card_sdk::Asked {
    no_breakup(player_id)
}
