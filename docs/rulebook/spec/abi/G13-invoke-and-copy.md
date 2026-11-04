# G13 — Skill invocation and borrowed-clause context

Status: **design revision** (2026-11-04 rewrite of the 2026-10-27 draft; no
source touched by this document). Worktree
`/home/monopoly/web/.claude/worktrees/g13-design-review`, branch
`worktree-g13-design-review`. Starts from master at ABI 113, SAVE 16,
RECORD 4, highest engine kind 131, highest Kani `Op` 442. It does not redo any
landed group.

Group: **G13**. Family: running another card's named clause — the entry the
clause is named by, the instance it runs on, the borrowing card's context, the
borrower's cost and the delay the body owes back to the borrower.

Names are spelled out (G03 R25, G06, G07, G10): an **invocation** is one
nested run of a named entry; the **invoking** card is the borrower and the card
it runs is the **target**; an **entry identifier** is the declared slug of one
clause (`On::….id("2")`); an **instance identifier** is the field row that
owns the target copy; a **hook** is a passive listener. `trigger` is not
shortened to `trig`, and `identifier` is not shortened to `id` in prose. Code
spelling is left as it is (`entry::PRESS`, `self_uid`, `invoke_entry`) so the
document can be followed against the tree.

Names chosen by this revision likewise carry no abbreviation: the entry
identifier is `"2"`; the keys are `skill.tomoriPoem.usedThisMove`,
`skill.tomoriPoem.owedSettle` and `skill.tomoriPoem.halfPrice`; the functions
are `before_settle`, `before_settle_applies`, `half_price_applies`,
`owed_settle`, `clear_used_this_move`, `used_this_move_key`, `clear_latches`
and `offer_open`; the schedule name is `"owed_settle"`. `usedThisMove` and
`owedSettle` replace the current `acted` and `due` key strings; `clear_acted`,
`half`, `not_latched` and `latch_key` are renamed to match, and the shout's
`clear_latch` becomes `clear_latches` because it clears three keys.

Two short handles used throughout: the **poem** is 高松灯's persona skill
`skill:高松灯:诗超绊` and the **shout** is `CRYCHIC:（灯）内心的呐喊`, the card
that borrows it. Neither is a file name.

Ground truth: `docs/rulebook/cards.json`, plus the character and band sheets
for the two borrowed clauses (`rules/skills/skill-personas/src/nanami.rs`,
`rules/skills/skill-personas/src/tomori.rs`). Every quoted fragment below was
re-read against that file on 2026-11-04 and is named with its card identifier
beside it. Row numbers are positions in
`docs/rulebook/card-review-2026-10-27.json` and are not `cards.json` indices.

## 0. Review record

This revision came out of a full review of the 2026-10-27 draft against the
tree as it stands after the G05 landing. The draft's central ABI was adopted
verbatim by G05 and is already on master; the review therefore had two jobs:
verify the landed surface against the draft's invariants and re-derive the one
item that is still broken. What the draft got wrong:

| # | Draft defect | Correction |
|---|---|---|
| R1 | The header put the draft in `card-rulebook-review` on a base of ABI 104 and presented both items as unbuilt. G05 has since landed the whole invocation surface, the nanami body and the mutsumi invocation. | Header carries the real base; §2 records what landed and what is still missing (tests), §1 splits the item into **landed code** and **missing coverage**. |
| R2 | The draft's §6.3 borrowed call is `invoke_entry_paid(…, "2")` and says "the settle then reaches `2-effect` through the normal filter". The engine dispatches the target card's own entries in the *same* raise pass, and the poem's skill instance has a lower instance identifier than the shout (bound at game start versus played later), so `2` and `2-effect` both run **before** the shout is reached. When the shout later invokes `2`, `2-effect` has already declined. | The single-entry design of §7: the borrowed body is `2` itself, so there is no second entry and no dispatch-order dependence (`wasm_rules.rs:5998` walks instances by identifier, seat by seat). |
| R3 | `2` and `2-effect` are two `SettleBefore` entries on one card and rely on the engine running every matching entry. `after_offer_applies` is evaluated on `2-effect` in the same pass as `2`; the borrowed call runs `2` but not `2-effect` (see R2). | One entry, `2`. Its body asks and pays only when `ctx::invoke_paid()` is false. |
| R4 | The draft's `"2-effect"` body sets `OFFERED` in `2`, clears it on entry, and gates `2-effect` on it. Three latches (`OFFERED`, `USED_THIS_MOVE`, `OWED_SETTLE`) for one clause. | Two latches: `USED_THIS_MOVE` (one use per move, already exists) and `OWED_SETTLE` (one owed settle). `OFFERED` disappears with the second entry. |
| R5 | The half-price is implemented on **both** cards: the shout keeps `half_key` (`tomori_inner_shout.rs:155-157, 172-181`) and the poem's `half` reads `OWED_SETTLE` (`tomori.rs:75-79`). On the borrowed path, when the roll is 6 or less, both `PaySplitMul` filters are active and the payment is quartered, not halved. | The latch moves into the poem, one filter only (§7.1); the shout's filter, key and `on_pay` are deleted. |
| R6 | The draft's settle callback clears `OWED_SETTLE` **before** settling, so the existing `OWED_SETTLE`-gated half filter is off during the very settle it was supposed to cover; the draft still claims "the existing `half` filter is correct as written". Contradiction. | `owed_settle` sets the half-price latch around its own `settle_tile_at` and clears it after (§7.1); `OWED_SETTLE` only marks that a settle is owed. |
| R7 | The draft moves `OWED_SETTLE` to the mover but leaves the *persona* delay's player wrong: the current `at_turn_end` settles `player_id`, and `Action::TurnEnd("actor == owner …")` dispatches with the card **owner**, so the plain (2) path settles the controller's tile at the controller's turn end, not the mover's. | `ctx::at_next_turn_end(mover, "owed_settle")` (§7.1) carries the mover as the callback's player; the callback settles the mover's own position. |
| R8 | The draft's `ctx::settle_tile_at(mover, …, true)` keeps the current `is_main_move = true` for a turn-end forced settle. | `false`: the owed settle is not the mover's main move. Row 734 is the same defect shape and already names the two non-main precedents (`tomori_crychic.rs:103`, `rinne_rain.rs:53`). |
| R9 | The draft never notices that the poem's own (2) offer and the shout offer both admit on the same landing, and that the poem is dispatched first (lower instance identifier). Current `before_settle_applies` only checks `USED_THIS_MOVE`; the shout only checks its own latch. | §7.3 documents the two offers and their order, and the shout's admission also reads the poem's `USED_THIS_MOVE` so a second invocation of the same skill is the player's explicit choice, never an accident. |
| R10 | The draft's §6.3 shout sets `half_key` and teleports itself, then invokes. With the body owning both, the borrowed path teleports twice and the half is set by the caller. | The body owns the teleport and its log; the shout sets only its once-per-move latch, pays, invokes, and logs the borrowed use (§7.2). |
| R11 | The draft's §4.5 lists `Press` as an invocable entry kind. There is no `OnKind::Press`; `entry::PRESS` is a reserved *resolver* that yields a `Play`. | The invocable set is the landed one: `Play`, `Action`, `Settle`, `Schedule`; `Filter`, `Counteract`, `Message` trap (`hostfns.rs:1636-1650`). |
| R12 | The draft's §5.1 step 4 finds the instance with `find(|(_, c)| c == id)`, written before G05's effective-card rule. A stand-in instance answers for its effective card. | The landed lookup compares `effective_card_at(uid)` (`hostfns.rs:1657-1664`); §3 keeps it. |
| R13 | Item 1's §9 tests were never written. The landed suite has two frame tests (`declare_arg.rs:497-531`) and the mutsumi cash-out (`card_paths.rs:361`), but none of `nanami_press_invokes_skill_two`, `…spends_one_crystal_only`, `…without_pot_refused`, the trigger seed/restore pair, the trap checks, or the PRESS/`play_card` discrimination. | §10.2 lists the missing item 1 coverage as a landing obligation, not as new design. |
| R14 | The draft says item 1's mutsumi half expects the existing test to flip from "no money clause"; it did flip (`card_paths.rs:433-436` asserts `+2 * 500`). The shortfall's zero-money assertion (`…cash_out_is_zero_on_a_shortfall`) is still missing. | §10.2 adds it to the shortfall test rather than a new test. |
| R15 | Row bookkeeping is still loose. The draft correctly rejected 437/996/187/995, but its replacement set includes 734 (noble blue's main-move flag) and 737 (the hand-limit lift, which is G14's `hand-limit-lift`), neither of them an invocation defect. | §1.1 keeps 735 (the `play_card`-runs-(3) defect) as item 1's row and cites 734/737 as adjacent evidence, not as the item. |
| R16 | The draft treats 407 (「最早拥有的格子」) as an item 2 row although it is the placement-order defect owned by `LOY-TOMORI-03`. | §1.2 lists 407 as an adjacent row with its own owner; the design does not touch it. |
| R17 | Row 426 is the unprinted `Banished` destination on mutsumi, not the lost instance read. Both were folded into item 1's narrative. | §7.4 keeps the invocation fix; the destination TODO is called out as a separate open reading on the same body. |
| R18 | The draft's §2 citations predate the G05 landing and drift by tens of lines (`hostfns.rs:2726-2747`, `:1358-1371`, `:1553`, `:3062`, `wasm_rules.rs:1458`, `:1409`, `:2520-2528`, `condition.rs:1393-1422`). | Every site is re-cited in §2-§7, mostly `hostfns.rs:1590-1718`, `wasm_rules.rs:2652-2681`, `nanami.rs:56-67`, `tomori.rs:47-207`, `tomori_inner_shout.rs:30, 192-224`. |
| R19 | The draft has no necessity section, no clause match and no explicit statement that the invariant survives a prompt pause. | §3, §12 and §13 exist; the pause invariant states the rule and §8 says why no save field is needed. |
| R20 | The draft's §5.5 Kani arm was speculative (`fate_hint`, `last_invoke` as a tuple). The landed model records a three-deep invocation stack and exposes `invoke_n` / `last_invoke` (`rules/kani/src/host.rs:2400-2465`). | §11 works from the landed model. |

## 1. Items and disposition

| # | slug | severity | rows | disposition |
|---|---|---|---|---|
| 1 | `invoke-skill-entry` (BACKLOG also carries `nanami-press-invokes-clause-3`) | medium | 735 | **code landed by G05**; §2 verifies it, §10.2 lists the missing tests |
| 2 | `tomori-borrowed-second-half` | medium | 408, 421 | **designed here**; no new ABI, skill body + shout body |

### 1.1 Item 1 — what is landed

G05 adopted the whole invocation surface and `9ce8a398` implements item 1's
two callers:

* `Mor:（NNM）稍微努力了一下` (3) asks its crystal question and invokes the
  persona skill's clause `2` (`nanami_effort.rs:144-185`), with its own cost
  checks (an owned persona skill, a pot) rather than the skill's condition;
* `CRYCHIC:（睦）从没有觉得...` (2) spends the crystals, invokes the band
  skill, and the band cash-out reads the bound instance
  (`mutsumi_never.rs:59-78`, `skill-groups/src/crychic.rs:128-139`);
* `invoke_entry` derives the target instance from the invoking seat's field
  with the effective-card rule and seeds the trigger (`hostfns.rs:1590-1718`);
* `query_guard` seeds the same frame (`hostfns.rs:3381-3459`).

Adjacent rows review the same cards but belong to other groups: 734 is the
main-move flag on a card-forced settle (kept in §7.1 as the precedent for the
owed settle's `is_main_move`), 737 is the hand-limit lift (`G14`
`hand-limit-lift`), and 426 is the unprinted `Banished` destination on
mutsumi's (2).

