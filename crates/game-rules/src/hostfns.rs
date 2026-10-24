//! Backend-neutral `ruleset` host import bodies.
//!
//! Extracted once from `host.rs`'s `build_linker` closures and maintained by
//! hand since. Every import is a plain function over [`HostCtx`]; the sandbox
//! linker (`host.rs`) and the native backend (`rules-native`) both call these,
//! so the two cannot drift in behaviour -- only in the memory / fuel plumbing
//! behind `HostCtx`.
//!
//! Errors are [`HostErr`] (not the wasm engine's `Error`) so this module has
//! no wasmi/wasmtime dependency; each backend converts at its boundary.

use card_sdk::abi::{self, export, OnKind, PromptKind};

use crate::host::{
    engine_msg, CallOut, GuestMem, HostCtx, HostErr, HostRequest, HostState, Prompt, PromptOption,
    RulesHandle, MAX_NESTING,
};
use crate::world::CardWorld;
use crate::{AbKind, Msg};

/// Decode a guest `Msg` (ABI v5: every text parameter is a `postcard` `Msg`).
pub fn guest_msg<C: HostCtx>(c: &mut C, ptr: i32, len: i32) -> Result<Msg, HostErr> {
    let bytes = c.read_guest(ptr, len)?;
    let m: card_sdk::msg::Msg = postcard::from_bytes(&bytes)
        .map_err(|_| HostErr::trap("guest message is not Msg postcard"))?;
    Ok(engine_msg(m))
}

/// Decode a guest UTF-8 string.
pub fn guest_str<C: HostCtx>(c: &mut C, ptr: i32, len: i32) -> Result<String, HostErr> {
    let bytes = c.read_guest(ptr, len)?;
    String::from_utf8(bytes).map_err(|_| HostErr::trap("guest string is not UTF-8"))
}

/// The abnormal-move counteraction gate, from inside a card run (see `host.rs`).
pub fn abnormal_gate<C: HostCtx>(c: &mut C, player_id: i32, kind: AbKind) -> Result<bool, HostErr> {
    let v = crate::inline::request_or_pause(
        c,
        HostRequest::Gate { player_id, kind },
        crate::inline::Pause::Trap,
    )?;
    Ok(v != 0)
}

pub fn roll<C: HostCtx>(c: &mut C, player_id: i32, count: i32, sides: i32) -> Result<i32, HostErr> {
    if !(1..=100).contains(&count) || !(1..=1000).contains(&sides) {
        return Err(HostErr::trap(format!("roll({count}d{sides}) out of range")));
    }
    Ok(c.st_mut().w().roll(player_id, count, sides))
}

pub fn log<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<(), HostErr> {
    let msg = guest_msg(c, p, n)?;
    c.st_mut().w().log(player_id, msg);
    Ok(())
}

pub fn effect<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<(), HostErr> {
    let msg = guest_msg(c, p, n)?;
    c.st_mut().w().effect(player_id, msg);
    Ok(())
}

// -- flash (v55) -------------------------------------------------------------

/// `ctx::flash` -- request the activation's flash at this point.
pub fn flash<C: HostCtx>(c: &mut C) -> Result<(), HostErr> {
    c.st_mut().w().flash();
    Ok(())
}

/// `ctx::no_flash` -- suppress the activation's flash (runtime decision).
pub fn no_flash<C: HostCtx>(c: &mut C) -> Result<(), HostErr> {
    c.st_mut().w().no_flash();
    Ok(())
}

/// `ctx::flash_card` -- proactive "look here" flash of another card.
pub fn flash_card<C: HostCtx>(
    c: &mut C,
    cptr: i32,
    cl: i32,
    p: i32,
    n: i32,
) -> Result<(), HostErr> {
    let card_id = guest_str(c, cptr, cl)?;
    let msg = guest_msg(c, p, n)?;
    c.st_mut().w().flash_card(&card_id, msg);
    Ok(())
}

/// `ctx::flash_card_keyed` -- proactive flash with an explicit identity key.
pub fn flash_card_keyed<C: HostCtx>(
    c: &mut C,
    kp: i32,
    kl: i32,
    cptr: i32,
    cl: i32,
    p: i32,
    n: i32,
) -> Result<(), HostErr> {
    let key = guest_str(c, kp, kl)?;
    let card_id = guest_str(c, cptr, cl)?;
    let msg = guest_msg(c, p, n)?;
    c.st_mut().w().flash_card_keyed(&key, &card_id, msg);
    Ok(())
}

/// `ctx::flash_tile` -- proactive "look here" flash of a tile.
pub fn flash_tile<C: HostCtx>(c: &mut C, tile: i32, p: i32, n: i32) -> Result<(), HostErr> {
    let msg = guest_msg(c, p, n)?;
    c.st_mut().w().flash_tile(tile, msg);
    Ok(())
}

pub fn tile_count<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().tile_count())
}

/// Bind `count` units of the running instance's counter `kind` to `tile`.
/// Returns the mark row's uid (the new row's, or the merged row's), or `-1`
/// when `count <= 0`.
pub fn place_mark<C: HostCtx>(
    c: &mut C,
    tile: i32,
    kp: i32,
    kl: i32,
    cptr: i32,
    cl: i32,
    owner: i32,
    src: i32,
    count: i32,
    stack: i32,
    np: i32,
    nl: i32,
) -> Result<i32, HostErr> {
    let kind = guest_str(c, kp, kl)?;
    let category = guest_str(c, cptr, cl)?;
    let note = guest_msg(c, np, nl)?;
    let fresh = stack != 0; // Stack::Fresh = 1
    Ok(c.st_mut()
        .w()
        .place_mark(tile, &kind, &category, owner, src, count, note, fresh))
}

/// Place **one** paired mark spanning tiles `a` and `b` (one row per tile,
/// `partner` cross-linked). Returns 1 on placement, 0 on the atomic refusal
/// (either end already carries a matching mark for that owner).
#[allow(clippy::too_many_arguments)]
pub fn place_mark_pair<C: HostCtx>(
    c: &mut C,
    a: i32,
    b: i32,
    kp: i32,
    kl: i32,
    cptr: i32,
    cl: i32,
    owner: i32,
    src: i32,
    count: i32,
    stack: i32,
    np: i32,
    nl: i32,
) -> Result<i32, HostErr> {
    let kind = guest_str(c, kp, kl)?;
    let category = guest_str(c, cptr, cl)?;
    let note = guest_msg(c, np, nl)?;
    let fresh = stack != 0; // Stack::Fresh = 1
    Ok(c.st_mut()
        .w()
        .place_mark_pair(a, b, &kind, &category, owner, src, count, note, fresh) as i32)
}

fn guest_filter<C: HostCtx>(
    c: &mut C,
    fp: i32,
    fl: i32,
) -> Result<game_core::state::MarkFilter<'static>, HostErr> {
    let bytes = c.read_guest(fp, fl)?;
    let f: card_sdk::abi::MarkFilter = postcard::from_bytes(&bytes)
        .map_err(|_| HostErr::trap("guest mark filter is not MarkFilter postcard"))?;
    // The guest filter's `-2` means "any"; the host filter uses `None`.
    let any = |v: i32| if v == -2 { None } else { Some(v) };
    Ok(game_core::state::MarkFilter {
        kind: Box::leak(f.kind.into_boxed_str()),
        category: Box::leak(f.category.into_boxed_str()),
        owner: any(f.owner),
        src: any(f.src),
        instance: any(f.instance),
    })
}

pub fn count_marks_f<C: HostCtx>(c: &mut C, tile: i32, fp: i32, fl: i32) -> Result<i32, HostErr> {
    let f = guest_filter(c, fp, fl)?;
    Ok(c.st().wr().count_marks(tile, &f))
}

pub fn bump_mark_f<C: HostCtx>(
    c: &mut C,
    tile: i32,
    fp: i32,
    fl: i32,
    delta: i32,
) -> Result<i32, HostErr> {
    let f = guest_filter(c, fp, fl)?;
    Ok(c.st_mut().w().bump_mark(tile, &f, delta))
}

pub fn remove_marks_f<C: HostCtx>(c: &mut C, tile: i32, fp: i32, fl: i32) -> Result<i32, HostErr> {
    let f = guest_filter(c, fp, fl)?;
    Ok(c.st_mut().w().remove_marks(tile, &f))
}

pub fn mark_src_at<C: HostCtx>(c: &mut C, tile: i32, fp: i32, fl: i32) -> Result<i32, HostErr> {
    let f = guest_filter(c, fp, fl)?;
    Ok(c.st().wr().mark_src_at(tile, &f))
}

pub fn mark_instance_at<C: HostCtx>(
    c: &mut C,
    tile: i32,
    fp: i32,
    fl: i32,
) -> Result<i32, HostErr> {
    let f = guest_filter(c, fp, fl)?;
    Ok(c.st().wr().mark_instance_at(tile, &f))
}

/// Partner tile of the first matching mark on `tile`, or -1.
pub fn mark_partner<C: HostCtx>(c: &mut C, tile: i32, fp: i32, fl: i32) -> Result<i32, HostErr> {
    let f = guest_filter(c, fp, fl)?;
    Ok(c.st().wr().mark_partner(tile, &f))
}

/// The marked tile of the row at `uid`, or -1 when no such row.
pub fn mark_tile<C: HostCtx>(c: &mut C, uid: i32) -> Result<i32, HostErr> {
    Ok(c.st().wr().mark_tile_by_uid(uid))
}

/// Drop the exact mark row at `uid`. Returns 1 when that row was present.
pub fn remove_mark<C: HostCtx>(c: &mut C, uid: i32) -> Result<i32, HostErr> {
    Ok(c.st_mut().w().remove_mark_by_uid(uid) as i32)
}

pub fn counter_self<C: HostCtx>(c: &mut C, np: i32, nl: i32) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    Ok(c.st().wr().counter(&name))
}

pub fn add_counter_self<C: HostCtx>(
    c: &mut C,
    np: i32,
    nl: i32,
    n: i32,
    max: i32,
) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    Ok(c.st_mut().w().add_counter(&name, n, max))
}

pub fn set_counter_self<C: HostCtx>(c: &mut C, np: i32, nl: i32, n: i32) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    Ok(c.st_mut().w().set_counter(&name, n))
}

pub fn counter_at<C: HostCtx>(c: &mut C, uid: i32, np: i32, nl: i32) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    Ok(c.st().wr().counter_at(uid, &name))
}

pub fn add_counter_at<C: HostCtx>(
    c: &mut C,
    uid: i32,
    np: i32,
    nl: i32,
    n: i32,
    max: i32,
) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    Ok(c.st_mut().w().add_counter_at(uid, &name, n, max))
}

pub fn card_counter<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    cptr: i32,
    cl: i32,
    np: i32,
    nl: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cptr, cl)?;
    let name = guest_str(c, np, nl)?;
    Ok(c.st().wr().card_counter(player_id, &card, &name))
}

pub fn add_card_counter<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    cptr: i32,
    cl: i32,
    np: i32,
    nl: i32,
    n: i32,
    max: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cptr, cl)?;
    let name = guest_str(c, np, nl)?;
    Ok(c.st_mut()
        .w()
        .add_card_counter(player_id, &card, &name, n, max))
}

pub fn count_held<C: HostCtx>(c: &mut C, np: i32, nl: i32, player_id: i32) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    Ok(c.st().wr().count_held(&name, player_id))
}

pub fn add_held<C: HostCtx>(
    c: &mut C,
    np: i32,
    nl: i32,
    player_id: i32,
    n: i32,
    max: i32,
) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    // Marker window (user ruling 2026-10-07): `markerSpend` / `markerGain` open
    // before the counters move. Sentinel pause, as `add_tok` did -- the guest
    // wrapper returns `Result<i32, Prompt>` and a cancelled link surfaces there.
    if n != 0 {
        let ok = crate::inline::request_or_pause(
            c,
            crate::HostRequest::Marker {
                player_id,
                name: name.clone(),
                delta: n,
            },
            crate::inline::Pause::Sentinel,
        )?;
        if ok != 1 {
            return Ok(ok);
        }
    }
    Ok(c.st_mut().w().add_held(&name, player_id, n, max))
}

pub fn count_held_name<C: HostCtx>(
    c: &mut C,
    np: i32,
    nl: i32,
    player_id: i32,
) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    Ok(c.st().wr().count_held_name(&name, player_id))
}

