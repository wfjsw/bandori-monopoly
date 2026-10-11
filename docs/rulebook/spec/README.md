# Formal verification spec (规则书 → executable clauses)

Each rulebook sentence that constrains the engine becomes a **clause**: a doc
entry with a verbatim quote, plus an executable Rust predicate checked over
every observed transition. A discrepancy is a **plain failing test**. No
`#[ignore]`, no known-findings allowlist.

## Purpose

* Turn the 规则书 into something a machine can check, sentence by sentence.
* Give the 21 parallel units a stable harness (`crates/spec-kit` +
  `rules/spec`) so they only write clauses and contracts.
* Surface engine discrepancies as red tests that stay red until the engine
  honours the book.

## Layout: generic kit vs game harness

```
crates/spec-kit/                 GENERIC (root workspace member). No game vocabulary.
  src/lib.rs                       Clause, Step, StepAct, Violation, check()
  src/observe.rs                   observed act choke point over game_core::Match
  src/snap.rs                      World snapshot / diff / frame()
  src/prop.rs                      forall(n, seed, ..) + SplitMix64 Rng

rules/spec/                      GAME-SPECIFIC crate `rules-spec`. Root workspace
                                 member, host-only (like rules/cond). NOT in the
                                 rules/ wasm workspace (its members list is explicit).
  src/lib.rs                       re-exports spec-kit; data/ruleset loading (../../dist)
  src/table.rs                     Table harness (adapted from game-rules tests/common)
  src/fuzz.rs                      arrange + drive (adapted from game-rules tests/fuzz)
  src/global/<area>.rs             CLAUSES, one file per area (units 1-8 own these)
  tests/global_soak.rs             one #[test] per area: fuzz soak + per-act clause check
  tests/example_card.rs            worked per-card contract (the template for units 9-21)
  tests/g_<area>.rs                (unit-owned) scripted scenarios for an area
  tests/card_<group>.rs            (unit-owned) card / skill / event contracts

docs/rulebook/spec/                this README + the clause docs
  global/<area>.md                 one doc per area (units 1-8 own these)
  cards/<group>.md                 (unit-owned) card / skill / event contracts
```

`crates/game-rules/tests` stays untouched. Moving the existing `rb_*` suite
into `rules/spec` is a separate follow-up.

## Source precedence

1. **Live sheet** (newest wins): `docs/rulebook/cards.json`,
   `data/characters.json`, `data/bands.json`, `data/events.json`.
2. **`docs/rulebook/rulebook-doc.md`** for global rules.
3. **TEST-FINDINGS §6 rulings** unless newer sheet text supersedes them.

`rulebook.txt` is **not** authoritative. Never derive a spec from
implementation behaviour, and never from the C#.

## Clause ID scheme

| kind | id | owner |
|---|---|---|
| global | `G-<AREA>-NN` | units 1-8, one area each |
| card / skill | `C-<GROUP>-<slug>-<clause>` | units 9-20 |
| event | `E-<slug>` | unit 21 |

`<AREA>` is `MONEY`, `CARDS`, `STATUS`, `MOVE`, `SETTLE`, `FLOW`, `BANKRUPT`,
`COUNTER`. The doc entry and the code `Clause::id` are the **same string**.

## Clause template

Every clause documents all of:

* **ID** -- per the scheme above.
* **Source** -- `rulebook-doc.md:<line>`, `cards.json#<id>`, `characters.json#<name>`, ...
* **Quote** -- verbatim from the source.
* **Trigger** -- when the clause applies (which observed input / event).
* **Condition** -- the pre-state the clause reads.
* **Effect** -- what must hold of `post` / the event window.
* **Frame** -- everything outside the allowed paths is unchanged
  (`spec_kit::frame`).
* **Rulings & OPEN** -- any ruling the clause depends on, and open questions.
* **Executable** -- the `fn(&Step) -> Result<(), String>` that checks it.

## The discrepancy rule

A discrepancy is a **plain failing test**.

* No `#[ignore]`.
* No known-findings allowlist.
* List each one in your doc's `## Findings` (clause ID, what the book says,
  what the engine does).
* Only fix tests that are wrong (setup illegal per the book, or the assertion
  does not follow from the quote). A genuine engine gap stays red.

## How to write a clause

1. Pick the quote and the area. Read only the sources in the precedence list
   (black-box: never `rules/cards`, `rules/skills`, `rules/fixtures`; never
   grep from the repo root).
2. Fill the clause template in `docs/rulebook/spec/global/<area>.md` (or
   `cards/<group>.md`).
3. Add a `Clause::new(id, source, quote, check)` to the area's `CLAUSES`
   slice in `rules/spec/src/global/<area>.rs`. Units 1-8 only append to their
   own file.
4. `check` follows from the quote alone. Return `Err(detail)` when the step
   breaks it. Never encode observed engine behaviour.
