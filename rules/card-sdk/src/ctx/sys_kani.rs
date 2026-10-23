//! Kani-side `sys` surface: plain Rust wrappers over [`crate::kani_host`].
//! Signatures match the `extern` block in `sys.rs` exactly.

use crate::kani_host::{self, Args, Op};

pub fn roll(player_id: i32, count: i32, sides: i32) -> i32 {
    kani_host::call_i32(Op::roll, [player_id, count, sides, 0, 0, 0])
}

pub fn roll_ask(player_id: i32, count: i32, sides: i32, source: i32) -> i32 {
    kani_host::call_i32(Op::roll_ask, [player_id, count, sides, source, 0, 0])
}

pub fn log(player_id: i32, ptr: i32, len: i32) {
    kani_host::call_unit(Op::log, [player_id, ptr, len, 0, 0, 0]);
}

pub fn effect(player_id: i32, ptr: i32, len: i32) {
    kani_host::call_unit(Op::effect, [player_id, ptr, len, 0, 0, 0]);
}

pub fn flash() {
    kani_host::call_unit(Op::flash, [0, 0, 0, 0, 0, 0]);
}

pub fn no_flash() {
    kani_host::call_unit(Op::no_flash, [0, 0, 0, 0, 0, 0]);
}

pub fn flash_card(cp: i32, cl: i32, p: i32, n: i32) {
    kani_host::call_unit(Op::flash_card, [cp, cl, p, n, 0, 0]);
}

pub fn flash_card_keyed(kp: i32, kl: i32, cp: i32, cl: i32, p: i32, n: i32) {
    kani_host::call_unit(Op::flash_card_keyed, [kp, kl, cp, cl, p, n]);
}

pub fn flash_tile(tile: i32, p: i32, n: i32) {
    kani_host::call_unit(Op::flash_tile, [tile, p, n, 0, 0, 0]);
}

pub fn tile_count() -> i32 {
    kani_host::call_i32(Op::tile_count, [0, 0, 0, 0, 0, 0])
}

pub fn tile_named(ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::tile_named, [ptr, len, 0, 0, 0, 0])
}

pub fn tile_owner(tile: i32) -> i32 {
    kani_host::call_i32(Op::tile_owner, [tile, 0, 0, 0, 0, 0])
}

pub fn player_pos(player_id: i32) -> i32 {
    kani_host::call_i32(Op::player_pos, [player_id, 0, 0, 0, 0, 0])
}

pub fn tile_steps_ahead(player_id: i32, steps: i32) -> i32 {
    kani_host::call_i32(Op::tile_steps_ahead, [player_id, steps, 0, 0, 0, 0])
}

pub fn rent_of(tile: i32) -> i32 {
    kani_host::call_i32(Op::rent_of, [tile, 0, 0, 0, 0, 0])
}

pub fn buy_price(tile: i32) -> i32 {
    kani_host::call_i32(Op::buy_price, [tile, 0, 0, 0, 0, 0])
}

pub fn build_cost(tile: i32) -> i32 {
    kani_host::call_i32(Op::build_cost, [tile, 0, 0, 0, 0, 0])
}

pub fn mortgage_value(tile: i32) -> i32 {
    kani_host::call_i32(Op::mortgage_value, [tile, 0, 0, 0, 0, 0])
}

pub fn owned_count(player_id: i32) -> i32 {
    kani_host::call_i32(Op::owned_count, [player_id, 0, 0, 0, 0, 0])
}

pub fn owned_at(player_id: i32, index: i32) -> i32 {
    kani_host::call_i32(Op::owned_at, [player_id, index, 0, 0, 0, 0])
}

pub fn tile_is(tile: i32, kind_ptr: i32, kind_len: i32) -> i32 {
    kani_host::call_i32(Op::tile_is, [tile, kind_ptr, kind_len, 0, 0, 0])
}

pub fn tile_attr(tile: i32, attr_ptr: i32, attr_len: i32) -> i32 {
    kani_host::call_i32(Op::tile_attr, [tile, attr_ptr, attr_len, 0, 0, 0])
}

pub fn tile_in(player_id: i32, tile: i32, class_ptr: i32, class_len: i32) -> i32 {
    kani_host::call_i32(Op::tile_in, [player_id, tile, class_ptr, class_len, 0, 0])
}

pub fn tile_group(tile: i32) -> i32 {
    kani_host::call_i32(Op::tile_group, [tile, 0, 0, 0, 0, 0])
}

pub fn tile_price(tile: i32) -> i32 {
    kani_host::call_i32(Op::tile_price, [tile, 0, 0, 0, 0, 0])
}

pub fn houses_of(tile: i32) -> i32 {
    kani_host::call_i32(Op::houses_of, [tile, 0, 0, 0, 0, 0])
}

pub fn rent_houses_of(tile: i32) -> i32 {
    kani_host::call_i32(Op::rent_houses_of, [tile, 0, 0, 0, 0, 0])
}

pub fn set_houses(tile: i32, n: i32) {
    kani_host::call_unit(Op::set_houses, [tile, n, 0, 0, 0, 0]);
}

pub fn add_house(tile: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::add_house, [tile, n, 0, 0, 0, 0])
}

pub fn mortgaged_of(tile: i32) -> i32 {
    kani_host::call_i32(Op::mortgaged_of, [tile, 0, 0, 0, 0, 0])
}

pub fn set_mortgaged(tile: i32, v: i32) {
    kani_host::call_unit(Op::set_mortgaged, [tile, v, 0, 0, 0, 0]);
}

pub fn set_owner(tile: i32, player_id: i32) {
    kani_host::call_unit(Op::set_owner, [tile, player_id, 0, 0, 0, 0]);
}

pub fn dist(a: i32, b: i32) -> i32 {
    kani_host::call_i32(Op::dist, [a, b, 0, 0, 0, 0])
}

pub fn tile_forward(a: i32, b: i32) -> i32 {
    kani_host::call_i32(Op::tile_forward, [a, b, 0, 0, 0, 0])
}

pub fn neighbor(player_id: i32, dir: i32) -> i32 {
    kani_host::call_i32(Op::neighbor, [player_id, dir, 0, 0, 0, 0])
}

pub fn players_on_count(tile: i32, except: i32) -> i32 {
    kani_host::call_i32(Op::players_on_count, [tile, except, 0, 0, 0, 0])
}

pub fn players_on_at(tile: i32, except: i32, index: i32) -> i32 {
    kani_host::call_i32(Op::players_on_at, [tile, except, index, 0, 0, 0])
}

pub fn player_count() -> i32 {
    // The 2..=4 clamp lives in `ctx::player_count` (next to the card body's
    // `0..n` Range). See that wrapper for why.
    kani_host::call_i32(Op::player_count, [0, 0, 0, 0, 0, 0])
}

pub fn player_out(player_id: i32) -> i32 {
    kani_host::call_i32(Op::player_out, [player_id, 0, 0, 0, 0, 0])
}

