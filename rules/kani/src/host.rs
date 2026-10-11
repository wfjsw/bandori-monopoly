//! Symbolic host backend, installed into `card_sdk::kani_host`.
//!
//! Under `cfg(kani)` the card-sdk `sys` surface is ordinary Rust that
//! funnels every `bandori_*` import through [`card_sdk::kani_host::call`].
//! This module is that backend: a small model of the world (see
//! [`crate::world`]) plus an effect log. Game vocabulary lives here, not in
//! card-sdk.
//!
//! Name lookups are **byte compares** against compile-time constants. We
//! deliberately do not build `String`s: `str::from_utf8_lossy`'s chunk loop
//! does not unwind under CBMC and was the pilot's first scaling cliff.

use card_sdk::kani_host::{self, Args, Op};

use crate::world::{
    self, Effect, World, CARD_RESONANCE, TILE_SHOP, TILE_SHOP_NAME, TOK_PP_NEG, TOK_PP_POS,
};

/// Call this at the top of every proof.
pub fn install() {
    kani_host::install(dispatch);
}

/// Byte-equal to `expected`. Manual fixed-bound compare -- `==` on slices
/// lowers to `memcmp`, whose CBMC unwinding assertion is tight.
fn guest_eq(handle: i32, len: i32, expected: &str) -> bool {
    let b = card_sdk::native::read(handle, len.max(0) as usize);
    let e = expected.as_bytes();
    if b.len() != e.len() || b.len() > 64 {
        return false;
    }
    let mut i = 0usize;
    let mut ok = true;
    while i < 64 {
        if i < b.len() && b[i] != e[i] {
            ok = false;
        }
        i += 1;
    }
    ok
}

fn card_id_of(handle: i32, len: i32) -> Option<u8> {
    if guest_eq(handle, len, world::CARD_AYA) {
        Some(0)
    } else if guest_eq(handle, len, world::CARD_RESONANCE) {
        Some(1)
    } else if guest_eq(handle, len, world::CARD_OTHER) {
        Some(2)
    } else if guest_eq(handle, len, world::CARD_EXTRA) {
        Some(3)
    } else {
        None
    }
}

