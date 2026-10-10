//! Backend-neutral `bandori` host import bodies.
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
    engine_msg, CallOut, GuestMem, HostCtx, HostErr, HostRequest, HostState, Prompt,
    PromptOption, RulesHandle, MAX_NESTING,
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

/// The C# `AbnormalGate` from inside a card run (see `host.rs`).
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

pub fn tile_count<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().tile_count())
}

/// Bind `count` units of the running instance's counter `kind` to `tile`.
pub fn place_mark<C: HostCtx>(
    c: &mut C,
    tile: i32,
    kp: i32,
    kl: i32,
    cp: i32,
    cl: i32,
    owner: i32,
    src: i32,
    count: i32,
    stack: i32,
    np: i32,
    nl: i32,
) -> Result<i32, HostErr> {
    let kind = guest_str(c, kp, kl)?;
    let category = guest_str(c, cp, cl)?;
    let note = guest_msg(c, np, nl)?;
    let fresh = stack != 0; // Stack::Fresh = 1
    Ok(c.st_mut().w().place_mark(
        tile, &kind, &category, owner, src, count, note, fresh,
    ))
}

fn guest_filter<C: HostCtx>(c: &mut C, fp: i32, fl: i32) -> Result<game_core::state::MarkFilter<'static>, HostErr> {
    let bytes = c.read_guest(fp, fl)?;
    let f: card_sdk::abi::MarkFilter = postcard::from_bytes(&bytes)
        .map_err(|_| HostErr::trap("guest mark filter is not MarkFilter postcard"))?;
    Ok(game_core::state::MarkFilter {
        kind: Box::leak(f.kind.into_boxed_str()),
        category: Box::leak(f.category.into_boxed_str()),
        owner: f.owner,
        src: f.src,
        instance: f.instance,
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

pub fn mark_instance_at<C: HostCtx>(c: &mut C, tile: i32, fp: i32, fl: i32) -> Result<i32, HostErr> {
    let f = guest_filter(c, fp, fl)?;
    Ok(c.st().wr().mark_instance_at(tile, &f))
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
    cp: i32,
    cl: i32,
    np: i32,
    nl: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cp, cl)?;
    let name = guest_str(c, np, nl)?;
    Ok(c.st().wr().card_counter(player_id, &card, &name))
}

pub fn add_card_counter<C: HostCtx>(
    c: &mut C,
    player_id: i32,
    cp: i32,
    cl: i32,
    np: i32,
    nl: i32,
    n: i32,
    max: i32,
) -> Result<i32, HostErr> {
    let card = guest_str(c, cp, cl)?;
    let name = guest_str(c, np, nl)?;
    Ok(c.st_mut().w().add_card_counter(player_id, &card, &name, n, max))
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
) -> Result<(), HostErr> {
    let name = guest_str(c, np, nl)?;
    c.st_mut().w().set_held_name(&name, player_id, v);
    Ok(())
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
    Ok(c.st_mut().w().move_units(
        &name, from_tile, from_player, to_tile, to_player, n,
    ))
}

pub fn self_uid<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().self_uid())
}

pub fn money<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().money(player_id)
    })
}

pub fn gain<C: HostCtx>(c: &mut C, player_id: i32, amount: i32, p: i32, n: i32) -> Result<i32, HostErr> {
            let src = guest_msg(c, p, n)?;
                        // A card-driven gain runs the same `Money` pipeline as a payment --
            // print (game -> player) -- so PayAdd / PayChoose / the `effect`
            // [反击] window all see it. The pause/resume is the same as `pay`:
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

pub fn pay<C: HostCtx>(c: &mut C, player_id: i32, amount: i32, p: i32, n: i32) -> Result<i32, HostErr> {
            let src = guest_msg(c, p, n)?;
                        // A card-driven payment is paused so the engine can raise a `pay`
            // trigger (and the [反击] window) before the money moves. The answer is
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

pub fn pay_to<C: HostCtx>(c: &mut C, from: i32, to: i32, amount: i32, p: i32, n: i32) -> Result<i32, HostErr> {
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

pub fn pay_total<C: HostCtx>(c: &mut C, from: i32, to: i32, amount: i32, p: i32, n: i32) -> Result<i32, HostErr> {
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

pub fn pay_leg<C: HostCtx>(c: &mut C, from: i32, to: i32, amount: i32, p: i32, n: i32) -> Result<i32, HostErr> {
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
    Ok({
        c.st().wr().player_out(player_id)
    })
}

pub fn others_count<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().others_count(player_id)
    })
}

pub fn others_at<C: HostCtx>(c: &mut C, player_id: i32, index: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().others_at(player_id, index)
    })
}

pub fn tile_named<C: HostCtx>(c: &mut C, p: i32, n: i32) -> Result<i32, HostErr> {
            let name = guest_str(c, p, n)?;
            Ok(c.st().wr().tile_named(&name))
}

pub fn tile_owner<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().tile_owner(tile)
    })
}

pub fn player_pos<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().player_pos(player_id)
    })
}

pub fn tile_steps_ahead<C: HostCtx>(c: &mut C, player_id: i32, steps: i32) -> Result<i32, HostErr> {
    Ok(c.st().wr().tile_steps_ahead(player_id, steps),)
}