pub fn others_count(player_id: i32) -> i32 {
    kani_host::call_i32(Op::others_count, [player_id, 0, 0, 0, 0, 0])
}

pub fn others_at(player_id: i32, index: i32) -> i32 {
    kani_host::call_i32(Op::others_at, [player_id, index, 0, 0, 0, 0])
}

pub fn money(player_id: i32) -> i32 {
    kani_host::call_i32(Op::money, [player_id, 0, 0, 0, 0, 0])
}

pub fn gain(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::gain, [player_id, amount, ptr, len, 0, 0])
}

pub fn gain_typed(player_id: i32, amount: i32, tp: i32, tl: i32, xp: i32, xl: i32) -> i32 {
    kani_host::call_i32(Op::gain_typed, [player_id, amount, tp, tl, xp, xl])
}

pub fn adjust_money(player_id: i32, delta: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::adjust_money, [player_id, delta, ptr, len, 0, 0])
}

pub fn pay(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::pay, [player_id, amount, ptr, len, 0, 0])
}

pub fn pay_to(from: i32, to: i32, amount: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::pay_to, [from, to, amount, ptr, len, 0])
}

pub fn pay_total(from: i32, to: i32, amount: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::pay_total, [from, to, amount, ptr, len, 0])
}

pub fn pay_leg(from: i32, to: i32, amount: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::pay_leg, [from, to, amount, ptr, len, 0])
}

pub fn draw(player_id: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::draw, [player_id, n, 0, 0, 0, 0])
}

pub fn draw_event(player_id: i32) -> i32 {
    kani_host::call_i32(Op::draw_event, [player_id, 0, 0, 0, 0, 0])
}

pub fn pay_rent(player_id: i32, tile: i32, half: i32) -> i32 {
    kani_host::call_i32(Op::pay_rent, [player_id, tile, half, 0, 0, 0])
}

pub fn charge_rent(player_id: i32, tile: i32, milli_scale: i32) -> i32 {
    kani_host::call_i32(Op::charge_rent, [player_id, tile, milli_scale, 0, 0, 0])
}

pub fn offer_buy(player_id: i32, tile: i32) -> i32 {
    kani_host::call_i32(Op::offer_buy, [player_id, tile, 0, 0, 0, 0])
}

pub fn offer_force_buy(player_id: i32, tile: i32) -> i32 {
    kani_host::call_i32(Op::offer_force_buy, [player_id, tile, 0, 0, 0, 0])
}

pub fn offer_build(player_id: i32, tile: i32) -> i32 {
    kani_host::call_i32(Op::offer_build, [player_id, tile, 0, 0, 0, 0])
}

pub fn add_to_hand(player_id: i32, ptr: i32, len: i32) {
    kani_host::call_unit(Op::add_to_hand, [player_id, ptr, len, 0, 0, 0]);
}

pub fn add_to_deck(player_id: i32, ptr: i32, len: i32, shuffle: i32) {
    kani_host::call_unit(Op::add_to_deck, [player_id, ptr, len, shuffle, 0, 0]);
}

pub fn add_to_deck_at(player_id: i32, ptr: i32, len: i32, pos: i32) {
    kani_host::call_unit(Op::add_to_deck_at, [player_id, ptr, len, pos, 0, 0]);
}

pub fn take_card(player_id: i32, pile: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::take_card, [player_id, pile, ptr, len, 0, 0])
}

pub fn cards_in(player_id: i32, pile: i32, buf: i32, cap: i32) -> i32 {
    kani_host::call_i32(Op::cards_in, [player_id, pile, buf, cap, 0, 0])
}

pub fn to_discard(player_id: i32, ptr: i32, len: i32) {
    kani_host::call_unit(Op::to_discard, [player_id, ptr, len, 0, 0, 0]);
}

pub fn hand_count(player_id: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::hand_count, [player_id, ptr, len, 0, 0, 0])
}

pub fn hand_size(player_id: i32) -> i32 {
    kani_host::call_i32(Op::hand_size, [player_id, 0, 0, 0, 0, 0])
}

pub fn discard_count(player_id: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::discard_count, [player_id, ptr, len, 0, 0, 0])
}

pub fn deck_count(player_id: i32) -> i32 {
    kani_host::call_i32(Op::deck_count, [player_id, 0, 0, 0, 0, 0])
}

pub fn discard_size(player_id: i32) -> i32 {
    kani_host::call_i32(Op::discard_size, [player_id, 0, 0, 0, 0, 0])
}

pub fn discard_from_hand(player_id: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::discard_from_hand, [player_id, ptr, len, 0, 0, 0])
}

pub fn shuffle_into_deck(player_id: i32, hand: i32, discard: i32) -> i32 {
    kani_host::call_i32(Op::shuffle_into_deck, [player_id, hand, discard, 0, 0, 0])
}

pub fn place_card_at(player_id: i32, cp: i32, cl: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::place_card_at, [player_id, cp, cl, ptr, len, 0])
}

pub fn set_dest(dest: i32) {
    kani_host::call_unit(Op::set_dest, [dest, 0, 0, 0, 0, 0]);
}

pub fn set_transfer_to_dest(to: i32, dest: i32) {
    kani_host::call_unit(Op::set_transfer_to_dest, [to, dest, 0, 0, 0, 0]);
}

pub fn send_to_dest(dest: i32) -> i32 {
    kani_host::call_i32(Op::send_to_dest, [dest, 0, 0, 0, 0, 0])
}

pub fn transfer_to_dest(to: i32, dest: i32) -> i32 {
    kani_host::call_i32(Op::transfer_to_dest, [to, dest, 0, 0, 0, 0])
}

pub fn rent_unit_mult() -> i32 {
    kani_host::call_i32(Op::rent_unit_mult, [0, 0, 0, 0, 0, 0])
}

pub fn add_rent_unit_bonus(n: i32) -> i32 {
    kani_host::call_i32(Op::add_rent_unit_bonus, [n, 0, 0, 0, 0, 0])
}

pub fn teleport_to(player_id: i32, tile: i32) {
    kani_host::call_unit(Op::teleport_to, [player_id, tile, 0, 0, 0, 0]);
}

pub fn unplace_card() -> i32 {
    kani_host::call_i32(Op::unplace_card, [0, 0, 0, 0, 0, 0])
}

pub fn event_expire(ip: i32, il: i32, removed: i32) {
    kani_host::call_unit(Op::event_expire, [ip, il, removed, 0, 0, 0]);
}

pub fn event_is_active(ip: i32, il: i32) -> i32 {
    kani_host::call_i32(Op::event_is_active, [ip, il, 0, 0, 0, 0])
}

pub fn event_deck_push(ip: i32, il: i32, face_down: i32) {
    kani_host::call_unit(Op::event_deck_push, [ip, il, face_down, 0, 0, 0]);
}