/// The one dispatch entry. Args are the ABI's six i32 slots, in declaration order.
pub fn dispatch(op: Op, a: Args) -> i64 {
    let [a0, a1, a2, a3, a4, a5] = a;
    match op {
        Op::roll => (a1 * (a2.max(1) / 2)) as i64,
        Op::log | Op::effect => {
            let _ = (a0, a1, a2, a3, a4, a5);
            world::with_world_mut(|w| w.push_narration(a0, 0));
            0
        }
        Op::tile_count => world::TILE_COUNT as i64,
        Op::tile_named => {
            if guest_eq(a0, a1, TILE_SHOP_NAME) && world::with_world(|w| w.tile_shop_present) {
                TILE_SHOP as i64
            } else {
                -1
            }
        }
        Op::player_pos => {
            world::with_world(|w| w.pos.get(a0 as usize).copied().unwrap_or(-1)) as i64
        }
        Op::fire => world::with_world(|w| w.fire.get(a0 as usize).copied().unwrap_or(0)) as i64,
        Op::teleport_to => {
            world::with_world_mut(|w| {
                if (a0 as usize) < world::MAX_PLAYERS {
                    w.pos[a0 as usize] = a1;
                }
                w.push(Effect::Teleport { player: a0, tile: a1 });
            });
            0
        }
        Op::player_count => world::with_world(|w| w.n_players) as i64,
        Op::player_out => world::with_world(|w| {
            if w.out.get(a0 as usize).copied().unwrap_or(true) {
                1
            } else {
                0
            }
        }) as i64,
        Op::money => world::with_world(|w| w.money.get(a0 as usize).copied().unwrap_or(0)) as i64,
        Op::count_held_name => {
            let is_pos = guest_eq(a0, a1, TOK_PP_POS);
            let is_neg = guest_eq(a0, a1, TOK_PP_NEG);
            world::with_world(|w| {
                let p = a2 as usize;
                if p >= world::MAX_PLAYERS {
                    return 0;
                }
                if is_pos {
                    w.fans_pos[p]
                } else if is_neg {
                    w.fans_neg[p]
                } else {
                    0
                }
            }) as i64
        }
        Op::set_held_name => {
            let is_pos = guest_eq(a0, a1, TOK_PP_POS);
            let is_neg = guest_eq(a0, a1, TOK_PP_NEG);
            world::with_world_mut(|w| {
                let p = a2 as usize;
                if p >= world::MAX_PLAYERS {
                    return;
                }
                if is_pos {
                    w.fans_pos[p] = a3;
                } else if is_neg {
                    w.fans_neg[p] = a3;
                }
            });
            0
        }
        Op::hand_count => {
            let is_res = guest_eq(a0, a1, CARD_RESONANCE);
            world::with_world(|w| {
                if is_res {
                    w.hand_resonance.get(a2 as usize).copied().unwrap_or(0)
                } else {
                    0
                }
            }) as i64
        }
        Op::hand_size => world::with_world(|w| {
            let p = a0 as usize;
            if p >= world::MAX_PLAYERS {
                return 0;
            }
            let mut n = w.hand_resonance[p];
            for i in 0..world::MAX_CARDS {
                if w.hand[i] && i != 1 {
                    n += 1;
                }
            }
            n
        }) as i64,
        Op::add_to_hand => {
            if let Some(id) = card_id_of(a1, a2) {
                world::with_world_mut(|w| {
                    w.hand[id as usize] = true;
                    w.push(Effect::AddToHand { player: a0, card: id });
                });
            }
            0
        }
        Op::to_discard => {
            if let Some(id) = card_id_of(a1, a2) {
                world::with_world_mut(|w| w.discard[id as usize] = true);
            }
            0
        }
        Op::take_card => {
            let id = match card_id_of(a2, a3) {
                Some(id) => id,
                None => return 0,
            };
            let is_res = guest_eq(a2, a3, CARD_RESONANCE);
            let player_id = a0;
            let pile = a1;
            world::with_world_mut(|w| {
                let slot = id as usize;
                if pile == 1 {
                    if w.discard[slot] {
                        w.discard[slot] = false;
                        w.push(Effect::TakeFromPile { player: player_id, pile, card: id });
                        1
                    } else {
                        0
                    }
                } else if pile == 0 {
                    if is_res {
                        if w.hand_resonance.get(player_id as usize).copied().unwrap_or(0) > 0 {
                            w.hand_resonance[player_id as usize] -= 1;
                            w.push(Effect::TakeFromPile {
                                player: player_id,
                                pile,
                                card: id,
                            });
                            1
                        } else {
                            0
                        }
                    } else if w.hand[slot] {
                        w.hand[slot] = false;
                        w.push(Effect::TakeFromPile { player: player_id, pile, card: id });
                        1
                    } else {
                        0
                    }
                } else {
                    0
                }
            })
        }
        Op::cards_in => {
            // Bounded postcard `Vec<String>` of known ids. Avoid `format!`
            // and UTF-8 scans: copy the constant bytes.
            let mut ids: [u8; world::MAX_CARDS] = [0; world::MAX_CARDS];
            let mut n = 0usize;
            world::with_world(|w| {
                if a1 == 1 {
                    for i in 0..world::MAX_CARDS {
                        if w.discard[i] && n < world::MAX_CARDS {
                            ids[n] = i as u8;
                            n += 1;
                        }
                    }
                } else if a1 == 0 {
                    let p = a0 as usize;
                    let res = w.hand_resonance.get(p).copied().unwrap_or(0);
                    for _ in 0..res.min(2) {
                        if n < world::MAX_CARDS {
                            ids[n] = 1;
                            n += 1;
                        }
                    }
                    for i in 0..world::MAX_CARDS {
                        if w.hand[i] && i != 1 && n < world::MAX_CARDS {
                            ids[n] = i as u8;
                            n += 1;
                        }
                    }
                }
            });
            // Simple layout ` [n, id0, id1, ...] ` -- not postcard. The
            // guest side (`ctx::cards_in` under `cfg(kani)`) rebuilds names
            // from these ids. postcard+String does not unwind under CBMC.
            let mut encoded = [0u8; 8];
            encoded[0] = n as u8;
            for i in 0..n {
                encoded[1 + i] = ids[i];
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
        Op::place_card_at => {
            let id = card_id_of(a1, a2).unwrap_or(2);
            world::with_world_mut(|w| {
                let dest = w.dest;
                w.push(Effect::PlaceCard { player: a0, dest, card: id });
                w.last_uid += 1;
                w.last_uid as i64
            })
        }
        Op::set_dest => {
            world::with_world_mut(|w| w.dest = a0);
            0
        }
        Op::gain_fire => {
            if a1 <= 0 {
                return 0;
            }
            world::with_world_mut(|w| {
                if (a0 as usize) < world::MAX_PLAYERS {
                    w.fire[a0 as usize] = w.fire[a0 as usize].saturating_add(a1);
                }
                w.push(Effect::GainFire { player: a0, n: a1 });
            });
            a1 as i64
        }
        Op::give_stay => {
            world::with_world_mut(|w| {
                if (a0 as usize) < world::MAX_PLAYERS {
                    w.stay[a0 as usize] = w.stay[a0 as usize].saturating_add(a1);
                }
                w.push(Effect::GiveStay { player: a0, n: a1 });
            });
            0
        }
        Op::give_stun => dispatch(Op::give_stay, a),
        Op::opt_int | Op::opt_str | Op::opt_tile => {
            world::with_world_mut(|w| w.opt_n += 1);
            0
        }
        Op::opt_price | Op::opt_ai => 0,
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
        Op::trig_kind => 0,
        Op::trig_player => world::with_world(|w| w.trig_player) as i64,
        Op::trig_value => world::with_world(|w| w.pay_amount) as i64,
        Op::trig_target => -1,
        Op::trig_tile => -1,
        Op::trig_name => 0,
        Op::trig_step => 0,
        Op::trig_set_pay_amount => {
            world::with_world_mut(|w| {
                w.pay_amount = a0;
                w.push(Effect::SetPayAmount { to: a0 });
            });
            0
        }
        other => panic!("unmodelled host import: {:?}", other),
    }
}

#[allow(dead_code)]
fn _w(_w: World) {}
