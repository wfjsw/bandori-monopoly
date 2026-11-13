# ABI design ABI-13 — temporary tile-rule swap

**Status:** implemented on the ABI-13 branch. Guest sugar only — the mechanism is
a pair of instance props resolved by the engine at every rule read, so the ABI
surface (`ABI_VERSION`) and the save shape (`SAVE_VERSION`) do not change.
**Worktree:** `.claude/worktrees/wf_2376cbc1-b6e-3` (branch
`worktree-wf_2376cbc1-b6e-3`).
**Ground truth:** the sheet text. Every quoted fragment below is verbatim.

Two cards write the same shape:

* 「协助CiRCLE重建」 (event A8, `rules/events/src/circle_rebuild.rs` header,
  `docs/EVENTS.md` row 49):
  > …期间CiRCLE的[触发结算]改为选择获得2层[停留]或失去500资金（该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果），**CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果**
* 「笑容大游行」 (1)/(3) (`docs/rulebook/cards.json` id `HHW:笑容大游行`):
  > （1）[反击]经过“弦卷集团”（#29格）时可将此卡放置在其上，**当次移动的移动终点视为“弦卷集团”地产商**…
  > （3）[持续] 当此卡位于格子上时，**那格视为与“弦卷集团”格子交换位置**，任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆。

Before this item both cards re-homed the effects one clause at a time: the event
hand-suppressed the [经过] reward with `prop::NO_REWARD`, re-implemented it on
the cafe by messaging `tile:circle`'s private settle body (`"settleReward"`),
and replaced both settle bodies by hook; every instance state bound to
`tile:circle` (the suppression arms themselves included) did not ride along, and
any effect the tile gains later would not either. Smile hand-rolled a
one-shot settle redirect (`settle_tile_at` + `set_settle_as_agent` +
`is_settle_redirect`) for (1).

## 0. Surface

```rust
ctx::tile_swap_rules(first, second)  // bind; either order
ctx::tile_clear_rule_swap()          // drop this instance's binding
```

Sugar over two [`ctx::set_prop`] writes of `prop::RULE_SWAP_A` /
`prop::RULE_SWAP_B` (`crates/card-sdk/src/ctx.rs`, `crates/card-sdk/src/abi.rs`
— mirrored in `crates/game-core/src/state.rs`). No sys export: the pair travels
through the existing `set_self_prop`, so `sys.rs` / `sys_kani.rs` /
`kani_host.rs` / `hostfns.rs` / `native_shims.rs` are untouched.

The binding lives on the **calling instance's row**. It is written by a direct
world write and reads through `World::governing_square`; it is in force the
moment the call returns (every later hook of the same window sees it) and is
gone when the row is — unplaced, destroyed, moved back to a zone. `-1` on both
halves (`tile_clear_rule_swap`) drops it early, for a one-shot clause. A value
`< 0` on either half means "no binding".

## 1. What moves, what does not

**Moves:** which *rule instances* govern a square. After
`tile_swap_rules(a, b)` the instances **homed** at `a` answer for every rule
read on `b` and vice versa:

* settle bodies and tile-carrying hooks (`tile_rule_instances` →
  `tile_rule_instances_homed(governing_square(tile))`, `wasm_rules.rs` board
  walk);
* the deed-popup base text and its replacement precedence
  (`tile_text.rs`), and the `prop::START` flush on the walk
  (`play.rs` reads the governing square too);
* tile props: `World::tile_prop` / `tile_prop_keys` / `set_tile_prop` resolve
  through `governing_square`, so `ctx::tile_prop` / `ctx::set_tile_prop` and
  the engine's rent / pay factor reads answer for the rule that actually runs
  on the square;
* the body's own reads: a tile body calls `ctx::self_tile()` and reaches its
  home instance, so `prop::NO_REWARD`, counters and every instance state ride
  along. This is the point of the mechanism: 「所有效果」 is the whole rule
  instance, not a list of clauses.

**Does not move — the square's identity:**

* `tile.id`, the kind (`tile_is(tile.id, "circle")`), `ctx::is_start` and
  `guards::OWN_START` — the static board;