pub fn rent_of<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().rent_of(tile)
    })
}

pub fn buy_price<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().buy_price(tile)
    })
}

pub fn build_cost<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().build_cost(tile)
    })
}

pub fn mortgage_value<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().mortgage_value(tile)
    })
}

pub fn owned_count<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().owned_count(player_id)
    })
}

pub fn owned_at<C: HostCtx>(c: &mut C, player_id: i32, index: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().owned_at(player_id, index)
    })
}

pub fn is_buyable<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().is_buyable(tile)
    })
}

pub fn is_shop<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().is_shop(tile)
    })
}

pub fn is_ring<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().is_ring(tile)
    })
}

pub fn is_circle<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().is_circle(tile)
    })
}

pub fn is_color<C: HostCtx>(c: &mut C, player_id: i32, tile: i32, group: i32) -> Result<i32, HostErr> {
    Ok({
            c.st().wr().is_color(player_id, tile, group) as i32
    })
}

pub fn is_live_house_for<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<i32, HostErr> {
    Ok(c.st().wr().is_live_house_for(player_id, tile),)
}

pub fn paid_in_settle<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().paid_in_settle()
    })
}

pub fn turn_rolls<C: HostCtx>(c: &mut C, buf: i32, cap: i32) -> Result<i32, HostErr> {
            let rolls = c.st().wr().turn_rolls();
            let bytes = postcard::to_allocvec(&rolls)
                .map_err(|e| HostErr::trap(format!("turn_rolls encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                c.write_guest( buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
}

pub fn field_instances<C: HostCtx>(c: &mut C, player_id: i32, buf: i32, cap: i32) -> Result<i32, HostErr> {
            let inst = c.st().wr().field_instances(player_id);
            let bytes = postcard::to_allocvec(&inst)
                .map_err(|e| HostErr::trap(format!("field_instances encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                c.write_guest( buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
}


pub fn unplace_at<C: HostCtx>(c: &mut C, uid: i32) -> Result<i32, HostErr> {
    Ok({
        c.st_mut().w().unplace_at(uid)
    })
}

pub fn tile_at<C: HostCtx>(c: &mut C, uid: i32) -> Result<i32, HostErr> {
    Ok(c.st().wr().tile_at(uid))
}

pub fn set_tile_at<C: HostCtx>(c: &mut C, uid: i32, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st_mut().w().set_tile_at(uid, tile) as i32
    })
}

pub fn is_face_down_at<C: HostCtx>(c: &mut C, uid: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().is_face_down_at(uid) as i32
    })
}

pub fn set_face_down_at<C: HostCtx>(c: &mut C, uid: i32, on: i32) -> Result<i32, HostErr> {
    Ok({
        c.st_mut().w().set_face_down_at(uid, on != 0) as i32
    })
}

pub fn is_immune_at<C: HostCtx>(c: &mut C, uid: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().is_immune_at(uid) as i32
    })
}

pub fn set_immune_at<C: HostCtx>(c: &mut C, uid: i32, on: i32) -> Result<i32, HostErr> {
    Ok({
        c.st_mut().w().set_immune_at(uid, on != 0) as i32
    })
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
    Ok({
        c.st().wr().turn_start_pos(player_id)
    })
}

pub fn turn_snap<C: HostCtx>(c: &mut C, player_id: i32, buf: i32) -> Result<i32, HostErr> {
            let (pos, stay, stun, exile) = c.st().wr().turn_snap(player_id);
            let mut out = [0u8; 16];
            for (i, v) in [pos, stay, stun, exile].into_iter().enumerate() {
                out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            c.write_guest( buf, &out)?;
            Ok(16)
}

pub fn is_agent<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().is_agent(tile)
    })
}

pub fn is_live_house<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().is_live_house(tile)
    })
}

pub fn tile_group<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().tile_group(tile)
    })
}

pub fn tile_price<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().tile_price(tile)
    })
}

pub fn houses_of<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().houses_of(tile)
    })
}

pub fn rent_houses_of<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().rent_houses_of(tile)
    })
}

pub fn set_houses<C: HostCtx>(c: &mut C, tile: i32, n: i32) -> Result<(), HostErr> {
        c.st_mut().w().set_houses(tile, n);
        Ok(())
}

pub fn add_house<C: HostCtx>(c: &mut C, tile: i32, n: i32) -> Result<i32, HostErr> {
    Ok({
        c.st_mut().w().add_house(tile, n)
    })
}

pub fn mortgaged_of<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().mortgaged_of(tile)
    })
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
    Ok({
        c.st().wr().dist(a, b)
    })
}

pub fn tile_forward<C: HostCtx>(c: &mut C, a: i32, b: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().tile_forward(a, b)
    })
}

pub fn neighbor<C: HostCtx>(c: &mut C, player_id: i32, dir: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().neighbor(player_id, dir)
    })
}

pub fn players_on_count<C: HostCtx>(c: &mut C, tile: i32, except: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().players_on_count(tile, except)
    })
}