pub fn set_held_name<C: HostCtx>(
    c: &mut C,
    np: i32,
    nl: i32,
    player_id: i32,
    v: i32,
    max: i32,
    instance: i32,
) -> Result<(), HostErr> {
    let name = guest_str(c, np, nl)?;
    c.st_mut()
        .w()
        .set_held_name(&name, player_id, v, max, instance);
    Ok(())
}

/// Live ceiling stored on the held row (`0` = uncapped / undeclared).
pub fn tok_max<C: HostCtx>(c: &mut C, np: i32, nl: i32, player_id: i32) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    Ok(c.st().wr().tok_max(player_id, &name))
}

pub fn move_units<C: HostCtx>(
    c: &mut C,
    np: i32,
    nl: i32,
    from_tile: i32,
    from_player: i32,
    to_tile: i32,
    to_player: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    Ok(c.st_mut()
        .w()
        .move_units(&name, from_tile, from_player, to_tile, to_player, n))
}

pub fn self_uid<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().self_uid())
}

pub fn money<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().money(player_id) })
}

pub fn gain<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    amount: i32,
    p: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let src = guest_msg(c, p, n)?;
    // A card-driven gain runs the same `Money` pipeline as a payment --
    // print (game -> player) -- so PaySplitAdd / PaySplitChoose / the `effect`
    // counteraction window all see it. The pause/resume is the same as `pay`:
    // the engine answers with the amount that actually moved.
    {
        let req = HostRequest::Pay {
            from: -1,
            to: player_id,
            amount,
            src: Some(src),
            total_stage: true,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

pub fn gain_typed<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    amount: i32,
    tp: i32,
    tl: i32,
    xp: i32,
    xl: i32,
) -> Result<i32, HostErr> {
    let typ = guest_str(c, tp, tl)?;
    let text = guest_msg(c, xp, xl)?;
    {
        let req = HostRequest::GainTyped {
            player_id,
            amount,
            typ,
            text: Some(text),
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

/// Raw money adjustment (`ctx::adjust_money` / `gain_raw` / `lose_raw`):
/// `money += delta` with a `log.gain` / `log.lose` line and **no** payment
/// pipeline (no `paySplitAdd`, no `effect` window, no `payAfter`). 规则书
/// 「不受其他任何效果影响」 -- the amount moves as-is.
pub fn adjust_money<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    delta: i32,
    p: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let src = guest_msg(c, p, n)?;
    // Non-pausing: the inline host may intercept (bot / simulation); otherwise
    // apply straight to the world copy, like `set_slot`.
    if let Some(v) = crate::inline::request_or_inline(
        c,
        HostRequest::AdjustMoney {
            player_id,
            delta,
            src: Some(src.clone()),
        },
    )? {
        return Ok(v);
    }
    Ok(c.st_mut().w().adjust_money_raw(player_id, delta, src))
}

pub fn pay<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    amount: i32,
    p: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let src = guest_msg(c, p, n)?;
    // A card-driven payment is paused so the engine can raise a `pay`
    // trigger (and the counteraction window) before the money moves. The answer is
    // the final amount to move -- 0 cancels the payment outright.
    //
    // The pause is the sentinel **as the return value**, not a trap:
    // `ctx::pay` reads it through `asked()` and hands the card
    // `Err(Prompt)`. Trapping here would tear the stack before the card
    // ever saw a `Result`.
    {
        let req = HostRequest::Pay {
            from: player_id,
            to: -1,
            amount,
            src: Some(src),
            total_stage: true,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

pub fn pay_to<C: HostCtx>(
    c: &mut C,
    from: i32,
    to: i32,
    amount: i32,
    p: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let src = guest_msg(c, p, n)?;
    {
        let req = HostRequest::Pay {
            from,
            to,
            amount,
            src: Some(src),
            total_stage: true,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

pub fn pay_total<C: HostCtx>(
    c: &mut C,
    from: i32,
    to: i32,
    amount: i32,
    p: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let src = guest_msg(c, p, n)?;
    {
        let req = HostRequest::PayTotal {
            from,
            to,
            amount,
            src: Some(src),
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

pub fn pay_leg<C: HostCtx>(
    c: &mut C,
    from: i32,
    to: i32,
    amount: i32,
    p: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let src = guest_msg(c, p, n)?;
    {
        let req = HostRequest::Pay {
            from,
            to,
            amount,
            src: Some(src),
            total_stage: false,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

pub fn player_count<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().player_count())
}

pub fn player_out<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().player_out(player_id) })
}

pub fn others_count<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().others_count(player_id) })
}

pub fn others_at<C: HostCtx>(c: &mut C, player_id: i32, index: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().others_at(player_id, index) })
}

pub fn tile_named<C: HostCtx>(c: &mut C, p: i32, n: i32) -> Result<i32, HostErr> {
    let name = guest_str(c, p, n)?;
    Ok(c.st().wr().tile_named(&name))
}

pub fn tile_owner<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().tile_owner(tile) })
}

pub fn player_pos<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().player_pos(player_id) })
}

pub fn tile_steps_ahead<C: HostCtx>(c: &mut C, player_id: i32, steps: i32) -> Result<i32, HostErr> {
    Ok(c.st().wr().tile_steps_ahead(player_id, steps))
}

pub fn rent_of<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().rent_of(tile) })
}

pub fn buy_price<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().buy_price(tile) })
}

pub fn build_cost<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().build_cost(tile) })
}

pub fn mortgage_value<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().mortgage_value(tile) })
}

pub fn owned_count<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().owned_count(player_id) })
}

pub fn owned_at<C: HostCtx>(c: &mut C, player_id: i32, index: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().owned_at(player_id, index) })
}

/// `tile_is(tile, kind_ptr, kind_len) -> i32` -- data kind by wire spelling.
/// Generic kind probe by ruleset tile-kind spelling.
pub fn tile_is<C: HostCtx>(c: &mut C, tile: i32, kp: i32, kl: i32) -> Result<i32, HostErr> {
    let kind = guest_str(c, kp, kl)?;
    Ok(c.st().wr().tile_is(tile, &kind) as i32)
}

/// `tile_attr(tile, attr_ptr, attr_len) -> i32` -- generic field read.
/// Replaces the attr family of `tile_group` / `tile_price` (those remain as
/// sugar). `kind` / `color` strings are `tile_is` / future string attrs.
pub fn tile_attr<C: HostCtx>(c: &mut C, tile: i32, ap: i32, al: i32) -> Result<i32, HostErr> {
    let attr = guest_str(c, ap, al)?;
    Ok(c.st().wr().tile_attr(tile, &attr))
}

/// `tile_in(player_id, tile, class_ptr, class_len) -> i32` -- class membership.
/// Generic class probe by ruleset class-name binding.
pub fn tile_in<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    cptr: i32,
    cl: i32,
) -> Result<i32, HostErr> {
    let class = guest_str(c, cptr, cl)?;
    Ok(c.st().wr().tile_in(player_id, tile, &class) as i32)
}

pub fn paid_in_settle<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().paid_in_settle() })
}

pub fn turn_rolls<C: HostCtx>(c: &mut C, buf: i32, cap: i32) -> Result<i32, HostErr> {
    let rolls = c.st().wr().turn_rolls();
    let bytes = postcard::to_allocvec(&rolls)
        .map_err(|e| HostErr::trap(format!("turn_rolls encode: {e}")))?;
    if bytes.len() as i32 <= cap {
        c.write_guest(buf, &bytes)?;
    }
    Ok(bytes.len() as i32)
}

pub fn field_instances<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    buf: i32,
    cap: i32,
) -> Result<i32, HostErr> {
    let inst = c.st().wr().field_instances(player_id);
    let bytes = postcard::to_allocvec(&inst)
        .map_err(|e| HostErr::trap(format!("field_instances encode: {e}")))?;
    if bytes.len() as i32 <= cap {
        c.write_guest(buf, &bytes)?;
    }
    Ok(bytes.len() as i32)
}

pub fn unplace_at<C: HostCtx>(c: &mut C, uid: i32) -> Result<i32, HostErr> {
    Ok({ c.st_mut().w().unplace_at(uid) })
}

pub fn tile_at<C: HostCtx>(c: &mut C, uid: i32) -> Result<i32, HostErr> {
    Ok(c.st().wr().tile_at(uid))
}

pub fn set_tile_at<C: HostCtx>(c: &mut C, uid: i32, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st_mut().w().set_tile_at(uid, tile) as i32 })
}

pub fn is_face_down_at<C: HostCtx>(c: &mut C, uid: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().is_face_down_at(uid) as i32 })
}

pub fn set_face_down_at<C: HostCtx>(c: &mut C, uid: i32, on: i32) -> Result<i32, HostErr> {
    Ok({ c.st_mut().w().set_face_down_at(uid, on != 0) as i32 })
}

pub fn is_immune_at<C: HostCtx>(c: &mut C, uid: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().is_immune_at(uid) as i32 })
}

pub fn set_immune_at<C: HostCtx>(c: &mut C, uid: i32, on: i32) -> Result<i32, HostErr> {
    Ok({ c.st_mut().w().set_immune_at(uid, on != 0) as i32 })
}

pub fn set_build_discount<C: HostCtx>(c: &mut C, n: i32, layers: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_build_discount(n, layers);
    Ok(())
}

pub fn set_build_cost_pct<C: HostCtx>(c: &mut C, pct: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_build_cost_pct(pct);
    Ok(())
}

pub fn turn_start_pos<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().turn_start_pos(player_id) })
}

pub fn turn_snap<C: HostCtx>(c: &mut C, player_id: i32, buf: i32) -> Result<i32, HostErr> {
    let snap = c.st().wr().turn_snap(player_id);
    let bytes = postcard::to_allocvec(&snap).unwrap_or_default();
    let n = bytes.len() as i32;
    c.write_guest(buf, &bytes)?;
    Ok(n)
}

pub fn tile_group<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().tile_group(tile) })
}

pub fn tile_price<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().tile_price(tile) })
}

pub fn houses_of<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().houses_of(tile) })
}

pub fn rent_houses_of<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().rent_houses_of(tile) })
}

pub fn set_houses<C: HostCtx>(c: &mut C, tile: i32, n: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_houses(tile, n);
    Ok(())
}

pub fn add_house<C: HostCtx>(c: &mut C, tile: i32, n: i32) -> Result<i32, HostErr> {
    Ok({ c.st_mut().w().add_house(tile, n) })
}

pub fn mortgaged_of<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().mortgaged_of(tile) })
}

pub fn set_mortgaged<C: HostCtx>(c: &mut C, tile: i32, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_mortgaged(tile, v);
    Ok(())
}

pub fn set_owner<C: HostCtx>(c: &mut C, tile: i32, player_id: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_owner(tile, player_id);
    Ok(())
}

pub fn dist<C: HostCtx>(c: &mut C, a: i32, b: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().dist(a, b) })
}

pub fn tile_forward<C: HostCtx>(c: &mut C, a: i32, b: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().tile_forward(a, b) })
}

pub fn neighbor<C: HostCtx>(c: &mut C, player_id: i32, dir: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().neighbor(player_id, dir) })
}

pub fn players_on_count<C: HostCtx>(c: &mut C, tile: i32, except: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().players_on_count(tile, except) })
}

pub fn players_on_at<C: HostCtx>(
    c: &mut C,
    tile: i32,
    except: i32,
    index: i32,
) -> Result<i32, HostErr> {
    Ok({ c.st().wr().players_on_at(tile, except, index) })
}

pub fn draw<C: HostCtx>(c: &mut C, player_id: i32, n: i32) -> Result<i32, HostErr> {
    // Negative replies encode draws the engine already applied, including
    // replacements, refill and after-hooks. Guest replay only returns their
    // count; the fresh world copy already contains the resulting piles.
    // Positive replies remain supported by standalone host fixtures.
    fn apply_reply<C: HostCtx>(c: &mut C, player_id: i32, n: i32, reply: i32) -> i32 {
        if reply < 0 {
            (-(reply + 1)).clamp(0, n.max(0))
        } else {
            c.st_mut().w().draw(player_id, reply.clamp(0, n.max(0)))
        }
    }
    {
        let st = c.st_mut();
        if let Some(&reply) = st.answers.get(st.next_answer) {
            let answer = st.next_answer;
            st.w().after_host(answer);
            st.next_answer += 1;
            return Ok(apply_reply(c, player_id, n, reply));
        }
    }
    if let Some(reply) = crate::inline::request_or_inline(c, HostRequest::Draw { player_id, n })? {
        return Ok(apply_reply(c, player_id, n, reply));
    }
    c.st_mut().host_request = Some(HostRequest::Draw { player_id, n });
    Ok(abi::EXIT_NEED_INPUT)
}