Remaining for item 1 is coverage (§10.2) and one reading call (§7.4). No code
design remains.

### 1.2 Item 2 — what is broken

`CRYCHIC:（灯）内心的呐喊` (3) (`tomori_inner_shout.rs:30, 192-224`) is a
hand-written copy of the first half of 高松灯 (2): it asks, spends one pot,
teleports the mover to the controller's tile, sets two latches and logs. It
never runs the skill's own body, so the roll, the [停留], the owed settle at
the mover's next turn end, the half-price on the *skill's* forced settle and
every later fix to the skill all stop at the card boundary. Row 408; the site
TODO at `:220-222` and `BACKLOG TOMORI-BIND-01` record it.

Row 421 is the same hook's window: `move.is_main_move` narrows a clause the
sheet states as 「时机合适时」 and the borrowed skill states as 「[移动终点]」,
not 「主要移动终点」. The revision keeps the narrowing and keeps the TODO; it
does not resolve 421 (`G13` does not own timing).

Row 407 (「最早拥有的格子」) is `LOY-TOMORI-03`; out of scope here.

## 2. Landed surface, verified

The following is on master and this design builds on it unchanged.

### 2.1 Guest

```rust
// crates/card-sdk/src/ctx.rs
pub fn invoke_entry(player_id: i32, card: &str, entry_id: &str) -> Result<Dest, Prompt>;
pub fn invoke_entry_paid(player_id: i32, card: &str, entry_id: &str) -> Result<Dest, Prompt>;
pub fn invoke() -> Option<InvokeFrame>;
pub fn invoke_paid() -> bool;
pub fn invoke_skill(player_id: i32, card: &str) -> Result<Dest, Prompt>; // sugar: PRESS
```

`invoke_entry` and `invoke_entry_paid` are the import pair
(`ctx.rs:2993-3017`); `invoke_skill` is sugar over `entry::PRESS`
(`ctx.rs:3069-3074`). The invocation frame is `abi::InvokeFrame`
(`abi.rs:1281-1311`) with the `invoke::COST_PAID` bit (`abi.rs:1279-1284`).

### 2.2 Host

`invoke_inner` (`hostfns.rs:1603-1718`) is the one core behind
`invoke_entry`, `invoke_skill` (`:1574-1587`) and `play_card`. It:

1. resolves the entry by `entry::PRESS` → the first Play
   (`host.rs:188-190`), else `entry_by_id` (`host.rs:162-170`);
2. rejects unknown cards with a trap and unknown/unnamed entries with
   `DEST_UNSET` (`hostfns.rs:1613-1634`);
3. admits `Play`, `Action`, `Settle`, `Schedule` only (`:1636-1650`);
4. finds the target instance on the invoking seat's field through
   `effective_card_at` (`:1657-1664`);
5. records `by_instance = self_uid()` and the caller's depth
   (`:1652-1655`);
6. opens the frame with `enter_invoke` (`wasm_rules.rs:2652-2670`): saves the
   caller's card, destination, destination target and instance, replaces
   `trigger.card` / `trigger.cards` with the target, and sets
   `current_uid = instance` (`-1` when unplaced);
7. runs `OP_RUN` only, without a gate or a declare
   (`hostfns.rs:1697-1698`);
8. restores the world, the answers, the asked prompt and the frame, then
   `leave_invoke` restores the caller's instance and trigger
   (`wasm_rules.rs:2671-2681`);
9. answers the inner `Dest`, `EXIT_NEED_INPUT` as `Ok(EXIT_NEED_INPUT)`.

`ruleset_invoke_entry` / `ruleset_invoke_frame` are wired in the host table,
the native shims and the Kani host (`host.rs:4978-5002`,
`native_shims.rs:241-242, 3295-3304`, `rules/kani/src/host.rs:2400-2465`).

### 2.3 Invariants the draft stated and the landing keeps

Named entry (resolver above), instance binding (`hostfns.rs:1665-1681`,
`wasm_rules.rs:2652-2670`), pause through the sentinel (`hostfns.rs:1711-1715`),
trigger replacement and restore (`wasm_rules.rs:2656-2679`), body only
(`hostfns.rs:1697-1698`), `Dest` returned only (`hostfns.rs:1713`), no player
decision inside the import, totality in wasm, native and Kani. No invariant
moved.

## 3. Capability as an invariant

**A borrowed clause runs its own body under the borrower's trigger, on the
target's live instance, with the borrower's cost already paid, and returns the
delay it owes to the borrower as a schedule on the moved player.**

The implementation must hold:

* **Target.** The borrowed clause is addressed by its declared entry
  identifier (`2`) on the character's **persona** skill, never by "the first
  Play" and never by a band skill.
* **Fabric.** The target instance is the persona skill's field row. The
  body's self-addressed reads (`crystals`, `is_placed`, `self_uid`, props)
  answer for that row.
* **Context.** `trigger.player_id` is the mover and `trigger.tile` is the
  abandoned landing — the borrowed clause reads both. `trigger.card` /
  `trigger.cards` name the borrowed card, so 「该技能」 in the body resolves to
  the skill, not the shout.
* **Cost.** The shout's cost is one pot; the skill's four must not be
  collected. `COST_PAID` is the signal, and only this path passes it.
* **Delay.** The roll and the owed settle belong to the **mover**, at the
  mover's next turn end, on the mover's own position. The callback survives
  the skill instance being gone or unplaced (the schedule carries the card
  identifier, the entry identifier and the instance captured at schedule time,
  `wasm_rules.rs:2581-2606`, `world.rs:466-495`).
* **Half price.** The forced settle and the owed settle are both payments
  under the skill; both are halved, exactly once.
* **Once per move.** The shout has one use per move per copy; the dropped
  latch travels with the move (`MoveResolved`).
* **Pause.** Any prompt in the body (the roll's own window; an abnormal
  gate inside `teleport_to`) pauses the whole invocation through
  `EXIT_NEED_INPUT`; the outer play re-runs and re-derives the chain. No frame
  field is saved.