pub fn players_on_at<C: HostCtx>(c: &mut C, tile: i32, except: i32, index: i32) -> Result<i32, HostErr> {
    Ok({
            c.st().wr().players_on_at(tile, except, index)
    })
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

pub fn pay_rent<C: HostCtx>(c: &mut C, player_id: i32, tile: i32, half: i32) -> Result<i32, HostErr> {
                        {
                let req = HostRequest::PayRent {
                player_id,
                tile,
                half: half != 0,
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

pub fn add_to_hand<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<(), HostErr> {
            let card = guest_str(c, p, n)?;
            c.st_mut().w().add_to_hand(player_id, &card);
            Ok(())
}

pub fn add_to_deck<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32, shuffle: i32) -> Result<(), HostErr> {
            let card = guest_str(c, p, n)?;
            c.st_mut().w().add_to_deck(player_id, &card, shuffle != 0);
            Ok(())
}

pub fn add_to_deck_at<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32, pos: i32) -> Result<(), HostErr> {
            let card = guest_str(c, p, n)?;
            c.st_mut().w().add_to_deck_at(player_id, &card, pos);
            Ok(())
}

pub fn take_card<C: HostCtx>(c: &mut C, player_id: i32, pile: i32, p: i32, n: i32) -> Result<i32, HostErr> {
            let pile = crate::CardPile::from_i32(pile)
                .ok_or_else(|| HostErr::trap(format!("bad card pile {pile}")))?;
            let id = guest_str(c, p, n)?;
            Ok(c.st_mut().w().take_card(player_id, pile, &id) as i32)
}

pub fn cards_in<C: HostCtx>(c: &mut C, player_id: i32, pile: i32, buf: i32, cap: i32) -> Result<i32, HostErr> {
            let pile = crate::CardPile::from_i32(pile)
                .ok_or_else(|| HostErr::trap(format!("bad card pile {pile}")))?;
            let list = c.st().wr().cards_in(player_id, pile);
            let bytes =
                postcard::to_allocvec(&list).map_err(|e| HostErr::trap(format!("cards_in encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                c.write_guest( buf, &bytes)?;
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

pub fn discard_count<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<i32, HostErr> {
            let card = guest_str(c, p, n)?;
            Ok(c.st().wr().discard_count(player_id, &card))
}

pub fn hand_size<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().hand_size(player_id)
    })
}

pub fn deck_count<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().deck_count(player_id)
    })
}

pub fn discard_size<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().discard_size(player_id)
    })
}

pub fn discard_from_hand<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<i32, HostErr> {
            let card = guest_str(c, p, n)?;
            Ok(c.st_mut().w().discard_from_hand(player_id, &card))
}

pub fn shuffle_into_deck<C: HostCtx>(c: &mut C, player_id: i32, hand: i32, discard: i32) -> Result<i32, HostErr> {
    Ok({
            c.st_mut()
                .w()
                .shuffle_into_deck(player_id, hand != 0, discard != 0)
    })
}

pub fn unplace_card<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st_mut().w().unplace_card()
    })
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

pub fn event_deck_push<C: HostCtx>(c: &mut C, ip: i32, il: i32, face_down: i32) -> Result<(), HostErr> {
            let id = guest_str(c, ip, il)?;
            c.st_mut().w().event_deck_push(&id, face_down != 0);
            Ok(())
}

pub fn event_banish<C: HostCtx>(c: &mut C, ip: i32, il: i32) -> Result<(), HostErr> {
            let id = guest_str(c, ip, il)?;
            c.st_mut().w().event_banish(&id);
            Ok(())
}

pub fn placed_cards<C: HostCtx>(c: &mut C, player_id: i32, buf: i32, cap: i32) -> Result<i32, HostErr> {
            let names = c.st().wr().placed_cards(player_id);
            let bytes = postcard::to_allocvec(&names)
                .map_err(|e| HostErr::trap(format!("placed_cards encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                c.write_guest( buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
}

pub fn card_text_mentions<C: HostCtx>(c: &mut C, cp: i32, cl: i32, np: i32, nl: i32) -> Result<i32, HostErr> {
            let card = guest_str(c, cp, cl)?;
            let needle = guest_str(c, np, nl)?;
            Ok(c.st().wr().card_text_mentions(&card, &needle) as i32)
}



pub fn is_placed<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().is_placed())
}

pub fn self_tile<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().self_tile())
}

pub fn set_self_tile<C: HostCtx>(c: &mut C, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st_mut().w().set_self_tile(tile) as i32
    })
}

pub fn self_face_down<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().self_face_down() as i32
    })
}

pub fn set_self_face_down<C: HostCtx>(c: &mut C, on: i32) -> Result<i32, HostErr> {
    Ok({
        c.st_mut().w().set_self_face_down(on != 0) as i32
    })
}

pub fn self_immune<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().self_immune() as i32
    })
}

