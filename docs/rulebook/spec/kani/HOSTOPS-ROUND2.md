# HOSTOPS round 2 — invoke index + host-model defect batch

Branch `kani-hostops2` at `/home/monopoly/kani-hostops2-wt`.
Agent: HOST-OPS round 2. Coordinator merges.

---

## Task A — kill the `invoke::find` cliff

### What landed

| piece | path |
|---|---|
| generator | `tools/kani/gen_card_ids.py` (`--check` for drift) |
| generated table | `rules/kani/src/generated/card_ids.rs` (285 ids, `RULESET` flatten order) |
| macro | `card_id!("…")` — compile-time literal → `usize`, `compile_error!` on unknown (same contract as `guard_id!`) |
| indexed API | `invoke::{by_idx, play_idx, entry_idx, hook_idx, hook_only_idx, counteract_idx, skill_play_idx, event_play_idx}` |
| migration | `tools/kani/migrate_invoke_idx.py` (sed-able; re-run after new harnesses) |
| proofs | all `rules/kani/src/proofs/*.rs` string-keyed `invoke::` sites rewritten (361 + 28 new HHW) |

`find` / `play` / `entry` / … still exist for native tests and walk `RULESET`
with `str ==`. **Do not use them in `#[kani::proof]` bodies.**

`by_idx` walks `RULESET` with integer arithmetic only (17 bands, no memcmp).
Compatibility aliases (`aya_longing_*`, `tae_sound_play`, `rinne_rain_play`)
go through `card_id!`.

Master's temporary hack (`find` always returned `AYA_LONGING`) is **removed**;
the const-index table is the general solution.

### Representative harnesses (runner `--jobs 1`, unwinding assertions on)