pub fn event_banish(ip: i32, il: i32) {
    kani_host::call_unit(Op::event_banish, [ip, il, 0, 0, 0, 0]);
}

pub fn self_tile() -> i32 {
    kani_host::call_i32(Op::self_tile, [0, 0, 0, 0, 0, 0])
}

pub fn set_self_tile(tile: i32) -> i32 {
    kani_host::call_i32(Op::set_self_tile, [tile, 0, 0, 0, 0, 0])
}

pub fn self_face_down() -> i32 {
    kani_host::call_i32(Op::self_face_down, [0, 0, 0, 0, 0, 0])
}

pub fn set_self_face_down(on: i32) -> i32 {
    kani_host::call_i32(Op::set_self_face_down, [on, 0, 0, 0, 0, 0])
}

pub fn self_immune() -> i32 {
    kani_host::call_i32(Op::self_immune, [0, 0, 0, 0, 0, 0])
}

pub fn set_self_immune(on: i32) -> i32 {
    kani_host::call_i32(Op::set_self_immune, [on, 0, 0, 0, 0, 0])
}

pub fn is_placed() -> i32 {
    kani_host::call_i32(Op::is_placed, [0, 0, 0, 0, 0, 0])
}

pub fn self_prop(kp: i32, kl: i32) -> i32 {
    kani_host::call_i32(Op::self_prop, [kp, kl, 0, 0, 0, 0])
}

pub fn set_self_prop(kp: i32, kl: i32, v: i32) -> i32 {
    kani_host::call_i32(Op::set_self_prop, [kp, kl, v, 0, 0, 0])
}

pub fn tile_prop(tile: i32, kp: i32, kl: i32) -> i32 {
    kani_host::call_i32(Op::tile_prop, [tile, kp, kl, 0, 0, 0])
}

pub fn set_tile_prop(tile: i32, kp: i32, kl: i32, v: i32) -> i32 {
    kani_host::call_i32(Op::set_tile_prop, [tile, kp, kl, v, 0, 0])
}

pub fn prop_at(uid: i32, kp: i32, kl: i32) -> i32 {
    kani_host::call_i32(Op::prop_at, [uid, kp, kl, 0, 0, 0])
}

pub fn set_prop_at(uid: i32, kp: i32, kl: i32, v: i32) -> i32 {
    kani_host::call_i32(Op::set_prop_at, [uid, kp, kl, v, 0, 0])
}

pub fn counter_self(np: i32, nl: i32) -> i32 {
    kani_host::call_i32(Op::counter_self, [np, nl, 0, 0, 0, 0])
}

pub fn add_counter_self(np: i32, nl: i32, n: i32, max: i32) -> i32 {
    kani_host::call_i32(Op::add_counter_self, [np, nl, n, max, 0, 0])
}

pub fn set_counter_self(np: i32, nl: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::set_counter_self, [np, nl, n, 0, 0, 0])
}

pub fn counter_at(uid: i32, np: i32, nl: i32) -> i32 {
    kani_host::call_i32(Op::counter_at, [uid, np, nl, 0, 0, 0])
}

pub fn add_counter_at(uid: i32, np: i32, nl: i32, n: i32, max: i32) -> i32 {
    kani_host::call_i32(Op::add_counter_at, [uid, np, nl, n, max, 0])
}

pub fn card_counter(player_id: i32, cp: i32, cl: i32, np: i32, nl: i32) -> i32 {
    kani_host::call_i32(Op::card_counter, [player_id, cp, cl, np, nl, 0])
}

pub fn add_card_counter(
    player_id: i32,
    cp: i32,
    cl: i32,
    np: i32,
    nl: i32,
    n: i32,
    max: i32,
) -> i32 {
    kani_host::call_i32(Op::add_card_counter, [player_id, cp, cl, np, nl, n])
}

pub fn place_mark(
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
) -> i32 {
    kani_host::call_i32(Op::place_mark, [tile, kp, kl, cp, cl, owner])
}

pub fn place_mark_pair(
    a: i32,
    b: i32,
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
) -> i32 {
    kani_host::call_i32(Op::place_mark_pair, [a, b, kp, kl, cp, cl])
}

pub fn count_marks_f(tile: i32, fp: i32, fl: i32) -> i32 {
    kani_host::call_i32(Op::count_marks_f, [tile, fp, fl, 0, 0, 0])
}

pub fn bump_mark_f(tile: i32, fp: i32, fl: i32, delta: i32) -> i32 {
    kani_host::call_i32(Op::bump_mark_f, [tile, fp, fl, delta, 0, 0])
}

pub fn remove_marks_f(tile: i32, fp: i32, fl: i32) -> i32 {
    kani_host::call_i32(Op::remove_marks_f, [tile, fp, fl, 0, 0, 0])
}

pub fn mark_src_at(tile: i32, fp: i32, fl: i32) -> i32 {
    kani_host::call_i32(Op::mark_src_at, [tile, fp, fl, 0, 0, 0])
}

pub fn mark_instance_at(tile: i32, fp: i32, fl: i32) -> i32 {
    kani_host::call_i32(Op::mark_instance_at, [tile, fp, fl, 0, 0, 0])
}

pub fn mark_partner(tile: i32, fp: i32, fl: i32) -> i32 {
    kani_host::call_i32(Op::mark_partner, [tile, fp, fl, 0, 0, 0])
}

pub fn count_held(np: i32, nl: i32, player_id: i32) -> i32 {
    kani_host::call_i32(Op::count_held, [np, nl, player_id, 0, 0, 0])
}

pub fn add_held(np: i32, nl: i32, player_id: i32, n: i32, max: i32) -> i32 {
    kani_host::call_i32(Op::add_held, [np, nl, player_id, n, max, 0])
}

pub fn count_held_name(np: i32, nl: i32, player_id: i32) -> i32 {
    kani_host::call_i32(Op::count_held_name, [np, nl, player_id, 0, 0, 0])
}

pub fn set_held_name(np: i32, nl: i32, player_id: i32, v: i32) {
    kani_host::call_unit(Op::set_held_name, [np, nl, player_id, v, 0, 0]);
}

pub fn move_units(
    np: i32,
    nl: i32,
    from_tile: i32,
    from_player: i32,
    to_tile: i32,
    to_player: i32,
    n: i32,
) -> i32 {
    kani_host::call_i32(
        Op::move_units,
        [np, nl, from_tile, from_player, to_tile, to_player],
    )
}

pub fn self_uid() -> i32 {
    kani_host::call_i32(Op::self_uid, [0, 0, 0, 0, 0, 0])
}

pub fn send(tp: i32, tl: i32, np: i32, nl: i32, pp: i32, pl: i32) -> i32 {
    kani_host::call_i32(Op::send, [tp, tl, np, nl, pp, pl])
}

pub fn msg_sender_uid() -> i32 {
    kani_host::call_i32(Op::msg_sender_uid, [0, 0, 0, 0, 0, 0])
}

