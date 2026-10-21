# Card & event rules

Official card behaviours are WASM card modules (the authoring choice: **wasmi +
`card-sdk`, one module per card family**), executed inside the match by
[`WasmRules`](../crates/game-rules/src/wasm_rules.rs). The engine shell
(`game-core`) never holds card text; every message is a key (see
[I18N.md](I18N.md)).

The same authoring shape covers the other rule categories: board tiles
(`rules/tiles`, [TILES.md](TILES.md)), tile marks (`rules/markers`, `mark:*`
-- [TILES.md](TILES.md) → 「Board marks」) and event cards (`rules/events`,
[EVENTS.md](EVENTS.md)). An event's `CardDef` id is `event:<id>` and its body is
the same `On::` table -- `On::Play` for what happens on draw, `On::Action` for
what runs while it is active, `On::Action(Action::AtEnd(...))` for expiry.

## How a card runs

A card effect is straight-line Rust. When it needs a player decision it calls an
`ask_*` helper: the host aborts the run, the engine raises the prompt like any
other (`Cx::ask`), and after the answer the run replays from the same world
snapshot -- so dice re-roll identically and the effect reaches the prompt again,
this time with an answer. No coroutines, no callbacks; the RNG lives in the
world copy the host runs against.

## Authoring template

```rust
// rules/cards/card-roselia/src/press.rs
use bandori_std::ctx;
use card_sdk::{key, CardDef, Filter, Action, Msg, On};

/// C# `CardPress` -- 压: gain 1,000.
pub const PRESS: CardDef = CardDef::new("R:[衍生] 压", &[
    On::Play("", "", None, press),
    // On::Play("", "", Some(cant_play), press),          // C# `Card.WhyNot` gate
    // On::Counteract(&[ChainKind::Pay], "", r#"你将要失去或支付资金"#, guard, counteract),  // [反击]: kinds + condition + reason + guard + effect
    // On::Action(Action::TurnEnd("", "", guard, decay)),          // field hooks, auto-run in play
    // On::Action(Action::AtEnd("", "", None, at_end)),        // scheduled turn-end body
    // On::Filter(Filter::DiceRollPlan("", None, roll_plan)), // shapes the main move
    // On::Settle(settle),                            // a tile rule's settle body (docs/TILES.md)
]);
fn press(player_id: i32) {
    ctx::gain(player_id, 1000, &Msg::new(key!("why_press")));
}
```