pub fn set_self_immune<C: HostCtx>(c: &mut C, on: i32) -> Result<i32, HostErr> {
    Ok({
        c.st_mut().w().set_self_immune(on != 0) as i32
    })
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

pub fn set_tile_prop<C: HostCtx>(c: &mut C, tile: i32, kp: i32, kl: i32, v: i32) -> Result<i32, HostErr> {
            let key = guest_str(c, kp, kl)?;
            Ok(c.st_mut().w().set_tile_prop(tile, &key, v))
}

pub fn prop_at<C: HostCtx>(c: &mut C, uid: i32, kp: i32, kl: i32) -> Result<i32, HostErr> {
            let key = guest_str(c, kp, kl)?;
            Ok(c.st().wr().prop_at(uid, &key))
}

pub fn set_prop_at<C: HostCtx>(c: &mut C, uid: i32, kp: i32, kl: i32, v: i32) -> Result<i32, HostErr> {
            let key = guest_str(c, kp, kl)?;
            Ok(c.st_mut().w().set_prop_at(uid, &key, v))
}





pub fn state_get<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32, field: i32) -> Result<i32, HostErr> {
            let key = guest_str(c, p, n)?;
            Ok(match field {
                1 => c.st().wr().state_min(player_id, &key),
                2 => c.st().wr().state_max(player_id, &key),
                3 => match c.st().wr().state_expires(player_id, &key) {
                    None => 0,
                    Some(game_core::state::Tick::TurnStart) => 1,
                    Some(game_core::state::Tick::TurnEnd) => 2,
                },
                _ => c.st().wr().state_get(player_id, &key),
            })
}

pub fn state_set<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32, field: i32, v: i32) -> Result<i32, HostErr> {
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
                _ => c.st_mut().w().state_set(player_id, &key, v),
            })
}

pub fn state_add<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32, delta: i32) -> Result<i32, HostErr> {
            let key = guest_str(c, p, n)?;
            Ok(c.st_mut().w().state_add(player_id, &key, delta))
}

pub fn slot<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<i32, HostErr> {
            let key = guest_str(c, p, n)?;
            Ok(c.st().wr().slot(player_id, &key))
}

pub fn set_slot<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32, v: i32) -> Result<(), HostErr> {
            let key = guest_str(c, p, n)?;
            c.st_mut().w().set_slot(player_id, &key, v);
            Ok(())
}

pub fn inc_slot<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32, by: i32) -> Result<i32, HostErr> {
            let key = guest_str(c, p, n)?;
            Ok(c.st_mut().w().inc_slot(player_id, &key, by))
}

// `band_crystals` / `add_band_crystals` stay on the guest surface for now
// (`ctx::band_crystals` still imports them); they are sugar over the
// band-skill instance's `counter::CRYSTALS` (see `ops.rs` `band_skill_uid`).
pub fn band_crystals<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok(c.st().wr().band_crystals(player_id))
}

pub fn add_band_crystals<C: HostCtx>(c: &mut C, player_id: i32, n: i32, max: i32) -> Result<i32, HostErr> {
    Ok(c.st_mut().w().add_band_crystals(player_id, n, max))
}

pub fn band_skill<C: HostCtx>(c: &mut C, player_id: i32, buf: i32, cap: i32) -> Result<i32, HostErr> {
            let s = c.st().wr().band_skill_id(player_id).unwrap_or_default();
            let bytes =
                postcard::to_allocvec(&s).map_err(|e| HostErr::trap(format!("band_skill encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                c.write_guest( buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
}

pub fn character_skill<C: HostCtx>(c: &mut C, player_id: i32, buf: i32, cap: i32) -> Result<i32, HostErr> {
            let s = c
                .st()
                .wr()
                .character_skill_id(player_id)
                .unwrap_or_default();
            let bytes = postcard::to_allocvec(&s)
                .map_err(|e| HostErr::trap(format!("character_skill encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                c.write_guest( buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
}

pub fn band_skills<C: HostCtx>(c: &mut C, player_id: i32, buf: i32, cap: i32) -> Result<i32, HostErr> {
            let list = c.st().wr().band_skills(player_id);
            let bytes = postcard::to_allocvec(&list)
                .map_err(|e| HostErr::trap(format!("band_skills encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                c.write_guest( buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
}

pub fn add_band_skill<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32, extra: i32) -> Result<i32, HostErr> {
            let id = guest_str(c, p, n)?;
            Ok(c.st_mut().w().add_band_skill(player_id, &id, extra != 0))
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
    let (res, inner, left) = c.call_entry(nested, card, entry, export::OP_RUN, player_id, false);
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

pub fn raise<C: HostCtx>(c: &mut C, player_id: i32, kind: i32, value: i32) -> Result<i32, HostErr> {
                        {
                let req = HostRequest::Raise {
                player_id,
                kind,
                value,
            };
                return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
            }
}

pub fn fire<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().fire(player_id)
    })
}

pub fn fire_max<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().fire_max(player_id)
    })
}

pub fn gain_fire<C: HostCtx>(c: &mut C, player_id: i32, n: i32, p: i32, l: i32) -> Result<i32, HostErr> {
            let why = guest_msg(c, p, l)?;
            // Marker window: `markerGain` opens before the fire moves.
            //
            // The pause is a **trap**, not the sentinel: `ctx::gain_fire`
            // returns `Result<i32, Prompt>` and a great many bodies discard
            // that result (`ctx::gain_fire(...);`). With the sentinel the body
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
                        name: "fire".into(),
                        delta: n,
                    },
                    crate::inline::Pause::Trap,
                )?;
                if ok != 1 {
                    return Ok(ok);
                }
            }
            Ok(c.st_mut().w().gain_fire(player_id, n, why))
}