* **Determinism.** The roll goes through the serialized random stream like
  every other roll; the entry identifier is a body literal.

## 4. Problem restated

### 4.1 The copy cannot be completed in place

Finishing `pull` by hand duplicates: the mover's condition, the destination
choice, the teleport, the roll, the stay, the owed-settle key and the
half-price. Every one of them already exists in `tomori.rs:135-207`. A second
implementation is exactly the drift that produced row 408.

### 4.2 The persona path is itself wrong about the delayed settle

`at_turn_end` (`tomori.rs:217-224`) receives the card **owner** because the
`TurnEnd` entry is a hook on the owner's instance (`:57-61`). It settles the
owner's position; the owner's turn end is the trigger. The sheet's clause
lives on the **mover**:

> 「…然后使其投掷1d20，若出目小于等于6则获得一层[停留]并在自己的下回合结束时
> [触发结算]」

`其` and `自己的` are the moved player. The half-price is likewise the moved
player's, and `PaySplitMul` receives the **payer**
(`play.rs:4094-4096, 3120-3133`), so the current `state::get(player_id, OWED_SETTLE)`
reads the payer while the body writes the owner — correct only in the (3) arm
where the mover and the owner are the same player.

### 4.3 Dispatch order defeats the draft's two-entry split

The hook walk is seat by seat, instance identifier ascending
(`wasm_rules.rs:5998`). The persona skill is bound during setup
(`game-core/src/engine/setup.rs:134`); the shout is placed only when played, so
the skill's entries always run first on a shared landing. A second
`SettleBefore` entry (`2-effect`) has already been dispatched, and declined, by
the time the shout invokes `2`.

### 4.4 Two half-price filters quarter the payment

The shout halves through `half_key` (`tomori_inner_shout.rs:36, 155-181`) and
the skill halves through `OWED_SETTLE` (`tomori.rs:56, 75-79`). Both are `Mul` stages
and compose. On a roll of 6 or less both are live for the forced settle.

### 4.5 The same landing carries two offers

Both residuals admit in the same window: the poem's (2) whenever another
player's endpoint is within five of the owner, and the shout's whenever the
controller is within five of this card and the endpoint is within five of the
controller. The poem runs first (§4.3). Without a shared once-per-move read,
both can be accepted and the skill runs twice.

## 5. Surface — no new ABI

The invocation surface is complete. This item adds **no import**, no manifest
field, no engine kind and no wire type. It changes two card bodies, adds one
guest state key and re-aims two existing keys at the mover:

```
skill.tomoriPoem.usedThisMove    -- exists as `skill.tomoriPoem.acted`; one use per move, on the mover
skill.tomoriPoem.owedSettle      -- exists as `skill.tomoriPoem.due`; an owed settle, on the mover
skill.tomoriPoem.halfPrice       -- new; the half-price latch, on the mover
```

All three are player slots, i.e. the existing dynamic key store; a new key
needs no save version (nothing enumerates the keys). The two existing keys are
renamed to spell the words out, and both stop being read off the owner — the
mover now carries them. The shout's `latch_key` is renamed `used_this_move_key`
to say which latch it is.

The only crossing identifier is the skill's card string
(`skill:高松灯:诗超绊`), obtained through `bandori_std::persona_skill`
(`bandori-std/src/lib.rs:320-322`), exactly as the nanami body does.

## 6. Engine wiring

Nothing engine-side changes. The relevant machinery is cited so a reviewer can
check the fit:

* `ctx::at_next_turn_end` pushes a `Scheduled` record with the player as owner
  **and** target, `skip = true` when scheduled during that player's own turn,
  and the current card and instance captured (`wasm_rules.rs:2581-2606`,
  `ctx.rs:3678-3685`);
* the turn-end pass takes due records out of the world before running them and
  invokes `Call::AtEnd { player_id: owner, entry }`
  (`wasm_rules.rs:6235-6280`), so the body receives the mover;
* `MoveResolved` is the existing per-move cleanup point
  (`tomori_inner_shout.rs:31-33`, `tomori.rs:64-65`);
* `settle_tile_at` runs the settle with an explicit main-move bit
  (`ctx.rs:4343-4345`, `hostfns.rs:2452-2465`).

The invocation core is reused verbatim; `COST_PAID` is passed through
`ctx::invoke_entry_paid`.

## 7. Card usage — before / after

### 7.1 `rules/skills/skill-personas/src/tomori.rs`

```rust
/// 「在此技能影响下的支付价格变为原本的一半」 -- on the moved player, around
/// the forced settle and the owed settle only.
const HALF_PRICE: &str = "skill.tomoriPoem.halfPrice";
```

Entry (replaces the unnamed `SettleBefore` at `:51-55`):

```rust
On::Action(Action::SettleBefore("tile.id >= 0", Some(before_settle_applies), before_settle))
    .id("2")
    .label("（2）")
```

The half-price filter keeps its shape (`:56`) and only changes its gate and
name; `is_placed` is dropped because the owed settle runs from a schedule,
after the skill instance may be gone:

```rust
On::Filter(Filter::PaySplitMul("", Some(half_price_applies), on_pay))
```

Body:

```rust
fn before_settle(player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    // (3) is the owner's own landing near someone else (cost 1, *their* tile);
    // (2) is someone else's landing near the owner (cost 4, the owner's tile).
    let (cost, destination) = if mover == player_id {
        (1, nearest_other_tile(player_id))
    } else {
        (4, ctx::player_pos(player_id))
    };
    if destination < 0 {
        return Ok(());
    }
    // A borrower already paid; only the skill's own offer collects.
    if !ctx::invoke_paid() {
        if !ctx::ask_yes(…)? {
            return Ok(());
        }
        if !bandori_std::fire::spend_skill(player_id, cost, …)? {
            return Ok(());
        }
    }
    state::set(mover, USED_THIS_MOVE, 1);
    state::set(mover, HALF_PRICE, 1);
    ctx::teleport_to(mover, destination);
    ctx::log(player_id, &Msg::new(key!("tomori_moved")).player_id("who", mover).tile("tile", destination));
    if mover != player_id {
        // (2) 「然后使其投掷1d20，若出目小于等于6则获得一层[停留]并在自己的下回合
        // 结束时[触发结算]」 -- the follow-up is the (2) arm, not the cost 4.
        let n = ctx::roll(mover, 1, 20, RollPurpose::Effect, roll_source::SKILL)?;
        if n <= 6 {
            rules_status::stay::add(mover, 1);
            state::set(mover, OWED_SETTLE, 1);
            ctx::at_next_turn_end(mover, "owed_settle");
            ctx::log(player_id, &Msg::new(key!("tomori_stay")).i("n", n as i64));
        }
    }
    Ok(())
}
```

Guard: the existing `USED_THIS_MOVE` refusal stays first; it is the once-per-move claim
that the shout also reads (§7.3):

```rust
fn before_settle_applies(player_id: i32) -> bool {
    let landing = ctx::trigger::tile();
    if landing < 0 { return false; }
    let mover = ctx::trigger::player_id();
    if ctx::slot(mover, USED_THIS_MOVE) != 0 { return false; }
    if mover != player_id {
        return ctx::dist(landing, ctx::player_pos(player_id)) <= 5;
    }
    (0..ctx::player_count()).any(|p| {
        p != player_id && !ctx::player_out(p) && ctx::dist(landing, ctx::player_pos(p)) <= 5
    })
}
```

Half filter — the latch sits on the paying player, who is the mover on both
settles, and no `is_placed` check is needed because the owed settle runs from a
schedule:

```rust
fn half_price_applies(player_id: i32) -> bool {
    state::get(player_id, HALF_PRICE) > 0
}
```

Clear (the existing `MoveResolved` hook at `:62-64`, renamed) and the owed
settle (replacing `at_turn_end` at `:217-224`):

```rust
fn clear_used_this_move(_player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    if mover >= 0 {
        state::set(mover, USED_THIS_MOVE, 0);
        state::set(mover, HALF_PRICE, 0);             // the forced settle is over
    }
    Ok(())
}

/// 「并在自己的下回合结束时[触发结算]」 -- the owed settle, at the mover's own
/// position. Not a main move (row 734's shape).
fn owed_settle(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, OWED_SETTLE, 0);
    state::set(player_id, HALF_PRICE, 1);
    let at = ctx::player_pos(player_id);
    if at >= 0 {
        ctx::settle_tile_at(player_id, at, false);
    }
    state::set(player_id, HALF_PRICE, 0);
    Ok(())
}
On::Action(Action::MoveResolved("", None, clear_used_this_move))
On::Schedule("owed_settle", "", None, owed_settle)
```

The `TurnEnd` entry and its `actor == owner` condition are removed; a schedule
is what 「自己的下回合结束时」 means and it does not need the instance to be
on the field.

### 7.2 `rules/cards/card-crychic/src/tomori_inner_shout.rs`

The hook at `:30` keeps its condition and swaps `Some(not_latched)` for
`Some(offer_open)`; the new body also stands down when the skill's own use
already claimed this move:

```rust
/// The borrowed skill's own key -- must match `tomori.rs`.
const POEM_USED_THIS_MOVE: &str = "skill.tomoriPoem.usedThisMove";
const POEM_HALF_PRICE: &str = "skill.tomoriPoem.halfPrice";

fn offer_open(_player_id: i32) -> bool {
    let mover = ctx::trigger::player_id();
    ctx::slot(mover, &used_this_move_key()) == 0 && ctx::slot(mover, POEM_USED_THIS_MOVE) == 0
}
```

`pull`:

```rust
fn pull(player_id: i32) -> card_sdk::Asked {
    let Some(skill) = ctx::persona_skill(player_id) else {
        return Ok(());
    };
    let mover = ctx::trigger::player_id();
    if ctx::player_pos(player_id) < 0 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("tomori_inner_shout_title")),
        &Msg::new(key!("tomori_inner_shout_pull")).player_id("who", mover),
    )? {
        return Ok(());
    }
    if !bandori_std::fire::spend(player_id, 1, &Msg::new(key!("tomori_inner_shout_spend")))? {
        return Ok(());
    }
    // Claim the card's own once-per-move before the body teleports, so the
    // redirect re-raise does not offer again.
    ctx::set_slot(mover, &used_this_move_key(), 1);
    // 「使用一次高松灯（MyGO!!!!!）的（2）技能」 -- the body, paid: one pot,
    // not four. The body owns the teleport, the roll and the delay.
    ctx::invoke_entry_paid(player_id, &skill, "2")?;
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_inner_shout_pulled"))
            .player_id("who", mover)
            .tile("tile", ctx::player_pos(player_id)),
    );
    Ok(())
}
```

Renamed: `latch_key` (`:148-153`) becomes `used_this_move_key`; its format
string and the per-instance suffix stay.

Deleted: `half_key`, `half`, `on_pay`, and the
`On::Filter(Filter::PaySplitMul(...))` entry (`:34-36, 155-157, 172-181`).
`clear_latches` keeps the shout's own latch and **also** clears the two poem
latches it uses, so they cannot leak when the skill instance is unplaced and
its own `MoveResolved` never runs:

```rust
fn clear_latches(_player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    if mover >= 0 {
        ctx::set_slot(mover, &used_this_move_key(), 0);
        ctx::set_slot(mover, POEM_USED_THIS_MOVE, 0);
        ctx::set_slot(mover, POEM_HALF_PRICE, 0);
    }
    Ok(())
}
```

### 7.3 The two offers on one landing

Sequence on a qualifying landing, with the poem placed and the shout placed:

1. the poem's `2` is dispatched first (lower instance identifier). It asks for
   four pots and, if accepted, sets `USED_THIS_MOVE` / `HALF_PRICE`, teleports, rolls, may owe
   a settle.
