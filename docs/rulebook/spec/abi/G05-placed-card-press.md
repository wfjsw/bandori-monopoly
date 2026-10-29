# ABI design G05 — Press surface for placed cards

**Status:** draft for user review · design only, no source touched.
**Worktree:** `.claude/worktrees/card-rulebook-review` (branch `worktree-card-rulebook-review`).
**Group:** G05, family "Press surface for placed cards".
**Overlap:** G13 covers the same nanami row for its **invocation** half; §4.4
adopts G13's import/frame wholesale and only adds the press binding, so the two
docs do not ship two `invoke_entry` shapes.
**Ground truth:** `docs/rulebook/cards.json`; every fragment below is verbatim from it.

Terminology: a **press** is the player activation of a clause printed on a card
that is already on the field (a skill button and a placed card's 「可以…」 clause
are the same mechanism). The request names the **instance** (`uid`) and the
**entry**; nothing is inferred from the card being a skill, from a `skill:`
prefix, or from the card being placed.

**Closed slugs and rows:** the consolidated row's rows plus the shared item
below. `hina-clause-2-swap-missing` is G01's by header, but its clause
(2) swap is a press of clause (2) of a *placed* card — the press entry here is
what makes it addressable, so its rows are repeated and the card migration is
listed in §6.

| # | slug | severity | rows |
|---|---|---|---|
| 1 | `nanami-press-invokes-clause-3` (shared with G13) | medium | 437 |
| 2 | `kanon-press-surface-abi` | high | 380, 955, 967 |
| 3 | `even-lost-not-pressable` | low | 1521 |
| 4 | `placed-card-press-surface` | high | 974 |
| 5 | `hina-clause-2-swap-missing` (press half; G01 owns the uid half) | medium | 487, 488, 490, 1555 |

Two items are **demoted to A**: #2 and #3. Their stated expectation ("appears
pressable and reaches `why_not_act`") is reachable today by dropping the
`skill:` filter in `engine/mod.rs:1315-1317` and deriving `source`; they are
still migrated to the new press entries below because #4 needs the capability
and a filter drop alone re-breaks every hand-only card. Item #1 and #4 are
**B-class**: no filter can express "address this clause of this instance".

---

## 1. Problem, per item

### 1.1 `nanami-press-invokes-clause-3` (row 437)

Book (`Mor:（NNM）稍微努力了一下`, clause (3)):

> （3）将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的
> （2）技能，此次技能不受数量或轮数限制

The press must run the **owner's persona skill clause (2)**. It currently runs
the skill card's `On::Play`, which is that skill's clause **(3)**:

`rules/cards/card-morfonica/src/nanami_effort.rs:266-287`

```rust
    // TODO(规则书)（你可以移除一个奇迹水晶视为发动你的）: the book runs the persona
    //   skill's (2) (pay a pot, gain a mark) -- `play_card` runs its (3) body, which
    //   spends a mark instead.
    ctx::play_card(id.as_str(), player_id)?;
```

`skill:广町七深:这是很普通的事吧？` declares (2) as an `On::Action(Action::CardUsed(…))`
entry (`rules/skills/skill-personas/src/nanami_ordinary.rs:62`) and (3) as its
press `On::Play` (line 50). `ctx::play_card` and `invoke_skill` both resolve
"the card's first `On::Play`", so (2) is unreachable by id. The clause's
「本次技能不受数量或轮数限制」 half (`nanami_effort.rs:276`) is also unwaived
because `play_card` never evaluates the (2) gate.

Spec: `docs/rulebook/spec/cards/mor.md` C-MOR-nnm-04 (line 523-537), OPEN-借用技能
(line 982). Kani row C-MOR-nnm-04: `docs/rulebook/spec/kani/mor.md:75`
(BLOCKED-HOST: "`placed_cards` is a count, not a name list").

### 1.2 `kanon-press-surface-abi` (rows 380, 955, 967)

Book (`HHW:（花音）Wacha Mocha 啪嗒进行曲`):

> [场]花音每次倒走时此卡获得一个水母标记；主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆。

The placed copy's jump is the second half. One `On::Play` serves both a hand
play (place) and a field press (jump), so the gate guesses the origin:

`rules/cards/card-hhw/src/kanon_march.rs:47-52`

```rust
    // TODO(规则书)(ABI)（视为本次主要移动）: the gate cannot tell a hand play
    // from a press -- `ctx::play_from_hand()` is only set once the hand play is
    // committed, so a held copy also admits a below-9 / move-spent press (which
    // `jump` then re-checks for markers only).
    if ctx::hand_count(player_id, ID) > 0 || !ctx::is_placed() {
        return None;
    }
```

and the body branches again (`kanon_march.rs:64-65`). The press surface never
shows the card at all: `view_extra_parts` lists only `skill:*` ids
(`crates/game-core/src/engine/mod.rs:1315-1317`). `TODO(规则书)` at line 79.

Spec: `docs/rulebook/spec/cards/hhw.md` C-HHW-wacha-02 (line 343).

### 1.3 `even-lost-not-pressable` (row 1521)

Book (`MyGO:即使迷茫着`, clause (2)):