pub fn draw_event<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    {
        let req = HostRequest::DrawEvent { player_id };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

pub fn pay_rent<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    half: i32,
) -> Result<i32, HostErr> {
    {
        let req = HostRequest::PayRent {
            player_id,
            tile,
            half: half != 0,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

/// `charge_rent(payer, tile, milli_scale) -> paid` -- a rent-shaped 「收费」 at
/// `milli_scale` (1000 = ×1.0) of the tile's ordinary-settle rent, outside a
/// full [触发结算] (`ctx::charge_rent`). Answers the amount actually paid
/// (0 = refused / cancelled).
pub fn charge_rent<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    milli_scale: i32,
) -> Result<i32, HostErr> {
    {
        let req = HostRequest::ChargeRent {
            player_id,
            tile,
            milli_scale,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

pub fn offer_buy<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<i32, HostErr> {
    {
        let req = HostRequest::OfferBuy { player_id, tile };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

pub fn offer_force_buy<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<i32, HostErr> {
    {
        let req = HostRequest::OfferForceBuy { player_id, tile };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

pub fn offer_build<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<i32, HostErr> {
    {
        let req = HostRequest::OfferBuildOne { player_id, tile };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel);
    }
}

pub fn add_to_dest<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    p: i32,
    n: i32,
    dest: i32,
) -> Result<(), HostErr> {
    // Field / Banished are not piles; `place_card` / "gone" is the right verb.
    if !matches!(dest, 0 | 1 | 4 | 5 | 6) {
        return Err(HostErr::trap(format!("add_to_dest: dest {dest} is not a pile")));
    }
    let card = guest_str(c, p, n)?;
    c.st_mut().w().add_to_dest(player_id, &card, dest);
    Ok(())
}

pub fn take_card<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    p: i32,
    n: i32,
    src: i32,
) -> Result<i32, HostErr> {
    let src = crate::CardPile::from_i32(src)
        .ok_or_else(|| HostErr::trap(format!("bad card pile {src}")))?;
    let id = guest_str(c, p, n)?;
    Ok(c.st_mut().w().take_card(player_id, &id, src) as i32)
}

pub fn cards_in<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    pile: i32,
    buf: i32,
    cap: i32,
) -> Result<i32, HostErr> {
    let pile = crate::CardPile::from_i32(pile)
        .ok_or_else(|| HostErr::trap(format!("bad card pile {pile}")))?;
    let list = c.st().wr().cards_in(player_id, pile);
    let bytes =
        postcard::to_allocvec(&list).map_err(|e| HostErr::trap(format!("cards_in encode: {e}")))?;
    if bytes.len() as i32 <= cap {
        c.write_guest(buf, &bytes)?;
    }
    Ok(bytes.len() as i32)
}

pub fn to_discard<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<(), HostErr> {
    let card = guest_str(c, p, n)?;
    c.st_mut().w().to_discard(player_id, &card);
    Ok(())
}

pub fn hand_count<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<i32, HostErr> {
    let card = guest_str(c, p, n)?;
    Ok(c.st().wr().hand_count(player_id, &card))
}

pub fn discard_count<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    p: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, p, n)?;
    Ok(c.st().wr().discard_count(player_id, &card))
}

pub fn hand_size<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().hand_size(player_id) })
}

pub fn deck_count<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().deck_count(player_id) })
}

pub fn discard_size<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().discard_size(player_id) })
}

pub fn discard_from_hand<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    p: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, p, n)?;
    Ok(c.st_mut().w().discard_from_hand(player_id, &card))
}

pub fn shuffle_into_deck<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    hand: i32,
    discard: i32,
) -> Result<i32, HostErr> {
    Ok({
        c.st_mut()
            .w()
            .shuffle_into_deck(player_id, hand != 0, discard != 0)
    })
}

pub fn unplace_card<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st_mut().w().unplace_card() })
}

pub fn event_expire<C: HostCtx>(c: &mut C, ip: i32, il: i32, removed: i32) -> Result<(), HostErr> {
    let id = guest_str(c, ip, il)?;
    c.st_mut().w().event_expire(&id, removed != 0);
    Ok(())
}

pub fn event_is_active<C: HostCtx>(c: &mut C, ip: i32, il: i32) -> Result<i32, HostErr> {
    let id = guest_str(c, ip, il)?;
    Ok(c.st().wr().event_is_active(&id) as i32)
}

pub fn event_deck_push<C: HostCtx>(
    c: &mut C,
    ip: i32,
    il: i32,
    face_down: i32,
) -> Result<(), HostErr> {
    let id = guest_str(c, ip, il)?;
    c.st_mut().w().event_deck_push(&id, face_down != 0);
    Ok(())
}

pub fn event_banish<C: HostCtx>(c: &mut C, ip: i32, il: i32) -> Result<(), HostErr> {
    let id = guest_str(c, ip, il)?;
    c.st_mut().w().event_banish(&id);
    Ok(())
}

pub fn placed_cards<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    buf: i32,
    cap: i32,
) -> Result<i32, HostErr> {
    let names = c.st().wr().placed_cards(player_id);
    let bytes = postcard::to_allocvec(&names)
        .map_err(|e| HostErr::trap(format!("placed_cards encode: {e}")))?;
    if bytes.len() as i32 <= cap {
        c.write_guest(buf, &bytes)?;
    }
    Ok(bytes.len() as i32)
}

pub fn card_text_mentions<C: HostCtx>(
    c: &mut C,
    cptr: i32,
    cl: i32,
    np: i32,
    nl: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cptr, cl)?;
    let needle = guest_str(c, np, nl)?;
    Ok(c.st().wr().card_text_mentions(&card, &needle) as i32)
}

/// One declared static property of the card rule `id` (`CardDef::props`);
/// `0` when the card does not declare it. The generic guest face of
/// `CardRules::card_prop` / `Ruleset::card_prop`.
pub fn card_prop<C: HostCtx>(
    c: &mut C,
    ip: i32,
    il: i32,
    kp: i32,
    kl: i32,
) -> Result<i32, HostErr> {
    let id = guest_str(c, ip, il)?;
    let key = guest_str(c, kp, kl)?;
    let rules = c
        .st()
        .rules
        .clone()
        .ok_or_else(|| HostErr::trap("card_prop unavailable here"))?;
    Ok(rules.card_prop(&id, &key))
}

pub fn is_placed<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().is_placed())
}

pub fn self_tile<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().self_tile())
}

pub fn set_self_tile<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({ c.st_mut().w().set_self_tile(tile) as i32 })
}

pub fn self_face_down<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().self_face_down() as i32 })
}

pub fn set_self_face_down<C: HostCtx>(c: &mut C, on: i32) -> Result<i32, HostErr> {
    Ok({ c.st_mut().w().set_self_face_down(on != 0) as i32 })
}

pub fn self_immune<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().self_immune() as i32 })
}

pub fn set_self_immune<C: HostCtx>(c: &mut C, on: i32) -> Result<i32, HostErr> {
    Ok({ c.st_mut().w().set_self_immune(on != 0) as i32 })
}

pub fn self_prop<C: HostCtx>(c: &mut C, kp: i32, kl: i32) -> Result<i32, HostErr> {
    let key = guest_str(c, kp, kl)?;
    Ok(c.st().wr().self_prop(&key))
}

pub fn set_self_prop<C: HostCtx>(c: &mut C, kp: i32, kl: i32, v: i32) -> Result<i32, HostErr> {
    let key = guest_str(c, kp, kl)?;
    Ok(c.st_mut().w().set_self_prop(&key, v))
}

pub fn tile_prop<C: HostCtx>(c: &mut C, tile: i32, kp: i32, kl: i32) -> Result<i32, HostErr> {
    let key = guest_str(c, kp, kl)?;
    Ok(c.st().wr().tile_prop(tile, &key))
}

pub fn set_tile_prop<C: HostCtx>(
    c: &mut C,
    tile: i32,
    kp: i32,
    kl: i32,
    v: i32,
) -> Result<i32, HostErr> {
    let key = guest_str(c, kp, kl)?;
    Ok(c.st_mut().w().set_tile_prop(tile, &key, v))
}

pub fn prop_at<C: HostCtx>(c: &mut C, uid: i32, kp: i32, kl: i32) -> Result<i32, HostErr> {
    let key = guest_str(c, kp, kl)?;
    Ok(c.st().wr().prop_at(uid, &key))
}

pub fn set_prop_at<C: HostCtx>(
    c: &mut C,
    uid: i32,
    kp: i32,
    kl: i32,
    v: i32,
) -> Result<i32, HostErr> {
    let key = guest_str(c, kp, kl)?;
    Ok(c.st_mut().w().set_prop_at(uid, &key, v))
}

pub fn state_get<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    p: i32,
    n: i32,
    field: i32,
) -> Result<i32, HostErr> {
    let key = guest_str(c, p, n)?;
    Ok(match field {
        1 => c.st().wr().state_min(player_id, &key),
        2 => c.st().wr().state_max(player_id, &key),
        3 => match c.st().wr().state_expires(player_id, &key) {
            None => 0,
            Some(game_core::state::Tick::TurnStart) => 1,
            Some(game_core::state::Tick::TurnEnd) => 2,
        },
        4 => c.st().wr().state_var(player_id, &key).flags as i32,
        _ => c.st().wr().state_get(player_id, &key),
    })
}

pub fn state_set<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    p: i32,
    n: i32,
    field: i32,
    v: i32,
) -> Result<i32, HostErr> {
    let key = guest_str(c, p, n)?;
    Ok(match field {
        1 => {
            let max = c.st().wr().state_max(player_id, &key);
            c.st_mut().w().state_set_bounds(player_id, &key, v, max);
            v
        }
        2 => {
            let min = c.st().wr().state_min(player_id, &key);
            c.st_mut().w().state_set_bounds(player_id, &key, min, v);
            v
        }
        3 => {
            let e = match v {
                1 => Some(game_core::state::Tick::TurnStart),
                2 => Some(game_core::state::Tick::TurnEnd),
                _ => None,
            };
            c.st_mut().w().state_set_expires(player_id, &key, e);
            v
        }
        4 => {
            c.st_mut().w().state_set_flags(player_id, &key, v as u8);
            v
        }
        _ => c.st_mut().w().state_set(player_id, &key, v),
    })
}

pub fn state_add<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    p: i32,
    n: i32,
    delta: i32,
) -> Result<i32, HostErr> {
    let key = guest_str(c, p, n)?;
    Ok(c.st_mut().w().state_add(player_id, &key, delta))
}

// `group_pool` / `add_group_pool` remain on the guest surface: a named pool on
// the group-skill field instance (see `ops.rs` `group_skill_uid`).
pub fn group_pool<C: HostCtx>(c: &mut C, player_id: i32, np: i32, nl: i32) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    Ok(c.st().wr().group_pool(player_id, &name))
}

pub fn add_group_pool<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    np: i32,
    nl: i32,
    n: i32,
    max: i32,
) -> Result<i32, HostErr> {
    let name = guest_str(c, np, nl)?;
    Ok(c.st_mut().w().add_group_pool(player_id, &name, n, max))
}

pub fn group_skill<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    buf: i32,
    cap: i32,
) -> Result<i32, HostErr> {
    let s = c.st().wr().group_skill_id(player_id).unwrap_or_default();
    let bytes =
        postcard::to_allocvec(&s).map_err(|e| HostErr::trap(format!("group_skill encode: {e}")))?;
    if bytes.len() as i32 <= cap {
        c.write_guest(buf, &bytes)?;
    }
    Ok(bytes.len() as i32)
}

pub fn persona_skill<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    buf: i32,
    cap: i32,
) -> Result<i32, HostErr> {
    let s = c.st().wr().persona_skill_id(player_id).unwrap_or_default();
    let bytes = postcard::to_allocvec(&s)
        .map_err(|e| HostErr::trap(format!("persona_skill encode: {e}")))?;
    if bytes.len() as i32 <= cap {
        c.write_guest(buf, &bytes)?;
    }
    Ok(bytes.len() as i32)
}

pub fn group_skills<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    buf: i32,
    cap: i32,
) -> Result<i32, HostErr> {
    let list = c.st().wr().group_skills(player_id);
    let bytes = postcard::to_allocvec(&list)
        .map_err(|e| HostErr::trap(format!("group_skills encode: {e}")))?;
    if bytes.len() as i32 <= cap {
        c.write_guest(buf, &bytes)?;
    }
    Ok(bytes.len() as i32)
}

