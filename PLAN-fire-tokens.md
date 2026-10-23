# Fire charge → skill-instance token counter

Branch: `worktree-fire-tokens`. Supersedes `docs/MARKERS.md` §5's "physical
re-homing … is not required by this design" — we are doing the re-homing.

## Ground facts (do not re-derive)

* 规则书 「标志物解释」: 「火罐：**技能指示物**」. It is a counter *of the skill*,
  not a free player resource. Contrast 奇迹水晶 「卡牌指示物」.
* User ruling 2026-10-10 (`marker-owned-by-creating-rule`): every mark/token is
  a counter of its creating (pseudo) card, bound to card / tile / **player**,
  and dies with the instance. "Held by a player" is a legal *location*.
* User ruling 2026-10-23: **抹茶芭菲 is NOT a `mark:*` standing pseudo card.**
  It is instance-owned by `skill:要乐奈:…` and is collected when the holder is
  down. Only `mark:cp` and `mark:pp_fans` are the standing hosts.
* Engine already has the target store: `MatchPlayer.tokens: Vec<Counter>`
  with `Counter { name, value, instance: Option<i32> }`, `ops.rs::bind_held`
  / `count_held` / `set_tok` / `add_tok`, and `destroy_instance_units(uid)`
  which does `p.tokens.retain(|t| t.instance != Some(uid))` on unplace /
  discard / player leave / bankrupt / event expire.
* Engine is a **dumb store**; policy lives in `rules/bandori-std/src/fire.rs`.
  `game-core/src/engine/ops.rs` `gain_marker` must stay an uncapped dumb add.

## Target model

One held token per pool, owned by the seat's **persona-skill field instance**:

| Concept | Before | After |
|---|---|---|
| 火罐 count | `MatchPlayer.state["charge"].value` | `tokens` row `name="charge"`, `instance=<persona-skill uid>` |
| 「上限M」 live cap | `MatchPlayer.state["charge"].max` | `Counter.max` on that row |
| 「（初始N，上限M）」 declaration | `CardDef` props `fireInitial`/`fireCap` | **unchanged** — still the source of truth for the printed numbers |
| lisa's 「可超过上限的临时火罐」 | `MatchPlayer.state["tempFire"]` | second token row `name="tempFire"`, **same instance** |
| Webui fire chip | `stateOf(x,"charge")/stateMax(x,"charge")` | read the `"charge"` token's `value`/`max` |
| cond-ir `charge` var | `state_get(p, "charge")` | read the `"charge"` token |

Keep the frozen **vocabulary** (`"charge"`, `"tempFire"`, `fireInitial`,
`fireCap`, `fire::KEY`, `fire::TEMP_KEY`). Only the container changes. This is
exactly the "non-semantic refactor" §5 said was allowed.

## Work items

### 1. `crates/game-core` — token carries a live cap

* `Counter` gains `pub max: i32` with `#[serde(default)]` (0 = uncapped).
  Backward-compatible: old saves/replays deserialize to `max = 0`.
* `set_named` / `set_tok` / `add_tok` / `bind_held` accept and store `max`.
  `bind_held` already clamps `value = min(max)` when `max > 0` — keep that
  (it is the pull-under for I2 「在任意时刻」).
* Add `persona_skill_uid(player_id) -> i32` beside `group_skill_uid`
  (`ops.rs:650`). The persona skill is `skill:<persona>:<skill>`; it is the
  field card whose `card` equals `World::persona_skill_id(player_id)`, owner
  `player_id`, not `group_skill`. Return -1 when absent.
  Mirror the wrapper in `game-rules` `World` trait + `wasm_rules` + `hostfns`
  + `card-sdk` `ctx`/`sys`/`sys_kani` the same way `group_skill_uid` is wired,
  **or** (preferred if it keeps the ABI smaller) have the fire kernel resolve
  the uid through an existing generic lookup and skip a new host import —
  check what `ctx::persona_skill` already returns before adding a verb.

### 2. `rules/bandori-std/src/fire.rs` — kernel rewrite

Public API **stays the same** so no call site changes:

`get`, `max`, `total`, `gain`, `spend`, `adjust_cap`, `add_max`, `declare`,
`redeclare`, `declare_own`, `declare_from`, `redeclare_from`, `overlay`,
`end_overlay`, `regime::{count, fill_to_cap, drain, burn_all}`,
`temp::{get, max, grant, spend}`, `FIRE_INITIAL`, `FIRE_CAP`, `KEY`,
`TEMP_KEY`, `gain_clamped`, `pull_under_cap`.

Behaviour to preserve exactly (the clause comments in the current file are
the spec — keep them, updating only the storage references):

* `gain` clamps with `gain_clamped`, refuses before the write when `want <= 0`
  and logs `log.gain_charge_capped`, otherwise logs one 效果适用 entry via the
  host verb. Opens `markerGain` before the pots move; a cancelled window moves
  nothing.
* `spend` draws `temp` first (MARKERS.md §10(2)), then the pool; `ChargePaying`
  opens even when the pool is empty (substitution path); `markerSpend` before
  the move; raises `markerSpent` after.
* `adjust_cap` moves the cap and pulls the value under it.
* `declare` writes **initial only at `DeckAtGameStart`** and the cap **only
  when the token row does not yet exist**. With instance ownership the row's
  existence *is* the "already declared" latch — the old
  "cap written only when unset (0)" hack goes away, because a per-turn
  `TurnStartBefore` re-hook now finds the live row and leaves it alone.
  Document that in the `declare` doc comment.