pub fn give_stay<C: HostCtx>(c: &mut C, player_id: i32, n: i32) -> Result<(), HostErr> {
            if n < 0 {
                c.st_mut().w().give_stay(player_id, n);
            } else if n > 0 && abnormal_gate(c, player_id, AbKind::Stay)? {
                c.st_mut().w().give_stay(player_id, n);
            }
            Ok(())
}

pub fn give_stun<C: HostCtx>(c: &mut C, player_id: i32, n: i32) -> Result<(), HostErr> {
            if n < 0 {
                c.st_mut().w().give_stun(player_id, n);
            } else if n > 0 && abnormal_gate(c, player_id, AbKind::Stun)? {
                c.st_mut().w().give_stun(player_id, n);
            }
            Ok(())
}

pub fn give_exile<C: HostCtx>(c: &mut C, player_id: i32, n: i32, to: i32) -> Result<(), HostErr> {
            if n > 0 && abnormal_gate(c, player_id, AbKind::Exile)? {
                c.st_mut().w().give_exile(player_id, n, to);
            }
            Ok(())
}

pub fn give_extra_turn<C: HostCtx>(c: &mut C, player_id: i32) -> Result<(), HostErr> {
        c.st_mut().w().give_extra_turn(player_id);
        Ok(())
}

pub fn can_pay<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().can_pay(player_id)
    })
}

pub fn cant_move<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().cant_move(player_id)
    })
}

pub fn spend_fire<C: HostCtx>(c: &mut C, player_id: i32, n: i32, p: i32, l: i32) -> Result<i32, HostErr> {
            let why = guest_msg(c, p, l)?;
            // Marker window (user ruling 2026-10-07): `markerSpend` opens
            // **before** the fire moves. A cancelled link spends nothing.
            if n > 0 {
                let ok = crate::inline::request_or_pause(
                    c,
                    crate::HostRequest::Marker {
                        player_id,
                        name: "fire".into(),
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
            Ok(c.st_mut().w().spend_fire(player_id, n, why))
}

pub fn stay_of<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().stay_of(player_id)
    })
}

pub fn stun_of<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().stun_of(player_id)
    })
}

pub fn exile_of<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().exile_of(player_id)
    })
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

pub fn character_is<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<i32, HostErr> {
            let name = guest_str(c, p, n)?;
            Ok(c.st().wr().character_is(player_id, &name))
}

pub fn in_band<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<i32, HostErr> {
            let name = guest_str(c, p, n)?;
            Ok(c.st().wr().in_band(player_id, &name))
}



pub fn do_move_roll<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
            Ok(c.st_mut().w().do_move_roll(player_id))
}

pub fn tok_names<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32, buf: i32, cap: i32) -> Result<i32, HostErr> {
            let prefix = guest_str(c, p, n)?;
            let names = c.st().wr().tok_names(player_id, &prefix);
            let bytes =
                postcard::to_allocvec(&names).map_err(|e| HostErr::trap(format!("tok_names encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                c.write_guest( buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
}

pub fn unplace_card_named<C: HostCtx>(c: &mut C, player_id: i32, cp: i32, cl: i32) -> Result<i32, HostErr> {
            let card = guest_str(c, cp, cl)?;
            Ok(c.st_mut().w().unplace_card_named(player_id, &card) as i32)
}

pub fn can_build_on<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().can_build_on(player_id, tile) as i32
    })
}

pub fn card_face_down<C: HostCtx>(c: &mut C, player_id: i32, cp: i32, cl: i32) -> Result<i32, HostErr> {
            let card = guest_str(c, cp, cl)?;
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
    Ok({
        c.st().wr().play_from_hand() as i32
    })
}

pub fn gain_fixed<C: HostCtx>(c: &mut C, player_id: i32, amount: i32, p: i32, n: i32) -> Result<i32, HostErr> {
            // V4 (`NEGATION-AUDIT`): Tritone's 「立刻获得此次失去的资金金额」 is
            // money movement and goes through the same `Money` pipeline as any
            // other gain -- the `effect` [反击] window, the modifier stages and
            // the `pay` settlement all see it. (It used to debit/credit the
            // world copy directly, outside every 支付阶段 window.)
            gain(c, player_id, amount, p, n)
}

