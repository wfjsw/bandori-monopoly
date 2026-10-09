//! Host functions available to card effects (the Rust port of the C# `H.*` surface).
//!
//! Every piece of text a card shows (log lines, mark notes, prompts, options) is a
//! [`Msg`]: a key in the card crate's `locales/*.json` plus typed arguments. The
//! host never sees display text, and each player reads it in their own language.
//!
//! Ops that would need a player decision do not exist here on purpose: `ask_*`
//! blocks, the host publishes the question, and the run is replayed with the
//! answer -- so a card reads like straight-line code while staying deterministic.

#[cfg(target_arch = "wasm32")]
use alloc::{string::String, vec::Vec};

pub use crate::abi::{AbKind, CardPile};
use crate::abi::{MoveKind, PromptKind, TriggerKind};
use crate::msg::Msg;
use crate::Prompt;

mod sys {
    // The link attribute is wasm-only; on other targets the imports stay
    // declared so the analyzer resolves `ctx::*` in card sources.
    // `C-unwind` on native so a host import can abort the run with a panic
    // that `rules-native`'s `catch_unwind` turns into "this card is out"
    // (the sandbox trap contract). Same calling convention as `C` on wasm32.
    #[cfg_attr(target_arch = "wasm32", link(wasm_import_module = "bandori"))]
    extern "C-unwind" {
        // dice & log
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_roll")]
        pub fn roll(player_id: i32, count: i32, sides: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_roll_ask")]
        pub fn roll_ask(player_id: i32, count: i32, sides: i32, source: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_log")]
        pub fn log(player_id: i32, ptr: i32, len: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_effect")]
        pub fn effect(player_id: i32, ptr: i32, len: i32);
        // board
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_tile_count")]
        pub fn tile_count() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_tile_named")]
        pub fn tile_named(ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_tile_owner")]
        pub fn tile_owner(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_player_pos")]
        pub fn player_pos(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_tile_steps_ahead")]
        pub fn tile_steps_ahead(player_id: i32, steps: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_rent_of")]
        pub fn rent_of(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_buy_price")]
        pub fn buy_price(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_build_cost")]
        pub fn build_cost(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_mortgage_value")]
        pub fn mortgage_value(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_owned_count")]
        pub fn owned_count(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_owned_at")]
        pub fn owned_at(player_id: i32, index: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_is_buyable")]
        pub fn is_buyable(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_is_shop")]
        pub fn is_shop(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_is_ring")]
        pub fn is_ring(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_is_circle")]
        pub fn is_circle(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_is_live_house")]
        pub fn is_live_house(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_tile_group")]
        pub fn tile_group(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_tile_price")]
        pub fn tile_price(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_houses_of")]
        pub fn houses_of(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_rent_houses_of")]
        pub fn rent_houses_of(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_houses")]
        pub fn set_houses(tile: i32, n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_house")]
        pub fn add_house(tile: i32, n: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_mortgaged_of")]
        pub fn mortgaged_of(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_mortgaged")]
        pub fn set_mortgaged(tile: i32, v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_owner")]
        pub fn set_owner(tile: i32, player_id: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_dist")]
        pub fn dist(a: i32, b: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_tile_forward")]
        pub fn tile_forward(a: i32, b: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_neighbor")]
        pub fn neighbor(player_id: i32, dir: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_players_on_count")]
        pub fn players_on_count(tile: i32, except: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_players_on_at")]
        pub fn players_on_at(tile: i32, except: i32, index: i32) -> i32;
        // players & money
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_player_count")]
        pub fn player_count() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_player_out")]
        pub fn player_out(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_others_count")]
        pub fn others_count(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_others_at")]
        pub fn others_at(player_id: i32, index: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_money")]
        pub fn money(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_gain")]
        pub fn gain(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_pay")]
        pub fn pay(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_pay_to")]
        pub fn pay_to(from: i32, to: i32, amount: i32, ptr: i32, len: i32) -> i32;
        // v42 (`PIPELINE-AUDIT` Q2): the command-wide pre-split stage and its
        // per-leg counterpart.
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_pay_total")]
        pub fn pay_total(from: i32, to: i32, amount: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_pay_leg")]
        pub fn pay_leg(from: i32, to: i32, amount: i32, ptr: i32, len: i32) -> i32;
        // hand & deck
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_draw")]
        pub fn draw(player_id: i32, n: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_draw_event")]
        pub fn draw_event(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_pay_rent")]
        pub fn pay_rent(player_id: i32, tile: i32, half: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_offer_buy")]
        pub fn offer_buy(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_offer_force_buy")]
        pub fn offer_force_buy(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_offer_build")]
        pub fn offer_build(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_to_hand")]
        pub fn add_to_hand(player_id: i32, ptr: i32, len: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_to_deck")]
        pub fn add_to_deck(player_id: i32, ptr: i32, len: i32, shuffle: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_to_deck_at")]
        pub fn add_to_deck_at(player_id: i32, ptr: i32, len: i32, pos: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_take_card")]
        pub fn take_card(player_id: i32, pile: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_cards_in")]
        pub fn cards_in(player_id: i32, pile: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_to_discard")]
        pub fn to_discard(player_id: i32, ptr: i32, len: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_hand_count")]
        pub fn hand_count(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_hand_size")]
        pub fn hand_size(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_discard_count")]
        pub fn discard_count(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_deck_count")]
        pub fn deck_count(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_discard_size")]
        pub fn discard_size(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_discard_from_hand")]
        pub fn discard_from_hand(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_shuffle_into_deck")]
        pub fn shuffle_into_deck(player_id: i32, hand: i32, discard: i32) -> i32;
        // field cards
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_place_card_at")]
        pub fn place_card_at(player_id: i32, cp: i32, cl: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_dest")]
        pub fn set_dest(dest: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_transfer_to_dest")]
        pub fn set_transfer_to_dest(to: i32, dest: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_send_to_dest")]
        pub fn send_to_dest(dest: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_transfer_to_dest")]
        pub fn transfer_to_dest(to: i32, dest: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_ring_multiplier")]
        pub fn ring_multiplier() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_ring_bonus")]
        pub fn add_ring_bonus(n: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_teleport_to")]
        pub fn teleport_to(player_id: i32, tile: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_unplace_card")]
        pub fn unplace_card() -> i32;
        // active events (`docs/EVENTS.md`)
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_event_expire")]
        pub fn event_expire(ip: i32, il: i32, removed: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_event_is_active")]
        pub fn event_is_active(ip: i32, il: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_event_deck_push")]
        pub fn event_deck_push(ip: i32, il: i32, face_down: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_event_banish")]
        pub fn event_banish(ip: i32, il: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_self_tile")]
        pub fn self_tile() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_self_tile")]
        pub fn set_self_tile(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_self_face_down")]
        pub fn self_face_down() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_self_face_down")]
        pub fn set_self_face_down(on: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_self_immune")]
        pub fn self_immune() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_self_immune")]
        pub fn set_self_immune(on: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_is_placed")]
        pub fn is_placed() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_crystals")]
        pub fn crystals() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_crystals")]
        pub fn set_crystals(n: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_crystals")]
        pub fn add_crystals(n: i32, max: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_self_prop")]
        pub fn self_prop(kp: i32, kl: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_self_prop")]
        pub fn set_self_prop(kp: i32, kl: i32, v: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_tile_prop")]
        pub fn tile_prop(tile: i32, kp: i32, kl: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_tile_prop")]
        pub fn set_tile_prop(tile: i32, kp: i32, kl: i32, v: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_settle_circle_reward")]
        pub fn settle_circle_reward(player_id: i32, landing: i32) -> i32;
        // marks & tokens
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_mark")]
        pub fn add_mark(tile: i32, player_id: i32, kp: i32, kl: i32, np: i32, nl: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_count_marks")]
        pub fn count_marks(tile: i32, kp: i32, kl: i32, owner: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_remove_marks")]
        pub fn remove_marks(tile: i32, kp: i32, kl: i32, owner: i32) -> i32;
        // [CP点] -- the `mark:cp` owner's tile-mark API plus the on-card count
        // (see `rules/tiles/src/cp.rs` / `docs/TILES.md`)
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_place_cp")]
        pub fn place_cp(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_count_cp")]
        pub fn count_cp(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_count_cp_from")]
        pub fn count_cp_from(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_clear_cp")]
        pub fn clear_cp(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_cp_src_at")]
        pub fn cp_src_at(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_cp_attached")]
        pub fn cp_attached() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_cp")]
        pub fn add_cp(n: i32, max: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_cp_at")]
        pub fn cp_at(uid: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_cp_at")]
        pub fn add_cp_at(uid: i32, n: i32, max: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_tok")]
        pub fn tok(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_tok")]
        pub fn set_tok(player_id: i32, ptr: i32, len: i32, v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_tok")]
        pub fn add_tok(player_id: i32, ptr: i32, len: i32, by: i32, max: i32) -> i32;
        // per-player slots
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_state_get")]
        pub fn state_get(player_id: i32, ptr: i32, len: i32, field: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_state_set")]
        pub fn state_set(player_id: i32, ptr: i32, len: i32, field: i32, v: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_state_add")]
        pub fn state_add(player_id: i32, ptr: i32, len: i32, delta: i32) -> i32;

        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_slot")]
        pub fn slot(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_slot")]
        pub fn set_slot(player_id: i32, ptr: i32, len: i32, v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_inc_slot")]
        pub fn inc_slot(player_id: i32, ptr: i32, len: i32, by: i32) -> i32;
        // pots & status
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_band_crystals")]
        pub fn band_crystals(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_band_crystals")]
        pub fn add_band_crystals(player_id: i32, n: i32, max: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_band_skill")]
        pub fn band_skill(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_character_skill")]
        pub fn character_skill(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_band_skills")]
        pub fn band_skills(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_band_skill")]
        pub fn add_band_skill(player_id: i32, p: i32, n: i32, extra: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_invoke_skill")]
        pub fn invoke_skill(player_id: i32, p: i32, n: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_raise_bought")]
        pub fn raise_bought(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_fire")]
        pub fn fire(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_fire_max")]
        pub fn fire_max(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_gain_fire")]
        pub fn gain_fire(player_id: i32, n: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_give_stay")]
        pub fn give_stay(player_id: i32, n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_give_stun")]
        pub fn give_stun(player_id: i32, n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_give_exile")]
        pub fn give_exile(player_id: i32, n: i32, to: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_give_extra_turn")]
        pub fn give_extra_turn(player_id: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_can_pay")]
        pub fn can_pay(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_cant_move")]
        pub fn cant_move(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_spend_fire")]
        pub fn spend_fire(player_id: i32, n: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_stay_of")]
        pub fn stay_of(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_stun_of")]
        pub fn stun_of(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_turn_player")]
        pub fn turn_player() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_round_no")]
        pub fn round_no() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_turn_key")]
        pub fn turn_key() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_character_is")]
        pub fn character_is(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_in_band")]
        pub fn in_band(player_id: i32, ptr: i32, len: i32) -> i32;
        // prompts & trigger
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_opt_int")]
        pub fn opt_int(v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_opt_str")]
        pub fn opt_str(ptr: i32, len: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_ask")]
        pub fn ask(
            kind: i32,
            player_id: i32,
            title_ptr: i32,
            title_len: i32,
            text_ptr: i32,
            text_len: i32,
        ) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_kind")]
        pub fn trig_kind() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_player")]
        pub fn trig_player() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_target")]
        pub fn trig_target() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_tile")]
        pub fn trig_tile() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_value")]
        pub fn trig_value() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_step")]
        pub fn trig_step() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_by_card")]
        pub fn trig_by_card() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_pay_is_rent")]
        pub fn trig_pay_is_rent() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_move_kind")]
        pub fn trig_move_kind() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_move_resolve")]
        pub fn trig_move_resolve() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_move_tag")]
        pub fn trig_move_tag(kp: i32, kl: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_move_main")]
        pub fn trig_move_main() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_move_dir")]
        pub fn trig_move_dir() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_move_remaining")]
        pub fn trig_move_remaining() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_move_total")]
        pub fn trig_move_total() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_cards")]
        pub fn trig_cards(buf: i32, cap: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_move_roll")]
        pub fn trig_move_roll() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_roll_source")]
        pub fn trig_roll_source() -> i32;
        // v40 purchase payload
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_buy_kind")]
        pub fn trig_buy_kind() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_seller")]
        pub fn trig_seller() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_price")]
        pub fn trig_price() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_set_price")]
        pub fn trig_set_price(v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_deal_owner")]
        pub fn trig_deal_owner() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_set_deal_owner")]
        pub fn trig_set_deal_owner(v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_deal_houses")]
        pub fn trig_deal_houses() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_set_deal_houses")]
        pub fn trig_set_deal_houses(v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_deal_mortgaged")]
        pub fn trig_deal_mortgaged() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_set_deal_mortgaged")]
        pub fn trig_set_deal_mortgaged(v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_set_reason")]
        pub fn trig_set_reason(ptr: i32, len: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_set_move_roll")]
        pub fn trig_set_move_roll(v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_set_pay_amount")]
        pub fn trig_set_pay_amount(v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_set_pay_target")]
        pub fn trig_set_pay_target(to: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_set_cancelled")]
        pub fn trig_set_cancelled();
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_set_negate_effect")]
        pub fn trig_set_negate_effect();
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_set_spare")]
        pub fn trig_set_spare(seat: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_cancelled")]
        pub fn trig_cancelled() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_seq")]
        pub fn trig_seq() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_answers")]
        pub fn trig_answers() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_effect_count")]
        pub fn trig_effect_count() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_effect_kind")]
        pub fn trig_effect_kind(i: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_effect_target")]
        pub fn trig_effect_target(i: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_effect_from")]
        pub fn trig_effect_from(i: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_effect_tile")]
        pub fn trig_effect_tile(i: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_effect_value")]
        pub fn trig_effect_value(i: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_declare_effect")]
        pub fn declare_effect(kind: i32, target: i32, from: i32, tile: i32, value: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_trig_card_is")]
        pub fn trig_card_is(ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_play_card")]
        pub fn play_card(id_ptr: i32, id_len: i32, player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_card_move")]
        pub fn card_move(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_agent_landing")]
        pub fn agent_landing(player_id: i32, agent: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_card_replayable")]
        pub fn card_replayable(player_id: i32, ptr: i32, len: i32) -> i32;
        // movement shaping: the move being planned (C# `TurnCtx.Plan`)
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_roller")]
        pub fn set_roller(player_id: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_steps")]
        pub fn set_steps(n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_reverse")]
        pub fn set_reverse(on: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_signed")]
        pub fn set_signed(on: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_stop_at")]
        pub fn set_stop_at(n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_parity")]
        pub fn set_parity(n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_resolve")]
        pub fn set_resolve(on: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_settle_tile")]
        pub fn set_settle_tile(n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_pay_factor")]
        pub fn set_pay_factor(n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_rent_factor")]
        pub fn set_rent_factor(n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_no_buy")]
        pub fn set_no_buy(on: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_no_build")]
        pub fn set_no_build(on: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_build_anywhere")]
        pub fn set_build_anywhere(on: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_kind")]
        pub fn set_kind(kind: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_teleport_to")]
        pub fn set_teleport_to(tile: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_start")]
        pub fn set_start(tile: i32, ptr: i32, len: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_base_dice")]
        pub fn set_base_dice(count: i32, sides: i32, ptr: i32, len: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_base_dice")]
        pub fn add_base_dice(count: i32, sides: i32, ptr: i32, len: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_extra_dice")]
        pub fn add_extra_dice(count: i32, sides: i32, ptr: i32, len: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_move_stopped")]
        pub fn move_stopped() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_min_roll")]
        pub fn set_min_roll(n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_extra_steps")]
        pub fn set_extra_steps(n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_more_steps")]
        pub fn set_more_steps(n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_tag")]
        pub fn set_tag(kp: i32, kl: i32, v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_settle_as_agent")]
        pub fn set_settle_as_agent(on: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_plan_add_follower")]
        pub fn plan_add_follower(player_id: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_bonus")]
        pub fn set_bonus(n: i32, ptr: i32, len: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_move_stop_at")]
        pub fn move_stop_at() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_move_parity")]
        pub fn move_parity() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_move_resolve")]
        pub fn move_resolve() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_move_steps")]
        pub fn move_steps() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_move_remaining")]
        pub fn move_remaining() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_move_total")]
        pub fn move_total() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_move_dir")]
        pub fn move_dir() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_abnormal_count")]
        pub fn abnormal_count(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_target")]
        pub fn target(player_id: i32, tile: i32, single: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_targeted_count")]
        pub fn targeted_count(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_gains_this_turn")]
        pub fn gains_this_turn(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_placed_tile")]
        pub fn placed_tile(player_id: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_play_doubled")]
        pub fn play_doubled() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_play_doubled")]
        pub fn set_play_doubled(n: i32);
        // turn plan & scheduling
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_schedule_turn_end")]
        pub fn schedule_turn_end(player_id: i32, mode: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_no_money_loss")]
        pub fn set_no_money_loss(player_id: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_fixed_roll")]
        pub fn set_fixed_roll(n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_fixed_roll")]
        pub fn fixed_roll() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_next_steps")]
        pub fn set_next_steps(player_id: i32, n: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_turn_main_steps")]
        pub fn turn_main_steps() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_fire_max")]
        pub fn add_fire_max(player_id: i32, n: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_card_crystals")]
        pub fn card_crystals(player_id: i32, cp: i32, cl: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_card_build")]
        pub fn card_build(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_can_build")]
        pub fn set_can_build(on: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_clear_dice")]
        pub fn clear_dice();
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_can_build_on")]
        pub fn can_build_on(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_card_buy")]
        pub fn card_buy(player_id: i32, tile: i32) -> i32;
        // v40 purchase surface (`docs/PURCHASE.md`)
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_buy_quotes")]
        pub fn buy_quotes(player_id: i32, kind: i32, buf: i32, n: i32, out: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_buy")]
        pub fn buy(player_id: i32, tile: i32, kind: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_acquire")]
        pub fn acquire(player_id: i32, from: i32, tile: i32, price: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_agent_offer")]
        pub fn agent_offer(player_id: i32, agent: i32, tile: i32, kind: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_linger")]
        pub fn linger(player_id: i32, expires: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_card_immune")]
        pub fn card_immune(player_id: i32, cp: i32, cl: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_card_mortgage")]
        pub fn card_mortgage(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_card_text_mentions")]
        pub fn card_text_mentions(cp: i32, cl: i32, np: i32, nl: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_gain_fixed")]
        pub fn gain_fixed(player_id: i32, amount: i32, ptr: i32, len: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_is_agent")]
        pub fn is_agent(tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_paid_in_settle")]
        pub fn paid_in_settle() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_place_card_on")]
        pub fn place_card_on(
            player_id: i32,
            tile: i32,
            cp: i32,
            cl: i32,
            ptr: i32,
            len: i32,
        ) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_play_from_hand")]
        pub fn play_from_hand() -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_build_discount")]
        pub fn set_build_discount(n: i32, layers: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_card_immune")]
        pub fn set_card_immune(player_id: i32, cp: i32, cl: i32, on: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_card_tile")]
        pub fn set_card_tile(player_id: i32, cp: i32, cl: i32, tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_extreme")]
        pub fn set_extreme(v: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_turn_rolls")]
        pub fn turn_rolls(buf: i32, cap: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_turn_snap")]
        pub fn turn_snap(player_id: i32, buf: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_turn_start_pos")]
        pub fn turn_start_pos(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_is_color")]
        pub fn is_color(player_id: i32, tile: i32, group: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_unplace_card_named")]
        pub fn unplace_card_named(player_id: i32, cp: i32, cl: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_bump_mark")]
        pub fn bump_mark(tile: i32, kp: i32, kl: i32, owner: i32, delta: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_tok_names")]
        pub fn tok_names(player_id: i32, p: i32, n: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_designations")]
        pub fn designations(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_cancel_designation")]
        pub fn cancel_designation(seat: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_designation_cancelled")]
        pub fn designation_cancelled(seat: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_gate")]
        pub fn gate(player_id: i32, kind: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_card_settle_at")]
        pub fn card_settle_at(player_id: i32, tile: i32, main: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_card_offer_build")]
        pub fn card_offer_build(player_id: i32, buf: i32, n: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_do_move_roll")]
        pub fn do_move_roll(player_id: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_is_live_house_for")]
        pub fn is_live_house_for(player_id: i32, tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_placed_cards")]
        pub fn placed_cards(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_field_instances")]
        pub fn field_instances(player_id: i32, buf: i32, cap: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_crystals_at")]
        pub fn crystals_at(uid: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_crystals_at")]
        pub fn add_crystals_at(uid: i32, n: i32, max: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_unplace_at")]
        pub fn unplace_at(uid: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_tile_at")]
        pub fn tile_at(uid: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_tile_at")]
        pub fn set_tile_at(uid: i32, tile: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_is_face_down_at")]
        pub fn is_face_down_at(uid: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_face_down_at")]
        pub fn set_face_down_at(uid: i32, on: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_is_immune_at")]
        pub fn is_immune_at(uid: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_immune_at")]
        pub fn set_immune_at(uid: i32, on: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_build_cost_pct")]
        pub fn set_build_cost_pct(pct: i32);
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_set_card_face_down")]
        pub fn set_card_face_down(player_id: i32, cp: i32, cl: i32, down: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_add_card_crystals")]
        pub fn add_card_crystals(player_id: i32, cp: i32, cl: i32, n: i32, max: i32) -> i32;
        #[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_card_face_down")]
        pub fn card_face_down(player_id: i32, cp: i32, cl: i32) -> i32;
    }
}

#[cfg(target_arch = "wasm32")]
fn s(v: &str) -> (i32, i32) {
    (v.as_ptr() as i32, v.len() as i32)
}

#[cfg(not(target_arch = "wasm32"))]
fn s(v: &str) -> (i32, i32) {
    crate::native::intern(v.as_bytes())
}

/// The i32 wire form of a message: `postcard` bytes leaked for the host to read
/// (same lifetime convention as every other buffer crossing the ABI).
#[cfg(target_arch = "wasm32")]
fn mj(m: &Msg) -> (i32, i32) {
    let b = m.to_bytes();
    let p = b.as_ptr() as i32;
    let l = b.len() as i32;
    core::mem::forget(b);
    (p, l)
}

#[cfg(not(target_arch = "wasm32"))]
fn mj(m: &Msg) -> (i32, i32) {
    crate::native::intern(&m.to_bytes())
}

// ------------------------------------------------------------- dice & log

/// `H.Roll(seat, count, sides, what)` -- sum of `count` d`sides`, logged as a dice event.
///
/// **No [反击] window.** A roll that 「掷骰结算前」 [反击]s must answer (Y.O.L.O
/// 「你的任意掷骰结算前」, 寄于指尖的执念 「当你使用火罐进行掷骰时」) goes through
/// [`roll_ask`] instead, which raises the `Roll` chain link.
pub fn roll(player_id: i32, count: i32, sides: i32) -> i32 {
    unsafe { sys::roll(player_id, count, sides) }
}

/// [`roll`], but the engine raises the `Roll` chain link on the face first -- the
/// 「掷骰结算前」 [反击] window -- with the roller, the face and `source` (a
/// [`crate::abi::roll_source`] code). Returns the face a counteraction left.
///
/// `sides == 0` is the `do_move_roll` shape: the face is the sum of the move
/// plan's dice tables (「使用火罐进行掷骰」 rerolls) rather than `count`d`sides`.
/// Pauses like [`card_move`] -- the trap unwinds the body and the replay reads
/// the answer.
pub fn roll_ask(player_id: i32, count: i32, sides: i32, source: i32) -> i32 {
    unsafe { sys::roll_ask(player_id, count, sides, source) }
}

/// [`roll_ask`] for a move-plan reroll (`sides == 0`).
pub fn do_move_roll_ask(player_id: i32, source: i32) -> i32 {
    unsafe { sys::roll_ask(player_id, 0, 0, source) }
}

/// `H.Log("text", seat, text)`.
pub fn log(player_id: i32, msg: &Msg) {
    let (p, l) = mj(msg);
    unsafe { sys::log(player_id, p, l) }
}

/// `H.Effect` -- announce which effect was applied. Reaches the player as a
/// popup as well as a log line; use this where the card has just picked a
/// branch (e.g. a 1d10 that names the branch taken) and the player should see
/// which. Additive with [`log`], not a replacement for it.
pub fn effect(player_id: i32, msg: &Msg) {
    let (p, l) = mj(msg);
    unsafe { sys::effect(player_id, p, l) }
}

// ------------------------------------------------------------------ board

pub fn tile_count() -> i32 {
    unsafe { sys::tile_count() }
}

/// Tile index of a name (a data key), or -1 (`H.TileNamed`).
pub fn tile_named(name: &str) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::tile_named(p, l) }
}

pub fn tile_owner(tile: i32) -> i32 {
    unsafe { sys::tile_owner(tile) }
}

/// The player's tile on the ring (`H.State.seats[seat].pos`).
pub fn player_pos(player_id: i32) -> i32 {
    unsafe { sys::player_pos(player_id) }
}

/// Tile `steps` ahead of a player around the ring.
pub fn tile_steps_ahead(player_id: i32, steps: i32) -> i32 {
    unsafe { sys::tile_steps_ahead(player_id, steps) }
}

pub fn rent_of(tile: i32) -> i32 {
    unsafe { sys::rent_of(tile) }
}

pub fn buy_price(tile: i32) -> i32 {
    unsafe { sys::buy_price(tile) }
}

pub fn build_cost(tile: i32) -> i32 {
    unsafe { sys::build_cost(tile) }
}

pub fn mortgage_value(tile: i32) -> i32 {
    unsafe { sys::mortgage_value(tile) }
}

pub fn owned_count(player_id: i32) -> i32 {
    unsafe { sys::owned_count(player_id) }
}

pub fn owned_at(player_id: i32, index: i32) -> i32 {
    unsafe { sys::owned_at(player_id, index) }
}

/// The player's deeds as tile indices (`H.OwnedBy`-style walks).
pub fn owned_tiles(player_id: i32) -> Vec<i32> {
    (0..owned_count(player_id))
        .map(|i| owned_at(player_id, i))
        .collect()
}

/// `TileData.IsBuyable` -- a deed tile (property / RiNG).
pub fn is_buyable(tile: i32) -> bool {
    unsafe { sys::is_buyable(tile) != 0 }
}

/// C# `H.IsShop` -- a 商店街 deed (buyable and colour group 10).
pub fn is_shop(tile: i32) -> bool {
    unsafe { sys::is_shop(tile) != 0 }
}

/// `TileData.kind == "ring"` -- a RiNG deed (never holds houses).
pub fn is_ring(tile: i32) -> bool {
    unsafe { sys::is_ring(tile) != 0 }
}

/// `TileData.kind == "circle"` -- the CiRCLE tile.
pub fn is_circle(tile: i32) -> bool {
    unsafe { sys::is_circle(tile) != 0 }
}

/// C# `H.IsLiveHouse` -- a Live House deed (buyable, colour group 6). The
/// `ExtraColor` band-skill colours are not visible here (TODO in the cards).
pub fn is_live_house(tile: i32) -> bool {
    unsafe { sys::is_live_house(tile) != 0 }
}

/// `TileData.group` -- the colour group (-1 for no tile).
pub fn tile_group(tile: i32) -> i32 {
    unsafe { sys::tile_group(tile) }
}

/// `H._tiles[t].price` -- the land price alone (houses are extra; cf. [`buy_price`]).
pub fn tile_price(tile: i32) -> i32 {
    unsafe { sys::tile_price(tile) }
}

/// `H.State.houses[t]` -- houses standing on the tile.
pub fn houses_of(tile: i32) -> i32 {
    unsafe { sys::houses_of(tile) }
}

/// The house count a **rent** lookup reads (`H.RentHouses`) -- the counted
/// value, which a 「房屋数视为…」 override may lift above [`houses_of`].
/// Real houses are untouched; build caps, raze and sale still see
/// [`houses_of`]. 「X为你收费格上的房屋数」 reads this.
pub fn rent_houses_of(tile: i32) -> i32 {
    unsafe { sys::rent_houses_of(tile) }
}

/// Set the tile's house count directly (house transfer effects).
pub fn set_houses(tile: i32, n: i32) {
    unsafe { sys::set_houses(tile, n) }
}

/// `H.AddHouse` -- move the house count by `n`; returns the new count.
pub fn add_house(tile: i32, n: i32) -> i32 {
    unsafe { sys::add_house(tile, n) }
}

/// Is the deed mortgaged? (C# `H.State.mortgaged[t]`.)
pub fn mortgaged_of(tile: i32) -> bool {
    unsafe { sys::mortgaged_of(tile) != 0 }
}

/// Mortgage or redeem a deed outright (transfer effects; no money moves).
pub fn set_mortgaged(tile: i32, v: bool) {
    unsafe { sys::set_mortgaged(tile, v as i32) }
}

/// Hand a deed to another player outright (C# `H.State.owners[t] = seat`; deed
/// transfer effects). The caller logs the transfer.
pub fn set_owner(tile: i32, player_id: i32) {
    unsafe { sys::set_owner(tile, player_id) }
}

/// C# `H.Dist` -- undirected ring distance between two tiles.
pub fn dist(a: i32, b: i32) -> i32 {
    unsafe { sys::dist(a, b) }
}

/// C# `H.Forward` -- steps forward from `a` to `b` around the ring.
pub fn tile_forward(a: i32, b: i32) -> i32 {
    unsafe { sys::tile_forward(a, b) }
}

/// C# `H.Neighbor(seat, dir)` -- the next present player in turn order (`dir` ±1), or -1.
pub fn neighbor(player_id: i32, dir: i32) -> i32 {
    unsafe { sys::neighbor(player_id, dir) }
}

/// C# `H.SeatsOn(tile, except)` -- present players standing on a tile.
pub fn players_on(tile: i32, except: i32) -> Vec<i32> {
    (0..players_on_count(tile, except))
        .map(|i| players_on_at(tile, except, i))
        .collect()
}

fn players_on_count(tile: i32, except: i32) -> i32 {
    unsafe { sys::players_on_count(tile, except) }
}

fn players_on_at(tile: i32, except: i32, index: i32) -> i32 {
    unsafe { sys::players_on_at(tile, except, index) }
}

// ------------------------------------------------------------------ players

pub fn player_count() -> i32 {
    unsafe { sys::player_count() }
}

/// 1 when the player is out of the game (`H.Out`).
pub fn player_out(player_id: i32) -> bool {
    unsafe { sys::player_out(player_id) != 0 }
}

pub fn others_count(player_id: i32) -> i32 {
    unsafe { sys::others_count(player_id) }
}

pub fn others_at(player_id: i32, index: i32) -> i32 {
    unsafe { sys::others_at(player_id, index) }
}

/// The other players still in the game (`H.Others`).
pub fn others(player_id: i32) -> Vec<i32> {
    (0..others_count(player_id))
        .map(|i| others_at(player_id, i))
        .collect()
}

/// The player's current cash (C# `H.Money`).
pub fn money_of(player_id: i32) -> i32 {
    unsafe { sys::money(player_id) }
}

/// `H.GainR` -- money in, logged with its reason (`src` is a message key).
/// Runs the same `Money` pipeline as [`pay`] (print, game -> player), so
/// `payAdd` / `payChoose` and the `effect` [反击] window see it; the answer is
/// the amount that actually moved (0 = cancelled).
pub fn gain(player_id: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    let (p, l) = mj(src);
    asked(unsafe { sys::gain(player_id, amount, p, l) })
}

/// `H.PayR` -- money out (what the player could pay), logged.
pub fn pay(player_id: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    let (p, l) = mj(src);
    asked(unsafe { sys::pay(player_id, amount, p, l) })
}

/// Player-to-player money -- one pipeline entry (`pay_to`), so the `effect`
/// declaration names both the payer and the payee and 「向其他玩家支付」 sees it.
/// The receiver gets exactly what the payer could pay.
pub fn transfer(from: i32, to: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    let (p, l) = mj(src);
    asked(unsafe { sys::pay_to(from, to, amount, p, l) })
}

// ------------------------------------------------------------- 「[分摊]」 / 分摊前

/// The command-wide **pre-split** stage (`PIPELINE-AUDIT` Q2): `payTotalAdd` →
/// `payTotalMul` → `payTotalCancel` shape `total` **before** any 「[分摊]」
/// divides it into shares. This is the 「分摊前」 figure of
/// 「此次支付的分摊前资金减少Y×100」 and kin (规则书 支付阶段 2/4/5 applied to
/// the command rather than to one pair).
///
/// Returns the shaped total, or `None` when a `payTotalCancel` hook dropped the
/// whole command. A single-pair payment runs the same three stages inside
/// [`transfer`] / [`pay`] / [`gain`] -- call this only when *you* are about to
/// split, and follow it with [`pay_leg`] per share (see [`split_pay`]).
pub fn pay_total(from: i32, to: i32, total: i32, src: &Msg) -> Result<Option<i32>, Prompt> {
    let (p, l) = mj(src);
    let shaped = asked(unsafe { sys::pay_total(from, to, total, p, l) })?;
    Ok((shaped >= 0).then_some(shaped))
}

/// One 「[分摊]」 leg: the same payment pipeline as [`transfer`] minus the
/// command-wide pre-split stage (the body already ran [`pay_total`]).
/// `from < 0` / `to < 0` name the bank side.
pub fn pay_leg(from: i32, to: i32, amount: i32, src: &Msg) -> Result<i32, Prompt> {
    let (p, l) = mj(src);
    asked(unsafe { sys::pay_leg(from, to, amount, p, l) })
}

/// `H.SplitPay` -- 「[分摊][支付]」: shape `total` with the command-wide
/// pre-split stage, then charge each of `payers` `ceil10(ceil(total/n))` to
/// `to` (the 规则书 「向上取整10」 share). Returns the per-leg amounts, or an
/// empty list when the command was cancelled.
///
/// The legs run [`pay_leg`], so a `payTotalAdd` hook hits the **total** once
/// rather than each share (`PIPELINE-AUDIT` Q2 / `rb_money::
/// pre_split_modifier_shapes_the_total_not_each_leg`).
pub fn split_pay(payers: &[i32], to: i32, total: i32, src: &Msg) -> Result<Vec<i32>, Prompt> {
    let list: Vec<i32> = payers
        .iter()
        .copied()
        .filter(|&p| p != to && !player_out(p))
        .collect();
    if list.is_empty() || total <= 0 {
        return Ok(Vec::new());
    }
    let Some(shaped) = pay_total(list[0], to, total, src)? else {
        return Ok(Vec::new());
    };
    let n = list.len() as i32;
    let per = (shaped + n - 1) / n;
    let share = (per + 9) / 10 * 10;
    let mut moved = Vec::with_capacity(list.len());
    for p in list {
        moved.push(pay_leg(p, to, share, src)?);
    }
    Ok(moved)
}

// ------------------------------------------------------------- hand / deck

/// `H.DrawR` -- draw `n` cards; returns how many were drawn.
///
/// One **before-draw** point per single card (an N-card draw is N iterations):
/// each card is adjudicated host-side (`drewBefore` may replace it), and the
/// plain ones are moved here on the run's own world copy. The after points
/// (`drawn` / `drew`) fire per card once the run commits. Pauses like [`pay`]
/// does; the body must `?` the result.
pub fn draw(player_id: i32, n: i32) -> Result<i32, Prompt> {
    let mut got = 0;
    for _ in 0..n.max(0) {
        got += asked(unsafe { sys::draw(player_id, 1) })?;
    }
    Ok(got)
}

/// `H.DrawEvent` -- draw the top event and resolve it (「抽取一个事件卡」).
/// The card does not enter the hand: it is revealed to every player, its
/// effect takes effect at once, and it goes to the event discard (reshuffling
/// that into a new event deck when the deck runs dry). Rulebook 「基础[结算]规则」.
/// Pauses like [`draw`]; the body must `?` the result.
pub fn draw_event(player_id: i32) -> Result<(), Prompt> {
    asked(unsafe { sys::draw_event(player_id) })?;
    Ok(())
}

/// `H.PayRent` -- 「[支付]拥有格子的玩家格子地契所标记的现等级地租」. The engine's
/// rent pipeline: the rent table at the tile's current level, or the RiNG
/// dice-rent (「地主拥有的 RiNG 数量 × ringMultiplier × 1d20」, TODO(规则书)),
/// and with `half` the agent's 「半价收费（向上取整10）」 cut. Raises `pay`.
/// Pauses; the body must `?` the result. `docs/TILES.md`.
pub fn pay_rent(player_id: i32, tile: i32, half: bool) -> Result<(), Prompt> {
    asked(unsafe { sys::pay_rent(player_id, tile, half as i32) })?;
    Ok(())
}

/// `H.OfferBuy` -- 「可选择[消耗]购买格子地契和建造已有房子的资金总价，获得格子
/// 地契和拥有权」 on a non-main landing on unowned land. Pauses; `?` it.
pub fn offer_buy(player_id: i32, tile: i32) -> Result<(), Prompt> {
    asked(unsafe { sys::offer_buy(player_id, tile) })?;
    Ok(())
}

/// `H.OfferForceBuy` -- 「可选择[支付]…资金总价的两倍，从该玩家处强行购买该格
/// 地契，获得的地契仍为抵押状态」 on a mortgaged deed. 「此次购买的价格不受任何
/// 资金变动效果影响」 is the engine's (it moves money directly). Pauses; `?` it.
pub fn offer_force_buy(player_id: i32, tile: i32) -> Result<(), Prompt> {
    asked(unsafe { sys::offer_force_buy(player_id, tile) })?;
    Ok(())
}

/// `H.OfferBuild` -- 「可选择[消耗]格子地契所标注的房屋建筑费进行升级建造」 on
/// one's own land. Pauses; `?` it.
pub fn offer_build(player_id: i32, tile: i32) -> Result<(), Prompt> {
    asked(unsafe { sys::offer_build(player_id, tile) })?;
    Ok(())
}

pub fn add_to_hand(player_id: i32, card: &str) {
    let (p, l) = s(card);
    unsafe { sys::add_to_hand(player_id, p, l) }
}

pub fn add_to_deck(player_id: i32, card: &str, shuffle: bool) {
    let (p, l) = s(card);
    unsafe { sys::add_to_deck(player_id, p, l, shuffle as i32) }
}

/// Where `add_to_deck_at` inserts into the draw pile (C# `H.AddToDeck`'s
/// `where` argument -- note the C# default is [`DeckPos::Random`]).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DeckPos {
    /// `"top"` -- the next `draw` gets it.
    Top = 0,
    /// `"bottom"` -- the pile is emptied first.
    Bottom = 1,
    /// `"shuffle"` -- mixed into the pile.
    Random = 2,
}

/// C# `H.AddToDeck(seat, card, where)` -- put one specific card into the draw
/// pile at a position. (`add_to_deck(player_id, card, shuffle)` is the older
/// top/shuffle shape and stays.)
pub fn add_to_deck_at(player_id: i32, card: &str, pos: DeckPos) {
    let (p, l) = s(card);
    unsafe { sys::add_to_deck_at(player_id, p, l, pos as i32) }
}

/// Take one copy of `card` out of `pile` *without* sending it anywhere (C#
/// `_hidden[s].hand.Remove(card)` / `.discard.Remove(card)`) -- the caller
/// decides where it goes next. Returns whether it was there.
pub fn take_card(player_id: i32, pile: CardPile, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::take_card(player_id, pile as i32, p, l) != 0 }
}

/// [`take_card`] from the hand -- for moving a card straight to the field or
/// the deck without discarding it.
pub fn take_from_hand(player_id: i32, card: &str) -> bool {
    take_card(player_id, CardPile::Hand, card)
}

/// Every card id in a player's `pile`, in pile order (the deck's top card
/// first). Duplicates are listed once per copy.
pub fn cards_in(player_id: i32, pile: CardPile) -> Vec<String> {
    let need = unsafe { sys::cards_in(player_id, pile as i32, 0, 0) };
    if need <= 0 {
        return Vec::new();
    }
    let mut buf: Vec<u8> = Vec::new();
    buf.resize(need as usize, 0);
    let got = unsafe { sys::cards_in(player_id, pile as i32, buf.as_mut_ptr() as i32, need) };
    if got != need {
        return Vec::new();
    }
    postcard::from_bytes(&buf).unwrap_or_default()
}

pub fn to_discard(player_id: i32, card: &str) {
    let (p, l) = s(card);
    unsafe { sys::to_discard(player_id, p, l) }
}

/// Copies of `card` in the player's hand (C# `_hidden[s].hand` count).
pub fn hand_count(player_id: i32, card: &str) -> i32 {
    let (p, l) = s(card);
    unsafe { sys::hand_count(player_id, p, l) }
}

/// Total cards in hand (C# `_hidden[s].hand.Count`).
pub fn hand_size(player_id: i32) -> i32 {
    unsafe { sys::hand_size(player_id) }
}

/// The authoritative opening hand size (`state_key::START_HAND`; the engine
/// seeds it to 2 before the before-match-start point). Effects lower it with
/// [`inc_start_hand`]; the opening draw reads the final value.
pub fn start_hand(player_id: i32) -> i32 {
    state::get(player_id, crate::abi::state_key::START_HAND)
}

/// 「初始手牌减1」 and kin: move the opening hand size by `delta`, floored at 0.
/// Live only at the before-match-start point (`DeckBeforeGame`) -- the opening
/// draw reads the value once, afterwards.
pub fn inc_start_hand(player_id: i32, delta: i32) {
    let v = (state::get(player_id, crate::abi::state_key::START_HAND) + delta).max(0);
    state::set(player_id, crate::abi::state_key::START_HAND, v);
}

/// Copies of `card` in the player's discard pile.
pub fn discard_count(player_id: i32, card: &str) -> i32 {
    let (p, l) = s(card);
    unsafe { sys::discard_count(player_id, p, l) }
}

/// Draw-pile size (C# `_hidden[s].draw.Count`).
pub fn deck_count(player_id: i32) -> i32 {
    unsafe { sys::deck_count(player_id) }
}

/// Discard-pile size (all ids).
pub fn discard_size(player_id: i32) -> i32 {
    unsafe { sys::discard_size(player_id) }
}

/// C# `H.DiscardFromHand` -- drop one copy of `card` from hand into the discard
/// pile. True when the player held one.
pub fn discard_from_hand(player_id: i32, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::discard_from_hand(player_id, p, l) != 0 }
}

/// C# `H.ShuffleAllIntoDeck(seat, hand: true, discard: true)` -- sweep hand +
/// discard into the draw pile and shuffle. Returns how many cards moved.
pub fn sweep_to_deck(player_id: i32) -> i32 {
    shuffle_into_deck(player_id, true, true)
}

/// C# `H.ShuffleAllIntoDeck(seat, hand, discard)` -- move the hand and/or the
/// discard pile into the draw pile, then shuffle it. Returns how many moved.
pub fn shuffle_into_deck(player_id: i32, hand: bool, discard: bool) -> i32 {
    unsafe { sys::shuffle_into_deck(player_id, hand as i32, discard as i32) }
}

// ------------------------------------------------------------ field cards

/// `H.PlaceFromPlay` -- this card stays in play at the player.
pub fn place_card(player_id: i32, card: &str, note: &Msg) -> i32 {
    place_card_at(player_id, card, note)
}

/// `H.PlaceCard` -- put a specific card (a derived one) into play at a player.
/// Returns the new instance's uid, which is what addresses it afterwards: the
/// *name* is not an identity, so a rule meaning "the copy I just placed" has to
/// hold this rather than re-resolving by name.
pub fn place_card_at(player_id: i32, card: &str, note: &Msg) -> i32 {
    let (cp, cl) = s(card);
    let (p, l) = mj(note);
    unsafe { sys::place_card_at(player_id, cp, cl, p, l) }
}

/// `PlayCtx.Dest` -- where this card goes afterwards (see [`Dest`]). For a
/// play it is the hand card's fate; for a field effect, the instance's.
pub fn set_dest(dest: Dest) {
    unsafe { sys::set_dest(dest as i32) }
}

/// [`set_dest`] aimed at another player's pile -- 「将此卡放入[使用者]弃卡区」
/// when [使用者] is not the one holding the card. `to` is whose discard / hand
/// / deck it lands in.
pub fn set_transfer_to_dest(to: i32, dest: Dest) {
    unsafe { sys::set_transfer_to_dest(to, dest as i32) }
}

/// [`set_dest`] applied **now** rather than when this effect finishes: for a
/// card that must be gone before the rest of the effect runs (a move or a
/// settle follows). Lands in its own owner's pile. Returns the owner it left,
/// or `None` when it was not in play.
pub fn send_to_dest(dest: Dest) -> Option<i32> {
    let v = unsafe { sys::send_to_dest(dest as i32) };
    (v >= 0).then_some(v)
}

/// [`send_to_dest`] aimed at another player's pile: `to` is whose.
pub fn transfer_to_dest(to: i32, dest: Dest) -> Option<i32> {
    let v = unsafe { sys::transfer_to_dest(to, dest as i32) };
    (v >= 0).then_some(v)
}

/// `H.RingMultiplier`.
pub fn ring_multiplier() -> i32 {
    unsafe { sys::ring_multiplier() }
}

/// `H._ringBonus += n`; returns the new multiplier.
pub fn add_ring_bonus(n: i32) -> i32 {
    unsafe { sys::add_ring_bonus(n) }
}

/// `H.ForceTeleport(..., resolve: false)` -- move a player without settling.
pub fn teleport_to(player_id: i32, tile: i32) {
    unsafe { sys::teleport_to(player_id, tile) }
}

/// `H.Unplace` -- take this card out of play. True when it was there.
/// Take **this instance** off the field; returns the owner it left (or -1).
/// [`unplace_card_named`] is the form for some *other* card.
pub fn unplace_self() -> i32 {
    unsafe { sys::unplace_card() }
}

/// Expire the active event `id` (`docs/EVENTS.md`): unbind its rule instance
/// and file it away. `removed = true` is 「永久移除」; `false` is 「放入事件弃牌」.
/// A one-shot event never calls this -- `draw_event` files it away itself when
/// the body does not stay.
pub fn event_expire(id: &str, removed: bool) {
    let (p, l) = s(id);
    unsafe { sys::event_expire(p, l, removed as i32) }
}

/// Is `id` active and face-up? 「此卡在场上则…」
pub fn event_is_active(id: &str) -> bool {
    let (p, l) = s(id);
    unsafe { sys::event_is_active(p, l) != 0 }
}

/// Put `id` on the top of the event deck. `face_down` is 「背面朝上放置于事件
/// 牌堆顶部」 -- the draw still reveals it (「向所有玩家公开」).
pub fn event_deck_push(id: &str, face_down: bool) {
    let (p, l) = s(id);
    unsafe { sys::event_deck_push(p, l, face_down as i32) }
}

/// Take `id` out of the event deck / discard / active list for good --
/// 「从所有非衍生事件中选择3个移除」.
pub fn event_banish(id: &str) {
    let (p, l) = s(id);
    unsafe { sys::event_banish(p, l) }
}

/// Is this card in play at the player?
pub fn is_placed() -> bool {
    unsafe { sys::is_placed() != 0 }
}

/// Miracle crystals on **this card instance** (C# `Card.Crystals`). The
/// instance is the one running -- its placement is not a parameter, because the
/// host knows it from the dispatch and the same card id can sit on several
/// players' fields at once. [`card_crystals`] is the form for a *named* other
/// card, which does need to say whose field to look on.
pub fn crystals() -> i32 {
    unsafe { sys::crystals() }
}

/// Set this card instance's crystals (C# `Card.Crystals = n`); returns the new
/// count.
pub fn set_crystals(n: i32) -> i32 {
    unsafe { sys::set_crystals(n) }
}

/// `H.AddCrystals` -- adjust this card instance's crystals by `n`, clamped at 0
/// and at `max` (`0` = uncapped); returns the new count.
pub fn add_crystals(n: i32, max: i32) -> Result<i32, Prompt> {
    asked(unsafe { sys::add_crystals(n, max) })
}

/// One declared **property** of the running rule instance (`FieldCard::props`,
/// see [`crate::abi::prop`]). A tile rule body reads its tile data this way
/// (`prop::PRICE`, `prop::RENT_PREFIX` + level, …); a key the instance does
/// not carry reads as its defined default, `0`. The instance is the one
/// running -- same reasoning as [`crystals`].
pub fn prop(key: &str) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::self_prop(p, l) }
}

/// Write a **property** on the running rule instance. What a card that bends a
/// tile does instead of an engine flag: 黑衣人的补给 sets `prop::NO_REWARD`,
/// （soyo） sets `prop::GROUP`, and so on. `docs/TILES.md`.
pub fn set_prop(key: &str, value: i32) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::set_self_prop(p, l, value) }
}

/// A **property of the rule instance governing `tile`** (`docs/TILES.md`) --
/// what a card that bends a *tile* writes instead of an engine flag. The
/// reader is the tile instance; the source owns the arming and the disarming.
/// A tile with no rule instance reads as the key's default (`0`).
pub fn tile_prop(tile: i32, key: &str) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::tile_prop(tile, p, l) }
}

/// Write [`tile_prop`] on the rule instance(s) governing `tile`; returns the
/// stored value. 「无法获取[CiRCLE奖励]」 arms `prop::NO_REWARD` on CiRCLE's
/// `tile:circle` instance this way, and the reward step reads it back.
pub fn set_tile_prop(tile: i32, key: &str, value: i32) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::set_tile_prop(tile, p, l, value) }
}

/// The [经过] CiRCLE reward -- `H.CircleReward`, the body of `tile:circle`'s
/// Pass entry. 规则书: 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]」.
/// The engine runs the whole step: it consults `prop::NO_REWARD` on this
/// instance, offers 「获得2000资金或抽1张卡」, raises `circleAffected`, and
/// pays out. `landing` picks the 「获得」 wording for a stop on CiRCLE as
/// against a pass over it. Pauses; `?` it.
pub fn settle_circle_reward(player_id: i32, landing: bool) -> Result<(), Prompt> {
    asked(unsafe { sys::settle_circle_reward(player_id, landing as i32) })?;
    Ok(())
}

// --------------------------------------------------------- marks & tokens

/// `H.AddMark` -- a marker on a tile. `kind` names it (an i18n key), `note` explains it.
pub fn add_mark(tile: i32, player_id: i32, kind: &str, note: &Msg) {
    let (kp, kl) = s(kind);
    let (np, nl) = mj(note);
    unsafe { sys::add_mark(tile, player_id, kp, kl, np, nl) }
}

/// `H.CountMarks` -- marks on a tile (`kind` "" = any; `owner` -2 = any).
pub fn count_marks(tile: i32, kind: &str, owner: i32) -> i32 {
    let (kp, kl) = s(kind);
    unsafe { sys::count_marks(tile, kp, kl, owner) }
}

pub fn remove_marks(tile: i32, kind: &str, owner: i32) -> i32 {
    let (kp, kl) = s(kind);
    unsafe { sys::remove_marks(tile, kp, kl, owner) }
}

// ------------------------------------------------------------ [CP点]
// Two kinds (user ruling 2026-10-07). **Tile marks** are the `mark:cp` rule
// owner's small API (`rules/tiles/src/cp.rs`): [CP点] is a tile-mark category
// of its own -- 「放置于路面上的指示物」 (`data/rules.txt` 125) -- held by the
// neutral board owner, **never by a player**. A card places / counts / clears
// through these and nothing else; the writer stamps the placing card instance
// as provenance (`TileMark.src`) so 「此卡在格子上添加的[CP点]及其产物」 can be
// told apart from someone else's. **On-card** [CP点] is `FieldCard::cp` --
// 「自己[场上]N个[CP点]」, the CP points attached to the card itself (the card
// rule's own stock, crystals-like) -- `cp_attached` / `add_cp` / `cp_at` /
// `add_cp_at`.

/// Place one [CP点] on `tile`, attached to **this card instance** (provenance;
/// not an owner). Returns how many [CP点] the tile now carries.
///
/// 通用:该清CP了 [手] 「在任意一个没有角色和[CP点]的格子上添加1个[CP点]」 --
/// the *where* is the caller's gate; this only owns the what and the attachment.
pub fn place_cp(tile: i32) -> i32 {
    unsafe { sys::place_cp(tile) }
}

/// [CP点] on `tile`, any provenance. 通用:该清CP了 [手] 「在拥有[CP]点的格子上
/// [结算]时」 / 「没有[CP点]的格子」 read this.
pub fn count_cp(tile: i32) -> i32 {
    unsafe { sys::count_cp(tile) }
}

/// [CP点] on `tile` that **this card instance** placed and their products --
/// 通用:该清CP了 (1) 「此卡在格子上添加的[CP点]及其产物」.
pub fn count_cp_from(tile: i32) -> i32 {
    unsafe { sys::count_cp_from(tile) }
}

/// Remove one [CP点] from `tile` (通用:该清CP了 [手] 「移除格子上的个[CP点]」);
/// returns how many are left there. A tile-mark write: it does not touch
/// anyone's on-card count.
pub fn clear_cp(tile: i32) -> i32 {
    unsafe { sys::clear_cp(tile) }
}

/// The card instance a [CP点] on `tile` is attached to (`TileMark.src`), or
/// `-1` when the tile has none. The settle clause finds 「自己[场上]1个[CP点]」
/// this way: that card's on-card count.
pub fn cp_src_at(tile: i32) -> i32 {
    unsafe { sys::cp_src_at(tile) }
}

/// On-card [CP点] on **this card instance** (`FieldCard::cp`) -- 「自己[场上]N个
/// [CP点]」 (user ruling 2026-10-07: 「the cp point attached to the card」). The
/// 该清CP了 graveyard rule -- 「as soon as the attached on-card cp mark is
/// empty」 -- is this hitting 0, as a [`crate::abi::HookKind::CpChanged`]
/// handler and not a re-check at each spend site.
pub fn cp_attached() -> i32 {
    unsafe { sys::cp_attached() }
}

/// Adjust **this card instance's** on-card [CP点] by `n`, clamped at 0 and at
/// `max` (`0` = uncapped); returns the new count.
///
/// 通用:该清CP了 [手] 「并在自己[场上]添加6个[CP点]」 is `add_cp(6, 0)`.
pub fn add_cp(n: i32, max: i32) -> i32 {
    unsafe { sys::add_cp(n, max) }
}

/// On-card [CP点] on the instance at `uid` (`FieldCard::cp`). The `mark:cp`
/// settle clause reads the card the tile mark is attached to this way.
pub fn cp_at(uid: i32) -> i32 {
    unsafe { sys::cp_at(uid) }
}

/// Adjust the on-card [CP点] on the instance at `uid`; `max` caps (0 =
/// uncapped). Returns the new count. 通用:该清CP了 [手] 「自己[场上]1个[CP点]」
/// (spent by the settle clause against the tile mark's `src` card) is
/// `add_cp_at(src, -1, 0)`.
pub fn add_cp_at(uid: i32, n: i32, max: i32) -> i32 {
    unsafe { sys::add_cp_at(uid, n, max) }
}

pub fn tok(player_id: i32, name: &str) -> i32 {
    let (p, l) = s(name);
    unsafe { sys::tok(player_id, p, l) }
}

pub fn set_tok(player_id: i32, name: &str, value: i32) {
    let (p, l) = s(name);
    unsafe { sys::set_tok(player_id, p, l, value) }
}

/// `H.AddTok` -- returns how much the counter actually moved by.
///
/// Marker window (user ruling 2026-10-07): `markerSpend` / `markerGain` open
/// **before** the counters move; a cancelled link moves nothing (`Ok(0)`).
pub fn add_tok(player_id: i32, name: &str, by: i32, max: i32) -> Result<i32, Prompt> {
    let (p, l) = s(name);
    asked(unsafe { sys::add_tok(player_id, p, l, by, max) })
}

// ---------------------------------------------------------- keyed state

/// The keyed state surface: `{value, min, max, expires}` items the engine holds
/// and enforces **nothing**.
///
/// Which keys mean what, and what their caps are, belongs to whoever uses them:
/// a character skill that mandates a fire-pot cap writes [`Self::max`] on
/// [`state_key::FIRE`], and the card that gains pots is free to honour it (the
/// `fire` / `gain_fire` helpers do) or ignore it. There is no engine-side
/// `match key`, and a raw [`Self::set`] / [`Self::add`] is never clamped.
pub mod state {
    use super::{s, sys};

    /// Which column of a state item.
    pub const VALUE: i32 = 0;
    pub const MIN: i32 = 1;
    /// The cap a consumer may enforce. 0 when none has been mandated.
    pub const MAX: i32 = 2;
    /// When it wears off: [`NEVER`] / [`TURN_START`] / [`TURN_END`].
    pub const EXPIRES: i32 = 3;

    /// Never wears off.
    pub const NEVER: i32 = 0;
    /// Loses a layer at its owner's start of turn.
    pub const TURN_START: i32 = 1;
    /// Loses a layer at its owner's end of turn.
    pub const TURN_END: i32 = 2;

    fn get_field(player_id: i32, key: &str, field: i32) -> i32 {
        let (p, l) = s(key);
        unsafe { sys::state_get(player_id, p, l, field) }
    }

    fn set_field(player_id: i32, key: &str, field: i32, value: i32) -> i32 {
        let (p, l) = s(key);
        unsafe { sys::state_set(player_id, p, l, field, value) }
    }

    /// The value.
    pub fn get(player_id: i32, key: &str) -> i32 {
        get_field(player_id, key, VALUE)
    }

    pub fn min(player_id: i32, key: &str) -> i32 {
        get_field(player_id, key, MIN)
    }

    /// The mandated cap -- `0` when no skill has declared one.
    pub fn max(player_id: i32, key: &str) -> i32 {
        get_field(player_id, key, MAX)
    }

    /// [`NEVER`] / [`TURN_START`] / [`TURN_END`].
    pub fn expires(player_id: i32, key: &str) -> i32 {
        get_field(player_id, key, EXPIRES)
    }

    /// Write the value raw. Not clamped to `min`/`max`.
    pub fn set(player_id: i32, key: &str, value: i32) -> i32 {
        set_field(player_id, key, VALUE, value)
    }

    /// Move the value by `delta` raw. No clamping, no logging.
    pub fn add(player_id: i32, key: &str, delta: i32) -> i32 {
        let (p, l) = s(key);
        unsafe { sys::state_add(player_id, p, l, delta) }
    }

    /// Declare the bounds a consumer may enforce. Stored, not applied.
    pub fn set_bounds(player_id: i32, key: &str, min: i32, max: i32) {
        set_field(player_id, key, MIN, min);
        set_field(player_id, key, MAX, max);
    }

    /// Raise (or lower) the cap. Does not touch the value -- the caller is the
    /// one that cares whether it should.
    pub fn set_max(player_id: i32, key: &str, max: i32) -> i32 {
        set_field(player_id, key, MAX, max)
    }

    /// Declare when this counter wears off.
    pub fn set_expires(player_id: i32, key: &str, expires: i32) -> i32 {
        set_field(player_id, key, EXPIRES, expires)
    }
}

// ------------------------------------------------------ per-player slots (V)

/// A free-form counter (C# `H.V`). Sugar over [`state`].
pub fn slot(player_id: i32, key: &str) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::slot(player_id, p, l) }
}

pub fn set_slot(player_id: i32, key: &str, value: i32) {
    let (p, l) = s(key);
    unsafe { sys::set_slot(player_id, p, l, value) }
}

pub fn inc_slot(player_id: i32, key: &str, by: i32) -> i32 {
    let (p, l) = s(key);
    unsafe { sys::inc_slot(player_id, p, l, by) }
}

// ----------------------------------------------------------- pots & status

/// 「乐队卡 / 团卡」 [奇迹水晶] -- the count on `player_id`'s **band-skill field
/// instance** (`skill:<band>:<skill>`). 0 when the player has no band skill.
///
/// This is one pool with [`crystals`] / [`add_crystals`] inside a band skill's
/// own handlers (they run on that same instance), so a card that says 「为乐队卡
/// 添加N个[奇迹水晶]」 feeds exactly what the band skill spends. There is no
/// second store.
pub fn band_crystals(player_id: i32) -> i32 {
    unsafe { sys::band_crystals(player_id) }
}

/// Add `n` to `player_id`'s band-card crystals ([`band_crystals`]). `max` > 0
/// clamps the result, `max` = 0 is uncapped -- the caller declares the cap
/// (「最多10个」), the engine does not invent one. Returns the new count; 0 (and
/// a no-op) when the player has no band skill.
pub fn add_band_crystals(player_id: i32, n: i32, max: i32) -> i32 {
    unsafe { sys::add_band_crystals(player_id, n, max) }
}

/// The rule id of `player_id`'s **band skill** attachment (C# `H._fx[i].bands`'s
/// own band card -- `skill:<band>:<skill>`, `FieldCard::band_skill`), or `None`
/// when the player has none. This is 「乐队技能」: 「立即执行乐队技能的（2）效果」
/// names it, and [`invoke_skill`] runs a numbered effect on it.
pub fn band_skill(player_id: i32) -> Option<String> {
    let cap = 1024;
    let mut buf = alloc::vec![0u8; cap as usize];
    let n = unsafe { sys::band_skill(player_id, buf.as_mut_ptr() as i32, cap) };
    if n <= 0 || n > cap {
        return None;
    }
    let s: String = postcard::from_bytes(&buf[..n as usize]).unwrap_or_default();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// The rule id of `player_id`'s **character skill** (C# `H._fx[i].skill` --
/// `skill:<character>:<skill>`), or `None`. A card that reaches 「你的技能」
/// (pareo_far's 「视为你的房屋总数增加」 -> `SkillPareo -> Offer()`) names it and
/// [`invoke_skill`] runs its offer.
pub fn character_skill(player_id: i32) -> Option<String> {
    let cap = 1024;
    let mut buf = alloc::vec![0u8; cap as usize];
    let n = unsafe { sys::character_skill(player_id, buf.as_mut_ptr() as i32, cap) };
    if n <= 0 || n > cap {
        return None;
    }
    let s: String = postcard::from_bytes(&buf[..n as usize]).unwrap_or_default();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// One band-skill attachment: `(uid, rule id, extra)` in placement order.
/// `extra` is a 「拿取」ed copy (C# `MakeBand(.., extra: true)`): 「相同乐队技能卡
/// 的效果不可叠加」 and 「不视为那个乐队的角色」.
pub fn band_skills(player_id: i32) -> Vec<(i32, String, i32)> {
    let cap = 8192;
    let mut buf = alloc::vec![0u8; cap as usize];
    let n = unsafe { sys::band_skills(player_id, buf.as_mut_ptr() as i32, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// Attach a band-skill instance to `player_id` (C# `H.MakeBand(band, user,
/// extra)`). `extra` marks a 「拿取」ed copy (「相同乐队技能卡的效果不可叠加」 /
/// 「不视为那个乐队的角色」). Returns the new instance's uid, or -1 when the id
/// is not a band skill or the player is gone. Idempotent per id: a second
/// attach of an id already present (extra or not) is refused, which is the
/// 「不可叠加」 half.
pub fn add_band_skill(player_id: i32, id: &str, extra: bool) -> i32 {
    let (p, l) = s(id);
    unsafe { sys::add_band_skill(player_id, p, l, extra as i32) }
}

/// Run a skill rule's **press entry** (`On::Play`) for `player_id`, nested in
/// this run the way [`play_card`] is (C# `BandCrychic.TransformNow()` /
/// `SkillPareo -> Offer()`). Returns where the skill says it goes ([`Dest`]);
/// skills normally leave their own instance in place. Unlike the engine's
/// `use_skill` (the player pressing the skill button) this raises **no**
/// `skillUsed` -- it is a card executing the body, not the player using it.
pub fn invoke_skill(player_id: i32, id: &str) -> Result<Dest, Prompt> {
    let (p, l) = s(id);
    let v = asked(unsafe { sys::invoke_skill(player_id, p, l) })?;
    Ok(Dest::from_i32(v))
}

/// C# `f.Bought(i, t)` -- announce that `player_id` just became the owner of
/// `tile`, so the `bought` hook chain (「购买」 hooks: Afterglow's free
/// house, ...) hears it. A card that hands a deed over outside the buy routine
/// (tomoe_savior's 「从该玩家处收购该地契」) calls this after the ownership
/// change; `buy()` raises the same hook itself. The run's own player is the
/// cause (`t.by_card`).
pub fn raise_bought(player_id: i32, tile: i32) -> bool {
    unsafe { sys::raise_bought(player_id, tile) != 0 }
}

pub fn fire(player_id: i32) -> i32 {
    unsafe { sys::fire(player_id) }
}

pub fn fire_max(player_id: i32) -> i32 {
    unsafe { sys::fire_max(player_id) }
}

/// `H.GainFire` -- fire pots, capped by the player's own cap.
pub fn gain_fire(player_id: i32, n: i32, why: &Msg) -> Result<i32, Prompt> {
    let (p, l) = mj(why);
    asked(unsafe { sys::gain_fire(player_id, n, p, l) })
}

pub fn give_stay(player_id: i32, n: i32) {
    unsafe { sys::give_stay(player_id, n) }
}

pub fn give_stun(player_id: i32, n: i32) {
    unsafe { sys::give_stun(player_id, n) }
}

/// `H.GiveExile(seat, layers, back_to)`.
pub fn give_exile(player_id: i32, n: i32, to: i32) {
    unsafe { sys::give_exile(player_id, n, to) }
}

/// `H.GiveExtraTurn`.
pub fn give_extra_turn(player_id: i32) {
    unsafe { sys::give_extra_turn(player_id) }
}

/// `H.CanPay` -- not out, not stunned, not exiled.
pub fn can_pay(player_id: i32) -> bool {
    unsafe { sys::can_pay(player_id) != 0 }
}

/// C# `H.MoveWhyNot` -- `None` when the player may still make this turn's main
/// move, else the shared refusal (`status.move_*` messages).
pub fn cant_move(player_id: i32) -> Option<Msg> {
    match unsafe { sys::cant_move(player_id) } {
        0 => None,
        1 => Some(Msg::new("status.move_not_your_turn")),
        2 => Some(Msg::new("status.move_already")),
        _ => Some(Msg::new("status.move_blocked")),
    }
}

/// C# `H.CardMove(c, m)` -- run the move being planned **now**. Shape it with
/// [`plan`] first (`set_steps` / `set_kind` / `set_tag` / ...), then call this:
/// the engine runs the move (it may prompt, and the usual move triggers fire)
/// and the effect continues after it. Like `play_card`, the run pauses for it.
pub fn card_move(player_id: i32) -> bool {
    unsafe { sys::card_move(player_id) != 0 }
}

/// C# `H.AgentLanding` -- `player_id` lands on `agent` as a 「星光代理」: the
/// buy-or-pay-rent routine runs engine-side.
pub fn agent_landing(player_id: i32, agent: i32) -> bool {
    unsafe { sys::agent_landing(player_id, agent) != 0 }
}

/// C# `H.SpendFire` -- spend `n` [火罐]; false when the player has fewer. Logs
/// the spend (`why` is the reason message).
///
/// Marker window (user ruling 2026-10-07): `markerSpend` opens **before** the
/// fire moves and may cancel it (`Err(Prompt)` to pause, `Ok(false)` when a
/// counteraction cancelled the link).
pub fn spend_fire(player_id: i32, n: i32, why: &Msg) -> Result<bool, Prompt> {
    let (p, l) = mj(why);
    asked(unsafe { sys::spend_fire(player_id, n, p, l) }).map(|v| v != 0)
}

/// `[停留]` layers on the player (C# `H.State.seats[s].stay`).
pub fn stay_of(player_id: i32) -> i32 {
    unsafe { sys::stay_of(player_id) }
}

/// `[晕眩]` layers on the player (C# `H.State.seats[s].stun`).
pub fn stun_of(player_id: i32) -> i32 {
    unsafe { sys::stun_of(player_id) }
}

/// `H.State.turn` -- whose turn it is (-1 when none).
pub fn turn_player() -> i32 {
    unsafe { sys::turn_player() }
}

/// `H.State.round` -- the round counter.
pub fn round_no() -> i32 {
    unsafe { sys::round_no() }
}

/// C# `H.TurnKey` -- `round * 100 + turn + 1`: a per-turn id for once-per-turn
/// latches (`set_slot(player_id, key, turn_key())` then compare).
pub fn turn_key() -> i32 {
    unsafe { sys::turn_key() }
}

/// Is the player's character exactly `name`? (C# `H.CharacterOf`-style checks.)
pub fn character_is(player_id: i32, name: &str) -> bool {
    let (p, l) = s(name);
    unsafe { sys::character_is(player_id, p, l) != 0 }
}

/// C# `H.BandOf(seat) == name` -- is the player's character in this band?
pub fn in_band(player_id: i32, name: &str) -> bool {
    let (p, l) = s(name);
    unsafe { sys::in_band(player_id, p, l) != 0 }
}

/// `PlayCtx.Dest` -- where a card goes when its effect finishes (C# fates
/// "discard" / "hand" / "placed" / "gone"). The port names them Graveyard /
/// Hand / Field / Banished; wire values 0..=3 are the C# order.
///
/// One fate, two movers. A **play** names where its hand card ends and the
/// engine applies it (`play_from_hand`); a **field effect** names where the
/// instance it is running for ends and the host applies it when the run
/// commits (C# `H.Unplace(this, "discard")` and kin). 「将此卡放入[使用者]
/// 弃卡区」 is `Dest::Graveyard` either way -- no separate unplace-plus-discard
/// dance. A run that names nothing leaves a placed card where it is.
///
/// Planned: the draw-pile fates, one per insert position -- C# `c.Dest =
/// "deck"` -> `H.AddToDeck(player, card, where)` with `where` = `"top"` /
/// `"bottom"` / `"shuffle"` (note the C# default is **shuffle**, not top):
/// `DeckTop` = 4, `DeckBottom` = 5, `DeckRandom` = 6. When they land they need
/// arms in `game-rules`'s `dest_from` and `game-core`'s play-from-hand dest
/// handling. (`ctx::add_to_deck`'s `shuffle` flag already covers top vs random;
/// bottom-insert is still missing there.)
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dest {
    /// The discard pile (弃卡区) -- the default.
    Graveyard = 0,
    /// Back to the hand (手牌).
    Hand = 1,
    /// Stays in play (场上) -- a persistent effect.
    Field = 2,
    /// 「[移除]」 -- out of the game entirely.
    Banished = 3,
    /// Onto the top of the draw pile (C# `H.AddToDeck(seat, card, "top")`).
    DeckTop = 4,
    /// Under the draw pile (C# `"bottom"`).
    DeckBottom = 5,
    /// Shuffled into the draw pile (C# `"shuffle"`, the `AddToDeck` default).
    DeckRandom = 6,
}

impl Dest {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Hand,
            2 => Self::Field,
            3 => Self::Banished,
            4 => Self::DeckTop,
            5 => Self::DeckBottom,
            6 => Self::DeckRandom,
            _ => Self::Graveyard,
        }
    }
}

// ------------------------------------------------------------- prompts

/// The host answers `EXIT_NEED_INPUT` when it has published a question and wants
/// the run to stop so it can be replayed with the answer. Every prompt-shaped
/// call funnels through here so the sentinel becomes `Err(Prompt)` once and the
/// card body just `?`s it.
fn asked(i: i32) -> Result<i32, Prompt> {
    if i == crate::abi::EXIT_NEED_INPUT {
        Err(Prompt)
    } else {
        Ok(i)
    }
}

fn ask_raw(kind: PromptKind, player_id: i32, title: &Msg, text: &Msg) -> Result<i32, Prompt> {
    let (tp, tl) = mj(title);
    let (xp, xl) = mj(text);
    asked(unsafe { sys::ask(kind as i32, player_id, tp, tl, xp, xl) })
}

/// `H.AskTileOf` -- returns the chosen **tile**, not the index.
/// Falls back to the first option if the answer is out of range.
pub fn ask_tile(player_id: i32, title: &Msg, text: &Msg, tiles: &[i32]) -> Result<i32, Prompt> {
    for &t in tiles {
        unsafe { sys::opt_int(t) }
    }
    let i = ask_raw(PromptKind::Tile, player_id, title, text)?;
    Ok(tiles[(i.max(0) as usize).min(tiles.len().saturating_sub(1))])
}

/// `H.AskPick` -- returns the chosen option index.
pub fn ask_pick(player_id: i32, title: &Msg, text: &Msg, options: &[Msg]) -> Result<usize, Prompt> {
    for o in options {
        let (p, l) = mj(o);
        unsafe { sys::opt_str(p, l) }
    }
    let i = ask_raw(PromptKind::Choice, player_id, title, text)?;
    Ok((i.max(0) as usize).min(options.len().saturating_sub(1)))
}

/// `H.AskYes`.
pub fn ask_yes(player_id: i32, title: &Msg, text: &Msg) -> Result<bool, Prompt> {
    Ok(ask_raw(PromptKind::YesNo, player_id, title, text)? == 0)
}

/// `H.AskSeat` -- returns the chosen player.
pub fn ask_player(player_id: i32, title: &Msg, text: &Msg, players: &[i32]) -> Result<i32, Prompt> {
    for &x in players {
        unsafe { sys::opt_int(x) }
    }
    let i = ask_raw(PromptKind::Player, player_id, title, text)?;
    Ok(players[(i.max(0) as usize).min(players.len().saturating_sub(1))])
}

/// `H.AskSeat` where each option also names a **card** to show with it (Returns
/// 「复制哪名玩家的团卡？」 -- the player's band card rides the option). The
/// answer is still the chosen player; only the label gains a `card` argument,
/// so a client renders the card in the option instead of a bare name.
///
/// Built on [`ask_pick`] (`PromptKind::Choice`) rather than `PromptKind::Player`
/// because only the string-option path can carry a card per option. C#'s own
/// `H.AskSeat` is an `AskPick` over player-name options too, so this is the
/// faithful shape; bot / autopilot answering is unchanged (an index either way).
pub fn ask_player_cards(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    players: &[i32],
    cards: &[&str],
) -> Result<i32, Prompt> {
    let n = players.len().min(cards.len());
    let mut options: Vec<Msg> = Vec::with_capacity(n);
    for (p, c) in players.iter().zip(cards.iter()).take(n) {
        options.push(Msg::new("ask.player").player_id("who", *p).card("card", c));
    }
    if options.is_empty() {
        return Ok(-1);
    }
    let i = ask_pick(player_id, title, text, &options)?;
    Ok(players[(i.max(0) as usize).min(players.len().saturating_sub(1))])
}

/// `H.AskCard` -- pick one of `cards` (ids); returns the index.
pub fn ask_card(player_id: i32, title: &Msg, text: &Msg, cards: &[&str]) -> Result<usize, Prompt> {
    for c in cards {
        // `opt_str` takes a serialized `Msg`, not a raw string -- a card id
        // wrapped as `{{card}}` is the label.
        let (p, l) = mj(&Msg::new("ask.cardOption").card("card", c));
        unsafe { sys::opt_str(p, l) }
    }
    let i = ask_raw(PromptKind::Card, player_id, title, text)?;
    Ok((i.max(0) as usize).min(cards.len().saturating_sub(1)))
}

/// `H.AskNumber` -- a number in `min..=max`. The C# builds this as an `AskPick`
/// over the range, so the option list is the faithful shape (ranges in the card
/// pool are small; for a wide range, narrow the candidates yourself first).
pub fn ask_number(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    min: i32,
    max: i32,
) -> Result<i32, Prompt> {
    let min = min.min(max);
    let max = max.max(min);
    let mut options: Vec<Msg> = Vec::new();
    for n in min..=max {
        options.push(Msg::new("ask.intOption").i("n", n as i64));
    }
    if options.is_empty() {
        return Ok(min);
    }
    let i = ask_pick(player_id, title, text, &options)?;
    Ok(min + i as i32)
}

/// Run another card's `play` effect right now (C# `NewCard` + `Play`).
/// `H.PlayCard` -- run another card's `play` inside this run, as that card.
/// Returns where it says it goes (its `Dest`); moving it there is the caller's
/// job, since only the caller knows where the card came from.
pub fn play_card(id: &str, player_id: i32) -> Result<Dest, Prompt> {
    let (p, l) = s(id);
    let v = asked(unsafe { sys::play_card(p, l, player_id) })?;
    Ok(Dest::from_i32(v))
}

/// `H.CanReplay` -- could `player_id` play `id` right now (it has a `play` effect and
/// its `cant_play` gate is open)?
pub fn card_replayable(player_id: i32, id: &str) -> bool {
    let (p, l) = s(id);
    unsafe { sys::card_replayable(player_id, p, l) != 0 }
}

/// The move being planned (C# `TurnCtx.Plan`, a `MoveCtx`): what an
/// `On::RollPlan` body shapes before the dice, and what a card shapes before
/// running a move of its own. (`trigger::move_*` is different: it describes the
/// move that *caused* the current trigger.)
pub mod plan {
    use super::*;

    /// C# `SetSteps` -- the planned length; keeps the sign of a reverse walk.
    pub fn set_steps(n: i32) {
        unsafe { sys::set_steps(n) }
    }

    /// 「上一名玩家代替进行此次投掷」 (幻觉来了) -- attribute the planned roll to
    /// another seat. The log line shows 「by」; the move still belongs to the
    /// original player, and 「所有影响投掷的效果服从于原本进行投掷的玩家」.
    pub fn set_roller(player_id: i32) {
        unsafe { sys::set_roller(player_id) }
    }

    /// C# `Reverse` -- walk backwards.
    pub fn set_reverse(on: bool) {
        unsafe { sys::set_reverse(on as i32) }
    }

    /// C# `Signed` -- a negative roll walks backwards instead of clamping to 0.
    pub fn set_signed(on: bool) {
        unsafe { sys::set_signed(on as i32) }
    }

    /// C# `StopAt` -- force the walk to stop on this tile; -1 clears.
    pub fn set_stop_at(n: i32) {
        unsafe { sys::set_stop_at(n) }
    }

    /// C# `Parity` -- only odd (1) / even (0) tiles count; -1 either.
    pub fn set_parity(n: i32) {
        unsafe { sys::set_parity(n) }
    }

    /// C# `Resolve` -- settle where the walk lands.
    pub fn set_resolve(on: bool) {
        unsafe { sys::set_resolve(on as i32) }
    }

    /// C# `SettleTile` -- settle on this tile instead of the landing; -1 clears.
    pub fn set_settle_tile(n: i32) {
        unsafe { sys::set_settle_tile(n) }
    }

    /// C# `PayFactor` in milli-units (500 = x0.5): money paid on this walk.
    pub fn set_pay_factor(n: i32) {
        unsafe { sys::set_pay_factor(n) }
    }

    /// C# `RentFactor` in milli-units (500 = x0.5): rent paid on this walk.
    pub fn set_rent_factor(n: i32) {
        unsafe { sys::set_rent_factor(n) }
    }

    /// C# `NoBuy` -- the walk cannot buy where it lands.
    pub fn set_no_buy(on: bool) {
        unsafe { sys::set_no_buy(on as i32) }
    }

    /// C# `NoBuild` -- the walk cannot build where it lands.
    pub fn set_no_build(on: bool) {
        unsafe { sys::set_no_build(on as i32) }
    }

    /// C# `BuildAnywhere` -- the walk may build anywhere it lands.
    pub fn set_build_anywhere(on: bool) {
        unsafe { sys::set_build_anywhere(on as i32) }
    }

    /// How the move gets there (C# `m.Teleport`): [`MoveKind::Walk`] goes by
    /// the path, [`MoveKind::Teleport`] jumps to its destination (with
    /// `set_teleport_to` naming it, or the roll deriving it -- the old
    /// `TeleportWalk`, 「视为 [传送]（只触发终点）」).
    pub fn set_kind(kind: MoveKind) {
        unsafe { sys::set_kind(kind as i32) }
    }

    /// C# `MinRoll` -- clamp the final face up to this, after the counteractions.
    pub fn set_min_roll(n: i32) {
        unsafe { sys::set_min_roll(n) }
    }

    /// C# `ExtraSteps`.
    pub fn set_extra_steps(n: i32) {
        unsafe { sys::set_extra_steps(n) }
    }

    /// C# `MoreSteps`.
    pub fn set_more_steps(n: i32) {
        unsafe { sys::set_more_steps(n) }
    }

    /// `MoveCtx.Tags` -- a free-form per-card counter on this move. Card rules
    /// own effects like [火罐] rolls through these instead of an engine flag:
    /// the card that arms one tags the move (`set_tag("fireRoll", 1)`) and
    /// readers ask [`trigger::move_tag`].
    pub fn set_tag(key: &str, value: i32) {
        let (p, l) = s(key);
        unsafe { sys::set_tag(p, l, value) }
    }

    /// C# `NoCircleReward` -- passing CiRCLE pays nothing on this walk.

    /// C# `SettleAsAgent`.
    pub fn set_settle_as_agent(on: bool) {
        unsafe { sys::set_settle_as_agent(on as i32) }
    }

    /// 「使你的下次主要移动结果对那些玩家一起执行」 -- record `player_id` as a
    /// **follower** of the move being planned (C# `LeadFx.Who` + `Follow`).
    /// After the mover settles, the engine replays this move's result for each
    /// follower in the order they were added (「你先触发结算，此后其他玩家按
    /// 行动顺序依次触发结算」 -- add them in action order). The follower's
    /// replay carries the same plan (steps / kind / destination / `pay_factor`),
    /// so 「触发结算时进行的支付价格减半」 reaches them too.
    pub fn add_follower(player_id: i32) {
        unsafe { sys::plan_add_follower(player_id) }
    }

    /// C# `TeleportTo` -- the teleport's destination. -1 derives it from the
    /// roll (the 「视为 [传送]（只触发终点）」 shape).
    pub fn set_teleport_to(tile: i32) {
        unsafe { sys::set_teleport_to(tile) }
    }

    /// The tile the walk begins on instead of the player's own; -1 for the
    /// player's own. `why` is shown on the move's log line.
    pub fn set_start(tile: i32, why: &str) {
        let (p, l) = s(why);
        unsafe { sys::set_start(tile, p, l) }
    }

    /// C# `Base` -- replace the roll's dice table with `count`d`sides`.
    pub fn set_base_dice(count: i32, sides: i32, why: &str) {
        let (p, l) = s(why);
        unsafe { sys::set_base_dice(count, sides, p, l) }
    }

    /// Append `count`d`sides` to the roll's base dice.
    pub fn add_base_dice(count: i32, sides: i32, why: &str) {
        let (p, l) = s(why);
        unsafe { sys::add_base_dice(count, sides, p, l) }
    }

    /// Append `count`d`sides` to the roll as an extra term. A flat add is a
    /// `0`-sided term: `add_extra_dice(n, 0, why)` is what C# `Bonus` did.
    pub fn add_extra_dice(count: i32, sides: i32, why: &str) {
        let (p, l) = s(why);
        unsafe { sys::add_extra_dice(count, sides, p, l) }
    }

    /// Did the walk stop before its full length? Read-only: the walk loop sets
    /// it. To force a stop, name the tile with [`Self::set_stop_at`].
    pub fn stopped() -> bool {
        unsafe { sys::move_stopped() != 0 }
    }

    /// C# `StopAt`, or -1.
    pub fn stop_at() -> i32 {
        unsafe { sys::move_stop_at() }
    }

    /// C# `Parity`: -1 either, 0 even, 1 odd.
    pub fn parity() -> i32 {
        unsafe { sys::move_parity() }
    }

    /// C# `Resolve`.
    pub fn resolve() -> bool {
        unsafe { sys::move_resolve() != 0 }
    }

    /// The planned length (`MovePlan.Landing` is where it ends).
    pub fn steps() -> i32 {
        unsafe { sys::move_steps() }
    }

    /// C# `Remaining` -- steps left to walk.
    pub fn remaining() -> i32 {
        unsafe { sys::move_remaining() }
    }

    /// C# `Total` -- steps walked (`lastWalk` is written from this).
    pub fn total() -> i32 {
        unsafe { sys::move_total() }
    }

    /// C# `Dir` -- +1 forwards, -1 backwards.
    pub fn dir() -> i32 {
        unsafe { sys::move_dir() }
    }
    /// names the whole face means exactly that face.
    pub fn clear_dice() {
        unsafe { sys::clear_dice() }
    }
    /// C# `MoveCtx.CanBuild` -- may this move build where it lands?
    pub fn set_can_build(on: bool) {
        unsafe { sys::set_can_build(on as i32) }
    }
}

/// C# `H.Target(c, seat)` for a single-target card -- try to target `player_id`.
/// `None` when the targeting failed (out, exiled, immune, untargetable, or the
/// `target` [反击] window cancelled it); otherwise the player actually targeted,
/// which a field `redirect` hook may have changed (C# `IRedirect`).
pub fn target(player_id: i32) -> Option<i32> {
    let r = unsafe { sys::target(player_id, -1, 1) };
    (r >= 0).then_some(r)
}

/// C# `H.TargetAll(c, seats, got)` -- target each player in turn (no redirect);
/// returns the players that were targeted, in order, without duplicates.
pub fn target_all(players: &[i32]) -> Vec<i32> {
    let mut got = Vec::new();
    for &s in players {
        let r = unsafe { sys::target(s, -1, 0) };
        if r >= 0 && !got.contains(&r) {
            got.push(r);
        }
    }
    got
}

/// C# `H.TargetTile(c, tile)` -- try to target `tile` (and so its owner, when
/// someone else owns it). False when the targeting failed.
pub fn target_tile(tile: i32) -> bool {
    unsafe { sys::target(-1, tile, 0) >= 0 }
}

/// C# `H._targeted[player_id]` -- times other players' cards targeted `player_id` since its
/// own turn last started.
pub fn targeted_count(player_id: i32) -> i32 {
    unsafe { sys::targeted_count(player_id) }
}

/// 「当前回合内你每获得过一次资金」 -- money-ins for `player_id` during the
/// current turn (any cause: a print, a pay-player credit, a `gain_fixed`).
/// Zeroed for everyone at each turn start. This is what a hand card reads when
/// it has no field stand-in observing the gains (HHW:（育美）(2)).
pub fn gains_this_turn(player_id: i32) -> i32 {
    unsafe { sys::gains_this_turn(player_id) }
}

/// The **static targeting query**: which players the play being resolved (by
/// `player_id`) designates (C# `H.Db.Card(id).Targeting` + `H.Others`). Empty
/// when the play names nobody. This is what 「有[指定]目标」 /
/// 「取消其对目标之一的[指定]」 branches on, before the play's body has run.
pub fn designations(player_id: i32) -> Vec<i32> {
    let cap = 4096;
    let mut buf = alloc::vec![0u8; cap as usize];
    let n = unsafe { sys::designations(player_id, buf.as_mut_ptr() as i32, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// Per-pair cancel (「取消其对目标之一的[指定]」, C# `play.Tags["immune"+seat]`):
/// mark `seat`'s designation on the play being resolved as cancelled. The rest
/// of the play's designations still land.
pub fn cancel_designation(seat: i32) {
    unsafe { sys::cancel_designation(seat) }
}

/// Is `seat`'s designation on the play being resolved cancelled?
pub fn designation_cancelled(seat: i32) -> bool {
    unsafe { sys::designation_cancelled(seat) != 0 }
}

/// C# `_abnormalTurn[player_id]` -- abnormal effects that got through to `player_id`
/// this turn (reset for every player at each turn start).
pub fn abnormal_count(player_id: i32) -> i32 {
    unsafe { sys::abnormal_count(player_id) }
}

/// Is `id` placed on `player_id`'s field? `Some(tile)` with the tile it is bound to
/// (-1 when it is not on a tile), `None` when the player has no such card in play.
pub fn placed_tile(player_id: i32, id: &str) -> Option<i32> {
    let (p, l) = s(id);
    let v = unsafe { sys::placed_tile(player_id, p, l) };
    (v >= -1).then_some(v)
}

/// Arm the doubling for the run in progress -- the *playing* card's
/// `PlayCtx.Doubled`. CiRCLE's band skill sets it from outside that card, which
/// is why it is a world setter and not something the card owns. `-1` clears.
pub fn set_play_doubled(n: i32) {
    unsafe { sys::set_play_doubled(n) }
}

/// C# `PlayCtx.N(k, value)` -- a number from this card's text, doubled when the
/// play doubles its `k`-th number (C# `PlayCtx.Doubled`). Use it instead of a
/// bare literal for the numbers a doubling effect can target.
pub fn n(k: i32, value: i32) -> i32 {
    if unsafe { sys::play_doubled() } == k {
        value * 2
    } else {
        value
    }
}

// ------------------------------------------------- turn plan & scheduling

/// C# `TurnCtx.AfterEnd` -- run this card's `On::AtEnd` (for `player_id`) when the
/// current turn ends, *after* the end-of-turn status wear-off (so a [停留] it
/// grants lasts through the next turn). The card need not be in play.
/// Scheduling twice runs it twice.
pub fn at_turn_end(player_id: i32) {
    unsafe { sys::schedule_turn_end(player_id, 0) }
}

/// C# `TurnCtx.AtEnd` -- run this card's `On::AtEnd` (for `player_id`) when the
/// current turn ends, *before* the status wear-off (「回合结束时」 money losses,
/// discarding down to a hand size).
pub fn before_turn_end(player_id: i32) {
    unsafe { sys::schedule_turn_end(player_id, 2) }
}

/// 「你的下回合结束时」 -- run this card's `On::AtEnd` (for `player_id`) at the end of
/// `player_id`'s next turn (not the current one, if it is `player_id`'s), after its wear-off.
pub fn at_next_turn_end(player_id: i32) {
    unsafe { sys::schedule_turn_end(player_id, 1) }
}

/// C# `TurnCtx.NoMoneyLoss` -- `player_id`'s money cannot drop for the rest of this
/// turn (payments it would make are waived; auctions are not payments).
pub fn set_no_money_loss(player_id: i32) {
    unsafe { sys::set_no_money_loss(player_id) }
}

/// C# `TurnCtx.Plan.FixedRoll` -- this turn's main-move roll is `n`.
pub fn set_fixed_roll(n: i32) {
    unsafe { sys::set_fixed_roll(n) }
}

/// This turn's fixed main-move roll, if a card set one.
pub fn fixed_roll() -> Option<i32> {
    let v = unsafe { sys::fixed_roll() };
    (v >= 0).then_some(v)
}

/// C# `NextStepsFx` -- `player_id`'s next main move walks exactly `n` steps.
pub fn set_next_steps(player_id: i32, n: i32) {
    unsafe { sys::set_next_steps(player_id, n) }
}

/// C# `TurnCtx.LastMain` -- steps this turn's main move walked (0 = none yet).
pub fn turn_main_steps() -> i32 {
    unsafe { sys::turn_main_steps() }
}

/// C# `Card.FireMaxDelta` -- raise (or, negative, lower) `player_id`'s [火罐] cap;
/// returns the new cap.
pub fn add_fire_max(player_id: i32, n: i32) -> i32 {
    unsafe { sys::add_fire_max(player_id, n) }
}

/// C# `DecayCard` tick -- burn one of this card's crystals. Returns the count
/// left.
///
/// The 「…为0时放入弃牌堆」 half of a decay clause is **not** here: it is the
/// card's `HookKind::CrystalsChanged` handler, so a count emptied by *any*
/// write (another card's `add_card_crystals`, say) leaves the field too. This
/// used to unplace-and-discard at 0, which only covered the tick.
pub fn decay() -> Result<i32, Prompt> {
    // Propagate, never swallow: `add_crystals` opens the `markerSpend` window
    // (`HostRequest::Marker`), so it can pause. The old `unwrap_or(0)` turned
    // that pause into "decay did nothing" and the body kept going -- which the
    // replay model's `finish` boundary check traps ("card published a prompt
    // and kept going", the card goes out) and the inline answer model
    // (`docs/BOT.md` §3.2) answers. `?` makes both paths see the same body.
    add_crystals(-1, 0)
}

/// The trigger a counteraction is being checked against (C# `Trigger`).
pub mod trigger {
    use super::*;

    pub fn kind() -> TriggerKind {
        TriggerKind::from_i32(unsafe { sys::trig_kind() })
    }

    /// `t.Seat` -- whose action this is (the mover, the payer, the player of the card).
    pub fn player_id() -> i32 {
        unsafe { sys::trig_player() }
    }

    /// `t.Target` -- who it is aimed at (`t.Pay.to` on pay triggers), or -1.
    pub fn target() -> i32 {
        unsafe { sys::trig_target() }
    }

    /// `t.Tile` -- the tile involved (settled on, passed, mortgaged), or -1.
    pub fn tile() -> i32 {
        unsafe { sys::trig_tile() }
    }

    /// `t.Value` -- the amount involved (`t.Pay.amount` on pay triggers), or 0.
    pub fn value() -> i32 {
        unsafe { sys::trig_value() }
    }

    /// `t.Step` -- the turn stage (0 = no turn; 1 开始 / 2 运营 / 3 移动 / 4 结束) active when this trigger fired.
    /// Only meaningful for kinds that aren't step-specific (e.g. `mortgage`
    /// can fire during both step 1 and step 3).
    pub fn step() -> i32 {
        unsafe { sys::trig_step() }
    }

    /// `t.ByCard` -- the player whose card caused this trigger, or `None` when the
    /// trigger was not card-caused (board-driven: rent, buy, build, turn flow).
    /// This is what C# `H.HitByOtherCard` keys on: `by_card().is_some_and(|by|
    /// by != player)` means "another player's card did this to me". Distinct from
    /// `player_id()` (the mover/payer/target), which on a `pay` trigger is the
    /// *payer*, not the card that forced the payment.
    pub fn by_card() -> Option<i32> {
        let v = unsafe { sys::trig_by_card() };
        (v >= 0).then_some(v)
    }

    /// `t.Pay.IsRent` -- is this `pay`/`paid` trigger rent (C# `t.Pay.kind == "rent"`),
    /// as opposed to a buy, a build, or a forced loss. Always false on a
    /// card-driven payment.
    pub fn pay_is_rent() -> bool {
        unsafe { sys::trig_pay_is_rent() != 0 }
    }

    /// `t.Move` -- how the move that caused this trigger got there (C#
    /// `m.Teleport`): `None` when the move did not cause it. Present on
    /// `moveRoll` / `pass` / `settleBefore` / `settle` / `settleAfter` raised
    /// while a player is moving. A walk enters every tile it steps on; a teleport
    /// enters only its destination.
    pub fn move_kind() -> Option<MoveKind> {
        MoveKind::from_i32(unsafe { sys::trig_move_kind() })
    }

    /// `t.Move.Resolve` -- does that move settle where it lands (C#
    /// `m.Resolve`)? False = a card effect prevented settle at all.
    pub fn move_resolve() -> bool {
        unsafe { sys::trig_move_resolve() != 0 }
    }

    /// `t.Move.Tags[key]` -- a per-card counter the move carries (C# `m.Tags`).
    /// A [火罐] roll is card-owned state: whoever armed it tagged the move
    /// (`"fireRoll"` by convention).
    pub fn move_tag(key: &str) -> i32 {
        let (p, l) = s(key);
        unsafe { sys::trig_move_tag(p, l) }
    }

    /// `t.Move.Main` -- was this the turn's main move (C# `MoveCtx.main`)?
    pub fn move_is_main() -> bool {
        unsafe { sys::trig_move_main() != 0 }
    }

    /// `t.Move.Dir` -- the direction of the move: 1 forward, -1 backward.
    /// Only meaningful when the move caused the trigger ([`move_kind`]`().is_some()`).
    pub fn move_dir() -> i32 {
        unsafe { sys::trig_move_dir() }
    }

    /// The abnormal effect on an `abnormalGuard` / `abnormal` trigger.
    pub fn abnormal_kind() -> Option<AbKind> {
        match kind() {
            TriggerKind::Abnormal | TriggerKind::AbnormalGuard => AbKind::from_i32(value()),
            _ => None,
        }
    }

    /// The cards the trigger is about: the drawn cards on a `drew` trigger (in
    /// draw order), the skill id on a `skillUsed` trigger. Empty otherwise.
    pub fn cards() -> Vec<String> {
        let need = unsafe { sys::trig_cards(0, 0) };
        if need <= 0 {
            return Vec::new();
        }
        let mut buf: Vec<u8> = Vec::new();
        buf.resize(need as usize, 0);
        if unsafe { sys::trig_cards(buf.as_mut_ptr() as i32, need) } != need {
            return Vec::new();
        }
        postcard::from_bytes(&buf).unwrap_or_default()
    }

    /// `t.Move.Remaining` -- steps the move has left to walk.
    pub fn move_remaining() -> i32 {
        unsafe { sys::trig_move_remaining() }
    }

    /// `t.Move.Path.Count` -- the move's path length so far.
    pub fn move_total() -> i32 {
        unsafe { sys::trig_move_total() }
    }

    /// `t.Move.Roll` -- the roll being counteracted to, or -1.
    pub fn move_roll() -> Option<i32> {
        let v = unsafe { sys::trig_move_roll() };
        (v >= 0).then_some(v)
    }

    /// `t.Roll.Source` -- where a `roll` / `moveRoll` face came from, as a
    /// [`crate::abi::roll_source`] code (`0` = unattributed, `1` = a fire pot,
    /// `2` = a card, `3` = a skill). 「当你使用火罐进行掷骰时」 reads this.
    /// [`crate::abi::roll_source::NONE`] on every non-roll trigger.
    pub fn roll_source() -> i32 {
        unsafe { sys::trig_roll_source() }
    }

    // ---- v40 purchase payload (`docs/PURCHASE.md`) ------------------------

    /// `t.Buy.Kind` -- a [`crate::abi::BuyKind`] as `i32` (`0` = land).
    pub fn buy_kind() -> i32 {
        unsafe { sys::trig_buy_kind() }
    }

    /// `t.Buy.Seller` -- the payee (`-1` = the bank).
    pub fn seller() -> i32 {
        unsafe { sys::trig_seller() }
    }

    /// `t.Buy.Price` -- the price the buyer would be charged.
    pub fn price() -> i32 {
        unsafe { sys::trig_price() }
    }

    /// Rewrite the quoted price (the `BuyAdd` / `BuyMul` / `BuySet` stages).
    pub fn set_price(v: i32) {
        unsafe { sys::trig_set_price(v) }
    }

    /// `t.Buy.DealOwner` -- ownership after the deal (default: the buyer).
    pub fn deal_owner() -> i32 {
        unsafe { sys::trig_deal_owner() }
    }

    /// Rewrite the deal's post-commit owner (`BuyAssign`).
    pub fn set_deal_owner(v: i32) {
        unsafe { sys::trig_set_deal_owner(v) }
    }

    /// `t.Buy.DealHouses` -- houses after the deal (default: as standing).
    pub fn deal_houses() -> i32 {
        unsafe { sys::trig_deal_houses() }
    }

    /// Rewrite the deal's post-commit house count (`BuyAssign`, raze).
    pub fn set_deal_houses(v: i32) {
        unsafe { sys::trig_set_deal_houses(v) }
    }

    /// `t.Buy.DealMortgaged` -- mortgage after the deal.
    pub fn deal_mortgaged() -> bool {
        unsafe { sys::trig_deal_mortgaged() != 0 }
    }

    /// Rewrite the deal's post-commit mortgage flag (`BuyAssign`).
    pub fn set_deal_mortgaged(v: bool) {
        unsafe { sys::trig_set_deal_mortgaged(v as i32) }
    }

    /// A `BuyGate` refusal's reason key, shown instead of a bare cancel.
    pub fn set_reason(reason: &str) {
        let (p, l) = s(reason);
        unsafe { sys::trig_set_reason(p, l) }
    }

    pub fn set_move_roll(v: i32) {
        unsafe { sys::trig_set_move_roll(v) }
    }

    /// Rewrite the amount of a pending `pay`/`paid` trigger (C# `PayCtx.amount`).
    /// `0` cancels the payment outright (C# `t.Pay.cancel = true`). The engine
    /// honours whatever this leaves on the trigger once the counteraction window
    /// resolves.
    pub fn set_pay_amount(v: i32) {
        unsafe { sys::trig_set_pay_amount(v) }
    }

    /// Redirect the payee of a pending `pay` (C# `PayCtx.to`); `-1` sends the
    /// money to the bank instead. Combine with [`set_pay_amount`] to reshape a
    /// payment completely.
    pub fn set_pay_target(to: i32) {
        unsafe { sys::trig_set_pay_target(to) }
    }

    /// Rewrite `t.target` -- on a `redirect` hook, the player that takes the hit
    /// instead (C# `IRedirect`).
    pub fn set_target(player_id: i32) {
        unsafe { sys::trig_set_pay_target(player_id) }
    }

    /// Negate this link's **activation** (Yu-Gi-Oh: the card "did not
    /// activate"). It never happened -- nothing settles -- and a listener on the
    /// effect does not see it. The effect body is skipped while the point's
    /// Before/After hooks still fire.
    pub fn set_cancelled() {
        unsafe { sys::trig_set_cancelled() }
    }

    /// Negate this link's **effect** (Yu-Gi-Oh: it activated, its effect does
    /// nothing). A listener on the effect still sees it; it just settles to
    /// nothing. Weaker than [`set_cancelled`], and does not undo it.
    pub fn negate_effect() {
        unsafe { sys::trig_set_negate_effect() }
    }

    /// Take one recipient out of settlement. The effect still settles for
    /// everyone else -- this is *not* a complete invalidation; use
    /// [`set_cancelled`] for that. Counter cards differ on which they mean.
    pub fn spare(seat: i32) {
        unsafe { sys::trig_set_spare(seat) }
    }

    /// Has a counteraction already cancelled this trigger (`Trigger.Cancelled`)?
    pub fn cancelled() -> bool {
        unsafe { sys::trig_cancelled() != 0 }
    }

    /// `t.Card == id` / `t.Play.Id == id` -- is this trigger about that card?
    pub fn card_is(id: &str) -> bool {
        let (p, l) = s(id);
        unsafe { sys::trig_card_is(p, l) != 0 }
    }

    /// Position of this link within the current chain, 1-based. 0 = not on a
    /// chain (a bare hook or gate raise).
    pub fn seq() -> i32 {
        unsafe { sys::trig_seq() }
    }

    /// Which link this one answers. 0 = this is the effect declaration itself.
    pub fn answers() -> i32 {
        unsafe { sys::trig_answers() }
    }
}

/// The effects a chain link declares, with their recipients already named.
///
/// A [反击] card chooses how much of this to read. For the rulebook's recurring
/// 「被其他玩家的卡效果影响」 clause the *whole list* is the right view -- any
/// effect of that play touching me -- while a card like
/// `CRYCHIC:主唱太拼命了` (「一次性向其他玩家支付5000以上资金时」) wants one
/// entry. Both are here; the card picks.
pub mod effect {
    use super::sys;
    use crate::abi::TriggerKind;

    /// How many effects this link declares.
    pub fn count() -> i32 {
        unsafe { sys::trig_effect_count() }
    }

    /// What effect `i` does, as a [`TriggerKind`] wire value.
    pub fn kind(i: i32) -> TriggerKind {
        TriggerKind::from_i32(unsafe { sys::trig_effect_kind(i) })
    }

    /// The seat effect `i` is aimed at (-1 for none).
    pub fn target(i: i32) -> i32 {
        unsafe { sys::trig_effect_target(i) }
    }

    /// The other party to effect `i`, where it has one. A payment touches both
    /// its payer and its payee.
    pub fn from(i: i32) -> i32 {
        unsafe { sys::trig_effect_from(i) }
    }

    /// The tile effect `i` is aimed at (-1 for none).
    pub fn tile(i: i32) -> i32 {
        unsafe { sys::trig_effect_tile(i) }
    }

    /// The amount effect `i` carries, where it has one.
    pub fn value(i: i32) -> i32 {
        unsafe { sys::trig_effect_value(i) }
    }

    /// Does any declared effect touch `seat`? The whole-list view: 「被…效果
    /// 影响」 without asking which effect. A payment touches both its payer and
    /// its payee.
    pub fn hits(seat: i32) -> bool {
        (0..count()).any(|i| target(i) == seat || from(i) == seat)
    }

    /// Is any declared effect of `kind`? The per-effect view, for a card that
    /// means one thing specifically.
    pub fn has(kind: TriggerKind) -> bool {
        (0..count()).any(|i| self::kind(i) == kind)
    }

    /// Append an effect to the current link, naming its recipient now -- at
    /// declaration, not at settlement.
    pub fn declare(kind: TriggerKind, target: i32, from: i32, tile: i32, value: i32) {
        unsafe { sys::declare_effect(kind as i32, target, from, tile, value) }
    }
}

// ------------------------------------------------------------- placement & skills

/// C# `H.IsColor` -- does `tile` count as colour `group` for `player_id`?
pub fn is_color(player_id: i32, tile: i32, group: i32) -> bool {
    unsafe { sys::is_color(player_id, tile, group) != 0 }
}

/// `H.Unplace` -- take this card out of play. True when it was there.
/// Take a *named* card off the player's field (C# `H.Unplace(card, ...)`).
/// [`unplace_card`] is the special case where the named card is the one running.
pub fn unplace_card_named(player_id: i32, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::unplace_card_named(player_id, p, l) != 0 }
}

/// Move one matching mark's `count` by `delta`, dropping it at 0 (C#
/// `mark.count--`). [`remove_marks`] clears every match; this is the single-tick
/// form the 「移除一个」 clauses want. Returns the count now stored.
pub fn bump_mark(tile: i32, kind: &str, owner: i32, delta: i32) -> i32 {
    let (kp, kl) = s(kind);
    unsafe { sys::bump_mark(tile, kp, kl, owner, delta) }
}

/// Names of the player's non-zero counters whose name starts with `prefix`.
/// The listing half of the counter query; [`tok`] reads one by name.
pub fn tok_names(player_id: i32, prefix: &str) -> Vec<String> {
    let (p, l) = s(prefix);
    let cap = 4096;
    let mut buf = alloc::vec![0u8; cap as usize];
    let n = unsafe { sys::tok_names(player_id, p, l, buf.as_mut_ptr() as i32, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// The C# `H.AbnormalGate` -- ask the engine whether an abnormal effect of
/// `kind` (`[强制停下]`, `[传送]`, ...) may land on `player_id`. Returns `false`
/// when a field card guarded it, the player is immune, or they are
/// `unstoppable`. Call this **before** applying the effect, and skip on `false`.
pub fn gate(player_id: i32, kind: crate::abi::AbKind) -> bool {
    unsafe { sys::gate(player_id, kind as i32) != 0 }
}

/// `H.SettleAt` -- a full [触发结算] of `tile` for this player. The player does
/// not move; the tile's own effect resolves. `main` marks it as the turn's
/// landing (it writes `landed`).
pub fn card_settle_at(player_id: i32, tile: i32, main: bool) -> bool {
    unsafe { sys::card_settle_at(player_id, tile, main as i32) != 0 }
}

/// `H.OfferBuildAmong` -- prompt to build on one of `tiles`, then build there.
/// Silently skips when none of them can take a house.
pub fn card_offer_build(player_id: i32, tiles: &[i32]) -> bool {
    let mut buf = alloc::vec![0u8; tiles.len() * 4];
    for (i, &t) in tiles.iter().enumerate() {
        buf[i * 4..i * 4 + 4].copy_from_slice(&t.to_le_bytes());
    }
    unsafe { sys::card_offer_build(player_id, buf.as_ptr() as i32, buf.len() as i32) != 0 }
}

/// `H.DoMoveRoll` -- sum the planned move's dice tables into one face. The
/// `rollAfter` / `moveRoll` hooks are **not** raised: a re-roll is usually being
/// requested from inside one of them, and re-raising would recurse.
pub fn do_move_roll(player_id: i32) -> i32 {
    unsafe { sys::do_move_roll(player_id) }
}

/// `H.IsLiveHouse` for one player: the base check plus their `Fx.ExtraColor`.
pub fn is_live_house_for(player_id: i32, tile: i32) -> bool {
    unsafe { sys::is_live_house_for(player_id, tile) != 0 }
}

/// Is this card in play at the player?
/// Ids of the player's placed field cards, in placement order.
pub fn placed_cards(player_id: i32) -> Vec<String> {
    let cap = 4096;
    let mut buf = alloc::vec![0u8; cap as usize];
    let n = unsafe { sys::placed_cards(player_id, buf.as_mut_ptr() as i32, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// 「加盖房屋时半价」 / 「本回合加盖房屋变为免费」 -- the build cost as a
/// percentage of its table price (100 = full, 50 = half, 0 = free).
pub fn set_build_cost_pct(pct: i32) {
    unsafe { sys::set_build_cost_pct(pct) }
}

/// Flip a placed card face-down / face-up (C# `H.SwitchState`).
pub fn set_card_face_down(player_id: i32, card: &str, down: bool) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::set_card_face_down(player_id, cp, cl, down as i32) != 0 }
}

/// C# `Card.AddCrystals` on a named placed card; `max` caps (0 = uncapped).
pub fn add_card_crystals(player_id: i32, card: &str, n: i32, max: i32) -> i32 {
    let (cp, cl) = s(card);
    unsafe { sys::add_card_crystals(player_id, cp, cl, n, max) }
}

/// Is this placed card face-down? The `!p.FaceDown` half of the C# field
/// filters; [`cards_in`] lists face-down cards too.
pub fn card_face_down(player_id: i32, card: &str) -> bool {
    let (p, l) = s(card);
    unsafe { sys::card_face_down(player_id, p, l) != 0 }
}

// ------------------------------------------------------------- placement & skills

/// `H.PlaceCard` -- put a specific card (a derived one) into play at a player.
/// `WhyNotBuildOn` -- may this player build on this tile? The same gate the
/// build step uses, so a card choosing a destination cannot pick one the engine
/// would then refuse.
pub fn can_build_on(player_id: i32, tile: i32) -> bool {
    unsafe { sys::can_build_on(player_id, tile) != 0 }
}

/// `H.BuyRoutine` -- buy `tile` now (the 「必须购买」 clauses).
/// Kept as the P0 alias of [`buy`] with [`crate::abi::BuyKind::Card`].
pub fn card_buy(player_id: i32, tile: i32) -> bool {
    unsafe { sys::card_buy(player_id, tile) != 0 }
}

/// Batched purchase quote (`docs/PURCHASE.md`): what would `player_id` be
/// charged for each tile in `tiles`, and may they buy it at all?
///
/// Returns a postcard `Vec<(price, eligible)>` parallel to `tiles`
/// (`price = -1` when the tile is not buyable at all). One `HostRequest::Quote`
/// for the whole batch.
pub fn buy_quotes(player_id: i32, kind: i32, tiles: &[i32]) -> Vec<(i32, bool)> {
    let cap = 4096;
    let mut buf = alloc::vec![0u8; cap as usize];
    let raw: Vec<i32> = tiles.to_vec();
    let bytes = postcard::to_allocvec(&raw).unwrap_or_default();
    let n = unsafe {
        sys::buy_quotes(
            player_id,
            kind,
            bytes.as_ptr() as i32,
            bytes.len() as i32,
            buf.as_mut_ptr() as i32,
        )
    };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// Buy `tile` as `kind` (`docs/PURCHASE.md`). Replaces [`card_buy`] for new
/// call sites; `kind` selects the pipeline shape (Land / Agent / Card run the
/// money pipeline, Force / Acquire name the owner as payee, Auction is the
/// direct win).
pub fn buy(player_id: i32, tile: i32, kind: i32) -> bool {
    unsafe { sys::buy(player_id, tile, kind) != 0 }
}

/// 「收购」 -- take `tile` from `from` at `price`: pipeline pay, then assign →
/// `bought` → `buyAfter` (`docs/PURCHASE.md`).
pub fn acquire(player_id: i32, from: i32, tile: i32, price: i32) -> bool {
    unsafe { sys::acquire(player_id, from, tile, price) != 0 }
}

/// The agent offer's chosen branch: buy (`kind = 0`) or build (`kind = 1`)
/// `tile` from the `agent` tile's colour set.
pub fn agent_offer(player_id: i32, agent: i32, tile: i32, kind: i32) -> bool {
    unsafe { sys::agent_offer(player_id, agent, tile, kind) != 0 }
}

/// Bind the running card's own def as a **turn-scoped** instance in
/// `TurnCtx.lingering` (`docs/PURCHASE.md`). `expires` is the turn count it
/// survives (`0` = this turn only). This is the hand-card home for 「本回合」
/// effects: a `BuyAdd` / `BuyMul` / `BuySet` / `BuyAssign` hook on the card's
/// own def reaches the buy pipeline, and a [`set_prop`] made before this call
/// lands on the instance's props (e.g. `prop::NO_BUILD`). Cleared at turn
/// start and carried across `NeedHost` by `adopt_turn_policy`.
pub fn linger(player_id: i32, expires: i32) -> bool {
    unsafe { sys::linger(player_id, expires) != 0 }
}

/// Is this placed field card immune to other effects?
pub fn card_immune(player_id: i32, card: &str) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::card_immune(player_id, cp, cl) != 0 }
}

/// `H.MortgageRoutine` -- mortgage one of the player's deeds.
pub fn card_mortgage(player_id: i32, tile: i32) -> bool {
    unsafe { sys::card_mortgage(player_id, tile) != 0 }
}

/// Does this card's rule text mention `needle`? 「所有效果包含[奇迹水晶]的卡」
/// is this query.
pub fn card_text_mentions(card: &str, needle: &str) -> bool {
    let (cp, cl) = s(card);
    let (np, nl) = s(needle);
    unsafe { sys::card_text_mentions(cp, cl, np, nl) != 0 }
}

/// A gain that used to bypass the pipeline (C# `fixedAmount`).
///
/// `NEGATION-AUDIT` V4 (user ruling 2026-10-07): Tritone's 「立刻获得此次失去
/// 的资金金额」 is money movement and goes through the same pipeline as any
/// other gain -- the `effect` [反击] window, the modifier stages and the `pay`
/// settlement all see it. The C# `fixedAmount` bypass is gone; this is now
/// [`gain`] under a name the cards already use.
pub fn gain_fixed(player_id: i32, amount: i32, why: &Msg) -> Result<i32, Prompt> {
    gain(player_id, amount, why)
}

/// Is this the 地产商 tile (C# `kind == "agent"`)?
pub fn is_agent(tile: i32) -> bool {
    unsafe { sys::is_agent(tile) != 0 }
}

/// What this turn's [触发结算]s have cost the player so far (C#
/// `TurnCtx.PaidInSettle`).
pub fn paid_in_settle() -> i32 {
    unsafe { sys::paid_in_settle() }
}

/// Place a field card **on a tile** (the mark sits on the board at `tile`
/// rather than with its owner). Returns the new instance's uid.
pub fn place_card_on(player_id: i32, tile: i32, card: &str, note: &Msg) -> i32 {
    let (cp, cl) = s(card);
    let (p, l) = mj(note);
    unsafe { sys::place_card_on(player_id, tile, cp, cl, p, l) }
}

/// Did the play being resolved come from the hand? `false` when the card was
/// played from somewhere else -- 「若此卡从手牌以外的地方打出」.
pub fn play_from_hand() -> bool {
    unsafe { sys::play_from_hand() != 0 }
}

/// 「下次盖房时减免N（可溢出），盖房后减少1层」 -- a layered cut on the build
/// cost. Each build pops one layer.
pub fn set_build_discount(n: i32, layers: i32) {
    unsafe { sys::set_build_discount(n, layers) }
}

/// C# `Card.Immune` -- 「此卡不受…效果影响」. Set it at play time; effects that
/// would touch the card read [`card_immune`] and skip.
pub fn set_card_immune(player_id: i32, card: &str, on: bool) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::set_card_immune(player_id, cp, cl, on as i32) != 0 }
}

/// Move a placed field card to `tile` (C# `card.Tile = t`). The card is already
/// in play; this only changes where it sits. `tile: -1` puts it back with its
/// owner.
pub fn set_card_tile(player_id: i32, card: &str, tile: i32) -> bool {
    let (cp, cl) = s(card);
    unsafe { sys::set_card_tile(player_id, cp, cl, tile) != 0 }
}

/// Force the play's number ranges to their theoretical max (`1`) or min
/// (`-1`) -- 「以理论最大值或最小值结算」. `0` clears.
pub fn set_extreme(v: i32) {
    unsafe { sys::set_extreme(v) }
}

/// C# `_turnCtx.Rolls` -- every face rolled this turn, in order.
/// 「与本回合内你骰出过的所有骰点都不同」 compares against this.
pub fn turn_rolls() -> Vec<i32> {
    let cap = 4096;
    #[cfg(target_arch = "wasm32")]
    let (p, buf) = {
        let mut buf = alloc::vec![0u8; cap as usize];
        (buf.as_mut_ptr() as i32, buf)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let (p, mut buf) = {
        let (h, l) = crate::native::reserve(cap as usize);
        (h, alloc::vec![0u8; l as usize])
    };
    let n = unsafe { sys::turn_rolls(p, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    #[cfg(not(target_arch = "wasm32"))]
    let buf = crate::native::read(p, n as usize);
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// C# `_turnSnap[i]` -- `(pos, stay, stun, exile)` when the turn started. The
/// four things 「回到起始地点并取消所有受到的效果」 restores.
pub fn turn_snap(player_id: i32) -> (i32, i32, i32, i32) {
    #[cfg(target_arch = "wasm32")]
    let (p, mut buf) = {
        let mut buf = [0u8; 16];
        (buf.as_mut_ptr() as i32, buf)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let (p, mut buf) = {
        let (h, _) = crate::native::reserve(16);
        (h, [0u8; 16])
    };
    let n = unsafe { sys::turn_snap(player_id, p) };
    #[cfg(not(target_arch = "wasm32"))]
    let buf = crate::native::read(p, n.max(0) as usize);
    if n < 16 {
        return (-1, 0, 0, 0);
    }
    let g = |i: usize| i32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
    (g(0), g(4), g(8), g(12))
}

/// Where the player stood when this turn started (C# `_turnSnap[i].pos`).
/// 「在Livehouse地块开始回合时」 is a question about that square.
pub fn turn_start_pos(player_id: i32) -> i32 {
    unsafe { sys::turn_start_pos(player_id) }
}

// --------------------------------------------- card crystals & build

/// C# `Card.Crystals` on a named placed card.
pub fn card_crystals(player_id: i32, card: &str) -> i32 {
    let (cp, cl) = s(card);
    unsafe { sys::card_crystals(player_id, cp, cl) }
}

/// `H.BuildRoutine` -- pay `tile`'s build cost and raise one house.
pub fn card_build(player_id: i32, tile: i32) -> bool {
    unsafe { sys::card_build(player_id, tile) != 0 }
}

/// 「位于此卡所在格子上的玩家无法使用角色及乐队技能」 / 「不可使用任何<BAND>
/// 角色的（2）技能」 -- the shared skill-press gate. `skillBlock` on the player
/// suppresses every skill; `skillBlock:<band>` suppresses one band's. A
/// `skillBlock` mark on the tile the player stands on suppresses everyone there.
/// `band` is the skill's own band, or `""` when it does not have one.
pub fn skill_blocked(player_id: i32, band: &str) -> bool {
    if tok(player_id, "skillBlock") > 0 {
        return true;
    }
    if !band.is_empty() {
        let key = alloc::format!("skillBlock:{}", band);
        if tok(player_id, &key) > 0 {
            return true;
        }
    }
    let t = player_pos(player_id);
    t >= 0 && count_marks(t, "skillBlock", -2) > 0
}

/// Where **this instance** sits, or -1 for "with its owner" / gone
/// (C# `Card.Tile`). [`placed_tile`] is the form for some *other* card.
pub fn self_tile() -> Option<i32> {
    let v = unsafe { sys::self_tile() };
    (v >= -1).then_some(v)
}

/// Move **this instance** to `tile` (-1 = back with its owner).
pub fn set_self_tile(tile: i32) -> bool {
    unsafe { sys::set_self_tile(tile) != 0 }
}

/// Is **this instance** face-down?
pub fn self_face_down() -> bool {
    unsafe { sys::self_face_down() != 0 }
}

/// Flip **this instance**; returns whether it is on the field.
pub fn set_self_face_down(on: bool) -> bool {
    unsafe { sys::set_self_face_down(on as i32) != 0 }
}

/// Is **this instance** marked 「不受任何效果影响」?
pub fn self_immune() -> bool {
    unsafe { sys::self_immune() != 0 }
}

/// Mark **this instance** 「不受任何效果影响」; returns whether it is on the field.
pub fn set_self_immune(on: bool) -> bool {
    unsafe { sys::set_self_immune(on as i32) != 0 }
}

/// Every card instance on `player_id`'s field, as `(uid, id)` in placement
/// order. Walk this rather than [`placed_cards`] whenever the next step
/// addresses the instance -- the *name* is not an identity, so a name-keyed
/// op would hit the first copy twice when one player holds two.
pub fn field_instances(player_id: i32) -> Vec<(i32, String)> {
    let cap = 8192;
    let mut buf = alloc::vec![0u8; cap as usize];
    let n = unsafe { sys::field_instances(player_id, buf.as_mut_ptr() as i32, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// The instance at `uid`, wherever it sits. [`crystals`] / [`self_tile`] are
/// the forms for the running instance; these address some *other* one.
pub fn crystals_at(uid: i32) -> i32 {
    unsafe { sys::crystals_at(uid) }
}

/// `H.AddCrystals` on the instance at `uid`; `max` caps (0 = uncapped).
pub fn add_crystals_at(uid: i32, n: i32, max: i32) -> i32 {
    unsafe { sys::add_crystals_at(uid, n, max) }
}

/// Take the instance at `uid` off the field; returns the owner it left.
pub fn unplace_at(uid: i32) -> i32 {
    unsafe { sys::unplace_at(uid) }
}

/// Where the instance at `uid` sits (`Some(-1)` = with its owner, `None` = gone).
pub fn tile_at(uid: i32) -> Option<i32> {
    let v = unsafe { sys::tile_at(uid) };
    (v >= -1).then_some(v)
}

/// Move the instance at `uid` to `tile` (-1 = back with its owner).
pub fn set_tile_at(uid: i32, tile: i32) -> bool {
    unsafe { sys::set_tile_at(uid, tile) != 0 }
}

/// Is the instance at `uid` face-down?
pub fn is_face_down_at(uid: i32) -> bool {
    unsafe { sys::is_face_down_at(uid) != 0 }
}

/// Flip the instance at `uid`; returns whether it exists.
pub fn set_face_down_at(uid: i32, on: bool) -> bool {
    unsafe { sys::set_face_down_at(uid, on as i32) != 0 }
}

/// Is the instance at `uid` marked 「不受任何效果影响」?
pub fn is_immune_at(uid: i32) -> bool {
    unsafe { sys::is_immune_at(uid) != 0 }
}

/// Mark the instance at `uid` 「不受任何效果影响」; returns whether it exists.
pub fn set_immune_at(uid: i32, on: bool) -> bool {
    unsafe { sys::set_immune_at(uid, on as i32) != 0 }
}

/// The uid of the instance named `name` on `player_id`'s field, or `None`.
/// A *resolver*, not an identity: with several copies in play it answers the
/// first, so a rule that means "a specific copy" has to hold the uid from
/// [`place_card`] instead of asking by name.
pub fn find_card(player_id: i32, name: &str) -> Option<i32> {
    field_instances(player_id)
        .into_iter()
        .find(|(_, id)| id == name)
        .map(|(uid, _)| uid)
}