* `redeclare` / `overlay` force-write 初始/上限 and pull under the new cap.
* `end_overlay` re-declares the seat's own face (its text is the ground fact).

Storage: resolve `uid = persona_skill_uid(player_id)`. When `uid < 0`
(no fire skill) every read is 0 and every write is a no-op — `declare_own`
already returns false in that case. `get` = `count_held(uid, KEY, player_id)`;
`max` = the row's `max`. Name the token `KEY` / `TEMP_KEY` so the wire
vocabulary is unchanged.

### 3. Rules-layer writers of `state["charge"]` / `state["tempFire"]`

After the kernel lands there must be **no** remaining writer of those state
keys outside tests. Grep for and update:

* `rules/bandori-std/src/lib.rs` `charge` / `charge_max` / `gain_charge` /
  `spend_charge` / `add_charge_max` — the thin sugar wrappers. Either delete
  them and their `ctx` re-exports (preferred: they are an uncapped bypass
  footgun) or make them forward to `fire::`. If kept, fix the doc comment on
  `gain_charge` which currently claims "capped by the player's own cap" and is
  a lie.
* `crates/game-rules/src/cond_pre.rs` — `state_get(p, "charge")` at
  lines ~231, ~324, ~1235, ~1377, ~1520 and the name table at ~1596. The
  cond-ir variable is still called `charge`; only its **source** becomes the
  token. Same for `wasm_rules.rs` ~7575 / ~7698.
* Anything reading `state_max(*, "charge")` for display.

### 4. Webui

* `webui/src/core/types.ts` `Counter` gains `max?: number`.
* `webui/src/scenes/board/Players.tsx:193` fire chip: read the token named
  `"charge"` (`x.tokens`) → `value`/`max`, falling back to `stateOf`/`stateMax`
  for old saves. Do **not** change the visual.
* `markersOf` already lists `x.tokens` — fire will now appear there via the
  token loop instead of the state loop. That is correct; leave it.
* Any other `stateOf(x, "charge")` / `"tempFire"` reader
  (`webui/src/game/autopilot.ts`, `webui/src/core/names.ts` callers).

### 5. Docs

* `docs/MARKERS.md` §5: rewrite. The decision of 2026-10-18 ("re-homing is not
  required") is **superseded 2026-10-23** — fire is now physically
  instance-owned. Keep the opacity invariant ("the engine treats these names
  as opaque data; only the rules-layer kernel writes them").
* `docs/MARKERS.md` §7: 抹茶芭菲 is **not** in the `mark:*` family. It is a
  counter of `skill:要乐奈:…` held by players / placed on a space, and is
  collected when the holder is down. Fix the earlier mis-classification.
* `docs/MARKERS.md` §6 table: note the re-homing landed.
* `docs/STATUS-HOSTS.md` if it repeats the old storage story.

### 6. Tests

* `rules/tests/tests/markers.rs` — extend the pins:
  * fire count is a `Counter` whose `instance` is the persona-skill uid;
  * `destroy_instance_units` / skill unplace drops the pots with it;
  * swap (`two_in_one`) leaves no row owned by the old instance;
  * `tempFire` is a second row of the same instance;
  * keep `fire_numbers_are_on_the_skill_def` as-is (declaration census).
* Every test that pokes `state_get/set("charge")` must move to the token
  store. Known sites: `rules/tests/tests/{ag,general,mygo,markers,status_layers,
  spec_card_*,roselia,hhw,ras,events,spec_events,gap_res,live_match}.rs`,
  `rules/tests/tests/common/mod.rs` (`charge()`, `set_fire`), `rules/tests/
  tests/fuzz/drive.rs`, `rules/tests/tests/q4/{snap,names}.rs`,
  `crates/game-core/tests/debug.rs`, `crates/game-rules/tests/common/mod.rs`.
* Kani: `rules/kani/src/host.rs` `arm_gain_marker` currently **models a cap
  clamp the real engine does not have** and hardcodes `w.charge`. Fix it to
  match `game-core`'s dumb add (or route it to the token store) — this is a
  real proof-model/reality divergence and is in scope. Also fix the stale
  "engine clamps to `charge_max`" comments in
  `rules/kani/src/spec/skills_ras.rs`, `rules/kani/src/spec/skills_mor.rs`,
  `rules/kani/tests/conf_hostops2.rs`, `docs/rulebook/spec/kani/ras.md`.

## Build / verify (from this worktree)

```
# ruleset wasm must be rebuilt after any rules/ or bandori-std edit,
# or ruleset-tests silently runs the stale wasm:
node scripts/build-ruleset.mjs        # do NOT set CARGO_TARGET_DIR (stale wasm)

cargo test -p ruleset-tests --test markers
cargo test -p ruleset-tests
cargo test -p game-core
cargo test -p game-rules
```

Webui: a fresh worktree may need `npm install` + `npx rsbuild build` (or
`npm run build`) only if you need to typecheck the TS change. Do **not**
`npm run build` into `webui/dist` on a shared tree — here it is a worktree so
it is fine, but prefer `npx tsc --noEmit` for a typecheck-only pass.

## Hard constraints

* No `Co-Authored-By` trailer on any commit.
* Do not push the feature branch or open a PR.
* Do not edit outside this worktree.
* Do not `rm -rf` the worktree directory.
* 规则书 text is ground fact. Anything that does less than the book says gets
  `TODO(规则书)`.
* Every 效果适用 logs which effect, why, and the outcome, as one entry.
* Rounding: money rounds to 10 once, after every add/mul.
* "counteraction" for [反击] windows, "hook" for passive listeners — never
  "reaction".