pub fn msg_sender_seat() -> i32 {
    kani_host::call_i32(Op::msg_sender_seat, [0, 0, 0, 0, 0, 0])
}

pub fn msg_name(p: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::msg_name, [p, n, 0, 0, 0, 0])
}

pub fn msg_a() -> i32 {
    kani_host::call_i32(Op::msg_a, [0, 0, 0, 0, 0, 0])
}

pub fn msg_b() -> i32 {
    kani_host::call_i32(Op::msg_b, [0, 0, 0, 0, 0, 0])
}

pub fn msg_c() -> i32 {
    kani_host::call_i32(Op::msg_c, [0, 0, 0, 0, 0, 0])
}

pub fn msg_tile() -> i32 {
    kani_host::call_i32(Op::msg_tile, [0, 0, 0, 0, 0, 0])
}

pub fn msg_seat() -> i32 {
    kani_host::call_i32(Op::msg_seat, [0, 0, 0, 0, 0, 0])
}

pub fn msg_text(p: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::msg_text, [p, n, 0, 0, 0, 0])
}

pub fn msg_reply(v: i32) {
    kani_host::call_unit(Op::msg_reply, [v, 0, 0, 0, 0, 0]);
}

pub fn state_get(player_id: i32, ptr: i32, len: i32, field: i32) -> i32 {
    kani_host::call_i32(Op::state_get, [player_id, ptr, len, field, 0, 0])
}

pub fn state_set(player_id: i32, ptr: i32, len: i32, field: i32, v: i32) -> i32 {
    kani_host::call_i32(Op::state_set, [player_id, ptr, len, field, v, 0])
}

pub fn state_add(player_id: i32, ptr: i32, len: i32, delta: i32) -> i32 {
    kani_host::call_i32(Op::state_add, [player_id, ptr, len, delta, 0, 0])
}

pub fn state_set_flags(player_id: i32, ptr: i32, len: i32, flags: i32) {
    let _ = kani_host::call_i32(Op::state_set_flags, [player_id, ptr, len, flags, 0, 0]);
}

pub fn slot(player_id: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::slot, [player_id, ptr, len, 0, 0, 0])
}

pub fn set_slot(player_id: i32, ptr: i32, len: i32, v: i32) {
    kani_host::call_unit(Op::set_slot, [player_id, ptr, len, v, 0, 0]);
}

pub fn inc_slot(player_id: i32, ptr: i32, len: i32, by: i32) -> i32 {
    kani_host::call_i32(Op::inc_slot, [player_id, ptr, len, by, 0, 0])
}

pub fn group_pool(player_id: i32, np: i32, nl: i32) -> i32 {
    kani_host::call_i32(Op::group_pool, [player_id, np, nl, 0, 0, 0])
}

pub fn add_group_pool(player_id: i32, np: i32, nl: i32, n: i32, max: i32) -> i32 {
    kani_host::call_i32(Op::add_group_pool, [player_id, np, nl, n, max, 0])
}

pub fn group_skill(player_id: i32, buf: i32, cap: i32) -> i32 {
    kani_host::call_i32(Op::group_skill, [player_id, buf, cap, 0, 0, 0])
}

pub fn persona_skill(player_id: i32, buf: i32, cap: i32) -> i32 {
    kani_host::call_i32(Op::persona_skill, [player_id, buf, cap, 0, 0, 0])
}

pub fn group_skills(player_id: i32, buf: i32, cap: i32) -> i32 {
    kani_host::call_i32(Op::group_skills, [player_id, buf, cap, 0, 0, 0])
}

pub fn add_group_skill(player_id: i32, p: i32, n: i32, extra: i32) -> i32 {
    kani_host::call_i32(Op::add_group_skill, [player_id, p, n, extra, 0, 0])
}

pub fn invoke_skill(player_id: i32, p: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::invoke_skill, [player_id, p, n, 0, 0, 0])
}

pub fn raise_bought(player_id: i32, tile: i32) -> i32 {
    kani_host::call_i32(Op::raise_bought, [player_id, tile, 0, 0, 0, 0])
}

pub fn raise(player_id: i32, kind_p: i32, kind_l: i32, value: i32) -> i32 {
    kani_host::call_i32(Op::raise, [player_id, kind_p, kind_l, value, 0, 0])
}

pub fn gain_marker(
    player_id: i32,
    name_p: i32,
    name_l: i32,
    n: i32,
    why_p: i32,
    why_l: i32,
) -> i32 {
    kani_host::call_i32(
        Op::gain_marker,
        [player_id, name_p, name_l, n, why_p, why_l],
    )
}

pub fn spend_marker(
    player_id: i32,
    name_p: i32,
    name_l: i32,
    n: i32,
    why_p: i32,
    why_l: i32,
) -> i32 {
    kani_host::call_i32(
        Op::spend_marker,
        [player_id, name_p, name_l, n, why_p, why_l],
    )
}

pub fn add_marker_max(player_id: i32, name_p: i32, name_l: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::add_marker_max, [player_id, name_p, name_l, n, 0, 0])
}

pub fn raise_after(player_id: i32, name_p: i32, name_l: i32, value: i32) {
    kani_host::call_unit(Op::raise_after, [player_id, name_p, name_l, value, 0, 0]);
}

pub fn skip_main_move(player_id: i32, locked: i32) {
    kani_host::call_unit(Op::skip_main_move, [player_id, locked, 0, 0, 0, 0]);
}

pub fn clear_move_skip(player_id: i32) {
    kani_host::call_unit(Op::clear_move_skip, [player_id, 0, 0, 0, 0, 0]);
}

pub fn give_extra_turn(player_id: i32) {
    kani_host::call_unit(Op::give_extra_turn, [player_id, 0, 0, 0, 0, 0]);
}

pub fn end_turn_now(player_id: i32) {
    kani_host::call_unit(Op::end_turn_now, [player_id, 0, 0, 0, 0, 0]);
}

pub fn teleport_as(player_id: i32, tile: i32, resolve: i32, as_main: i32) {
    kani_host::call_unit(Op::teleport_as, [player_id, tile, resolve, as_main, 0, 0]);
}

pub fn log_as(player_id: i32, kp: i32, kn: i32, p: i32, n: i32) {
    kani_host::call_unit(Op::log_as, [player_id, kp, kn, p, n, 0]);
}

pub fn can_pay(player_id: i32) -> i32 {
    kani_host::call_i32(Op::can_pay, [player_id, 0, 0, 0, 0, 0])
}

pub fn cant_move(player_id: i32) -> i32 {
    kani_host::call_i32(Op::cant_move, [player_id, 0, 0, 0, 0, 0])
}

pub fn turn_player() -> i32 {
    kani_host::call_i32(Op::turn_player, [0, 0, 0, 0, 0, 0])
}

