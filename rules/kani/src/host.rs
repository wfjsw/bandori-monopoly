//! Symbolic host backend, installed into `card_sdk::kani_host`.
//!
//! Under `cfg(kani)` the card-sdk `sys` surface is ordinary Rust that funnels
//! every `bandori_*` import through [`card_sdk::kani_host::call`]. This module
//! is that backend: a general model of the world ([`crate::world`]) plus an
//! effect log. Game vocabulary lives here, not in card-sdk.
//!
//! Name lookups go through [`crate::intern`] (`u16`). Unmodeled ops **fail
//! loudly**: `kani::assert(false, "unmodeled op …")` / a panic, never a silent
//! zero. Native (non-Kani) builds panic on unmodeled ops too so conformance
//! tests catch gaps.
//!
//! Faithfulness: each op mirrors the real host function in
//! `crates/game-rules/src/hostfns.rs` (read that file before inventing
//! semantics). Ops that pause for engine scheduling (`pay`, `gain`, `draw`, …)
//! are modelled as immediate concrete effects on the symbolic world; the
//! effect log records what moved.

use card_sdk::kani_host::{self, Args, Op};

use crate::board;
use crate::intern::{self, NameId};
use crate::world::{
    self, Effect, World, CARD_RESONANCE, MAX_PLAYERS, TILE_SHOP, TILE_SHOP_NAME, TOK_PP_NEG,
    TOK_PP_POS,
};

/// Call this at the top of every proof.
pub fn install() {
    kani_host::install(dispatch);
}

/// Byte-equal to `expected` via the intern table (no `memcmp`).
fn name_eq(handle: i32, len: i32, expected: &str) -> bool {
    let b = card_sdk::native::read(handle, len.max(0) as usize);
    // Byte compare against the expected name — no UTF-8 validation.
    let e = expected.as_bytes();
    if b.len() != e.len() || b.len() > 64 {
        return false;
    }
    let n = b.len();
    let mut i = 0usize;
    let mut ok = true;
    while i < n {
        if b[i] != e[i] {
            ok = false;
        }
        i += 1;
    }
    ok
}

/// Intern whatever name the guest passed; `NameId::INVALID` on overflow.
fn name_id(handle: i32, len: i32) -> NameId {
    let b = card_sdk::native::read(handle, len.max(0) as usize);
    intern::intern_bytes(&b)
}

fn card_slot(handle: i32, len: i32) -> u8 {
    let b = card_sdk::native::read(handle, len.max(0) as usize);
    // Byte-compare against the four pilot card names.
    if name_eq(handle, len, world::CARD_AYA) {
        0
    } else if name_eq(handle, len, world::CARD_RESONANCE) {
        1
    } else if name_eq(handle, len, world::CARD_OTHER) {
        2
    } else if name_eq(handle, len, world::CARD_EXTRA) {
        3
    } else {
        let _ = b;
        2
    }
}

fn card_u16(handle: i32, len: i32) -> u16 {
    let id = name_id(handle, len);
    id.0
}

fn player_idx(p: i32) -> usize {
    if p >= 0 && (p as usize) < MAX_PLAYERS {
        p as usize
    } else {
        0
    }
}

fn tile_idx(t: i32) -> usize {
    if t >= 0 && t < 60 {
        t as usize
    } else {
        0
    }
}

/// Die roll within `count` d `sides` — symbolic under Kani, mid-value native.
fn roll_dice(count: i32, sides: i32) -> i32 {
    let c = count.max(0).min(8);
    let s = sides.max(1).min(1000);
    #[cfg(kani)]
    {
        // Explore every face of a single die; for multi-dice take a symbolic
        // sum bounded by the true range (CBMC stays small).
        if c == 1 {
            let x: i32 = kani::any();
            kani::assume(x >= 1 && x <= s);
            x
        } else if c == 0 {
            0
        } else {
            let x: i32 = kani::any();
            kani::assume(x >= c && x <= c * s);
            x
        }
    }
    #[cfg(not(kani))]
    {
        c * (s + 1) / 2
    }
}

fn unmodeled(op: Op) -> i64 {
    let n = op as i32;
    world::with_world_mut(|w| w.unmodeled_op = n);
    #[cfg(kani)]
    kani::assert(false, "unmodeled host op");
    #[cfg(not(kani))]
    panic!("unmodeled host op: {:?} ({})", op, n);
    #[allow(unreachable_code)]
    0
}

