# Activation arguments + cross-card applicability (ABI v98)

Status: **design** (2026-10-23). Two ABI gaps that `detour.rs`
(`AG:回家的路上绕个道`) hits at once, and the one mechanism each.

| # | gap | mechanism |
|---|---|---|
| 1 | a decision that is part of *declaring* an activation cannot be asked for after the trigger, because the activation cannot be cancelled | **`declare`** -- an optional per-entry collector that runs before the point of no return and hands its result to the body as a **typed argument** (the card's own struct: a pick, a tile, a card, …) |
| 2 | a hook's guard must ask whether *another* card's hook / gate would admit, without running that entry's body | **`ctx::query_guard`** -- a probe-mode applicability query by the entry id the target declares, routed through `admits` |

Both are generalisations of seams that already exist
(`COUNTERACT_MOVE_EXTENSION`, `ctx::card_replayable` -- the latter folded into `query_guard`). Neither invents a third
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

The arg is a **card-authored struct**, not a wire scalar. The collector and the
body live in one file and share the type; only the wire encoding is fixed
(postcard), the same way `ctx::message::Message` is a typed record on the wire
while `FilterPayload` types are card-visible vocabulary.

```rust
/// A typed activation argument. Blanket-impl'd for any postcard record.
///
/// `T` is the **card's own** struct -- a tile, a card id, a seat, a pick, a
/// count -- whatever that activation decides. Nothing about it has to be
/// shared with other cards, because the collector and the body are in the
/// same file and only `postcard` crosses.
pub trait ActivationArg: serde::Serialize + serde::de::DeserializeOwned + Default {}
impl<T: serde::Serialize + serde::de::DeserializeOwned + Default> ActivationArg for T {}

/// What a `declare` collector returns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activation<T: ActivationArg = i32> {
    /// The player declined. The entry does not fire at all -- no body, no
    /// header, no flash, no log. For a [反击] this is "do not declare"; for a
    /// play it is "the card never left the hand"; for a hook it is the
    /// 「可选择」 opt-out.
    Decline,
    /// Proceed; the body receives `T` as its second parameter.
    Arg(T),
}

/// Collect the activation argument **before** the activation commits.
/// `Err(Prompt)` pauses and re-runs, same multi-pass as [`Asked`].
pub type Declared<T: ActivationArg = i32> = Result<Activation<T>, Prompt>;
```

`T = i32` is the default, so the common "which of N" / yes-no case stays
`Declared` / `Activation::Arg(7)` with no ceremony and no postcard noise
beyond two bytes. A pick that is a tile, a card, or several of those is just
that card's struct:

```rust
/// The card's own activation argument -- private to this file.
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
struct DetourPick {
    /// Index into the candidate skill list.
    plan: i32,
    /// The tile the plan aims at, or -1.
    tile: i32,
    /// The seat it aims at, or -1.
    seat: i32,
}

fn pick(player_id: i32) -> Declared<DetourPick> {
    let tile = ctx::ask_tile(player_id, &title, &text, &tiles)?;
    Ok(Activation::Arg(DetourPick { plan: 3, tile, seat: -1 }))
}

fn body(player_id: i32, pick: DetourPick) -> Asked { /* … */ }

On::Counteract(links, pre, reason, guard, body).declare_arg(pick, body),
```

`Result` keeps the `?` propagation the codebase's `Asked = Result<(), Prompt>`
relies on; the two-arm is in `Activation` so "declined" is never confused with
"proceed".

Attached with a builder, same style as `On::pre`:

```rust
On::Counteract(links, pre, guard, body).counter_reason(..)
    .declare_arg(pick, body),   // pick: fn(i32) -> Declared<T>, body: fn(i32, T) -> Asked

On::Play(pre, why_not, body).counter_reason(..)
    .declare_arg(pick, body),
```

`T` is inferred from the two fns. Bodies are erased + driven (the
`FilterShape` pattern in `card-sdk/src/kinds.rs`), so the ~310 existing
`fn(i32) -> Asked` bodies are untouched:

| shape | signature | when |
|---|---|---|
| `BodyShape::Plain` | `fn(i32) -> Asked` | no `declare` |
| `BodyShape::WithArg` | `fn(i32, T) -> Asked` | entry has `declare` |

`On::….declare(f)` (decline-only, body stays `fn(i32) -> Asked`) remains as
sugar for `declare_arg` with `T = ()`.

### When `declare` runs

| entry | `OP_DECLARE` runs | decline means |
|---|---|---|
| `On::Play` | in the `act:"play"` / `act:"skill"` path, after the gate admits and **before** `play_from_hand`'s `hand.remove` | the card stays in hand |
| `On::Counteract` | in `finish_offer`, after the player has picked this card and **before** `declare_one` returns `Some` / `build_round`'s `hand.remove` | no counter is declared this visit |
| `On::Action` / `Settle` / `Mortgage` / `Redeem` / `Buy` / `Build` / `Message` | in `run_hook` / `entry_admitted`, after `admits` and before announce/flash/body | the hook does not fire |
| **`On::Filter`** | **rejected at ruleset build** | — |

For the event-reaction hooks this is also the right home for the rulebook's
「可选择」 offers: a decline is structurally impossible to get wrong, because
the body never runs and nothing is announced. (A body-level `ask_yes` + early
return still leaves the entry "fired", and any effect before the ask has
already been re-run once by the prompt's multi-pass replay.)

`declare` may call `ctx::ask_*`, including `ctx::query_guard` to build its option
list (mechanism B). That is how the options shown are exactly the ones that
would work.

### `On::Filter` cannot declare (fail closed)

A filter body is a **pure value fold** — `FilterShape::Value` / `Flag` / `Gate`
/ `Payload`, driven under `pure::enter()`, with `pure::refuse_if_filter` on
every effect import (`card-sdk/src/pure.rs`). Pausing a `PaySplitMul`,
`MoveGate` or `DiceRollPlan` mid-pipeline for a player prompt is the same
architectural mistake as letting a filter emit effects: the pipeline is
already committed to folding this value.

`build-ruleset` rejects a `declare` on `On::Filter`, the same way a mixed
Action/Filter kind list is rejected. If a filter stage needs to be skippable,
the opt-out is an `On::Action` at an interactive pre-stage carrying the
`declare` — the pattern `crimson_soul` already uses, putting its interactive
−1000 on `On::Action(Action::PaySplitChooseBefore(…))` so the ask sits on an
event reaction immediately before the pure `PaySplitChoose` fold (v78).

### Wire

The arg is **postcard bytes**, carried the way `group_skills` and
`ruleset_metadata` already carry structured guest data: the guest hands a
buffer out and pulls one back. `export::ON` therefore stays at **four**
params -- the arg is not a call parameter, it is per-activation state the host
holds between `OP_DECLARE` and `OP_RUN`.

```
ruleset_on(card, entry, op, player_id) -> i64
    OP_RUN     = 0      // body; the drive pulls the arg via ctx::declared_arg
    OP_GUARD   = 1      // unchanged, pure
    OP_DECLARE = 2      // returns packed (ptr<<32)|len of postcard(T),
                        // or EXIT_NEED_INPUT, or EXIT_DECLINED

ruleset_declared_arg(out_ptr, cap) -> n     // host fills `out_ptr`; n = bytes
                                            // (the group_skills out_buf shape)
```

* **`OP_DECLARE`'s return** uses the existing packed-`(ptr<<32)|len` convention
  (`export::MANIFEST` / `METADATA` already do). `EXIT_NEED_INPUT`
  (`0x0B_A0_D0`) is unchanged -- `OP_DECLARE` uses the same multi-pass /
  `Prompt` machinery as a body `Asked`. `EXIT_DECLINED` (`0x0B_A0_D1`) is a
  new sentinel so the host can tell "player said no" from "paused".
* **No sentinel collision.** The sentinels are small integers and the arg is a
  buffer address, so an `Arg(T)` can never be mistaken for "paused" /
  "declined". The raw-`i32` assert the first cut needed is gone with the
  raw-`i32` arg.
* The host **stores `Vec<u8>`** on the activation (the chain link for a [反击],
  the in-flight play for a play) and serves it back on `OP_RUN` through
  `ruleset_declared_arg`. The body's `drive` calls `ctx::declared_arg::<T>()`
  and passes `T` to `fn(i32, T) -> Asked`, so `T` is still a literal parameter.
* **Trap severity: a trapping `OP_DECLARE` never commits and never costs the
  card.** The collector runs *before* the point of no return, so there is
  nothing to graveyard -- the card is still in hand (a play / [反击]) or still
  in play (a hook). A trap is therefore a **decline**, not a destroy. The rule
  that lines the sites up:

  | site | on trap | card |
  |---|---|---|
  | residual guard (`entry_admitted`) | declines | untouched |
  | **`OP_DECLARE`** (every entry, both paths) | **declines** | **untouched** |
  | `OP_RUN` (body) | `log.card_trap` + `DEST_GRAVEYARD` | lost |

  The body's `DEST_GRAVEYARD` is not a louder severity setting -- it is a
  **post-commit consequence**: the card already left the hand and then blew
  up, and losing it is the established cost of a trapping module
  (`wasm_rules.rs`: "a trapping module must not take the match down with it").
  A declare trap has no such cost to charge. Propagating it into that arm
  destroys a card whose body never ran.

  Visibility is **not** uniform yet, and the split is backwards -- see the
  follow-up below. The **severity** is what must not vary.

### Follow-up (tracked): every "the entry did not fire" trap should log

**Not landed** -- deferred rather than widened into the end of the ABI
delivery. Severity is correct and pinned (`trapping_declare_never_costs_the_card`);
this is observability only.

Today's split:

| site | logs `log.card_trap`? | the player… |
|---|---|---|
| residual guard (`entry_admitted`) | no ("historical dispatch leniency") | — |
| `OP_DECLARE` on a hook (`declare_after`) | **no** | just clicked nothing; a 「可选择」 silently did not apply |
| `OP_DECLARE` on a play / [反击] | yes | just clicked something and would notice |
| `OP_RUN` (body) | yes | — |

That is loudest where the player needs it least and silent where they need it
most. The hook path is the automatic 「可选择」 case -- the entire reason
`declare` exists for hooks -- and the reference is `AG:绯红之魂`'s `pay_offer`.
If that collector traps the player is **never asked**, the −1000 silently does
not apply, and nothing is logged: they are looking at a payment that did not
get cut, not at a dialog that failed to appear. `log.card_trap` naming the
card and the detail turns "绯红之魂 didn't trigger, with no evidence" into a
one-line diagnosis. The residual guard has the same hole and it is older.

**The shape** (peer-reviewed): an `on_trap: Option<&mut dyn FnMut(&mut W, i32)>`
callback on `entry_admitted`, mirroring `run_hook`'s existing
`on_body: Option<&mut dyn FnMut(&mut W, i32)>` -- that callback already exists
for exactly this ("the host wants to know when a thing happened inside
`entry_admitted`"). **Do not** put `trapped` on `Admitted::No` / `HookRun`: a
trapping declare yields `Admitted::No`, which makes `run_hook` return
`Ok(None)`, so the drive has no channel left to learn "and by the way one of
them trapped". The callback is the one that survives that.

**Land both arms together** -- the hook `OP_DECLARE` *and* the residual
guard's trap decline -- or the table above gains a fifth row and we are back
to a half-fix.
* `Call::{Play, Counteract}` and the chain link carry the arg as `Vec<u8>` (postcard); `resolve_jobs`
  / `drive` pass it into `OP_RUN` exactly the way `COUNTERACT_MOVE_EXTENSION`
  is stamped today, but as a real parameter rather than a move tag. **The
  stored `arg` is the only source of truth at resolve time.** `OP_DECLARE` runs
  on the *offer* pass and its result is recorded; `resolve_jobs` **never
  re-enters `OP_DECLARE`**. Recomputing the arg at resolve time is exactly
  where answer-cursor drift lives -- the declare's prompts are in the offer
  drive's log and the body's prompts are in the resolve drive's log, and a
  recompute at resolve time has to reach back into a log it does not own.
* **`OP_DECLARE` and `OP_RUN` share one `next_answer` cursor within a single
  drive** -- one `HostState`, across `admits` + `OP_DECLARE` + `OP_RUN`.
  `entry_admitted` returns the cursor *after* `admits` + `OP_DECLARE`;
  `OP_RUN` starts there. Otherwise a declare prompt followed by a body prompt
  re-reads answer 0. This rule is **per-drive**: `finish_offer` and
  `resolve_jobs` are separate `run_hook` / `run` calls with separate
  `HostState`s, each starting `next_answer = 0` over its *own* `answers`
  slice, and that is correct. Consequences:
  * `finish_offer` must not create a fresh store per `OP_DECLARE` step (a
    two-step declare -- pick, then confirm -- is one cursor, both prompts
    advance it);
  * if `finish_offer` is re-entered on a multi-pass pause, `OP_DECLARE`
    re-reads its answers from index 0 of *that* drive's log and must not
    double-consume.
* **ABI_VERSION 97 → 98.** `SAVE_VERSION` / `RECORD_VERSION` unchanged (the
  answer log is still one `i32` per prompt; the arg is per-activation, stored
  on the link / play, and is read back -- not recomputed -- on replay).
* **Player-visible ordering (one changelog sentence).** For `On::Play` the
  `declare` prompt fires *before* the `card` counteraction window, because
  `hand.remove` is the commit and the window opens after it. That is the right
  order -- the mode is part of declaring the play, matching
  `COUNTERACT_MOVE_EXTENSION`'s two-step-before-declaration -- but it is a
  change cards like `see_you_tomorrow` will show.

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

`OP_DECLARE` runs inside the same drive as the body, so its prompts land in
that drive's `answers: Vec<i32>` log and re-run the same way. `fill_ai` /
`Ask::ai` / chaos fills need no change beyond the prompt itself being earlier.

**A decline records its own answer.** Only a prompt-free `Decline` is
log-silent -- a `declare` that returns `Activation::Decline` without asking
replays deterministically and declines again. A `declare` that *asked* "use
this?" and got **no** has consumed an answer-log slot at the index it was
asked, exactly like any other prompt. Dropping that answer because "the
activation never happens" makes replay re-enter `OP_DECLARE`, find no answer
at that index, and **re-prompt the player** (or pause the match). Rule: *a
decline that came from a prompt records that prompt's answer; only a
prompt-free `Decline` is log-silent.*

## 3. Mechanism B -- `ctx::query_guard` (cross-card applicability)

### Named entries

The query is by **identifier the target card declares**, not by a positional
`(kind, trigger)` pair. That is the `On::Message(names, …)` shape already in
the tree: entries carry names, the other side matches by name. It removes the
"which of this card's three hooks did you mean?" ambiguity outright, and the
name is a deliberate contract the target owns.

### Identity is not display text

An entry's `id` is **internal identity**. It is never shown to a player and it
is not the rulebook's printable clause label. The tree already keeps these
apart, and `id` must land on the identity side of that line:

| field | what it is | example | shown to players? |
|---|---|---|---|
| `id` (**new**) | stable internal slug, ASCII, per-card unique | `"decay"`, `"tick"` | **never** |
| `label` (existing, `ManifestOn::label`) | opaque display text for the 「效果适用」 line | `"（2）"` | yes |
| `counter_reason` (existing) | a `MsgKey` -- a locale **key**, resolved per player | `key!("himari_plus_one_when")` | resolved text |
| `On::Message(names)` | same family as `id` -- internal slugs | `"clear_cp"` | never |

An id that is printable rulebook text is a bug even though it "works":

* renumbering or re-styling the clause would silently break every query;
* `（2）` and `(2)` are different strings -- full-width vs half-width parens is
  a mismatch that looks identical in a diff;
* it invites someone to "fix the wording" in the one place that is not wording.

So: the skill's press entry carries the clause as its **label** only --
`entry::PRESS` already addresses it, and `"press"` is reserved so no entry may
declare it. A *hook* that wants to be queried declares its own slug
(`.id("decay")`). `build-ruleset` **rejects a non-slug id** (the same fail-closed
net as `On::Filter` cannot declare): an id must match
`[A-Za-z0-9_][A-Za-z0-9_.:-]*`, and may not be `entry::PRESS`. The rulebook
clause is not an id.

```rust
// In the skill's file -- identity and display are separate fields.
On::Play("", Some(can_use), use_skill).label("（2）")   // "press" is reserved; no .id needed

On::Action(Action::TurnEnd("card.placed", None, tick)).id("decay")

// In detour's file -- the query names the slug, never the label.
ctx::query_guard(player_id, "skill:美竹兰:叛逆的红挑染", entry::PRESS)
```

`id` is `&'static str` on the entry, `""` = not externally addressable (the
common case; nothing pays for it). It rides the manifest as
`ManifestOn::id: Option<String>`, always serialized. Ids are unique **per
card**, not globally.

(Pre-existing, not introduced here: a **card** id like
`skill:美竹兰:叛逆的红挑染` is itself CJK text. That is the established
identity of a card and it is load-bearing -- `rules/rulebook/cards.json` keys
are those strings -- so it is out of scope. The new field is the per-entry
`id`, and that one must be a slug.)

### Guest surface — one query

`card_replayable` is **not** a second concept: "may I press this card?" is
just "does the press entry admit?", asked of the press entry. So there is one
function and one reserved id.

```rust
pub mod entry {
    /// The reserved id of a card's press entry (`On::Play`) -- the
    /// **press surface** question, "may I use this card / this skill
    /// button?". Not a declared id: it always resolves the entry the
    /// engine would pick (`play_entry_preferring_gated`). `build-ruleset`
    /// rejects a declaration of `.id(PRESS)` so the word cannot be
    /// squatted.
    pub const PRESS: &str = "press";
}

/// Does the named entry of `card_id` admit for `player_id` right now?
///
/// Evaluates that entry's **condition + residual guard / play gate** through
/// `admits` (`docs/GUARDS.md` §4.4 item 6) and does **not** run its body.
///
/// **Probe mode** -- never prompts. An entry whose residual guard would ask
/// answers `false`, the same same way an eligibility probe folds
/// `RuleError::GuardPrompted` to "not eligible". Unknown card, unknown or
/// unnamed entry, or nesting overflow (`MAX_NESTING`) also answer `false`.
pub fn query_guard(player_id: i32, card_id: &str, entry_id: &str) -> bool;
```

One call shape, and the two questions read differently at the call site
because the address differs:

| question | call |
|---|---|
| "may I press this card / use this skill?" | `query_guard(p, id, entry::PRESS)` |
| "does that named entry on that card admit?" | `query_guard(p, id, "decay")` |

`card_replayable` is **deleted**, not kept as sugar. Its three call sites
(`nfo.rs`, `please_choose.rs`, `detour.rs`) become `query_guard(…, entry::PRESS)`
— same semantics, and the misleading name goes with it ("replay" already means
match replay here; it always meant *playable*, the bool of
`cant_play(...).is_none()`).

The name is `query_guard` -- the verb from the originating ask ("a guard
querying the guard of other cards"), and the doc's own phrasing ("any host
request that asks another card's guard"). It replaces `can_fire`,
`entry_admits` and `card_replayable`; none is kept as an alias.

This generalises the existing `card_replayable` seam
(`card-sdk/src/ctx.rs`, `crates/game-rules/src/hostfns.rs`, which already
nests `OP_GUARD` under `admits_gate`). GUARDS.md §4.4 item 6 already names the
case:

> **Every other path that reaches a guard** (UI `playable` / `why_not_act`,
> `ai_play`, **any host request that asks another card's guard**, …) goes
> through one host function `admits(entry, ctx) = pre(ctx) && guard(ctx)`.
> No caller may call a guard directly.

`query_guard` **is** that host request. It is callable from a residual guard,
from a `declare`, and from a body -- guards are guest code and may make host
calls (`detour.rs` already calls `rules_status::move_blocked` from its guards).

`can_fire` / `can_fire_any` (the first cut's positional forms) are **not**
kept. The identifier is the fix for the ambiguity they carried.

### What it deliberately does not answer

* **What the body would do.** Only applicability. A card that needs the other
  rule's *answer* (a number, a yes/no about state) uses `ctx::send` /
  `On::Message` (`CARDS.md` cross-card messages).
* **Whether the body would succeed.** The gate is "may I press this?", not
  "would pressing do anything". Residual checks that belong to the body stay
  there (the `mutsumi_never` pattern: the skill body re-checks its own
  「若你的抽牌堆与弃牌堆中都没有卡」 gate).
* **Cost.** One nested wasm instantiation per query, same as today's
  `query_guard`. Prefer a CEL `pre` when the question is expressible
  there; `query_guard` is for "the answer lives in another card's residual
  guard", which `pre` cannot see.

### Who may query whom

Any entry to any entry. The motivating shape is "my guard depends on that
skill's press gate":

```rust
// detour: 「(2)…（视为使用一次初始Afterglow角色的（2）效果）」
// The card can only be activated when that (2) can be activated.
fn detour_eligible(player_id: i32) -> bool {
    PLANS.iter().any(|id| ctx::query_guard(player_id, id, entry::PRESS))
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
// In the skill's file — the id *is* the rulebook clause.
// rules/skills/skill-personas/src/ran_red.rs
On::Play("", None, Some(can_use), use_skill).label("（2）")   // "press" is reserved; no .id needed

// In detour's file.
const PLANS: [&str; 4] = [
    "skill:美竹兰:叛逆的红挑染",
    "skill:宇田川巴:豚骨酱油拉面大姐",
    "skill:上原绯玛丽:大家一起迈出新的一步",
    "skill:上原绯玛丽:大家一起迈出新的一步",
];

/// The card's own activation argument -- private to this file.
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
struct DetourPick {
    plan: i32,   // index into PLANS
    tile: i32,   // the plan's landing tile, or -1
    seat: i32,   // the seat it aims at, or -1
}

fn detour_eligible(player_id: i32) -> bool {
    PLANS.iter().any(|id| ctx::query_guard(player_id, id, entry::PRESS))
}

fn pick_afterglow_2(player_id: i32) -> Declared<DetourPick> {
    // Only the (2)s that actually admit are offered.
    let live: Vec<usize> = (0..PLANS.len())
        .filter(|&i| ctx::query_guard(player_id, PLANS[i], entry::PRESS))
        .collect();
    let k = ctx::ask_pick(player_id, &title, &text, &labels(&live))?;
    Ok(Activation::Arg(DetourPick {
        plan: live[k] as i32,
        tile: ctx::tile_named("银河拉面馆"),
        seat: -1,
    }))
}

On::Counteract(
    &[ChainKind::DiceRollResult],
    "actor == owner && move.roll != null && move.kind != Teleport",
    None,
    Some(detour_eligible),
    counteract_with_plan,           // fn(i32, DetourPick) -> Asked
)
.declare_arg(pick_afterglow_2, counteract_with_plan),
```

* `pick_afterglow_2` builds its option list with `ctx::query_guard` per
  candidate skill's press entry (`entry::PRESS`), then `ctx::ask_pick`. Decline = no counter
  declared. The whole `DetourPick` is the arg.
* `counteract_with_plan(player_id, pick)` receives it. Until
  **OPEN-afterglow式移动** (docs/rulebook/spec/OPEN-QUESTIONS.md:224) is
  resolved it keeps the inlined C# plan mutations and the existing
  `TODO(规则书)[judgement]`; once the book names which (2), the body becomes
  `ctx::invoke_skill(player_id, PLANS[pick.plan as usize])` plus the
  「并将下一次的移动掷骰变更为1d6」 half.

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

* **`Guard::Asks` is not shipped** (the uncommitted `guard-prompt` worktree,
  2026-10-23, proposed `Guard::Asks(fn(i32) -> Guarded)` with
  `Guarded = Result<bool, Prompt>`). It is the yes/no case of `declare`, and
  keeping it would be a second spelling for one rulebook concept (「可选择」).

  The decisive argument is not "less machinery" -- it is that **an asking
  residual poisons every eligibility probe.** Probes (`can_counteract` /
  `cant_play` / `ai_play`, and `query_guard` itself) wrap the guard so a pause
  surfaces as `RuleError::GuardPrompted`, which every caller folds to "not
  eligible". So a card whose 「可选择」 ask lives in its *guard* answers
  `false` to `ctx::query_guard` -- i.e. it looks inapplicable to *other cards*
  that are only asking "may this fire?", when in fact it would fire the moment
  someone asked the player. That is exactly mechanism B's motivating shape
  (`detour`: "can I activate depends on can that (2) activate?"), and an
  asking guard makes it answer **wrong**.

  With the ask in `declare` and the residual guard staying pure
  (`Option<fn(i32) -> bool>`, no enum whose only variant is `Pure`),
  `query_guard` answers "may I press this?" truthfully and never predicts the
  player -- the same split `cant_play` already has, which does not predict
  whether the player will actually play the card.

* **No `On::Skill`.** A skill's press is `On::Play`; its passives are
  `On::Action`. `declare` and `query_guard` attach to the entries that exist.
* **No direct Rust call into another card's body.** `ctx::invoke_skill` /
  `ctx::play_card` (nested `On::Play` body) and `ctx::send` remain the only
  cross-card effect seams; `query_guard` is the query half next to them.
* **No `declare` on `On::Filter`** (§2) and **no hand-window one-gesture
  labels** in v98.

## 6. Implementation map (rev 2 — generic `T`, named entry ids)

| layer | change |
|---|---|
| `rules/card-sdk/src/lib.rs` | `ActivationArg`, `Activation<T>`, `Declared<T>`, `BodyShape`; entry structs gain `id: &'static str` + `declare` (erased) + `declare_drive` + body `drive`; `On::id` / `On::declare_arg` builders. **`FilterEntry` gains no `declare`.** `Guard` is `Option<fn(i32) -> bool>`. |
| `rules/card-sdk/src/payload.rs` or `declare.rs` | `drive_declare::<T>` / `drive_declared_arg::<T>` — the **only** place the erased fns are cast back |
| `rules/card-sdk/src/kinds.rs` | same for `ActionEntry`; `filter_*` unchanged |
| `rules/card-sdk/src/rt.rs` | `OP_DECLARE` arm returning packed `(ptr<<32)|len` of postcard, or `EXIT_DECLINED` / `EXIT_NEED_INPUT`; `OP_RUN` drives `BodyShape` (the drive pulls the arg) |
| `rules/card-sdk/src/abi.rs` | `ABI_VERSION = 98`; `export::ON` **stays 4 params**; `OP_DECLARE`; `EXIT_DECLINED`; `export::DECLARED_ARG`; `ManifestOn::{has_declare, id}`; v98 changelog |
| `rules/card-sdk/src/ctx.rs` + `ctx/sys.rs` + `sys_kani.rs` | `declared_arg::<T>()`, `query_guard`, `entry::PRESS`; **delete `card_replayable`** |
| `rules/card-sdk/src/kani_host.rs` | `Op::declared_arg`, `Op::entry_admits`; Kani passes an unconstrained `T` so a proof sees every pick |
| `tools/build-ruleset.mjs` | reject `declare` on `On::Filter`; reject a duplicate `id` within a card; **reject a non-slug `id`** (`[A-Za-z0-9_][A-Za-z0-9_.:-]*`) so identity cannot be dressed up as display text |
| `crates/game-rules/src/hostfns.rs` | `query_guard` host (named-entry lookup + `admits_gate`); `declared_arg` host (serve the stored bytes) |
| `crates/game-rules/src/host.rs` | `Call::{Play,Counteract}` carry `arg: Vec<u8>`; `entry_admitted` returns `Admitted` with the `next_answer` cursor *after* `admits` + `OP_DECLARE`; `on_trap` follow-up unchanged |
| `crates/game-rules/src/wasm_rules.rs` | `finish_offer` runs `OP_DECLARE` before `declare_one` commits; `build_round` stores `arg: Vec<u8>` on the chain link; `resolve_jobs` serves it to `OP_RUN`; `play` path runs `OP_DECLARE` before `play_from_hand` |
| `crates/game-rules/src/native_shims.rs` + `crates/rules-native/src/lib.rs` | `op_entry_admits`, `op_declared_arg`, the `on` mirror |
| `rules/cards/card-ag/src/detour.rs` | queries the press entry via `entry::PRESS`; `Declared<DetourPick>` carrying at least `plan` (+ `tile`/`seat` as the plan needs); guard / option list via `query_guard(…, entry::PRESS)` |
| `rules/skills/skill-personas` | `.label("（2）")` on each Afterglow press entry -- `entry::PRESS` already addresses it; label is display, not identity |
| `rules/fixtures/test-cards` | one fixture with a **struct** arg (pick + tile + seat), one with `i32`; a named-entry pair for `query_guard`; the trap fixture |
| `rules/tests` / `crates/game-rules/tests` | every existing `declare_arg` case, plus: the body sees a **struct** with a tile; `query_guard` true/false by id; querying an unnamed or wrong id answers `false` (the ambiguity is gone by construction) |
| `docs/CARDS.md`, `docs/GUARDS.md` | pointers here |

`build-ruleset.mjs` must re-run after any `rules/cards` edit (the ruleset wasm
is staged; see the standing note). Do **not** set `CARGO_TARGET_DIR` for it.