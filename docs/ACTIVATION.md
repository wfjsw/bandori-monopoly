# Activation arguments + cross-card applicability (ABI v98)

Status: **design** (2026-10-23). Two ABI gaps that `detour.rs`
(`AG:回家的路上绕个道`) hits at once, and the one mechanism each.

| # | gap | mechanism |
|---|---|---|
| 1 | a decision that is part of *declaring* an activation cannot be asked for after the trigger, because the activation cannot be cancelled | **`declare`** -- an optional per-entry collector that runs before the point of no return and hands its result to the body as an argument |
| 2 | a hook's guard must ask whether *another* card's hook / gate would admit, without running that entry's body | **`ctx::can_fire`** -- a probe-mode applicability query over any entry, routed through `admits` |

Both are generalisations of seams that already exist
(`COUNTERACT_MOVE_EXTENSION`, `ctx::card_replayable`). Neither invents a third
axis next to `On::Play` / `On::Action` (`skill-personas/src/lib.rs`: "there is
no `On::Skill`"), and neither lets one rule's Rust call another's
(`CARDS.md` cross-card messages: "One rule's body is that rule's business").

## 1. Why decisions must precede the trigger

An activation is irreversible at a specific line, not "somewhere in the body":

| entry | point of no return | where |
|---|---|---|
| `On::Play` | `hand.remove` | `crates/game-core/src/engine/play.rs:4583` (`play_from_hand`) |
| `On::Counteract` | `hand.remove` | `crates/game-rules/src/wasm_rules.rs:5794` (`build_round`, right after `declare_one` returns `Some`) |
| hooks (`On::Action` / `Filter` / `Settle` / …) | the hook fires and announces | `run_hook` / `entry_admitted` |

Everything after that line -- the `card` counteraction window, the body, the
dest -- runs with the commitment already made. The only "undo" is a post-hoc
`ctx::set_dest(Dest::Hand)` from inside the body, which still leaves the play
announced (log, `card` window, played counters). That is
`rules/cards/card-pp/src/see_you_tomorrow.rs:65-70` and backlog **LOY-SEEYOU-01**
(docs/rulebook/BACKLOG.md): *"do not commit the play until 共鸣 is paid."*

So a decision that is part of the activation -- which mode, which target, which
sub-effect -- has to be collected **before** that line, and the body has to see
the result. Today the only ask site is `ctx::ask_*` inside the body, which is
too late: `detour.rs`'s four-way 「"afterglow"式的移动」 pick, `haruhikage.rs`'s
「使用一次Crychic角色的技能」 pick, and `nanami_ordinary.rs`'s 角色标记 pick all
sit there.

### Prior art: `COUNTERACT_MOVE_EXTENSION`

The one offer-time choice that already works this way
(`docs/CARDS.md`, `abi.rs:881`):

1. `finish_offer` (`wasm_rules.rs:6668-6758`) collects it in a **two-step
   prompt before any declaration or resource change**;
2. it is stored on the chain link (`move_extension`, `wasm_rules.rs:7011`);
3. `resolve_jobs` stamps it onto the trigger as `mv.tags`
   (`wasm_rules.rs:6159-6165`) before `drive(Call::Counteract)`;
4. the body reads `trigger::move_tag(COUNTERACT_MOVE_EXTENSION)`
   (`skill-personas/src/sayo_thorns.rs:119`).

`declare` is that shape, lifted from one hard-coded i32 to a per-entry
collector + a real body argument. (`CounteractHint` at
`wasm_rules.rs:6672-6673` is the same idea, still TODO.)

## 2. Mechanism A -- `declare` (activation argument)

### Guest surface

```rust
/// Collect the activation argument **before** the activation commits.
///
/// * `Ok(None)` -- the player declined. The entry does not fire at all: no
///   body, no header, no flash. For a [反击] this is "do not declare"; for a
///   play it is "put the card back"; for a hook it is the 「可选择」 opt-out.
/// * `Ok(Some(arg))` -- proceed. The body receives `arg` as its second
///   parameter.
/// * `Err(Prompt)` -- pause and re-run, same multi-pass as [`Asked`].
pub type Declared = Result<Option<i32>, Prompt>;
```

Attached with a builder, same style as `On::pre` / `guard_ask`:

```rust
On::Counteract(links, pre, counter_reason, guard, body_with_arg)
    .declare(pick),          // pick: fn(player_id: i32) -> Declared

On::Play(pre, counter_reason, why_not, body_with_arg)
    .declare(pick),
```

Body shapes (erased + driven, the `FilterShape` pattern in
`card-sdk/src/kinds.rs` -- so the 310 existing `fn(i32) -> Asked` bodies are
untouched):

| shape | signature | when |
|---|---|---|
| `BodyShape::Plain` | `fn(i32) -> Asked` | no `declare` (default) |
| `BodyShape::WithArg` | `fn(i32, i32) -> Asked` | entry has `declare` |

`arg` is an `i32` -- the wire's lingua franca (prompt answers already are). A
card that needs more packs it, or reads the rest from `ctx`.

### When `declare` runs

| entry | `OP_DECLARE` runs | decline means |
|---|---|---|
| `On::Play` | in the `act:"play"` / `act:"skill"` path, after the gate admits and **before** `play_from_hand`'s `hand.remove` | the card stays in hand |
| `On::Counteract` | in `finish_offer`, after the player has picked this card and **before** `declare_one` returns `Some` / `build_round`'s `hand.remove` | no counter is declared this visit |
| hooks (`On::Action` / `Filter` / `Settle` / `Mortgage` / …) | in `run_hook` / `entry_admitted`, after `admits` and before the body | the hook does not fire |

For hooks this is also the right home for the rulebook's 「可选择」 offers: a
decline is structurally impossible to get wrong, because the body never runs
and nothing is announced. (A body-level `ask_yes` + early return still leaves
the entry "fired", and any effect before the ask has already been re-run once
by the prompt's multi-pass replay.)

`declare` may call `ctx::ask_*`, including `ctx::can_fire` to build its option
list (mechanism B). That is how the options shown are exactly the ones that
would work.

### Wire

```
ruleset_on(card, entry, op, player_id, arg) -> i64
    OP_RUN     = 0      // body; `arg` is the declared activation argument
    OP_GUARD   = 1      // unchanged, pure; `arg` ignored
    OP_DECLARE = 2      // returns the arg, EXIT_NEED_INPUT, or EXIT_DECLINED
```

* `export::ON` widens from 4 params to 5 (`arg`).
* `EXIT_NEED_INPUT` (`0x0B_A0_D0`) is unchanged -- `OP_DECLARE` uses the same
  multi-pass / `Prompt` machinery as a body `Asked`.
* `EXIT_DECLINED` is a new sentinel (a dedicated `i32`, next to
  `EXIT_NEED_INPUT`) so the host can tell "player said no" from "paused".
* `Call::{Play, Counteract}` and the chain link carry `arg: i32`; `resolve_jobs`
  / `drive` pass it into `OP_RUN` exactly the way `COUNTERACT_MOVE_EXTENSION`
  is stamped today, but as a real parameter rather than a move tag.
* **ABI_VERSION 97 → 98.** `SAVE_VERSION` / `RECORD_VERSION` unchanged (the
  answer log is still one `i32` per prompt; the arg is per-activation, not
  per-prompt, and is recomputed by re-running `OP_DECLARE` on replay -- same as
  every other body re-run).

### Manifest

`ManifestOn` gains one field, always serialized (postcard is not
self-describing):

```rust
/// Does this entry take an activation argument (its body is
/// `fn(i32, i32) -> Asked` and the host must run `OP_DECLARE` first)?
pub has_declare: bool,
```

Display export is unchanged: the declare's prompt is an ordinary `Prompt` once
`OP_DECLARE` runs, so bots (`aiAnswer`) and the UI need **no new answer shape**.
The only behavioural difference the player sees is ordering -- the parameter
question is asked while cancel is still real.

A later UX pass may surface the declare's options in the hand-window label
("回家的路上绕个道 ▸ 兰式 / 巴式 / …") so the pick is one gesture. Not required
for v98; `finish_offer` already does multi-step prompts.

### Bots and the answer log

`OP_DECLARE` runs inside the same drive loop as a body, so its prompts land in
the same `answers: Vec<i32>` log and re-run the same way. `fill_ai` /
`Ask::ai` / chaos fills need no change beyond the prompt itself being earlier.
A declined declare produces **no** answer-log entry on the activation (the
activation never happens).

## 3. Mechanism B -- `ctx::can_fire` (cross-card applicability)

### Guest surface

```rust
/// Can `card_id`'s `kind` entry fire for `player_id` right now?
///
/// Evaluates that entry's **condition + residual guard / play gate** through
/// `admits` (`docs/GUARDS.md` §4.4 item 6) and does **not** run its body.
///
/// **Probe mode** -- never prompts. An entry whose residual guard would ask
/// answers `false`, the same way an eligibility probe folds
/// `RuleError::GuardPrompted` to "not eligible". Unknown id, no such entry,
/// or nesting overflow (`MAX_NESTING`) also answer `false`.
pub fn can_fire(
    player_id: i32,
    card_id: &str,
    kind: OnKind,
    trigger: Option<TriggerKind>,
) -> bool;

/// Could `player_id` play `id` right now (it has a `play` effect and its
/// `cant_play` gate is open)? -- unchanged spelling; now sugar for
/// `can_fire(player_id, id, OnKind::Play, None)`.
pub fn card_replayable(player_id: i32, id: &str) -> bool;
```

This is the existing `card_replayable` seam
(`card-sdk/src/ctx.rs:2292`, `crates/game-rules/src/hostfns.rs:2750-2814`,
which already nests `OP_GUARD` under `admits_gate`) generalised from
`On::Play` to any entry. GUARDS.md §4.4 item 6 already names this case:

> **Every other path that reaches a guard** (UI `playable` / `why_not_act`,
> `ai_play`, **any host request that asks another card's guard**, …) goes
> through one host function `admits(entry, ctx) = pre(ctx) && guard(ctx)`.
> No caller may call a guard directly.

`can_fire` **is** that host request. It is callable from a residual guard,
from a `declare`, and from a body -- guards are guest code and may make host
calls (`detour.rs` already calls `rules_status::move_blocked` from its guards).

For `OnKind::Play`, use `play_entry_preferring_gated` (the "alternative
activations" selection at `host.rs:153-164`) rather than "the first Play
entry", so a card with several press modes answers "is *any* press open" the
way the engine picks a mode.

### What it deliberately does not answer

* **What the body would do.** Only applicability. A card that needs the other
  rule's *answer* (a number, a yes/no about state) uses `ctx::send` /
  `On::Message` (`CARDS.md` cross-card messages).
* **Whether the body would succeed.** The gate is "may I press this?", not
  "would pressing do anything". Residual checks that belong to the body stay
  there (the `mutsumi_never` pattern: the skill body re-checks its own
  「若你的抽牌堆与弃牌堆中都没有卡」 gate).
* **Cost.** One nested wasm instantiation per query, same as today's
  `card_replayable`. Prefer a CEL `pre` when the question is expressible
  there; `can_fire` is for "the answer lives in another card's residual
  guard", which `pre` cannot see.

### Who may query whom

Any entry to any entry. The motivating shape is "my guard depends on that
skill's press gate":

```rust
// detour: 「(2)…（视为使用一次初始Afterglow角色的（2）效果）」
// The card can only be activated when that (2) can be activated.
fn detour_eligible(player_id: i32) -> bool {
    PLANS.iter().any(|id| ctx::card_replayable(player_id, id))
}
```

`OnKind::Action` / `Filter` guards are in scope too (「这个被动开着吗」), which
is what "query the guard of certain hooks in other cards" literally asks for.

## 4. How `detour.rs` uses both

`AG:回家的路上绕个道` (docs/rulebook/cards.json):

> (1)此卡可作为反击使用
> (2)进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的（2）效果），
> 并将下一次的移动掷骰变更为1d6

The (2) of an initial Afterglow character is that character skill's press
entry (`skill-personas`: "Started by the user … `On::Play`, gate and effect").
So the card is: *pick one of those (2)s, and only when that (2) is activatable.*

```rust
On::Counteract(
    &[ChainKind::DiceRollResult],
    "actor == owner && move.roll != null && move.kind != Teleport",
    None,
    Some(detour_eligible),      // mechanism B: some (2) can fire
    counteract,                 // fn(i32, i32) -> Asked
)
.declare(pick_afterglow_2),     // mechanism A: which (2), before commit
```

* `pick_afterglow_2` builds its option list with `ctx::card_replayable` per
  candidate skill, then `ctx::ask_pick`. Decline = no counter declared. The
  pick is the `arg`.
* `counteract(player_id, pick)` receives the pick. Until
  **OPEN-afterglow式移动** (docs/rulebook/spec/OPEN-QUESTIONS.md:224) is
  resolved it keeps the inlined C# plan mutations and the existing
  `TODO(规则书)[judgement]`; once the book names which (2), the body becomes
  `ctx::invoke_skill(player_id, PLANS[pick])` plus the 「并将下一次的移动掷骰
  变更为1d6」 half.

The same split applies to the `On::Play` entry (the play path's four-way pick
moves into its `declare`).

### Rulings this deliberately does **not** make

* **OPEN-afterglow式移动** -- which character's (2) is 「"afterglow"式的移动」.
  The book never says 「初始」. The four plans stay the C# reading, flagged.
* **Does 「视为使用」 pay the skill's cost?** `ctx::invoke_skill` runs the body,
  which spends a fire pot; the C# detour was free. That is a consequence of
  switching to `invoke_skill`, not of these two mechanisms.
* **Does 「视为使用」 raise `skillUsed`?** `invoke_skill` deliberately does not
  (ABI v35); the engine's `use_skill` does. Needed only if the press should
  tick Afterglow (1)'s rest counter / feed `CardUsed` hooks.

## 5. What this replaces / does not add

* **`Guard::Asks`** (the uncommitted `guard-prompt` worktree, 2026-10-23): a
  `fn(i32) -> Guarded` residual that may ask "use this?" with
  `Guarded = Result<bool, Prompt>`. That is the yes/no case of `declare`, and
  putting the ask in the *guard* makes the eligibility probes (`can_counteract`
  / `cant_play` / `ai_play`) have to refuse it (`RuleError::GuardPrompted`) and
  fold that to "not eligible" -- extra machinery for a question `declare`
  already answers *before* the point of no return, where a decline is real.
  New code should use `declare`; a pure `Guard` stays pure.
* **No `On::Skill`.** A skill's press is `On::Play`; its passives are
  `On::Action`. `declare` and `can_fire` attach to the entries that exist.
* **No direct Rust call into another card's body.** `ctx::invoke_skill` /
  `ctx::play_card` (nested `On::Play` body) and `ctx::send` remain the only
  cross-card effect seams; `can_fire` is the query half next to them.
* **No hand-window one-gesture labels** in v98 (see §2).

## 6. Implementation map

| layer | change |
|---|---|
| `rules/card-sdk/src/lib.rs` | `Declared`, `BodyShape`, `PlayEntry` / `CounteractEntry` / `RuleEntry` / `ActionEntry` / `FilterEntry` / `MessageEntry` gain `declare: Option<fn(i32) -> Declared>` + erased body + `drive`; `On::declare` builder |
| `rules/card-sdk/src/kinds.rs` | same for `ActionEntry` / `FilterEntry`; `action` / `filter_*` ctors keep `fn(i32) -> Asked` sugar |
| `rules/card-sdk/src/rt.rs` | `OP_DECLARE` arm; `OP_RUN` takes `arg` and drives `BodyShape` |
| `rules/card-sdk/src/abi.rs` | `ABI_VERSION = 98`; `export::ON` 5 params; `OP_DECLARE`; `EXIT_DECLINED`; `ManifestOn::has_declare`; v98 changelog note |
| `rules/card-sdk/src/ctx.rs` + `ctx/sys.rs` + `sys_kani.rs` | `can_fire` |
| `rules/card-sdk/src/kani_host.rs` | `Op::can_fire` |
| `crates/game-rules/src/hostfns.rs` | `can_fire` host (generalise `card_replayable`); `call_entry` gains `arg` |
| `crates/game-rules/src/host.rs` | `Call::{Play,Counteract}` gain `arg`; `run` / `run_hook` / `entry_admitted` run `OP_DECLARE` before the body; `has_declare` plumbing |
| `crates/game-rules/src/wasm_rules.rs` | `finish_offer` runs `OP_DECLARE` before `declare_one` commits; `build_round` stores `arg` on the chain link; `resolve_jobs` passes it to `OP_RUN`; `play` path runs `OP_DECLARE` before `play_from_hand` |
| `crates/game-rules/src/native_shims.rs` | `op_can_fire`, widened `ruleset_on` |
| `crates/rules-native/src/lib.rs` | mirror the widened `on` + `can_fire` |
| `crates/game-core/src/engine/play.rs` / `mod.rs` | `act:"play"` / `act:"skill"` collect the arg before `hand.remove`; `NetMessage` gains an optional `arg` only if the UI wants to pre-declare (v98: host runs `OP_DECLARE`, so no net change) |
| `rules/fixtures/test-cards` | a fixture card with a `declare` (pick one of N, cancel declines) and a fixture pair whose guard reads the other via `can_fire` |
| `rules/tests` | black-box: decline leaves the card in hand / no counter declared; the body sees the pick; `can_fire` true/false across a gated pair; probe mode does not prompt |
| `docs/CARDS.md`, `docs/GUARDS.md` | pointers here |

`build-ruleset.mjs` must re-run after any `rules/cards` edit (the ruleset wasm
is staged; see the standing note). Do **not** set `CARGO_TARGET_DIR` for it.