5. Undefined or ambiguous text ([共鸣], [触发结算], 分摊, ...) gets an
   `OPEN-<ID>` entry. Specify only what the text determines. Don't invent.

`tests/global_soak.rs` picks `CLAUSES` up automatically -- never edit it.

## How to write a card contract

See `rules/spec/tests/example_card.rs` (the `通用:10次招募（1回限定）`
template). Shape:

1. Quote the card text in the test header and in `docs/rulebook/spec/cards/<group>.md`.
2. `forall(n, seed, |rng, i| { ... })` over randomized pre-states.
3. Arrange with `Table` setters (unchecked -- the arranged state is not a
   transition), then `t.observe(CLAUSES)`.
4. Drive **one** transition (`t.play`, `t.skill`, ...).
5. Assert exact numbers, then `frame(pre, post, allowed_prefixes)`.

A card unit owns `rules/spec/tests/card_<group>.rs` +
`docs/rulebook/spec/cards/<group>.md`. Group membership for skills comes from
the `band` field in `data/characters.json`.

## How to run

```bash
# 1. Ruleset (once; produces dist/cards + dist/fixtures). Do NOT set CARGO_TARGET_DIR.
node tools/build-ruleset.mjs

# 2. Host builds (shared target dir per the batch recipe).
export CARGO_TARGET_DIR=/home/monopoly/spec-target-<k>   # k = unit# mod 3
export CARGO_BUILD_JOBS=8

# 3. Your target.
cargo test -p rules-spec --test <your test file>
cargo test -p rules-spec --test global_soak <area>       # global units
FUZZ_ITERS=300 cargo test -p rules-spec --test global_soak <area>

# 4. Nothing else broken.
cargo test -p rules-spec --no-run
cargo build -p game-rules
```

Validating a failure: check that the setup is legal per the book and the
assertion follows from the quote. Debug with `Table::dump_prompt`,
`keys_since`, and `snap::diff`. A genuine discrepancy stays failing, with a
message naming the clause ID and the quote.

## Open questions (index)

Consolidated from the area docs. Book gaps that clauses must not invent:

| OPEN | text | notes |
|---|---|---|
| OPEN-共鸣 | [共鸣] | undefined in the book |
| OPEN-触发结算 | [触发结算] | undefined; appears in 行动阶段 row 15 |
| OPEN-分摊 | 分摊 | undefined |
| OPEN-乐队卡 | 乐队卡 | undefined as a card class |
| OPEN-场上 | [场上] | undefined (used as 「自己[场上]N个[CP点]」) |
| OPEN-收费标价 | 收费标价 | undefined (Sweet Escape gate; TEST-FINDINGS §6) |
| OPEN-强制停下 | [强制停下] | definition truncated (`rulebook-doc.md:93`) |
| OPEN-结算定义 | [结算] | defined twice (`rulebook-doc.md:61` and `:87`) |
| OPEN-回合流程 | 回合流程 rows 1-3 | corrupt; use **行动阶段** (`rulebook-doc.md:199+`) |
| OPEN-支付阶段7 | 支付阶段 step 7 placement | undecided |

Any clause that depends on one of these records the OPEN id in its
**Rulings & OPEN** field and specifies only what the text determines.

## Gotchas (read before writing a clause)

* **`StepAct::who` is an engine member id, not a seat index.** Use
  [`Step::player_of_act`] (`step.player_of_act()`) to resolve it. The Table
  harness seats player `k` as member `k + 1`, but always look the member up.
* **The live prompt is not in `World`.** `Step::pre_prompt_open` /
  `post_prompt_open` carry it. A clause about "at rest" must skip when
  `post_prompt_open` is true -- the step stopped mid-decision.
* **Arranged states are non-canonical.** `Table` setters and `fuzz::arrange`
  write whatever the test asks for (over-limit hands, odd statuses). Only
  transitions are observed; never check the arranged pre-state itself. A
  clause that reads an absolute `post` invariant should also require the same
  fact held in `pre`, or skip when `pre` is already broken.
* **The event tail is capped at 400 entries** (`EVENT_KEEP`). Read per-act
  windows (`Step::events`); never walk further back.
* **Cards have no instance identity** (`Hidden.hand` is `Vec<String>`).
  Exactly-one-zone can only be checked as a multiset.
* **`world_mut` is only legal at rest** (no pending routine). The observer
  never calls it; arrange setters do, so never arrange while a routine is
  pending.
* **Bankruptcy and auction money have no ledger legs.** `G-MONEY-01` and
  friends skip those windows.

## Findings

* **G-CARDS-01** (genuine, stays failing): human seats sit over `handLimit`
  after a draw; only bots auto-discard. Book says discard down immediately.
  Detail in `docs/rulebook/spec/global/cards.md` § Findings.

(Units append here and in their area docs; the coordinator consolidates into
`docs/rulebook/TEST-FINDINGS.md`.)