> （2）[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次主要移动格数为你当前手牌张数。

The card is never played from hand (it enters via its [反击] (1)); its only
`On::Play` is the press body, and it is on no press surface:

`rules/cards/card-mygo/src/even_lost.rs:20-21`

```rust
        // TODO(ABI): a placed plain-card instance is not offered as a pressable
        // skill -- the view lists only `skill:*` ids, so (2) has no client entry.
```

`can_go` also guards `!ctx::is_placed()` (line 60-63) even though a press is
always placed. Spec: `docs/rulebook/spec/cards/mygo.md` C-MYGO-even-lost-02
(line 127).

### 1.4 `placed-card-press-surface` (row 974)

The umbrella: there is no field-activation identity at all.

* `view_extra_parts` skips every id without the `skill:` prefix
  (`engine/mod.rs:1315`: `if !id.starts_with("skill:") || !self.rules.has_play(&id)`).
* `SkillAction.source` is derived by `strip_prefix("skill:")`
  (`engine/mod.rs:1327-1331`), so a plain card has no source.
* The client's only press sender is `act: "skill"` off that list
  (`webui/src/scenes/board/Side.tsx:218`), and `skillMarks` finds the instance by
  **card id**, not uid (`Side.tsx:91`).
* Both bot AIs read the same list (`webui/src/game/autopilot.ts:694`,
  `crates/game-core/src/engine/ai.rs:386-396`).

Simply dropping the prefix filter is not enough: `CardRules::has_play` is true
for every hand play, so hand-only bodies (`[手]` plays) would appear and rerun
their hand body on a placed copy with no reservation. The row's own
`fix_plan`: "Needs a capability, not a filter tweak".

### 1.5 `hina-clause-2-swap-missing` (rows 487, 488, 490, 1555) — press half

The problem statement is G01 §2.10; on this group's half it is the same
missing surface as 1.4, one level harder: the clause lives on a card that is
no longer on the field (the stand-in), so the press must be found through the
substitute's instance, not through its own card id. `hina_sound.rs:87-96` has
both TODOs — no real-id marker on the placed substitute and no offer — and the
`skill:`-only surface means even a marker today would have nothing to hang the
clause on.

---

## 2. Shared root causes

1. **One `On::Play` is two clauses.** `On::Play` means "play from hand" *and*
   "press on the field"; the body and gate recover which one by
   `is_placed()` / `play_from_hand()` / `hand_count()`. The recovery is
   per-card, unenforced, and wrong with a second copy in hand.
2. **No clause identity on activations.** Activating a placed skill is
   identified by the *card* (`act:"skill"`, `card: id`); the clause inside the
   card is found positionally (`entry(OnKind::Play, None)`). A card whose
   activation is a different entry kind cannot be invoked at all.
3. **No instance identity on activations.** `Ruleset::play` / `cant_play`
   re-find the instance by first match of the id (`wasm_rules.rs:9384-9400`,
   `9076-9081`), so two copies of a card share one press and one marker read.
4. **The offer surface is a prefix convention.** `skill:` is a rule-id
   namespace, not a capability bit; the view and both AI readers hard-code it.

---

## 3. Capability as an invariant

**Entry identity is a clause id, and a press is an entry of a placed instance
addressed by `(uid, entry)` — never a card-wide yes/no and never an origin
guess.**

Corollaries, which the surface below must make true:

1. One entry is one printed clause. A hand play and a field activation of the
   same card are **different entries** (`On::Play` vs `On::Press`), so no body
   branches on `is_placed()` / `play_from_hand()` and no gate consults
   `hand_count()`.
2. A press entry always has a slug `id` (the same identity type
   `docs/ACTIVATION.md` §3 already defines for `query_guard`). Its label
   (「（2）」) stays display text.
3. The pressed instance is the request's `uid`. Dispatch validates that the uid
   is in play, on the presser's field, and carries the requested card id; it
   never re-finds by id. All instance reads (`is_placed`, named counters,
   `crystals`, props, `dest`) answer for that uid.
4. The offer is enumerated: for every field instance, every press entry the
   card declares is offered and gated through the same `why_not_act` a request
   takes, so "shown enabled" ⇔ "accepted". No prefix test remains, except the
   temporary skill-alias arm (§4.6, `has_press`).
5. A press is an activation, not a hook: it is not re-entrancy-marked, it
   raises a card-activation category, and a counteraction may negate it before
   the body.
6. Cross-card invocation names the entry: `ctx::invoke_entry(player, card,
   entry)`. `ctx::invoke_skill` is sugar over it, resolving the card's default
   press entry.
7. Same barrier order as play: gate at offer, `declare` before the point of no
   return, body after; no re-gate at body time.

The invariant is what makes #1 solvable at all: "run the (2) hook" is
`invoke_entry(skill, "2")`; "offer the jump" is a press entry on the instance;
neither is expressible as a card-level flag.

---

## 4. Surface

### 4.1 `card_sdk::lib` — `On::Press`

New entry struct, modelled on `PlayEntry` but without settle `texts` (a press
is never a tile text):

```rust
/// A placed-card **press** entry: the clause a player may activate while the
/// card is on their field. `id` is required.
#[derive(Clone, Copy)]
pub struct PressEntry {
    pub id: &'static str,
    pub pre: &'static str,
    pub counter_reason: Option<MsgKey>,
    pub why_not: Option<fn(player_id: i32) -> Option<Msg>>,
    pub body: *const (),
    pub drive: BodyDrive,
    pub shape: BodyShape,
    pub declare: Option<*const ()>,
    pub declare_drive: Option<DeclareDrive>,
    pub label: &'static str,
    pub flash: crate::abi::Flash,
    pub header: crate::abi::Presentation,
}
```

Constructor, next to `On::Play` (`lib.rs:871`):

```rust
impl On {
    /// Press this placed card: run `body` when the player activates clause
    /// `id` of the instance. `pre` / `why_not` are the gate (the same pair,
    /// same meaning, as [`Self::Play`]); `body` is the effect.
    ///
    /// `id` is a required per-card-unique ASCII slug
    /// (`[A-Za-z0-9_][A-Za-z0-9_.:-]*`). It is the clause's identity for
    /// `ctx::query_guard` and for the press request; it is never display text
    /// (that is `.label("（2）")`). `"press"` stays reserved (see
    /// [`abi::entry::PRESS`]); declare any other slug.
    #[allow(non_snake_case)]
    pub const fn Press(
        id: &'static str,
        pre: &'static str,
        why_not: Option<fn(player_id: i32) -> Option<Msg>>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self { /* PressBody(PressEntry { .. }) */ }
}
```

`On` gains `PressBody(PressEntry)`; the builder macro arms (`on_entry_field!`)
gain `On::PressBody` for `.id`, `.label`, `.counter_reason`, `.declare`,
`.flash`, `.silent`, `.quiet_when_empty`. `On::kind()` maps it to
`abi::OnKind::Press`; `On::triggers()` is empty.

### 4.2 `card_sdk::abi`

```rust
pub enum OnKind {
    // …
    /// v111: a placed card's **press** entry (`On::Press(id, pre, why_not,
    /// body)`). Addressed by `uid` + the entry's required `id`; a press request
    /// raises `cardUsed` (the category `use_skill` raises). Id 15 (4/5/6 stay
    /// retired forever).
    Press = 15,
}
```

`OnKind::from_i32` gains `15 => Self::Press`.

`entry::PRESS` doc changes: it is the **alias** for a card's default press
entry — the first `On::Press` entry, or, for a card not yet migrated, the first
`On::Play` (`play_entry_preferring_gated`). `"press"` stays undeclarable.
`ctx::query_guard`'s doc example is unchanged in spelling, but its **answer can
change**: a card that gains a press entry and also has a Play now resolves
`PRESS` to the press (with its own `why_not`), not to the Play. Cards that read
`query_guard(PRESS)` to decide a hand activation must therefore read their Play
by entry id after migration; the §9 tests pin one such card.

No new exports: a press entry uses the existing `OP_GUARD` / `OP_DECLARE` /
`OP_RUN` exactly as `PlayEntry` does (`rt.rs:230-280`). `rt.rs`'s `OP_GUARD`
arm gains `On::PressBody(PressEntry { why_not, .. })` (identical body to the
Play arm), `declare_of` (`rt.rs:331-345`) gains the variant, and the body union
gains `On::PressBody`. The `On` accessors `condition` / `has_guard` /
`has_declare` / `id_of` / `label_of` / `header_of` each gain a `PressBody` arm
(the same field reads as `PlayBody`).

### 4.3 `ManifestOn` — wire fields

No new fields. `kind: 15` uses the existing shape; `id` is **required** for
kind 15 (`build-ruleset` rejects a press without one, §4.6). `triggers` empty.
`has_declare` / `has_guard` mean what they mean for Play: a press gate that is
CEL-only has `has_guard = false` and the host skips `OP_GUARD`.

### 4.4 Imports (`card_sdk::ctx`, re-exported through `bandori_std::ctx`)

**Shared with G13.** G05 does not respell G13's invocation surface; it adopts
it verbatim and adds only what a press needs (the caller's instance in the
frame). G13 owns the import, the frame, and the cost-paid flag; if both groups
land, G13 §4.1-4.4 is the normative text and this section only adds
`entry::PRESS`'s press-first resolution.

```rust
// crates/card-sdk/src/ctx/sys.rs
pub fn invoke_entry(
    player_id: i32, card_p: i32, card_n: i32, entry_p: i32, entry_n: i32, flags: i32,
) -> i32; // link_name = "ruleset_invoke_entry"
/// The innermost invocation frame, postcard `InvokeFrame`; `(0, 0)` = size probe.
pub fn invoke_frame(out_ptr: i32, cap: i32) -> i32; // link_name = "ruleset_invoke_frame"
```

```rust
// card_sdk::abi::invoke (G13)
/// The caller already paid the entry's cost; the body must not collect it.
pub const COST_PAID: i32 = 1;
```

```rust
// card_sdk::ctx
/// Run one named entry of `card_id` nested, as `player_id`, with its field
/// instance bound. Body only -- it does **not** evaluate the target's gate
/// (ask `query_guard` first when the gate matters); it raises no category
/// (the invoke frame seeds `trigger::card` / `trigger::cards` instead).
/// [`entry::PRESS`] resolves the card's default press entry (first press
/// entry, else first `On::Play`).
pub fn invoke_entry(player_id: i32, card_id: &str, entry_id: &str) -> Result<Dest, Prompt>;
/// `invoke_entry` with the caller having paid the cost (`ctx::invoke_paid()`).
pub fn invoke_entry_paid(player_id: i32, card_id: &str, entry_id: &str) -> Result<Dest, Prompt>;
pub fn invoke() -> Option<InvokeFrame>;
pub fn invoke_paid() -> bool;
```

`ctx::invoke_skill(player_id, id)` becomes guest sugar
(`card-sdk/src/ctx.rs`, no `sys` declaration):
`invoke_entry(player_id, id, entry::PRESS)`.

The host keeps `ruleset_invoke_skill` and `ruleset_play_card` as thin host-side
calls into the same core (G13 §4.2), so a ruleset wasm built before v111 still
instantiates. Their guest declarations are gone from the SDK.

**One correction to G13 §4.2** (source-only, no wire change): the two aliases do
not share `entry::PRESS`. `invoke_skill` uses `entry::PRESS`; `play_card` keeps
a **Play-first** lookup (`entry(OnKind::Play, None).or_else(default press)`),
because `ctx::play_card` on a card that has both entries means "play this card",
not "press it". G13 §4.4's "`PRESS` keeps resolving through
`play_entry_preferring_gated`" becomes "press entry first, `On::Play` fallback".
The frame's `entry` is always the **resolved** entry's declared id (`""` when
that entry is unnamed, e.g. a `play_card` into a bare `On::Play`), never the
alias spelling.

All three targets: wasm32 resolves `ruleset_invoke_entry` /
`ruleset_invoke_frame` as imports; native / `native-rules` needs
`HostOps::op_invoke_entry` / `op_invoke_frame` + their forwarding impls in
`crates/game-rules/src/native_shims.rs` (the two old ops stay as aliases) and
the `crates/rules-native/tests/abi_link_names.rs` pin; Kani routes the `sys`
declarations through `sys_kani` to `Op::invoke_entry` / `Op::invoke_frame`
(§5.6).

### 4.5 `game-rules` — dispatch surface

```rust
pub enum Call {
    // …
    /// `On::Press` -- a placed card's activation. `uid` is the instance,
    /// `entry` the resolved manifest entry index (resolved in `WasmRules::press`,
    /// which is why the run path does not re-look-up by id); `arg` is the
    /// argument the request's `OP_DECLARE` collected.
    Press { card: i32, player_id: i32, uid: i32, entry: i32, arg: Vec<u8> },
}
```

`Call::player_id()` / `Call::card()` include it. `Ruleset::run` resolves
`Call::Press { entry, .. } => Some(entry)`, takes `arg` like `Play`, and does
**not** include Press in the body-time admission block (gate at offer, as for
Play). `drive` does not reentrancy-mark it:
`!matches!(call, Call::Play { .. } | Call::Press { .. })` (`wasm_rules.rs:3039`).
`announce_drive` maps it to `card_trigger::PRESS` and the 「发动」 wording
(`wasm_rules.rs:2864-2870`, `2896-2900`).

New `Ruleset` methods:

```rust
/// Does this card declare a press entry? (Static; no world.)
pub fn has_press_entry(&self, card: i32) -> bool;
/// The press entries as `(entry index, id, label)` in declaration order.
pub fn press_entries(&self, card: i32) -> Vec<(i32, &str, &str)>;
/// The press gate `why_not` for one entry, from a world copy with
/// `current_uid = uid` (probe: a prompting guard answers "no reason").
pub fn press_cant_act<W: CardWorld>(
    &self, world: &W, card: i32, entry: i32, player_id: i32,
) -> Result<Option<crate::Msg>, RuleError>;
/// `press_gate_vanishes` mirror of `play_gate_vanishes` for the G4 cheap path.
pub fn press_gate_vanishes(&self, card: i32, entry: i32) -> bool;
```

`WasmRules::press_cant_act` mirrors `cant_play`: the `press_gate_vanishes`
cheap pre-filter first (no guard and no condition -> `None`, skipping the uid
lookup and the guard store), then the `Run` with `current_uid = uid`.

`Ruleset::build` (`host.rs:1058-1085` region): a kind-15 entry must have
`id: Some(slug)`, the slug must match the grammar and must not be `"press"`,
and ids are unique per card. A press entry must not be a filter (it has a
body/declare). Error: `RuleError::BadPressId` (message names the card and
entry index).

`WasmRules::press` (the `CardRules` impl; mirror of `play`, which begins at
`wasm_rules.rs:9384`):

```rust
fn press(&self, cx: &mut Cx, player_id: usize, card: &str, uid: i32, entry: &str)
    -> Flow<Dest>
```

1. `ruleset.card(card)`; unknown -> `log.card_not_ported`, `Dest::Graveyard`.
2. Validate `field_by_uid(uid)`, `holder_of(uid) == player_id`,
   `field.card == card`; else trap/log and no body.
3. Resolve `entry_by_id(entry)`; missing -> `DEST_UNSET` no-op (G13 §4.4).
4. `drive(cx, Call::Press { …, arg: take(press_arg) }, card, uid, &mut trigger)`.
5. Dest applies to that uid (`drive_inner_body_from:4917-4922`), so
   `Dest::Graveyard` / `Hand` unplace the pressed instance;
   `DEST_FIELD` is a no-op for a press (the instance is already placed —
   documented on `set_dest`).

The play gate fallback: `Ruleset::run`'s `Call::Play` arm and
`hostfns::play_card`'s entry lookup resolve
`entry(OnKind::Play, None).or_else(|| press_entry_preferring_gated())`, so
`ctx::play_card` on a migrated skill (no `On::Play`) still runs its press, and
`invoke_skill` on it still resolves the default press.

### 4.6 `game-core` — request, view, revocation

```rust
pub struct NetMessage {
    // …
    /// `press`: the field instance's uid. 0 = not supplied (the `skill` alias
    /// resolves the first instance; old records omit it).
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub uid: i32,
    /// `press`: the press entry's slug. Empty = the card's default press
    /// entry (the `skill` alias and the `query_guard` alias).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub entry: String,
}
```

`skip_serializing_if` is load-bearing: `body_check` hashes the re-serialized
`RecordBody` (`record.rs:462-465`), so an always-emitted new field would change
every stored hash. `is_zero_i32` is a new private helper beside `NetMessage`
(matching `state.rs`'s `is_zero`, which is not visible here).

`CardRules` (`engine/rules.rs:467`):

```rust
/// Does this card rule declare a press entry? (Static.) `WasmRules`: a press
/// entry exists, or -- until the skill migration lands -- the id is a
/// `skill:` with `has_play` (the compat arm; no other prefix test remains).
fn has_press(&self, _card: &str) -> bool { false }
/// Press entries of `card`: id, label, for the viewer list (static).
/// `WasmRules` returns one synthetic `entry::PRESS` row for an unmigrated
/// `skill:` card (label = its first `On::Play`'s), so skills keep their button
/// during the migration.
fn press_decls(&self, _card: &str) -> std::sync::Arc<[PressDecl]> { Arc::from(Vec::new()) }
/// The press gate `why_not` for `(uid, entry)`; `None` = pressable.
fn press_cant_act(&self, _cx: &Cx, _player: usize, _card: &str, _uid: i32, _entry: &str) -> Option<Msg> { None }
/// Pre-commit argument (docs/ACTIVATION.md §2), as `declare_activation`.
fn press_declare(&self, _cx: &mut Cx, _player: usize, _card: &str, _uid: i32, _entry: &str)
    -> Flow<Option<Vec<u8>>> { Ok(Some(Vec::new())) }
/// Run the press. Returns the instance's fate.
fn press(&self, cx: &mut Cx, player: usize, card: &str, uid: i32, entry: &str) -> Flow<Dest>;
```

`PressDecl { id: String, label: String }` is game-core's own shape (like
`TileTextDecl`); `WasmRules` builds it from the manifest. `StubRules` keeps the
defaults. The answer is memoised separately from `rules_cant_play`:
`Cx::press_memo` keyed `(player, uid, entry)` with the same world-stamp reset
(`cx.rs:1051-1074`), because two copies of one card differ per uid and the
existing `(player, card)` memo would cross-talk.

A press belongs to the instance's **effective card**:
`real_id_at(uid).unwrap_or(<the instance's own card>)`, the G01 marker a
stand-in writes on the card it placed (§6.5). Every `card` / `id` below is that
id. When a real-id is set the instance's own entries are replaced by the
effective card's (a substitute does not also offer its own press); entry slugs
are per-card-unique, so the `(uid, entry)` key stays unambiguous either way.

`Match::use_press(player, m)` replaces `use_skill` (`play.rs:979`):

1. Resolve the instance (uid, or first instance for the alias), its effective
   card, entry (slug, or `entry::PRESS` alias).
2. Raise `cardUsed` with `card` / `cards = [id]` / `by_card = player`
   (unchanged payload shape). `is_cancelled` -> `card_activated(PRESS, …,
   negated = true)` and stop.
3. `rules.press_declare`; `Decline` -> stop; `Halt`/prompt propagates.
4. Store the arg in `turn.press_arg`; `rules.press`; `turn.pressed.push`.
5. `card_activated(card_trigger::PRESS, …)` for the non-negated case.

`use_skill` becomes the alias arm: entry `entry::PRESS`, uid 0.

`why_not_act` (`mod.rs:2932`): a `"press"` arm, and `"skill"` delegates to it
with `entry = "press"` and `uid = first instance`. Order:

1. `!playing() || step != OPS || busy` -> `err.press_not_now`.
2. Resolve `(uid, entry)`; a uid not on the viewer's field, or whose effective
   card is not `m.card` -> `err.press_stale`; unknown card/entry, or
   `!has_press(card)` -> `err.press_unknown`.
3. `Gate::OnBoard` / `Gate::Act` (with `card_act_exemption`), as the skill arm
   does.
4. `cx.rules_press_cant_act(i, card, uid, entry)`.

New locale keys (game.json `err.*`): `err.press_not_now`, `err.press_stale`,
`err.press_unknown`. `err.skill_*` stay for the alias.

`view_extra_parts` (`mod.rs:1313-1342`): drop the prefix filter; for each
`(uid, id)` on the viewer's field with `id` = the effective card, for each
`PressDecl` push a `SkillAction` (the JSON field name `skills` stays; its doc
widens to "pressable entries"):

```rust
pub struct SkillAction {
    pub id: String,
    /// Press entry slug ([`abi::entry::PRESS`] for a skill's default press).
    pub entry: String,
    /// The field instance the entry belongs to.
    pub uid: i32,
    /// Display source: the `skill:` owner for a skill, else the card id.
    pub source: String,
    pub title: Msg,
    pub text: Msg,
    pub label: String,
    pub enabled: bool,
    pub reason: Msg,
}
```

`source` for a non-`skill:` id is the card id (the strip_prefix result is used
only when present). `title` / `text` stay `ask.cardOption` card references;
`label` (「（2）」) is what the face renders.

`TurnCtx` (`world.rs:143`): `press_arg: Vec<u8>` (scoped like `play_arg`) and
`pressed: Vec<PressUse>` (`{ uid: i32, entry: String }`), both
`#[serde(default)]`, bot bookkeeping only (a press does **not** increment
`cards_played`).

### 4.7 Client and bots

* `webui/src/scenes/board/Side.tsx`: `SkillSlot` gains `uid` / `entry`;
  `skillMarks(me, a.uid)` finds by uid; `key = uid + ":" + entry`; `fire` sends
  `{ act: "press", card: id, uid, entry }`; the face shows `label`.
* `webui/src/scenes/board/anim.ts:848`: `case "skill":` grows `case "press":`
  (the skill wording when the card id is a `skill:`, else a new `anim.press`
  banner, same sfx); an unknown kind otherwise falls to `default` and skips the
  banner.
* `webui/src/core/types.ts` `SkillAction` gains `entry` / `uid` / `label`.
* `webui/src/game/autopilot.ts` `usableSkills` returns the action objects;
  presses send `act: "press"`; the per-policy `skills` weight table keys on
  `a.id` and, when a card has no entry there (a plain card), the press is taken
  once per turn (standard) — a documented policy change in `docs/BOT.md` §3.8.
* `crates/game-core/src/engine/ai.rs` `ai_skill_choice` scans
  `field_instances` x `press_decls`, skips `turn.pressed`, gates with
  `rules_press_cant_act`, and keeps the strategy weight keyed by card id; chaos
  presses every enabled entry once.

### 4.8 Classification

| # | surface | class |
|---|---|---|
| 1 | `OnKind::Press = 15` + required press ids in the manifest | NEW / **WIRE-BREAK** (old host rejects kind 15) |
| 2 | `On::Press` / `PressEntry` / builder arms | NEW (source) |
| 3 | G13 imports `invoke_entry` / `invoke_frame`; `invoke_skill` / `play_card` become sugar; press adds `by_uid` | NEW / **WIRE-BREAK** (removed guest imports; host aliases kept) |
| 4 | `Call::Press`; `WasmRules::press`; `Ruleset::press_*` | NEW (internal) |
| 5 | `CardRules::has_press` / `press_decls` / `press_cant_act` / `press_declare` / `press` | EXTENDED trait |
| 6 | `NetMessage.uid` / `.entry`, `act: "press"` | EXTENDED wire; `skill` accepted as alias |
| 7 | `SkillAction.uid` / `.entry` / `.label`; `skills` list contents | EXTENDED view |
| 8 | `TurnCtx.pressed` / `press_arg` | EXTENDED save (**SAVE_VERSION 16**) |
| 9 | `card_trigger::PRESS` | EXTENDED event string (replay-safe) |

---

## 5. Engine wiring

### 5.1 Press request path

`Match::act` routes `"press"` to `use_press`; `why_not_act` has already run the
same checks in the same order (`act` calls `why_not_act` before dispatch as for
every turn action). `"skill"` remains an accepted alias forever for records and
third-party clients.

### 5.2 Gate, declare, commit

The gate is `press_cant_act` at offer. `press_declare` runs with the
`OP_DECLARE` barrier; the arg rides `Call::Press.arg` and `ctx::declared_arg::<T>`
serves it in the body (the `drive` monomorphization already does this for any
entry with a declare). A decline returns before any state write.

### 5.3 Dest and instance reads

The whole drive runs with `current_uid = uid`, so `is_placed` is true, named
counters / `crystals` / props resolve on that card, and `add_tok` binds to it
(`ctx::add_tok` -> `add_held(self_uid)`). `set_dest(Graveyard)` at effect end
unplaces it and sends it to the discard (`apply_dest`, `wasm_rules.rs:5847`).
`Dest::Field` from a press is a no-op (already placed); a press that wants a
fresh instance must `unplace` first and `place_card`.

### 5.4 Cross-card invocation (`ctx::invoke_entry`)

Follow G13 §5 (the routine replaces today's `hostfns::invoke_skill`,
`hostfns.rs:1338-1412`, and `play_card`'s entry lookup) with the press
additions marked:

1. Resolve `card_id`; find its first instance on the caller's field (uid) for
   binding; **`by_uid` = the caller's `current_uid`** (the pressed instance);
   check `MAX_NESTING`.
2. Compute the entry (`entry::PRESS` -> default press entry; other ids ->
   `entry_by_id`); unknown/unnamed -> `DEST_UNSET` no-op.
3. `enter_card`; push a frame `{ card, entry, by = caller, by_uid, uid, paid,
   depth }`; bind `current_uid = uid`; **seed** the nested trigger from the
   frame (`card`, `cards = [card_id]`, `by_card = caller`). No raise (G13
   §5.3): the press raised `cardUsed` for the caller's line already, and the
   seed is what makes `trigger::cards()` answer inside the body.
4. Run `OP_RUN` (after `OP_DECLARE` when `has_declare`; decline ->
   `EXIT_DECLINED`).
5. `leave_card`; pop the frame; return the nested `Dest`; restore fuel.

Notes:

* The gate is **not** evaluated (G13: "body only"); that is how 「此次技能不受
  数量或轮数限制」 waives (2)'s CD/cap without a flag. The press gate still
  checks the caller-side costs (crystal, pot — §6.1) and the body's
  `fire::spend` remains the last line.
* No new `TriggerKind`. The outer press raises `cardUsed` once; `nanami_ordinary`'s
  (2) condition `actor != owner` keeps it out of its own raise.
* `ctx::invoke_paid` / `COST_PAID` (G13) let a borrowing body skip its own
  cost prompt when the caller paid.
* Inline/bot/sandbox backends need no work: `invoke_entry` is a `hostfns`
  routine on the same `HostCtx` path as `play_card`.

### 5.5 Offer

`view_extra_parts` enumerates; no prefix test. Order: field order, then
declaration order. The per-card `press_decls` is cached on the engine (manifests
are static per match). A card with several press entries yields several
actions, each gated separately.

### 5.6 Kani model

* `Op::invoke_entry = 391` (132 `invoke_skill` retired, never reused; 391-400
  is this group's README band);
  G13 §5.5's `arm_invoke_entry` / `invoke_n` / `last_invoke` are adopted as-is,
  plus the press binding: the record extends to `(card, entry, paid, by_uid)`
  and `by_uid` must equal `last_uid` before the call frame is pushed. It does
  not call the nested body (the host has no per-id dispatcher); proofs assert
  the invocation record and the caller's effects. `invoke_frame` returns the
  model record.
* `arm_is_placed` becomes instance-bound:
  `w.field.iter().any(|f| f.uid == w.last_uid)` (was `field_n > 0`).
* `arm_placed_cards` already resolves each `FieldCard.card` through
  `intern::resolve_str(NameId(..))` and returns real card names
  (`host.rs:2310-2325`), not `"p0".."p3"`; the model needs no field for this,
  and `owner_skill`'s `skill:` prefix test is already meaningful under CBMC.
* A harness helper `place_instance(player, card) -> uid` writes `field[]`
  (`FieldCard { uid, card, player, .. }`) and `last_uid`; a press harness sets
  `last_uid = uid`.

---

## 6. Card usage before / after, per item

### 6.1 `nanami-press-invokes-clause-3` (row 437)

Before (one entry, origin branch, wrong target):

```rust
On::Play("", Some(cant_play), play),
// …
fn play(p) { if ctx::is_placed() { use_skill(p) } else { nanami_effort(p) } }
// gate: if ctx::is_placed() { crystals, owner_skill, marks } else { tokens }
// press: play_card(skill)  -> skill's On::Play = clause (3)
```

After — two entries, no branch, named target:

```rust
On::Play("", Some(cant_play_from_hand), nanami_effort),
On::Press("press_skill", "", Some(cant_press), use_skill),

fn cant_play_from_hand(p) -> Option<Msg> { /* token sum only */ }
fn cant_press(p) -> Option<Msg> {
    if ctx::crystals() < 1 { return Some(Msg::new(key!("nanami_effort_no_crystal"))); }
    if bandori_std::fire::get(p) < 1 { return Some(Msg::new(key!("nanami_effort_no_pot"))); }
    if owner_skill(p).is_none() { return Some(Msg::new(key!("nanami_effort_no_skill"))); }
    None // the (2) gate itself is waived by the card text, not by a mark check
}

fn use_skill(p) -> Asked {
    let Some(id) = owner_skill(p) else { return Ok(()) };
    ctx::add_crystals(-1, 0)?;
    ctx::invoke_entry(p, id.as_str(), "2")?; // body only; the gate is waived
    Ok(())
}
```

`nanami_ordinary`'s (2) gains `.id("2")` (G13 §4.4's convention; the label
「（2）」 stays). New locale key
`nanami_effort_no_pot` (zh/en). The two `TODO(规则书)` at
`nanami_effort.rs:276-285` close; the card's own gate no longer needs the
"spends a mark" re-check because it no longer runs (3).

### 6.2 `kanon-press-surface-abi` (rows 380, 955, 967) — demote to A

The A-only fix (ship-now, if the group slips): delete the `skill:` test in
`view_extra_parts`, derive `source` defensively; the card then appears and
`why_not_act` reaches its gate. Known cost: hand-only cards appear too (#4).

Under G05, split the entry:

```rust
On::Play("", None, place),                     // hand play: place only
On::Press("jump", "", Some(cant_jump), jump),  // placed press: jump only

fn cant_jump(p) -> Option<Msg> {
    if ctx::tok(p, JELLY) >= 9 { return rules_status::move_blocked(p); }
    Some(Msg::new(key!("kanon_march_need")))
}
```

* `place` is the old hand branch without `play_from_hand() || !is_placed()`.
* `jump` is the old tail without the marker re-check (the gate owns it) and
  without `is_placed`.
* `cant_play`'s `hand_count > 0 || !is_placed` (line 51) and both TODOs
  (lines 47-50, 79-81) go.
* Bottom gate: hand play always allowed -> `why_not: None`; the placed copy
  shows `kanon_march_need` below 9 and `move_blocked` after the main move.

### 6.3 `even-lost-not-pressable` (row 1521) — demote to A

A-only fix: the same filter drop; `can_go`'s `is_placed` check already refuses
a hand copy (there is never one on the field). Under G05:

```rust
On::Press("go", "", Some(can_go), go),   // On::Play is deleted
fn can_go(p) -> Option<Msg> { rules_status::move_blocked(p) }  // no is_placed
```

The `ctx::play_card` fallback (§4.5) keeps `ctx::play_card("MyGO:即使迷茫着")`
working. TODO at line 20-21 closes.

### 6.4 `placed-card-press-surface` (row 974)

Before: prefix filter + `has_play` + id-derived source + client/bot assumptions.

After: §4.6/§4.7. The `anon_tokyo` / `haneoka` hand-only cards now declare no
press entry and never appear, so their `[手]` bodies cannot be replayed from
the field. A card that is *both* hand-playable and field-activated (kanon,
nanami) has two entries and two independent gates.

### 6.5 `hina-clause-2-swap-missing` (rows 487, 488, 490, 1555) — press half; G01 owns the marker

Book (`PP:[冰川日菜]会发出怎样的声音呢？`, clause (2)):

> （2）[共鸣]本卡在[场地]时改变本卡代替的专属卡效果。

「本卡」 is the stand-in, which is banished once its substitute lands
(`hina_sound.rs:86`); the [共鸣] clause therefore lives on the *substitute*
from then on. That is exactly what the G01 real-id marker is for
(`set_real_id_at`, G01 §4), and what §4.6's effective-card rule addresses: the
press entry is declared on `HINA_SOUND`, found through the substitute's uid.

Before (`hina_sound.rs:21`, two dead TODOs at `:87-96`):

```rust
On::Play("", None, hina_sound),   // no marker written; no offer surface
```

After — the play stamps the marker where the sub landed; the swap is a press:

```rust
use card_sdk::abi::counter;
const ID: &str = "PP:[冰川日菜]会发出怎样的声音呢？";

On::Play("", None, hina_sound),
On::Press("press_swap", "", Some(cant_swap), hina_swap).label("（2）"),

// in hina_sound, before the ask (`:63`):
let before: Vec<i32> = ctx::field_instances(player_id).iter().map(|(u, _)| *u).collect();

// in hina_sound, after the sub is placed (`:77-82`):
let Some(uid) = new_uid_since(player_id, &before, sub) else { return Ok(()) };
ctx::set_real_id_at(uid, ID);

fn cant_swap(p: i32) -> Option<Msg> {
    if ctx::hand_count(p, crate::resonance::RESONANCE.id) < 1 {
        return Some(Msg::new(key!("hina_swap_no_resonance")));
    }
    None
}

fn hina_swap(player_id: i32) -> Asked {
    let uid = ctx::self_uid();                 // the substitute, via the press
    let crystals = ctx::counter_at(uid, counter::CRYSTALS);
    let cur = match ctx::real_id_at(uid) {
        Some(id) => id.as_str().to_string(),
        // unmarked: the entry is still reachable by id on the sub itself
        None => ctx::field_instances(player_id).iter()
            .find(|(u, _)| *u == uid).map(|(_, c)| c.as_str())
            .unwrap_or_default().to_string(),
    };
    let others: Vec<&str> = SUBS.iter().copied().filter(|s| *s != cur).collect();
    if !crate::resonance::try_resonance(player_id)? { return Ok(()); }
    let i = ctx::ask_card(
        player_id,
        &Msg::new(key!("hina_sound_title")),
        &Msg::new(key!("hina_sound_swap_ask")),
        &others,
    )?;
    let sub = others[i];
    let as_msg = Msg::new(key!("hina_sound_as")).card("card", ID).card("sub", sub);
    ctx::log(player_id, &as_msg);
    ctx::unplace_at(uid);
    let before: Vec<i32> = ctx::field_instances(player_id).iter().map(|(u, _)| *u).collect();
    if sub == MAMI {
        ctx::place_card(player_id, sub, &as_msg);
    } else {
        ctx::play_card(sub, player_id)?;
    }
    let Some(new_uid) = new_uid_since(player_id, &before, sub) else { return Ok(()) };
    ctx::set_real_id_at(new_uid, ID);
    ctx::add_counter_at(new_uid, counter::CRYSTALS, crystals, 0);
    Ok(())
}

/// uid of the instance of `card` placed since `before`; the play/place imports
/// do not return the instance (`play_card` returns `Dest`, `place_card` returns
/// its own uid, so MAMI could skip the diff -- one path is cheaper to keep
/// right).
fn new_uid_since(p: i32, before: &[i32], card: &str) -> Option<i32> {
    ctx::field_instances(p)
        .iter()
        .find(|(u, c)| !before.contains(u) && c.as_str() == card)
        .map(|(u, _)| *u)
}
```

* `new_uid_since` is taken **before** `unplace_at`/the play in each use; HINA_SOUND
  itself is on the field while its own body runs, so it is in `before` and the
  diff cannot pick it up.
* The crystals are read as a plain counter and re-added at the new instance,
  which is what keeps them across `unplace` + re-place (`crystal_swap.rs:51,60`
  is the same read/write pair).
* `[共鸣]` spends through `resonance::try_resonance` (it asks the yes/no itself);
  the press has already committed, so a "no" returns with the original sub
  intact.
* New locale keys `hina_sound_swap_ask` / `hina_swap_no_resonance` (zh/en);
  `hina_sound_title` is reused.
* G01 §6.10's sketch spells this as `On::Action(Action::Actions(…))`; on the
  press half that is superseded by `On::Press` (the `card.placed` condition and
  the `actor == owner` pre drop out — a press is always a placed, owner-acted
  entry). Its marker and counter spellings are adopted verbatim.
* Test `g05_hina_swap_keeps_crystals_and_re_mark` (two copied instances,
  different uids, same crystal count) pins the per-instance marker and counter
  carry.

---

## 7. Save, record, replay

* **Save:** `TurnCtx.press_arg` / `pressed` are `#[serde(default)]`;
  **SAVE_VERSION 15 -> 16** (the constant is exact, `engine/mod.rs:374`;
  older saves are refused and replays ride archived bundles).
* **Record:** new `NetMessage` fields are `skip_serializing_if`, so a record
  body written before v111 re-serializes byte-identically and `body_check`
  stays equal. `RECORD_VERSION` stays 3.
* **Replay:** an old `act: "skill"` replays through the alias with the same
  instance/entry resolution the old engine used (first instance, first
  `On::Play`; after the skill migration, the first press entry — same body).
* **Event log:** new presses emit `MatchEvent.kind = "press"`; old `"skill"`
  events still render (the client's kind lookup defaults).
* **Bot bookkeeping** `turn.pressed` is saved so a restored bot does not loop
  on a no-op press; it never gates a human.

---

## 8. Versioning

**ABI_VERSION 110 -> 111** (eighth group in the README landing order; take the
next free number if the order moves). Wire-breaks: manifest `OnKind` 15 (an old host's
`from_i32` rejects it) and the removed guest imports `invoke_skill` /
`play_card` in favour of G13's `invoke_entry` / `invoke_frame` (name-keyed
imports; the host keeps `ruleset_invoke_skill` / `ruleset_play_card` as aliases
so pre-v111 wasm still links). Changelog block, placed after the
`pub const ABI_VERSION` line as v104 is:

```rust
/// v111: **press entries for placed cards**. A placed card's printed
///    activation is its own `On::Press(id, pre, why_not, body)` entry with a
///    required ASCII slug id; `OnKind::Press = 15` (4/5/6 stay retired).
///    Requests address the field instance by `uid` and the clause by that id
///    (`act: "press"`, `NetMessage.uid` / `.entry`; `act: "skill"` is an
///    alias resolving the first instance and `entry::PRESS`).
///    `entry::PRESS` now resolves the card's first press entry, falling back
///    to its first `On::Play` for an unmigrated ruleset; `"press"` stays
///    undeclarable. ManifestOn is unchanged (id is required for kind 15).
///    Shared with G13: host imports `invoke_entry` / `invoke_frame`, guest
///    `ctx::invoke_entry` / `invoke_entry_paid` / `invoke`; the guest
///    declarations of `invoke_skill` / `play_card` collapse onto them
///    (`ruleset_invoke_skill` / `ruleset_play_card` remain host aliases).
///    `ctx::invoke_skill` is sugar over `invoke_entry(…, entry::PRESS)`.
///    **SAVE_VERSION 15 -> 16** (`TurnCtx.pressed` / `press_arg`, both
///    `serde(default)`). RECORD_VERSION unchanged (new NetMessage fields are
///    `skip_serializing_if`; old `act: "skill"` records replay via the alias).
```

The effective-card resolution (§4.6) reads G01's `FieldCard.real_id`; it adds
no field of its own, and with no marker it falls back to the instance's own
card, so it is correct before §6.5's migration too.

`docs/ACTIVATION.md` §3's `entry::PRESS` paragraph and the
`On::Play("", Some(can_use), use_skill)` example move to `On::Press`.

---

## 9. Tests

Ruleset tests (harness gains `Table::press(who, uid, entry)`; `Table::skill`
delegates to it with the alias):

* 1 (nanami): `rules/tests/tests/morfonica.rs::nanami_effort_press_invokes_skill_two`
  (crystal removed, pot spent, mark gained, CD set, card stays placed);
  `…_press_waives_cap_and_cd` (pre-set CD 3 and existing mark, 3+ alive; still
  gains); `…_press_without_crystal_refused`.
* 2 (kanon): `rules/tests/tests/hhw.rs::kanon_press_offered_at_nine`;
  `…_press_jumps_and_discards`; `…_press_below_nine_refused`;
  `…_hand_play_places_and_does_not_jump`;
  `…_press_second_copy_binds_uid` (copy A 9 markers, copy B 0: pressing B
  refuses, pressing A jumps).
* 3 (even-lost): `rules/tests/tests/mygo.rs::even_lost_press_discards_and_moves_hand_size`;
  `…_press_needs_main_move`; `…_press_not_in_hand`.
* 4 (surface): `crates/game-core/tests/view_extra.rs::press_list_has_uid_entry_label`,
  `…_hand_only_play_not_offered`, `…_stale_uid_refused`;
  `crates/game-core/tests/engine.rs::press_unknown`, `press_not_now`,
  `press_negated_by_counteraction` (raise cancelled -> no crystal spent).
* 5 (hina): `rules/tests/tests/pp.rs::hina_press_swaps_sub_and_keeps_crystals`,
  `…_press_without_resonance_refused`, `…_press_binds_second_copy` (two marked
  substitutes; pressing B swaps B and leaves A's marker alone). Interface half
  with 4: `view_extra.rs::press_offer_uses_effective_card` (the marked
  substitute offers `press_swap` on the sub's uid, `source` = `HINA_SOUND`) and
  `…_substitute_own_entries_suppressed`.
* ABI/ruleset: `crates/game-rules/tests/kind_table_pin.rs` kind 15;
  `crates/game-rules/tests/ruleset.rs::press_without_id_rejected`,
  `…_press_id_reserved_rejected`, `…_duplicate_press_id_rejected`;
  `crates/rules-native/tests/abi_link_names.rs` covers
  `ruleset_invoke_entry` (and the retained `ruleset_invoke_skill`).
* Record: `crates/game-core/tests/record.rs::old_skill_record_body_check_unchanged`
  (golden hash), `…_press_record_round_trip`.
* Invocation is G13's test surface; G05 adds only the press binding:
  `crates/game-rules/tests/declare_arg.rs::invoke_frame_by_uid_is_pressed_instance`
  and `…_invoke_from_plain_card_and_skill`.

Kani obligations (all in `rules/kani`):

* `Op::invoke_entry` / `arm_invoke_entry` + the G13 record fields;
  `arm_is_placed` uid-bound; `arm_placed_cards` real names;
  `place_instance` harness helper.
* `proofs/mor.rs`: `nanami_effort_press_iff_at(user)` — post = crystal -1,
  invoke recorded at the owner skill with `by_uid` = the pressed instance and
  `paid = 0`, exactly one effect log, frame unchanged;
  `…_press_no_crystal_noop_at`; `…_press_binds_second_instance`.
* `proofs/hhw.rs`: `kanon_jump_at_nine_iff` (markers cleared, teleport plan,
  dest graveyard), `…_jump_below_nine_noop`.
* `proofs/pp.rs`: `hina_swap_iff_at(uid)` — the entry is offered only on a
  marked instance with resonance; post = that uid unplaced, exactly one new
  instance exists with `real_id = "PP:[冰川日菜]会发出怎样的声音呢？"` and the
  carried crystal count, the resonance row gone, a second marked copy's marker
  unchanged; `…_swap_without_resonance_noop` (world snapshot equal).
* Invariant proof: `press_is_placed_is_uid_bound` — with two field instances,
  `is_placed` is true only while `last_uid` is one of them.

---

## 10. Alternatives

**A1 — filter drop only (no ABI).** Delete the `skill:` test and derive
`source`. Cheapest; satisfies #2/#3's stated expectation, which is why they are
demoted. Fails #4 (hand-only cards rerun their hand body from the field), has no
uid, and cannot name a clause, so #1 stays impossible.

**A2 — `prop::FIELD_PRESS` card prop.** One card-level boolean marks "this card
is activatable from the field"; the view filters on it and presses the first
`On::Play` with the uid bound. Small diff, one data bit, no manifest kind. But
it is exactly the card-wide yes/no the invariant rejects: a two-clause card
cannot address the second clause, the body still branches on `is_placed()`, and
a card with a hand play and a press still shares one gate. Kept as a fallback if
kind 15 churn is judged too high.

**A3 — several `On::Play` entries + `uid`, no new kind.** Press entries stay
`On::Play` with slug ids; the press surface offers non-first entries, hand play
offers the first. Avoids the new kind and the skill migration, but
`entry(OnKind::Play, None)` (used in `play`, `play_card`, `invoke_skill`,
`has_play`, `play_gate_vanishes`) becomes origin-aware everywhere, and the
"first entry is the hand play" rule is positional again — the rule this design
wants to remove. `On::Play`'s docs would have to say "the first is hand play";
a card that lists its press first silently breaks. Rejected.

**A4 — raise-based press.** `ctx::raise` a new `cardPressed` kind and let a
hook run the body; the client sends only `act:"press"` with the card id. No
`Call`/`CardRules` additions. But a raise is synchronous fan-out with no return
`Dest`, no declare barrier, and cancellation would have to abort by convention;
the existing `drive` already gives all three for a resolved entry. Rejected.

---

## 11. Open questions

1. **C-Q46 / 「不受数量或轮数限制」.** `invoke_entry` never evaluates the
   target's gate (G13 §5.6), so the waiver is structural rather than a flag.
   It does not waive the skill's *cost* — nanami's (2) spends a pot inside the
   body and the press gate checks a pot first (§6.1). Confirm the waiver covers
   the 「每种角色标记上限1」 cap and the CD (assumed yes), and that a cost the
   body cannot pay is a legal no-op.
2. **Which category does the nanami press raise?** `use_press` raises
   `cardUsed` once for the pressed effort card; the invocation seeds, and does
   not raise, the skill side (G13 §5.3). Is one raise expected, or should the
   skill's use be a second answerable window? A tae_police 「抵消」 answers the
   press today; under this design it cannot answer the skill separately.
3. **Should the press of a skill go through `use_skill`'s `skillUsed`
   semantics at all now that `entry::PRESS` is generic?** The design keeps the
   payload identical, so no behavior change; confirm no ruling named
   `skillUsed` separately from `cardUsed`.
4. **Opponent presses.** Every press is offered only to the instance's holder.
   No card text today lets another seat press; if one appears, `why_not_act`
   needs a holder override and the request a target seat.
5. **`Dest::Field` on a press.** Documented no-op (already placed). Nanami's
   (3) hand play still places; a press that should re-place must unplace first.
   Confirm no clause wants a "return to field" press. Related: a press body
   reached through `ctx::play_card` from **hand** (the §4.5 fallback) has no
   instance, so `send_to_dest` is a no-op there — such a body must use
   `set_dest`, not `send_to_dest`, if both paths must move the card.
6. **`entry::PRESS` for a multi-press card** resolves the *first* press entry;
   other clauses are queried by slug. Is "first" an acceptable default alias, or
   should `PRESS` mean "any press entry admits" for `query_guard`?
7. **`skill_blocked(player, "")`.** Skill presses keep the skill's own gate;
   plain-card presses do not consult it. Should a global 「技能不能使用」 status
   also block a card-invoked skill (an ungated `invoke_entry`)? Assumed
   the card's gate decides, since the card text is the permission.
8. **Effective-card scope.** §4.6 resolves a real-id for every press read.
   Should the same resolution apply to the `query_guard`/`invoke_entry` reads a
   body makes, and to `entry::PRESS` (so a substitute's default press is the
   stand-in's first press)? Assumed yes for press resolution and unchanged for
   `query_guard` (the body addresses by slug, §6.5).
9. **`entry::PRESS` naming** once skills migrate: the word no longer means
   "`On::Play`". Rename to `entry::DEFAULT_PRESS` (wire spelling unchanged,
   source-only) or keep `PRESS` and let its doc carry the fallback?
