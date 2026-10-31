//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Sumimi不会解散哦`）:
//! > Sumimi不会解散哦：
//! >  当你本回合未进行过抵押/赎回操作且资金不含有相同数字时可打出，进行一次可使你的资金变动为拥有相同数字的传送，视为你的主要移动；若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果
//!
//! settle-teleport that must leave money with a repeated digit, else fully
//! revert (the revert half is not wired -- see the TODO R3 below).

use alloc::vec::Vec;

use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const NO_BREAKUP: CardDef = CardDef::new(
    "Sumimi:Sumimi不会解散哦",
    &[On::Play(
        // The repeated-digit clause stays in the residual guard (not `condition`):
        // a play gate must not trade its specific refusal
        // (`no_breakup_repeated`) for the generic `err.play_pre` a pre-reject
        // surfaces (GUARDS.md §4.4).
        "",
        Some(cant_play),
        no_breakup)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Sumimi不会解散哦"#)
        .raw_name(r#"Sumimi不会解散哦"#)
        .tags(&[])
        .text(CardText::para(r#"当你本回合未进行过抵押/赎回操作且资金不含有相同数字时可打出，进行一次可使你的资金变动为拥有相同数字的传送，视为你的主要移动；若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果"#))
        // sheet cell L3
        .fields(&[("group", r#"Sumimi"#), ("owner", "")]));

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
    // TODO(规则书) R3 (`SETTLE-STAGES.md` §9) [judgement]: is 「触发结算后」 `settleAfter` or
    // the `tileResolved` terminal? "取消所有受到的效果" (undo everything the settle did) is
    // easier at `tileResolved` (complete-as-nothing) than after the body has run.
    // Left as-is pending the ruling.
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