2. the shout is dispatched. `offer_open` is false because its own latch is
   clear but `USED_THIS_MOVE` is 1, so the shout does **not** offer — the skill already
   ran this move.
3. if the poem's offer was declined, `USED_THIS_MOVE` stays 0; the shout offers, pays
   one pot, invokes `2` with `COST_PAID`, and the body sets `USED_THIS_MOVE` / `HALF_PRICE`
   itself.
4. the redirect re-raise at the destination dispatches both residuals again;
   `USED_THIS_MOVE` and the shout's own latch both refuse.

There is always at most one skill use per landing. The shout's own once-per-move
latch is still needed for the case the poem instance is not on the field (the
harness `Table::vanilla`; a skill that has been removed), where no poem offer
exists at all.

The order has one cost: the player is asked for the skill's four pots first and
must decline to take the shout's one-pot route. When the shout is the cheaper
route, the skill could stand down on its own; that needs the skill body to scan
for a placed shout, and is left as open question 1 rather than added here.

### 7.4 Mutsumi's destination reading

The invocation fix does not touch row 426. `branch_crystals` sets
`ctx::set_dest(ctx::Dest::Banished)` (`mutsumi_never.rs:99`) although only (3)
prints 「此卡[移除]」. The engine default for a played card is the graveyard,
so (2) should probably not set a destination at all. It is small, same body,
and listed as an open question (§14 q4) rather than smuggled into this item.

## 8. Save, record and replay

* **SAVE_VERSION unchanged (16).** `skill.tomoriPoem.halfPrice` is a player slot;
  slots are a dynamic key/value map. `Scheduled` is already saved and already
  carries `card`, `owner`, `target`, `skip`, `early`, `uid` and `entry`
  (`world.rs:466-495`); the owed settle uses that shape. The two renamed key
  strings (`acted` → `usedThisMove`, `due` → `owedSettle`) cost at most one
  extra use when a save made before the rename is loaded mid-move; the pending
  schedule carries its entry identifier, so the delayed settle itself is not
  lost. The `TurnEnd` gate that read the old `due` key is removed. A pause inside the
  borrowed body returns `EXIT_NEED_INPUT` through the caller; the resume
  re-runs the outer entry and re-derives the whole chain. The invocation frame
  stack is transient (`host.rs:3189-3196`).
* **RECORD_VERSION unchanged (4).** Entry identifiers are body literals; the
  trigger and the roll answers are recorded as today. The 1d20 uses the
  serialized random stream.
* **Replay.** A record made after this change replays on a bundle that has it;
  an archived bundle replays on its own host. Nothing in the record names an
  import.

## 9. Versioning

**No bump of its own.** The surface is the G05/v111 adoption described in
`abi.rs:771-782`; this revision adds card code only. If a reviewer wants the
follow-up recorded, append one line under the v111 block:

```
/// v111 (continued, G13): the borrowed-clause follow-up is card-only --
///    `skill:高松灯:诗超绊` names its (2) `"2"` and honours
///    `ctx::invoke_paid()`; the shout invokes it paid and the body schedules
///    the owed settle on the mover. No wire change.
```

## 10. Tests

### 10.1 Item 2

