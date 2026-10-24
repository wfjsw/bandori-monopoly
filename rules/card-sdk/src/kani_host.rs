//! Generic host-import dispatch for Kani verification (no game vocabulary).
//!
//! Under `cfg(kani)` the `extern "C-unwind"` `ruleset_*` imports cannot be
//! linked to host shims (Kani treats them as foreign C calls; see
//! kani#2423). Instead `super::sys` becomes ordinary Rust that funnels every
//! import through [`call`]. The verifier crate (`rules-kani`) installs a
//! backend with [`install`] at proof start.
//!
//! Keep this module generic: opcodes and arg shapes only.

#![allow(missing_docs)]

/// Opcodes, one per `ruleset_*` import (see `ctx/sys.rs`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum Op {
    roll = 0,
    roll_ask = 1,
    log = 2,
    effect = 3,
    tile_count = 4,
    tile_named = 5,
    tile_owner = 6,
    player_pos = 7,
    tile_steps_ahead = 8,
    rent_of = 9,
    buy_price = 10,
    build_cost = 11,
    mortgage_value = 12,
    owned_count = 13,
    owned_at = 14,
    // v56: six kind/class ops -> tile_is / tile_attr / tile_in (15,19 free).
    tile_is = 16,
    tile_attr = 17,
    tile_in = 18,
    tile_group = 20,
    tile_price = 21,
    houses_of = 22,
    rent_houses_of = 23,
    set_houses = 24,
    add_house = 25,
    mortgaged_of = 26,
    set_mortgaged = 27,
    set_owner = 28,
    dist = 29,
    tile_forward = 30,
    neighbor = 31,
    players_on_count = 32,
    players_on_at = 33,
    player_count = 34,
    player_out = 35,
    others_count = 36,
    others_at = 37,
    money = 38,
    gain = 39,
    gain_typed = 40,
    pay = 41,
    pay_to = 42,
    pay_total = 43,
    pay_leg = 44,
    draw = 45,
    draw_event = 46,
    pay_rent = 47,
    offer_buy = 48,
    offer_force_buy = 49,
    offer_build = 50,
    add_to_dest = 51,
    // 52/53 freed (was add_to_deck / add_to_deck_at)
    take_card = 54,
    cards_in = 55,
    to_discard = 56,
    hand_count = 57,
    hand_size = 58,
    discard_count = 59,
    deck_count = 60,
    discard_size = 61,
    discard_from_hand = 62,
    shuffle_into_deck = 63,
    place_card_at = 64,
    set_dest = 65,
    set_transfer_to_dest = 66,
    send_to_dest = 67,
    transfer_to_dest = 68,
    rent_unit_mult = 69,
    add_rent_unit_bonus = 70,
    teleport_to = 71,
    unplace_card = 72,
    event_expire = 73,
    event_is_active = 74,
    event_deck_push = 75,
    event_banish = 76,
    self_tile = 77,
    set_self_tile = 78,
    self_face_down = 79,
    set_self_face_down = 80,
    self_immune = 81,
    set_self_immune = 82,
    is_placed = 83,
    self_prop = 84,
    set_self_prop = 85,
    tile_prop = 86,
    set_tile_prop = 87,
    prop_at = 88,
    set_prop_at = 89,
    counter_self = 90,
    add_counter_self = 91,
    set_counter_self = 92,
    counter_at = 93,
    add_counter_at = 94,
    card_counter = 95,
    add_card_counter = 96,
    place_mark = 97,
    count_marks_f = 98,
    bump_mark_f = 99,
    remove_marks_f = 100,
    mark_src_at = 101,
    mark_instance_at = 102,
    count_held = 103,
    add_held = 104,
    count_held_name = 105,
    set_held_name = 106,
    move_units = 107,
    self_uid = 108,
    send = 109,
    msg_sender_uid = 110,
    msg_sender_seat = 111,
    msg_name = 112,
    msg_a = 113,
    msg_b = 114,
    msg_c = 115,
    msg_tile = 116,
    msg_seat = 117,
    msg_text = 118,
    msg_reply = 119,
    state_get = 120,
    state_set = 121,
    state_add = 122,
    // 123/124/125 retired (slot/set_slot/inc_slot sugar; use state_get/state_set/state_add).
    group_pool = 126,
    add_group_pool = 127,
    group_skill = 128,
    persona_skill = 129,
    group_skills = 130,
    add_group_skill = 131,
    invoke_skill = 132,
    raise_bought = 133,
    raise = 134,
    // 135/136 retired (charge / charge_max named readers; use state_get).
    gain_marker = 137,
    /// HostOps bridge only (the guest `apply_abnormal` import is gone -- rules
    /// compose `gate` + `state::*` + `raise_after`). The C export
    /// `ruleset_apply_abnormal` still routes here for native-rules.
    apply_abnormal = 138,
    // 139 retired (second abnormal-kind grant).
    // 140 was the third abnormal-kind grant.
    give_extra_turn = 141,
    can_pay = 142,
    cant_move = 143,
    spend_marker = 144,
    // 145/146/147 retired (status-layer readers; use state_get).
    turn_player = 148,
    round_no = 149,
    turn_key = 150,
    persona_is = 151,
    in_group = 152,
    opt_int = 153,
    opt_str = 154,
    opt_tile = 155,
    opt_price = 156,
    opt_ai = 157,
    ask = 158,
    trig_kind = 159,
    trig_player = 160,
    trig_target = 161,
    trig_tile = 162,
    trig_value = 163,
    trig_name = 164,
    trig_step = 165,
    trig_by_card = 166,
    trig_pay_is_rent = 167,
    trig_move_kind = 168,
    trig_move_resolve = 169,
    trig_move_tag = 170,
    trig_move_main = 171,
    trig_move_dir = 172,
    trig_move_remaining = 173,
    trig_move_from = 174,
    trig_move_total = 175,
    trig_cards = 176,
    trig_move_roll = 177,
    trig_roll_source = 178,
    trig_buy_kind = 179,
    trig_seller = 180,
    trig_price = 181,
    trig_set_price = 182,
    trig_deal_owner = 183,
    trig_set_deal_owner = 184,
    trig_deal_houses = 185,
    trig_set_deal_houses = 186,
    trig_deal_mortgaged = 187,
    trig_set_deal_mortgaged = 188,
    trig_set_reason = 189,
    trig_set_move_roll = 190,
    trig_set_pay_amount = 191,
    trig_set_pay_target = 192,
    trig_set_cancelled = 193,
    trig_set_negate_effect = 194,
    trig_set_spare = 195,
    trig_cancelled = 196,
    trig_seq = 197,
    trig_answers = 198,
    trig_effect_count = 199,
    trig_effect_kind = 200,
    trig_effect_target = 201,
    trig_effect_from = 202,
    trig_effect_tile = 203,
    trig_effect_value = 204,
    declare_effect = 205,
    trig_card_is = 206,
    play_card = 207,
    card_move = 208,
    /// Retired (v98): folded into `query_guard` via `entry::PRESS`.
    /// Never reuse this id.
    card_replayable = 209,
    set_roller = 210,
    set_steps = 211,
    set_reverse = 212,
    set_signed = 213,
    set_stop_at = 214,
    set_parity = 215,
    set_resolve = 216,
    set_settle_tile = 217,
    set_pay_factor = 218,
    set_rent_factor = 219,
    set_no_buy = 220,
    set_no_build = 221,
    set_build_anywhere = 222,
    set_kind = 223,
    set_teleport_to = 224,
    set_start = 225,
    set_base_dice = 226,
    add_base_dice = 227,
    add_extra_dice = 228,
    move_stopped = 229,
    set_min_roll = 230,
    set_extra_steps = 231,
    set_more_steps = 232,
    set_tag = 233,
    set_settle_as_agent = 234,
    plan_add_follower = 235,
    move_stop_at = 236,
    move_parity = 237,
    move_resolve = 238,
    move_steps = 239,
    move_remaining = 240,
    move_total = 241,
    move_dir = 242,
    abnormal_count = 243,
    target = 244,
    targeted_count = 245,
    gains_this_turn = 246,
    placed_tile = 247,
    play_doubled = 248,
    set_play_doubled = 249,
    schedule_turn_end = 250,
    set_no_money_loss = 251,
    set_fixed_roll = 252,
    fixed_roll = 253,
    set_next_steps = 254,
    turn_main_steps = 255,
    // 256 retired (named marker-cap move; use add_marker_max).
    card_build = 257,
    set_can_build = 258,
    clear_dice = 259,
    can_build_on = 260,
    card_buy = 261,
    buy_quotes = 262,
    ai_agent_choice = 263,
    buy = 264,
    acquire = 265,
    agent_offer = 266,
    linger = 267,
    card_immune = 268,
    card_mortgage = 269,
    card_text_mentions = 270,
    gain_fixed = 271,
    // is_agent = 272 (removed v56)
    paid_in_settle = 273,
    place_card_on = 274,
    play_from_hand = 275,
    set_build_discount = 276,
    set_card_immune = 277,
    set_card_tile = 278,
    set_extreme = 279,
    turn_rolls = 280,
    turn_snap = 281,
    turn_start_pos = 282,
    // is_color = 283 (removed v56)
    unplace_card_named = 284,
    tok_names = 285,
    designations = 286,
    cancel_designation = 287,
    designation_cancelled = 288,
    gate = 289,
    card_settle_at = 290,
    card_offer_build = 291,
    do_move_roll = 292,
    // is_live_house_for = 293 (removed v56)
    placed_cards = 294,
    field_instances = 295,
    unplace_at = 296,
    tile_at = 297,
    set_tile_at = 298,
    is_face_down_at = 299,
    set_face_down_at = 300,
    is_immune_at = 301,
    set_immune_at = 302,
    set_build_cost_pct = 303,
    set_card_face_down = 304,
    card_face_down = 305,
    // v55 flash family -- once-per-flash identity lives host-side.
    flash = 306,
    no_flash = 307,
    flash_card = 308,
    flash_card_keyed = 309,
    flash_tile = 310,
    // 真奈 join (CH-05) -- declaration-scoped self-redirect.
    join_redirect_target = 311,
    set_join_used = 312,
    // Raw money adjustment (v57 closeout) -- no payment pipeline.
    adjust_money = 313,
    // Static card property read (v62) -- `CardRules::card_prop` guest face.
    card_prop = 314,
    // v72: write the persona **name** `persona_is` matches (character swap).
    set_persona = 315,
    apply_mortgage = 316,
    apply_redeem = 317,
    set_tag_id = 318,
    recorded_roll = 319,
    pay_buy = 320,
    pay_build = 321,
    assign_deed = 322,
    // v73: paired-mark placement (one unit bound to two tiles).
    place_mark_pair = 323,
    mark_partner = 324,
    trig_settle_redirect = 325,
    // A rent-shaped 「收费」 at milli_scale of the tile's ordinary-settle rent.
    charge_rent = 326,
    end_turn_now = 327,
    teleport_as = 328,
    log_as = 329,
    // v88: generic marker cap + move-latch verbs.
    add_marker_max = 330,
    skip_main_move = 331,
    clear_move_skip = 332,
    // v89: state flag word + post-commit raise with opaque wire name.
    // 333 retired (state_set_flags; use state_set field FLAGS).
    raise_after = 334,
    // v97: multi-select choice -- how many of the pushed options the ask wants.
    opt_count = 335,
    // v98: named-entry applicability + the activation arg buffer.
    query_guard = 336,
    declared_arg = 337,
    // v99: live ceiling on a held-token row (`Counter.max`), `max < 0` leaves
    // the stored one alone. Fire's 「上限M」 reads back through this.
    tok_max = 338,
    // v100: mark refs -- resolve / drop one row by its stable uid.
    mark_tile = 339,
    remove_mark = 340,
}

/// Six i32 wire slots, matching the ABI's arg convention.
pub type Args = [i32; 6];

/// Host backend: one entry point, six i32 slots in, i64 out.
pub type Backend = fn(Op, Args) -> i64;

static mut BACKEND: Option<Backend> = None;

/// Install the symbolic host. Call once at the top of every proof.
pub fn install(f: Backend) {
    unsafe {
        BACKEND = Some(f);
    }
}

/// Dispatch one import to the installed backend.
pub fn call(op: Op, a: Args) -> i64 {
    let f = unsafe { BACKEND };
    let f = f.expect("rules-kani host backend not installed");
    f(op, a)
}

/// `call` truncated to i32 (most imports).
pub fn call_i32(op: Op, a: Args) -> i32 {
    call(op, a) as i32
}

/// `call` as unit.
pub fn call_unit(op: Op, a: Args) {
    let _ = call(op, a);
}