pub fn round_no() -> i32 {
    kani_host::call_i32(Op::round_no, [0, 0, 0, 0, 0, 0])
}

pub fn turn_key() -> i32 {
    kani_host::call_i32(Op::turn_key, [0, 0, 0, 0, 0, 0])
}

pub fn persona_is(player_id: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::persona_is, [player_id, ptr, len, 0, 0, 0])
}

pub fn set_persona(player_id: i32, ptr: i32, len: i32) {
    kani_host::call_unit(Op::set_persona, [player_id, ptr, len, 0, 0, 0]);
}

pub fn in_group(player_id: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::in_group, [player_id, ptr, len, 0, 0, 0])
}

pub fn opt_int(v: i32) {
    kani_host::call_unit(Op::opt_int, [v, 0, 0, 0, 0, 0]);
}

pub fn opt_str(ptr: i32, len: i32) {
    kani_host::call_unit(Op::opt_str, [ptr, len, 0, 0, 0, 0]);
}

pub fn opt_tile(tile: i32, lp: i32, ll: i32) {
    kani_host::call_unit(Op::opt_tile, [tile, lp, ll, 0, 0, 0]);
}

pub fn opt_price(price: i32) {
    kani_host::call_unit(Op::opt_price, [price, 0, 0, 0, 0, 0]);
}

pub fn opt_ai(ai: i32) {
    kani_host::call_unit(Op::opt_ai, [ai, 0, 0, 0, 0, 0]);
}

pub fn opt_count(count: i32) {
    kani_host::call_unit(Op::opt_count, [count, 0, 0, 0, 0, 0]);
}

pub fn ask(
    kind: i32,
    player_id: i32,
    title_ptr: i32,
    title_len: i32,
    text_ptr: i32,
    text_len: i32,
) -> i32 {
    kani_host::call_i32(
        Op::ask,
        [kind, player_id, title_ptr, title_len, text_ptr, text_len],
    )
}

pub fn trig_kind(p: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::trig_kind, [p, n, 0, 0, 0, 0])
}

pub fn trig_player() -> i32 {
    kani_host::call_i32(Op::trig_player, [0, 0, 0, 0, 0, 0])
}

pub fn trig_target() -> i32 {
    kani_host::call_i32(Op::trig_target, [0, 0, 0, 0, 0, 0])
}

pub fn trig_tile() -> i32 {
    kani_host::call_i32(Op::trig_tile, [0, 0, 0, 0, 0, 0])
}

pub fn trig_value() -> i32 {
    kani_host::call_i32(Op::trig_value, [0, 0, 0, 0, 0, 0])
}

pub fn trig_name(p: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::trig_name, [p, n, 0, 0, 0, 0])
}

pub fn trig_step() -> i32 {
    kani_host::call_i32(Op::trig_step, [0, 0, 0, 0, 0, 0])
}

pub fn trig_by_card() -> i32 {
    kani_host::call_i32(Op::trig_by_card, [0, 0, 0, 0, 0, 0])
}

pub fn trig_pay_is_rent() -> i32 {
    kani_host::call_i32(Op::trig_pay_is_rent, [0, 0, 0, 0, 0, 0])
}

pub fn trig_move_kind() -> i32 {
    kani_host::call_i32(Op::trig_move_kind, [0, 0, 0, 0, 0, 0])
}

pub fn trig_move_resolve() -> i32 {
    kani_host::call_i32(Op::trig_move_resolve, [0, 0, 0, 0, 0, 0])
}

pub fn trig_move_tag(kp: i32, kl: i32) -> i32 {
    kani_host::call_i32(Op::trig_move_tag, [kp, kl, 0, 0, 0, 0])
}

pub fn trig_move_main() -> i32 {
    kani_host::call_i32(Op::trig_move_main, [0, 0, 0, 0, 0, 0])
}

pub fn trig_settle_redirect() -> i32 {
    kani_host::call_i32(Op::trig_settle_redirect, [0, 0, 0, 0, 0, 0])
}

pub fn trig_move_dir() -> i32 {
    kani_host::call_i32(Op::trig_move_dir, [0, 0, 0, 0, 0, 0])
}

pub fn trig_move_remaining() -> i32 {
    kani_host::call_i32(Op::trig_move_remaining, [0, 0, 0, 0, 0, 0])
}

pub fn trig_move_from() -> i32 {
    kani_host::call_i32(Op::trig_move_from, [0, 0, 0, 0, 0, 0])
}

pub fn trig_move_total() -> i32 {
    kani_host::call_i32(Op::trig_move_total, [0, 0, 0, 0, 0, 0])
}

pub fn trig_cards(buf: i32, cap: i32) -> i32 {
    kani_host::call_i32(Op::trig_cards, [buf, cap, 0, 0, 0, 0])
}

pub fn trig_move_roll() -> i32 {
    kani_host::call_i32(Op::trig_move_roll, [0, 0, 0, 0, 0, 0])
}

pub fn trig_roll_source() -> i32 {
    kani_host::call_i32(Op::trig_roll_source, [0, 0, 0, 0, 0, 0])
}

pub fn trig_buy_kind() -> i32 {
    kani_host::call_i32(Op::trig_buy_kind, [0, 0, 0, 0, 0, 0])
}

pub fn trig_seller() -> i32 {
    kani_host::call_i32(Op::trig_seller, [0, 0, 0, 0, 0, 0])
}

pub fn trig_price() -> i32 {
    kani_host::call_i32(Op::trig_price, [0, 0, 0, 0, 0, 0])
}

pub fn trig_set_price(v: i32) {
    kani_host::call_unit(Op::trig_set_price, [v, 0, 0, 0, 0, 0]);
}

pub fn trig_deal_owner() -> i32 {
    kani_host::call_i32(Op::trig_deal_owner, [0, 0, 0, 0, 0, 0])
}

pub fn trig_set_deal_owner(v: i32) {
    kani_host::call_unit(Op::trig_set_deal_owner, [v, 0, 0, 0, 0, 0]);
}

pub fn trig_deal_houses() -> i32 {
    kani_host::call_i32(Op::trig_deal_houses, [0, 0, 0, 0, 0, 0])
}

pub fn trig_set_deal_houses(v: i32) {
    kani_host::call_unit(Op::trig_set_deal_houses, [v, 0, 0, 0, 0, 0]);
}

pub fn trig_deal_mortgaged() -> i32 {
    kani_host::call_i32(Op::trig_deal_mortgaged, [0, 0, 0, 0, 0, 0])
}

pub fn trig_set_deal_mortgaged(v: i32) {
    kani_host::call_unit(Op::trig_set_deal_mortgaged, [v, 0, 0, 0, 0, 0]);
}

pub fn trig_set_reason(ptr: i32, len: i32) {
    kani_host::call_unit(Op::trig_set_reason, [ptr, len, 0, 0, 0, 0]);
}