pub fn add_group_skill<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    p: i32,
    n: i32,
    extra: i32,
) -> Result<i32, HostErr> {
    let id = guest_str(c, p, n)?;
    Ok(c.st_mut().w().add_group_skill(player_id, &id, extra != 0))
}

pub fn invoke_skill<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<i32, HostErr> {
    let id = guest_str(c, p, n)?;
    let rules = c
        .st()
        .rules
        .clone()
        .ok_or_else(|| HostErr::trap("invoke_skill unavailable here"))?;
    let card = rules
        .by_id(&id)
        .ok_or_else(|| HostErr::trap(format!("invoke_skill: unknown skill {id:?}")))?;
    if c.st().depth >= MAX_NESTING {
        return Err(HostErr::trap(format!(
            "invoke_skill nested deeper than {MAX_NESTING}"
        )));
    }
    let fuel = c.fuel()?;
    let st = c.st_mut();
    let saved = st.w().enter_card(&id);
    let was_from_hand = st.w().play_from_hand();
    st.w().set_play_from_hand(false);
    let mut nested = HostState::new(
        rules.clone(),
        st.world.take().expect("world present"),
        std::mem::take(&mut st.answers),
        st.depth + 1,
    );
    nested.next_answer = st.next_answer;
    let entry = match rules.card(card).and_then(|ci| ci.entry(OnKind::Play, None)) {
        Some(e) => e,
        None => {
            let st = c.st_mut();
            st.world = nested.world;
            if let Some(w) = st.world.as_mut() {
                w.set_play_from_hand(was_from_hand);
            }
            st.answers = nested.answers;
            st.next_answer = nested.next_answer;
            let dest = st.w().leave_card(saved);
            c.set_fuel(fuel)?;
            return Ok(dest);
        }
    };
    let (res, inner, left) = c.call_entry(nested, card, entry, export::OP_RUN, player_id, &[], false);
    let st = c.st_mut();
    st.world = inner.world;
    if let Some(w) = st.world.as_mut() {
        w.set_play_from_hand(was_from_hand);
    }
    st.answers = inner.answers;
    st.next_answer = inner.next_answer;
    if inner.asked.is_some() {
        st.asked = inner.asked;
    }
    if inner.host_request.is_some() {
        st.host_request = inner.host_request;
    }
    let dest = st.w().leave_card(saved);
    c.set_fuel(left)?;
    match res {
        Ok(_) => Ok(dest),
        Err(e) if e.is_need_input() => Ok(abi::EXIT_NEED_INPUT),
        Err(e) => Err(e),
    }
}

pub fn raise_bought<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<i32, HostErr> {
    {
        let req = HostRequest::RaiseBought { player_id, tile };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

/// Pack a `DealValue` (BuyAssign) raise result into the single answer log
/// slot: bit 0 = cancelled, bits 1-4 = houses + 1 (0 = unset, 1 = zero
/// houses, … 15 = fourteen), bit 5 = mortgaged, bits 6-31 = owner + 2
/// (0 = unset, 1 = bank / -1, 2+ = seat + 2).
///
/// `i32::MIN` on either field means "leave the deal's value alone" and packs
/// as the unset code. Houses must therefore encode `0` (razed) distinctly
/// from unset — the `+ 1` does that. Transient within one raise; nothing
/// persists this packing.
pub fn pack_deal(cancelled: bool, owner: i32, houses: i32, mortgaged: bool) -> i32 {
    let o = if owner == i32::MIN {
        0
    } else {
        (owner + 2).clamp(0, (1 << 26) - 1)
    };
    let h = if houses == i32::MIN {
        0
    } else {
        (houses + 1).clamp(1, 15)
    };
    ((cancelled as i32) & 1) | ((h & 15) << 1) | (((mortgaged as i32) & 1) << 5) | (o << 6)
}

/// Inverse of [`pack_deal`].
pub fn unpack_deal(v: i32) -> (bool, i32, i32, bool) {
    let cancelled = v & 1 != 0;
    let h = (v >> 1) & 15;
    let houses = if h == 0 { i32::MIN } else { h - 1 };
    let mortgaged = (v >> 5) & 1 != 0;
    let o = (v >> 6) & ((1 << 26) - 1);
    let owner = if o == 0 { i32::MIN } else { o - 2 };
    (cancelled, owner, houses, mortgaged)
}

/// Raise a trigger point from a guest body. `kind` is the wire **spelling**.
pub fn raise<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    kind: &str,
    value: i32,
) -> Result<i32, HostErr> {
    // v78: a `DealValue` kind (BuyAssign) carries the default
    // deal from the guest's trigger in, and writes the rewritten
    // deal back so `trigger::deal_*` reads the fold.
    let tk = card_sdk::abi::TriggerKind::from_str(kind);
    let is_deal = tk == card_sdk::abi::TriggerKind::BuyAssign;
    let trig = c.st().wr().trigger();
    let tile = trig.tile;
    // Flatten boundary: `Option<Payee>` / `Option<i32>` <-> wire `i32`
    // (`i32::MIN` = no rewrite) on `HostRequest::Raise`.
    let (deal_owner, deal_houses, deal_mortgaged) = if is_deal {
        match trig.buy {
            Some(b) => (
                b.deal_owner.map(|p| p.as_i32()).unwrap_or(i32::MIN),
                b.deal_houses.unwrap_or(i32::MIN),
                b.deal_mortgaged as i32,
            ),
            None => (i32::MIN, i32::MIN, 0),
        }
    } else {
        (i32::MIN, i32::MIN, 0)
    };
    let req = HostRequest::Raise {
        player_id,
        kind: kind.to_string(),
        value,
        tile,
        deal_owner,
        deal_houses,
        deal_mortgaged,
    };
    let packed = crate::inline::request_or_pause(c, req, crate::inline::Pause::Sentinel)?;
    if is_deal {
        let (_cancelled, o, h, m) = unpack_deal(packed);
        let st = c.st_mut();
        if o != i32::MIN {
            st.w()
                .set_trigger_deal_owner(Some(game_kinds::Payee::from_i32(o)));
        }
        if h != i32::MIN {
            st.w().set_trigger_deal_houses(Some(h));
        }
        st.w().set_trigger_deal_mortgaged(m as i32);
    }
    Ok(packed)
}

/// Named-counter read. The name is data.
pub fn marker_get<C: HostCtx>(c: &mut C, player_id: i32, name: &str) -> Result<i32, HostErr> {
    Ok(c.st().wr().state_get(player_id, name))
}

/// Named-counter cap read. The name is data.
pub fn marker_max<C: HostCtx>(c: &mut C, player_id: i32, name: &str) -> Result<i32, HostErr> {
    Ok(c.st().wr().state_max(player_id, name))
}

/// Generic marker gain. The name is data; opens `markerGain` with that name.
pub fn gain_marker<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    name: &str,
    n: i32,
    p: i32,
    l: i32,
) -> Result<i32, HostErr> {
    let why = guest_msg(c, p, l)?;
    // Marker window: `markerGain` opens before the markers move.
    //
    // The pause is a **trap**, not the sentinel: `ctx::gain_marker`
    // returns `Result<i32, Prompt>` and a great many bodies discard
    // that result (`ctx::gain_marker(...);`). With the sentinel the body
    // would catch the `Err(Prompt)` and keep going, which the replay
    // model's `finish` boundary check traps ("card published a prompt
    // and kept going" -- the card goes out) while the inline answer
    // model (`docs/BOT.md` §3.2) answers and the body completes. A
    // trap tears the stack before the body can swallow the pause, so
    // both models replay / answer the same body.
    if n > 0 {
        let ok = crate::inline::request_or_pause(
            c,
            crate::HostRequest::Marker {
                player_id,
                name: name.to_string(),
                delta: n,
            },
            crate::inline::Pause::Trap,
        )?;
        if ok != 1 {
            return Ok(ok);
        }
    }
    Ok(c.st_mut().w().gain_marker(player_id, name, n, why))
}

/// Legacy layered write. New guests use the split form (`gate` + `state_add`
/// + [`raise_after`]).
pub fn apply_abnormal<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    kind: i32,
    n: i32,
    to: i32,
) -> Result<(), HostErr> {
    if n < 0 {
        c.st_mut().w().apply_abnormal(player_id, kind, n, to);
    } else if n > 0 {
        if let Some(k) = AbKind::from_i32(kind) {
            if abnormal_gate(c, player_id, k)? {
                c.st_mut().w().apply_abnormal(player_id, kind, n, to);
            }
        }
    }
    Ok(())
}

/// Declare the generic state-flag bits for a key (see
/// [`game_core::state::state_flag`]).
pub fn state_set_flags<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    key: &str,
    flags: u8,
) -> Result<(), HostErr> {
    c.st_mut().w().state_set_flags(player_id, key, flags);
    Ok(())
}

/// Queue a post-commit raise whose wire event name is caller-supplied data.
pub fn raise_after<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    name: &str,
    value: i32,
) -> Result<(), HostErr> {
    c.st_mut().w().raise_after(player_id, name, value);
    Ok(())
}

/// Set this turn's main-move latch (`locked != 0` skips; `0` releases).
/// Generic turn-flow; the move-gate host drives it. Off-board is not part of the
/// latch (closeout-2 ruling).
pub fn skip_main_move<C: HostCtx>(c: &mut C, player_id: i32, locked: i32) -> Result<(), HostErr> {
    c.st_mut().w().skip_main_move(player_id, locked);
    Ok(())
}

/// [`skip_main_move`] with `locked = 0`.
pub fn clear_move_skip<C: HostCtx>(c: &mut C, player_id: i32) -> Result<(), HostErr> {
    c.st_mut().w().clear_move_skip(player_id);
    Ok(())
}

pub fn give_extra_turn<C: HostCtx>(c: &mut C, player_id: i32) -> Result<(), HostErr> {
    c.st_mut().w().give_extra_turn(player_id);
    Ok(())
}

/// Generic "end this player's turn now" latch. `turn_start` honors it after
/// the `turnStartBefore` / `turnStart` raises (auto-skip pacing).
pub fn end_turn_now<C: HostCtx>(c: &mut C, player_id: i32) -> Result<(), HostErr> {
    c.st_mut().w().end_turn_now(player_id);
    Ok(())
}

/// Teleport with the move's `main` flag named and **no** abnormal window
/// (the caller's own clause -- see [`HostRequest::TeleportAs`]).
pub fn teleport_as<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    resolve: i32,
    as_main: i32,
) -> Result<(), HostErr> {
    {
        let st = c.st_mut();
        if st.answers.get(st.next_answer).is_some() {
            let answer = st.next_answer;
            st.w().after_host(answer);
            st.next_answer += 1;
            return Ok(());
        }
    }
    let make = || HostRequest::TeleportAs {
        player_id,
        tile,
        resolve: resolve != 0,
        as_main: as_main != 0,
    };
    if crate::inline::request_or_inline(c, make())?.is_some() {
        return Ok(());
    }
    let st = c.st_mut();
    st.host_request = Some(make());
    Err(HostErr::NeedInput)
}

/// [`log`] with an explicit event kind (the engine's own skip / over lines use
/// `"status"`; cards use the default `"text"` via [`log`]).
pub fn log_as<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    kind_p: i32,
    kind_n: i32,
    p: i32,
    n: i32,
) -> Result<(), HostErr> {
    let kind = guest_str(c, kind_p, kind_n)?;
    let msg = guest_msg(c, p, n)?;
    c.st_mut().w().log_as(player_id, &kind, msg);
    Ok(())
}

/// The payer's gate. Not out, and the `PayGate` conditions of the standing
/// `status:*` rules (CEL-only, evaluated natively) all leave it open
/// (`docs/STATUS-HOSTS.md`). A handle-less run (a test world) falls back to
/// the world's own answer.
fn pay_gate_open<C: HostCtx>(c: &mut C, player_id: i32) -> bool {
    if c.st().wr().player_out(player_id) != 0 {
        return false;
    }
    match c.st().rules.clone() {
        Some(rules) => {
            let win = crate::gate::window_of_card_world(c.st().wr(), player_id);
            rules.gate_open(crate::TriggerKind::PayGate, &win)
        }
        None => c.st().wr().can_pay(player_id) != 0,
    }
}

pub fn can_pay<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok(pay_gate_open(c, player_id) as i32)
}

pub fn cant_move<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().cant_move(player_id) })
}

