# G02 — Pay pipeline: fixed legs, factors, kinds and context

Status: **draft, design only** (2026-10-28). No source is edited by this document.
Group: **G02**. Family: payment command context, pay shapes, factor scoping,
settle-tile/outcome evidence and spend windows.

Ground truth is `docs/rulebook/cards.json`; every quoted fragment below was
read there, and every "wrong site" is the current worktree line. Where the
group file's `current`/`expected` disagree with the tree (the tomori item was
written before the uncommitted `fire.rs` whole-cost window landed), the tree
wins and the item is demoted.

Reading order: `docs/rulebook/spec/global/money.md` (G-MONEY-13/14 and the
`OPEN` list), `docs/rulebook/rulebook-doc.md:227-235` (支付阶段),
`docs/rulebook/SETTLE-STAGES.md` (§4 settle entries, §7 move), and
`docs/rulebook/NEGATION-AUDIT.md` (V1-V5). `docs/rulebook/PAYMENT.md` does not
exist; the pipeline description lives in `docs/rulebook/PIPELINE-AUDIT.md`
§1, which this design extends rather than replaces.

## 1. Items

| # | Slug | Sev | Triage rows | Disposition |
|---|------|-----|-------------|-------------|
| 1 | `rui-fixed-100` | high | 226, 230, 959, 1031, 1241 | ABI: `PayShape::Fixed` |
| 2 | `mashiro-pay-kind-rewrite` | medium | 323 | ABI: `set_pay_kind` + `PayShape::PayeeHalf` |
| 3 | `sakiko-pay-factor-composition` | medium | 131, 132 | ABI: scoped `mul_pay_factor` |
| 4 | `kokoro-pay-buy` | medium | 305 | ABI: kind `Buy` + mandatory pay |
| 5 | `kokoro-rent-kind` | high | 306, 953 | ABI: kind `Rent` on a caller-supplied figure |
| 6 | `kokoro-pay-tile` | high | 309, 1005 | ABI: `pay_spec` tile |
| 7 | `tomori-charge-paying-temp-share` | low | 459 | **Demote to A** (`fire.rs:323` whole-cost window already landed) |
| 8 | `popipapapipopa-spend-window` | medium | 572 | ABI: `SpendBefore` for redeem/auction |
| 9 | `aya-stage-2-interactive-hook-missing` | medium | 1397, 1539 | **Demote to A** (`PayCombinedAddBefore`, id 106) |
| 10 | `door-pay-tile-vs-settle-tile` | medium | 1427 | ABI: `settle_tile` on the pay payload |
| 11 | `anon-tokyo-link-rent-figure` | medium | 513, 1388 | row 1388 ABI `rent_quote`; **row 513 demote to A** |
| 12 | `miracle-free-build-choice` (G09's row) | medium | 521 | ABI: `build_cost_effective` |
| 13 | `guerrilla-paid-per-seat` | medium | 1472 | ABI: `paid_in_settle_for` |
| 14 | `repaint-pay-factor` | medium | 835 | ABI: `mul_pay_factor` (move scope) |
| 15 | `taki-pay-factor-leak` | high | 560, 1361 | ABI: `mul_pay_factor` (settle scope) |
| 16 | `rana-pay-factor-multiply` | medium | 1483 | ABI: `mul_pay_factor` (settle scope) |
| 17 | `fire-bird-paid-signal` | medium | 1441 | ABI: `PayReport::settled` |
| 18 | `council-check-cancelled-settle` (G10's row) | medium | 1449 | ABI: `settle_outcome` on `tileResolved` |
| 19 | `kokoro-circlenormal-skip` | medium | 1353 | ABI: `PayShape::SkipNormal` |
| 20 | `vocal-split-leg` | medium | 21 | ABI: split-leg/total flag on `PayCombinedAddBefore` |
| 21 | `tritone-lose-half-unreachable` | medium | 1333 | ABI: `SpendBefore` on the `lose_raw` path |
| 22 | `tomori-charge-paying-skill-scope` | low | 458, 459 | ABI: source flag + whole `n` on `ChargePaying` (459's temp share demoted to A) |
| 23 | `hagumi-money-in-counter-abi` | medium | 1009, 1353 | ABI: per-seat gains counter on the pay payload |

Group coverage against `BACKLOG.md` section B: the consolidated slugs are
`pay-payload-kind-tile` (items 1, 2, 4, 5, 6, 10, 19), `pay-stage-presplit`
(items 9, 20), `raw-money-window` (items 8, 21), `charge-paying-source`
(items 7, 22), `fire-bird-paid-signal` (17), `rent-figure` (11) and
`money-counters` (13, 23). The extra pre-consolidation slugs and rows the
consolidation absorbed ride as follows: **19 `kokoro-circlenormal-skip` (1353)**
→ `PayShape::SkipNormal` beside item 4's kind; **20 `vocal-split-leg` (21)** →
item 9's `PayCombinedAddBefore` split-leg/total flag; **21
`tritone-lose-half-unreachable` (1333)** → item 8's `SpendBefore` on the
`lose_raw` path; **22 `tomori-charge-paying-skill-scope` (458) + temp share
(459)** → item 7, `ChargePaying` carries a source flag and the whole `n`;
**23 `hagumi-money-in-counter-abi` (1009, 1353)** → a per-seat gains counter read
from the same payload as item 13's `paid_in_settle_for`. Items 3, 14, 15 and
16 share one surface; items 4, 5 and 6 share one surface; items 1 and 2 share
the shape word. Nothing below duplicates a per-card import except where the
card genuinely owns a different
fact (a rent quote, an effective build cost).

## 2. Problem, per item

Each row gives the verbatim sheet fragment, the wrong current site, the demand, and the shared root cause(s). Root causes are named **R1-R7** and defined after the list.

### 2.1 `rui-fixed-100` (high; rows 226, 230, 959, 1031, 1241)

Fragment: 「向全场玩家支付100资金（该数值不可被任何效果影响）」.
Wrong site: `rules/cards/card-morfonica/src/rui_devil.rs:34`

**Demand:** the 100 leg moves at face value, untouched by any effect, while the 300-back leg stays shapeable.

```rust
ctx::transfer(player_id, o, 100, &why)?;
```

`ctx::transfer` enters `HostRequest::Pay`, whose handler raises the `effect`
declaration and every split stage. 八潮瑠唯's 「正论暴击」 listens on that
window and may void or double the leg: a hit pays 200, a void pays 0. The
comment at `rui_devil.rs:26` and the TODO at 31 know this. The
300-back leg at 42 must stay shapeable.

Root cause **R1** (no pay shape) and **R5** partly.

### 2.2 `mashiro-pay-kind-rewrite` (medium; row 323)

Fragment: 「此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金」.
Wrong site: `rules/cards/card-morfonica/src/mashiro_pay.rs:47` and `:57-64`

**Demand:** the payment becomes a bank loss at the settled figure, and the payee loses half of that settled figure.

```rust
trigger::set_pay_target(-1);
...
ctx::pay(to, amount / 2, &Msg::new(key!("mashiro_pay_half"))...)?;
```

Redirecting to the bank leaves `p.kind == PayKind::Card`; the engine then folds
`rent` factors and the tile's `tile_money_scale` into a payment the sheet calls
a plain loss. The half is read from the pre-scale `trigger::value()` before any
`paySplitMul`, so a 500-factor gives 250/250 instead of 250/125.

Root causes **R1** (kind/shape locked by the caller), **R2** (no way to read the settled figure) and **R5**.

### 2.3 `sakiko-pay-factor-composition` (medium; rows 131, 132)

Fragment: 「触发结算时进行的[支付]价格减半」.
Wrong site: `rules/cards/card-crychic/src/sakiko_lead.rs:77`

**Demand:** two halvings multiply to a quarter and the result dies with the shared move.

```rust
ctx::plan::set_pay_factor(500);
```

`set_pay_factor` assigns to the turn-long `turn.plan.pay_factor`. Two live
halves stay 0.5 instead of 0.25, and the factor is still there for the owner's
own later landing.

Root cause **R2** (absolute, turn-scoped factors, no multiply, no read).

### 2.4 `kokoro-pay-buy` (medium; row 305)

Fragment: 「支付10000资金（视为买地花费）」.
Wrong site: `rules/cards/card-hhw/src/kokoro_circle.rs:44-45`

**Demand:** the 10000 is a mandatory Buy-shaped payment with the shortfall path.

```rust
let paid = ctx::pay(player_id, 10000, &Msg::new(key!("kokoro_circle_why")))?;
if paid < 10000 { return Ok(()); }
```

`ctx::pay` hard-codes `PayKind::Card`. `ctx::pay_buy` carries `PayKind::Buy`
but never sets `must`, so a short payer silently underpays and the early return
spends the card without the 10000.

Root causes **R1** (kind and must are not declarable together) and **R5**.

### 2.5 `kokoro-rent-kind` (high; rows 306, 953)

Fragment: 「向所有者支付6000资金，视为格子的收款」.
Wrong site: `rules/cards/card-hhw/src/kokoro_circle.rs:109-114`

**Demand:** the 6000 is a rent-kind collection of the caller-supplied figure.

```rust
ctx::transfer(mover, player_id, 6000, ...)?;
```

`PayKind::Card` makes `pay_is_rent` false; `rent_factor` and
`tile_money_scale` never apply, the HHW band skill and 梦幻的回礼 do not see a
collection, and `paid_in_settle` stays cold. `charge_rent` cannot be reused
because CiRCLE is unowned and its own rent is 0.

Root cause **R1**.

### 2.6 `kokoro-pay-tile` (high; rows 309, 1005)

Fragment: 「视为格子的收款」.
Wrong site: same call, `rules/cards/card-hhw/src/kokoro_circle.rs:109`

**Demand:** the collection names CiRCLE so tile-keyed riders can see it.

`HostRequest::Pay` never writes `p.tile`; `trigger::tile()` is -1. 明天之门's
`tile.id >= 0 && tile.owner == owner` and `kaede_support`'s `tile.id >= 0`
dead-end.

Root cause **R1** (no tile in the card-facing payload).

### 2.7 `tomori-charge-paying-temp-share` (low; row 459) — demote to A

Fragment: 「以代替此次技能的火罐消耗」.
Site: `rules/cards/card-mygo/src/tomori_no_longer.rs:61`.

**Demand:** a fully-temp cost still offers the substitution, once, for the whole cost.

The window is opened by `rules/bandori-std/src/fire.rs:323` over the **whole**
cost, before any temp or pool pot moves; a fully-temp cost still reaches it.
The item text predates this uncommitted fix. No ABI is needed.

Historical root cause: the window lived in `spend_pool`, below the temp drain, so it could not see the whole cost; the fix moved it to `spend`. No R1-R7 cause remains. The one remaining obligation is the black-box test numbered 7 in §9, so the fix cannot regress.

### 2.8 `popipapapipopa-spend-window` (medium; row 572)

Fragment: 「（2）消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）。」.
Wrong site: `rules/cards/card-ppp/src/popipapapipopa.rs:40`

**Demand:** every spend money movement offers the crystal choice, redeem and auction included.

```rust
On::Action(Action::PaySplitChooseBefore("... value > 0 ...", None, pay_choose)),
```

Only the pay arm is hooked. `Cx::apply_redeem` (`play.rs:3220-3242`) and the
auction win (`play.rs:4473`) write `money -= n` directly.

Root cause **R4** (no window for raw 消耗).

### 2.9 `aya-stage-2-interactive-hook-missing` (medium; rows 1397, 1539) — demote to A

Fragment: 「[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）。」.
Wrong site: `rules/cards/card-pp/src/aya_longing.rs:60`.

**Demand:** the -1500 lands once per command at stage 2, not per leg at stage 3.

The −1500 rides `PaySplitMulBefore` (stage 3, per leg). The claimed missing
"interactive pre-stage before the `PaySplitAdd` fold" already exists:
`PayCombinedAddBefore` (kind id 106), raised once per command at
`play.rs:3912-3925` before the stage-2 fold. Move the action there and delete
the stage-3 action. The [消耗] arm gains a `SpendBefore` registration from
item 8's surface. No new ABI.

Root cause (historical): a placement error, not an ABI gap — the interactive command-wide hook was simply not used.

### 2.10 `door-pay-tile-vs-settle-tile` (medium; row 1427)

Fragment: 「…[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付…」.
Wrong site: `rules/cards/card-ppp/src/tomorrows_door.rs:34`

**Demand:** the gate is the settle tile, not the payment tile.

```rust
Filter::PaySplitAdd("card.placed && pay_is_rent && actor != owner && tile.id >= 0 && tile.owner == owner", ...)
```

`tile` is the payment's tile. 练习室里的风暴's remote 「收费」 is stamped with
the owner's tile, so the surcharge folds into a settle on an unowned tile; and
because the filter requires an owner, a settle on the unowned 梦开始的地方
(which the clause also names) is missed unless `settle_body` catches it.

Root cause **R7** (settle identity absent).

### 2.11 `anon-tokyo-link-rent-figure` (medium; rows 513, 1388)

Fragment: 「被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的一半」.
Wrong site: `rules/cards/card-mygo/src/anon_tokyo.rs:222`

**Demand:** the rider adds half the partner's real charge; a mortgaged partner charges nothing.

```rust
let bonus = ctx::rent_of(other) / 2;
```

`rent_of` is the table number: a mortgaged partner still contributes, a
dice-rent partner contributes the table, and a `RENT_FACTOR` partner ignores
the factor. **Row 513 is demoted to A**: it is the same line's comment/ruling
note, already covered by the TODO, with no separate mechanism.

Root cause **R6** (no quote that equals the charge).

### 2.12 `miracle-free-build-choice` (medium; row 521)

Fragment: 「当你进行加盖动作时可移除此卡上的一个奇迹水晶以代替资金花费。」.
Wrong site: `rules/cards/card-mygo/src/miracle.rs:86`

**Demand:** substitute only when a money cost actually exists.

```rust
ctx::set_build_discount(ctx::build_cost(t), 1);
```

`build_cost` is the base. `build_cost_pct` and `build_discount` are write-only,
so the body cannot tell whether there is any 资金花费 left. It always zeroes
and always spends.

Root cause **R6** (effective figure not readable).

### 2.13 `guerrilla-paid-per-seat` (medium; row 1472)

Fragment: 「如果本回合的[结算]向其他玩家支付了至少1000资金」.
Wrong site: `rules/cards/card-ras/src/guerrilla.rs:201`

**Demand:** count only the card owner's own settle payments.

```rust
if ctx::paid_in_settle() < 1000 { ... }
```

`turn.paid_in_settle` is one world counter (`play.rs:3856`); another seat's
payment revives the card.

Root cause **R3** (world-wide money facts).

### 2.14 `repaint-pay-factor` (medium; row 835)

Fragment: 「对方此次结算的支付减半」.
Wrong site: `rules/cards/card-ras/src/repaint.rs:98`.

**Demand:** two halvings on the move compose and stop at move end.

Same absolute write as 2.3, but the scope is the opponent's **current move**.
Two halvings on one move lose one.

Root cause **R2**.

### 2.15 `taki-pay-factor-leak` (high; rows 560, 1361)

Fragment: 「本次结算导致的所有[支付]变为原价的四分之一」.
Wrong site: `rules/cards/card-mygo/src/taki_serious.rs:77`.

**Demand:** the quarter applies to that settle only.

Absolute turn-plan write; every later payment in the turn is a quarter, and an
existing factor is overwritten.

Root cause **R2**, scope one settle.

### 2.16 `rana-pay-factor-multiply` (medium; row 1483)

Fragment: 「由此卡效果导致[触发结算]时需支付资金减半」.
Wrong site: `rules/cards/card-mygo/src/rana_funny.rs:120`.

**Demand:** the half multiplies the live factor instead of replacing it.

Assignment replaces a walk factor; 0.5 × 0.5 must be 0.25.

Root cause **R2**.

### 2.17 `fire-bird-paid-signal` (medium; row 1441)

Fragment: 「支付1600资金将此卡放置在自己场地上」.
Wrong site: `rules/cards/card-roselia/src/fire_bird.rs:49`

**Demand:** placement depends on the payment settling, not on the amount equalling the nominal figure.

```rust
let paid = ctx::pay(player_id, 1600, ...)?;
if paid < 1600 { return Ok(()); }
```

`ctx::pay` returns the amount actually moved. A 0.5× modifier settles 800 and
reads as "could not pay 1600", so the card is discarded.

Root cause **R5** (amount and success conflated).

### 2.18 `council-check-cancelled-settle` (medium; row 1449)

Fragment: 「…结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金」.
Wrong site: `rules/cards/card-roselia/src/council_check.rs:34` (the only cleanup is `SettleAfter`), and `:101` (same discounted-payment shortfall read).

**Demand:** a cancelled or aborted settle refunds the placement cost, unplaces, and arms nothing.

`SettleStage::RaiseSettle`, on cancel or payer-out, jumps to
`RaiseTileResolved`; `settleAfter` never raises, so `SLOT_STOP` stays armed
and `SLOT_PAID` is never refunded or unplaced.

Root causes **R3** (no settle outcome) and **R5**.

### 2.19 Shared root causes

- **R1 — The command record is incomplete and caller-fixed.** `Pay` has
  `kind`, `tile`, `must` and `allow_mortgage`, but `HostRequest::Pay` hard-codes
  `PayKind::Card`, `must = true`, `tile = None`; there is no shape. A card can
  only pick between two call verbs, and neither carries the full tuple.
- **R2 — Factor writes are absolute and turn-scoped.** `set_pay_factor` assigns
  on `TurnCtx.plan`; the plan is never reset, no getter exists, no multiply
  exists, and there is no move/settle scope.
- **R3 — Money facts are global or erased.** `paid_in_settle` has no payer;
  cancel/out paths carry no outcome; `settleAfter` is skipped.
- **R4 — Some 消耗 bypasses the pipeline.** Redeem and auction debit directly.
- **R5 — The return value conflates amount with success.** `ctx::pay` returns
  money moved; a discounted settled payment is indistinguishable from a
  shortfall.
- **R6 — Derived figures are write-only or quote≠charge.** Effective build
  cost is not readable; `rent_of` is the table, not the charge.
- **R7 — Settle identity is not on any payload.** The payment tile is the only
  tile; the settle's own tile is unavailable and `Unplace`/outcome is inferred.

## 3. Capability as an invariant

The capability is one sentence: **a payment command is a complete record, its
figure is produced by exactly one declared shape, and every fact a card can
observe is explicit on the command rather than inferred from a global.**

Testable invariants:

- **I1 (context).** Every `Pay` has `kind`, `shape`, `tile` (-1 = none),
  `settle_tile` (-1 outside a settle), `by_card`, `must`, `allow_mortgage` and
  a reason before any window opens. Nothing is filled in by a later stage.
- **I2 (fixed).** A `Fixed` leg moves exactly the declared amount. The
  declaration and `pay` windows still open for observers, but their amount,
  target and cancel writes are ignored. Affordability still applies: a
  mandatory fixed leg raises funds and can bankrupt.
- **I3 (payee half).** `PayeeHalf` captures the declared payee, settles the
  main leg as a delete to the bank, and then charges the captured payee
  `floor(settled / 2)` as a `Fixed` leg to the payer. No half on cancel or
  shortfall.
- **I4 (factor scope).** `pay_factor()` returns the product of every factor
  scope live at this instant (4.5). A factor write outlives exactly its scope:
  next move, this move, next settle or this settle; the engine clears each at
  the scope end.
- **I5 (spend).** Every 消耗 money movement raises `spendBefore` with the full
  cost before any money moves. The hooked amount is floored at 0; a cancel
  means "cost reduced to 0", not "spend failed". One `spendBefore` per spend.
- **I6 (settle tile).** On every pay-window trigger, `settle_tile` is the
  innermost open settle's tile, or -1. It is never the payment's tile.
- **I7 (outcome).** `tileResolved` raises on every settle that enters the
  stage machine, including a cancelled settle and a payer-out, and always
  carries `Settled`, `Cancelled`, `PayerOut` or `Aborted`. The settle-depth
  cap fires before the machine starts and is the one path with no
  `tileResolved` and no outcome.
- **I8 (quotes).** `rent_quote(p, t)` equals the figure
  `charge_rent(p, t, 1000)` would move at this instant (owner/mortgage checks,
  table or dice base × the tile's own factors × the live rent/pay factors, one
  ceil10), with no money movement; -1 when a rent charge would not happen
  (unowned, mortgaged, owner is `p`, an endpoint out). `build_cost_effective(t)`
  equals the money the next build on `t` would charge here, without popping a
  discount layer.
- **I9 (per payer).** `paid_in_settle_for(seat)` counts only payments whose
  payer is `seat`, while a settle is open and the declared counterparty is
  another player.
- **I10 (report).** `PayReport::settled` is true iff the payment committed;
  `amount` is the money actually moved and is independent of the flag.

## 4. Surface

Everything below is guest-facing ABI unless marked internal. "New" = added;
"extended" = existing shape grows; "wire-breaking" = existing wire meaning
changes. No import decides anything: `pay_spec` pauses and the engine opens the
windows, a spend hook may ask and suspends the spend call, and the quoted
readers move no money (`rent_quote` rolls a fresh d20 for a dice-rent tile and
therefore advances the RNG, 11.8). All ten imports exist identically on
wasm32, native and Kani
(4.8, 5.7). Nothing carries display text: the only string argument is the
existing postcard `Msg` (`ptr`/`len`) and every log line keeps its current
`Msg` shape and keys.

### 4.1 New ABI constants and kinds (`crates/card-sdk/src/abi.rs`)

```rust
// wire-breaking: the exact host/module version check rejects stale wasm.
pub const ABI_VERSION: i32 = 107;              // was 106 (third group)

// New action kind (this group's README band id; 104 retired).
impl TriggerKind { pub const SpendBefore: Self = Self::engine(122); }
impl ActionKind { pub const SpendBefore: Self = Self::engine(122); }

pub mod pay_kind {
    pub const CARD: i32 = 0; pub const RENT: i32 = 1; pub const BUY: i32 = 2;
    pub const BUILD: i32 = 3; pub const LOSS: i32 = 4; pub const FORCE_BUY: i32 = 5;
    pub const GAIN: i32 = 6;
}
pub mod pay_shape { pub const NORMAL: i32 = 0; pub const FIXED: i32 = 1; pub const PAYEE_HALF: i32 = 2; }
pub mod pay_flag {
    pub const MUST: i32 = 1 << 0;
    pub const ALLOW_MORTGAGE: i32 = 1 << 1;
    pub const HAS_TILE: i32 = 1 << 2;
    pub const TOTAL_STAGE: i32 = 1 << 3;
}
pub mod pay_report {
    pub const AMOUNT_MASK: i32 = 0x0FFF_FFFF;
    pub const SETTLED: i32 = 1 << 31;
    pub const CANCELLED: i32 = 1 << 30;
    pub const SHORTFALL: i32 = 1 << 29;
    pub const FIXED: i32 = 1 << 28;
}

#[repr(i32)] pub enum PayKind { Card = 0, Rent = 1, Buy = 2, Build = 3, Loss = 4, ForceBuy = 5, Gain = 6 }
#[repr(i32)] pub enum PayShape { Normal = 0, Fixed = 1, PayeeHalf = 2 }
#[repr(i32)] pub enum PayFactorScope { NextMove = 0, ThisMove = 1, NextSettle = 2, ThisSettle = 3 }
#[repr(i32)] pub enum SettleOutcome { Settled = 0, Cancelled = 1, PayerOut = 2, Aborted = 3 }
```

`spec` bit layout for 4.2: bits 0-3 shape, 4-7 kind, 8-11 flags, 12-31
`tile + 1` (`0` = no tile). Unknown shape/kind falls back to `Normal`/`Card`;
an unknown flag bit is ignored. The host validates on decode.

### 4.2 New host import

```rust
/// Host slot: ruleset_pay_spec
pub fn pay_spec(from: i32, to: i32, amount: i32, spec: i32, ptr: i32, len: i32) -> i32;
```

Semantics: exactly `pay_to` plus context. `from < 0` = print, `to < 0` = delete,
both `>= 0` = transfer. `ptr`/`len` are the postcard `Msg`. The answer is a
packed `PayReport`: low 28 bits moved amount, high flag bits. `EXIT_NEED_INPUT`
(`0x0B_A0_D0`) is still the pause sentinel; a real report always sets
`SETTLED`, `CANCELLED` or `SHORTFALL`, all of which are outside the sentinel's
bit range.

Answer codes: `SETTLED` on any commit, including a `money_locked` zero;
`CANCELLED` when no money moved for a no-settle reason (immunity, pay gate,
zero/out-of-range amount, a cancel, payer off-board); `SHORTFALL` when a
mandatory payment could not be completed (the payer went out through
`raise_funds`). `amount` is the amount **actually moved**, `paid.moved()`
(max of loss and gain) -- the same number the old `pay`/`transfer`/`gain`
verbs returned -- and is 0 for `CANCELLED`, `SHORTFALL` and a `money_locked`
commit. The settled figure is not separately exposed; a
`money_locked` commit is distinguishable only by `SETTLED`. The `FIXED` flag
echoes the shape, so a caller can tell an immutable zero from a shaped one.

### 4.3 New guest builder (`crates/card-sdk/src/ctx.rs`) — sugar over 4.2

```rust
pub struct TransferCtx { /* from, to, amount, spec, src: Msg */ }
impl TransferCtx {
    pub fn new(from: i32, to: i32, amount: i32, src: &Msg) -> Self;
    pub fn kind(self, kind: PayKind) -> Self;
    pub fn shape(self, shape: PayShape) -> Self;
    pub fn fixed(self) -> Self;             // shape(Fixed) + kind(Card) default kept
    pub fn payee_half(self) -> Self;        // shape(PayeeHalf)
    pub fn tile(self, tile: i32) -> Self;   // tile < 0 = none
    pub fn optional(self) -> Self;          // must = false, allow_mortgage = true
    pub fn total_stage(self, on: bool) -> Self;
    pub fn send(self) -> Result<PayReport, Prompt>;   // load-bearing
}
```

`ctx::pay`, `ctx::transfer` and `ctx::pay_leg` keep their signatures and become
`TransferCtx::new(...).send()?.amount()` (with `pay` `to = -1`, `pay_leg`
`total_stage(false)`). The builder is not an ABI type; it fills the packed
`spec` before calling `pay_spec`.

```rust
#[repr(transparent)] pub struct PayReport(pub i32);
impl PayReport {
    pub fn amount(self) -> i32; pub fn settled(self) -> bool;
    pub fn cancelled(self) -> bool; pub fn shortfall(self) -> bool;
    pub fn fixed(self) -> bool;
}
```

### 4.4 Pay-window mutators (`ctx::trigger`)

```rust
pub fn set_pay_kind(kind: PayKind);
pub fn set_pay_shape(shape: PayShape);
```

Valid in any pay window (`effect`, combined/split stages, `pay`, `paid`).
Writes are recorded on the pending trigger and folded with the same precedence
as `set_pay_amount`: **last writer wins**. Setting any non-`Normal` shape
after the effect window has already opened still works only because the effect
window runs before every stage.

### 4.5 Factor API (`ctx::plan`)

```rust
pub fn mul_pay_factor(milli: i32, scope: PayFactorScope);   // load-bearing
pub fn pay_factor() -> i32;                                 // milli, scoped product
pub fn set_pay_factor(milli: i32);                          // absolute, Auto scope
// `set_pay_factor` keeps its name and arity; its meaning widens from "the
// turn plan" to Auto's scoped target. In a settle it now writes the settle
// scope, where before it always wrote the plan -- a caller inside a settle
// that means the plan says so with `mul_pay_factor`/scope=ThisMove.
// Regression test: `g02_settle_set_factor_does_not_leak_to_the_move`.
```

`Auto` = `ThisSettle` if a settle is open, else `ThisMove` if a move is active,
else `NextMove`. `mul` composes into the named scope; a scope that is not
currently open falls forward to its next instance (`ThisMove` without a move →
`NextMove`; `ThisSettle` without a settle → `NextSettle`). `milli <= 0` writes
0; a milli buffer saturates at `i32` rather than wrapping.

**Who owns which symbol.** G04 owns the plan module and its plan-local read
`ctx::plan::move_pay_factor()`. This group owns the settle half and the
composition: `ctx::plan::pay_factor()` answers the **whole** scoped product and
already includes the plan half -- callers never add the two, and the engine
reads the same composed value at `paySplitMul`. `pay_factor()` is
the **scoped** product only (plan × every open settle),
truncated to milli and saturated at `i32` (11.15); the per-payment rent factor,
tile props and `milli_scale`
need the pending payment and are not in the getter. The engine reads the
scoped product once at `paySplitMul`, applies the per-payment factors, and
does the single end-of-calculation round after all of them.

### 4.6 Settle and quote readers

```rust
pub fn settle_tile() -> i32;                 // -1 outside a settle
pub fn settle_outcome() -> i32;              // SettleOutcome; -1 outside tileResolved
pub fn paid_in_settle_for(seat: i32) -> i32;
pub fn rent_quote(payer: i32, tile: i32) -> i32; // -1 = no rent charge
pub fn build_cost_effective(tile: i32) -> i32; // >= 0
```

`settle_tile` and `settle_outcome` are also pre-language scalars (5.3).

### 4.7 Extended pre language (`crates/game-rules/src/cond_pre.rs`)

New pure scalars, usable in any guard because they read the trigger:

- `pay_is_fixed()` — the pending payment's shape is `Fixed`.
- `pay_kind()` — the wire kind, or -1 when not a pay window.
- `settle_tile_id()`, `settle_tile_owner()`, `settle_tile_houses()`,
  `settle_tile_mortgaged()` — the innermost settle, or -1/0 when outside.
- `settle_outcome()` — the terminal outcome, or -1.

`tile.id`/`tile.owner` keep meaning the **payment's** tile. `pay_is_rent`
remains the declared `p.rent` flag (not `kind == Rent`), so a rent rewritten to
`Loss` still reads as rent (5.2).

### 4.8 Host plumbing (internal; all three targets)

| Layer | Addition |
|---|---|
| `game-rules/src/host.rs` `HostRequest` | `PaySpec { from, to, amount, spec, src }` (`TOTAL_STAGE` decodes from the spec) |
| `game-rules/src/hostfns.rs` | `pay_spec`, `trig_set_pay_kind`, `trig_set_pay_shape`, `mul_pay_factor`, `pay_factor`, `settle_tile`, `settle_outcome`, `paid_in_settle_for`, `rent_quote`, `build_cost_effective` |
| `game-rules/src/host.rs` `func_wrap` | one registration per 4.2/4.4-4.6 name |
| `game-rules/src/native_shims.rs` | `op_pay_spec`, `op_trig_set_pay_kind`, … one per import; `ruleset_pay_spec` panics on a bad decode (host bug) |
| `card-sdk/src/ctx/sys.rs` | externs with `#[link_name = "ruleset_*"]` |
| `card-sdk/src/ctx/sys_kani.rs` | same wrappers over `kani_host::call_i32` |
| `card-sdk/src/kani_host.rs` | `Op::pay_spec = 341`, `Op::trig_set_pay_kind = 342`, `Op::trig_set_pay_shape = 343`, `Op::mul_pay_factor = 344`, `Op::pay_factor = 345`, `Op::settle_tile = 346`, `Op::settle_outcome = 347`, `Op::paid_in_settle_for = 348`, `Op::rent_quote = 349`, `Op::build_cost_effective = 350` |
| `rules/kani/build.rs` | the same ten names appended in discriminant order; `op_*` HostOps arms in `conf_sym_ops.rs` |
| `rules/kani/tests/conf_model.rs` | inline-host arms for all ten |
| `card-sdk/src/lib.rs` `TriggerKind::from_str`/`spelling` | `spendBefore = 122` (asserted id); `game-rules/src/trigger_why.rs` puts it in the pay family |

`ApplyRedeem` and the auction debit raise `spendBefore`; `apply_redeem` and
`redeem` become `Flow` so a prompt can suspend them (5.4).

## 5. Engine wiring

### 5.1 Stage order, unchanged

`money_inner` continues to run:

1. endpoint/dead/amount/locked/gate checks;
2. `effect` declaration (read-only when `Fixed`);
3. command-wide `payCombinedAddBefore` → `payCombinedAdd` → `payCombinedMul`
   → `payCombinedCancel`, when `total_stage`;
4. per-leg `paySplitAdd` → `paySplitMulBefore` → `paySplitMul` →
   `paySplitChooseBefore` → `paySplitChoose` → `paySplitAt`;
5. `pay` window;
6. affordability (`must` raise-funds/bankruptcy, optional mortgage);
7. debit/credit, `paid_in_settle_by`, log;
8. `payAfter`, `paid`;
9. payee-half leg, when shaped.

No stage moves. Shape only decides which of 2-5 run and whether their rewrites
fold. `SpendBefore` is a new **fifth** pipeline-shaped point that is not a
payment and runs inside `apply_redeem`/auction before their direct debit.

### 5.2 Shape branches

The engine gains `PayKind::Loss` (spelling `"loss"`, `PayEvent::Lose` when
derived) and `PayShape { Normal, Fixed, PayeeHalf }`; the host decoder maps the
guest wire codes to them (an unknown code falls back as in 4.1). `Pay` gains
`shape: PayShape` and runtime-only `partner: Option<usize>` (captured from the
declared `to` before any response).

- `Normal`: today's code exactly.
- `Fixed`: raise `effect` (step 2) and `pay` (step 5) but ignore
  `t.value`, `t.target` and `is_cancelled()`. Skip 3-4 entirely. Do not apply
  `milli_scale`, plan/settle factors or `tile_money_scale`. Keep the
  affordability path, the log, `payAfter`/`paid` and the settle counter.
- `PayeeHalf`: run the full `Normal` pipeline, but at entry set
  `partner = to`; after the `pay` window (before affordability and commit)
  force `to = None`. After step 8,
  if `partner` is alive and `loss > 0`, run
  `self.money(Pay { from: partner, to: Some(payer), amount: loss / 2, kind: Card, shape: Fixed, must: true, by_card, reason, .. })`.
  The half leg is a fresh pipeline at `money_depth + 1`, so it logs, fires
  `paid` and counts in `paid_in_settle_by`. No half when the main leg was
  cancelled, short or the payer is out.

`set_pay_kind` and `set_pay_shape` write `trigger.pay`; the fold happens where
`set_pay_amount` folds and copies `t.pay.kind` / `t.pay.shape` / `t.pay.is_rent`
into `p.kind` / `p.shape` / `p.rent`. A `Fixed` leg that a responder turns
`Normal` re-enters the skipped stages because the fold at the effect boundary
sees the final shape. `Pay` keeps a separate declared `rent: bool` (`Pay::new`
initializes it `kind == Rent`), because Mashiro keeps 「同等的资金」:
`set_pay_kind` sets `t.pay.is_rent` true for `Rent`, false for the other kinds
**except `Loss`, which preserves the current value**, and the fold carries it
into `p.rent`. `TriggerPay.is_rent`, the rent factor and the rent half of
`tile_money_scale` read `p.rent`; `log_money` reads the final `p.kind`, so the
redirected leg logs as a loss while still scaling as the rent it was. The
`paid_in_settle_by` branch at step 7 also counts a `PayeeHalf` main leg whose
committed `to` is `None`, by its `loss` (11.3). Concretely, the `let rent = p.kind ==
PayKind::Rent;` at `play.rs:3418` becomes `let rent = p.rent;`, and
`log_money` keeps testing the final `p.kind` (not `p.rent`).

`Paid` gains `pub shortfall: bool`. Only the mandatory `raise_funds` path sets
it, at the `self.out(f)` early return; every other `Paid::default()` return is
a cancel. The handler packs `SETTLED` from `paid.paid`, `SHORTFALL` from
`shortfall`, `CANCELLED` otherwise, and `amount` from `paid.moved()`.

### 5.3 Settle tile and outcome

`Cx` replaces the bare `settle_open: u32` with
`settle_scopes: Vec<SettleScope>` where `SettleScope { player, tile,
pay_factor: f64 }`. A push initializes `pay_factor` from
`turn.next_settle_pay_factor` (milli, then reset to 1000), so a `NextSettle`
write lands on exactly one settle. `step_settle` pushes before
`step_settle_run` and pops on completion (not on suspension). `settle_open()`
returns
`settle_scopes.len() as u32`, so the existing `> 0` call sites are unchanged.

- `TriggerPay` loses its bare `new(is_rent)` and becomes
  `TriggerPay::new(rent, kind, shape, settle_tile)`; `is_rent` is stored from
  the declared `p.rent` (5.2). Every pay raise passes
  `settle_tile: innermost().map_or(-1, |s| s.tile)` and the current folded
  kind/shape.
- `SettleFrame` gains `outcome: SettleOutcome`, updated at `RaiseSettle`
  (out → `PayerOut`, cancel → `Cancelled`), `RaiseSettleBody` (body cancel →
  `Cancelled`), `RaiseSettleAfter` (out → `PayerOut`, `!playing()` →
  `Aborted`), and `RaiseTileResolved` (`Settled` only when no earlier terminal
  outcome was recorded). `Trigger` gains
  `outcome: Option<SettleOutcome>`; `RaiseTileResolved` sets it from the frame
  and every other raise leaves it `None`. `tileResolved` carries it on the
  trigger; `settle_outcome()` and the pre scalar read `trigger.outcome`.
- `settle_tile()`/`settle_outcome()` and the pre scalars read the innermost
  scope / trigger. Nothing reads `player_pos`.

### 5.4 SpendBefore

`apply_redeem(i, t, amount, by_card) -> Flow<i32>` and the auction debit both:

```
let t = raise!(self, "spendBefore", i, tile = tile, value = cost, by_card = by_card)?;
let cost = t.value.max(0);
```

Both raises run at the top of their commit path, before the `raise_funds`
affordability test, so a reduction can prevent a shortfall (11.10). Then the
existing direct debit uses `cost`. The auction keeps the original `bid` for
`default_deal`/`BuyAssign`; only its affordability and debit use `cost`
(11.9). `redeem`/`standalone_redeem` become
`Flow<bool>`; the `ApplyRedeem` handler awaits them. The `ApplyRedeem` host
function goes through `request_or_pause(..., Pause::Sentinel)` exactly as
`pay` does, and the guest wrapper `ctx::apply_redeem` switches to `asked(...)`
and `Result<i32, Prompt>`; its one call site (`toko_swap.rs:134`) adds `?`.
`apply_mortgage` is untouched. The debug `redeem` arm (`debug.rs:94`) awaits it
too, so `debug_open` and the log are restored after the suspension. `spendBefore`
never opens the pay pipeline,
never applies factors, and never moves money itself. A cancel is not signalled
as failure; `t.value == 0` is a free spend.

### 5.5 Factor scopes and storage

| Scope | Storage | Lifetime |
|---|---|---|
| `NextMove` | `TurnCtx.next_move_pay_factor: i32` (milli, default 1000) | folded into `turn.plan.pay_factor` at move start, then reset to 1000 |
| `ThisMove` | `turn.plan.pay_factor: f64` (the live plan; `MoveCtx`'s own copy is the carried value, not the write target) | cleared to 1.0 at move end (`moveAfter` tail) |
| `NextSettle` | `TurnCtx.next_settle_pay_factor: i32` (milli, default 1000) | moved into the next `SettleScope`, then reset to 1000; also reset at turn end |
| `ThisSettle` | `SettleScope.pay_factor: f64` | dropped with the scope |

`scale_settle_payment` multiplies, in code order: rent factor (rent only) ×
`turn.plan.pay_factor` (1.0 outside a move because the move tail clears it) ×
every open settle factor × `milli_scale` × `tile_money_scale` (the rent half
only when `rent`). `scale_settle_payment` itself **truncates** each multiply
(`(amount as f64 * f) as i32`); the one `ceil10` is not in it but in the pay
chain (`play.rs:3690`) and only fires for a positive figure a modifier touched
(`modified && amount > 0`). An equality test on `rent_quote` must reproduce
both halves, truncation included. The getter returns the **scoped** product (plan × open
settles) in milli, not the per-payment rent/tile/milli scales (4.5). Writing
`ThisMove` therefore writes the plan, which is what the scale and the move
followers already read; `adopt_plan` still copies it into a follower's `m`
only as the carried value.

### 5.6 Who may write what

| Field | Writer | When |
|---|---|---|
| `shape`, `kind` at declaration | the card body | before `pay_spec` |
| `shape`, `kind` in a window | any responder via `set_pay_*` | any pay window, last wins |
| `amount` | existing `set_pay_amount`; `spendBefore` for spends | as today; ignored by `Fixed` |
| `tile` | declaration only (`pay_spec`) | immutable afterwards |
| `must`, `allow_mortgage` | declaration only | immutable |
| `settle_tile`, `settle_outcome` | engine only | raise time |
| factor of a scope | any card body/hook | its scope only |
| `paid_in_settle_by` | engine only | settle commit |

### 5.7 Kani model

Add ten `Op` arms with `[i32; 6]` args, all deterministic:

- `pay_spec` decodes the spec, pushes `Effect::PayTo` and a `PayReport`,
  mutating `w.money` as `arm_pay_to` does; `Fixed` moves `amount` verbatim.
- `trig_set_pay_kind` / `trig_set_pay_shape` rewrite the installed trigger.
- `mul_pay_factor` multiplies the scope model and asserts no overflow;
  `pay_factor` returns the product.
- `settle_tile` / `settle_outcome` read the symbolic settle model;
  `paid_in_settle_for` a per-seat array; `rent_quote` a symbolic figure per
  (payer, tile); `build_cost_effective` the symbolic cost minus the write-only
  knobs.
`SpendBefore` has no import of its own, so it adds no eleventh `Op`:
`arm_apply_redeem` calls an internal `fold_spend_before` helper (the same
`set_pay_amount` fold the pay stages use) before its debit, and the auction
helper does the same. The model always answers, so the Kani `asked()` never
sees a sentinel for these two.

Every proof's `host_subset!` lists only what its card calls; the smoke proof
covers all ten. Invariants live in `rules/kani/src/spec/{mor,crychic,hhw,ppp,…}.rs`
and are obligations, not just stubs (9).

## 6. Card usage, before and after

### 6.1 `rui_devil.rs:34`

Before:
```rust
ctx::transfer(player_id, o, 100, &why)?;
```
After:
```rust
ctx::TransferCtx::new(player_id, o, 100, &why).fixed().send()?;
```
The 300-back leg at 42 keeps `ctx::transfer`. `Fixed` raises `paid`, so the
existing "skill fired" bookkeeping is unchanged; `set_slot` writes stay.

### 6.2 `mashiro_pay.rs:47,57-64`

Before: `set_pay_target(-1)` then a nested `ctx::pay(to, amount/2)`, half from
the pre-scale value.
After:
```rust
trigger::set_pay_kind(PayKind::Loss);
trigger::set_pay_shape(PayShape::PayeeHalf);
```
and delete the nested `ctx::pay` (the engine now charges the half after the
settled figure). `set_pay_target(-1)` stays as the declared intent, though the
shape forces the bank on its own (11.17). The log at 49-55 keeps
`trigger::value()` for its display amount; the engine's half is authoritative
for money.

### 6.3 `sakiko_lead.rs:77`

Before: `ctx::plan::set_pay_factor(500);`
After: `ctx::plan::mul_pay_factor(500, PayFactorScope::NextMove);`

### 6.4 `kokoro_circle.rs:44-45`

Before: `ctx::pay(..., 10000)` and `if paid < 10000 { return }`.
After:
```rust
let report = ctx::TransferCtx::new(player_id, -1, 10000, &why)
    .kind(PayKind::Buy).send()?;
if !report.settled() { return Ok(()); }
```
Mandatory is the builder default; `.optional()` would drop it. The early
return stays only for the open "card not effective" ruling (11.4).

### 6.5 `kokoro_circle.rs:109-114`

Before: `ctx::transfer(mover, player_id, 6000, ...)`.
After:
```rust
let report = ctx::TransferCtx::new(mover, player_id, 6000, &why)
    .kind(PayKind::Rent).tile(target).send()?;
```
The `paid` value for the log comes from `report.amount()`.

### 6.6 `kokoro_circle.rs:109` (tile)

Same call's `.tile(target)` supplies both the `pay_is_rent`-adjacent riders and
`trigger::tile()`; no separate code.

### 6.7 `tomori_no_longer.rs:61` (demoted)

No card change. `fire.rs:323-325`:
```rust
if !ctx::raise(player_id, crate::kind::ChargePaying, n)? { return Ok(true); }
```
already runs before `from_temp`/`from_pool`; a regression test locks it.

### 6.8 `popipapapipopa.rs:40`

Before: one `PaySplitChooseBefore` registration.
After: add a second registration to the same body:
```rust
On::Action(Action::SpendBefore(
    "card.placed && actor == owner && value > 0 && card.counter('crystals') > 0",
    None, pay_choose)),
```
`pay_choose` already reads `trigger::value()` and calls `set_pay_amount`, so
the body is shared unchanged; its max clamp still never asks beyond zero.

### 6.9 `aya_longing.rs:60` (demoted)

Before: `Action::PaySplitMulBefore(...)`.
After: `Action::PayCombinedAddBefore("actor == owner && value > 0", None, pay_res)`
and a `SpendBefore` registration with the same pre and `pay_res`. `pay_res`'s
`(value - 1500).max(0)` needs no change. The sibling `PaySplitAdd` filter at 27
stays (it is a different clause).

### 6.10 `tomorrows_door.rs:34`

Before: `pay_is_rent && tile.id >= 0 && tile.owner == owner`.
After:
`actor != owner && (settle_tile_owner == owner || settle_tile_id == tile_named('梦开始的地方'))`
(no `pay_is_rent`; the clause is about a settle, and 梦开始的地方 is the
unowned half of it). `settle_body` keeps its existing fallback.

### 6.11 `anon_tokyo.rs:222`

Before: `let bonus = ctx::rent_of(other) / 2;`
After: `let quoted = ctx::rent_quote(trigger::player_id(), other); if quoted < 0 { return v; } let bonus = quoted / 2;`

### 6.12 `miracle.rs:86-87`

Before: unconditional `set_build_discount(build_cost(t), 1)` and an
unconditional crystal spend in `after_build`.
After:
```rust
let cost = ctx::build_cost_effective(t);
if cost <= 0 { return Ok(()); }
ctx::set_build_discount(cost, 1);
```
`after_build` spends only when the arm fired. The guard already requires a
crystal; the new read adds the 资金花费 condition. Whether a prompt is needed
is 11.12.

### 6.13 `guerrilla.rs:201`

Before: `if ctx::paid_in_settle() < 1000`.
After: `if ctx::paid_in_settle_for(player_id) < 1000`.

### 6.14 `repaint.rs:98`

Before: `ctx::plan::set_pay_factor(500);`
After: `ctx::plan::mul_pay_factor(500, PayFactorScope::ThisMove);`

### 6.15 `taki_serious.rs:77`

Before: `ctx::plan::set_pay_factor(250);` in the loop.
After: `ctx::plan::mul_pay_factor(250, PayFactorScope::NextSettle);`
Each iteration arms the next `card_settle_at`, which consumes it.

### 6.16 `rana_funny.rs:120`

Before: `ctx::plan::set_pay_factor(500);`
After: `ctx::plan::mul_pay_factor(500, PayFactorScope::NextSettle);`

### 6.17 `fire_bird.rs:48-53`

Before: `let paid = ctx::pay(...)?; if paid < 1600 { return }`.
After:
```rust
let report = ctx::TransferCtx::new(player_id, -1, 1600, &why).send()?;
if !report.settled() { return Ok(()); }
```
A discounted 800 has `settled() == true`. The `must` default already gives the
shortfall path.

### 6.18 `council_check.rs:34,96-102,202`

Before: `SettleAfter` registration; `if paid < amount { return }`; cleanup
only in `settle_after`.
After:
- registration `On::Action(Action::TileResolved("card.placed", None, settle_ended))`;
- `settle_ended` (the old `settle_after` body) reads `ctx::settle_outcome()`;
  if it is not `Settled`, refund the placement cost and unplace without the
  `got < expect` gate; if `Settled`, keep the existing comparison;
- the placement pay uses `TransferCtx` and `if !report.settled()`; the refund
  slot stores `report.amount()` (the money actually spent), not the pre-scale
  `amount`.
`TileResolved` carries the same `tile` as `settleAfter`, so the
`trigger::tile() == tile` copy check stays.

## 7. Save, record, replay

- `TurnCtx.next_move_pay_factor: i32` — `#[serde(default = "milli_one")]`.
- `TurnCtx.paid_in_settle_by: Vec<i32>` — `#[serde(default)]`; reads use
  `.get(seat).copied().unwrap_or(0)` and the vector is resized at turn start.
- `TurnCtx.next_settle_pay_factor: i32` — `#[serde(default = "milli_one")]`.
- `fn milli_one() -> i32 { 1000 }` is a one-line serde helper added beside
  `full_build_cost`; it is not guest-visible. The manual `Default for TurnCtx`
  (which exists so `build_cost_pct` is not zeroed) sets both milli fields to
  1000 and leaves the vector empty.
- `SAVE_VERSION` stays **13** (G08 already moved it). Old saves load these three as 1000/empty; no
  other field changes meaning. A save→restore identity test must cover pending
  next-move and next-settle factors.
- `RECORD_VERSION` stays **3**. The record envelope does not change; new
  `HostRequest::PaySpec` answers ride the existing answer log exactly like
  `Pay`, and archived engine bundles replay older records with their own host.
- `pay_spec` is a `Pause::Sentinel` request: the packed answer is stored in the
  answer log, replay returns it without re-running the engine, and the card
  resumes once. `SpendBefore` prompts are ordinary hook prompts in the same
  log. Nothing in `Pay`, `TriggerPay`, `SettleScope`, `PayReport` or
  `PayShape` is serialized outside `World`/`TurnCtx`.
- `settle_scopes` is `Cx`-only runtime state; a mid-settle save re-derives it
  by replaying the answer log to the prompt, the same way the work stack is
  re-derived today.

## 8. Versioning

Exactly one wire break, so exactly one bump:

`crates/card-sdk/src/abi.rs:845` reads 106 when this group lands (G01 and
G08 took 105 and 106):

and becomes

```rust
pub const ABI_VERSION: i32 = 107;
```

with a `v107` doc block immediately below it recording: new import `pay_spec`; new imports
for kind/shape/factors/settle/quotes; new `ActionKind::SpendBefore = 122`; new
`pay_kind` / `pay_shape` / `pay_flag` / `pay_report` wire code tables; new
`PayKind::Loss`, `PayShape`, `PayFactorScope`, `SettleOutcome`; `SAVE_VERSION`
and `RECORD_VERSION` unchanged.

Why a break even though old imports keep their arity: the host checks the
module export for **exact** equality (`host.rs:3207-3213`), and a stale card
wasm has neither the new import names nor the new kind ids. Every card wasm
must be rebuilt in the same batch as the host. There is no compatibility shim
and no dual-version host: records replay through the archived bundle.

Source-only changes that do **not** bump: the `TransferCtx` builder, the
`ctx::plan::set_pay_factor` implementation gaining its Auto scope (the one-arg
signature stays; call sites migrate to `mul` in the same commit), `apply_redeem`
gaining `Flow`, and the pre scalars. Old card sources do not need the new verbs
until migrated.

## 9. Tests and Kani obligations

One black-box test per item, in a new `rules/tests/tests/g02_pay.rs` unless
noted; names are final. Discrepancy-only tests use `#[ignore]` with the row id
in the reason, per `docs/rulebook/TEST-FINDINGS.md` convention.

| # | Test | Asserts |
|---|------|---------|
| 1 | `g02_rui_fixed_leg_ignores_pay_modifiers` | crit on/off leaves each 100 leg exactly 100; the 300-back leg still doubles |
| 2 | `g02_mashiro_redirect_is_loss_and_half_settles` | payer loses settled amount to the bank; payee loses exactly half after a 0.5 factor (250/125) |
| 3 | `g02_sakiko_two_halves_compose_and_leave_with_move` | two live halves pay 0.25; the next move pays full |
| 4 | `g02_kokoro_10000_is_buy_and_must` | kind is Buy; a 5000-money payer mortgages/bankrupts rather than underpaying |
| 5 | `g02_kokoro_6000_is_rent_kind` | `pay_is_rent` hook sees it; a 0.5 rent factor halves it |
| 6 | `g02_kokoro_6000_carries_circle_tile` | `trigger::tile() == CiRCLE`; 明天之门 and `kaede_support` fire |
| 7 | `g02_tomori_full_temp_cost_substitutable` (`fire.rs` test) | a fully-temp cost opens `ChargePaying` once with the full `n`; a cancel spends no pot |
| 8 | `g02_popi_crystal_choice_on_redeem_and_auction` | redeem and auction each raise `spendBefore` with the full cost; 2 crystals reduce by 300 |
| 9 | `g02_aya_1500_once_per_command_at_stage_two` | a split command prompts once, −1500 before the add fold; a sibling +500 composes |
| 10 | `g02_door_surcharge_needs_settle_tile` | remote 收费 outside a settle collects nothing; a settle on the owner's tile does |
| 11 | `g02_anon_half_of_real_charge` | mortgaged partner → 0; dice/factor partner → half the actual charge |
| 12 | `g02_miracle_no_crystal_when_cost_already_zero` | pct 0/discount-covered build spends no crystal; a real cost spends one |
| 13 | `g02_guerrilla_counts_only_own_payments` | another seat's 1000 settle payment does not revive; the owner's does |
| 14 | `g02_repaint_two_halves_compose` | two Repaints on one move pay 0.25; the next move pays full |
| 15 | `g02_taki_quarter_stops_after_settle` | the forced settle pays 0.25; the owner's later landing pays full |
| 16 | `g02_rana_half_composes_with_walk_factor` | a walk already at 0.5 pays 0.25 |
| 17 | `g02_fire_bird_discounted_payment_places` | a 0.5 modifier settles 800 and the card is placed |
| 18 | `g02_council_cancelled_settle_refunds` | a cancelled settle and a payer-out both unplace, refund the cost, clear `SLOT_STOP`, and do not arm the next settle |

Kani obligations (each proof carries `host_subset!` with only its own ops, and
a named invariant in `rules/kani/src/spec/`):

- `mor::rui_devil_fixed_leg` — `pay_spec(Fixed)` report amount == 100 and
  `fixed()`; a symbolic factor never changes it.
- `mor::mashiro_halves_after_scale` — `PayeeHalf` report is `settled`, main
  loss equals settled, half leg from the captured partner.
- `crychic::sakiko_factor_product` — two `mul_pay_factor(500, NextMove)`
  compose to 250; the scope reset restores 1000.
- `hhw::kokoro_buy_kind` / `hhw::kokoro_rent_tile` — kind and tile reach the
  trigger; `must` is set.
- `ppp::popi_spend_before` — `SpendBefore` with n crystals reduces exactly
  150·n and spends n.
- `pp::aya_combined_before` — one raise per command; a fixed −1500 floors at 0.
- `ppp::door_settle_tile` — payment tile and settle tile can differ; the guard
  reads the settle tile.
- `mygo::anon_rent_quote` / `mygo::miracle_effective_cost` — quote/effective
  equality and the mortgaged/zero cases.
- `ras::guerrilla_per_payer` and `ras::repaint_two_halves`.
- `mygo::taki_factor_scope` and `mygo::rana_compose`.
- `r::fire_bird_discount_settles` and `r::council_outcome_refund`.
- `general::pay_report_flags` — settled/cancelled/shortfall are mutually
  exclusive; `amount` masks correctly.
- `general::settle_outcome_total` — tileResolved always raises exactly once and
  reports the frame's outcome.

Machine-checked exactness follows the formal-verification rule: the invariant
is an iff where the sheet is an iff; a mismatch fails, never `#[ignore]`.
`conf_model.rs` gains the inline implementations so the conformance harness can
drive all ten imports.

## 10. Alternatives rejected

1. **Grow `pay_to` instead of a new import.** `pay_to(from,to,amount,ptr,len)`
   already uses all six shim slots; adding kind/shape/tile/must needs either a
   second pointer or a wire struct, and changes every existing caller's answer
   from amount to report. Rejected: a six-slot stack payload plus a new answer
   encoding is exactly what `pay_spec` is, and the old verb stays stable.
2. **One bespoke import per capability** (`pay_fixed`, `pay_rent_fixed`,
   `pay_buy_must`, `pay_half`). Rejected: each new shape multiplies imports and
   the combination order is re-encoded per verb; a packed spec keeps one
   decoder and one engine entry.
3. **Only mutators, no declarative builder.** Every site would set kind, tile,
   must, shape in sequence with no atomic default; a missed reset leaks into
   the next call. Rejected: defaults and one `send` make the context complete
   by construction (I1). The builder is sugar over one import, so it respects
   the sugar rule.
4. **Factors as card-instance props or tile props.** Props are keyed to an
   instance/tile and have no settle lifetime; two cards' factors collide by
   key, not compose. Rejected: a scope stack is the only model that gives I4
   and the 0.25 outcomes.
5. **Four separate factor imports instead of a scope argument.** Works, but
   the four differ only in a storage selector and each needs its own Kani arm.
   Rejected: one import with a four-value enum.
6. **`settle_open` counter plus `player_pos(payer)` as settle-tile heuristic.**
   Breaks under a settle redirect (`settleBefore` re-target) and a
   `card_settle_at` on another tile, and has no answer for a nested settle.
   Rejected: a frame stack is exact.
7. **Always raise `settleAfter`, even on cancel/out.** Many settleAfter hooks
   assume a body actually ran; raising them on a cancelled settle would change
   unrelated cards. Rejected: keep the skip and add a terminal outcome on
   `tileResolved` that already always raises.
8. **Several getters instead of a packed report.** `pay_amount`,
   `pay_settled`, `pay_shortfall` would each read "the last payment" and be
   clobbered by a nested payment before the caller resumes. Rejected: the
   answer is returned atomically from the call.
9. **Infer build affordability from `money < cost` in the card.** The card
   cannot see `build_cost_pct`/`build_discount`; the inference is wrong exactly
   in the discounted case. Rejected: an effective reader mirrors the engine.
10. **Turn the `Paid` struct into the ABI return directly.** `Paid` is
    engine-only and serde-free; a guest mirror duplicates the layout with no
    gain. Rejected: packed flags are smaller and already round-trip through
    the answer log.

## 11. Open questions

No ruling below is invented; each is a decision the implementer must not
silently guess.

1. **Fixed legs and observers.** I2 lets the `effect`/`pay` windows run
   read-only. Should a 「支付时」 listener fire at all for a value the book
   calls 「不可被任何效果影响」, and does a cancel write still cancel the whole
   leg? Current proposal: listeners fire for observability but every write,
   cancel included, is ignored; the fixed bit lets a guard exclude itself.
2. **Mashiro rounding.** Is the half `floor`, `ceil10`, or the normal payment
   rounding? C# uses integer `/2`; this design uses `floor`. Does the half leg
   round to 10? Current proposal: no rounding because the leg is fixed.
3. **Mashiro's settle counter.** After the rewrite the money goes to the bank.
   Does it still count toward 「本回合的[结算]向其他玩家支付」 for 游击演出?
   Current proposal: yes, because the declaration named a player; marked in
   I9 and the engine branch.
4. **Kokoro 10000 when the payer cannot cover.** 「视为买地花费」 makes it
   mandatory here, but the book does not state the card's state when the payer
   goes out (foreign 「视为此卡未生效」). The current early return is kept.
5. **Buy-shaped riders on the 10000.** Does 「视为买地花费」 invite buy price
   modifiers (`BuyAdd`/`BuyMul`) or only the kind for pay-scale exemption? This
   design uses `PayKind::Buy` through the ordinary pay pipeline, not the
   `buy_quote` stages.
6. **CiRCLE's normal settle carve-out.** C# skips the 6000 on a normal CiRCLE
   reward; the trigger has no move tag (BACKLOG ABI-02). Out of G02's surface.
7. **`must` on the 6000.** A settle collection is mandatory in the same sense
   as rent; if CiRCLE's settle can be a special unowned-tile flow, the payer
   may prefer a refusal. Current proposal: mandatory.
8. **Dice rent quoting.** `rent_quote` rolls a fresh d20 for a dice-rent tile.
   Is that the partner's "real 收费" (a fresh collection) or should the figure
   be frozen for the current payment? Current proposal: fresh roll.
9. **Auction reduce-below-bid.** If crystals cut an auction's money movement,
   does the recorded deal price stay the bid or become the reduced figure?
   Current proposal: the movement is reduced, the bid record stays.
10. **Spend window vs raise funds.** Should `spendBefore` run before the
    redemption's affordability check so a reduction can prevent a shortfall?
    Current proposal: yes; the window precedes the check.
11. **Raw losses.** Does 「消耗」 cover `lose_raw` / `adjust_money` losses, or
    only redeem and auction? Current scope: the two named money moves.
12. **Miracle choice.** 「可」 may be a player choice. C# auto-returns 0 while a
    crystal remains. Is an ask required, and if so at `BuildBefore`? Current
    proposal: no ask; spend whenever a cost exists.
13. **`settleBody` cancel is 「未结算」.** It currently still raises
    `settleAfter`. For 学生会的检查 it must count as not settled. The outcome
    enum carries `Cancelled`; the card's refund branch must not treat its
    `SettleAfter` as success.
14. **Game end during a settle.** `!playing()` maps to `Aborted`; does the
    council refund still apply after the match is decided? Current proposal:
    yes, the card movement is harmless.
15. **Factor overflow/precision.** milli products beyond `i32` and f64
    precision for many factors are unbounded in the book. Proposal: saturate
    the milli getter and keep f64 arithmetic like today.
16. **`paid_in_settle_for` and the rewrite in 11.3.** If the half leg also
    counts, the payee can inflate its own counter by accepting a Mashiro
    rewrite. Confirm both legs count.
17. **`PayeeHalf` plus an explicit target.** The shape forces the main leg to
    the bank. If a responder both sets `PayeeHalf` and a different target, is
    the target honoured as the half's payer? Current proposal: the main leg
    stays a bank delete and the declared payee is the half victim; a target
    write is ignored.