File: `rules/tests/tests/crychic.rs`, beside the existing window tests
(`:1503-1600`). The two offer-order tests bind the persona skill explicitly
(the same setup as `mygo.rs`'s skill tests); the rest place only the shout.

| test | assertion |
|---|---|
| `tomori_shout_pull_runs_the_borrowed_second_body` | the skill's own `tomori_moved` log appears (proves the body, not the copy); the mover is on the controller's tile |
| `tomori_shout_pull_rolls_once_and_leans_on_six_or_less` | forced 6 ⇒ one `stay` and one owed schedule; forced 7 ⇒ neither |
| `tomori_shout_spends_one_fire_not_four` | pool 2, exactly one spent (`COST_PAID` suppresses the four) |
| `tomori_shout_delayed_settle_runs_at_the_movers_next_turn_end` | the settle fires at the mover's next turn end, on the mover's position, and then clears |
| `tomori_shout_delayed_settle_pays_half` | the owed settle is halved once, not quartered |
| `tomori_shout_does_not_offer_when_the_skill_already_ran` | accept the skill's own offer ⇒ the shout offers no second use |
| `tomori_shout_offers_after_the_skill_offer_is_declined` | decline the skill's offer ⇒ the shout still offers |

File: `rules/tests/tests/mygo.rs`, for the persona path without the shout:

| test | assertion |
|---|---|
| `tomori_second_settles_for_the_mover_at_their_turn_end` | the settle is the mover's, at the mover's position, at the mover's turn end |
| `tomori_second_halves_every_payment_of_the_forced_settle` | a roll above 6 still halves the forced settle (the current `OWED_SETTLE` gate would not) |

The existing `tomori_shout_pulls_to_the_controllers_own_tile_with_settle`
stays; its half assertion becomes meaningful rather than quartered.

### 10.2 Item 1 coverage still owed

File `crates/game-rules/tests/declare_arg.rs` (extension of `:497-531`):

* `invoke_binds_the_target_and_restores_the_outer_frame` — target `self_uid`
  inside the body, caller restored after;
* `invoke_seeds_and_restores_the_trigger_card`;
* `invoke_unknown_entry_is_a_noop`;
* `invoke_unknown_card_traps`;
* `invoke_filter_entry_traps`;
* `invoke_skill_is_invoke_entry_press`;
* `play_card_still_picks_the_play_entry`.

File `rules/tests/tests/morfonica.rs`:

* `nanami_press_invokes_skill_two` — one pot, one mark, the `nanami_mark`
  log, no `nanami_which`;
* `nanami_press_spends_one_crystal_only`;
* `nanami_press_without_pot_refused` — no crystal is spent.

And extend `card_paths.rs::mutsumi_never_2_runs_the_band_skill_2_on_a_crystal_shortfall`
with `assert_eq!(t.money(0), m0)`.

Static: `on_parse.rs` (`.id` on the `SettleBefore` survives the manifest),
`kind_table_pin.rs` (unchanged kinds).

## 11. Kani obligations

The Kani host records the invocation rather than running it
(`rules/kani/src/host.rs:2400-2465`): `invoke_n`, `last_invoke = (card, entry,
paid, by_instance)` and a bounded stack. Proofs:

* `tomori_pull_invokes_paid_iff_accepted` — `paid == true`, one pot spent, no
  invocation on a declined answer or a failed substitution;
* `tomori_follow_up_iff_the_mover_is_not_the_owner` — the follow-up arm is
  `mover != player_id`, not `cost == 4`;
* `tomori_owed_settle_marks_the_mover_iff_the_roll_is_low` — `stay` and `OWED_SETTLE`
  only on 6 or less;
* `tomori_owed_settle_clears_its_latches_iff_it_settles` — `OWED_SETTLE` and `HALF_PRICE` are
  zero after;
* the `invoke_paid` model read is `last_invoke.paid`.

## 12. Alternatives

1. **Finish the copy instead** (roll, stay, schedule in the shout). No step
   forward: the shout already drifts, the hand-written destination logic would
   be duplicated, and nothing binds the skill instance. Rejected.
2. **Two entries with an `OFFERED` latch** (the draft). Correct only if the
   borrowed call names the second entry *and* the effect can be reached
   without the dispatch pass; more state and a dispatch-order trap. Rejected
   for a single entry.
3. **Keep the half-price latch on the shout** and make the poem's filter
   stand down when borrowed. The settle filters run after the frame is popped,
   so a body cannot see that it was borrowed; it would need a third persistent
   marker. Rejected.
4. **Let `ctx::settle_tile_at(..., true)` stand.** The owed settle is then
   reported as a main move and on-main-move hooks (row 734's shape) answer
   wrongly. Rejected.
5. **Raise a synthetic `cardUsed` for the borrowed run** so the skill's own
   condition re-runs. A second counteraction window opens on an already
   committed line and every `cardUsed` listener re-runs. Rejected; body-only
   is the landed contract.
6. **A declared argument carrying the mover** (`invoke_entry::<i32>`). One
   more value crosses only to avoid reading `trigger.player_id`, which the
   inherited trigger already supplies. Rejected for this item.

## 13. Necessity

Two questions decide the group: can the defect actually be reached, and is
there a smaller route than one named-entry invoke? The first is yes and
player-visible; the second has no surviving candidate.

### 13.1 The defect is reached in normal play

「内心的呐喊」 is a shipped card played by 高松灯（CRYCHIC） once the band has
transformed to MyGO!!!!!. Its offer fires on an ordinary landing and the
existing tests pin that window (`crychic.rs:1503-1600`). The player answers,
pays one pot, and gets only the teleport: the roll, the [停留], the delay and
the half-price of the skill's own settle never happen (site TODO,
`tomori_inner_shout.rs:220-222`; row 408). The visible failure is not an edge
case behind a gate — it is the body of the card.

The same shape broke two other cards before G05: 七深 (3) with a crystal and no
mark burned the crystal and ran the skill's *other* clause (row 735), and 睦
(2) invoked the band skill with no bound instance, so the crystal cash-out
read `0` and the spend never happened (row 426's neighbourhood, now pinned at
`card_paths.rs:433-436`). Borrowed clauses are a live, player-facing mechanic,
not a hypothetical.

### 13.2 No existing surface can address the clause

Every "run that card" route resolves the first `On::Play`:

* `ctx::play_card` and `ctx::invoke_skill` both pass `entry::PRESS`;
  `PRESS` resolves `play_entry_preferring_gated()` (`host.rs:188-190`);
* **the poem has no `On::Play` at all.** Its entries are all `Action`
  (`tomori.rs:42-65`). `play_card` or `invoke_skill` on it finds no Play,
  returns `DEST_UNSET` (`hostfns.rs:1632-1634`) and runs **nothing**. There is
  no route to the poem's (2) before this group;
* `query_guard` probes an entry but is read-only (`hostfns.rs:3317-3362`);
* `declare` collects an argument and does not run a body (`ACTIVATION.md` §2);
* a copy is the only fallback, and it cannot bind the skill's instance.

The first-Play convention does work for *some* skills, by coincidence: 鳰原
令王那's (2) is its `On::Play` (`pareo.rs:33`), 冰川纱夜's press is its (2) as
well (`sayo.rs:34`). A caller therefore cannot tell "the skill's press" from
"the skill's (2)": on 七深 and 诗超绊 they differ. That is exactly the class of
defect row 735 records, and it is why `PRESS` and a clause identifier must be
two different resolutions (§2.2).

### 13.3 The borrowers are not one card

Cross-card use is printed on several cards and is already implemented in
several places:

| card | clause | how it reaches the other card |
|---|---|---|
| `Mor:（NNM）稍微努力了一下` (3) | 「视为发动你的（2）技能」 | `invoke_entry(…, "2")` — landed |
| `CRYCHIC:（睦）从没有觉得...` (2) | 「立即执行乐队技能的（2）效果」 | `invoke_skill` — landed |
| `CRYCHIC:（灯）内心的呐喊` (3) | 「使用一次高松灯（MyGO!!!!!）的（2）技能」 | **this item** |
| `CRYCHIC:春日影` (2) | 「使用一次Crychic角色的技能」 | `play_card` + `PRESS` (`haruhikage.rs:109-127`) |
| `R:（纱夜）弹奏弹奏弹奏，继续弹奏` | 「视为使用一次此卡使用者的技能」 | announces only; the invoke is the TODO (`sayo_play.rs:58-68`) |
| `HHW:梦幻的回礼` | 「自动使用一次你的乐队技能」 | copy |
| `Mujica:（海铃）` | takes every band skill card | no run yet |
| `Mujica:（睦/mortis）表演的本能` | copies the previous card | `play_card` (`mortis_instinct.rs:40`) |

Five of the eight already go through a run import; the other three are
announcements or copies. A fix shaped only for 灯 leaves the same first-Play
assumption under 春日影, 纱夜 and 表演的本能, where it is wrong whenever the
borrowed clause is not the first Play.

### 13.4 What the copy alternative costs, measured

Finishing the shout's copy is the only alternative that needs no shared body.
It is not cheaper:

* **It already drifted.** The copy halves unconditionally (as the sheet's
  semicolon says) while the skill gates the half on the roll; the skill's own
  delay settles the controller's tile at the controller's turn end while the
  copy has no delay at all (§4.2, §4.4). Two implementations have already
  produced three different readings of one pair of sentences.
* **The sheet numbers clauses, not bodies.** Both callers want "the (2)"; a
  copy re-derives when the (2) fires and what it costs, so every future
  correction (the delay target, the half, a ruling on 421) is two edits with
  no compiler link between them.
* **A copy cannot bind.** The borrowed body must answer `is_placed`,
  `self_uid` and its own counters for the skill instance. Only the engine
  frame does that; a second copy would either re-implement the binding
  (impossible in the guest) or leave the instance unreadable. 睦's cash-out is
  the proof: before `enter_invoke` it read `0` (`card_paths.rs:433-436`).

### 13.5 Every piece is load-bearing

| remove | what breaks | evidence |
|---|---|---|
| the entry identifier `2` | the poem is unreachable (no Play) | §13.2 |
| the instance binding | the body reads a void instance; 睦's cash-out is 0 | `card_paths.rs:433-436` |
| the trigger seed | 「该技能」/「该角色」 resolves to the shout | nanami mark name, §2.3, trigger replacement |
| `COST_PAID` | the body charges four pots after the shout charged one | §7.1 |
| the schedule on the mover | the settle lands on the controller, on the controller's tile, at the controller's turn end | §4.2 |
| the single `HALF_PRICE` latch | the charge quarters (two filters) or is lost after `MoveResolved` | §4.4 |
| the `USED_THIS_MOVE` read in the shout | the skill runs twice on one landing | §4.5 |
| the body's own teleport | the caller and the body both teleport | §4.4 / R10 |

The minimum version is therefore the single-entry, paid, mover-scheduled,
single-latch design. The draft's second entry adds state without removing any
of the above, and the copy alternative removes none.

### 13.6 What would make the group unnecessary

* **A ruling that a borrowed use pays the borrowed card's own cost.** Then
  `COST_PAID` goes; the body still has to be reached, so §13.2 stands.
* **A universal "skill use" event** that carries the clause and runs it (a
  larger surface than one entry identifier, and still needs the instance).
* **A ruling that only a first Play is ever borrowable.** 灯's shout would
  still fail because the poem has none; 七深's (3) would still recurse into
  (3) instead of (2).
* **Rejecting the group after G05.** G05 already ships calls to this surface;
  rolling it back would re-break 七深 and 睦, and leaving it would leave the
  poem's (2) reachable only by a copy — the outcome §13.4 rejects.

## 14. Open questions

1. **May the skill's own offer and the shout's both be accepted on one
   landing?** §7.3 says no (the second stands down through `USED_THIS_MOVE`). The sheet
   gives each card its own once-per-move wording and does not rank them; if a
   ruling says a borrowed use is a *separate* use, the `POEM_USED_THIS_MOVE` term is
   removed from `offer_open` and the two might chain.
2. **Does the delayed settle halve?** The sheet writes 「在此技能影响下的支付」
   after the semicolon and the owed settle is caused by the skill; §7.1 halves
   it. A contrary reading halves only the forced settle.
3. **「时机合适时」 (row 421).** The hook is narrowed to main moves. The
   borrowed clause says 「[移动终点]」, which is any move endpoint. Keeping the
   narrowing keeps the TODO; widening it is a timing decision outside G13.
4. **Mutsumi's destination (row 426).** Should (2) leave the played card to the
   engine default? §7.4 says yes, but the mark is `TODO(规则书)`-worthy and is
   not needed for the invocation.
5. **Unplaced target.** A persona skill is always placed; the shout path still
   works if it is not (instance `-1`, the owed schedule captures `uid = -1`).
   A card invoked from a hand or a pile reads `is_placed` false, which the half
   filter no longer depends on. No transient instance is needed.
6. **A missing persona skill.** §7.2 resolves it before the ask, so a missing
   skill is a silent no-op and nothing is paid. A card-side warning message
   could name it, but the shout's own admission already requires a MyGO band
   skill and the persona is bound for the only player of the card.

## 15. Clause match

| card | clause | where it is implemented |
|---|---|---|
| 高松灯 (2) | 「当其他玩家的[移动终点]位于您前后5格内时可以选择使用4个火罐」 | guard `before_settle_applies` (dist arm, `mover != player_id`), cost `4` in `before_settle` |
| 高松灯 (2) | 「在该玩家[触发结算]前强制将该玩家[传送]至自己所在的格子并[触发结算]」 | `teleport_to(mover, player_pos(owner))` inside `SettleBefore`; the redirect re-raises the window at the destination |
| 高松灯 (2) | 「然后使其投掷1d20，若出目小于等于6则获得一层[停留]并在自己的下回合结束时[触发结算]」 | `roll` on the mover, `stay::add`, `OWED_SETTLE` on the mover, `at_next_turn_end(mover, "owed_settle")`, `owed_settle` |
| 高松灯 (2) | 「在此技能影响下的支付价格变为原本的一半」 | `HALF_PRICE` latch around the forced settle and the owed settle, one `PaySplitMul` |
| 内心的呐喊 (3) | 「你位于此卡前后5格内时可在时机合适时消耗1火罐使用一次高松灯（MyGO!!!!!）的（2）技能」 | the hook's `card.placed && dist(pos(owner), card.tile) <= 5`, one-pot `fire::spend`, `invoke_entry_paid(…, "2")` |