/// Generic marker spend. The name is data; opens `markerSpend` with that name.
pub fn spend_marker<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    name: &str,
    n: i32,
    p: i32,
    l: i32,
) -> Result<i32, HostErr> {
    let why = guest_msg(c, p, l)?;
    // Marker window (user ruling 2026-10-07): `markerSpend` opens
    // **before** the markers move. A cancelled link spends nothing.
    if n > 0 {
        let ok = crate::inline::request_or_pause(
            c,
            crate::HostRequest::Marker {
                player_id,
                name: name.to_string(),
                delta: -n,
            },
            crate::inline::Pause::Sentinel,
        )?;
        // Propagate the pause sentinel to the guest (`asked()` turns it
        // into `Err(Prompt)`); `0` means a counteraction cancelled it.
        if ok != 1 {
            return Ok(ok);
        }
    }
    Ok(c.st_mut().w().spend_marker(player_id, name, n, why))
}

pub fn turn_player<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().turn_player())
}

pub fn round_no<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().round_no())
}

pub fn turn_key<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().turn_key())
}

pub fn persona_is<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<i32, HostErr> {
    let name = guest_str(c, p, n)?;
    Ok(c.st().wr().persona_is(player_id, &name))
}

pub fn set_persona<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<(), HostErr> {
    let name = guest_str(c, p, n)?;
    c.st_mut().w().set_persona(player_id, &name);
    Ok(())
}

pub fn in_group<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<i32, HostErr> {
    let name = guest_str(c, p, n)?;
    Ok(c.st().wr().in_group(player_id, &name))
}

pub fn do_move_roll<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok(c.st_mut().w().do_move_roll(player_id))
}

pub fn tok_names<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    p: i32,
    n: i32,
    buf: i32,
    cap: i32,
) -> Result<i32, HostErr> {
    let prefix = guest_str(c, p, n)?;
    let names = c.st().wr().tok_names(player_id, &prefix);
    let bytes = postcard::to_allocvec(&names)
        .map_err(|e| HostErr::trap(format!("tok_names encode: {e}")))?;
    if bytes.len() as i32 <= cap {
        c.write_guest(buf, &bytes)?;
    }
    Ok(bytes.len() as i32)
}

pub fn unplace_card_named<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    cptr: i32,
    cl: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cptr, cl)?;
    Ok(c.st_mut().w().unplace_card_named(player_id, &card) as i32)
}

pub fn can_build_on<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<i32, HostErr> {
    // The tile-level reasons are the world's; the payer's gate is the
    // status rules' (`PayGate`).
    Ok((c.st().wr().can_build_on(player_id, tile) && pay_gate_open(c, player_id)) as i32)
}

pub fn card_face_down<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    cptr: i32,
    cl: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cptr, cl)?;
    Ok(c.st().wr().card_face_down(player_id, &card) as i32)
}

pub fn extreme<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().extreme())
}

pub fn set_extreme<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_extreme(v);
    Ok(())
}

pub fn play_from_hand<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().play_from_hand() as i32 })
}

pub fn gain_fixed<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    amount: i32,
    p: i32,
    n: i32,
) -> Result<i32, HostErr> {
    // V4 (`NEGATION-AUDIT`): Tritone's 「立刻获得此次失去的资金金额」 is
    // money movement and goes through the same `Money` pipeline as any
    // other gain -- the `effect` counteraction window, the modifier stages and
    // the `pay` settlement all see it. (It used to debit/credit the
    // world copy directly, outside every 支付阶段 window.)
    gain(c, player_id, amount, p, n)
}

pub fn set_card_face_down<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    cptr: i32,
    cl: i32,
    down: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cptr, cl)?;
    Ok(c.st_mut()
        .w()
        .set_card_face_down(player_id, &card, down != 0) as i32)
}

pub fn clear_dice<C: HostCtx>(c: &mut C) -> Result<(), HostErr> {
    c.st_mut().w().clear_dice();
    Ok(())
}

/// The first field instance of `card` on `player_id`'s seat, or -1.
fn field_uid_of<C: HostCtx>(c: &C, player_id: i32, card: &str) -> i32 {
    c.st()
        .wr()
        .field_instances(player_id)
        .into_iter()
        .find(|(_, id)| id == card)
        .map(|(u, _)| u)
        .unwrap_or(-1)
}

/// 「不受任何效果影响」 on a named placed card. Resolved through the instance
/// (`is_immune_at` / `set_immune_at`): the card name is not an identity, so
/// this hits the first field copy.
pub fn set_card_immune<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    cptr: i32,
    cl: i32,
    on: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cptr, cl)?;
    let uid = field_uid_of(c, player_id, &card);
    if uid < 0 {
        return Ok(0);
    }
    Ok(c.st_mut().w().set_immune_at(uid, on != 0) as i32)
}

pub fn card_immune<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    cptr: i32,
    cl: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cptr, cl)?;
    let uid = field_uid_of(c, player_id, &card);
    if uid < 0 {
        return Ok(0);
    }
    Ok(c.st().wr().is_immune_at(uid) as i32)
}

pub fn set_card_tile<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    cptr: i32,
    cl: i32,
    tile: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cptr, cl)?;
    let uid = field_uid_of(c, player_id, &card);
    if uid < 0 {
        return Ok(0);
    }
    Ok(c.st_mut().w().set_tile_at(uid, tile) as i32)
}

pub fn place_card_on<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    cptr: i32,
    cl: i32,
    p: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cptr, cl)?;
    let note = guest_msg(c, p, n)?;
    Ok(c.st_mut().w().place_card_on(player_id, tile, &card, note))
}

pub fn place_card_at<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    cptr: i32,
    cl: i32,
    p: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cptr, cl)?;
    let note = guest_msg(c, p, n)?;
    Ok(c.st_mut().w().place_card(player_id, &card, note))
}

pub fn set_dest<C: HostCtx>(c: &mut C, dest: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_dest(dest);
    Ok(())
}

pub fn set_transfer_to_dest<C: HostCtx>(c: &mut C, to: i32, dest: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_transfer_to_dest(to, dest);
    Ok(())
}

pub fn send_to_dest<C: HostCtx>(c: &mut C, dest: i32) -> Result<i32, HostErr> {
    Ok(c.st_mut().w().send_to_dest(dest).unwrap_or(-1))
}

pub fn transfer_to_dest<C: HostCtx>(c: &mut C, to: i32, dest: i32) -> Result<i32, HostErr> {
    Ok(c.st_mut().w().transfer_to_dest(to, dest).unwrap_or(-1))
}

pub fn rent_unit_mult<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().rent_unit_mult() })
}

pub fn add_rent_unit_bonus<C: HostCtx>(c: &mut C, n: i32) -> Result<i32, HostErr> {
    Ok({ c.st_mut().w().add_rent_unit_bonus(n) })
}

pub fn teleport_to<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<(), HostErr> {
    // A position write with no settle (`resolve: false`).
    // Paused like `card_move` rather than written to the run's world
    // copy: a host routine that follows (`card_move`) runs against the
    // live world, and a copy-only write left the move starting from
    // the pre-teleport tile -- then the replay re-executed the write
    // and `Done`'s `swap_world` teleported the player back over the
    // move that had already run. The engine applies the gate + the
    // write once; the replay reads the logged answer and skips. In
    // simulation mode the engine applies it inline and the guest
    // continues on the same world.
    {
        let st = c.st_mut();
        if st.answers.get(st.next_answer).is_some() {
            let answer = st.next_answer;
            st.w().after_host(answer);
            st.next_answer += 1;
            return Ok(());
        }
    }
    if crate::inline::request_or_inline(c, HostRequest::Teleport { player_id, tile })?.is_some() {
        return Ok(());
    }
    let st = c.st_mut();
    st.host_request = Some(HostRequest::Teleport { player_id, tile });
    Err(HostErr::NeedInput)
}

pub fn gate<C: HostCtx>(c: &mut C, player_id: i32, kind: i32) -> Result<i32, HostErr> {
    let Some(kind) = crate::AbKind::from_i32(kind) else {
        return Err(HostErr::trap(format!("gate: unknown abnormal kind {kind}")));
    };
    Ok(abnormal_gate(c, player_id, kind)? as i32)
}

pub fn abnormal_count<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().abnormal_count(player_id) })
}

pub fn targeted_count<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().targeted_count(player_id) })
}

pub fn gains_this_turn<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().gains_this_turn(player_id) })
}

pub fn designations<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    buf: i32,
    cap: i32,
) -> Result<i32, HostErr> {
    let list = c.st().wr().designations(player_id);
    let bytes = postcard::to_allocvec(&list)
        .map_err(|e| HostErr::trap(format!("designations encode: {e}")))?;
    if bytes.len() as i32 <= cap {
        c.write_guest(buf, &bytes)?;
    }
    Ok(bytes.len() as i32)
}

pub fn cancel_designation<C: HostCtx>(c: &mut C, seat: i32) -> Result<(), HostErr> {
    c.st_mut().w().cancel_designation(seat);
    Ok(())
}

pub fn designation_cancelled<C: HostCtx>(c: &mut C, seat: i32) -> Result<i32, HostErr> {
    Ok({ c.st().wr().designation_cancelled(seat) as i32 })
}

