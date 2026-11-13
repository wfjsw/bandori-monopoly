# ABI design — payment-pipeline class-B batch

**Status: implemented on `worktree-wf_2376cbc1-b6e-2`. Worktree
`/home/monopoly/web/.claude/worktrees/wf_2376cbc1-b6e-2`.**

Four `docs/rulebook/BACKLOG.md` class-B items, all in the payment / purchase
pipeline:

| item | slug | what it fixes |
|---|---|---|
| ABI-11 | `now-sumimi-running-loss` | SUMIMI's [反击] read the single payment, not the turn's running loss |
| ABI-12 | `force-stays-mortgaged-tri-state` | `tile_prop` could not tell an explicit `FORCE_STAYS_MORTGAGED = 0` from an absent key |
| FORCE-WRITE-01 | `force-buy-write-stages` | 强制购买 had no price stages an effect could write |
| NFO-CANCEL-01 | `nfo-cancel-unplace` | NFO's unplace ran on a payment cancelled before its shave stage |

No `ABI_VERSION` change (the batch appends to the engine kind table and the
host import surface; the ABI-version doc block is untouched by instruction).
No `SAVE_VERSION` bump: both new world fields are `#[serde(default)]` and
deserialize from older saves as empty / clear.

---

## 1. Ground truth

**ABI-11** — `docs/rulebook/cards.json`, id `Sumimi:现在她是Sumimi的小初啦`:

> 「[反击] 当你在你的本回合开始后到下回合开始前之间失去资金的总额即将超过你所在格子的[收费标价]时打出此卡，获得相当于你所在格子[收费标价]数额的资金（RiNG则为其基础乘数）」

「失去资金的**总额即将超过**」 is a crossing window over a running total that
starts at the seat's **own** turn start; the old guard compared only the
current payment's `value`, so it fired on every payment past the tag.

**ABI-12** — 基础[结算] 6.2:

> 「[支付]…购买格子地契和建造已有房子的资金总价的两倍，从该玩家处强行购买该格地契，获得的地契仍为抵押状态」

and 其他规则 6:

> 「抵押，赎回，和强制购买的资金变动不受任何效果影响，除非效果写明改动抵押，赎回，或强制购买」

`FORCE_STAYS_MORTGAGED` is the prop a rule that must instead free the deed
writes (`0`). `World::tile_prop` folds absent onto `0`, so the prop was
unreadable as a tri-state.

**FORCE-WRITE-01** — the same 其他规则 6 clause. 「除非效果写明…强制购买」 is the
door: an effect that spells 强制购买 may touch the force-buy figure. The
generic `BuyAdd` / `BuyMul` / `BuySet` stages must keep skipping a Force quote.

**NFO-CANCEL-01** — `docs/rulebook/cards.json`, id `R:NFO` face 3:

> 「若结果为3，将此卡置于场上，你下次付款时自动减免1000资金的消耗并将此卡置入弃牌堆」

The unplace rides `PayAfter`, which also fires for a cancelled or zeroed
payment. Only a payment that actually reached the card's `paySplitAt` shave is
「付款时…减免」 and uses the card up.

---

## 2. Surface

### 2.1 New guest reads

| verb | signature | meaning |
|---|---|---|
| `ruleset_lost_this_turn` | `(player: i32) -> i32` | money the player has actually lost since their own turn started |
| `ruleset_trigger_pay_split_ran` | `() -> i32` | the in-flight payment reached its `paySplitAt` stage |
| `ruleset_tile_prop_state` | `(tile: i32, key: ptr, len: i32) -> i32` | `-1` absent / `0` present-clear / `1` present-non-zero |

SDK wrappers: `ctx::lost_this_turn(player)`, `ctx::trigger::pay_split_ran()`,
`ctx::tile_prop_state(tile, key)`.

`lost_this_turn` covers every **actual debit**: the pay pipeline's settlement
(`money_inner`), `pay_money`, `adjust_money_raw`'s loss direction, the raw
`lose`, force-buy, auction settlement and redeem. A cancelled or
shave-to-0 payment adds nothing (the debit never runs).

### 2.2 New kinds

Three Filter kinds, appended to the engine table (ids **143–145**, after
`payCalcBefore` = 142):

| id | wire | payload |
|---|---|---|
| 143 | `forceBuyAdd` | `Payload` (the `BuyAdd` record) |
| 144 | `forceBuyMul` | `Value` |
| 145 | `forceBuySet` | `Value` |

