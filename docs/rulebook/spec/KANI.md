# Kani formal-verification pilot (machine-checked card clauses)

Status: **feasibility pilot**. Three representative clauses, unmodified card
bodies, reference specs written from the 规则书 clause docs only.

Deliverable shape: for each clause, a `#[kani::proof]` that asserts
**exactly** the book (iff): post-state == spec(pre), the effect log is exactly
the specified effects (no less / no more), and a false trigger is a no-op.
Each proof is paired with a deliberate MUTANT that Kani must report as failing
(`#[kani::should_panic]`), so the proofs are not vacuous.

---

## 1. Toolchain

| piece | version / path |
|---|---|
| `kani-verifier` | 0.68.0 (`cargo install --locked kani-verifier`) |
| Kani release | `~/.kani/kani-0.68.0` (via `cargo kani setup`) |
| CBMC | 6.11.0 |
| Kani rustc | `nightly-2026-08-21` (1.100.0-nightly) |
| host rustc (non-Kani) | 1.96.1 |
| build dir | `CARGO_TARGET_DIR=/home/monopoly/kani-target` (outside the repo) |

Run from the task worktree:

```sh
export CARGO_TARGET_DIR=/home/monopoly/kani-target
cd /home/monopoly/kani-pilot-wt
cargo kani -p rules-kani --harness <name> --default-unwind 80 --no-unwinding-checks
```

`--no-unwinding-checks` is required for the fixed 64-byte `guest_eq` compare
loop and for `memcmp` over `String::eq` inside unmodified card bodies (see
§5). Every such loop is statically bounded; we do not disable checks on
unbounded constructs.

---

## 2. Backend design

### 2.1 Why not `#[no_mangle] ruleset_*` shims?

The first attempt defined host imports as `#[no_mangle] extern "C-unwind"`
(the same shape `rules-native` uses). Kani **does not resolve** those as
calls to the linked definitions: it treats `card_sdk::ctx::sys`'s `extern`
block as foreign C (kani#2423). With `-Z c-ffi` the definition is still
"missing". Additionally, `extern "C-unwind"` monomorphizes `catch_unwind`,
which ICEs kani-compiler 0.68.0 (`intrinsics.rs:243`).

### 2.2 `cfg(kani)` dispatch (minimal, generic card-sdk change)

Under `cfg(kani)` the `sys` surface becomes ordinary Rust that funnels every
import through one generic hook:

| file | role |
|---|---|
| `rules/card-sdk/src/kani_host.rs` | `Op` enum (one variant per `ruleset_*`), `install(Backend)`, `call(Op, [i32;6]) -> i64` |
| `rules/card-sdk/src/ctx/sys_kani.rs` | generated wrappers, same signatures as the `extern` block |
| `rules/card-sdk/src/ctx/sys.rs` | `#[cfg(not(kani))]` keeps the real `extern "C-unwind"` block; `#[cfg(kani)]` re-exports `sys_kani` |

No game vocabulary lives in card-sdk. The verifier crate installs a backend
at proof start (`rules_kani::host::install()`).

Also under `cfg(kani)` (still generic, no game words):

* `card_sdk::native` arena uses a `static mut` instead of `thread_local!`
  (TLS monomorphizes `catch_unwind`).
* `Msg` payload is `()` instead of `BTreeMap<String, Arg>` (BTreeMap Drop does
  not unwind). Builders still exist so card bodies compile unmodified.
* `Msg::to_bytes` is empty; `mj` interns empty bytes. Narration is recorded
  as a counter only — the book's **frame** is about game effects.
* `ctx::cards_in` decodes a `[n, id…]` byte layout instead of postcard.

Production / wasm / `rules-native` builds are unchanged (`cfg(not(kani))`).

### 2.3 Symbolic world (`rules/kani/src/world.rs`)

* N ∈ 2..=4 players; symbolic money / out / pos / fire / stay / fans
  (`P✽P粉丝(正)` + `(反)`).
* Discard / hand as bitsets over 4 interned card ids (no `String` in the model).
* Pending pay trigger: `trig_player`, `pay_amount` (rewritten by `set_pay_amount`).
* Prompt answers: symbolic index (`kani::any` + range assume) when
  `symbolic_answers` is set — **every pick branch is explored in one proof**.
* **Effect log** (bounded, 24): `SetPayAmount` / `Teleport` / `GainFire` /
  `GiveStay` / `PlaceCard` / `TakeFromPile` / `AddToHand`.
* **Narration** is a separate counter (not part of the frame).

### 2.4 Running unmodified card bodies

`rules/kani/src/invoke.rs` pulls the `fn(i32) -> Asked` out of the card's
`On` table (`card_pp::aya_longing::AYA_LONGING`, `card_ppp::tae_sound::TAE_SOUND`)
and calls it. The card sources are **not** copied or rewritten.

---

## 3. How to write a harness from a clause doc

1. Open `docs/rulebook/spec/cards/<group>.md` (or `cards.json`).
2. Copy the **Quote** into `rules/kani/src/spec.rs` as a pure function over
   pre-state + effect log + post-state. Never look at engine behaviour.
3. In `rules/kani/src/proofs/<slug>.rs`:
   * build a symbolic pre-world (`kani::any` + `assume`ed ranges);
   * `host::install(); world::reset_world(w);`
   * invoke the unmodified entry;
   * assert `post == spec(pre)` **and** `effect_log == exactly(spec_effects)`
     **and** (when the trigger/condition is false) the log is empty;
   * add `kani::cover!` on both branches;
   * add a `#[kani::should_panic]` MUTANT (spec off-by-one / extra effect).
4. Isolation assumptions (e.g. "no [共鸣] fuel in hand" to pin one clause of a
   stacked hook) must be stated in the module docs and `assume`d, not hidden.

---

## 4. Representative clauses and results

### (a) Money-shave hook — `C-PP-aya-haruka-02`

> 「（1）如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]或[支付]时
> 将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。」