pub fn target<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    single: i32,
) -> Result<i32, HostErr> {
    // Paused like `gate`: the engine runs the targeting pipeline (it
    // raises hooks and opens the `target` counteraction window) and the replay
    // reads its answer.
    {
        let req = HostRequest::Target {
            player_id,
            tile,
            single: single != 0,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn card_move<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    // The card shaped the plan and wants the move to run *now*.
    // Paused like the others: the engine runs `Cx::card_move`
    // (which may prompt) and the replay reads the answer. The plan is
    // captured here because the run's world copy is discarded on pause
    // (in simulation mode it is not -- the engine runs the move against
    // the guest's own world and the body continues).
    {
        let st = c.st_mut();
        if let Some(&ok) = st.answers.get(st.next_answer) {
            let answer = st.next_answer;
            st.w().after_host(answer);
            st.next_answer += 1;
            return Ok(ok);
        }
    }
    let plan = c.st().wr().move_plan();
    let req = HostRequest::Move { player_id, plan };
    if let Some(ok) = crate::inline::request_or_inline(c, req.clone())? {
        return Ok(ok);
    }
    let st = c.st_mut();
    st.host_request = Some(req);
    Err(HostErr::NeedInput)
}

pub fn roll_ask<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    count: i32,
    sides: i32,
    source: i32,
) -> Result<i32, HostErr> {
    {
        let req = HostRequest::Roll {
            player_id,
            count,
            sides,
            source,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn card_settle_at<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    main: i32,
) -> Result<i32, HostErr> {
    {
        let req = HostRequest::SettleAt {
            player_id,
            tile,
            main: main != 0,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn card_buy<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<i32, HostErr> {
    {
        let req = HostRequest::Buy {
            player_id,
            tile,
            kind: 2, // `BuyKind::Card` -- the legacy `card_buy` shape.
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn buy<C: HostCtx>(c: &mut C, player_id: i32, tile: i32, kind: i32) -> Result<i32, HostErr> {
    {
        let req = HostRequest::Buy {
            player_id,
            tile,
            kind,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn buy_quotes<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    kind: i32,
    buf: i32,
    n: i32,
    out: i32,
) -> Result<i32, HostErr> {
    let bytes = c.read_guest(buf, n)?;
    let tiles: Vec<i32> = bytes
        .chunks_exact(4)
        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    // HOST-07: the same hook-aware quote `buy` charges
    // (`CardWorld::buy_quotes` → `CardRules::buy_quote`), so the
    // guest's preview is the price the commit will take (quote ==
    // charge). Still a query -- no pause, no world write.
    let quotes = c.st().wr().buy_quotes(player_id, kind, &tiles);
    let bytes = postcard::to_allocvec(&quotes)
        .map_err(|e| HostErr::trap(format!("buy_quotes encode: {e}")))?;
    c.write_guest(out, &bytes)?;
    Ok(bytes.len() as i32)
}

pub fn ai_agent_choice<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    buf: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let bytes = c.read_guest(buf, n)?;
    let tiles: Vec<i32> = bytes
        .chunks_exact(4)
        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    {
        let req = HostRequest::AiAgentChoice { player_id, tiles };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn acquire<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    from: i32,
    tile: i32,
    price: i32,
) -> Result<i32, HostErr> {
    {
        let req = HostRequest::Acquire {
            player_id,
            from,
            tile,
            price,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn agent_offer<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    agent: i32,
    tile: i32,
    kind: i32,
) -> Result<i32, HostErr> {
    {
        let req = HostRequest::AgentOffer {
            player_id,
            agent,
            tile,
            kind,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn linger<C: HostCtx>(c: &mut C, player_id: i32, expires: i32) -> Result<i32, HostErr> {
    {
        let req = HostRequest::Linger { player_id, expires };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn card_build<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<i32, HostErr> {
    {
        let req = HostRequest::Build { player_id, tile };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn card_offer_build<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    buf: i32,
    n: i32,
) -> Result<i32, HostErr> {
    let bytes = c.read_guest(buf, n)?;
    let tiles: Vec<i32> = bytes
        .chunks_exact(4)
        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    {
        let req = HostRequest::OfferBuild { player_id, tiles };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn card_mortgage<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<i32, HostErr> {
    {
        let req = HostRequest::Mortgage { player_id, tile };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

/// Mechanism half of a mortgage the tile rule already approved.
pub fn apply_mortgage<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    amount: i32,
) -> Result<i32, HostErr> {
    {
        let req = HostRequest::ApplyMortgage {
            player_id,
            tile,
            amount,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

/// Mechanism half of a redeem the tile rule already approved.
pub fn apply_redeem<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    amount: i32,
) -> Result<i32, HostErr> {
    {
        let req = HostRequest::ApplyRedeem {
            player_id,
            tile,
            amount,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

/// Pay the buy price through the money pipeline. `kind` is the
/// [`card_sdk::abi::BuyKind`] wire value. Answers the amount actually paid.
pub fn pay_buy<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    amount: i32,
    kind: i32,
) -> Result<i32, HostErr> {
    {
        let req = HostRequest::PayBuy {
            player_id,
            tile,
            amount,
            kind,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

/// Pay the build cost through the money pipeline. Answers the amount paid.
pub fn pay_build<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    amount: i32,
) -> Result<i32, HostErr> {
    {
        let req = HostRequest::PayBuild {
            player_id,
            tile,
            amount,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

/// Write the deed after a buy (`purchase::assign_deed`).
pub fn assign_deed<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    tile: i32,
    owner: i32,
    houses: i32,
    mortgaged: i32,
) -> Result<i32, HostErr> {
    {
        let req = HostRequest::AssignDeed {
            player_id,
            tile,
            owner,
            houses,
            mortgaged,
        };
        return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
    }
}

pub fn placed_tile<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<i32, HostErr> {
    let id = guest_str(c, p, n)?;
    Ok(c.st().wr().placed_tile(player_id, &id))
}

pub fn play_doubled<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().play_doubled())
}

pub fn set_play_doubled<C: HostCtx>(c: &mut C, n: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_play_doubled(n);
    Ok(())
}

pub fn trig_cards<C: HostCtx>(c: &mut C, buf: i32, cap: i32) -> Result<i32, HostErr> {
    let list = c.st().wr().trigger().cards;
    let bytes = postcard::to_allocvec(&list)
        .map_err(|e| HostErr::trap(format!("trig_cards encode: {e}")))?;
    if bytes.len() as i32 <= cap {
        c.write_guest(buf, &bytes)?;
    }
    Ok(bytes.len() as i32)
}

pub fn opt_int<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().options.push(PromptOption::Int(v));
    Ok(())
}

pub fn opt_str<C: HostCtx>(c: &mut C, p: i32, n: i32) -> Result<(), HostErr> {
    let s = guest_msg(c, p, n)?;
    c.st_mut().options.push(PromptOption::Str(s));
    Ok(())
}

pub fn opt_tile<C: HostCtx>(c: &mut C, tile: i32, lp: i32, ll: i32) -> Result<(), HostErr> {
    let label = guest_msg(c, lp, ll)?;
    c.st_mut().options.push(PromptOption::Tile { tile, label });
    Ok(())
}

pub fn opt_price<C: HostCtx>(c: &mut C, price: i32) -> Result<(), HostErr> {
    c.st_mut().prices.push(price);
    Ok(())
}

pub fn opt_ai<C: HostCtx>(c: &mut C, ai: i32) -> Result<(), HostErr> {
    c.st_mut().ai_hint = ai;
    Ok(())
}

/// How many of the pushed options a multi-select ask wants (`ask_pick_multi`).
/// Pushed before `ask`, like `opt_price` / `opt_ai`.
pub fn opt_count<C: HostCtx>(c: &mut C, n: i32) -> Result<(), HostErr> {
    c.st_mut().count = n;
    Ok(())
}

pub fn ask<C: HostCtx>(
    c: &mut C,
    kind: i32,
    player_id: i32,
    tp: i32,
    tl: i32,
    xp: i32,
    xl: i32,
) -> Result<i32, HostErr> {
    let kind = PromptKind::from_i32(kind)
        .ok_or_else(|| HostErr::trap(format!("bad prompt kind {kind}")))?;
    let title = guest_msg(c, tp, tl)?;
    let text = guest_msg(c, xp, xl)?;
    let (options, prices, ai_hint, count, answer_slot) = {
        let st = c.st_mut();
        let options = std::mem::take(&mut st.options);
        if kind != PromptKind::YesNo && options.is_empty() {
            return Err(HostErr::trap(format!(
                "{} prompt with no options",
                kind.as_str()
            )));
        }
        let prices = std::mem::take(&mut st.prices);
        let ai_hint = std::mem::replace(&mut st.ai_hint, -1);
        let count = std::mem::replace(&mut st.count, 0);
        (options, prices, ai_hint, count, st.next_answer)
    };
    // Like `pay`: the pause is the sentinel as the return value, so
    // `ctx::ask_*` can hand the card `Err(Prompt)` instead of unwinding.
    // In simulation mode the provider answers inline (or declines and
    // this pauses, tree-relevant prompts only).
    crate::inline::prompt_or_pause(
        c,
        Prompt {
            kind,
            player_id,
            title,
            text,
            options,
            answer_slot,
            prices,
            ai_hint,
            count,
        },
    )
}

/// The raise's [`TriggerKind`] wire **spelling** (`TriggerKind::spelling`).
/// Two-call out-buffer pattern like [`trig_name`].
pub fn trig_kind<C: HostCtx>(c: &mut C, p: i32, n: i32) -> Result<i32, HostErr> {
    let s = c.st().wr().trigger().kind.spelling();
    write_str(c, &s, p, n)
}

pub fn trig_player<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().trigger().player_id })
}

pub fn trig_target<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().target)
}

pub fn trig_tile<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().tile)
}

pub fn trig_value<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().value)
}

pub fn trig_step<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().step)
}

pub fn trig_by_card<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().trigger().by_card.unwrap_or(-1) })
}

pub fn trig_pay_is_rent<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st()
        .wr()
        .trigger()
        .pay
        .map(|p| p.is_rent)
        .unwrap_or(false) as i32)
}

pub fn trig_move_kind<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().mv.map_or(-1, |m| m.kind.as_i32()))
}

pub fn trig_move_resolve<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().mv.map_or(0, |m| m.resolve as i32))
}

pub fn trig_move_tag<C: HostCtx>(c: &mut C, p: i32, n: i32) -> Result<i32, HostErr> {
    let key = guest_str(c, p, n)?;
    Ok(c.st()
        .wr()
        .trigger()
        .mv
        .and_then(|m| m.tags.iter().find(|(k, _)| *k == key).map(|(_, v)| *v))
        .unwrap_or(0))
}

pub fn trig_move_main<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().main as i32)
}

/// `t.Move.SettleRedirect` -- was this settle initiated by a card
/// (`card_settle_at`) rather than by the player's own move landing?
/// `false` when there is no move payload.
pub fn trig_settle_redirect<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st()
        .wr()
        .trigger()
        .mv
        .map_or(0, |m| m.settle_redirect as i32))
}

pub fn trig_move_dir<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    // 1 when no move (matches CoreTrigger::new / MoveSnap / guest docs).
    Ok(c.st().wr().trigger().mv.map_or(1, |m| m.dir.as_i32()))
}

pub fn trig_move_remaining<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().mv.map_or(0, |m| m.remaining))
}

pub fn trig_move_from<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().mv.map_or(-1, |m| m.from))
}

pub fn trig_move_total<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().mv.map_or(0, |m| m.total))
}

pub fn trig_move_roll<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().mv.and_then(|m| m.roll).unwrap_or(-1))
}

pub fn trig_roll_source<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().roll_source.as_i32())
}

pub fn trig_buy_kind<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().buy.map_or(0, |b| b.kind as i32))
}

pub fn trig_seller<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().buy.map_or(-1, |b| b.seller.as_i32()))
}

pub fn trig_price<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().buy.map_or(0, |b| b.price))
}

pub fn trig_set_price<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_price(v);
    Ok(())
}

/// Flatten boundary: `Option<Payee>` <-> wire `i32` (`-1` = bank / unset).
pub fn trig_deal_owner<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st()
        .wr()
        .trigger()
        .buy
        .and_then(|b| b.deal_owner)
        .map(|p| p.as_i32())
        .unwrap_or(-1))
}

pub fn trig_set_deal_owner<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    // Flatten boundary: wire `i32` -> `Option<Payee>`.
    c.st_mut()
        .w()
        .set_trigger_deal_owner(Some(game_kinds::Payee::from_i32(v)));
    Ok(())
}

/// Flatten boundary: `Option<i32>` <-> wire `i32` (`0` when unset).
pub fn trig_deal_houses<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st()
        .wr()
        .trigger()
        .buy
        .and_then(|b| b.deal_houses)
        .unwrap_or(0))
}

pub fn trig_set_deal_houses<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_deal_houses(Some(v));
    Ok(())
}

pub fn trig_deal_mortgaged<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st()
        .wr()
        .trigger()
        .buy
        .map_or(0, |b| b.deal_mortgaged as i32))
}

pub fn trig_set_deal_mortgaged<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_deal_mortgaged(v);
    Ok(())
}

pub fn trig_set_reason<C: HostCtx>(c: &mut C, p: i32, n: i32) -> Result<(), HostErr> {
    let reason = guest_str(c, p, n)?;
    c.st_mut().w().set_trigger_reason(&reason);
    Ok(())
}

pub fn trig_set_move_roll<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_move_roll(v);
    Ok(())
}

pub fn trig_set_pay_amount<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_value(v);
    Ok(())
}

pub fn trig_set_pay_target<C: HostCtx>(c: &mut C, to: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_target(to);
    Ok(())
}

pub fn trig_set_cancelled<C: HostCtx>(c: &mut C) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_cancelled();
    Ok(())
}

pub fn trig_set_negate_effect<C: HostCtx>(c: &mut C) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_negate_effect();
    Ok(())
}

pub fn trig_set_spare<C: HostCtx>(c: &mut C, seat: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_spare(seat);
    Ok(())
}

pub fn trig_cancelled<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().trigger().is_cancelled() as i32 })
}

pub fn trig_seq<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().seq as i32)
}

pub fn trig_answers<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().trigger().answers as i32 })
}

pub fn trig_effect_count<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().trigger().effects.len() as i32 })
}

/// The effect `i`'s kind **spelling**. Two-call out-buffer like [`trig_name`].
pub fn trig_effect_kind<C: HostCtx>(c: &mut C, i: i32, p: i32, n: i32) -> Result<i32, HostErr> {
    let s = {
        let t = c.st().wr().trigger();
        t.effects
            .get(i.max(0) as usize)
            .map(|e| e.kind.clone())
            .unwrap_or_default()
    };
    write_str(c, &s, p, n)
}

pub fn trig_effect_target<C: HostCtx>(c: &mut C, i: i32) -> Result<i32, HostErr> {
    Ok({
        c.st()
            .wr()
            .trigger()
            .effects
            .get(i.max(0) as usize)
            .map_or(-1, |e| e.target)
    })
}

pub fn trig_effect_from<C: HostCtx>(c: &mut C, i: i32) -> Result<i32, HostErr> {
    Ok({
        c.st()
            .wr()
            .trigger()
            .effects
            .get(i.max(0) as usize)
            .map_or(-1, |e| e.from)
    })
}

pub fn trig_effect_tile<C: HostCtx>(c: &mut C, i: i32) -> Result<i32, HostErr> {
    Ok({
        c.st()
            .wr()
            .trigger()
            .effects
            .get(i.max(0) as usize)
            .map_or(-1, |e| e.tile)
    })
}

pub fn trig_effect_value<C: HostCtx>(c: &mut C, i: i32) -> Result<i32, HostErr> {
    Ok({
        c.st()
            .wr()
            .trigger()
            .effects
            .get(i.max(0) as usize)
            .map_or(0, |e| e.value)
    })
}