* owners, houses, prices, the deed (`prop::PRICE` and kin are read from the
  **homed** instance: buying, building, mortgaging and redeeming follow the
  deed, not the swapped effects — `drive_tile_op`,
  `tile_rule_instances_homed`);
* `prop::START`: stamped on `tile:circle`, a fact of the square, so
  「每次[经过]CiRCLE时获得N个[火罐]」 still fires on the real CiRCLE and does
  not ride the migration. Test:
  `circle_rebuild_leaves_charge_replenish_on_the_real_circle`.

Cards keyed *on* a square are not the square's own effects and stay put; only
the latter migrate.

The position exchange 「交换位置」 (`prop::SWAPPED_TILE`,
`World::tile_standing_at`) is a separate, older mechanism: it swaps what
*stands* at a slot (display and player placements) and is what smile (3) keeps.
A rule swap is the effects-only counterpart and composes with it.

## 2. Resolution

`World::governing_square(tile)` follows the first binding naming `tile` (row
order: `board_field` first, then seats) to the other end and repeats, at most
one pass per square, so a cyclic pair terminates. `tile < 0` stays `tile`.

`governing_square_of(uid)` reads a board instance's **home** through
`governing_square`; `bears_rule_swap_for(uid, tile)` says whether the instance
is either end of a binding naming `tile`.

Dispatch narrows by the *governing* square, so a tile body runs only on the
square it governs (`field_hears` / `on_tile`, and the resolving pass for a body
suspended at a prompt — without the latter a resumed walk replays its earlier
steps and pays a tile reward once per stepped tile).

`tileResolved` of a settle is answered by **slot**, with a special branch: a
one-shot binder also hears it at either binding end
(`bears_rule_swap_for`). Smile (1) sits on 弦卷集团 while the endpoint settles,
so slot alone would never drop its binding.

## 3. Lifetime and save

The binding is a pair of props on an instance row; the row owns it. A restore
under `Match::restore` carries it because `FieldCard::props` serializes with
`serde(default)`; no save field, no `SAVE_VERSION` bump. Archived engine
bundles replay it as a world write.

## 4. Consumers

* `event:协助CiRCLE重建` `play` — one call swaps `tile:circle` onto
  「CiRCLE咖啡厅」; the event's own row dies at expiry and the swap with it.
  The event's hooks and its settle replacement stay keyed on the **physical**
  CiRCLE (`tile_is(tile.id, "circle")`), and its deed-popup replacements claim
  the CiRCLE slots. Passing CiRCLE pays nothing; passing the cafe pays
  `tile:circle`'s reward and settling it draws CiRCLE's hand card.
* `HHW:笑容大游行` (1) — `settle_body` binds the endpoint to 弦卷集团 for the
  one settle; `on_resolved` calls `tile_clear_rule_swap` at the settle's
  terminal.

## 5. Tests

`rules/tests/tests/ab_tile_swap.rs` (fixture `TEST:rule_swap`,
`TEST:rule_swap_off`, `TEST:rule_identity_probe`):

* `circle_rebuild_swap_binds_both_squares` — both directions, `START` stays on
  its own row (never copied onto the partner);
* `rule_swap_moves_the_settle_body_and_keeps_identity` — the body moves, the
  kind / start reads do not;
* `rule_swap_moves_the_pass_reward` — the [经过] half moves, CiRCLE stops
  paying;
* `rule_swap_reverses_when_the_instance_leaves`;
* `rule_swap_rides_save_and_restore`;
* `rule_swap_composes_tile_text` — the swapped-in texts win, the emptied
  square shows its own (none);
* `smile_parade_one_shot_swap_ends_with_its_settle` — the (2)-declined path
  still drops (1)'s binding at `tileResolved`.

`spec_events` keeps the marina-box regression (a pass prompt suspends and
replays the walk; only the governing square pays) and the updated
`circle_rebuild_*` text expectations.

## 6. Not built

`ctx::tile_set_rule(tile, rule_id)` (install a *named* rule rather than move an
existing instance) — no card needs it yet; the swap covers 「迁移至」 and
「视为…交换位置」. Overlapping bindings resolve first-in-row; no card stacks
two, and the resolution order is documented rather than priority-ordered.