* Card: `PP:[丸山彩]憧憬的前方` `On::Filter(PayAdd)` (unmodified).
* Isolation: no `PP:[衍生]共鸣]` in hand (clause (2)'s stacked 1500 shave
  cannot fire). Host-side condition `value > 0` is assumed.
* Spec: `spec::aya_haruka_02_{x,amount,log_ok}` + `is_poorest`.

| harness | result | time | notes |
|---|---|---|---|
| `aya_haruka_02_shave_iff` | **SUCCESSFUL** | 203.8 s | 0/2909 failed; **2/2 covers** (poorest / not-poorest) |
| `aya_haruka_02_mutant_x_off_by_one` | **SUCCESSFUL (panic as expected)** | 207.4 s | 1/2864 failed = the mutant assert — non-vacuous |

### (b) Teleport / status card body — `PPP:（多惠）寻找更好的声音`

> 「传送到"江户川乐器店"并获得一个[火罐]」

* Card: `On::Play` (unmodified).
* Spec: effect log is exactly `Teleport(shop)`, `GainFire(1)` in that order;
  post is on the shop with fire+1; frame holds.

| harness | result | time | notes |
|---|---|---|---|
| `tae_sound_teleport_and_fire_iff` | **SUCCESSFUL** | 68.3 s | 0/2146 failed; **2/2 covers** |
| `tae_sound_mutant_extra_fire` | **SUCCESSFUL (panic as expected)** | 47.8 s | mutant wants a third effect |

### (c) Prompt / choice — `C-PP-aya-haruka-01`

> 「[手]：将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。」

* Card: `On::Play` (unmodified) — `ctx::ask_card` over the discard.
* Spec: `PlaceCard(Field)` + exactly one `TakeFromPile(Discard)` +
  `AddToHand`; empty discard ⇒ place only.
* Answers: symbolic index into the option list (all picks in one proof).

| harness | result | time | notes |
|---|---|---|---|
| `aya_haruka_01_prompt_all_picks_iff` | **incomplete at pilot deadline** | >15 min | blocked by `String::contains` / `memcmp` in the unmodified play body (`ids.contains(&c)`); see §5 |
| `aya_haruka_01_empty_discard` | **incomplete at pilot deadline** | >15 min | same `String` path, even when the loop body is empty |
| `aya_haruka_01_mutant_two_cards` | not run | — | same blocker |

The **architecture** for (c) is in place (symbolic answers, effect-log
equivalence, empty-discard case). Finishing it is a matter of killing the
remaining `String`/`memcmp` unwinding (below), not of designing the proof.

---

## 5. What blocks scaling (measured)

| cliff | symptom | mitigation used / still needed |
|---|---|---|
| `extern "C-unwind"` host imports | foreign-call unsupported / `catch_unwind` ICE | `cfg(kani)` `kani_host` dispatch (done) |
| `thread_local!` in `native` | `catch_unwind` ICE | `static mut` arena under `cfg(kani)` (done) |
| `Msg` = `BTreeMap<String, Arg>` | BTreeMap Drop / postcard unwind | unit payload under `cfg(kani)` (done) |
| `str::from_utf8_lossy` | Utf8Chunks loop | byte compares (done) |
| `postcard` `Vec<String>` | varint + serde seq | `[n, id…]` layout (done) |
| `String::eq` / `Vec::contains` in **card bodies** | `memcmp` unwind | `--no-unwinding-checks` + fixed 64-byte `guest_eq`; card-body `contains` still expensive (prompt) |
| `String` / `Vec` / `BTreeMap` in general | unwinding bounds | keep capacities tiny; consider a `cfg(kani)` interned-str facade for card bodies **or** accept 5–20 min/harness |

**Unwinding bounds used:** `--default-unwind 80` + `--no-unwinding-checks`
for harnesses that compare UTF-8 names; `--default-unwind 10` was enough for
(a) once Msg/TLS were fixed.

### What must be hand-written vs run unmodified

