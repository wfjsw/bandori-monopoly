//! The raw `ruleset` host imports behind [`super`]'s safe wrappers.
//!
//! One `extern` block, one declaration per host function. On wasm32 these are
//! the module's `ruleset` imports; on native targets each `link_name`
//! resolves to the `ruleset_*` shim `rules-native` exports. Card code never
//! calls these directly -- it goes through the typed `ctx::*` wrappers.

// The link attribute is wasm-only; on other targets the imports remain
// declared so the analyzer resolves `ctx::*` in card sources.
// `C-unwind` on native so a host import can abort the run with a panic
// that `rules-native`'s `catch_unwind` turns into "this card is out"
// (the sandbox trap contract). Same calling convention as `C` on wasm32.
// Kani cannot resolve these as foreign imports to host shims (kani#2423),
// and `extern "C-unwind"` monomorphizes `catch_unwind`, which ICEs
// kani-compiler 0.68.0. Under `cfg(kani)` the same names are ordinary Rust
// (see `sys_kani.rs`) dispatching through [`crate::kani_host`].
#[cfg(kani)]
#[path = "sys_kani.rs"]
mod imp;
#[cfg(kani)]
pub use imp::*;

#[cfg(not(kani))]
mod abi {
    #[cfg_attr(
        all(target_arch = "wasm32", not(feature = "native-rules")),
        link(wasm_import_module = "ruleset")
    )]
    extern "C-unwind" {
        // dice & log
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_roll"
        )]
        pub fn roll(player_id: i32, count: i32, sides: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_roll_ask"
        )]
        pub fn roll_ask(player_id: i32, count: i32, sides: i32, source: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_log"
        )]
        pub fn log(player_id: i32, ptr: i32, len: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_effect"
        )]
        pub fn effect(player_id: i32, ptr: i32, len: i32);
        // flash (v55) -- once-per-flash identity lives host-side (drive ordinal /
        // explicit key); the guest just requests.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_flash"
        )]
        pub fn flash();
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_no_flash"
        )]
        pub fn no_flash();
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_flash_card"
        )]
        pub fn flash_card(cp: i32, cl: i32, p: i32, n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_flash_card_keyed"
        )]
        pub fn flash_card_keyed(kp: i32, kl: i32, cp: i32, cl: i32, p: i32, n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_flash_tile"
        )]
        pub fn flash_tile(tile: i32, p: i32, n: i32);
        // board
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_count"
        )]
        pub fn tile_count() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_named"
        )]
        pub fn tile_named(ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_owner"
        )]
        pub fn tile_owner(tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_player_pos"
        )]
        pub fn player_pos(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_steps_ahead"
        )]
        pub fn tile_steps_ahead(player_id: i32, steps: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_rent_of"
        )]
        pub fn rent_of(tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_buy_price"
        )]
        pub fn buy_price(tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_build_cost"
        )]
        pub fn build_cost(tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_mortgage_value"
        )]
        pub fn mortgage_value(tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_owned_count"
        )]
        pub fn owned_count(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_owned_at"
        )]
        pub fn owned_at(player_id: i32, index: i32) -> i32;
        // v56: generic tile queries replace is_buyable/is_shop/is_ring/is_circle/
        // is_live_house/is_agent/is_color/is_live_house_for.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_is"
        )]
        pub fn tile_is(tile: i32, kind_ptr: i32, kind_len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_attr"
        )]
        pub fn tile_attr(tile: i32, attr_ptr: i32, attr_len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_in"
        )]
        pub fn tile_in(player_id: i32, tile: i32, class_ptr: i32, class_len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_group"
        )]
        pub fn tile_group(tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_price"
        )]
        pub fn tile_price(tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_houses_of"
        )]
        pub fn houses_of(tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_rent_houses_of"
        )]
        pub fn rent_houses_of(tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_houses"
        )]
        pub fn set_houses(tile: i32, n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_house"
        )]
        pub fn add_house(tile: i32, n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_mortgaged_of"
        )]
        pub fn mortgaged_of(tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_mortgaged"
        )]
        pub fn set_mortgaged(tile: i32, v: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_owner"
        )]
        pub fn set_owner(tile: i32, player_id: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_dist"
        )]
        pub fn dist(a: i32, b: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_forward"
        )]
        pub fn tile_forward(a: i32, b: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_neighbor"
        )]
        pub fn neighbor(player_id: i32, dir: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_players_on_count"
        )]
        pub fn players_on_count(tile: i32, except: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_players_on_at"
        )]
        pub fn players_on_at(tile: i32, except: i32, index: i32) -> i32;
        // players & money
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_player_count"
        )]
        pub fn player_count() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_player_out"
        )]
        pub fn player_out(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_others_count"
        )]
        pub fn others_count(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_others_at"
        )]
        pub fn others_at(player_id: i32, index: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_money"
        )]
        pub fn money(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_gain"
        )]
        pub fn gain(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32;
        // v50: bank print with the Pay's event `typ` and a replacement log line.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_gain_typed"
        )]
        pub fn gain_typed(player_id: i32, amount: i32, tp: i32, tl: i32, xp: i32, xl: i32) -> i32;
        // Raw money adjustment (no payment pipeline). `delta` > 0 gain, < 0 loss.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_adjust_money"
        )]
        pub fn adjust_money(player_id: i32, delta: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_pay"
        )]
        pub fn pay(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_pay_to"
        )]
        pub fn pay_to(from: i32, to: i32, amount: i32, ptr: i32, len: i32) -> i32;
        // v42 (`PIPELINE-AUDIT` Q2): the command-wide pre-split stage and its
        // per-leg counterpart.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_pay_total"
        )]
        pub fn pay_total(from: i32, to: i32, amount: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_pay_leg"
        )]
        pub fn pay_leg(from: i32, to: i32, amount: i32, ptr: i32, len: i32) -> i32;
        // hand & deck
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_draw"
        )]
        pub fn draw(player_id: i32, n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_draw_event"
        )]
        pub fn draw_event(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_pay_rent"
        )]
        pub fn pay_rent(player_id: i32, tile: i32, half: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_charge_rent"
        )]
        pub fn charge_rent(player_id: i32, tile: i32, milli_scale: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_offer_buy"
        )]
        pub fn offer_buy(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_offer_force_buy"
        )]
        pub fn offer_force_buy(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_offer_build"
        )]
        pub fn offer_build(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_to_hand"
        )]
        pub fn add_to_hand(player_id: i32, ptr: i32, len: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_to_deck"
        )]
        pub fn add_to_deck(player_id: i32, ptr: i32, len: i32, shuffle: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_to_deck_at"
        )]
        pub fn add_to_deck_at(player_id: i32, ptr: i32, len: i32, pos: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_take_card"
        )]
        pub fn take_card(player_id: i32, pile: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_cards_in"
        )]
        pub fn cards_in(player_id: i32, pile: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_to_discard"
        )]
        pub fn to_discard(player_id: i32, ptr: i32, len: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_hand_count"
        )]
        pub fn hand_count(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_hand_size"
        )]
        pub fn hand_size(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_discard_count"
        )]
        pub fn discard_count(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_deck_count"
        )]
        pub fn deck_count(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_discard_size"
        )]
        pub fn discard_size(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_discard_from_hand"
        )]
        pub fn discard_from_hand(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_shuffle_into_deck"
        )]
        pub fn shuffle_into_deck(player_id: i32, hand: i32, discard: i32) -> i32;
        // field cards
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_place_card_at"
        )]
        pub fn place_card_at(player_id: i32, cp: i32, cl: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_dest"
        )]
        pub fn set_dest(dest: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_transfer_to_dest"
        )]
        pub fn set_transfer_to_dest(to: i32, dest: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_send_to_dest"
        )]
        pub fn send_to_dest(dest: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_transfer_to_dest"
        )]
        pub fn transfer_to_dest(to: i32, dest: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_rent_unit_mult"
        )]
        pub fn rent_unit_mult() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_rent_unit_bonus"
        )]
        pub fn add_rent_unit_bonus(n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_teleport_to"
        )]
        pub fn teleport_to(player_id: i32, tile: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_unplace_card"
        )]
        pub fn unplace_card() -> i32;
        // active events (`docs/EVENTS.md`)
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_event_expire"
        )]
        pub fn event_expire(ip: i32, il: i32, removed: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_event_is_active"
        )]
        pub fn event_is_active(ip: i32, il: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_event_deck_push"
        )]
        pub fn event_deck_push(ip: i32, il: i32, face_down: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_event_banish"
        )]
        pub fn event_banish(ip: i32, il: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_self_tile"
        )]
        pub fn self_tile() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_self_tile"
        )]
        pub fn set_self_tile(tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_self_face_down"
        )]
        pub fn self_face_down() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_self_face_down"
        )]
        pub fn set_self_face_down(on: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_self_immune"
        )]
        pub fn self_immune() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_self_immune"
        )]
        pub fn set_self_immune(on: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_is_placed"
        )]
        pub fn is_placed() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_self_prop"
        )]
        pub fn self_prop(kp: i32, kl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_self_prop"
        )]
        pub fn set_self_prop(kp: i32, kl: i32, v: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_prop"
        )]
        pub fn tile_prop(tile: i32, kp: i32, kl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_tile_prop"
        )]
        pub fn set_tile_prop(tile: i32, kp: i32, kl: i32, v: i32) -> i32;
        // v50: one declared property of a field instance (`crystals_at` naming).
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_prop_at"
        )]
        pub fn prop_at(uid: i32, kp: i32, kl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_prop_at"
        )]
        pub fn set_prop_at(uid: i32, kp: i32, kl: i32, v: i32) -> i32;
        // named counters & bound units (v51)
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_counter_self"
        )]
        pub fn counter_self(np: i32, nl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_counter_self"
        )]
        pub fn add_counter_self(np: i32, nl: i32, n: i32, max: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_counter_self"
        )]
        pub fn set_counter_self(np: i32, nl: i32, n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_counter_at"
        )]
        pub fn counter_at(uid: i32, np: i32, nl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_counter_at"
        )]
        pub fn add_counter_at(uid: i32, np: i32, nl: i32, n: i32, max: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_card_counter"
        )]
        pub fn card_counter(player_id: i32, cp: i32, cl: i32, np: i32, nl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_card_counter"
        )]
        pub fn add_card_counter(
            player_id: i32,
            cp: i32,
            cl: i32,
            np: i32,
            nl: i32,
            n: i32,
            max: i32,
        ) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_place_mark"
        )]
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
        ) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_place_mark_pair"
        )]
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
        ) -> i32;

        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_count_marks_f"
        )]
        pub fn count_marks_f(tile: i32, fp: i32, fl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_bump_mark_f"
        )]
        pub fn bump_mark_f(tile: i32, fp: i32, fl: i32, delta: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_remove_marks_f"
        )]
        pub fn remove_marks_f(tile: i32, fp: i32, fl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_mark_src_at"
        )]
        pub fn mark_src_at(tile: i32, fp: i32, fl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_mark_instance_at"
        )]
        pub fn mark_instance_at(tile: i32, fp: i32, fl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_mark_partner"
        )]
        pub fn mark_partner(tile: i32, fp: i32, fl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_count_held"
        )]
        pub fn count_held(np: i32, nl: i32, player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_held"
        )]
        pub fn add_held(np: i32, nl: i32, player_id: i32, n: i32, max: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_count_held_name"
        )]
        pub fn count_held_name(np: i32, nl: i32, player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_held_name"
        )]
        pub fn set_held_name(np: i32, nl: i32, player_id: i32, v: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_move_units"
        )]
        pub fn move_units(
            np: i32,
            nl: i32,
            from_tile: i32,
            from_player: i32,
            to_tile: i32,
            to_player: i32,
            n: i32,
        ) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_self_uid"
        )]
        pub fn self_uid() -> i32;
        // cross-card messages
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_send"
        )]
        pub fn send(tp: i32, tl: i32, np: i32, nl: i32, pp: i32, pl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_msg_sender_uid"
        )]
        pub fn msg_sender_uid() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_msg_sender_seat"
        )]
        pub fn msg_sender_seat() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_msg_name"
        )]
        pub fn msg_name(p: i32, n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_msg_a"
        )]
        pub fn msg_a() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_msg_b"
        )]
        pub fn msg_b() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_msg_c"
        )]
        pub fn msg_c() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_msg_tile"
        )]
        pub fn msg_tile() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_msg_seat"
        )]
        pub fn msg_seat() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_msg_text"
        )]
        pub fn msg_text(p: i32, n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_msg_reply"
        )]
        pub fn msg_reply(v: i32);
        // per-player slots
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_state_get"
        )]
        pub fn state_get(player_id: i32, ptr: i32, len: i32, field: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_state_set"
        )]
        pub fn state_set(player_id: i32, ptr: i32, len: i32, field: i32, v: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_state_add"
        )]
        pub fn state_add(player_id: i32, ptr: i32, len: i32, delta: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_state_set_flags"
        )]
        pub fn state_set_flags(player_id: i32, ptr: i32, len: i32, flags: i32);

        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_slot"
        )]
        pub fn slot(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_slot"
        )]
        pub fn set_slot(player_id: i32, ptr: i32, len: i32, v: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_inc_slot"
        )]
        pub fn inc_slot(player_id: i32, ptr: i32, len: i32, by: i32) -> i32;
        // pots & status
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_group_pool"
        )]
        pub fn group_pool(player_id: i32, np: i32, nl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_group_pool"
        )]
        pub fn add_group_pool(player_id: i32, np: i32, nl: i32, n: i32, max: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_group_skill"
        )]
        pub fn group_skill(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_persona_skill"
        )]
        pub fn persona_skill(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_group_skills"
        )]
        pub fn group_skills(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_group_skill"
        )]
        pub fn add_group_skill(player_id: i32, p: i32, n: i32, extra: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_invoke_skill"
        )]
        pub fn invoke_skill(player_id: i32, p: i32, n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_raise_bought"
        )]
        pub fn raise_bought(player_id: i32, tile: i32) -> i32;
        // v50: guest-raised trigger points (e.g. `circleAffected` from tile:circle).
        // v91: `kind` is the wire spelling (pointer+len).
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_raise"
        )]
        pub fn raise(player_id: i32, kind_p: i32, kind_l: i32, value: i32) -> i32;
        // v88: generic marker family (the name is data). Readers are `state_get`
        // / `state_max` with the marker's wire key.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_gain_marker"
        )]
        pub fn gain_marker(
            player_id: i32,
            name_p: i32,
            name_l: i32,
            n: i32,
            why_p: i32,
            why_l: i32,
        ) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_spend_marker"
        )]
        pub fn spend_marker(
            player_id: i32,
            name_p: i32,
            name_l: i32,
            n: i32,
            why_p: i32,
            why_l: i32,
        ) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_marker_max"
        )]
        pub fn add_marker_max(player_id: i32, name_p: i32, name_l: i32, n: i32) -> i32;
        // v89: post-commit raise with a caller-supplied wire name (opaque string).
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_raise_after"
        )]
        pub fn raise_after(player_id: i32, name_p: i32, name_l: i32, value: i32);
        // v88: this turn's main-move latch (the turn-flow skip; not every layer host uses it).
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_skip_main_move"
        )]
        pub fn skip_main_move(player_id: i32, locked: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_clear_move_skip"
        )]
        pub fn clear_move_skip(player_id: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_give_extra_turn"
        )]
        pub fn give_extra_turn(player_id: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_end_turn_now"
        )]
        pub fn end_turn_now(player_id: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_teleport_as"
        )]
        pub fn teleport_as(player_id: i32, tile: i32, resolve: i32, as_main: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_log_as"
        )]
        pub fn log_as(player_id: i32, kp: i32, kn: i32, p: i32, n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_can_pay"
        )]
        pub fn can_pay(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_cant_move"
        )]
        pub fn cant_move(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_turn_player"
        )]
        pub fn turn_player() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_round_no"
        )]
        pub fn round_no() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_turn_key"
        )]
        pub fn turn_key() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_persona_is"
        )]
        pub fn persona_is(player_id: i32, ptr: i32, len: i32) -> i32;
        // v72: write the persona **name** `persona_is` matches (character swap).
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_persona"
        )]
        pub fn set_persona(player_id: i32, ptr: i32, len: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_in_group"
        )]
        pub fn in_group(player_id: i32, ptr: i32, len: i32) -> i32;
        // prompts & trigger
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_opt_int"
        )]
        pub fn opt_int(v: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_opt_str"
        )]
        pub fn opt_str(ptr: i32, len: i32);
        // v50: tile ask with per-option labels, prices, and an AI choice hint.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_opt_tile"
        )]
        pub fn opt_tile(tile: i32, lp: i32, ll: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_opt_price"
        )]
        pub fn opt_price(price: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_opt_ai"
        )]
        pub fn opt_ai(ai: i32);
        // v97: multi-select choice -- how many of the pushed options the ask wants.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_opt_count"
        )]
        pub fn opt_count(count: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_ask"
        )]
        pub fn ask(
            kind: i32,
            player_id: i32,
            title_ptr: i32,
            title_len: i32,
            text_ptr: i32,
            text_len: i32,
        ) -> i32;
        // v91: two-call out-buffer -- returns the needed length, writes the
        // spelling into `(p, n)`. Same pattern as `trig_name`.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_kind"
        )]
        pub fn trig_kind(p: i32, n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_player"
        )]
        pub fn trig_player() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_target"
        )]
        pub fn trig_target() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_tile"
        )]
        pub fn trig_tile() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_value"
        )]
        pub fn trig_value() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_name"
        )]
        pub fn trig_name(p: i32, n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_step"
        )]
        pub fn trig_step() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_by_card"
        )]
        pub fn trig_by_card() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_pay_is_rent"
        )]
        pub fn trig_pay_is_rent() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_move_kind"
        )]
        pub fn trig_move_kind() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_move_resolve"
        )]
        pub fn trig_move_resolve() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_move_tag"
        )]
        pub fn trig_move_tag(kp: i32, kl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_move_main"
        )]
        pub fn trig_move_main() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_settle_redirect"
        )]
        pub fn trig_settle_redirect() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_move_dir"
        )]
        pub fn trig_move_dir() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_move_remaining"
        )]
        pub fn trig_move_remaining() -> i32;
        // v50: `t.Move.From` -- the move's 移动起点, or -1 when no move is in flight.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_move_from"
        )]
        pub fn trig_move_from() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_move_total"
        )]
        pub fn trig_move_total() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_cards"
        )]
        pub fn trig_cards(buf: i32, cap: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_move_roll"
        )]
        pub fn trig_move_roll() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_roll_source"
        )]
        pub fn trig_roll_source() -> i32;
        // v40 purchase payload
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_buy_kind"
        )]
        pub fn trig_buy_kind() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_seller"
        )]
        pub fn trig_seller() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_price"
        )]
        pub fn trig_price() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_set_price"
        )]
        pub fn trig_set_price(v: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_deal_owner"
        )]
        pub fn trig_deal_owner() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_set_deal_owner"
        )]
        pub fn trig_set_deal_owner(v: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_deal_houses"
        )]
        pub fn trig_deal_houses() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_set_deal_houses"
        )]
        pub fn trig_set_deal_houses(v: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_deal_mortgaged"
        )]
        pub fn trig_deal_mortgaged() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_set_deal_mortgaged"
        )]
        pub fn trig_set_deal_mortgaged(v: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_set_reason"
        )]
        pub fn trig_set_reason(ptr: i32, len: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_set_move_roll"
        )]
        pub fn trig_set_move_roll(v: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_set_pay_amount"
        )]
        pub fn trig_set_pay_amount(v: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_set_pay_target"
        )]
        pub fn trig_set_pay_target(to: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_set_cancelled"
        )]
        pub fn trig_set_cancelled();
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_set_negate_effect"
        )]
        pub fn trig_set_negate_effect();
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_set_spare"
        )]
        pub fn trig_set_spare(seat: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_cancelled"
        )]
        pub fn trig_cancelled() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_seq"
        )]
        pub fn trig_seq() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_answers"
        )]
        pub fn trig_answers() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_effect_count"
        )]
        pub fn trig_effect_count() -> i32;
        // v91: two-call out-buffer like `trig_kind`.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_effect_kind"
        )]
        pub fn trig_effect_kind(i: i32, p: i32, n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_effect_target"
        )]
        pub fn trig_effect_target(i: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_effect_from"
        )]
        pub fn trig_effect_from(i: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_effect_tile"
        )]
        pub fn trig_effect_tile(i: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_effect_value"
        )]
        pub fn trig_effect_value(i: i32) -> i32;
        // v91: `kind` is the wire spelling (pointer+len).
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_declare_effect"
        )]
        pub fn declare_effect(
            kind_p: i32,
            kind_l: i32,
            target: i32,
            from: i32,
            tile: i32,
            value: i32,
        );
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_trig_card_is"
        )]
        pub fn trig_card_is(ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_play_card"
        )]
        pub fn play_card(id_ptr: i32, id_len: i32, player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_card_move"
        )]
        pub fn card_move(player_id: i32) -> i32;
        /// Cross-card applicability query by named entry id
        /// (docs/ACTIVATION.md §3).
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_query_guard"
        )]
        pub fn query_guard(
            player_id: i32,
            ptr: i32,
            len: i32,
            e_ptr: i32,
            e_len: i32,
        ) -> i32;
        /// The running activation's declared arg as postcard bytes
        /// (docs/ACTIVATION.md §2). `out_ptr=0` asks for the size.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_declared_arg"
        )]
        pub fn declared_arg(out_ptr: i32, cap: i32) -> i32;
        // movement shaping: the move being planned
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_roller"
        )]
        pub fn set_roller(player_id: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_steps"
        )]
        pub fn set_steps(n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_reverse"
        )]
        pub fn set_reverse(on: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_signed"
        )]
        pub fn set_signed(on: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_stop_at"
        )]
        pub fn set_stop_at(n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_parity"
        )]
        pub fn set_parity(n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_resolve"
        )]
        pub fn set_resolve(on: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_settle_tile"
        )]
        pub fn set_settle_tile(n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_pay_factor"
        )]
        pub fn set_pay_factor(n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_rent_factor"
        )]
        pub fn set_rent_factor(n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_no_buy"
        )]
        pub fn set_no_buy(on: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_no_build"
        )]
        pub fn set_no_build(on: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_build_anywhere"
        )]
        pub fn set_build_anywhere(on: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_kind"
        )]
        pub fn set_kind(kind: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_teleport_to"
        )]
        pub fn set_teleport_to(tile: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_start"
        )]
        pub fn set_start(tile: i32, ptr: i32, len: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_base_dice"
        )]
        pub fn set_base_dice(count: i32, sides: i32, ptr: i32, len: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_base_dice"
        )]
        pub fn add_base_dice(count: i32, sides: i32, ptr: i32, len: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_add_extra_dice"
        )]
        pub fn add_extra_dice(count: i32, sides: i32, ptr: i32, len: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_move_stopped"
        )]
        pub fn move_stopped() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_min_roll"
        )]
        pub fn set_min_roll(n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_extra_steps"
        )]
        pub fn set_extra_steps(n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_more_steps"
        )]
        pub fn set_more_steps(n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_tag"
        )]
        pub fn set_tag(kp: i32, kl: i32, v: i32);
        /// v78: `set_tag` with the key as an intern id.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_tag_id"
        )]
        pub fn set_tag_id(key_id: i32, v: i32);
        /// v78: host-supplied recorded roll, or `i32::MIN` when none.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_recorded_roll"
        )]
        pub fn recorded_roll() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_settle_as_agent"
        )]
        pub fn set_settle_as_agent(on: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_plan_add_follower"
        )]
        pub fn plan_add_follower(player_id: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_move_stop_at"
        )]
        pub fn move_stop_at() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_move_parity"
        )]
        pub fn move_parity() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_move_resolve"
        )]
        pub fn move_resolve() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_move_steps"
        )]
        pub fn move_steps() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_move_remaining"
        )]
        pub fn move_remaining() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_move_total"
        )]
        pub fn move_total() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_move_dir"
        )]
        pub fn move_dir() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_abnormal_count"
        )]
        pub fn abnormal_count(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_target"
        )]
        pub fn target(player_id: i32, tile: i32, single: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_targeted_count"
        )]
        pub fn targeted_count(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_gains_this_turn"
        )]
        pub fn gains_this_turn(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_placed_tile"
        )]
        pub fn placed_tile(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_play_doubled"
        )]
        pub fn play_doubled() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_play_doubled"
        )]
        pub fn set_play_doubled(n: i32);
        // turn plan & scheduling
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_schedule_turn_end"
        )]
        pub fn schedule_turn_end(player_id: i32, mode: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_no_money_loss"
        )]
        pub fn set_no_money_loss(player_id: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_fixed_roll"
        )]
        pub fn set_fixed_roll(n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_fixed_roll"
        )]
        pub fn fixed_roll() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_next_steps"
        )]
        pub fn set_next_steps(player_id: i32, n: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_turn_main_steps"
        )]
        pub fn turn_main_steps() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_card_build"
        )]
        pub fn card_build(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_can_build"
        )]
        pub fn set_can_build(on: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_clear_dice"
        )]
        pub fn clear_dice();
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_can_build_on"
        )]
        pub fn can_build_on(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_card_buy"
        )]
        pub fn card_buy(player_id: i32, tile: i32) -> i32;
        // v40 purchase surface (`docs/PURCHASE.md`)
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_buy_quotes"
        )]
        pub fn buy_quotes(player_id: i32, kind: i32, buf: i32, n: i32, out: i32) -> i32;
        // v50: which option the player's AI would take among agent-offer tiles.
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_ai_agent_choice"
        )]
        pub fn ai_agent_choice(player_id: i32, buf: i32, n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_buy"
        )]
        pub fn buy(player_id: i32, tile: i32, kind: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_acquire"
        )]
        pub fn acquire(player_id: i32, from: i32, tile: i32, price: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_agent_offer"
        )]
        pub fn agent_offer(player_id: i32, agent: i32, tile: i32, kind: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_linger"
        )]
        pub fn linger(player_id: i32, expires: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_card_immune"
        )]
        pub fn card_immune(player_id: i32, cp: i32, cl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_card_mortgage"
        )]
        pub fn card_mortgage(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_apply_mortgage"
        )]
        pub fn apply_mortgage(player_id: i32, tile: i32, amount: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_apply_redeem"
        )]
        pub fn apply_redeem(player_id: i32, tile: i32, amount: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_pay_buy"
        )]
        pub fn pay_buy(player_id: i32, tile: i32, amount: i32, kind: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_pay_build"
        )]
        pub fn pay_build(player_id: i32, tile: i32, amount: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_assign_deed"
        )]
        pub fn assign_deed(
            player_id: i32,
            tile: i32,
            owner: i32,
            houses: i32,
            mortgaged: i32,
        ) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_card_text_mentions"
        )]
        pub fn card_text_mentions(cp: i32, cl: i32, np: i32, nl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_card_prop"
        )]
        pub fn card_prop(ip: i32, il: i32, kp: i32, kl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_gain_fixed"
        )]
        pub fn gain_fixed(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_paid_in_settle"
        )]
        pub fn paid_in_settle() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_place_card_on"
        )]
        pub fn place_card_on(
            player_id: i32,
            tile: i32,
            cp: i32,
            cl: i32,
            ptr: i32,
            len: i32,
        ) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_play_from_hand"
        )]
        pub fn play_from_hand() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_build_discount"
        )]
        pub fn set_build_discount(n: i32, layers: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_card_immune"
        )]
        pub fn set_card_immune(player_id: i32, cp: i32, cl: i32, on: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_card_tile"
        )]
        pub fn set_card_tile(player_id: i32, cp: i32, cl: i32, tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_extreme"
        )]
        pub fn set_extreme(v: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_turn_rolls"
        )]
        pub fn turn_rolls(buf: i32, cap: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_turn_snap"
        )]
        pub fn turn_snap(player_id: i32, buf: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_turn_start_pos"
        )]
        pub fn turn_start_pos(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_unplace_card_named"
        )]
        pub fn unplace_card_named(player_id: i32, cp: i32, cl: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tok_names"
        )]
        pub fn tok_names(player_id: i32, p: i32, n: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_designations"
        )]
        pub fn designations(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_cancel_designation"
        )]
        pub fn cancel_designation(seat: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_designation_cancelled"
        )]
        pub fn designation_cancelled(seat: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_join_redirect_target"
        )]
        pub fn join_redirect_target() -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_join_used"
        )]
        pub fn set_join_used();
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_gate"
        )]
        pub fn gate(player_id: i32, kind: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_card_settle_at"
        )]
        pub fn card_settle_at(player_id: i32, tile: i32, main: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_card_offer_build"
        )]
        pub fn card_offer_build(player_id: i32, buf: i32, n: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_do_move_roll"
        )]
        pub fn do_move_roll(player_id: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_placed_cards"
        )]
        pub fn placed_cards(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_field_instances"
        )]
        pub fn field_instances(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_unplace_at"
        )]
        pub fn unplace_at(uid: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_tile_at"
        )]
        pub fn tile_at(uid: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_tile_at"
        )]
        pub fn set_tile_at(uid: i32, tile: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_is_face_down_at"
        )]
        pub fn is_face_down_at(uid: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_face_down_at"
        )]
        pub fn set_face_down_at(uid: i32, on: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_is_immune_at"
        )]
        pub fn is_immune_at(uid: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_immune_at"
        )]
        pub fn set_immune_at(uid: i32, on: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_build_cost_pct"
        )]
        pub fn set_build_cost_pct(pct: i32);
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_set_card_face_down"
        )]
        pub fn set_card_face_down(player_id: i32, cp: i32, cl: i32, down: i32) -> i32;
        #[cfg_attr(
            any(not(target_arch = "wasm32"), feature = "native-rules"),
            link_name = "ruleset_card_face_down"
        )]
        pub fn card_face_down(player_id: i32, cp: i32, cl: i32) -> i32;
    }
} // mod abi

#[cfg(not(kani))]
pub use abi::*;