Every handler is a `fn(player_id: i32)` (the player the card is placed at for hooks);
`On::Counteract` carries its [反击] trigger kinds + the condition + a pure `bool`
guard + the effect, `On::Action` one event-hook kind (`passTile`, `drawn`,
`turnEnd`, ... -- the kind names ARE the C# `Fx.*` names) and `On::Filter` one
modifier stage (`paySplitAdd`, `buySet`, ...). Constructors come from the
`filter_kinds!` / `action_kinds!` tables (`Filter::DiceRollPlan(pre, guard, body)`,
`Action::TurnEnd(pre, counter_reason, guard, body)`). **One kind per entry** --
a card that listens at two raises declares two entries. Ruleset kinds use their
own namespace (`Filters::MyRewrite`, `Actions::Exile`).

* `id` is the **data** card id from the roster (Chinese; an identifier,
  not display text).
* every string a card produces is a message key: `key!("...")` namespaces it as
  `cards:<crate>.<key>`, and the translation lives in the crate's
  `locales/zh-CN.json` + `locales/en.json`. The message crosses the guest/host
  boundary as `postcard` bytes (ABI v26) -- a serde format, so neither side
  hand-writes an encoding. The engine renders it to each client in that
  client's language.
* names are arguments, not text: `Msg::new(key!("x")).player_id("who", s).tile("tile", t)`
  -- the client resolves them to display names.

## Action vs Filter (do_action / apply_filters) — final architecture

The entry surface is the WordPress `do_action` / `apply_filters` split. The
final `On` set (ABI v93):

| variant | body signature | role | may emit effects? |
|---|---|---|---|
| `On::Play(pre, counter_reason, gate, body)` | `fn(i32) -> Asked` | play from hand (`counter_reason` = `Option<MsgKey>`, the prompt's 「…时」 clause) | yes |
| `On::Counteract(kinds, pre, counter_reason, guard, body)` | `fn(i32) -> Asked` | [反击] at those chain links (`counter_reason` = `Option<MsgKey>`) | yes |
| `On::Message(names, pre, guard, body)` | `fn(i32) -> Asked` | cross-card message | yes (log) |
| `On::Settle(pre, guard, body)` | `fn(i32) -> Asked` | a tile rule's settle body | yes |
| `On::Mortgage(pre, guard, body)` | `fn(i32) -> Asked` | a deed's [抵押] body (v73) | yes (`ctx::apply_mortgage`) |
| `On::Redeem(pre, guard, body)` | `fn(i32) -> Asked` | a deed's [赎回] body (v73) | yes (`ctx::apply_redeem`) |
| `On::Buy(pre, guard, body)` | `fn(i32) -> Asked` | a deed's buy / perform body (v80) | yes (`ctx::pay_buy`, `ctx::assign_deed`, `ctx::raise`, `ctx::log`) |
| `On::Build(pre, guard, body)` | `fn(i32) -> Asked` | a deed's build / perform body (v80) | yes (`ctx::pay_build`, `ctx::raise`, `ctx::log`) |
| `On::Action(ActionEntry)` | `fn(i32) -> Asked` | **event reaction** at one kind (`counter_reason` = `Option<MsgKey>`) | yes (`ctx::gain`, log, place, …) |
| `On::Filter(FilterEntry)` | shape-dependent, see below | **pure value / decision / gate / payload rewrite** at one kind | no (pure) |

`On::Mortgage` / `On::Redeem` / `On::Buy` / `On::Build` stay their own
variants (like `On::Settle`): a deed body owns eligibility + the amounts and
calls `ctx::apply_mortgage` / `ctx::apply_redeem` / `ctx::pay_buy` /
`ctx::pay_build` / `ctx::assign_deed`, so it is not a pure value transform.
Wire ids 9 / 10 (master's v73) and 12 / 13 (v80).

`On::Action` / `On::Filter` are **one kind per entry** (every multi-kind list
was split). Constructors come from the `action_kinds!` / `filter_kinds!` macros
in `rules/card-sdk/src/kinds.rs`; the body shape is pinned by the constructor
(`FilterShape`), so a filter body can never be registered on the wrong kind
and there is no `check_kind_variant`. `On::Counteract` still takes a list of
`ChainKind` (a [反击] may answer several chain links).

### Filter body shapes (`FilterShape`)

| shape | body | used by | fold |
|---|---|---|---|
| `Value` | `fn(i32, i32) -> i32` | scalar modifiers (`PaySplitMul`, `BuyAdd`, `TurnBaseTime`, …) | host writes the returned figure (`set_pay_amount` / `set_price` / `set_move_roll` / clock seconds) |
| `Flag` | `fn(i32, i32) -> bool` | pure decision stages (`PaySplitChoose`, `PayCombinedCancel`) | **bool polarity:** `false` cancels the pipeline stage |
| `Gate` | `fn(i32, GateValue) -> GateValue` | the 10 decision gates (`ActGate`, `BuyGate`, `ImmuneAll`, …) | `GateValue { allowed: Some(false), reason }` refuses; `allowed: None` leaves the host's answer |
| `Payload` | `fn(i32, P) -> P` | typed rewrites (`PlanValue`, `RollValue`, `DealValue`, `PayAddValue`, `RedirectValue`) | host writes back only the fields the body changed (`None` = leave alone) |

Integer pipelines (pay amount, buy price, turn-clock seconds, roll face) are
the scalar fold -- one number in, one number out. Cancel stages
(`PayCombinedCancel`) and the decision gates are booleans. `PaySplitChoose` is
a Flag; a card that both rewrites the amount and declines is **split into two
entries**. `ChargePaying` is **not** a filter: its 「可」 opt-in prompts, so it
stays `On::Action`.

`BuyAssign` and `DiceRollPlan` do not carry a single integer -- they rewrite a
small record (deal owner/houses/mortgaged; roller/dice/…). Those use
`FilterShape::Payload` with `DealValue` / `PlanValue`. Fields the body leaves
as `None` are not written back.

### Retired wire ids (never reuse)

| id | was | folded into | ABI |
|---|---|---|---|
| 4 | `OnKind::AtEnd` | `On::Action(Action::AtEnd(...))` | v76 |
| 5 | `OnKind::RollPlan` | `On::Filter(Filter::DiceRollPlan(...))` | v76 |
| 6 | `OnKind::Gate` | `On::Filter(Filter::ActGate(...))` / `On::Filter(Filter::DiceRollPlan(...))` at the gate kinds | v77 |

`On::Hook` (the pre-v74 name) is gone; `OnKind::Hook = 3` was renamed
`OnKind::Action` and keeps id 3. `OnKind::Filter = 11` (not 9 -- 9/10 are the
v73 deed kinds).

### On::Settle is kept (not folded into SettleBody)

`On::Settle` is the tile rule's **base settle body** (what runs when that tile
is [结算]d). `ActionKind::SettleBody` is a field hook that may **cancel / replace**
that body (`set_cancelled`). They are two roles in one chain link:

1. raise `SettleBody` -- a placed card may claim the body (`set_cancelled` = "do
   my thing instead");
2. if not cancelled, run the tile rule's `On::Settle`.

Folding would make the tile body just another `Action(SettleBody)` and lose the
"one base body, optional replacement" shape (and the `[反击]`-able
`ChainKind::SettleBody` link). **Decision: keep `On::Settle`.**

### Filter payload types (the minimal clean set)

Per-kind payloads live in `rules/card-sdk/src/payload.rs`. `None` = leave the
host's answer alone; there is no `UNCHANGED` sentinel and no `i32::MIN`
encoding. One `FilterPayload` impl each, flattened to `i32` only at the
`hostfns.rs` wasm-import edge.

| type | body | kinds | fields |
|---|---|---|---|
| `GateValue` | `fn(i32, GateValue) -> GateValue` | the 10 gates | `allowed: Option<bool>`, `reason: Option<MsgKey>` |
| `PlanValue` | `fn(i32, PlanValue) -> PlanValue` | `DiceRollPlan` | `roller` / `base` / `extra` / `add_base` / `reverse` / `move_kind` / `teleport_to` / `resolve` / `clear_dice` / `min_roll` / `tag: Option<(MsgKey, i32)>` / `recorded_roll` |
| `RollValue` | `fn(i32, RollValue) -> RollValue` | `DiceRollValue` | `value: i32`, `recorded_roll: Option<i32>`, `resolve: Option<bool>` |
| `DealValue` | `fn(i32, DealValue) -> DealValue` | `BuyAssign` | `owner: Option<Payee>`, `houses: Option<i32>`, `mortgaged: Option<bool>` |
| `PayAddValue` | `fn(i32, PayAddValue) -> PayAddValue` | `PaySplitAdd` | `value: i32`, `payee: Option<Payee>` |
| `RedirectValue` | `fn(i32, RedirectValue) -> RedirectValue` | `Redirect` | `payee: Option<Payee>` |

`GateValue { allowed: None, reason: None }` leaves the host's answer untouched.
`allowed: Some(false)` refuses (and `reason` is the denial message key). Scalar
folds (`Value`) and pure decisions (`Flag`) carry no struct -- the `i32` / `bool`
is the whole payload.

### Typed kinds (`ActionKind` / `FilterKind`) — v90–v93

`TriggerKind { ns: KindNs, id: i32 }` is the **wire identity** of every raise
point. `KindNs::Engine` / `KindNs::Ruleset(key)` separate the id spaces so a
rule-defined kind can never collide with a future engine id -- or with another
ruleset's ids. `ActionKind` / `FilterKind` wrap `TriggerKind` and are what the
typed constructors pin; `ChainKind` is the `TriggerKind` alias `On::Counteract`
takes.

| view | `On` variants | what it is |
|---|---|---|
| `ActionKind` | `On::Action(ActionEntry)` | event reactions (`fn(i32) -> Asked`) |
| `FilterKind` | `On::Filter(FilterEntry)` | pure value / decision / gate / payload rewrites |
| `ChainKind` | `On::Counteract` | [反击] window selectors (still a list) |

**Spelling.** Engine kinds serialize as camelCase (`"turnStart"`,
`"diceRollPlan"`). Ruleset kinds serialize as `"r:{key}:{id}"` (e.g.
`"r:bandori:21"`). Bare decimals (`"21"`) do not parse. CEL `kind` is a
**string** (`VarTy::Str`) comparing these spellings; `move.kind` stays int
(`MoveKind`).

**Registries.** `filter_kinds!` / `action_kinds!` emit a PascalCase `const fn`
per kind plus `Filter::KINDS` / `Action::KINDS` (`&'static [KindInfo]`) for pin
tests and tooling. The constructor type pins the side -- there is no
`check_kind_variant`. The engine table (ids, spellings, `is_filter`) lives in
`crates/game-kinds`; the DSL constructors live in `rules/card-sdk/src/kinds.rs`.

**Dual-role ids** (one wire id, two typed views) are ids used both as a
[反击] chain link and as an Action-listened event. They have an `ActionKind`
const *and* a `ChainKind` value. From the current codebase:

`DiceRollBefore`, `Effect`, `Bankrupt`, `Card`, `Event`, `MoveBefore`,
`MoveAfter`, `Pass`, `PassBefore`, `PassTile`, `Settle`, `SettleBefore`,
`SettleBody`, `MarkerSpent`, `CardUsed`, `HouseAdded`, plus the rule-defined
`PassRewardAffected` (26) and `ChargePaying` (78). `Effect` (72) is the
canonical example. No Filter kind is a ChainKind.

**Rule-defined kinds** live in `rules/bandori-std/src/kind.rs` and join the
Action side today. Four are genuinely rules/-emitted (raised from `rules/` with
this game's vocabulary):

```rust
pub const PassRewardAffected: ActionKind = ActionKind(TriggerKind::ruleset(RULESET_KEY, 26));
pub const Exile: ActionKind = ActionKind(TriggerKind::ruleset(RULESET_KEY, 21));
pub const ChargePaying: ActionKind = ActionKind(TriggerKind::ruleset(RULESET_KEY, 78));
pub const FansFlip: ActionKind = ActionKind(TriggerKind::ruleset(RULESET_KEY, 107));
```

Three universal raises moved to the engine namespace in ABI v90 (they cover the
whole mechanic, not one game's flavour): **`markerSpent` (73)** any named-counter
spend commits; **`cardUsed` (74)** a field card's `On::Play` runs;
**`houseAdded` (75)** a deed's house count is written. The bandori spellings
`FireSpent` / `SkillUsed` / `HouseAdded` and `"r:bandori:73/74/75"` are gone.

A future rule-defined **filter** kind declares `FilterKind::ruleset(RULESET_KEY, id)`
via `filter_kinds!` in the same file. The fixture is `Filters::MyRewrite`
(`rules/bandori-std/src/kind.rs`, wire `"r:bandori:1"`): a ruleset filter kind
exercised end to end (declaration, registration, dispatch) with no game
semantics.

**Kind-ownership boundary.** Engine kinds are defined in `crates/game-kinds`;
ruleset kinds in `rules/bandori-std`. Each side treats the other's ids as
opaque tokens -- never inspects them, never invents one.

`HookKind` and `GateKind` are retired (no alias). Gates that used to be
`GateKind` (`ImmuneAll`, `ActGate`, …) are `FilterKind` consts: `On::Gate` is
long gone and those questions are `On::Filter(Filter::ActGate(...))` entries.

### Purity

Filter bodies may not prompt and may not call mutating `ctx` effects.
`card_sdk::pure::refuse_if_filter` debug-asserts (and tests enforce) every
mutating call: `gain` / `draw` / `place_card` / `set_dest` / `set_slot` /
`inc_slot` / `roll` / `set_prop` / `state::set` / `plan::set_tag` /
`add_counter`. They **may** `ctx::log` (the why-line of the effect-applied
log-why rule). A fused hook (value rewrite + gain/place/…) is split into a
Filter half and an `On::Action` half on the appropriate event (e.g. `PayAfter`).

A Filter body that would have rolled dice reads `RollValue.recorded_roll`
(v78: the host pre-rolls into the recorded RNG stream so replay stays
deterministic). A named plan tag rides `PlanValue.tag: Option<(MsgKey, i32)>`
(rewritten through `plan::set_tag(key, value)`).

**The one exception (user decision 2026-10-17, keep it):** an `On::Action`
at `DiceRollAfter` may prompt, roll dice, and write the face (trigger value
read back). Every other reshape is the pure `DiceRollValue` filter. See the
filter-kind section below.

### Interactive pre-stages (v78)

A prompt cannot live in a Filter. Where the rulebook puts a 「可」 offer at a
modifier stage, the prompt rides an `On::Action` at a **pre-stage** kind raised
immediately before the pure filter stage:

| pre-stage (Action) | pure stage (Filter*) | rulebook position |
|---|---|---|
| `PaySplitChooseBefore` (105) | `PaySplitChoose` | 支付阶段, before the choose filter |
| `PayCombinedAddBefore` (106) | `PayCombinedAdd` | 分摊前资金增减, before the total-add filter |

`ChargePaying` stays a plain Action (its 「可」 opt-in is the whole window).

### Filter kinds

The engine kind table in `crates/game-kinds` (`ENGINE_KINDS`) is the **single
declaration source of truth** for ids, spellings, and `is_filter`. The typed
mirror is `Filter::KINDS` / `Action::KINDS` in `rules/card-sdk/src/kinds.rs`.
Today (27 filter kinds):

* payment amount stages -- `PaySplitChoose`, `PaySplitAdd`, `PaySplitMul`, `PaySplitAt`,
  `PayCombinedAdd`, `PayCombinedMul`, `PayCombinedCancel`
* buy price / deal stages -- `BuyAdd`, `BuyMul`, `BuySet`, `BuyAssign`
* turn clock -- `TurnBonusTime`, `TurnBaseTime`
* roll / plan shaping -- `DiceRollPlan`, `DiceRollValue` (v75)
* decision gates (v77) -- `ActGate`, `PayGate`, `MoveGate`, `ExclusiveUse`,
  `PlayGate`, `BuyGate`, `AbnormalGuard`, `ImmuneAll`, `Untargetable`,
  `Redirect`

`DiceRollValue` (id 102) is raised **immediately before** `DiceRollAfter` with the
same payload: the pure roll-reshape stage. `DiceRollAfter` stays an Action event
(dice already landed) and no longer rewrites the face.

**Documented exception (user decision 2026-10-17, keep it):** an
`On::Action` at `DiceRollAfter` **may** prompt, roll dice, and write the face
(trigger value read back). That is the one exception to the filter-purity
rule — the only Action that may reshape a roll. Prefer the pure
`DiceRollValue` filter when no prompt is needed; use the Action half only
when the rulebook puts a 「可」 offer on the reshape.

`ChargePaying` (bandori-std kind 78) is **not** a filter kind: its 「可」
opt-in prompts and Filter bodies cannot. It stays on `On::Action`.

Everything else is `On::Action` (turn/pass/draw/settle/buy/build/counter/…
event points, including the engine kinds `MarkerSpent` / `CardUsed` /
`HouseAdded` and the ruleset kinds `PassRewardAffected` / `Exile` /
`ChargePaying` / `FansFlip`).

**Kind/variant agreement is a compile error:** the `Filter::` / `Action::`
constructor type pins the side, so a filter body can never be registered on an
action kind and vice versa. There is no `check_kind_variant`. `On::Counteract`
still takes a list of `ChainKind`.

### Combined vs Split payment levels (v82)

A payment COMMAND (e.g. 「其他玩家[分摊][支付]2000」) first runs the
**Combined** stages on the WHOLE figure before 「[分摊]」 divides it; then
each settled leg/share runs the **Split** stages. A single non-split payment
runs both levels once each -- the names name the LEVEL, not whether a split
happened.

| level | kinds | when |
|---|---|---|
| Combined | `PayCombinedAdd` / `PayCombinedMul` / `PayCombinedCancel` (+ `PayCombinedAddBefore` prompt) | once per payment command, on the figure before any 「[分摊]」 divide |
| Split | `PaySplitAdd` / `PaySplitMul` / `PaySplitChoose` / `PaySplitAt` (+ `PaySplitChooseBefore` prompt) | on each settled leg/share (and once for a non-split payment) |

### Host application order

The host raises the pipeline stages in rulebook order and folds each stage's
return before the next raise, within a stage in placement order:

* pay: `effect` declaration → `payCombinedAddBefore` (Action) → `payCombinedAdd` →
  `payCombinedMul` → `payCombinedCancel` → `paySplitAdd` → `paySplitMul` →
  `paySplitChooseBefore` (Action) → `paySplitChoose` → `paySplitAt` → `pay` settlement
* buy price: `BuyAdd` → `BuyMul` → `BuySet`, each floored at 0
* roll: `DiceRollPlan` → dice → `DiceRollValue` → `DiceRollAfter` → `diceRollResult` ([反击] reroll window).
  `DiceRollAfter` is the one Action that may still roll and write the face
  (documented exception above); everything else reshapes purely at `DiceRollValue`.

Money rounds **up to a multiple of 10 once**, after every add/mul modifier
has rewritten the running figure (user ruling 2026-10-15). An amount no
modifier touched stays exactly as the base formula produced it.

### v76: DiceRollPlan / AtEnd fold

`On::RollPlan` is `On::Filter(Filter::DiceRollPlan(pre, guard, body))` (the
plan is a `PlanValue`). `On::AtEnd` is `On::Action(Action::AtEnd(pre, reason,
guard, body))`: the one-shot-ness lives in the existing `schedule_turn_end`
list (`ctx::at_turn_end` / `at_next_turn_end`), not in the entry variant. Wire
ids `OnKind::AtEnd = 4` / `OnKind::RollPlan = 5` are **retired** and must never
be reused.

**ChargePaying stays Action.** Its 「可」 opt-in prompts and Filter bodies
cannot. Everything else that used to be a gate is a Filter.

**CEL-only status gates** (`ActGate` / `PayGate` / `MoveGate` /
`ExclusiveUse` / `PlayGate`) keep their native path: the condition is the
whole answer, evaluated by `gate_open` / `admits_pre` with no wasm. The
`On::Filter(Filter::ActGate(...))` body only runs if the hook path asks.

## Authoring a condition (`pre`, ABI v45)

A guarded entry (`On::Counteract`, `On::Action`, `On::Play` gates) carries a
**condition** string immediately before the residual guard -- category →
condition → guard → body, the three-layer model of
docs/[GUARDS.md](GUARDS.md) §4.3. `""` = no condition. The condition is a CEL
expression over the §4.2 window/candidate vocabulary; it is compiled **once**
at ruleset build and evaluated natively before the wasm guard is ever
instantiated.

```rust
use card_sdk::{pre, On, Action};

// condition (`pre: &'static str`) before the guard
On::Counteract(&[ChainKind::DiceRollValue], "actor == owner && move.roll != null",
               r#"你将要失去或支付资金"#, can_counteract, counteract),

// sugar: `pre::MINE` = `actor == owner`
On::Action(Action::TurnEnd(pre::MINE, "", guard, decay)),

// or the const builder
On::Play("", "", Some(cant_play), play).pre("owner.money >= 1500"),
```

**Distinct layers (nothing is rejected twice).** The category filter (the
`On::*` kind) owns the trigger kind; the condition owns everything
expressible in the schema; the wasm guard owns only the residual. A guard must
not restate its condition. `kind` is an ordinary string variable
(`VarTy::Str`, comparing spellings), so an entry may still *branch* on it --
never use it to reject what the category filter already rejected.

Fail-closed: a parse error, an unknown variable/function, or a float literal
(the schema is int-only) fails `build-ruleset` / `RulesetBuilder::build` with
`RuleError::BadPre`. An absent condition means "no clauses beyond the guard".

The compiled lean form (`Cond::to_bytes(false)`) rides the ruleset so the
browser glue evaluates with `rules-cond`'s `runtime-only` build and never ships
the CEL parser.

## Silent bookkeeping entries (`CardDef::silent`, ABI v53)

Most entries flash the card and write a 「效果适用」 header when their body
runs. Some entries are **bookkeeping**: a mechanical declaration or reset the
player never sees as a clause firing. Those run their body but post no
`"card"` event and no header -- mark them with `CardDef::silent(&[entry, …])`
(indices into the `&[On]` table, 0-based):

```rust
pub const TOMOE_RAMEN: CardDef = CardDef::new(
    "skill:宇田川巴:豚骨酱油拉面大姐",
    &[
        On::Play("", "", Some(can_use), use_skill),                         // 0: 「发动」 -- flashes
        // 「初始1，上限1」 -- declares the fire-pot cap; invisible setup
        On::Action(Action::TurnStartBefore("", "", None, declare_cap)),     // 1: bookkeeping
        On::Action(Action::DeckAtGameStart("", "", None, declare_cap)),     // 2: bookkeeping
        On::Action(Action::TurnEnd("", "", Some(afterglow), tick)),         // 3: 「获得[火罐]」 -- flashes
    ],
)
.silent(&[1, 2]);
```

**When to use it.** An effect the rulebook describes as happening (gain, pay,
place, move, draw, status, mark, stun, exile, deal, build, destroy) flashes.
A mechanical declaration or reset does not. Mark silent:

* `declare_cap` / `fire_pot` / `state::set_bounds` / initial values /
  `DeckAtGameStart` / `DeckBeforeGame` declarations that only install caps or
  starting counts;
* `TurnStartBefore` re-arms that only reset counters or flags;
* latch clears and once-per-turn flag resets;
* `AtEnd` expiries that only clear state without a rulebook-visible effect;
* empty bodies kept only so an entry exists (`Ok(())`).

**When not to use it.** If the body can pay / move / draw / place / mark / log
on any path, leave it flashing (or split the bookkeeping half into its own
silent entry). The flag exists to hide the flash, not the effect: a silent
entry that produces a visible result is a bug, and the host's debug guard
(`game_rules::take_silent_visible` / `debug_assert!`) records it. Nested
activations from a silent body still announce normally.

Path: `CardDef::silent` → `ManifestOn::silent` (ABI v53) →
`announce_drive` skips the `"card"` post and the 「效果适用」 header; the body
runs the full category → pre → guard layering unchanged.

## Flash policy (`CardDef::flash`, ABI v55)

Beside the header mode (Visible / Silent / QuietWhenEmpty), each entry carries
a **flash policy** -- whether the activation's centre flash (and field-slot
pulse) shows:

* `Flash::Auto` (default) -- today's host classification: owner-self-only
  effects keep the log entry but no flash; anything that reached another
  player, the board, or movement flashes.
* `Flash::Always` -- the rule says this entry always flashes, even an
  owner-self-only effect the player must notice.
* `Flash::Never` -- the log entry stays; no flash / pulse, even for a
  cross-player effect.

```rust
pub const T: CardDef = CardDef::new("skill:…", &[
    On::Play("", "", None, use_skill),                  // 0: Auto
    On::Action(Action::TurnEnd("", "", None, tick)),    // 1: Always (self-only)
]).flash(&[(1, Flash::Always)]);
```

**Precedence** (highest first): `Flash::Never` > `ctx::no_flash()` >
`ctx::flash()` / `Flash::Always` > `Auto`-by-scope. A body may override at
runtime:

* `ctx::flash()` -- request the flash for THIS activation at this point
  (overrides `Auto`; a no-op under `Never`). Posts a `"flash"` cue (once-per-
  flash) and flushes the lazy 「效果适用」 header first -- a flash is a visible
  outcome and the header names which clause is firing.
* `ctx::no_flash()` -- suppress for this activation (decided at runtime).
* `ctx::flash_card(card_id, &Msg)` / `ctx::flash_tile(tile, &Msg)` -- a
  proactive "look here" cue as its own `"flash"` event, parent-linked under
  the activation. Usable by pseudo cards (tiles, `mark:*`).
* `ctx::flash_card_keyed(key, card_id, &Msg)` -- like `flash_card` with an
  explicit identity key whose position can shift between runs.

**Once-per-flash** (user, 2026-10-11): a flash is identified by `(drive,
ordinal within the drive)` or an explicit key. Once posted to the stream, a
body re-run after a prompt must NOT post or play it again. Unlike logs
(replace-by-index, dropped on halt), a flash already shown before the prompt
stays shown. The identity lives in the drive's persistent state and survives
halt; the client dedupes by event id so a re-delivered frame never replays a
flash.

**QuietWhenEmpty interaction**: a requested flash counts as visible -- if
`ctx::flash()` ran, the header is kept.

Path: `CardDef::flash` → `ManifestOn::flash` (v55) → `commit_after` stamps the
`"card"` event's `flash` flag under the precedence above. Proactive `"flash"`
events play through the client's cardFlash path.

## Bot-only estimated execution cost

`prop::EST_COST` (`"estCost"`) is a card's **estimated execution cost** (user
ruling 2026-10-07): what activating the card is expected to cost the player, in
资金. Read **only** by bots / autopilot as a reserve check -- never by legality.
Constant for now; an X-dependent cost may later become a `rules-cond`
expression. Declare it with `CardDef::new(...).props(&[(prop::EST_COST, N)])`.
`0` (the default) means unknown / assume free.

```rust
pub const GACHA10: CardDef = CardDef::new("通用:10次招募（1回限定）", &[On::Play("", "", None, gacha10)])
    .props(&[(card_sdk::abi::prop::EST_COST, 1500)]);
```

Filled for cards with fixed payments (the six formerly money-gated ones plus
the other flat `ctx::pay(N)` bodies); leave it at `0` for computed costs.

## Declared card properties (`CardDef::props`)

Some behaviours are **static properties of the card rule**, not effects that
run at a trigger. They are a generic **property map** -- named `key -> i32`
values declared on the `CardDef`, keyed by constants so the names are never
stringly scattered:

```rust
use card_sdk::abi::prop;
// 规则书[持续]（1）: 「手卡上限数量减1」 -- C# `Card.HandLimitDelta`
pub const CUT: CardDef = CardDef::new("PP:不要背负期待", &[...])
    .props(&[(prop::HAND_LIMIT_DELTA, -1)]);
// 规则书: 「（此卡可在眩晕时打出）」 -- C# `Card.PlayableStunned`
pub const H: CardDef = CardDef::new("MyGO:壱雫空", &[...])
    .props(&[(prop::PLAYABLE_STUNNED, 1)]);
```

The keys the engine reads live in **one place** each side (the two crates
cannot share a definition, so they are mirrored -- keep them in step):

* `card_sdk::abi::prop` -- what a card rule declares against;
* `game_core::state::prop` -- what the engine reads back.

| key | constant | rulebook clause | C# | what the engine does with it |
|---|---|---|---|---|
| `handLimitDelta` | `prop::HAND_LIMIT_DELTA` | 「手卡上限数量减1」 (`-1`) | `Card.HandLimitDelta` | stamped onto the field instance at placement (`FieldCard::props`) and summed into the owner's hand limit while it sits there; gone with the card. A `place_raw` test arrangement sees it too. |
| `playableStunned` | `prop::PLAYABLE_STUNNED` | 「（此卡可在眩晕时打出）」 (`1`) | `Card.PlayableStunned` | the card skips the stun gate in `cannot_play`. The exile and no-hand gates have no such exception in the pool. |
| `song` | `prop::SONG` | 「卡名为歌名的卡」 (`1`) | -- | a song-title card (Kizuna Music's X). A ruleset-level marker other rules read via `ctx::card_prop`; the engine itself never reads it. Declared on exactly the twenty song cards. |

**Tile-rule properties** (the same map, on a *board-owned* tile rule instance --
[TILES.md](TILES.md)): `price` / `house` / `group` / `buildMax` / `rentLen` /
`rent:N` / `ringMult` are the tile's data, stamped at `bind_tiles`; `noReward` /
`rentFactor` / `payFactor` / `buyDiscount` / `freeBuy` / `razeOnBuy` / `noBuild`
are the modifiers a card writes in place of the engine flags those used to be.
Two write surfaces, matching the two clause shapes (ABI v32):

* `ctx::prop` / `ctx::set_prop` -- the **running** instance (a per-player veto
  riding a placed card: PPP band (2) 「无法获取[CiRCLE奖励]」, 梦在前方's
  「[拥有者]不可盖房」). Read back by scanning the passing / building player's
  field.
* `ctx::tile_prop` / `ctx::set_tile_prop` -- the rule instance(s) governing a
  **tile** (a tile-scoped or walk-scoped veto: 「[经过]CiRCLE时不获得[CiRCLE奖励]」
  arms `prop::NO_REWARD` on CiRCLE's `tile:circle` instance).

`prop::NO_REWARD` is the CiRCLE-reward veto (`tile:circle`'s Pass entry reads
and consumes the tile-scoped one); `prop::NO_BUILD` gates `why_not_build_on`;
`prop::RENT_FACTOR` / `PAY_FACTOR` scale a settlement payment at the money
pipeline's `paySplitMul` stage. A rule body's own instance is the one running
(`ctx::self_tile()` names its tile).

**Defined default: `0`** for every key -- a card that does not declare a
property reads as `0`, and `!= 0` is how a boolean-ish key like
`playableStunned` is tested.

Path: `CardDef::props` → `ManifestEntry.props` (a key-sorted
`Vec<(String, i32)>`, so the wire bytes are deterministic; ABI v30) →
`CardInfo::props` (a `BTreeMap<String, i32>` -- not a `HashMap`, so iteration
and serialization stay deterministic) → `CardRules::card_props(card)` /
`::card_prop(card, key)` → stamped onto `FieldCard::props` at placement.

Each declaration cites its clause like any other line (see below). The current
holders: `PP:不要背负期待`, `PP:梦在前方，结彩当下`, `PP:练习生解密指南` and
`Sumimi:#L12` declare `(prop::HAND_LIMIT_DELTA, -1)`; `MyGO:壱雫空` declares
`(prop::PLAYABLE_STUNNED, 1)`; the twenty song-title cards declare
`(prop::SONG, 1)`.

## Card metadata (ABI v63 / v86) -- data lives with the rule

The deck-builder / gallery / home-screen rows (cards / characters / bands) are
**authored beside the rule code** and exported from the ruleset. There is no
flat `data/{cards,characters,bands}.json` any more: the Rust metadata is the
single source (cutover complete; equivalence to the old JSON was verified at
commit `7c3d115`).

* **Cards** -- `CardDef::meta(&CardMeta::new()...)` on the card's `CardDef`
  (`rules/cards/card-*/src/*.rs`). Carries `name` / `rawName` / `text` /
  `tags` / `derived` / `cell`, plus a generic `fields` map for the game slots
  `group` and `owner` (`bandori_std::roster::meta_key`). Multi-line CJK text is
  a raw string literal -- byte-exact newlines, full-width punctuation and
  trailing spaces round-trip untouched.
* **Characters** -- `bandori_std::roster::PersonaDef` beside each character
  skill (`rules/personas/src/*.rs`), collected in that crate's
  `roster_table::PERSONAS`.
* **Bands** -- `bandori_std::roster::GroupDef` beside each band skill
  (`rules/skills/skill-groups/src/*.rs`), collected in `roster_table::GROUPS`.

`card-sdk` stays vocabulary-free: `CardMeta` is the generic card-row shape and
`MetaRow` / `MetaDoc` the wire form. `PersonaDef` / `GroupDef` name this
game's concepts and live in `bandori-std` / the skill crates.

### Display columns leave `ruleset_metadata` (ABI v86)

Display-only columns (rule text, sheet cells, art / costume / logo ids, accent
colours, short labels, CN ids) are still **authored** on the same defs -- the
`.text(...)` / `.display(...)` / `.color(...)` / `.art(...)` / … builders stay
-- but they no longer ride `ruleset_metadata`. The runtime blob carries only
the engine's roster fields (card `name` / `group` / `owner` / `derived` /
`tags`; persona `name` / `group` / `skill` / `exclusiveCards` / `preset` /
`variantOf`; group `name` / `skill` / `sharesExclusives`).

The display columns are exported at build time into
`webui/src/generated/display.json` by `tools/export-display.mjs` (wired into
`webui`'s `predev` / `prebuild`). Pipeline, field list and regenerate command:
`docs/DISPLAY.md`.

### Transport

`ruleset_metadata` (`export::METADATA`) returns a packed postcard
`meta::MetaDoc` -- **not** through `ManifestEntry`, so the match path and the
per-card manifest stay lean. The blob is built only when the host asks (load
time, never per run). Host: `Ruleset::metadata()` / `::roster()`, then
`game_core::data::GameData::load(get_file, roster)`. Native
(`rules-native`): `rules_native::roster_metadata()` / `::roster()`.

### Canonical order

The Rust crates register cards alphabetically inside each band, while
`GameData` iterates the roster in its own order (deck pools, presets).
A shared generated table (`rules/bandori-std/src/roster_order.rs`) maps
id → ordinal; the export sorts by `(kind, ordinal)` so the rebuilt vectors
equal the roster order exactly.

### Fixture gate

The lift is **complete** (184 cards / 54 characters / 12 bands) and the
`tools/lift-roster.mjs` lifter is retired (deleted; see `docs/ROSTER-TOOLS.md`
for the dump tooling). The test fixture `crates/game-core/tests/fixtures/
roster.json` is a generated dump of the Rust roster (one
`{cards, characters, bands}` document), used by
crates that have no ruleset (game-core / game-rules / bot-core / server) and by
`webui/src/core/cards.test.ts`. Regenerate it with:

```sh
cargo run -p rules-native --example dump_roster -- --fixture crates/game-core/tests/fixtures/roster.json
```

`cargo test -p rules-native --test roster_equiv` is the gate: the fixture must
be byte-equal to a fresh dump (ordering / field fidelity included). Equivalence
to the retired `data/{cards,characters,bands}.json` baseline was verified at
the cutover (commit `7c3d115`, the last tree with those files as the baseline).

### Cutover complete: consumers take the roster from the ruleset

Runtime hosts no longer read `data/{cards,characters,bands}.json` (those files
are deleted). The roster is ruleset metadata end to end:

| Host | Roster source |
| --- | --- |
| `crates/server`, `crates/rules-worker` | `Ruleset::roster()` after loading `dist/cards` (load the ruleset first -- it does not need `GameData`) |
| `crates/web-glue` | `Ruleset::roster()` / `roster_json()`; `load_data` pairs it with the remaining `DATA_FILES` |
| `webui` (`core/data.ts`, workers) | `glue.roster_json()` after `ruleset_build`, before `load_data` |
| `crates/bot-glue`, `crates/bot-service` | ruleset / `rules_native::roster()` |
| tests / examples (game-core, game-rules, bot-core, server, rules-worker) | `Roster::from_fixture` over `crates/game-core/tests/fixtures/roster.json` |
| `webui/src/core/cards.test.ts` | the same fixture for `text` / `tags` |

`GameData::load(read, roster)` is the one constructor. `DATA_FILES` is the
remaining two engine tables (board / match_rules). The data hash
(`EngineStamp::data_sha256`) covers exactly those; the roster (cards /
personas / groups / events) is covered by `ruleset_sha256` (the metadata lives
in the modules that hash names).

Display-only overlays live in `webui/src/generated/display.json` (rule text,
sheet cells, art / costume / logo ids, and the card `counterReason` list).
The engine does not read them.

Tools that need JSON rows use `tools/roster.py` against a dump written by
`tools/roster-dump.mjs` (see `docs/ROSTER-TOOLS.md`); they do not read
`data/{cards,characters,bands}.json` either. The other `data/*.json` files
(tiles / events / schools / voice / home / board / match_rules) stay JSON.

## The vocabulary (`card_sdk::ctx` vs `bandori_std::ctx`)

**Split (ABI v56).** `card-sdk` is only generalized card-runtime mechanisms
plus the guest ABI wire codes. This ruleset's names (火罐 / 奇迹水晶 / CiRCLE /
RiNG / Live House / band skills / rent-buy verbs) live in `bandori-std`.
Card bodies import `use bandori_std::ctx` (a facade that re-exports the
generic surface and the game sugar).

Generic tile queries (host ABI v56):

* `tile_is(tile, "ring"|"circle"|"agent"|"property"|…)` -- data `TileKind`
* `tile_attr(tile, "group"|"area"|"tier"|"price"|"house"|"rent"|"index")` -- i32 field
* `tile_in(player, tile, "buyable"|"shop"|"livehouse"|"color:<g>")` -- ruleset
  class; per-player overrides are class-membership props (`class:` /
  `classFor:<p>:`) plus the existing `anyColor` / `colorFor:<p>`

Game sugar in `bandori_std::ctx` (kept readable for card code):
`is_ring(t) = tile_is(t, "ring")`, `is_circle` / `is_agent` likewise;
`is_buyable` / `is_shop` / `is_live_house` / `is_live_house_for` / `is_color`
are `tile_in` classes.

| group | functions |
|---|---|
| dice & log | `roll`, `log` |
| board (generic) | `tile_count`, `tile_named`, `tile_owner`, `player_pos`, `tile_steps_ahead`, `rent_of`, `buy_price`, `build_cost`, `mortgage_value`, `owned_tiles`, `tile_is`, `tile_attr`, `tile_in`, `tile_group`, `tile_price`, `houses_of`, `set_houses`, `add_house`, `mortgaged_of`, `set_mortgaged`, `set_owner`, `dist`, `tile_forward`, `neighbor`, `players_on` |
| board (game sugar) | `is_buyable`, `is_shop`, `is_ring`, `is_circle`, `is_live_house`, `is_agent` |
| players & money | `player_count`, `player_out`, `others`, `money`, `gain`, `pay`, `can_pay`, `cant_move`, `turn_player`, `round_no`, `turn_key`, `gains_this_turn` (「当前回合内你每获得过一次资金」 -- the engine's per-turn money-in counter, reset at each turn start). Game: `character_is`, `in_band` |
| targeting | `target`, `target_all`, `target_tile`, `targeted_count`, `designations` (the **static targeting query** -- which players the play being resolved designates, C# `H.Db.Card(id).Targeting` + `H.Others`; empty when it names nobody), `cancel_designation` / `designation_cancelled` (per-pair cancel, C# `play.Tags["immune"+seat]` -- one designation drops, the rest land) |
| hand & deck | `draw`, `add_to_hand`, `add_to_deck`, `to_discard`, `hand_count`, `hand_size`, `discard_count`, `deck_count`, `discard_size`, `discard_from_hand`, `sweep_to_deck`, `add_to_deck_at` (`DeckPos::{Top,Bottom,Random}`), `cards_in(player_id, CardPile)` (list a pile; deck top first), `take_card(player_id, CardPile, id)` / `take_from_hand` (remove without discarding) |
| marks & tokens | `place_mark`, `place_mark_new`, `count_marks`, `bump_mark`, `remove_marks`, `mark_src_at`, `count_held`, `add_held`, `move_units`, `tok`, `set_tok`, `add_tok` -- every mark / token is a unit of the creating instance's named counter (`MarkFilter` selects by kind / category / owner / src / instance); see 「Named counters and bound units」 |
| tile designations | `designate_tile`, `undesignate_tile`, `designated_tiles`, `designates` -- a [指定] of tiles while a field card / skill is in play is a mark of [`abi::mark::DESIGNATE_CATEGORY`] owned by that instance (see 「Tile designations」) |
| named counters (ABI v50) | `counter` / `add_counter` / `set_counter` (this instance), `counter_at` / `add_counter_at` (by uid), `card_counter` / `add_card_counter` (a placed card by id). Wire names `cp` / `crystals` map to `FieldCard::cp` / `FieldCard::crystals`; anything else lives in `FieldCard::counters`. The old CP helpers (`place_cp` / `count_cp` / `clear_cp` / `add_cp` / `cp_attached`) are **gone** -- use the mark / counter API, and see 「Cross-card messages」 for the 该清CP了 ↔ `mark:cp` shape |
| per-player slots | `slot`, `set_slot`, `inc_slot` |
| pots & status (generic) | `give_stay`, `give_stun`, `give_exile`, `give_extra_turn`, `stay_of`, `stun_of` |
| pots & status (game) | `band_crystals`, `add_band_crystals`, `fire`, `fire_max`, `gain_fire`, `spend_fire`, `crystals` / `set_crystals` / `add_crystals` / `decay` |
| skills & band attachments (ABI v35) | `band_skill` / `character_skill` (the bound rule id, C# `H._fx[i].bands` / `.skill`), `band_skills` (every band attachment as `(uid, id, extra)`), `add_band_skill` (C# `H.MakeBand`; `extra` = 「拿取」 copy: 「相同乐队技能卡的效果不可叠加」 / 「不视为那个乐队的角色」), `invoke_skill` (run a skill rule's press entry `On::Play` for a player -- 「立即执行乐队技能的（2）效果」 / `SkillPareo -> Offer()`. No `skillUsed`: that is the player's own press (`use_skill`).) |
| ring | `ring_multiplier`, `add_ring_bonus`, `teleport_to` |
| prompts | `ask_yes`, `ask_pick`, `ask_tile`, `ask_tiles` (labels + prices + AI hint), `ask_player`, `ask_card`, `ask_number` |
| nesting & trigger | `play_card`, `invoke_skill`, `raise_bought` (C# `f.Bought(i, t)` -- a card that handed a deed over announces it), `trigger::{kind, player_id, target, tile, value, step, by_card, move_roll, set_move_roll, set_pay_amount, set_pay_target, set_cancelled, cancelled, card_is, move_flags, move_is_main, move_dir}` |
| purchase (ABI v40/41) | `buy_quotes(player, kind, &[tile])` (batched quote), `buy(player, tile, kind)` (replaces `card_buy`), `acquire(player, from, tile, price)` (「收购」: pipeline pay, then assign → `bought` → `buyAfter`), `agent_offer`, `linger(player, expires)` (bind the running def as a turn-scoped instance in `TurnCtx.lingering` -- the hand-card home for 「本回合」 effects; a `set_prop` before it lands on the instance's props), `buy_price(t)` (the deed's base value). `trigger::{buy_kind, seller, price, set_price, deal_owner, set_deal_owner, deal_houses, set_deal_houses, deal_mortgaged, set_deal_mortgaged, set_reason}` carry the `BuyGate` / `BuyAdd` → `BuyMul` → `BuySet` / `BuyAssign` payload (`docs/PURCHASE.md`). |
| field cards | `place_card`, `place_card_at`, `unplace_card`, `is_placed`, `set_dest`, `placed_tile`, `crystals`, `set_crystals`, `add_crystals` (sugar over this instance's named `'crystals'` counter), `decay` |
| cross-card messages (ABI v51) | `send(Target, name, &Message)`, `On::Message(names, pre, guard, body)`, `ctx::message::{sender_uid, sender_seat, name, a, b, c, tile, seat, text, reply}` -- see 「Cross-card messages」 |
| tile rules | `self_tile`, `prop`, `set_prop`, `tile_prop`, `set_tile_prop`, `prop_at`, `set_prop_at`, `draw`, `draw_event`, `pay_rent`, `offer_buy`, `offer_build`, `offer_force_buy`, `buy`, `buy_quotes`, `card_build`, `ask_tiles`, `ai_agent_choice`, `is_color`, `raise`, `gain_typed`, `exile_of`, `stun_of`, `card_settle_at` (the settle / pass primitives; `docs/TILES.md`) |
| event rules | `event_expire`, `event_is_active`, `event_deck_push`, `event_banish` (the active-list / deck handles; `docs/EVENTS.md`) |

Prompts can carry an AI preference (`ask_tiles`'s `ai` hint, or the prompt
fallback when it is -1).

## Named counters and bound units (ABI v51)

Every tile mark and player token is a **unit of some card instance's named
counter**, bound to a tile or a holder; destroying the instance destroys its
units (user ruling 2026-10-10). There is one model, not two:

* **On-card pool** -- the count on the instance itself. The two known names
  keep their wire names and storage: `counter::CP` (`"cp"`) → `FieldCard::cp`
  (「自己[场上]N个[CP点]」), `counter::CRYSTALS` (`"crystals"`) →
  `FieldCard::crystals` (奇迹水晶). Anything else lives in
  `FieldCard::counters[name]`.
* **Tile marks** (`TileMark`) -- units bound to a tile. `TileMark.instance` is
  the owning counter's card instance; `TileMark.src` stays **provenance**
  (which card instance placed it). A mark dies with its instance, not with its
  placer.
* **Held tokens** (`Counter`) -- units bound to a holder (a player).
  `Counter.instance` is the owning counter's card instance.

The guest surface is the generic verb set:

```rust
// on-card named counter of this instance
ctx::counter("cp");
ctx::add_counter("cp", 6, 0)?;          // max 0 = uncapped; raises CounterChanged
ctx::set_counter("cp", 0);
// another instance / a placed card by id
ctx::counter_at(uid, "cp");
ctx::add_counter_at(uid, "cp", -1, 0);
ctx::card_counter(player, "通用:该清CP了", "cp");

// tile marks, selected by MarkFilter (kind / category / owner / src / instance;
// empty / mark::ANY = any)
ctx::place_mark(tile, "mark:cp", "cp", -1, self_uid(), 1, &note);
ctx::count_marks(tile, &MarkFilter::any().category("cp"));
ctx::bump_mark(tile, &filter, -1);
ctx::remove_marks(tile, &filter);
ctx::mark_src_at(tile, &filter);        // provenance (TileMark.src)

// holder-bound units of this instance's counter
ctx::count_held("抹茶芭菲", player);
ctx::add_held("抹茶芭菲", player, 1, 0)?;
ctx::move_units("抹茶芭菲", -1, other, space, -1, 1);
```

`tok` / `add_tok` / `set_tok` are the holder-binding verbs of the creating
instance's counter (`add_tok` = `add_held`). `crystals` / `set_crystals` /
`add_crystals` are sugar over this instance's named `'crystals'` counter;
`band_crystals` / `add_band_crystals` are sugar over the band-skill field
instance's. The CP helpers (`place_cp` / `count_cp` / `clear_cp` / `add_cp` /
`cp_attached`) are **gone**.

SAVE_VERSION 5 → 6: `TileMark.instance` / `Counter.instance` are new
(serde default `-1` for legacy rows; a row whose creator cannot be determined
is kept, never auto-purged). `Match::restore` attaches CP marks to the standing
`mark:cp` and other rows to the creating rule's field instance when it can be
found.

One `CounterChanged` hook (wire names `counterChanged`|`crystalsChanged`|
`cpChanged`) replaces `CpChanged` / `CrystalsChanged`; it carries
`Trigger.name` = the counter name and `Trigger.value` = the signed delta.
「…时」 clauses on a count (「此卡上不再拥有[奇迹水晶]时」, 通用:该清CP了's
graveyard rule) live in one event handler on that hook, not at each spend site.

## Tile designations ([指定] a tile)

A field card / skill that 「指定…格子 / 地契 / 地块」 while it is in play records
each pick as a **designation mark**: `mark_category::DESIGNATE`
(`"designate"`), `mark_kind::DESIGNATE` (`"mark:designate"`), `owner` = the
designating seat (the label's colour), `instance`/`src` = the designating
card or skill uid. `TileMark.card` is the designating card id (filled from
the instance), which is what the board label and tooltip show.

Lifetime matches 「指定」 lasting while the card is in play: the mark is a unit
of the designating instance's counter, so `destroy_instance_units` clears it
when that instance leaves the field. A designation that ends **earlier** (a
state exit, a turn boundary) calls `ctx::undesignate_tile` at that point.

```rust
ctx::designate_tile(tile, player_id, &Msg::new(key!("note")))?;
// later, if the designation ends before the card does:
ctx::undesignate_tile(tile);
// the pick as the source of truth:
if ctx::designates(tile) { .. }
for t in ctx::designated_tiles() { .. }
```

`designate_tile` is sugar over `place_mark` with the designation category and
`Stack::Fresh` (one label per pick). Prefer it over ad-hoc slots / state keys
when the mark can be the source of truth; where the rule needs a value the
mark does not carry (a counterpart player, an engine-owned return tile), keep
that storage and add the mark as the displayed record.

## Cross-card messages (ABI v51)

`ctx::send(Target, name, &Message)` is the generic cross-card channel.
`On::Message(names, pre, guard, body)` is the answering entry -- same
condition / residual guard / body layering as `On::Action`. The handler reads
the sender and payload from `ctx::message::*` and returns a reply value via
`ctx::message::reply(v)`; the sender's `ctx::send` returns it (0 when no
receiver admitted the message). Handling a message does not flash the card.

Addressing (`abi::Target`): `Uid(i32)` (a specific field instance),
`Card { player, card }` (the instance of a card id on a seat), or
`Board(String)` (a standing board-owned pseudo card, e.g. `mark:cp`).
Multiple receivers of a name: the first admitted one in field order answers;
a handler whose `pre` / guard rejects is not a receiver.

`Message` is a few typed ints plus an optional name -- `name`, `a`, `b`, `c`,
`tile`, `seat`, `text`. `ctx::message::{sender_uid, sender_seat, name, a, b,
c, tile, seat, text}` read them; `Trigger.name` is the message name (the same
field a `CounterChanged` hook uses for the counter name).

### 该清CP了 ↔ `mark:cp` (before / after)

通用:该清CP了 seeds tile [CP点] and an on-card stock; `mark:cp` owns the tile
marks and the landing clause that spends both kinds
(`rules/cards/card-general/src/clear_cp.rs`, `rules/markers/src/cp.rs`).
The shape:

**Before** -- 该清CP了 called the CP helpers directly and listened to
`ActionKind::CpChanged` (retired; now `CounterChanged`):

```rust
// 通用:该清CP了
On::Play("", "", Some(cant_play), clear_cp),
On::Action(&[ActionKind::CpChanged], "", "", Some(cp_empty_guard), on_cp_empty),

fn clear_cp(player_id: i32) -> Asked {
    // ...
    ctx::place_cp(tile);            // tile [CP点], stamped onto this instance
    ctx::add_cp(6, 0);              // on-card [CP点] stock
    // ...
}
fn cp_empty_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::card_is(ID) && ctx::cp_attached() == 0
        && trigger::value() <= 0
}
```

**After** -- 该清CP了 `ctx::send`s a message to `Target::Board("mark:cp")`
asking it to place CP units; `mark:cp`'s `On::Message` handler runs
`ctx::place_mark`. The landing hook spends the placer's on-card CP via
`ctx::add_counter_at`. The graveyard rule is an `On::Action` on
`Action::CounterChanged` filtering `counter_is('cp')` /
`card.counter('cp') == 0`:

```rust
// 通用:该清CP了
On::Play("", "", Some(cant_play), clear_cp),
On::Action(Action::CounterChanged("", "", Some(cp_empty_guard), on_cp_empty)),

fn clear_cp(player_id: i32) -> Asked {
    // ...
    place_tile_cp(tile);                        // the message, below
    ctx::add_counter(counter::CP, 6, 0)?;       // on-card [CP点] stock
    // ...
}

/// Ask the `mark:cp` standing rule to put `count` [CP点] on `tile`.
fn place_tile_cp(tile: i32) -> Asked {
    ctx::send(
        &Target::Board("mark:cp".into()),
        "place",
        &Message {
            name: "place".into(),
            tile,
            a: 1,                               // 「添加1个[CP点]」
            ..Default::default()
        },
    )?;
    Ok(())
}

fn cp_empty_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::card_is(ID)
        && trigger::name() == counter::CP     // or counter_is('cp') in `pre`
        && ctx::counter(counter::CP) == 0
        && trigger::value() <= 0
}

// rules/markers/src/cp.rs
On::Message(&["place"], "", Some(places_cp), place_cp),
On::Action(Action::SettleBody("", "", Some(lands_on_cp), on_land)),

fn place_cp(_owner: i32) -> Asked {
    let tile = ctx::message::tile();
    let src = ctx::message::sender_uid();       // provenance 「此卡在格子上添加的」
    ctx::place_mark(
        tile, mark::CP_KIND, mark::CP_CATEGORY,
        /* owner */ -1, src, ctx::message::a().max(1), &Msg::new("log.cp_placed"),
    );
    ctx::message::reply(1);
    Ok(())
}

fn on_land(_owner: i32) -> Asked {
    let seat = trigger::player_id();
    let tile = trigger::tile();
    let src = ctx::mark_src_at(
        tile,
        &MarkFilter::any().category(mark::CP_CATEGORY),
    );
    ctx::bump_mark(
        tile,
        &MarkFilter::any().category(mark::CP_CATEGORY),
        -1,
    );
    // 「自己[场上]1个[CP点]」 -- the placer's on-card stock
    if src >= 0 {
        ctx::add_counter_at(src, counter::CP, -1, 0);
    }
    ctx::gain(seat, 800, &Msg::new("log.cp_clean").player_id("who", seat))?;
    Ok(())
}
```

The landing clause (settle on a CP tile: remove the tile unit + one on-card
CP of the placer, gain 800) lives on `mark:cp`; the spread clause (「此卡在
格子上添加的[CP点]及其产物」) keys on `TileMark.src` provenance the same way
it does today.

### PP fans in the per-player state dict (ABI v83)

The PP fan counters are two keys in the **per-player state dict**
(`ctx::state`, `MatchPlayer.state`), owned by **no card**:

| key | meaning |
|---|---|
| `ppFansUp` | face-up fans (`P㏄P粉丝(正)`) |
| `ppFansDown` | face-down fans (`P㏄P粉丝(反)`) |

Key constants live once in `rules/skills/skill-groups/src/fans.rs`
(`skill_groups::fans::{UP, DOWN}`). Every grant / flip goes through
`fans::{grant, flip_up, flip_down}`; readers use `fans::{up, down, total}`.
CEL pre strings read them as `slot('ppFansUp')` (the existing 1-arg
per-player state accessor). The webui lists non-zero state values in the
player marker strip like tokens.

`fans::flip_up(seat, x)` is the one flip helper for 「将X个反面…变正」. It
raises the **ruleset-defined** Action kind `FansFlip`
(`bandori_std::kind`, wire 107, `trigger::value()` = the effect's intended
X) **before** the dict writes, so a listener still sees the pre-flip
face-down count. A cancelled / aborted raise (`Ok(false)` / `Err`) leaves
both counters untouched. `flip_down` does not raise (clause （2） is only
「变正」).

Band clause （2） lives on `skill:Pastel㏄Palettes:与偶像一起` as
`On::Action(Actions::FansFlip("actor == owner", …))`: when `X >` pre-flip
face-downs it adds `min(X - down, 10 - held)` 奇迹水晶 to **this card** and
logs the amount actually added (`pastel_over`). 「最多10个」 caps the
card's **TOTAL** crystals at 10 (user ruling 2026-10-17), not the overflow
amount — a pile already at/over 10 (clause （4）'s uncapped tick can
overshoot) gets nothing and is never force-reduced. 「…时」 state clause =
event handler on the change, not an inline check at each flip site. No band
card on the board → the raise still moves the counters; only the crystal
grant is silently absent.

That replaces the retired `FlipEffect` / `FlipAfter` pair (TriggerKind
94/104, never reuse).

## Trigger points (`trigger::kind()`)

Every point the engine raises is a pair: a `*Before` half that fires before the
thing happens (so a counteraction can block or rewrite it) and a `*After` half that
fires after it commits. A few legacy kinds from the original port kept their
original names where the pairing is already implied (`settleBefore` →
`settle` → `settleAfter`, `pay` → `paid`, `diceRollBefore` → `diceRollResult`).

| pair | when |
|---|---|
| `turnStartBefore` / `turnStart` | a turn begins -- before / after the exile & stun status ticks |
| `diceRollBefore` / `diceRollResult` | the main roll -- before the d20 is cast / after it (after `diceRollAfter`, 行动阶段 10 「移动掷骰后」), before walking. `diceRollResult` is a [反击] counteraction window; a counteraction may reroll via `set_move_roll` |
| `passBefore` / `pass` / `passTile` | each tile stepped over (CiRCLE and the destination) -- before / after the player arrives. `passTile` is the [经过] step (行动阶段 12); a 「被[经过]」 clause reads the tile being entered. `passPlayer` is the end-tile [重叠] only |
| `moveBefore` / `moveAfter` | the move head / tail (ABI v43, `SETTLE-STAGES.md` §7). `moveBefore` fires once the plan is fixed and before the first step / the teleport -- counteractions that cancel or alter the move go here. `moveAfter` is 「移动后」/「主要移动结束时」/「[移动终点]」: after `passPlayer`, before `settleBefore`, for every completed move including a 「不触发结算」 one |
| `settleBefore` / `settle` / `settleBody` / `settleAfter` | landing -- before resolving the tile / before its effect / the settle's effect list (a field card may replace it with 「将本次结算改为…」) / after it fully resolves |
| `mortgageBefore` / `mortgage` | mortgaging a deed -- before any guard (can block) / after it applied |
| `pay` / `paid` | money leaving a player -- before the deduction / after. `player_id` = payer, `target` = payee, `value` = amount. `paid` is the [反击] window and opens only on a payer-side loss (再次牵起手来 / 游击演出 are 「[消耗]或[支付]」 / 「被…收取资金」). `payAfter` is the 「资金变动」 hook (rulebook 支付阶段 7, 「合并到[支付后]」) and fires on **any** money change, a print (`gain`) and a `gain_fixed` included |
| `payCombinedAdd` / `payCombinedMul` / `payCombinedCancel` | the **command-wide pre-split** stage (「分摊前」, `PIPELINE-AUDIT` Q2): `payCombinedAdd` (fixed ±) → `payCombinedMul` (×) → `payCombinedCancel` (drop the whole command) shape the figure **before** any 「[分摊]」 divides it into shares. A single-pair payment's command total is its own amount, so the three run there too. `trigger::value()` / `set_pay_amount` rewrite the total; `set_cancelled` on `payCombinedCancel` drops the command. The per-share counterparts are `paySplitAdd` / `paySplitMul` / `paySplitChoose` / `paySplitAt` |
| `tileResolved` / `moveResolved` / `bankruptResolved` | terminal points (「结算完成时」, `PIPELINE-AUDIT` Q6). `tileResolved` fires after `settleAfter` (and after a cancelled settle); `moveResolved` after a move and any settle it asked for; `bankruptResolved` after the seat is cleared and the leftover auctions finish. `payAfter` / `buyAfter`+`bought` / `eventAfter` are already the terminals of their pipelines |
| `bankruptBefore` / `bankrupt` | bankruptcy -- before asset cash-in / after cash-in, before removal from the game. The seat is marked **dead before `bankruptBefore`** (B3), so the dying player's own sources cannot join that window; other players' hooks still fire and may inspect the remaining state. `bankruptResolved` is the terminal |
| `card` / `cardAfter` | playing a card from hand -- before its `play` body / after its `Dest` handling |
| `event` / `eventAfter` | drawing an event -- before it resolves / after it is filed away |
| `buyBefore` / `buyAfter` | buying a deed -- before any guard (fires even on a no-op attempt) / after the deed changes hands |
| `buyGate` / `buyAdd` / `buyMul` / `buySet` / `buyAssign` | the purchase surface (`docs/PURCHASE.md`). `buyGate` (an `On::Filter(Filter::BuyGate(...))` gate, runs for **every** `BuyKind` -- Force included) refuses a buy with `GateValue { allowed: Some(false), reason }`. `buyAdd` (fixed ±) → `buyMul` (×) → `buySet` (free / fixed) are the quote's price stages, each floored at 0, rewriting the running `trigger::price()`. `buyAssign` runs at commit and rewrites `deal_owner` / `deal_houses` / `deal_mortgaged` before the one ownership write. `bought` / `buyAfter` still do not fire for Force or Auction (rulings 6 / 7, undecided). |
| `buildBefore` / `buildAfter` | building a house -- before payment / after the house commits (not on the refund path) |
| `discardBefore` / `discardAfter` | discarding from hand -- before the card leaves / after it is in the pile |
| `endTurnBefore` / `endTurnAfter` | ending a turn -- the player's command / any turn end, including stun & exile auto-skips |
| `leaveBefore` / `leaveAfter` | forfeiting -- before any guard / after the player is cleared, before the game-over check |
| `deckBeforeGame` / `deckAtGameStart` | the two match-start points. `deckBeforeGame` = **before match start**, before the opening hands are drawn: start positions and the authoritative initial hand size (`ctx::inc_start_hand`, default 2, minimum 0) are decided here. `deckAtGameStart` = **after match start**, after the opening deal and mulligan: initial tokens/resources (fire pots 「初始N」, P✽P fans). Both dispatch to **every effect source** of the player they are raised for -- field cards including skills (per player, in field order) and the card ids in that player's piles/hands -- each source running its own hook with `t.card` naming it. There is no separate "match started" point; nothing in the pool wants "after positions are set, before the deal". |
| `drewBefore` / `drawn` + `drew` | the per-draw points, **one raise per single card** (an N-card draw is N iterations, each payload naming one card). `drewBefore` is the *replacement* point: `t.card` is the deck's top, and a hook that replaces the draw calls `trigger::set_cancelled()` and performs its own look/pick -- whatever it adds to the hand is the replacement draw (「此次加手视为抽卡动作」) and the after points fire for it. After the card is in hand: `drawn` runs on the **drawn card itself** (C# `AfterDraw`), `drew` on the **field cards** (crystal-per-draw effects). Opening hands raise **none** of the three -- 「抽卡」 means a draw during play, and 朝同一片天空迈进's 「（开局时抽到此卡洗回）」 is a `deckAtGameStart` clause for exactly that reason. |

`diceRollBefore` carries `value = -1` as a "no roll yet" sentinel so roll-counteracting cards
(which match `DiceRollBefore | DiceRollResult`) stay dormant before the dice are cast.

## Every line cites the rule book

The rule book is the spec: `docs/rulebook/cards-sheet.csv` (one column per band,
one cell per card) exported from
[this sheet](https://docs.google.com/spreadsheets/d/1xZ3avBsNBXbl3bQ74lmPs0YQFgZkPD0Sdzmx7ZGGEDY),
plus
[this document](https://docs.google.com/document/d/1jM0aThg70XQ_9wYCubZ6oWRhS_f9SgIpmjpxvt4OlGU)
(band and character skills). `tools/rulebook/extract.py` turns the sheet into
`docs/rulebook/cards.json` (card id -> passage); list markers `（1）（2）…` each get
their own line there.

A card's file carries the passage it implements and every code line cites the
sentence it implements:

```rust
//! 规则书（docs/rulebook/cards.json, id `PPP:Popipa`）:
//! > Popipa：
//! > [手]：
//! > 获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]
...
fn popipa(player_id: i32) {
    ctx::set_dest(ctx::Dest::Banished);          // 规则书[手]: 「然后此卡[移除]」
    ctx::gain(player_id, 1000, &Msg::new(key!("why")));  // 规则书[手]: 「获得1000资金」
    ...
}
```

Rule book text that the code cannot express yet (hooks the ABI lacks) goes in as
`// TODO(规则书): …` naming the missing hook, never silently dropped.

```sh
python tools/rulebook/extract.py   # sheet -> docs/rulebook/cards.json
python tools/rulebook/cite.py      # (re)quote the passage into each card file
python tools/rulebook/check.py     # every card quotes its passage and cites it
```

## Porting checklist

`cardmap.json` (extracted from `MatchHost.cs` line ~18500) maps every C# class to
its data id: 184 cards over 176 classes. Porting a card:

1. read its C# `Play` / `Counteract` / `WhyNot` (MatchHost.cs);
2. write a `CardDef` + body against the vocabulary above;
3. add the message keys to `locales/zh-CN.json` and `locales/en.json`;
4. `node tools/build-ruleset.mjs && python tools/i18n/check.py && cargo test -p game-rules`.

`tools/i18n/check.py` verifies every `key!(...)` has a translation, so a card
cannot ship with a raw key showing to players.

## Status

* seam: card modules run inside real matches (server and browser), prompts come
  out as engine prompts and effects commit -- `crates/game-rules/tests/live_match.rs`;
* vocabulary above is implemented end to end (ABI v35 as of 2026-10-06 --
  `card_sdk::abi::ABI_VERSION`; the wire format note above still says v26
  because that is when `postcard` was adopted);
* **all 184 cards are ported** (`rules/cards/card-*`, one module per card family
  crate, one `.rs` per card). `python tools/rulebook/check.py` verifies every
  card quotes its passage and cites it; `node tools/build-ruleset.mjs` ships
  them as one module (`dist/cards`, 184 cards).
* [反击] cards fire through the hand-counteraction window (`WasmRules::hand_counteractions`):
  every trigger opens one **round per timing** (rulebook 32 / 89), the counters
  settle newest-first before the timing they answer, and a counter is itself a
  new timing that may be answered (depth-capped runaway guard at 16; the C# cuts
  at 4, which is tighter than the card pool needs).
* **Ordering when several cards answer one trigger** (rulebook 89): the ring
  starts at the seat after the player the timing belongs to and runs forward in
  turn order, so that player is asked **last**; each visit a responder declares
  one eligible hand card answering **that timing** or passes (several players may
  counter the same effect, each answering X rather than each other); a
  declaration advances priority to the next responder and the round closes after
  a full consecutive pass. Once the round closes the declared counters become
  new timings, taken **newest first**, each with its own round starting after its
  declarer (clause 89's 「…后可对新的时点发动[反击]」). Resolution is LIFO over the
  resulting answer tree: a counter's own answers settle before it, sibling
  counters settle newest first, and every counter settles before the timing it
  answers. Where this rule does **not** apply (left as-is, flagged for a later
  pass):
  * the acting card's own follow-up (`t.card`'s `counteract`) resolves *before* the
    window, outside the tree -- a card answering its own play is not competing
    with counteractions to it;
  * one declaration per player per visit -- a player with two eligible cards picks
    one and may declare the other when the ring comes back;
  * multi-player declaration order is fixed by player position, not chosen.
* `trigger::step()` is the turn step (0/1/2/3) the trigger fired in. It is only
  informative for kinds that are not already step-specific -- `mortgage`, for
  example, can fire during both step 1 (awaiting roll) and step 3 (settling),
  and `step()` tells those apart.
* **Field-card (`Fx`) hooks** are persistent effects on a placed card. They are
  *not* [反击] points: the engine runs every placed card's `counteract` against them
  **automatically**, in placement order per player, with no player declaration.
  They use their own `TriggerKind`s (`turnEnd`, `drewBefore`, `drew`,
  `passTile`, `payAfter`,
  `diceRollAfter`, `cardPlayed`, `targeted`, `paySplitChoose`) so a card can tell a field
  effect from a hand counteraction by its kind alone -- `match trigger::kind()` is the
  dispatch. Every hook kind is raised except `targeted` (it waits on the
  targeting pipeline). `drawn` runs on the card just drawn (named on
  `t.card`, still in hand) rather than on placed cards; `drewBefore` / `drew`
  are the per-draw field points (one raise per single card). Hook-only kinds open no
  [反击] window; `turnStart` / `settleAfter` are both a hook and a [反击] point,
  so a card there tells which by `is_placed(player_id)`.
  Per-card miracle crystals (`crystals` / `set_crystals` / `add_crystals`) are
  the decay counter `DecayCard.TurnEnd` uses -- sugar over this instance's
  named `'crystals'` counter (`FieldCard::crystals`). **Band-card (「乐队卡 /
  团卡」) crystals are one pool with them**: they live on the band skill's own
  field instance (`skill:<band>:<skill>`), so `band_crystals` /
  `add_band_crystals` are sugar over that instance's count. No keyed-state
  side store. Edge cases: a player with no band skill reads 0 and every write
  is a no-op; several band cards (PPP:Returns) target the first in placement
  order; a swapped or removed band card takes its crystals with it. The
  empty-out clause (「此卡上不再拥有[奇迹水晶]时」) is an `On::Action` on
  `Action::CounterChanged` filtering `counter_is('crystals')`, not a check
  at each spend site.
* A counteraction can reshape the trigger it answered: `set_move_roll` rewrites a
  move roll, `set_pay_amount` reduces or cancels (0) a payment, `set_pay_target`
  redirects its payee (-1 = the bank), and `set_cancelled` / `negate_effect` / `spare` shape what settles. The engine honours all four once the counteraction window closes.
* **[反击] is keyed on the effect, not on an outcome.** `ChainKind::Effect` is
  raised when an effect's recipients are named, before settlement; `ctx::effect`
  lists what that link declared (`count` / `kind` / `target` / `from` / `tile` /
  `value`), so a guard can take the whole list (「被…效果影响」 -- `effect::hits`)
  or one entry (「一次性支付5000以上」 -- `effect.has(Pay)`; `Pay` is a
  string spelling, `kind` is `VarTy::Str`).
  `Target` / `Abnormal` / `Pay` are settlement hooks now and cannot reconstruct
  that clause. A counteraction is a chain link: it resolves **before** the effect and
  may `set_cancelled()` (negate the activation -- the link never happened),
  `negate_effect()` (it happened, settles to nothing), or `spare(seat)` (everyone
  but that seat settles).
* `trigger::by_card()` is the player whose card caused the trigger (`None` when it
  was board-driven, e.g. rent or a buy). This is what 「来自你以外」 checks against
  (C# `H.HitByOtherCard`): `by_card().is_some_and(|by| by != player_id)`. It is
  distinct from `player_id()` -- on a `pay` trigger `player_id()` is the *payer*, not the
  card that forced the payment, so only `by_card()` can tell a card-caused
  payment from rent.
* most cards are **partial**: what the vocabulary cannot express is marked
  `TODO(规则书)` / `TODO(ABI)` naming the missing hook (78 markers remain as of
  the v35 wave -- 2 of them `TODO(ABI)`, and those two are only the convention
  notes in `skill-bands` / `skill-characters`'s `lib.rs`; the card pool's
  `TODO(ABI)` list is empty). What is **landed** since the first cut: the `On::` handler
  form (Play / CantPlay / Counteract / Hook / AtEnd / DiceRollPlan), the field-hook kinds
  (`PassTile`, `PaySplitAdd/PaySplitMul/PaySplitChoose/PaySplitAt/PayAfter`, `SettleAfter` /
  `SettleBody` -- `SettleBody` replaced `SettleInstead` --,
  `TurnStart`/`TurnEnd(+Before/After)`, `Drew/Drawn`, `DiceRollAfter`, `Reshuffled`,
  `Bought`, `Discarded`, `DeckBeforeGame`/`DeckAtGameStart`, `BeforeOut`,
  `Teleported`), per-card `Crystals`, pile enumeration/take
  (`cards_in`/`take_card`/`card_replayable`), the turn-end scheduler
  (`before_turn_end` = C# AtEnd / `at_turn_end` = AfterEnd), the abnormal gate
  (`abnormal_count`, `abnormal_kind`, `AbnormalGuard`, the `abnormal` [反击]
  window), the trigger payload (`by_card`, `pay_is_rent`, `move_flags/main/dir`,
  `step`, pay mutators, `cards()`), and the plan-shaping ops
  (`ctx::plan::*`), and (v35) the **skill / band / follow** surface:
  `band_skill` / `character_skill` / `band_skills` / `add_band_skill`
  (C# `H._fx[i].bands` / `.skill` / `H.MakeBand`; `extra` = 「拿取」 copy),
  `invoke_skill` (run a skill's press entry `On::Play` -- 「立即执行乐队技能的
  （2）效果」, `SkillPareo -> Offer()`), `raise_bought` (C# `f.Bought(i, t)`,
  a card-driven hand-over announcing the acquisition), and
  `plan::add_follower` (「使你的下次主要移动结果对那些玩家一起执行」 -- the
  engine replays the move's result for each follower after the mover settles,
  in the recorded order, carrying the same plan / `pay_factor`).
  What **remains**, from the markers:
  * **`ctx::card_move`** (C# `H.CardMove`: run the move now as the main move) --
    the largest family (~20 markers); plan-shaping is written and waits only for
    this + the plan-readback wiring in `main_move`;
  * plan gaps: `set_start` (start-tile override), `set_teleport_to`
    (teleport-with-settle shaping), multi-die `MoveCtx.Base` tables, and
    in-flight move writeback (counteractions setting `m.Stopped`/`m.ExtraSteps`);
  * **targeting** is landed in v26 (`target`/`target_all`/`target_tile`,
    `ImmuneAll`/`Untargetable`/`Redirect`, `targeted_count`) and, since the
    2026-10-06 group-A wave, the **static targeting query** (`designations`,
    C# `H.Db.Card(id).Targeting` + `H.Others`, declared via
    `prop::DESIGNATES`) and **per-pair cancel** (`cancel_designation` /
    `designation_cancelled`, C# `play.Tags["immune"+seat]` -- one designation
    drops, the rest land). What remains is `PlayCtx` `Effective`/`Extreme`,
    `t.Pay.tile`, the skill system (`TryResonance`, `SwitchState`, ...), and
    host->guest strings (`AbName`/`EventTitle`);
    (`H.ExtraOf` skill attachments landed in v35 as `band_skills`' `extra`);
  * routines (`BuyRoutine`/`BuildRoutine`/`MortgageRoutine`/`SettleAt`),
    `AgentLanding`, `RevealSeen`/`Actions`, world snapshot/restore.

Those are engine-side extensions (a nested `Cx` run inside a host call shares the
answer log; the `Fx` hook set becomes new `CardRules` methods), not rewrites of
what is here. The engine seam is co-developed with two peer sessions -- check
`ListAgents` before editing `game-core`/`game-rules`/`card-sdk`.