pub fn trig_set_move_roll(v: i32) {
    kani_host::call_unit(Op::trig_set_move_roll, [v, 0, 0, 0, 0, 0]);
}

pub fn trig_set_pay_amount(v: i32) {
    kani_host::call_unit(Op::trig_set_pay_amount, [v, 0, 0, 0, 0, 0]);
}

pub fn trig_set_pay_target(to: i32) {
    kani_host::call_unit(Op::trig_set_pay_target, [to, 0, 0, 0, 0, 0]);
}

pub fn join_redirect_target() -> i32 {
    kani_host::call_i32(Op::join_redirect_target, [0, 0, 0, 0, 0, 0])
}

pub fn set_join_used() {
    kani_host::call_unit(Op::set_join_used, [0, 0, 0, 0, 0, 0]);
}

pub fn trig_set_cancelled() {
    kani_host::call_unit(Op::trig_set_cancelled, [0, 0, 0, 0, 0, 0]);
}

pub fn trig_set_negate_effect() {
    kani_host::call_unit(Op::trig_set_negate_effect, [0, 0, 0, 0, 0, 0]);
}

pub fn trig_set_spare(seat: i32) {
    kani_host::call_unit(Op::trig_set_spare, [seat, 0, 0, 0, 0, 0]);
}

pub fn trig_cancelled() -> i32 {
    kani_host::call_i32(Op::trig_cancelled, [0, 0, 0, 0, 0, 0])
}

pub fn trig_seq() -> i32 {
    kani_host::call_i32(Op::trig_seq, [0, 0, 0, 0, 0, 0])
}

pub fn trig_answers() -> i32 {
    kani_host::call_i32(Op::trig_answers, [0, 0, 0, 0, 0, 0])
}

pub fn trig_effect_count() -> i32 {
    kani_host::call_i32(Op::trig_effect_count, [0, 0, 0, 0, 0, 0])
}

pub fn trig_effect_kind(i: i32, p: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::trig_effect_kind, [i, p, n, 0, 0, 0])
}

pub fn trig_effect_target(i: i32) -> i32 {
    kani_host::call_i32(Op::trig_effect_target, [i, 0, 0, 0, 0, 0])
}

pub fn trig_effect_from(i: i32) -> i32 {
    kani_host::call_i32(Op::trig_effect_from, [i, 0, 0, 0, 0, 0])
}

pub fn trig_effect_tile(i: i32) -> i32 {
    kani_host::call_i32(Op::trig_effect_tile, [i, 0, 0, 0, 0, 0])
}

pub fn trig_effect_value(i: i32) -> i32 {
    kani_host::call_i32(Op::trig_effect_value, [i, 0, 0, 0, 0, 0])
}

pub fn declare_effect(kind_p: i32, kind_l: i32, target: i32, from: i32, tile: i32, value: i32) {
    kani_host::call_unit(
        Op::declare_effect,
        [kind_p, kind_l, target, from, tile, value],
    );
}

pub fn trig_card_is(ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::trig_card_is, [ptr, len, 0, 0, 0, 0])
}

pub fn play_card(id_ptr: i32, id_len: i32, player_id: i32) -> i32 {
    kani_host::call_i32(Op::play_card, [id_ptr, id_len, player_id, 0, 0, 0])
}

pub fn card_move(player_id: i32) -> i32 {
    kani_host::call_i32(Op::card_move, [player_id, 0, 0, 0, 0, 0])
}

pub fn card_replayable(player_id: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::card_replayable, [player_id, ptr, len, 0, 0, 0])
}

pub fn set_roller(player_id: i32) {
    kani_host::call_unit(Op::set_roller, [player_id, 0, 0, 0, 0, 0]);
}

pub fn set_steps(n: i32) {
    kani_host::call_unit(Op::set_steps, [n, 0, 0, 0, 0, 0]);
}

pub fn set_reverse(on: i32) {
    kani_host::call_unit(Op::set_reverse, [on, 0, 0, 0, 0, 0]);
}

pub fn set_signed(on: i32) {
    kani_host::call_unit(Op::set_signed, [on, 0, 0, 0, 0, 0]);
}

pub fn set_stop_at(n: i32) {
    kani_host::call_unit(Op::set_stop_at, [n, 0, 0, 0, 0, 0]);
}

pub fn set_parity(n: i32) {
    kani_host::call_unit(Op::set_parity, [n, 0, 0, 0, 0, 0]);
}

pub fn set_resolve(on: i32) {
    kani_host::call_unit(Op::set_resolve, [on, 0, 0, 0, 0, 0]);
}

pub fn set_settle_tile(n: i32) {
    kani_host::call_unit(Op::set_settle_tile, [n, 0, 0, 0, 0, 0]);
}

pub fn set_pay_factor(n: i32) {
    kani_host::call_unit(Op::set_pay_factor, [n, 0, 0, 0, 0, 0]);
}

pub fn set_rent_factor(n: i32) {
    kani_host::call_unit(Op::set_rent_factor, [n, 0, 0, 0, 0, 0]);
}

pub fn set_no_buy(on: i32) {
    kani_host::call_unit(Op::set_no_buy, [on, 0, 0, 0, 0, 0]);
}

pub fn set_no_build(on: i32) {
    kani_host::call_unit(Op::set_no_build, [on, 0, 0, 0, 0, 0]);
}

pub fn set_build_anywhere(on: i32) {
    kani_host::call_unit(Op::set_build_anywhere, [on, 0, 0, 0, 0, 0]);
}

pub fn set_kind(kind: i32) {
    kani_host::call_unit(Op::set_kind, [kind, 0, 0, 0, 0, 0]);
}

pub fn set_teleport_to(tile: i32) {
    kani_host::call_unit(Op::set_teleport_to, [tile, 0, 0, 0, 0, 0]);
}

pub fn set_start(tile: i32, ptr: i32, len: i32) {
    kani_host::call_unit(Op::set_start, [tile, ptr, len, 0, 0, 0]);
}

pub fn set_base_dice(count: i32, sides: i32, ptr: i32, len: i32) {
    kani_host::call_unit(Op::set_base_dice, [count, sides, ptr, len, 0, 0]);
}

pub fn add_base_dice(count: i32, sides: i32, ptr: i32, len: i32) {
    kani_host::call_unit(Op::add_base_dice, [count, sides, ptr, len, 0, 0]);
}

pub fn add_extra_dice(count: i32, sides: i32, ptr: i32, len: i32) {
    kani_host::call_unit(Op::add_extra_dice, [count, sides, ptr, len, 0, 0]);
}

pub fn move_stopped() -> i32 {
    kani_host::call_i32(Op::move_stopped, [0, 0, 0, 0, 0, 0])
}

pub fn set_min_roll(n: i32) {
    kani_host::call_unit(Op::set_min_roll, [n, 0, 0, 0, 0, 0]);
}

pub fn set_extra_steps(n: i32) {
    kani_host::call_unit(Op::set_extra_steps, [n, 0, 0, 0, 0, 0]);
}