pub fn set_card_face_down<C: HostCtx>(c: &mut C, player_id: i32, cp: i32, cl: i32, down: i32) -> Result<i32, HostErr> {
            let card = guest_str(c, cp, cl)?;
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
pub fn set_card_immune<C: HostCtx>(c: &mut C, player_id: i32, cp: i32, cl: i32, on: i32) -> Result<i32, HostErr> {
    let card = guest_str(c, cp, cl)?;
    let uid = field_uid_of(c, player_id, &card);
    if uid < 0 {
        return Ok(0);
    }
    Ok(c.st_mut().w().set_immune_at(uid, on != 0) as i32)
}

pub fn card_immune<C: HostCtx>(c: &mut C, player_id: i32, cp: i32, cl: i32) -> Result<i32, HostErr> {
    let card = guest_str(c, cp, cl)?;
    let uid = field_uid_of(c, player_id, &card);
    if uid < 0 {
        return Ok(0);
    }
    Ok(c.st().wr().is_immune_at(uid) as i32)
}

pub fn set_card_tile<C: HostCtx>(c: &mut C, player_id: i32, cp: i32, cl: i32, tile: i32) -> Result<i32, HostErr> {
    let card = guest_str(c, cp, cl)?;
    let uid = field_uid_of(c, player_id, &card);
    if uid < 0 {
        return Ok(0);
    }
    Ok(c.st_mut().w().set_tile_at(uid, tile) as i32)
}

pub fn place_card_on<C: HostCtx>(c: &mut C, player_id: i32, tile: i32, cp: i32, cl: i32, p: i32, n: i32) -> Result<i32, HostErr> {
            let card = guest_str(c, cp, cl)?;
            let note = guest_msg(c, p, n)?;
            Ok(c.st_mut().w().place_card_on(player_id, tile, &card, note))
}

pub fn place_card_at<C: HostCtx>(c: &mut C, player_id: i32, cp: i32, cl: i32, p: i32, n: i32) -> Result<i32, HostErr> {
            let card = guest_str(c, cp, cl)?;
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

pub fn ring_multiplier<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().ring_multiplier()
    })
}

pub fn add_ring_bonus<C: HostCtx>(c: &mut C, n: i32) -> Result<i32, HostErr> {
    Ok({
        c.st_mut().w().add_ring_bonus(n)
    })
}

pub fn teleport_to<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<(), HostErr> {
            // C# `H.ForceTeleport(..., resolve: false)` / a bare `pos` write.
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
            if crate::inline::request_or_inline(c, HostRequest::Teleport { player_id, tile })?.is_some()
            {
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
    Ok({
        c.st().wr().abnormal_count(player_id)
    })
}

pub fn targeted_count<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().targeted_count(player_id)
    })
}

pub fn gains_this_turn<C: HostCtx>(c: &mut C, player_id: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().gains_this_turn(player_id)
    })
}

pub fn designations<C: HostCtx>(c: &mut C, player_id: i32, buf: i32, cap: i32) -> Result<i32, HostErr> {
            let list = c.st().wr().designations(player_id);
            let bytes = postcard::to_allocvec(&list)
                .map_err(|e| HostErr::trap(format!("designations encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                c.write_guest( buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
}

pub fn cancel_designation<C: HostCtx>(c: &mut C, seat: i32) -> Result<(), HostErr> {
        c.st_mut().w().cancel_designation(seat);
        Ok(())
}

pub fn designation_cancelled<C: HostCtx>(c: &mut C, seat: i32) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().designation_cancelled(seat) as i32
    })
}

pub fn target<C: HostCtx>(c: &mut C, player_id: i32, tile: i32, single: i32) -> Result<i32, HostErr> {
            // Paused like `gate`: the engine runs the targeting pipeline (it
            // raises hooks and opens the `target` [反击] window) and the replay
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
            // C# `H.CardMove(c, m)` -- the card shaped the plan and wants the move
            // to run *now*. Paused like the others: the engine runs `Cx::card_move`
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

pub fn roll_ask<C: HostCtx>(c: &mut C, player_id: i32, count: i32, sides: i32, source: i32) -> Result<i32, HostErr> {
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

pub fn card_settle_at<C: HostCtx>(c: &mut C, player_id: i32, tile: i32, main: i32) -> Result<i32, HostErr> {
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

pub fn buy_quotes<C: HostCtx>(c: &mut C, player_id: i32, kind: i32, buf: i32, n: i32, out: i32) -> Result<i32, HostErr> {
            let _ = (player_id, kind);
            let bytes = c.read_guest( buf, n)?;
            let tiles: Vec<i32> = bytes
                .chunks_exact(4)
                .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            // Native quote (`purchase::quote_native`): `buy_price` is the deed
            // plus standing houses; `-1` / not-eligible when the tile is not
            // buyable. Hook-aware quotes (`BuyAdd` / `BuyGate`) re-quote at
            // commit inside `buy`, so the charge is right even when this
            // figure is the native one.
            let quotes: Vec<(i32, bool)> = tiles
                .iter()
                .map(|&t| {
                    if c.st().wr().is_buyable(t) != 0 {
                        (c.st().wr().buy_price(t), true)
                    } else {
                        (-1, false)
                    }
                })
                .collect();
            let bytes = postcard::to_allocvec(&quotes)
                .map_err(|e| HostErr::trap(format!("buy_quotes encode: {e}")))?;
            c.write_guest(out, &bytes)?;
            Ok(bytes.len() as i32)
}

pub fn ai_agent_choice<C: HostCtx>(c: &mut C, player_id: i32, buf: i32, n: i32) -> Result<i32, HostErr> {
            let bytes = c.read_guest( buf, n)?;
            let tiles: Vec<i32> = bytes
                .chunks_exact(4)
                .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
                        {
                let req = HostRequest::AiAgentChoice { player_id, tiles };
                return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
            }
}

pub fn acquire<C: HostCtx>(c: &mut C, player_id: i32, from: i32, tile: i32, price: i32) -> Result<i32, HostErr> {
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

pub fn agent_offer<C: HostCtx>(c: &mut C, player_id: i32, agent: i32, tile: i32, kind: i32) -> Result<i32, HostErr> {
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
                let req = HostRequest::Linger {
                player_id,
                expires,
            };
                return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
            }
}

pub fn card_build<C: HostCtx>(c: &mut C, player_id: i32, tile: i32) -> Result<i32, HostErr> {
                        {
                let req = HostRequest::Build { player_id, tile };
                return crate::inline::request_or_pause(c, req, crate::inline::Pause::Trap);
            }
}

pub fn card_offer_build<C: HostCtx>(c: &mut C, player_id: i32, buf: i32, n: i32) -> Result<i32, HostErr> {
            let bytes = c.read_guest( buf, n)?;
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
            let bytes =
                postcard::to_allocvec(&list).map_err(|e| HostErr::trap(format!("trig_cards encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                c.write_guest( buf, &bytes)?;
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

pub fn ask<C: HostCtx>(c: &mut C, kind: i32, player_id: i32, tp: i32, tl: i32, xp: i32, xl: i32) -> Result<i32, HostErr> {
            let kind =
                PromptKind::from_i32(kind).ok_or_else(|| HostErr::trap(format!("bad prompt kind {kind}")))?;
            let title = guest_msg(c, tp, tl)?;
            let text = guest_msg(c, xp, xl)?;
            let (options, prices, ai_hint, answer_slot) = {
                let st = c.st_mut();
                let options = std::mem::take(&mut st.options);
                if kind != PromptKind::YesNo && options.is_empty() {
                    return Err(HostErr::trap(format!("{} prompt with no options", kind.as_str())));
                }
                let prices = std::mem::take(&mut st.prices);
                let ai_hint = std::mem::replace(&mut st.ai_hint, -1);
                (options, prices, ai_hint, st.next_answer)
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
                },
            )
}

pub fn trig_kind<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().kind as i32
    })
}

pub fn trig_player<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().player_id
    })
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
    Ok({
        c.st().wr().trigger().by_card.unwrap_or(-1)
    })
}