| harness | before | after | covers | notes |
|---|---|---|---|---|
| `proofs::ppp::wait_aoki_01_place_on_field_iff` | TIMEOUT / vacuous | **SUCCESSFUL** 195 s (283 s wall) | 1/1 | real verify, not vacuous |
| `proofs::mor::rainbow_01_half_iff` | TIMEOUT / vacuous | **FAILED** 194 s | 3/3 | cliff gone; spec wants `amount/2` but card is `CeilTo(amount/2, 10)` (book 「减半」 + C# CeilTo). Unit-spec gap, not a host gap |
| `proofs::ras::riot_02_03_cycle_iff` | TIMEOUT | **TIMEOUT** 600 s | — | no longer `invoke::find`; now `Vec<u8>` `drop_glue` unwinding inside card bodies (String/Vec cliff — see follow-up) |

Migration note for unit authors: after adding a harness call, run

```sh
python3 tools/kani/migrate_invoke_idx.py rules/kani/src/proofs/<unit>.rs
# then prefix bare card_id! → crate::card_id! (or `use crate::card_id;`)
python3 tools/kani/gen_card_ids.py --check   # after adding cards
```

### Follow-up (coordinator finding, not done here)

`Vec<String>::contains` / `String::eq` memcmp in unmodified card bodies
(e.g. `aya_longing` non-empty-discard path) still unrolls 30+ and times out.
Needs a String-identity facade under `cfg(kani)` (extend the 1-byte
`cards_in` facade so `String` eq is a `u8` compare). That is the remaining
all-picks / riot cliff.

---

## Task B — host-model defects

Engine ground fact: `crates/game-rules/src/hostfns.rs` +
`crates/game-core/src/engine/ops.rs` (+ `wasm_rules` `WorldOps`).
Each row has a conformance test in `rules/kani/tests/conf_hostops2.rs`
(12 tests, all green) and/or `conf_model.rs` (37 tests, still green).

### Per-op before → after

| op | before | after (modeled) | still PROTOCOL / gap | unblocks (examples) |
|---|---|---|---|---|
| `state_set` | wrote field id `a3` as the value; dropped `a4` | `a3`=field `a4`=value; field 0 VALUE + `SetState`; 1 MAX / 2 MIN / 3 EXPIRES | decay of EXPIRES is protocol | sumimi twoinone fire; rio/masking caps |
| `state_get` | ignored field | field 1/2/3 read MAX/MIN/EXPIRES | | |
| `play_doubled` / `ctx::n` | stub 0 → `n(0,v)` doubled | default `-1` (engine `None`); `set_play_doubled` writes | | any `ctx::n(0,…)` clause |
| `trig_set_cancelled` / `trig_set_negate_effect` | read `a0` (always 0 from sys_kani) | the call **is** the effect (sets true) | | mor band_02 cancel leg |
| `tile_named` | only 「江户川乐器店」 | every `board.json` name (`board::TILE_NAMES` / `tile_named_bytes`) | pilot board keeps TILE_SHOP=3 when all `pos<8` | sumimi mana; ras livehouse; events lulu/school/asukayama/never_ends; ag childhood/detour |
| `neighbor` | tile `pos±1` | living-seat neighbour in turn order, wrap, skip `out` | | mor courage / NNW-style seat picks |
| `give_stun` / `stun_of` | shared `stay` slot | `stun[]` separate (`key::STUN`) | | stun≠stay proofs |
| `give_exile` / `exile_of` | log only / stub 0 | `exile[]` + `exile_to[]` (`key::EXILE`/`EXILE_TO`) | layer decay = protocol | embers exile; kizuna |
| `give_extra_turn` | silent 0 | records `extra_turns[]` + `Effect::GiveExtraTurn` | **scheduling is PROTOCOL** | mujica sparkler (partial) |
| `add_counter_self` / `add_counter_at` / `add_card_counter` | ignored `max`; player-key only | `(was+n).max(0)` then `min(max)` if `max>0`; uid-addressed; crystals → `FieldCard.crystals` | `set_counter` effect-log row still thin | pp eve cap-3; ras storm; pareo cap-3 |
| `counter_at` | ignored uid | reads `uid` instance | | mujica crystal-move |
| `gain_fire` | no cap | `n.clamp(0, fire_max-was)`, 0 if out/cap≤0 | | touko/nanami at-cap; rio/masking caps |
| `shuffle_into_deck` | stub 0 | multiset move hand/discard → draw | shuffle order symbolic | nnm draw; general reshuffle |
| `set_self_prop` | no-op | writes running-instance prop | | turn counters on instance |
| `is_shop` | `kind==5` (edogawa only) | `is_buyable && group==10` (property\|ring) | | ag shop-street predicates |
| `place_card_on` | name from `(a1,a2)=(tile,cp)` | ABI `[player,tile,cp,cl,ptr,len]`; name at `(a2,a3)`; lands on `tile` | | hhw kkr-02 CiRCLE |
| `set_slot` | (unchanged) | still `a3`=value (its ABI has no field col) | | |

### Still stub / PROTOCOL-LEVEL (loud, never silent-zero-as-success)

| surface | status |
|---|---|
| move-plan setters (`set_steps`, `set_base_dice`, `set_teleport_to`, …) | PROTOCOL (scheduling) |
| `schedule_turn_end`, `linger`, `gate` | PROTOCOL |
| `turn_start_pos`, `turn_main_steps`, `turn_rolls`, `turn_snap` | stubs (need turn-snap fixture) |
| `trig_move_*` getters that need a live move payload | partially in `MoveSnap`; several still unmodeled |
| `trig_card_is`, `trig_cards`, `trig_by_card`, `trig_effect_*` | stubs |
| `tok_names`, `placed_cards`, `cards_in(Hand)` per-player, `field_instances` buffer | partial / count-only |
| `card_text_mentions`, `play_from_hand` | stubs |
| `card_mortgage` | stub (no `SetMortgaged` effect yet) |
| `remove_marks_f`, `bump_mark_f`, `mark_src_at` | stubs |
| `pay_total` | shapes + transfers in model; real pipeline is request/pause (PROTOCOL) |
| `msg_tile` / `msg_a` / `msg_sender_uid` | stubs |
| `ask_yes` with `opt_n==0` | still stuck-on-yes (see conf_model) |
| `is_color` | stub |
| `rent_of` / price tables with house override | OPEN-收费标价 (not this batch) |

### Conformance

* `rules/kani/tests/conf_hostops2.rs` — **12/12 green** (state_set packing,
  play_doubled, trig_set no-arg, tile_named board, neighbor seats,
  stay/stun/exile, counter max, gain_fire clamp, is_shop, shuffle,
  place_card_on, card_ids↔RULESET).
* `rules/kani/tests/conf_model.rs` — **37/37 green** (unchanged suite).
* `cargo test -p rules-kani` **3× deterministic** (all green).
* `cargo test -p rules-cond --features lower` green.
* `cargo check --manifest-path rules/Cargo.toml --workspace --target wasm32-unknown-unknown` green.
* `node tools/build-ruleset.mjs` without `CARGO_TARGET_DIR` — 285 cards,
  411940 bytes. card-sdk was **not** edited by this branch (master's
  `cards_in` 1-byte facade rode in on the merge); wasm path unchanged here.

---

## Task C — coverage notes

`KANI-COVERAGE.md` summary deltas from this batch (full table still the
unit agents' live view; re-run unit tables against the new host):

* `tile_named` rows: was `blocked-by {tile_named}` → **unblocked** for every
  board name (still OPEN where the clause also needs `rent_of` / livehouse
  ownership / move-plan).
* `state_set` packing rows → **unblocked**.
* `add_counter` max / `gain_fire` cap rows → **unblocked** (cap legs now
  modelled; at-cap no-op is assertable).
* `play_doubled` / `ctx::n(0,·)` rows → **unblocked**.
* `trig_set_cancelled` / `trig_set_negate_effect` rows → **unblocked**.
* `is_shop` / shop-street rows → **unblocked** (predicate fixed).
* `neighbor` seat rows → **unblocked**.
* stay/stun/exile split rows → **unblocked**.
* `place_card_on` tile rows (HHW kkr-02) → **unblocked**.
* Still blocked: move-plan / `trig_move_*` scheduling, `rent_of` tables,
  `tok_names` / `placed_cards` name lists, `card_text_mentions`,
  `card_mortgage`, marks remove/bump, `pay_total` pipeline, String/Vec
  memcmp cliffs in card bodies.

---

## KANI-FOUNDATION.md — modeled vs protocol (this batch)

Add to §2 “Modeled vs unmodeled”:

* **Indexed invoke** — proofs call `play_idx(card_id!("…"), p)`. `find` is
  native-only.
* `state_set` field column (0/1/2/3 = VALUE/MAX/MIN/EXPIRES) is modeled.
* `play_doubled` default is `-1` (no doubling).
* No-arg `trig_set_*` setters are the effect (ignore `[0;6]` payload).
* `tile_named` covers all 60 `board.json` names.
* `neighbor` is a seat, not a tile.
* stay / stun / exile are three fields; `give_extra_turn` only **records**.
* `add_counter*` / `gain_fire` honour `max` / `fire_max`.
* `is_shop` = buyable ∧ `group==10`.
* `shuffle_into_deck` is a multiset pile move (order symbolic).
* `place_card_on` ABI is `[player, tile, cp, cl, ptr, len]`.