pub fn set_more_steps(n: i32) {
    kani_host::call_unit(Op::set_more_steps, [n, 0, 0, 0, 0, 0]);
}

pub fn set_tag(kp: i32, kl: i32, v: i32) {
    kani_host::call_unit(Op::set_tag, [kp, kl, v, 0, 0, 0]);
}

pub fn set_tag_id(key_id: i32, v: i32) {
    kani_host::call_unit(Op::set_tag_id, [key_id, v, 0, 0, 0, 0]);
}

pub fn recorded_roll() -> i32 {
    kani_host::call_i32(Op::recorded_roll, [0, 0, 0, 0, 0, 0])
}

pub fn set_settle_as_agent(on: i32) {
    kani_host::call_unit(Op::set_settle_as_agent, [on, 0, 0, 0, 0, 0]);
}

pub fn plan_add_follower(player_id: i32) {
    kani_host::call_unit(Op::plan_add_follower, [player_id, 0, 0, 0, 0, 0]);
}

pub fn move_stop_at() -> i32 {
    kani_host::call_i32(Op::move_stop_at, [0, 0, 0, 0, 0, 0])
}

pub fn move_parity() -> i32 {
    kani_host::call_i32(Op::move_parity, [0, 0, 0, 0, 0, 0])
}

pub fn move_resolve() -> i32 {
    kani_host::call_i32(Op::move_resolve, [0, 0, 0, 0, 0, 0])
}

pub fn move_steps() -> i32 {
    kani_host::call_i32(Op::move_steps, [0, 0, 0, 0, 0, 0])
}

pub fn move_remaining() -> i32 {
    kani_host::call_i32(Op::move_remaining, [0, 0, 0, 0, 0, 0])
}

pub fn move_total() -> i32 {
    kani_host::call_i32(Op::move_total, [0, 0, 0, 0, 0, 0])
}

pub fn move_dir() -> i32 {
    kani_host::call_i32(Op::move_dir, [0, 0, 0, 0, 0, 0])
}

pub fn abnormal_count(player_id: i32) -> i32 {
    kani_host::call_i32(Op::abnormal_count, [player_id, 0, 0, 0, 0, 0])
}

pub fn target(player_id: i32, tile: i32, single: i32) -> i32 {
    kani_host::call_i32(Op::target, [player_id, tile, single, 0, 0, 0])
}

pub fn targeted_count(player_id: i32) -> i32 {
    kani_host::call_i32(Op::targeted_count, [player_id, 0, 0, 0, 0, 0])
}

pub fn gains_this_turn(player_id: i32) -> i32 {
    kani_host::call_i32(Op::gains_this_turn, [player_id, 0, 0, 0, 0, 0])
}

pub fn placed_tile(player_id: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::placed_tile, [player_id, ptr, len, 0, 0, 0])
}

pub fn play_doubled() -> i32 {
    kani_host::call_i32(Op::play_doubled, [0, 0, 0, 0, 0, 0])
}

pub fn set_play_doubled(n: i32) {
    kani_host::call_unit(Op::set_play_doubled, [n, 0, 0, 0, 0, 0]);
}

pub fn schedule_turn_end(player_id: i32, mode: i32) {
    kani_host::call_unit(Op::schedule_turn_end, [player_id, mode, 0, 0, 0, 0]);
}

pub fn set_no_money_loss(player_id: i32) {
    kani_host::call_unit(Op::set_no_money_loss, [player_id, 0, 0, 0, 0, 0]);
}

pub fn set_fixed_roll(n: i32) {
    kani_host::call_unit(Op::set_fixed_roll, [n, 0, 0, 0, 0, 0]);
}

pub fn fixed_roll() -> i32 {
    kani_host::call_i32(Op::fixed_roll, [0, 0, 0, 0, 0, 0])
}

pub fn set_next_steps(player_id: i32, n: i32) {
    kani_host::call_unit(Op::set_next_steps, [player_id, n, 0, 0, 0, 0]);
}

pub fn turn_main_steps() -> i32 {
    kani_host::call_i32(Op::turn_main_steps, [0, 0, 0, 0, 0, 0])
}

pub fn card_build(player_id: i32, tile: i32) -> i32 {
    kani_host::call_i32(Op::card_build, [player_id, tile, 0, 0, 0, 0])
}

pub fn set_can_build(on: i32) {
    kani_host::call_unit(Op::set_can_build, [on, 0, 0, 0, 0, 0]);
}

pub fn clear_dice() {
    kani_host::call_unit(Op::clear_dice, [0, 0, 0, 0, 0, 0]);
}

pub fn can_build_on(player_id: i32, tile: i32) -> i32 {
    kani_host::call_i32(Op::can_build_on, [player_id, tile, 0, 0, 0, 0])
}

pub fn card_buy(player_id: i32, tile: i32) -> i32 {
    kani_host::call_i32(Op::card_buy, [player_id, tile, 0, 0, 0, 0])
}

pub fn buy_quotes(player_id: i32, kind: i32, buf: i32, n: i32, out: i32) -> i32 {
    kani_host::call_i32(Op::buy_quotes, [player_id, kind, buf, n, out, 0])
}

pub fn ai_agent_choice(player_id: i32, buf: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::ai_agent_choice, [player_id, buf, n, 0, 0, 0])
}

pub fn buy(player_id: i32, tile: i32, kind: i32) -> i32 {
    kani_host::call_i32(Op::buy, [player_id, tile, kind, 0, 0, 0])
}

pub fn acquire(player_id: i32, from: i32, tile: i32, price: i32) -> i32 {
    kani_host::call_i32(Op::acquire, [player_id, from, tile, price, 0, 0])
}

pub fn agent_offer(player_id: i32, agent: i32, tile: i32, kind: i32) -> i32 {
    kani_host::call_i32(Op::agent_offer, [player_id, agent, tile, kind, 0, 0])
}

pub fn linger(player_id: i32, expires: i32) -> i32 {
    kani_host::call_i32(Op::linger, [player_id, expires, 0, 0, 0, 0])
}

pub fn card_immune(player_id: i32, cp: i32, cl: i32) -> i32 {
    kani_host::call_i32(Op::card_immune, [player_id, cp, cl, 0, 0, 0])
}

pub fn card_mortgage(player_id: i32, tile: i32) -> i32 {
    kani_host::call_i32(Op::card_mortgage, [player_id, tile, 0, 0, 0, 0])
}

pub fn apply_mortgage(player_id: i32, tile: i32, amount: i32) -> i32 {
    kani_host::call_i32(Op::apply_mortgage, [player_id, tile, amount, 0, 0, 0])
}

pub fn apply_redeem(player_id: i32, tile: i32, amount: i32) -> i32 {
    kani_host::call_i32(Op::apply_redeem, [player_id, tile, amount, 0, 0, 0])
}