pub fn trig_pay_is_rent<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().pay_is_rent as i32
    })
}

pub fn trig_move_kind<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().move_kind.map_or(-1, |k| k as i32)
    })
}

pub fn trig_move_resolve<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().move_resolve as i32
    })
}

pub fn trig_move_tag<C: HostCtx>(c: &mut C, p: i32, n: i32) -> Result<i32, HostErr> {
            let key = guest_str(c, p, n)?;
            Ok(c.st()
                .wr()
                .trigger()
                .move_tags
                .iter()
                .find(|(k, _)| *k == key)
                .map_or(0, |(_, v)| *v))
}

pub fn trig_move_main<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().move_main as i32
    })
}

pub fn trig_move_dir<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().move_dir
    })
}

pub fn trig_move_remaining<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().move_remaining
    })
}

pub fn trig_move_from<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().move_from
    })
}

pub fn trig_move_total<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().move_total
    })
}

pub fn trig_move_roll<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().move_roll.unwrap_or(-1)
    })
}

pub fn trig_roll_source<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().roll_source
    })
}

pub fn trig_buy_kind<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().buy_kind
    })
}

pub fn trig_seller<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().seller)
}

pub fn trig_price<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().price)
}

pub fn trig_set_price<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
        c.st_mut().w().set_trigger_price(v);
    Ok(())
}

pub fn trig_deal_owner<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().deal_owner
    })
}

pub fn trig_set_deal_owner<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
        c.st_mut().w().set_trigger_deal_owner(v);
    Ok(())
}

pub fn trig_deal_houses<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().deal_houses
    })
}

pub fn trig_set_deal_houses<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
        c.st_mut().w().set_trigger_deal_houses(v);
    Ok(())
}

pub fn trig_deal_mortgaged<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().deal_mortgaged as i32
    })
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
    Ok({
        c.st().wr().trigger().is_cancelled() as i32
    })
}

pub fn trig_seq<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().trigger().seq as i32)
}

pub fn trig_answers<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().answers as i32
    })
}

pub fn trig_effect_count<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().trigger().effects.len() as i32
    })
}

