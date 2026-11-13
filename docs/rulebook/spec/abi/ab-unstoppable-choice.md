# STA-02 — the [不可阻挡] holder's per-effect choice

Backlog: `docs/rulebook/BACKLOG.md`, slug `STA-02` (class B, medium), rows
403, 404, 1035, 1454. Landing: G12 §5.1's gate refusal becomes **choosable**;
the G12 doc keeps the original design and gains a "Landed (STA-02)" section.

## 0. The clause

Glossary 14 ([不可阻挡]), `docs/rulebook/cards.json` / `data/rules.txt` 51:

> \[不可阻挡\]：处于该状态时无视已存在的\[停留\]，\[晕眩\]效果也不会获得新的层数；无法获得新的\[除外\]层数。\[停留\]，\[晕眩\]，\[除外\]在适当时机依旧掉层。**处于该状态时可选择受到的\[传送\]，\[强制移动\]，\[强制停下\]效果是否生效。**

The first sentences are STA-01 and the G12 landing (grant refusals, stay
exemption, unchanged decay). The bolded one is a **choice**: a received
movement abnormal is not simply refused for the holder — the holder says
whether that landing may take effect.

## 1. Reading

* 「受到的」 scopes the clause to moves caused by another player. A card's own
  `run_planned_move` (e.g. 里美's 「期间[不可阻挡]」) is *chosen*, not
  received, and must keep walking without a prompt.
* 「[传送]，[强制移动]，[强制停下]」 names the movement abnals, engine kinds
  4–7 (teleport, forced move, forced stop, reverse). The reverse shares the
  [强制停下] gate family and takes the same choice.
* 「是否生效」 is per effect, not a turn flag: the same holder may allow one
  teleport and refuse the next.
* The choice is only meaningful where the G12 gate would otherwise refuse —
  i.e. the ruleset closed it. A kind the gate leaves open never prompts.
* **Yes** means the effect lands exactly as for an unlimited player (a
  teleport moves, a forced move walks, a stop / reverse lands its marker); the
  landing is still measured for blocked-ness (a forced walk still holds the
  move).
* **No** is today's refusal: `abnormalRefused` fires, no `effect` window
  opens, `log.unstoppable_skip` is logged, and the move is not held.

## 2. Design

The choice is an **engine-raised pause**, not a filter body. The engine owns
the raise point (it alone knows the gate closed and the causer); the ruleset
owns the question and the answer semantics. This keeps the gate generic: a
future status with a per-effect opt-in reuses `abnormalChoice` with its own
condition.

### 2.1 Kind

New engine kind **143**, wire `abnormalChoice`: an `ActionKind` /
`TriggerKind` pair, `is_filter: false`, `hook_only: true` (it is raised at a
fixed engine point, never declared by a card's `On::Hook`). Appended after
HOST-03's `payCalcBefore` (91 in the action pin, 143 in the kind table).

### 2.2 Raise point — `wasm_rules.rs::abnormal_gate`

Inside the `!gate_open` branch, after the gate's refusal read and before the
refusal itself:

```rust
if !cx.world().gate_open(self, ask, s) {
    // 「处于该状态时可选择受到的[传送]，[强制移动]，[强制停下]效果
    // 是否生效」 (glossary 14)
    let chosen = if by_player != player_id && (4..=7).contains(&kind.as_i32()) {
        let t = self.raise_core(cx, "abnormalChoice", player_id, on)?;
        t.abnormal_allowed
    } else {
        false
    };
    if !chosen {
        self.refuse_abnormal(cx, player_id, by_player, kind, on)?;
        cx.log(player_id, self.gate_skip_msg(ask, s).player_id("who", player_id));
        return Ok(false);
    }
}
```

* The raise targets the **holder** (`player_id`); the payload is the abnormal
  (`target`, `value = kind`, `by_card = by_player`) — the same `on` closure
  the nearby refusal hooks use.
* A self-caused move skips the raise (`by_player != player_id`), and so does
  every non-movement kind (stay / stun / exile keep their plain gate read).
* The window is a normal pausable action window: `raise_core` journals the
  raise, the prompt pauses and replays, and the answer is re-read on the
  resumed run.

### 2.3 Answer — the status body

`rules/status/src/unstoppable.rs`:

```rust
On::Action(Action::AbnormalChoice(
    "state(actor,'unstoppable') > 0 && value >= 4 && by != actor",
    None,
    on_abnormal_choice,
)),
```

```rust
fn on_abnormal_choice(_owner: i32) -> card_sdk::Asked {
    let p = trigger::player_id();
    let title = Msg::new("ask.unstoppable_choice.title");
    let text = Msg::new("ask.unstoppable_choice.text").player_id("who", p);
    if ctx::ask_yes(p, &title, &text)? {
        trigger::allow_abnormal();
    }
    Ok(())
}
```

* The host runner invokes board-owned status hooks with
  `player_id = BOARD_OWNER` (−1), so the holder is the raise seat
  (`trigger::player_id`), not `_owner`; the condition is keyed on `actor`
  (the invoke seat, read from the trigger) for the same reason.
* `ctx::ask_yes` is a `YesNo` prompt: option 0 是 / 1 否, **fallback 1**. A
  timeout and the standard bot therefore refuse, which is the pre-STA-02
  behaviour and the safe default.

### 2.4 The answer back — `Trigger.abnormal_allowed`

`ctx::trigger::allow_abnormal` is a trigger write, the same shape as
`ctx::trigger::cancel`:

```rust
pub fn allow_abnormal() { unsafe { sys::trig_set_abnormal_allowed() } }
```

* Guest: `sys::trig_set_abnormal_allowed` (`link_name
  ruleset_trig_set_abnormal_allowed`) → `hostfns::trig_set_abnormal_allowed` →
  `CardWorld::set_trigger_abnormal_allowed` → `self.trigger.abnormal_allowed = true`.
* The flag rides the guest `world::Trigger` mirror back to `CoreTrigger` at the
  two `counteract` write-back sites, so `raise_core`'s returned trigger carries
  the holder's answer. The gate reads `t.abnormal_allowed` and only then
  decides whether to refuse.
* On a replay the body re-runs against the re-raised trigger, re-reads the
  recorded prompt answer and re-writes the same flag before the gate consumes
  it — the answer is journaled with the prompt, not in the save.

`Trigger.abnormal_allowed` is a plain in-memory bool (default `false`) on the
non-serialized engine `Trigger`; `SAVE_VERSION` / `RECORD_VERSION` are
unchanged.

### 2.5 The full ABI surface

| layer | addition |
|---|---|
| `crates/game-kinds/src/lib.rs` | kind row 143 (`abnormalChoice`, `hook_only`), `from_wire` arm, HOOKS pin |
| `crates/card-sdk/src/abi.rs` | `ActionKind::AbnormalChoice` / `TriggerKind::AbnormalChoice` doc constants |
| `crates/card-sdk/src/kinds.rs` | engine action row, action pin 91 → 92 |
| `crates/card-sdk/src/ctx.rs` | `trigger::allow_abnormal` |
| `crates/card-sdk/src/ctx/sys.rs` | extern `trig_set_abnormal_allowed` |
| `crates/card-sdk/src/ctx/sys_kani.rs`, `kani_host.rs` | Kani `Op` 499 + dispatch |
| `crates/game-rules/src/{host,hostfns,native_shims,world,wasm_rules}.rs` | host function, `NullWorld` / `CardWorld` method, native shim, both write-backs |
| `rules/kani/src/{host,world}.rs` | model field, dispatch, arm |
| `rules/status/src/unstoppable.rs` | the `On::Action` entry and body |
| `webui/src/i18n/locales/{zh-CN,en}` | `ask.unstoppable_choice.title` / `.text`, `anim.chainKind.abnormalChoice` |

No `ABI_VERSION` bump: the kind and import are additive at the tail and the
release marker carries the new ids.

## 3. Interaction with the rest of the gate

The raise sits **after** out → untargetable → `abnormalGuard` → `immuneAll`:
all four already refused the effect, so their holder never gets a question
that could not change anything. It sits **before** the exemption / blocked-ness
read: a landed, allowed abnormal is still measured like any landing (a forced
walk holds the move, a teleport does not).

A holder that somehow answers yes while the gate is open never prompts, so a
stale yes cannot leak into another effect.

## 4. Tests

`rules/tests/tests/ab_unstoppable_choice.rs`:

| case | test |
|---|---|
| received teleport, yes → lands, no refusal log | `received_teleport_yes_lands` |
| received teleport, no → stays + `abnormalRefused` + skip line | `received_teleport_no_refuses_and_logs` |
| received forced move, yes → walks | `received_forced_move_yes_walks` |
| received forced move, no → no walk + skip line | `received_forced_move_no_refuses_and_logs` |
| received stop, yes → marker lands / no → no marker | `received_stop_yes_lands`, `received_stop_no_refuses_and_logs` |
| reverse rides the same choice | `received_reverse_yes_lands` |
| self-caused walk / self teleport never prompt | `self_caused_forced_move_does_not_prompt`, `self_caused_teleport_does_not_prompt` |

Every case asserts the prompt title, the single asked seat and the two
options, then answers explicitly — the replay re-runs the body and re-reads
the recorded answer instead of relying on `skip_all`. The four
`g12_unstoppable_refuses_a_*` tests now answer no through the same helper,
which keeps their original refusal assertions and C-G12-unstoppable-04
coverage.

`message_args::chain_kinds_and_counters_have_locale_text` pins the
`anim.chainKind.abnormalChoice` label; `tools/i18n/check.py` pins both ask
keys.

## 5. Alternatives

* **A1 — a card-side `On::Hook(AbnormalGate)` prompt.** The filter body is
  CEL-only, so the prompt would have to run inside `gate_open`, and the gate's
  two reads (refusal and exemption) would both prompt; the engine raise runs
  once, at the single landing point.
* **A2 — a turn flag.** 「是否生效」 is per effect; a flag set once would
  answer later abnals without asking.
* **A3 — refuse first, prompt later.** The refusal closes the window before
  the answer exists; the raise has to precede `refuse_abnormal`.
* **A4 — ask with a custom option list.** The yes/no prompt's fallback already
  gives the timeout and the standard AI the pre-STA-02 refusal; a custom list
  would need its own fallback ordering.
* **A5 — a new filter `AbnormalChoiceGate`.** The status already raises an
  action family; a filter shape would need a second read path into the same
  gate.

## 6. Open questions

1. **Should the prompt fire when the effect is already exempt by another
   rule?** Today it does not: the engine asks only when `gate_open` returned
   closed. A second card's exemption opening the gate would skip the holder's
   question. Reading: correct — a landing that cannot be refused needs no
   answer.
2. **Does a chosen landing count as 「受到影响」 for the cards that read the
   window?** It reaches the `effect` window like any landing, so yes; a
   refused one stays outside it, consistent with G12 §11.1.
3. **Repeated prompts for one effect family.** A multi-landing effect prompts
   once per landing. The clause says 「效果」 (the effect), and the engine
   raises per landing; a single ask per effect would need the raise to move
   into the caller.