| run unmodified | hand-written Kani contract / stub |
|---|---|
| Card `On::*` bodies (`fn(i32) -> Asked`) | Host model (`rules/kani/src/host.rs`) |
| `ctx::*` call path | `kani_host` backend (generic) |
| Guard/condition **strings** (CEL) | **not** run under Kani — the harness `assume`s the host-side condition (e.g. `value > 0`) and proves the body against the clause's own Condition |
| Engine scheduling, chain/counteract windows | out of scope — see §6 |
| Multi-module `play_card` nesting | stub as a single inner call; prove the inner body alone |

---

## 6. What Kani can NOT prove here (and what would)

* **Multi-turn chains / [反击] windows / settle pipelines** — the engine's
  scheduling is a concurrent, replay-with-abort protocol. Kani proves a
  straight-line card body against a snapshot. Complementary method:
  **TLA+/Alloy or SMT (Z3)** over the chain/settle protocol
  (`docs/ENGINE.md`), with the per-clause Kani results as the leaf
  refinement.
* **Unbounded loops / unbounded player counts** — the model is N≤4 and every
  loop is a fixed bound. A clause that quantifies over "all future turns"
  needs an inductive invariant (Kani loop contracts, still experimental).
* **CEL guard strings** — compiled at ruleset build; Kani does not evaluate
  them. The harness must state the condition as an `assume` and the clause
  doc's Condition as the proof obligation on the body.
* **String identity of card ids in general** — only 4 interned ids are in the
  pilot model. Scaling to ~180 cards needs an intern table (u16) on both
  sides, not `String`.
* **wasm-only paths** (bump allocator, wasmi) — not modelled; native
  `rules-native` semantics are the stand-in, which is the same Rust.

---

## 7. Plan + effort estimate (~180 cards + 66 skills + 28 events)

Assume the (c) `String` cliff is fixed (interned ids in card bodies under
`cfg(kani)`, or a 10× CPU budget).

| phase | content | effort | wall-clock (est.) |
|---|---|---|---|
| 0 | Toolchain + `kani_host` + world + 3 pilots | **done** | 1–2 days |
| 1 | Interned-str facade; finish (c) prompt + mutants | 1–2 days | minutes/harness |
| 2 | Money / pay pipeline clauses (~20 hooks like (a)) | 3–5 days | ~3–5 min each |
| 3 | Teleport / status / stay-stun-exile (~25) | 3–5 days | ~1–3 min each |
| 4 | Hand / draw / discard / field placement (~30) | 5–8 days | ~5–15 min each |
| 5 | Prompt/choice cards (~40) | 8–12 days | ~10–30 min each |
| 6 | Skills (66) + events (28) — mostly hooks + prompts | 10–15 days | same mix |
| 7 | CI: one harness per clause, fail-closed, no ignores | 2–3 days | batch by band |

**Total: ~6–8 engineer-weeks** to cover the clause set at pilot quality
(iff + mutant + cover), assuming the interned-str work lands first and
harnesses stay at 1–30 minutes. CPU is the bottleneck: a full suite of ~270
clauses × 2 harnesses × ~5 min ≈ **45 hours** of CBMC — parallelise by band
on a 16-core box ≈ 3 hours/night.

**Must stay hand-written:** the reference `spec::*` functions (from the
book), the symbolic world, and the per-clause isolation assumes. The card
bodies themselves run unmodified.

---

## 8. Repo map

```
rules/kani/                    crate `rules-kani` (ROOT workspace member,
                               NOT in rules/ wasm workspace)
  src/world.rs                 symbolic world + effect log
  src/host.rs                  kani_host backend (game vocabulary lives here)
  src/invoke.rs                run unmodified CardDef entries
  src/spec.rs                  reference specs FROM clause docs
  src/proofs/money_shave.rs    (a) + mutant
  src/proofs/teleport.rs       (b) + mutant
  src/proofs/prompt.rs         (c) + empty-discard + mutant
rules/card-sdk/src/kani_host.rs        generic Op table + install()
rules/card-sdk/src/ctx/sys_kani.rs     generated kani sys surface
rules/card-sdk/src/ctx/sys.rs          cfg split (extern vs kani)
rules/card-sdk/src/native.rs           cfg(kani) static arena
rules/card-sdk/src/msg.rs              cfg(kani) unit payload
docs/rulebook/spec/KANI.md     this file
```

Discrepancies found by the pilot: **none** in the two completed clauses —
the unmodified bodies match the book under the stated isolation. The
incomplete prompt clause is not evidence either way.

---

## 9. Honest bottom line

Kani **can** machine-check that a card body does exactly one clause's
Trigger/Condition/Effect/Frame, with symbolic inputs, against a
book-derived reference, in minutes per harness — once the host is a Rust
`cfg(kani)` backend rather than `extern "C-unwind"` shims, and once
`String`/`BTreeMap`/`postcard` are kept out of the unwinding path.

Kani **cannot** replace the engine's chain/settle protocol model; pair it
with an SMT/TLA+ model of the protocol. Existing unit tests stay regression
tests and are not this deliverable.