pub fn pay_buy(player_id: i32, tile: i32, amount: i32, kind: i32) -> i32 {
    kani_host::call_i32(Op::pay_buy, [player_id, tile, amount, kind, 0, 0])
}

pub fn pay_build(player_id: i32, tile: i32, amount: i32) -> i32 {
    kani_host::call_i32(Op::pay_build, [player_id, tile, amount, 0, 0, 0])
}

pub fn assign_deed(player_id: i32, tile: i32, owner: i32, houses: i32, mortgaged: i32) -> i32 {
    kani_host::call_i32(
        Op::assign_deed,
        [player_id, tile, owner, houses, mortgaged, 0],
    )
}

pub fn card_text_mentions(cp: i32, cl: i32, np: i32, nl: i32) -> i32 {
    kani_host::call_i32(Op::card_text_mentions, [cp, cl, np, nl, 0, 0])
}

pub fn card_prop(ip: i32, il: i32, kp: i32, kl: i32) -> i32 {
    kani_host::call_i32(Op::card_prop, [ip, il, kp, kl, 0, 0])
}

pub fn gain_fixed(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::gain_fixed, [player_id, amount, ptr, len, 0, 0])
}

pub fn paid_in_settle() -> i32 {
    kani_host::call_i32(Op::paid_in_settle, [0, 0, 0, 0, 0, 0])
}

pub fn place_card_on(player_id: i32, tile: i32, cp: i32, cl: i32, ptr: i32, len: i32) -> i32 {
    kani_host::call_i32(Op::place_card_on, [player_id, tile, cp, cl, ptr, len])
}

pub fn play_from_hand() -> i32 {
    kani_host::call_i32(Op::play_from_hand, [0, 0, 0, 0, 0, 0])
}

pub fn set_build_discount(n: i32, layers: i32) {
    kani_host::call_unit(Op::set_build_discount, [n, layers, 0, 0, 0, 0]);
}

pub fn set_card_immune(player_id: i32, cp: i32, cl: i32, on: i32) -> i32 {
    kani_host::call_i32(Op::set_card_immune, [player_id, cp, cl, on, 0, 0])
}

pub fn set_card_tile(player_id: i32, cp: i32, cl: i32, tile: i32) -> i32 {
    kani_host::call_i32(Op::set_card_tile, [player_id, cp, cl, tile, 0, 0])
}

pub fn set_extreme(v: i32) {
    kani_host::call_unit(Op::set_extreme, [v, 0, 0, 0, 0, 0]);
}

pub fn turn_rolls(buf: i32, cap: i32) -> i32 {
    kani_host::call_i32(Op::turn_rolls, [buf, cap, 0, 0, 0, 0])
}

pub fn turn_snap(player_id: i32, buf: i32) -> i32 {
    kani_host::call_i32(Op::turn_snap, [player_id, buf, 0, 0, 0, 0])
}

pub fn turn_start_pos(player_id: i32) -> i32 {
    kani_host::call_i32(Op::turn_start_pos, [player_id, 0, 0, 0, 0, 0])
}

pub fn unplace_card_named(player_id: i32, cp: i32, cl: i32) -> i32 {
    kani_host::call_i32(Op::unplace_card_named, [player_id, cp, cl, 0, 0, 0])
}

pub fn tok_names(player_id: i32, p: i32, n: i32, buf: i32, cap: i32) -> i32 {
    kani_host::call_i32(Op::tok_names, [player_id, p, n, buf, cap, 0])
}

pub fn designations(player_id: i32, buf: i32, cap: i32) -> i32 {
    kani_host::call_i32(Op::designations, [player_id, buf, cap, 0, 0, 0])
}

pub fn cancel_designation(seat: i32) {
    kani_host::call_unit(Op::cancel_designation, [seat, 0, 0, 0, 0, 0]);
}

pub fn designation_cancelled(seat: i32) -> i32 {
    kani_host::call_i32(Op::designation_cancelled, [seat, 0, 0, 0, 0, 0])
}

pub fn gate(player_id: i32, kind: i32) -> i32 {
    kani_host::call_i32(Op::gate, [player_id, kind, 0, 0, 0, 0])
}

pub fn card_settle_at(player_id: i32, tile: i32, main: i32) -> i32 {
    kani_host::call_i32(Op::card_settle_at, [player_id, tile, main, 0, 0, 0])
}

pub fn card_offer_build(player_id: i32, buf: i32, n: i32) -> i32 {
    kani_host::call_i32(Op::card_offer_build, [player_id, buf, n, 0, 0, 0])
}

pub fn do_move_roll(player_id: i32) -> i32 {
    kani_host::call_i32(Op::do_move_roll, [player_id, 0, 0, 0, 0, 0])
}

pub fn placed_cards(player_id: i32, buf: i32, cap: i32) -> i32 {
    kani_host::call_i32(Op::placed_cards, [player_id, buf, cap, 0, 0, 0])
}

pub fn field_instances(player_id: i32, buf: i32, cap: i32) -> i32 {
    kani_host::call_i32(Op::field_instances, [player_id, buf, cap, 0, 0, 0])
}

pub fn unplace_at(uid: i32) -> i32 {
    kani_host::call_i32(Op::unplace_at, [uid, 0, 0, 0, 0, 0])
}

pub fn tile_at(uid: i32) -> i32 {
    kani_host::call_i32(Op::tile_at, [uid, 0, 0, 0, 0, 0])
}

pub fn set_tile_at(uid: i32, tile: i32) -> i32 {
    kani_host::call_i32(Op::set_tile_at, [uid, tile, 0, 0, 0, 0])
}

pub fn is_face_down_at(uid: i32) -> i32 {
    kani_host::call_i32(Op::is_face_down_at, [uid, 0, 0, 0, 0, 0])
}

pub fn set_face_down_at(uid: i32, on: i32) -> i32 {
    kani_host::call_i32(Op::set_face_down_at, [uid, on, 0, 0, 0, 0])
}

pub fn is_immune_at(uid: i32) -> i32 {
    kani_host::call_i32(Op::is_immune_at, [uid, 0, 0, 0, 0, 0])
}

pub fn set_immune_at(uid: i32, on: i32) -> i32 {
    kani_host::call_i32(Op::set_immune_at, [uid, on, 0, 0, 0, 0])
}

pub fn set_build_cost_pct(pct: i32) {
    kani_host::call_unit(Op::set_build_cost_pct, [pct, 0, 0, 0, 0, 0]);
}

pub fn set_card_face_down(player_id: i32, cp: i32, cl: i32, down: i32) -> i32 {
    kani_host::call_i32(Op::set_card_face_down, [player_id, cp, cl, down, 0, 0])
}

pub fn card_face_down(player_id: i32, cp: i32, cl: i32) -> i32 {
    kani_host::call_i32(Op::card_face_down, [player_id, cp, cl, 0, 0, 0])
}