/// Append a declared effect. `kind` is the wire **spelling**.
pub fn declare_effect<C: HostCtx>(
    c: &mut C,
    kind: &str,
    target: i32,
    from: i32,
    tile: i32,
    value: i32,
) -> Result<(), HostErr> {
    c.st_mut()
        .w()
        .declare_trigger_effect(kind, target, from, tile, value);
    Ok(())
}

pub fn trig_card_is<C: HostCtx>(c: &mut C, p: i32, n: i32) -> Result<i32, HostErr> {
    let id = guest_str(c, p, n)?;
    Ok(c.st().wr().trig_card_is(&id))
}

pub fn play_card<C: HostCtx>(c: &mut C, p: i32, n: i32, player_id: i32) -> Result<i32, HostErr> {
    let id = guest_str(c, p, n)?;
    let rules = c
        .st()
        .rules
        .clone()
        .ok_or_else(|| HostErr::trap("play_card unavailable here"))?;
    let card = rules
        .by_id(&id)
        .ok_or_else(|| HostErr::trap(format!("play_card: unknown card {id:?}")))?;
    if c.st().depth >= MAX_NESTING {
        return Err(HostErr::trap(format!(
            "play_card nested deeper than {MAX_NESTING}"
        )));
    }
    let fuel = c.fuel()?;
    let st = c.st_mut();
    let saved = st.w().enter_card(&id);
    let was_from_hand = st.w().play_from_hand();
    st.w().set_play_from_hand(false);
    let mut nested = HostState::new(
        rules.clone(),
        st.world.take().expect("world present"),
        std::mem::take(&mut st.answers),
        st.depth + 1,
    );
    nested.next_answer = st.next_answer;
    let entry = match rules.card(card).and_then(|ci| ci.entry(OnKind::Play, None)) {
        Some(e) => e,
        None => {
            let st = c.st_mut();
            st.world = nested.world;
            if let Some(w) = st.world.as_mut() {
                w.set_play_from_hand(was_from_hand);
            }
            st.answers = nested.answers;
            st.next_answer = nested.next_answer;
            let dest = st.w().leave_card(saved);
            c.set_fuel(fuel)?;
            return Ok(dest);
        }
    };
    let (res, inner, left) = c.call_entry(nested, card, entry, export::OP_RUN, player_id, &[], false);
    let st = c.st_mut();
    st.world = inner.world;
    if let Some(w) = st.world.as_mut() {
        w.set_play_from_hand(was_from_hand);
    }
    st.answers = inner.answers;
    st.next_answer = inner.next_answer;
    if inner.asked.is_some() {
        st.asked = inner.asked;
    }
    if inner.host_request.is_some() {
        st.host_request = inner.host_request;
    }
    let dest = st.w().leave_card(saved);
    c.set_fuel(left)?;
    match res {
        Ok(_) => Ok(dest),
        Err(e) if e.is_need_input() => Ok(abi::EXIT_NEED_INPUT),
        Err(e) => Err(e),
    }
}

/// Cross-card applicability query (docs/ACTIVATION.md §3): does the named
/// entry of `card_id` admit for `player_id` right now?
///
/// Evaluates that entry's condition + residual guard / play gate through
/// `admits` and does **not** run its body. Probe mode: a residual that would
/// prompt answers `false` (the same way [`RuleError::GuardPrompted`] folds an
/// eligibility scan to "not eligible").
///
/// The entry is addressed by its stable internal slug (`ManifestOn::id`), not
/// by a positional `(kind, trigger)` pair -- that is what removes "which of
/// this card's three hooks?" ambiguity. An unnamed or unknown id answers `0`.
pub fn query_guard<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    p: i32,
    n: i32,
    ep: i32,
    el: i32,
) -> Result<i32, HostErr> {
    let id = guest_str(c, p, n)?;
    let entry_id = guest_str(c, ep, el)?;
    let rules = c
        .st()
        .rules
        .clone()
        .ok_or_else(|| HostErr::trap("query_guard unavailable here"))?;
    let Some(card) = rules.by_id(&id) else {
        return Ok(0);
    };
    let Some(info) = rules.card(card) else {
        return Ok(0);
    };
    if c.st().depth >= MAX_NESTING {
        return Ok(0);
    }
    // Named-entry lookup (docs/ACTIVATION.md §3). `entry::PRESS` is reserved:
    // it always resolves the press entry the engine itself would pick, so a
    // card with several press modes answers "is *any* press open" the way
    // `play_entry_preferring_gated` does. Any other id must be a declared
    // slug; unnamed / unknown is not externally addressable and fails closed.
    let entry = if entry_id == card_sdk::abi::entry::PRESS {
        match info.play_entry_preferring_gated() {
            Some(e) => e,
            None => return Ok(0),
        }
    } else {
        match info.entry_by_id(&entry_id) {
            Some(e) => e,
            None => return Ok(0),
        }
    };
    let kind = match info.on.get(entry as usize) {
        Some(o) => OnKind::from_i32(o.kind).unwrap_or(OnKind::Play),
        None => return Ok(0),
    };
    Ok(query_guard_entry(c, player_id, &id, card, entry, kind)? as i32)
}

/// Serve the running activation's declared argument as `postcard` bytes
/// (docs/ACTIVATION.md §2). `out_ptr = 0` asks for the size only -- the
/// `group_skills` / `out_buf` shape.
pub fn declared_arg<C: HostCtx>(c: &mut C, out_ptr: i32, cap: i32) -> Result<i32, HostErr> {
    let bytes = c.st().declared.clone();
    let n = bytes.len() as i32;
    if out_ptr == 0 || cap < n {
        return Ok(n);
    }
    if n > 0 {
        c.write_guest(out_ptr, &bytes)?;
    }
    Ok(n)
}

/// [`admits`] for one entry: condition + residual guard / play gate, body
/// never run, probe mode.
fn query_guard_entry<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    id: &str,
    card: i32,
    entry: i32,
    kind: OnKind,
) -> Result<bool, HostErr> {
    let rules = c
        .st()
        .rules
        .clone()
        .ok_or_else(|| HostErr::trap("query_guard unavailable here"))?;
    let pre = rules.pre(card, entry);
    let scope;
    let scope: Option<&crate::cond_pre::WindowScope> = if pre.is_some() {
        scope = crate::cond_pre::window_scope_owned(crate::cond_pre::fill_window_ambient(
            c.st().wr(),
            player_id,
        ));
        Some(&scope)
    } else {
        None
    };
    let cand = crate::cond_pre::fill_candidate(c.st().wr(), player_id, id, false);
    if kind == OnKind::Play {
        // The Play gate answers `Option<Msg>` (None = playable) -- `admits_gate`.
        let asked = crate::cond_pre::admits_gate(
            pre,
            scope,
            &cand,
            || (),
            || -> Result<Option<()>, HostErr> {
                let mut world = c.st().wr().clone();
                world.enter_card(id);
                let depth = c.st().depth + 1;
                let nested = HostState::new(rules.clone(), world, vec![], depth);
                let (res, _inner, left) =
                    c.call_entry(nested, card, entry, export::OP_GUARD, player_id, &[], true);
                c.set_fuel(left)?;
                let playable = match res {
                    Ok(CallOut::Msg(None)) => true,
                    Ok(CallOut::Code(0)) => true,
                    _ => false,
                };
                Ok(if playable { None } else { Some(()) })
            },
        );
        return Ok(matches!(asked, Ok(None)));
    }
    // A residual guard answers 0/1. Probe mode: a pause is "does not admit".
    let asked: Result<bool, ()> = crate::cond_pre::admits(pre, scope, &cand, || {
        let mut world = c.st().wr().clone();
        world.enter_card(id);
        let depth = c.st().depth + 1;
        let nested = HostState::new(rules.clone(), world, vec![], depth);
        let (res, _inner, left) =
            c.call_entry(nested, card, entry, export::OP_GUARD, player_id, &[], false);
        let _ = c.set_fuel(left);
        match res {
            Ok(CallOut::Code(v)) => Ok(v != 0),
            Ok(CallOut::Msg(None)) => Ok(true),
            // Would prompt, or trapped: not eligible (probe mode).
            _ => Ok(false),
        }
    });
    Ok(asked.unwrap_or(false))
}

pub fn schedule_turn_end<C: HostCtx>(c: &mut C, player_id: i32, mode: i32) -> Result<(), HostErr> {
    c.st_mut()
        .w()
        .schedule_turn_end(player_id, mode & 1 != 0, mode & 2 != 0);
    Ok(())
}

pub fn set_no_money_loss<C: HostCtx>(c: &mut C, player_id: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_no_money_loss(player_id);
    Ok(())
}

pub fn set_fixed_roll<C: HostCtx>(c: &mut C, n: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_fixed_roll(n);
    Ok(())
}

pub fn fixed_roll<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().fixed_roll())
}

pub fn set_next_steps<C: HostCtx>(c: &mut C, player_id: i32, n: i32) -> Result<(), HostErr> {
    Ok(c.st_mut().w().set_next_steps(player_id, n))
}

pub fn turn_main_steps<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().turn_main_steps() })
}

/// Generic marker-cap move. The name is data; dumb storage fold.
pub fn add_marker_max<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    name: &str,
    n: i32,
) -> Result<i32, HostErr> {
    Ok(c.st_mut().w().add_marker_max(player_id, name, n))
}

pub fn set_roller<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    // Flatten boundary: wire `i32` -> typed. Negative = no claim.
    c.st_mut().w().set_roller(if v >= 0 {
        Some(game_kinds::Seat(v))
    } else {
        None
    });
    Ok(())
}

pub fn set_steps<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_steps(v);
    Ok(())
}

pub fn set_reverse<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_reverse(v != 0);
    Ok(())
}

pub fn set_signed<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_signed(v != 0);
    Ok(())
}

pub fn set_stop_at<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    // Flatten boundary: wire `i32` -> `Option<Tile>`. Negative = clear.
    c.st_mut().w().set_stop_at(if v >= 0 {
        Some(game_kinds::Tile(v))
    } else {
        None
    });
    Ok(())
}

pub fn set_parity<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_parity(v);
    Ok(())
}

pub fn set_resolve<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_resolve(v != 0);
    Ok(())
}

pub fn set_no_buy<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_no_buy(v != 0);
    Ok(())
}

pub fn set_kind<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_kind(v);
    Ok(())
}

pub fn set_trigger_target<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_target(v);
    Ok(())
}

pub fn set_trigger_cancelled<C: HostCtx>(c: &mut C) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_cancelled();
    Ok(())
}

pub fn set_teleport_to<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    // Flatten boundary: wire `i32` -> `Option<Tile>`. Negative = walk / clear.
    c.st_mut().w().set_teleport_to(if v >= 0 {
        Some(game_kinds::Tile(v))
    } else {
        None
    });
    Ok(())
}

pub fn set_start<C: HostCtx>(c: &mut C, t: i32, p: i32, n: i32) -> Result<(), HostErr> {
    let why = guest_str(c, p, n)?;
    // Flatten boundary: wire `i32` -> `Option<Tile>`. Negative = own tile.
    c.st_mut().w().set_start(
        if t >= 0 {
            Some(game_kinds::Tile(t))
        } else {
            None
        },
        &why,
    );
    Ok(())
}

pub fn set_base_dice<C: HostCtx>(
    c: &mut C,
    count: i32,
    sides: i32,
    p: i32,
    n: i32,
) -> Result<(), HostErr> {
    let why = guest_str(c, p, n)?;
    c.st_mut().w().set_base_dice(count, sides, &why);
    Ok(())
}

pub fn add_base_dice<C: HostCtx>(
    c: &mut C,
    count: i32,
    sides: i32,
    p: i32,
    n: i32,
) -> Result<(), HostErr> {
    let why = guest_str(c, p, n)?;
    c.st_mut().w().add_base_dice(count, sides, &why);
    Ok(())
}

pub fn add_extra_dice<C: HostCtx>(
    c: &mut C,
    count: i32,
    sides: i32,
    p: i32,
    n: i32,
) -> Result<(), HostErr> {
    let why = guest_str(c, p, n)?;
    c.st_mut().w().add_extra_dice(count, sides, &why);
    Ok(())
}

pub fn set_tag<C: HostCtx>(c: &mut C, kp: i32, kl: i32, v: i32) -> Result<(), HostErr> {
    let key = guest_str(c, kp, kl)?;
    c.st_mut().w().set_tag(&key, v);
    Ok(())
}