pub fn trig_effect_kind<C: HostCtx>(c: &mut C, i: i32) -> Result<i32, HostErr> {
    Ok({
        let t = c.st().wr().trigger();
        t.effects
            .get(i.max(0) as usize)
            .map_or(-1, |e| crate::TriggerKind::from_str(e.kind) as i32)
    })
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

pub fn declare_effect<C: HostCtx>(c: &mut C, kind: i32, target: i32, from: i32, tile: i32, value: i32) -> Result<(), HostErr> {
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
    let (res, inner, left) = c.call_entry(nested, card, entry, export::OP_RUN, player_id, false);
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

pub fn card_replayable<C: HostCtx>(c: &mut C, player_id: i32, p: i32, n: i32) -> Result<i32, HostErr> {
    let id = guest_str(c, p, n)?;
    let rules = c
        .st()
        .rules
        .clone()
        .ok_or_else(|| HostErr::trap("card_replayable unavailable here"))?;
    let Some(card) = rules.by_id(&id) else {
        return Ok(0);
    };
    let info = match rules.card(card) {
        Some(i) => i,
        None => return Ok(0),
    };
    if !info.has_play() || c.st().depth >= MAX_NESTING {
        return Ok(0);
    }
    let Some(entry) = info.entry(OnKind::Play, None) else {
        return Ok(1);
    };
    // docs/GUARDS.md §4.4 item 5: every path that reaches a guard goes through
    // `admits`. A rejecting condition is a block; the nested `OP_GUARD` is
    // skipped.
    let pre = rules.pre(card, entry);
    // Scope only when a condition exists (`pre == None` never reads it).
    let scope;
    let scope: Option<&crate::cond_pre::WindowScope> = if pre.is_some() {
        scope = crate::cond_pre::window_scope(&crate::cond_pre::fill_window_ambient(
            c.st().wr(),
            player_id,
        ));
        Some(&scope)
    } else {
        None
    };
    let cand = crate::cond_pre::fill_candidate(c.st().wr(), player_id, &id, false);
    let asked = crate::cond_pre::admits_gate(
        pre,
        scope,
        &cand,
        || (),
        || -> Result<Option<()>, HostErr> {
            let mut world = c.st().wr().clone();
            world.enter_card(&id);
            let depth = c.st().depth + 1;
            let nested = HostState::new(rules.clone(), world, vec![], depth);
            let (res, _inner, left) =
                c.call_entry(nested, card, entry, export::OP_GUARD, player_id, true);
            c.set_fuel(left)?;
            let playable = match res {
                Ok(CallOut::Msg(None)) => true,
                Ok(CallOut::Code(0)) => true,
                _ => false,
            };
            Ok(if playable { None } else { Some(()) })
        },
    );
    let playable = matches!(asked, Ok(None));
    Ok(playable as i32)
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
    Ok(c.st_mut().w().set_next_steps(player_id, n),)
}

pub fn turn_main_steps<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().turn_main_steps()
    })
}

pub fn add_fire_max<C: HostCtx>(c: &mut C, player_id: i32, n: i32) -> Result<i32, HostErr> {
    Ok({
        c.st_mut().w().add_fire_max(player_id, n)
    })
}

pub fn set_roller<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
        c.st_mut().w().set_roller(v);
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
        c.st_mut().w().set_stop_at(v);
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
        c.st_mut().w().set_teleport_to(v);
    Ok(())
}

pub fn set_start<C: HostCtx>(c: &mut C, t: i32, p: i32, n: i32) -> Result<(), HostErr> {
            let why = guest_str(c, p, n)?;
            c.st_mut().w().set_start(t, &why);
            Ok(())
}

pub fn set_base_dice<C: HostCtx>(c: &mut C, count: i32, sides: i32, p: i32, n: i32) -> Result<(), HostErr> {
            let why = guest_str(c, p, n)?;
            c.st_mut().w().set_base_dice(count, sides, &why);
            Ok(())
}

pub fn add_base_dice<C: HostCtx>(c: &mut C, count: i32, sides: i32, p: i32, n: i32) -> Result<(), HostErr> {
            let why = guest_str(c, p, n)?;
            c.st_mut().w().add_base_dice(count, sides, &why);
            Ok(())
}

pub fn add_extra_dice<C: HostCtx>(c: &mut C, count: i32, sides: i32, p: i32, n: i32) -> Result<(), HostErr> {
            let why = guest_str(c, p, n)?;
            c.st_mut().w().add_extra_dice(count, sides, &why);
            Ok(())
}

pub fn set_tag<C: HostCtx>(c: &mut C, kp: i32, kl: i32, v: i32) -> Result<(), HostErr> {
            let key = guest_str(c, kp, kl)?;
            c.st_mut().w().set_tag(&key, v);
            Ok(())
}

pub fn set_min_roll<C: HostCtx>(c: &mut C, v: i32) -> Result<(), HostErr> {
        c.st_mut().w().set_min_roll(v);
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
    Ok(c.st().wr().move_stop_at())
}

pub fn move_stopped<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().move_stopped() as i32
    })
}

pub fn move_parity<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().move_parity())
}

pub fn move_resolve<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().move_resolve() as i32
    })
}

pub fn move_kind<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().move_kind())
}

pub fn move_steps<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok(c.st().wr().move_steps())
}

pub fn move_remaining<C: HostCtx>(c: &mut C) -> Result<i32, HostErr> {
    Ok({
        c.st().wr().move_remaining()
    })
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
/// that seat; `Board` names a standing board pseudo card (`mark:cp` and kin)
/// via [`CardWorld::mark_rule_instances`].
fn resolve_receivers<C: HostCtx>(
    c: &C,
    target: &abi::Target,
) -> Vec<(i32, String)> {
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
            for (u, id) in w.mark_rule_instances() {
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
            .mark_rule_instances()
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
        // 2026-10-10: `mark:cp` answers `"place"` and binds units to its own
        // counter). `enter_card` would start a fresh uid like `invoke_skill`;
        // pin `current_uid` to the resolved receiver so `ctx::place_mark`
        // stamps `instance = mark:cp` and `ctx::self_uid()` is that card.
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
        let (res, inner, left) = c.call_entry(nested, card, entry, export::OP_RUN, sender_seat, false);
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
    let s = c.st().wr().trigger().name.clone();
    write_str(c, &s, p, n)
}