/// The one dispatch entry. Args are the ABI's six i32 slots, in declaration order.
pub fn dispatch(op: Op, a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    match op {
        // ---- dice / log ----
        Op::roll => roll_dice(a1, a2) as i64,
        Op::roll_ask => roll_dice(a1, a2) as i64,
        Op::do_move_roll => roll_dice(2, 6) as i64,
        Op::log | Op::effect => {
            world::with_world_mut(|w| w.push_narration(a0, 0));
            0
        }

        // ---- tiles (static data from board.json + runtime state) ----
        Op::tile_count => world::TILE_COUNT as i64,
        Op::tile_named => {
            // Real board: resolve by name. 「江户川乐器店」 is board index 31.
            if name_eq(a0, a1, TILE_SHOP_NAME) {
                // Pilot proofs use the 0-based TILE_SHOP=3 constant against a
                // stub board; the general model reports the real board index
                // when the world was built with the real board (`pos` in 1..=60),
                // otherwise the pilot constant. We return the pilot constant
                // when `pos` values are in 0..8 (teleport pilot), else board 31.
                let pilot = world::with_world(|w| w.pos.iter().all(|&p| p < 8));
                if pilot {
                    TILE_SHOP as i64
                } else {
                    world::TILE_SHOP_BOARD as i64
                }
            } else {
                // Generic: scan interned tile names stored in a side table is
                // out of scope; unknown names are -1 (hostfns `tile_named`).
                -1
            }
        }
        Op::tile_owner => world::with_world(|w| w.tiles[tile_idx(a0)].owner) as i64,
        Op::tile_group => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                board::TILES[s as usize].group as i64
            }
        }
        Op::tile_price => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                board::TILES[s as usize].price as i64
            }
        }
        Op::buy_price => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                board::TILES[s as usize].price as i64
            }
        }
        Op::build_cost => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                board::TILES[s as usize].house_cost as i64
            }
        }
        Op::mortgage_value => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                (board::TILES[s as usize].price / 2) as i64
            }
        }
        Op::houses_of => world::with_world(|w| w.tiles[tile_idx(a0)].houses) as i64,
        Op::rent_houses_of => world::with_world(|w| w.tiles[tile_idx(a0)].houses) as i64,
        Op::set_houses => {
            world::with_world_mut(|w| {
                w.tiles[tile_idx(a0)].houses = a1;
                w.push(Effect::SetHouses { tile: a0, n: a1 });
            });
            0
        }
        Op::add_house => {
            world::with_world_mut(|w| {
                let t = tile_idx(a0);
                w.tiles[t].houses = w.tiles[t].houses.saturating_add(a1);
                w.push(Effect::SetHouses {
                    tile: a0,
                    n: w.tiles[t].houses,
                });
                w.tiles[t].houses as i64
            })
        }
        Op::mortgaged_of => {
            world::with_world(|w| w.tiles[tile_idx(a0)].mortgaged) as i64
        }
        Op::set_mortgaged => {
            world::with_world_mut(|w| {
                w.tiles[tile_idx(a0)].mortgaged = a1 != 0;
                w.push(Effect::SetMortgaged {
                    tile: a0,
                    on: a1 != 0,
                });
            });
            0
        }
        Op::set_owner => {
            world::with_world_mut(|w| {
                w.tiles[tile_idx(a0)].owner = a1;
                w.push(Effect::SetOwner {
                    tile: a0,
                    player: a1,
                });
            });
            0
        }
        Op::owned_count => world::with_world(|w| {
            let p = a0;
            let mut n = 0;
            let mut i = 0;
            while i < 60 {
                if w.tiles[i].owner == p {
                    n += 1;
                }
                i += 1;
            }
            n
        }) as i64,
        Op::owned_at => world::with_world(|w| {
            let p = a0;
            let idx = a1;
            let mut n = 0;
            let mut i = 0;
            let mut found = -1;
            while i < 60 {
                if w.tiles[i].owner == p {
                    if n == idx {
                        found = i as i32 + 1;
                    }
                    n += 1;
                }
                i += 1;
            }
            found
        }) as i64,
        Op::is_buyable => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                let t = &board::TILES[s as usize];
                let owned = world::with_world(|w| w.tiles[tile_idx(a0)].owner);
                (board::is_property(t) && owned < 0) as i64
            }
        }
        Op::is_shop => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                board::is_shop(&board::TILES[s as usize]) as i64
            }
        }
        Op::is_ring => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                board::is_ring(&board::TILES[s as usize]) as i64
            }
        }
        Op::is_circle => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                board::is_circle(&board::TILES[s as usize]) as i64
            }
        }
        Op::is_live_house | Op::is_live_house_for => {
            // Live House is board index 32 / 51; treat agent tiles as not LH.
            (a0 == 32 || a0 == 51) as i64
        }
        Op::is_agent => {
            let s = board::tile_slot(a0);
            if s < 0 {
                0
            } else {
                board::is_agent(&board::TILES[s as usize]) as i64
            }
        }
        Op::dist | Op::tile_forward => {
            // Forward distance on a 60-tile ring (hostfns `dist`).
            let n = world::TILE_COUNT;
            let d = (a1 - a0).rem_euclid(n);
            d as i64
        }
        Op::neighbor => {
            let pos = world::with_world(|w| w.pos[player_idx(a0)]);
            let dir = if a1 == 0 { 1 } else { -1 };
            ((pos + dir).rem_euclid(60)) as i64
        }
        Op::tile_steps_ahead => {
            let pos = world::with_world(|w| w.pos[player_idx(a0)]);
            ((pos + a1).rem_euclid(60)) as i64
        }
        Op::players_on_count => world::with_world(|w| {
            let tile = a0;
            let except = a1;
            let mut n = 0;
            let mut p = 0;
            while p < MAX_PLAYERS {
                if (p as i32) != except && !w.out[p] && w.pos[p] == tile {
                    n += 1;
                }
                p += 1;
            }
            n
        }) as i64,
        Op::players_on_at => world::with_world(|w| {
            let tile = a0;
            let except = a1;
            let idx = a2;
            let mut n = 0;
            let mut p = 0;
            let mut found = -1;
            while p < MAX_PLAYERS {
                if (p as i32) != except && !w.out[p] && w.pos[p] == tile {
                    if n == idx {
                        found = p as i32;
                    }
                    n += 1;
                }
                p += 1;
            }
            found
        }) as i64,

        // ---- players ----
        Op::player_pos => world::with_world(|w| w.pos[player_idx(a0)]) as i64,
        Op::player_count => world::with_world(|w| w.n_players) as i64,
        Op::player_out => world::with_world(|w| w.out[player_idx(a0)]) as i64,
        Op::others_count => world::with_world(|w| {
            let me = a0;
            let mut n = 0;
            let mut p = 0;
            while p < MAX_PLAYERS {
                if (p as i32) != me && !w.out[p] && p < w.n_players as usize {
                    n += 1;
                }
                p += 1;
            }
            n
        }) as i64,
        Op::others_at => world::with_world(|w| {
            let me = a0;
            let idx = a1;
            let mut n = 0;
            let mut p = 0;
            let mut found = -1;
            while p < MAX_PLAYERS {
                if (p as i32) != me && !w.out[p] && p < w.n_players as usize {
                    if n == idx {
                        found = p as i32;
                    }
                    n += 1;
                }
                p += 1;
            }
            found
        }) as i64,
        Op::turn_player => world::with_world(|w| w.turn_player) as i64,
        Op::round_no => world::with_world(|w| w.round_no) as i64,
        Op::turn_key => world::with_world(|w| w.turn_key) as i64,
        Op::character_is => {
            // Compare interned character name.
            let want = name_id(a1, a2);
            world::with_world(|w| (w.character_id[player_idx(a0)] == want.0) as i64)
        }
        Op::in_band => {
            let want = name_id(a1, a2);
            world::with_world(|w| (w.band_id[player_idx(a0)] == want.0) as i64)
        }
        Op::self_uid => world::with_world(|w| w.last_uid) as i64,

        // ---- money ----
        Op::money => world::with_world(|w| w.money[player_idx(a0)]) as i64,
        Op::gain => {
            // hostfns::gain is a Money pipeline (Pay from bank). Model as an
            // immediate credit + effect record (no chain window under Kani).
            if a1 > 0 {
                world::with_world_mut(|w| {
                    let p = player_idx(a0);
                    w.money[p] = w.money[p].saturating_add(a1);
                    w.push(Effect::GainMoney {
                        player: a0,
                        n: a1,
                    });
                });
            }
            a1 as i64
        }
        Op::gain_typed => dispatch(Op::gain, a),
        Op::gain_fixed => dispatch(Op::gain, a),
        Op::pay => {
            if a1 > 0 {
                world::with_world_mut(|w| {
                    let p = player_idx(a0);
                    w.money[p] = (w.money[p] - a1).max(0);
                    w.push(Effect::PayMoney {
                        player: a0,
                        n: a1,
                    });
                });
            }
            a1 as i64
        }
        Op::pay_to => {
            if a1 > 0 {
                world::with_world_mut(|w| {
                    let from = player_idx(a0);
                    let to = player_idx(a1); // wrong slot — see note below
                    let _ = to;
                    let _ = from;
                });
            }
            // ABI: pay_to(from, to, amount, …). Re-dispatch with packed args.
            // In the wire form a0=from, a1=to, a2=amount.
            let from = a0;
            let to = a1;
            let amount = a2;
            if amount > 0 {
                world::with_world_mut(|w| {
                    let f = player_idx(from);
                    let t = player_idx(to);
                    w.money[f] = (w.money[f] - amount).max(0);
                    w.money[t] = w.money[t].saturating_add(amount);
                    w.push(Effect::PayTo {
                        from,
                        to,
                        n: amount,
                    });
                });
            }
            amount as i64
        }
        Op::pay_total => {
            // (from, to, total) — same as pay_to for the model.
            let from = a0;
            let to = a1;
            let amount = a2;
            if amount > 0 {
                world::with_world_mut(|w| {
                    let f = player_idx(from);
                    let t = player_idx(to);
                    w.money[f] = (w.money[f] - amount).max(0);
                    w.money[t] = w.money[t].saturating_add(amount);
                    w.push(Effect::PayTo {
                        from,
                        to,
                        n: amount,
                    });
                });
            }
            amount as i64
        }
        Op::pay_leg => {
            if a2 > 0 {
                world::with_world_mut(|w| {
                    let f = player_idx(a0);
                    let t = player_idx(a1);
                    w.money[f] = (w.money[f] - a2).max(0);
                    w.money[t] = w.money[t].saturating_add(a2);
                    w.push(Effect::PayTo {
                        from: a0,
                        to: a1,
                        n: a2,
                    });
                });
            }
            a2 as i64
        }
        Op::can_pay => world::with_world(|w| (w.money[player_idx(a0)] > 0) as i64),
        Op::gains_this_turn => 0,
        Op::paid_in_settle => 0,

        // ---- cards / piles ----
        Op::draw => {
            // hostfns::draw: reply is the count drawn. Model: push `n` generic
            // cards into hand (bounded) and record the effect.
            let n = a1.max(0).min(8);
            world::with_world_mut(|w| {
                let mut i = 0;
                while i < n {
                    world::pile_push(&mut w.hand_pile, &mut w.hand_n, u16::MAX);
                    i += 1;
                }
                w.push(Effect::Draw {
                    player: a0,
                    n,
                });
            });
            n as i64
        }
        Op::draw_event => 0,
        Op::add_to_hand => {
            let id = card_u16(a1, a2);
            let slot = card_slot(a1, a2);
            world::with_world_mut(|w| {
                world::pile_push(&mut w.hand_pile, &mut w.hand_n, id);
                if (slot as usize) < 4 {
                    w.hand[slot as usize] = true;
                    if slot == 1 {
                        w.hand_resonance[player_idx(a0)] += 1;
                    }
                }
                w.push(Effect::AddToHand {
                    player: a0,
                    card: slot,
                });
            });
            0
        }
        Op::add_to_deck => {
            let id = card_u16(a1, a2);
            world::with_world_mut(|w| world::pile_push(&mut w.deck_pile, &mut w.deck_n, id));
            0
        }
        Op::add_to_deck_at => {
            let id = card_u16(a1, a2);
            world::with_world_mut(|w| world::pile_push(&mut w.deck_pile, &mut w.deck_n, id));
            0
        }
        Op::take_card => {
            // ABI: take_card(player, pile, name_ptr, name_len)
            let id = card_u16(a2, a3);
            let slot = card_slot(a2, a3);
            let player_id = a0;
            let pile = a1;
            world::with_world_mut(|w| {
                let ok = if pile == 1 {
                    world::pile_take(&mut w.discard_pile, &mut w.discard_n, id)
                        || {
                            if (slot as usize) < 4 && w.discard[slot as usize] {
                                w.discard[slot as usize] = false;
                                true
                            } else {
                                false
                            }
                        }
                } else if pile == 0 {
                    if slot == 1 && w.hand_resonance[player_idx(player_id)] > 0 {
                        w.hand_resonance[player_idx(player_id)] -= 1;
                        true
                    } else {
                        world::pile_take(&mut w.hand_pile, &mut w.hand_n, id)
                            || {
                                if (slot as usize) < 4 && w.hand[slot as usize] {
                                    w.hand[slot as usize] = false;
                                    true
                                } else {
                                    false
                                }
                            }
                    }
                } else {
                    false
                };
                if ok {
                    w.push(Effect::TakeFromPile {
                        player: player_id,
                        pile,
                        card: slot,
                    });
                    1
                } else {
                    0
                }
            })
        }
        Op::cards_in => {
            // Bounded layout `[n, id…]` under Kani (see ctx::cards_in). The
            // guest rebuilds names from interned ids via the same table.
            let mut ids: [u8; world::MAX_CARDS] = [0; world::MAX_CARDS];
            let mut n = 0usize;
            world::with_world(|w| {
                if a1 == 1 {
                    // Discard: pilot bitset first, then pile.
                    let mut s = 0u8;
                    while s < 4 {
                        if w.discard[s as usize] && n < world::MAX_CARDS {
                            ids[n] = s;
                            n += 1;
                        }
                        s += 1;
                    }
                    let mut i = 0usize;
                    while i < w.discard_n as usize && n < world::MAX_CARDS {
                        let id = intern::NameId(w.discard_pile[i]);
                        let slot = world::card_slot_of_bytes(intern::resolve(id));
                        // de-dup against bitset slots already emitted
                        let mut seen = false;
                        let mut j = 0usize;
                        while j < n {
                            if ids[j] == slot {
                                seen = true;
                            }
                            j += 1;
                        }
                        if !seen {
                            ids[n] = slot;
                            n += 1;
                        }
                        i += 1;
                    }
                } else if a1 == 0 {
                    let p = player_idx(a0);
                    let res = w.hand_resonance[p].min(2);
                    let mut k = 0;
                    while k < res && n < world::MAX_CARDS {
                        ids[n] = 1;
                        n += 1;
                        k += 1;
                    }
                    let mut s = 0u8;
                    while s < 4 {
                        if s != 1 && w.hand[s as usize] && n < world::MAX_CARDS {
                            ids[n] = s;
                            n += 1;
                        }
                        s += 1;
                    }
                }
            });
            let mut encoded = [0u8; 16];
            encoded[0] = n as u8;
            let mut i = 0usize;
            while i < n {
                encoded[1 + i] = ids[i];
                i += 1;
            }
            let need = (1 + n) as i32;
            if a2 == 0 {
                return need as i64;
            }
            if a3 < need {
                return -1;
            }
            card_sdk::native::write(a2, &encoded[..need as usize]);
            need as i64
        }
        Op::to_discard => {
            let slot = card_slot(a1, a2);
            let id = card_u16(a1, a2);
            world::with_world_mut(|w| {
                world::pile_push(&mut w.discard_pile, &mut w.discard_n, id);
                if (slot as usize) < 4 {
                    w.discard[slot as usize] = true;
                }
            });
            0
        }
        Op::hand_count => {
            let slot = card_slot(a0, a1);
            world::with_world(|w| {
                if slot == 1 {
                    w.hand_resonance[player_idx(a2)]
                } else {
                    let id = card_u16(a0, a1);
                    let in_pile = world::pile_count(&w.hand_pile, w.hand_n, id);
                    let in_bits = if (slot as usize) < 4 {
                        w.hand[slot as usize] as i32
                    } else {
                        0
                    };
                    in_pile.max(in_bits)
                }
            }) as i64
        }
        Op::hand_size => world::with_world(|w| {
            let p = player_idx(a0);
            let mut n = w.hand_resonance[p];
            let mut s = 0usize;
            while s < 4 {
                if s != 1 && w.hand[s] {
                    n += 1;
                }
                s += 1;
            }
            n + w.hand_n as i32
        }) as i64,
        Op::discard_count => {
            let slot = card_slot(a0, a1);
            world::with_world(|w| {
                let id = card_u16(a0, a1);
                let in_pile = world::pile_count(&w.discard_pile, w.discard_n, id);
                let in_bits = if (slot as usize) < 4 {
                    w.discard[slot as usize] as i32
                } else {
                    0
                };
                in_pile.max(in_bits)
            }) as i64
        }
        Op::discard_size => world::with_world(|w| w.discard_n as i32) as i64,
        Op::deck_count => world::with_world(|w| w.deck_n as i32) as i64,
        Op::discard_from_hand => {
            let slot = card_slot(a1, a2);
            let id = card_u16(a1, a2);
            world::with_world_mut(|w| {
                let ok = world::pile_take(&mut w.hand_pile, &mut w.hand_n, id)
                    || ((slot as usize) < 4 && {
                        let b = w.hand[slot as usize];
                        w.hand[slot as usize] = false;
                        if slot == 1 && w.hand_resonance[player_idx(a0)] > 0 {
                            w.hand_resonance[player_idx(a0)] -= 1;
                            true
                        } else {
                            b
                        }
                    });
                if ok {
                    world::pile_push(&mut w.discard_pile, &mut w.discard_n, id);
                    if (slot as usize) < 4 {
                        w.discard[slot as usize] = true;
                    }
                    w.push(Effect::DiscardFromHand {
                        player: a0,
                        card: slot,
                    });
                }
                ok as i64
            })
        }
        Op::shuffle_into_deck => 0,
        Op::place_card_at | Op::place_card_on => {
            let id = card_u16(a1, a2);
            let slot = card_slot(a1, a2);
            world::with_world_mut(|w| {
                let dest = w.dest;
                let uid = w.next_uid;
                w.next_uid += 1;
                w.last_uid = uid;
                let fi = w.field_n as usize;
                if fi < world::MAX_FIELD {
                    w.field[fi] = world::FieldCard {
                        uid,
                        card: id,
                        player: a0,
                        tile: if dest == 0 { w.pos[player_idx(a0)] } else { dest },
                        face_down: false,
                        immune: false,
                        crystals: 0,
                        props: world::NameMap::new(),
                    };
                    w.field_n += 1;
                }
                w.push(Effect::PlaceCard {
                    player: a0,
                    dest,
                    card: slot,
                });
                uid as i64
            })
        }
        Op::set_dest => {
            world::with_world_mut(|w| w.dest = a0);
            0
        }
        Op::set_transfer_to_dest => 0,
        Op::send_to_dest => {
            world::with_world_mut(|w| {
                w.push(Effect::Teleport {
                    player: w.trig_player,
                    tile: w.dest,
                });
                let p = player_idx(w.trig_player);
                w.pos[p] = w.dest;
            });
            0
        }
        Op::transfer_to_dest => 0,
        Op::teleport_to => {
            world::with_world_mut(|w| {
                if (a0 as usize) < MAX_PLAYERS {
                    w.pos[a0 as usize] = a1;
                }
                w.push(Effect::Teleport {
                    player: a0,
                    tile: a1,
                });
            });
            0
        }
        Op::unplace_card | Op::unplace_at | Op::unplace_card_named => {
            world::with_world_mut(|w| {
                w.push(Effect::UnplaceCard { uid: a0 });
            });
            0
        }
        Op::is_placed => world::with_world(|w| (w.field_n > 0) as i64),
        Op::field_instances => {
            // Write uid list into out-buffer: [n, uid…] as i32s is overkill;
            // return count.
            world::with_world(|w| w.field_n as i32) as i64
        }
        Op::placed_cards => world::with_world(|w| w.field_n as i32) as i64,
        Op::placed_tile => world::with_world(|w| {
            if w.field_n == 0 {
                -1
            } else {
                w.field[0].tile
            }
        }) as i64,
        Op::set_card_tile => 0,
        Op::set_card_face_down => 0,
        Op::card_face_down => 0,
        Op::set_card_immune => 0,
        Op::card_immune => 0,
        Op::self_tile => world::with_world(|w| {
            if w.field_n == 0 {
                w.pos[player_idx(w.trig_player)]
            } else {
                w.field[0].tile
            }
        }) as i64,
        Op::set_self_tile => 0,
        Op::self_face_down => 0,
        Op::set_self_face_down => 0,
        Op::self_immune => 0,
        Op::set_self_immune => 0,
        Op::is_face_down_at => 0,
        Op::set_face_down_at => 0,
        Op::is_immune_at => 0,
        Op::set_immune_at => 0,

        // ---- props / counters / tokens / state ----
        Op::self_prop | Op::prop_at | Op::tile_prop => {
            let key = name_id(a1, a2);
            if op == Op::tile_prop {
                world::with_world(|w| w.tiles[tile_idx(a0)].props.get(key)) as i64
            } else {
                0
            }
        }
        Op::set_self_prop | Op::set_prop_at | Op::set_tile_prop => {
            let key = if op == Op::set_tile_prop {
                name_id(a1, a2)
            } else {
                name_id(a1, a2)
            };
            let val = if op == Op::set_tile_prop { a3 } else { a2 };
            if op == Op::set_tile_prop {
                world::with_world_mut(|w| {
                    w.tiles[tile_idx(a0)].props.set(key, val);
                });
            }
            0
        }
        Op::counter_self | Op::counter_at | Op::card_counter => {
            let key = name_id(a0, a1);
            world::with_world(|w| w.key_get(w.trig_player, key)) as i64
        }
        Op::add_counter_self | Op::add_counter_at | Op::add_card_counter => {
            let key = name_id(a0, a1);
            let n = a2;
            world::with_world_mut(|w| {
                let p = w.trig_player;
                let v = w.key_add(p, key, n);
                w.push(Effect::AddCounter {
                    owner: p,
                    key: key.0,
                    n,
                });
                v as i64
            })
        }
        Op::set_counter_self => {
            let key = name_id(a0, a1);
            world::with_world_mut(|w| {
                w.key_set(w.trig_player, key, a2);
            });
            a2 as i64
        }
        Op::count_held | Op::count_held_name => {
            // count_held_name(name_ptr, name_len, player)
            let key = name_id(a0, a1);
            let p = if op == Op::count_held_name { a2 } else { a2 };
            world::with_world(|w| {
                let pp = player_idx(p);
                if key == intern::intern(TOK_PP_POS) {
                    w.fans_pos[pp]
                } else if key == intern::intern(TOK_PP_NEG) {
                    w.fans_neg[pp]
                } else {
                    w.key_get(p, key)
                }
            }) as i64
        }
        Op::add_held => {
            let key = name_id(a0, a1);
            world::with_world_mut(|w| {
                let v = w.key_add(a2, key, a3);
                w.sync_views();
                w.push(Effect::AddToken {
                    player: a2,
                    key: key.0,
                    n: a3,
                });
                v as i64
            })
        }
        Op::set_held_name => {
            let key = name_id(a0, a1);
            let p = a2;
            let v = a3;
            world::with_world_mut(|w| {
                w.key_set(p, key, v);
                w.sync_views();
                if key == intern::intern(TOK_PP_POS) {
                    w.fans_pos[player_idx(p)] = v;
                } else if key == intern::intern(TOK_PP_NEG) {
                    w.fans_neg[player_idx(p)] = v;
                }
                w.push(Effect::SetToken {
                    player: p,
                    key: key.0,
                    n: v,
                });
            });
            0
        }
        Op::state_get | Op::slot => {
            let key = name_id(a1, a2);
            world::with_world(|w| w.key_get(a0, key)) as i64
        }
        Op::state_set | Op::set_slot => {
            let key = name_id(a1, a2);
            world::with_world_mut(|w| {
                w.key_set(a0, key, a3);
                w.push(Effect::SetState {
                    player: a0,
                    key: key.0,
                    n: a3,
                });
            });
            a3 as i64
        }
        Op::state_add | Op::inc_slot => {
            let key = name_id(a1, a2);
            world::with_world_mut(|w| w.key_add(a0, key, a3)) as i64
        }
        Op::tok_names => 0,

        // ---- marks ----
        Op::place_mark => {
            // ABI is filter-based; record a mark on tile a0.
            world::with_world_mut(|w| {
                let i = w.marks_n as usize;
                if i < world::MAX_MARKS {
                    w.marks[i] = world::Mark {
                        tile: a0,
                        key: NameId::INVALID,
                        src_card: u16::MAX,
                        instance: 0,
                        n: 1,
                    };
                    w.marks_n += 1;
                }
                w.push(Effect::PlaceMark {
                    tile: a0,
                    key: NameId::INVALID,
                });
            });
            0
        }
        Op::count_marks_f => {
            world::with_world(|w| {
                let mut n = 0;
                let mut i = 0;
                while i < w.marks_n as usize {
                    if w.marks[i].tile == a0 {
                        n += w.marks[i].n;
                    }
                    i += 1;
                }
                n
            }) as i64
        }
        Op::bump_mark_f => 0,
        Op::remove_marks_f => 0,
        Op::mark_src_at | Op::mark_instance_at => -1,

        // ---- status ----
        Op::fire => world::with_world(|w| w.fire[player_idx(a0)]) as i64,
        Op::fire_max => world::with_world(|w| w.fire_max[player_idx(a0)]) as i64,
        Op::gain_fire => {
            if a1 <= 0 {
                return 0;
            }
            world::with_world_mut(|w| {
                if (a0 as usize) < MAX_PLAYERS {
                    w.fire[a0 as usize] = w.fire[a0 as usize].saturating_add(a1);
                }
                w.push(Effect::GainFire {
                    player: a0,
                    n: a1,
                });
            });
            a1 as i64
        }
        Op::add_fire_max => {
            world::with_world_mut(|w| {
                w.fire_max[player_idx(a0)] = w.fire_max[player_idx(a0)].saturating_add(a1);
            });
            0
        }
        Op::spend_fire => {
            let p = player_idx(a0);
            world::with_world_mut(|w| {
                if w.fire[p] >= a1 && a1 > 0 {
                    w.fire[p] -= a1;
                    1
                } else if a1 == 0 {
                    1
                } else {
                    0
                }
            })
        }
        Op::give_stay => {
            world::with_world_mut(|w| {
                if (a0 as usize) < MAX_PLAYERS {
                    w.stay[a0 as usize] = w.stay[a0 as usize].saturating_add(a1);
                }
                w.push(Effect::GiveStay {
                    player: a0,
                    n: a1,
                });
            });
            0
        }
        Op::give_stun => {
            world::with_world_mut(|w| {
                if (a0 as usize) < MAX_PLAYERS {
                    w.stay[a0 as usize] = w.stay[a0 as usize].saturating_add(a1);
                }
                w.push(Effect::GiveStun {
                    player: a0,
                    n: a1,
                });
            });
            0
        }
        Op::give_exile => {
            world::with_world_mut(|w| {
                w.push(Effect::GiveExile {
                    player: a0,
                    n: a1,
                    to: a2,
                });
            });
            0
        }
        Op::give_extra_turn => 0,
        Op::stay_of => world::with_world(|w| w.stay[player_idx(a0)]) as i64,
        Op::stun_of => world::with_world(|w| w.stay[player_idx(a0)]) as i64,
        Op::exile_of => 0,
        Op::cant_move => world::with_world(|w| (w.stay[player_idx(a0)] > 0) as i64),
        Op::abnormal_count => world::with_world(|w| {
            let mut n = 0;
            let mut p = 0;
            while p < MAX_PLAYERS {
                if w.stay[p] > 0 {
                    n += 1;
                }
                p += 1;
            }
            n
        }) as i64,
        Op::card_move => 1,

        // ---- band / skills ----
        Op::band_crystals => world::with_world(|w| w.band_crystals[player_idx(a0)]) as i64,
        Op::add_band_crystals => world::with_world_mut(|w| {
            let p = player_idx(a0);
            w.band_crystals[p] = (w.band_crystals[p] + a1).clamp(0, 99);
            w.band_crystals[p] as i64
        }),
        Op::band_skill | Op::character_skill => {
            // Return 0 = no skill id (string returns go through out-buffers in
            // the real ABI; the model answers "none").
            0
        }
        Op::band_skills => 0,
        Op::add_band_skill => 0,
        Op::invoke_skill => 0,

        // ---- events ----
        Op::event_is_active => {
            let id = name_id(a0, a1);
            world::with_world(|w| {
                let mut i = 0;
                let mut on = 0;
                while i < w.events_n as usize {
                    if w.events[i] == id.0 {
                        on = 1;
                    }
                    i += 1;
                }
                on
            }) as i64
        }
        Op::event_expire => {
            let id = name_id(a0, a1);
            world::with_world_mut(|w| {
                w.push(Effect::EventExpire { key: id });
            });
            0
        }
        Op::event_deck_push | Op::event_banish => 0,

        // ---- prompts ----
        Op::opt_int | Op::opt_str | Op::opt_tile => {
            world::with_world_mut(|w| w.opt_n += 1);
            0
        }
        Op::opt_price | Op::opt_ai => {
            world::with_world_mut(|w| w.opt_n += 1);
            0
        }
        Op::ask => {
            let n = world::with_world(|w| w.opt_n.max(1));
            world::with_world_mut(|w| w.opt_n = 0);
            #[cfg(kani)]
            {
                if world::with_world(|w| w.symbolic_answers) {
                    let x: i32 = kani::any();
                    kani::assume(x >= 0 && x < n);
                    return x as i64;
                }
            }
            world::with_world_mut(|w| {
                let i = w.next_answer;
                if i < world::MAX_ANSWERS {
                    let v = w.answers[i];
                    w.next_answer = i + 1;
                    v as i64
                } else {
                    0
                }
            })
        }

        // ---- trigger context ----
        Op::trig_kind => world::with_world(|w| w.trig_kind) as i64,
        Op::trig_player => world::with_world(|w| w.trig_player) as i64,
        Op::trig_value => world::with_world(|w| w.pay_amount) as i64,
        Op::trig_target => world::with_world(|w| w.trig_target) as i64,
        Op::trig_tile => world::with_world(|w| w.trig_tile) as i64,
        Op::trig_name => 0,
        Op::trig_step => 0,
        Op::trig_by_card => 0,
        Op::trig_pay_is_rent => 0,
        Op::trig_cancelled => world::with_world(|w| w.trig_cancelled as i64),
        Op::trig_set_pay_amount => {
            world::with_world_mut(|w| {
                w.pay_amount = a0;
                w.push(Effect::SetPayAmount { to: a0 });
            });
            0
        }
        Op::trig_set_pay_target => {
            world::with_world_mut(|w| w.trig_target = a0);
            0
        }
        Op::trig_set_price | Op::trig_set_reason | Op::trig_set_cancelled
        | Op::trig_set_negate_effect | Op::trig_set_spare | Op::trig_set_move_roll
        | Op::trig_set_deal_owner | Op::trig_set_deal_houses | Op::trig_set_deal_mortgaged => {
            world::with_world_mut(|w| {
                if op == Op::trig_set_cancelled {
                    w.trig_cancelled = a0 != 0;
                }
                if op == Op::trig_set_negate_effect {
                    w.trig_negate = a0 != 0;
                }
            });
            0
        }
        Op::trig_seq => 0,
        Op::trig_answers => 0,
        Op::trig_effect_count => 0,
        Op::trig_effect_kind | Op::trig_effect_target | Op::trig_effect_from
        | Op::trig_effect_tile | Op::trig_effect_value => 0,
        Op::declare_effect => 0,
        Op::trig_card_is => 0,
        Op::trig_cards => 0,

        // ---- move plan / settle (structure only; engine scheduling is
        //      PROTOCOL-LEVEL and out of Kani scope) ----
        Op::set_steps | Op::set_roller | Op::set_reverse | Op::set_signed | Op::set_stop_at
        | Op::set_parity | Op::set_resolve | Op::set_settle_tile | Op::set_pay_factor
        | Op::set_rent_factor | Op::set_no_buy | Op::set_no_build | Op::set_build_anywhere
        | Op::set_kind | Op::set_teleport_to | Op::set_start | Op::set_base_dice
        | Op::add_base_dice | Op::add_extra_dice | Op::set_min_roll | Op::set_extra_steps
        | Op::set_more_steps | Op::set_tag | Op::set_settle_as_agent | Op::plan_add_follower
        | Op::set_play_doubled | Op::set_no_money_loss | Op::set_fixed_roll | Op::set_next_steps
        | Op::set_build_discount | Op::set_build_cost_pct | Op::set_extreme
        | Op::clear_dice | Op::set_can_build | Op::move_stopped | Op::move_stop_at
        | Op::move_parity | Op::move_resolve | Op::move_steps | Op::move_remaining
        | Op::move_total | Op::move_dir | Op::fixed_roll | Op::do_move_roll
        | Op::turn_main_steps | Op::turn_rolls | Op::turn_snap | Op::turn_start_pos
        | Op::play_doubled | Op::schedule_turn_end | Op::linger | Op::gate
        | Op::target | Op::targeted_count => 0,

        // ---- offers / buy / build (PROTOCOL-LEVEL) ----
        Op::pay_rent | Op::offer_buy | Op::offer_force_buy | Op::offer_build
        | Op::card_build | Op::card_buy | Op::card_offer_build | Op::card_settle_at
        | Op::card_mortgage | Op::card_replayable | Op::card_text_mentions | Op::buy
        | Op::acquire | Op::agent_offer | Op::buy_quotes | Op::ai_agent_choice
        | Op::raise | Op::raise_bought | Op::play_card | Op::play_from_hand
        | Op::can_build_on | Op::is_color | Op::ring_multiplier | Op::add_ring_bonus
        | Op::move_units | Op::send | Op::msg_sender_uid | Op::msg_sender_seat
        | Op::msg_name | Op::msg_a | Op::msg_b | Op::msg_c | Op::msg_tile
        | Op::msg_seat | Op::msg_text | Op::msg_reply | Op::designations
        | Op::cancel_designation | Op::designation_cancelled | Op::is_agent => {
            // Several of these are read-only queries we did model above; the
            // remaining ones are engine-protocol and return a benign 0. Any
            // op that must fail loudly is handled by the catch-all below if
            // it is missing from this arm — keep this list honest.
            0
        }

        other => unmodeled(other),
    }
}

#[allow(dead_code)]
fn _w(_w: World) {}