/// v78: `set_tag` with the key as an intern id. The host stores the id as the
/// tag key's decimal spelling (the guest's `write_filter_value` resolves the
/// id to a known name via a small table and calls the string form instead;
/// this import exists for the wire and for tests).
pub fn set_tag_id<C: HostCtx>(c: &mut C, key_id: i32, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_tag(&key_id.to_string(), v);
    Ok(())
}

/// v78: host-supplied recorded roll (wire `i32::MIN` when none). The host
/// fills this before a Filter raise that would have rolled dice.
/// Flatten boundary: `Option<i32>` <-> wire `i32`.
pub fn recorded_roll<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().recorded_roll().unwrap_or(i32::MIN))
}

pub fn set_min_roll<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    // Flatten boundary: wire `i32` -> `Option<i32>`. `0` = no floor.
    c.st_mut()
        .w()
        .set_min_roll(if v > 0 { Some(v) } else { None });
    Ok(())
}

pub fn set_extra_steps<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_extra_steps(v);
    Ok(())
}

pub fn move_tag<C: HostCtx>(c: &mut C, kp: i32, kl: i32) -> Result<i32, HostErr> {
    let key = guest_str(c, kp, kl)?;
    Ok(c.st().wr().move_tag(&key))
}

pub fn set_settle_tile<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_settle_tile(v);
    Ok(())
}

pub fn set_pay_factor<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_pay_factor(v);
    Ok(())
}

pub fn set_rent_factor<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_rent_factor(v);
    Ok(())
}

pub fn set_can_build<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_can_build(v != 0);
    Ok(())
}

/// HOST-08: `set_no_build(on)` ≡ `set_can_build(!on)` -- one plan flag.
pub fn set_no_build<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_no_build(v != 0);
    Ok(())
}

/// HOST-08: this move may build away from the landing.
pub fn set_build_anywhere<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_build_anywhere(v != 0);
    Ok(())
}

pub fn set_settle_as_agent<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_settle_as_agent(v != 0);
    Ok(())
}

pub fn plan_add_follower<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().plan_add_follower(v);
    Ok(())
}

pub fn set_more_steps<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().w().set_more_steps(v);
    Ok(())
}

pub fn move_stop_at<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    // Flatten boundary: `Option<Tile>` -> wire `i32` (`-1` = none).
    Ok(c.st().wr().move_stop_at().map(|t| t.0).unwrap_or(-1))
}

pub fn move_stopped<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().move_stopped() as i32 })
}

pub fn move_parity<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().move_parity())
}

pub fn move_resolve<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().move_resolve() as i32 })
}

pub fn move_kind<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().move_kind())
}

pub fn move_steps<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().move_steps())
}

pub fn move_remaining<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({ c.st().wr().move_remaining() })
}

pub fn move_total<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().move_total())
}

pub fn move_dir<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().move_dir())
}

// -- cross-card messages ----------------------------------------------------

/// Resolve `Target` to the receiver instance uids that could answer, in field
/// order. `Uid` names one; `Card` names the first field copy of that card id on
/// that seat; `Board` names a standing board-owned rule by id -- a tile rule
/// (`tile:circle`), a mark owner (`mark:cp`), an active event, a status host.
/// They all live in the one `BOARD_OWNER` list, so the id is what
/// disambiguates.
fn resolve_receivers<C: HostCtx>(c: &C, target: &abi::Target) -> Vec<(i32, String)> {
    let w = c.st().wr();
    match target {
        abi::Target::Uid(uid) => {
            // The rule id comes from whichever listing owns the instance.
            let uid = *uid;
            let mut hit = Vec::new();
            for p in 0..w.player_count() {
                for (u, id) in w.field_instances(p) {
                    if u == uid {
                        hit.push((u, id));
                        return hit;
                    }
                }
            }
            // Board-owned standing rules (tile / event / mark / status).
            for (u, id) in w.field_instances(game_core::state::BOARD_OWNER) {
                if u == uid {
                    hit.push((u, id));
                    return hit;
                }
            }
            hit
        }
        abi::Target::Card { player, card } => w
            .field_instances(*player)
            .into_iter()
            .filter(|(_, id)| id == card)
            .collect(),
        abi::Target::Board(id) => w
            .field_instances(game_core::state::BOARD_OWNER)
            .into_iter()
            .filter(|(_, rid)| rid == id)
            .collect(),
    }
}

/// `ctx::send` -- deliver `name`/`payload` to the addressed receiver's
/// `On::Message` entry and return its reply.
///
/// **Returns 0** when there is no receiver, no matching message name, or the
/// entry's pre-guard rejects (documented contract: "no receiver / no matching
/// name / pre-guard reject → 0"). The **first admitted** receiver in field
/// order answers. The handler does **not** flash the card.
pub fn send<C: HostCtx>(
    c: &mut C,
    tp: i32,
    tl: i32,
    np: i32,
    nl: i32,
    pp: i32,
    pl: i32,
) -> Result<i32, HostErr> {
    let tbytes = c.read_guest(tp, tl)?;
    let target: abi::Target = postcard::from_bytes(&tbytes)
        .map_err(|_| HostErr::trap("guest target is not Target postcard"))?;
    let name = guest_str(c, np, nl)?;
    let pbytes = c.read_guest(pp, pl)?;
    let payload: abi::Message = postcard::from_bytes(&pbytes)
        .map_err(|_| HostErr::trap("guest payload is not Message postcard"))?;

    let rules = c
        .st()
        .rules
        .clone()
        .ok_or_else(|| HostErr::trap("send unavailable here"))?;

    // Sender context for the handler (`ctx::message::*`).
    let sender_uid = c.st().wr().self_uid();
    let sender_seat = c.st().wr().turn_player();

    for (uid, rule_id) in resolve_receivers(c, &target) {
        let Some(card) = rules.by_id(&rule_id) else {
            continue;
        };
        let Some(info) = rules.card(card) else {
            continue;
        };
        // The `On::Message` entry whose `messages` list contains `name`.
        let Some(entry) = info.on.iter().enumerate().find_map(|(i, o)| {
            (o.kind == OnKind::Message as i32 && o.messages.iter().any(|m| m == &name))
                .then_some(i as i32)
        }) else {
            continue;
        };
        // Pre-guard reject: this entry is not a receiver.
        if let Some(pre) = rules.pre(card, entry) {
            let cand = crate::cond_pre::fill_candidate(c.st().wr(), sender_seat, &info.id, false);
            if !crate::cond_pre::admits_pre(Some(pre), None, &cand) {
                continue;
            }
        }
        if c.st().depth >= MAX_NESTING {
            return Err(HostErr::trap(format!(
                "send nested deeper than {MAX_NESTING}"
            )));
        }
        let fuel = c.fuel()?;
        let st = c.st_mut();
        // The handler runs **as the receiver instance** (user ruling
        // 2026-10-10: the tile-mark owner answers `"place"` and binds units to its own
        // counter). `enter_card` would start a fresh uid like `invoke_skill`;
        // pin `current_uid` to the resolved receiver so `ctx::place_mark`
        // stamps `instance = the tile-mark owner` and `ctx::self_uid()` is that card.
        let saved = st.w().enter_card(&rule_id);
        st.w().set_self_uid(uid);
        let was_from_hand = st.w().play_from_hand();
        st.w().set_play_from_hand(false);
        let mut nested = HostState::new(
            rules.clone(),
            st.world.take().expect("world present"),
            std::mem::take(&mut st.answers),
            st.depth + 1,
        );
        nested.next_answer = st.next_answer;
        // Stash the message context; the handler reads it via `ctx::message`.
        nested.msg_sender_uid = sender_uid;
        nested.msg_sender_seat = sender_seat;
        nested.msg_name = name.clone();
        nested.msg = payload.clone();
        nested.msg_reply = 0;
        let (res, inner, left) =
            c.call_entry(nested, card, entry, export::OP_RUN, sender_seat, &[], false);
        let st = c.st_mut();
        st.world = inner.world;
        if let Some(w) = st.world.as_mut() {
            w.set_play_from_hand(was_from_hand);
        }
        st.answers = inner.answers;
        st.next_answer = inner.next_answer;
        if inner.asked.is_some() {
            st.asked = inner.asked;
        }
        if inner.host_request.is_some() {
            st.host_request = inner.host_request;
        }
        let reply = inner.msg_reply;
        let _ = st.w().leave_card(saved);
        c.set_fuel(left)?;
        match res {
            Ok(_) => return Ok(reply),
            Err(e) if e.is_need_input() => return Ok(abi::EXIT_NEED_INPUT),
            Err(e) => return Err(e),
        }
    }
    Ok(0)
}

/// The sender's instance uid (`ctx::message::sender_uid`).
pub fn msg_sender_uid<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().msg_sender_uid)
}

/// The sender's seat (`ctx::message::sender_seat`).
pub fn msg_sender_seat<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().msg_sender_seat)
}

/// The message name being handled. Two-call string pattern: `p`/`n` are the
/// guest buffer; `(0,0)` returns the byte length needed.
pub fn msg_name<C: HostCtx>(c: &mut C, p: i32, n: i32) -> Result<i32, HostErr> {
    let s = c.st().msg_name.clone();
    write_str(c, &s, p, n)
}

pub fn msg_a<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().msg.a)
}
pub fn msg_b<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().msg.b)
}
pub fn msg_c<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().msg.c)
}
pub fn msg_tile<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().msg.tile)
}
pub fn msg_seat<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().msg.seat)
}

/// The payload text.
pub fn msg_text<C: HostCtx>(c: &mut C, p: i32, n: i32) -> Result<i32, HostErr> {
    let s = c.st().msg.text.clone();
    write_str(c, &s, p, n)
}

/// Set the reply the sender receives.
pub fn msg_reply<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
    c.st_mut().msg_reply = v;
    Ok(())
}

/// Two-call string export: write `s` into the guest buffer when it fits,
/// returning the byte length the guest should allocate.
fn write_str<C: HostCtx>(c: &mut C, s: &str, p: i32, n: i32) -> Result<i32, HostErr> {
    let bytes = s.as_bytes();
    if p != 0 && n >= bytes.len() as i32 {
        c.write_guest(p, bytes)?;
    }
    Ok(bytes.len() as i32)
}

/// `t.Name` -- the counter name on a `CounterChanged` raise, or the message
/// name on an `On::Message` dispatch. Two-call string pattern like
/// [`msg_name`].
pub fn trig_name<C: HostCtx>(c: &mut C, p: i32, n: i32) -> Result<i32, HostErr> {
    let s = c.st().wr().trigger().name.clone().unwrap_or_default();
    write_str(c, &s, p, n)
}

/// 真奈 join: the seat self-targeted effects of counters answering this link
/// are redirected to (「改为你」). `-1` outside a join.
pub fn join_redirect_target<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger_join_redirect().unwrap_or(-1))
}

/// 真奈 join: mark that a joiner used the redirect (shielded the target), so
/// the join opener knows not to take its fallback draw.
pub fn set_join_used<C: HostCtx>(c: &mut C) -> Result<(), HostErr> {
    c.st_mut().w().set_trigger_join_used();
    Ok(())
}

#[cfg(test)]
mod deal_tests {
    use super::{pack_deal, unpack_deal};

    /// `i32::MIN` means "leave the deal's field alone" and must survive the
    /// pack/unpack round-trip as `i32::MIN` -- never as 0 (a raze) or -2.
    #[test]
    fn unset_fields_round_trip_as_unset() {
        let (_, owner, houses, _) = unpack_deal(pack_deal(false, i32::MIN, i32::MIN, false));
        assert_eq!(owner, i32::MIN, "unset owner must stay unset");
        assert_eq!(houses, i32::MIN, "unset houses must stay unset, not 0");
    }

    /// A real 0 houses (razed) is a value and must not collapse into unset.
    #[test]
    fn zero_houses_is_not_unset() {
        let (_, _, houses, _) = unpack_deal(pack_deal(false, i32::MIN, 0, false));
        assert_eq!(houses, 0, "a raze must stay a raze");
    }

    #[test]
    fn owner_and_houses_round_trip() {
        for (owner, houses) in [(0i32, 0i32), (-1, 4), (3, 2), (7, 14)] {
            let (c, o, h, m) = unpack_deal(pack_deal(true, owner, houses, true));
            assert!(c && m);
            assert_eq!((o, h), (owner, houses), "owner={owner} houses={houses}");
        }
    }
}