Typed mirrors: `card_sdk::abi::{TriggerKind,FilterKind}::ForceBuy*`.

---

## 3. Wiring

* **ABI-11**
  * `World::losses: Vec<i32>` (save, `serde(default)`), `bump_loss`,
    `losses_this_turn`; zeroed for the starting seat in `turn_start`.
  * `bump_loss` at each actual debit site listed in §2.1.
  * Guest: `sys.rs` / `sys_kani.rs` / `kani_host.rs` (`Op` 499) / `ctx.rs` /
    `hostfns.rs` (`lost_this_turn`) / `CardWorld` + `wasm_rules` + trait
    default + `native_shims.rs` / `host.rs` / `rules/kani/src/{host,world}.rs`
    + `rules/kani/build.rs` `ARRAY_OPS`.
  * `rules/cards/card-sumimi/src/now_sumimi.rs`: guard is now
    `lost + value > tag && lost <= tag`; the `TODO(ABI)` is gone.
* **ABI-12**
  * `World::tile_prop_state` (engine read); `offer_force_buy` passes
    `stays_mortgaged = tile_prop_state(...) != 0` into `default_deal` (default
    `true`).
  * Guest: the same full chain (`Op` 501).
* **FORCE-WRITE-01**
  * `game_kinds` rows, `abi.rs` consts, `kinds.rs` fold table,
    `kind_table_pin.rs` rows + shape map, `trigger_reason.rs`.
  * `wasm_rules.rs`: `FORCE_BUY_STAGES` beside `BUY_STAGES`; the quote picks the
    list by `BuyKind::Force`; `declares(kind)` keeps a stage off a ruleset that
    never spells it.
  * Locale labels `anim.chainKind.forceBuy{Add,Mul,Set}` in
    `webui/src/i18n/locales/{en,zh-CN}/ui.json` (the chain-start animation
    reads them; `message_args.rs` requires every engine spelling).
* **NFO-CANCEL-01**
  * `World::pay_split_ran: bool` (save, `serde(default)`); `money()` brackets it
    per payment (save / clear at start, restore after) so a nested payment
    cannot eat the outer leg's answer; `money_inner` sets it when the
    `paySplitAt` stage is about to run; guest `trigger::pay_split_ran()` (Op
    500).
  * `nfo.rs` PayAfter guard is a Rust fn `pay_split_ran`.

Docs: `docs/PURCHASE.md` stage list / P3 / risk bullet now name the force
stages.

---

## 4. Save and record impact

* `losses` and `pay_split_ran` are saved world state, both `#[serde(default)]`
  (empty Vec / false). An old save loads with a zeroed running total and a
  clear bit.
* No view (`MatchState`) or `MatchPrompt` field changes; no wire field added.
* Existing recordings keep running: no kind was renumbered and no import was
  removed.

---

## 5. Tests

`rules/tests/tests/ab_payment.rs` (four of the nine fail on the pre-fix
engine -- one per item; the rest pin the untouched behaviour):

1. `now_sumimi_fires_only_at_the_crossing_payment` — 200 / 200 / 200 over a 320
   tag: fires only on the second; a fresh own turn re-arms the crossing.
2. `force_buy_keeps_the_mortgage_by_default`
3. `force_buy_with_an_explicit_zero_clears_the_mortgage`
4. `force_buy_with_a_nonzero_prop_keeps_the_mortgage`
5. `a_force_buy_add_hook_retunes_the_force_price` — 2 × 3000 + 1500 = 7500
6. `a_generic_buy_mul_hook_retunes_an_ordinary_buy` (control)
7. `a_generic_buy_mul_hook_does_not_retune_a_force_buy`
8. `nfo_stays_placed_when_the_payment_is_cancelled_before_the_split`
9. `nfo_is_unplaced_when_the_shave_brings_the_payment_to_zero`

Fixtures: `TEST:payAmount` (pay `TEST.pay.amount`), `TEST:buyMulHalf`
(generic `BuyMul` ½), `TEST:forceBuyAdd` (`ForceBuyAdd` +1500).

Regression suites re-run: `kind_table_pin`, `guards`, `g06_windows`,
`roselia`, `spec_card_r`, `rulebook`, `spec_g_money`, `spec_g_settle`,
`agent_funding`, `g02_pay`, `g08_zone_lifecycle`, `silent_shipped`,
`gap_money`, `cargo test -p game-core`.
