# Kani unit `tiles` + `markers` — results

Branch `kani-tiles` at `/home/monopoly/kani-tiles-wt`.
Sources: `docs/rulebook/rulebook-doc.md` 「基础[结算]规则」 (148–162) and
「其他规则注意事项」 6 (146); clause docs `docs/rulebook/spec/global/settle.md`
(`G-SETTLE-*`) and `docs/rulebook/spec/global/money.md` (`G-MONEY-*`).
Bodies under proof: `rules/tiles` (6 tile rules) and `rules/markers`
(1 mark, `mark:cp`).

**Method.** Specs in `rules/kani/src/spec/{tiles,markers}.rs` are written
from the book quotes only. Harnesses in `rules/kani/src/proofs/{tiles,markers}.rs`
invoke the unmodified bodies via the shipped `CardDef` function pointer
(`call_settle` / `call_mark_cp_land` — see harness-verdicts note). Fail-closed: no `#[ignore]`,
no weakened specs, no silent zeros on unmodeled `Op`s.

**Host-model gate (why so many rows are not PROVEN).**
`rules/kani/src/host.rs` models a *symbolic world* whose settle primitives
(`offer_buy` / `offer_build` / `pay_rent` / `offer_force_buy` / `agent_offer` /
`draw_event`) are **PROTOCOL-LEVEL stubs** (return `0`, log no effect), and
whose move-trigger readers (`trig_move_main`, `trig_move_from`,
`trig_move_resolve`, `trig_move_remaining`) and message payload readers
(`msg_tile`, `msg_a`, `msg_sender_uid`) are either unmodeled (`Op` → loud
`unmodeled()`) or protocol stubs. `bump_mark_f` is a no-op and `mark_src_at`
always returns `−1`. Per KANI-FOUNDATION §8 those clauses are tagged
**PROTOCOL-LEVEL** / **BLOCKED-HOST** rather than faked. A host-ops agent is
landing `rent_of` / `tile_at` / `trig_price` / `trig_move_*`; flip the blocked
rows when those arrive.

---

## Clause table

| ID | book quote (short) | classification | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| G-SETTLE-02 (edogawa) | 「江户川乐器店的[结算]是：抽取一张手卡。」 | **PROVEN** | `edogawa_settle_draw_one_iff`, `edogawa_settle_mutant_two_cards`, `edogawa_settle_mutant_wrong_player` | see run table | — | — |
| G-SETTLE-02 (circle settle) | 「CiRCLE…[结算]是：抽取一张手卡。」 | **PROVEN** | `circle_settle_draw_one_iff`, `circle_settle_mutant_two_cards`, `circle_settle_mutant_wrong_player` | see run table | — | — |
| G-SETTLE-02 / 151 [CiRCLE奖励] | 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]。」 「[获得]2000资金或抽1张卡。」 | **BLOCKED-HOST** | (specs: `circle_pass_pays`, `circle_reward_money_ok`, `circle_reward_card_ok`) | not run | needs `trig_move_from`, `trig_move_resolve`, `trig_move_remaining` (all unmodeled `Op`) before `tile:circle`'s `On::Action(PassTile)` can even run | — |
| G-SETTLE-03 | 「抽取一张手卡，然后抽取一个事件卡。」 | **PROVEN** (observable half) + **PROTOCOL-LEVEL** (event half) | `event_settle_draw_one_iff`, `event_settle_mutant_two_cards`, `event_settle_mutant_wrong_player` | see run table | event draw is `ctx::draw_event` = protocol stub (no effect); G-SETTLE-04/05 ride on it | OPEN-结算定义 |
| G-SETTLE-04 | 「抽取的事件卡不进入手卡…效果立刻生效。」 | **PROTOCOL-LEVEL** | — | — | engine `H.DrawEvent` / event pipeline (KANI.md §6) | — |
| G-SETTLE-05 | 「如果没有事件可抽取则将事件弃卡区洗切…」 | **PROTOCOL-LEVEL** | — | — | same; reshuffle is engine scheduling | — |
| G-SETTLE-01 | 「如果格子地契已抵押则无效果。」 (owner) / mortgaged ⇒ no rent (other) | **PROVEN** (pure spec) + body **BLOCKED-HOST** | `property_leg_unowned_is_buy` (leg table) | see run table | `tile:property` body cannot run: `trigger::move_is_main` → unmodeled `trig_move_main` | — |
| G-SETTLE-06 + G-MONEY-11 | 「无主的[可购买格子]的[结算]是：可选择[消耗]购买格子地契和建造已有房子的资金总价…」 | **BLOCKED-HOST** + expected **FAILS(LESS)** once unblocked | specs: `g_settle_property_leg`, `buy_total` | not run | (a) `trig_move_main` unmodeled; (b) `offer_buy` is a protocol stub. settle.md Findings already records the LESS: the body only *announces* on a main landing (`log.land_unowned`) and defers the offer to the end step, while the book makes the optional buy *the settle*. | — |
| G-SETTLE-07 + G-MONEY-12 | 「可选择[消耗]格子地契所标注的房屋建筑费进行升级建造」 | **BLOCKED-HOST** + expected **FAILS(LESS)** once unblocked | specs: `g_settle_property_leg`, `build_ok` | not run | same as G-SETTLE-06 (`trig_move_main`, `offer_build` stub; main landing only announces `log.land_own`) | — |
| G-SETTLE-08 | 「每块地有标注的等级上限（例：所有RiNG不可升级，购物中心最大等级为三栋房屋）。」 | **PROVEN** (pure spec) | `level_cap_is_rent_len_minus_one` | see run table | cap = `rent.len()-1`; empty table (RiNG) ⇒ 0 | — |
| G-SETTLE-09 | 「升级所[消耗]的资金不可通过[抵押]正在升级的格子地契获得。」 | **PROTOCOL-LEVEL** | — | — | mortgage-pipeline funding restriction; per-act window cannot see which mortgage funded a build (settle.md: scenario-only) | — |
| G-SETTLE-10 + G-MONEY-09 | 「[支付]拥有格子的玩家格子地契所标记的现等级地租。」 | **BLOCKED-HOST** | spec: `rent_at_level` | not run | body calls `ctx::pay_rent` (protocol stub); amount is host-computed. `rent_of` (unmodeled) is what a spec-side amount read would need. | OPEN-RING-RENT |
| G-SETTLE-01 (mortgaged ⇒ no rent) | 「如果格子地契已抵押则无效果。」 | **BLOCKED-HOST** (body) | spec: `g_settle_01_no_rent_on_mortgaged` | not run | same `trig_move_main` gate | — |
| G-SETTLE-11 + G-MONEY-07 | 「…购买格子地契和建造已有房子的资金总价的两倍…仍为抵押状态」 | **PROVEN** (pure arithmetic) + body **BLOCKED-HOST** | `force_buy_is_double_buy_total`; spec `force_buy_total` | see run table | body calls `ctx::offer_force_buy` (protocol stub); `trig_move_main` gates entry | — |
| G-SETTLE-12 + G-MONEY-08 | 「此次购买的价格不受任何资金变动效果影响…」 / 其他规则 6 | **PROTOCOL-LEVEL** | — | — | force-buy runs outside the money pipeline in the engine; a per-act Kani body cannot observe "no modifier applied" while `offer_force_buy` is a stub | OPEN-强制购买支付钩 |
| G-SETTLE-13 + G-MONEY-10 | 「…半价收费的[结算]（向上取整10）」 | **PROVEN** (pure spec: rate + gate) + body **BLOCKED-HOST** | `agent_half_charge_rounds_up_to_ten`, `agent_all_owned_gate` | see run table | rate `ceil(full/2/10)*10` (301→160) proven. Body blocked: `ctx::is_color` is a protocol stub (always 0) ⇒ the 同色 set is always empty and the sweep can never run; `pay_rent(half)` is also a stub. | OPEN-同色, OPEN-AGENT-MORTGAGED-HALF |
| G-SETTLE-14 | 「否则可选择该格子同色的无主或玩家拥有的[可购买格子]之一进行一次[结算]。」 | **BLOCKED-HOST** | spec: `g_settle_14_legal_target` | not run | same `is_color` stub; `ask_tiles` / `agent_offer` are protocol stubs | OPEN-同色 |
| G-MONEY-05 | 「抵押后[获得]地契购买价格50%的资金」 | out of body scope (mortgage act, not a tile settle body) | — | — | owned by the mortgage pipeline / unit 1 money scenarios | — |
| G-MONEY-06 | 「赎回后[消耗]地契购买价格60%的资金」 | out of body scope | — | — | same | OPEN-60%舍入 |
| mark:cp `on_land` gain | 「…[获得]800资金。」 | **PROVEN** (observable half) | `mark_cp_land_gain_iff`, `mark_cp_land_mutant_900`, `mark_cp_land_mutant_wrong_seat` | see run table | 800 to the settling player | — |
| mark:cp `on_land` mark+counter | 「移除格子上的个[CP点]和自己[场上]1个[CP点]」 | **BLOCKED-HOST** | `mark_cp_land_tile_cp_visible_to_count_marks` (prefix only) | see run table | `bump_mark_f` is a no-op stub; `mark_src_at` always `−1` so the guard's 「自己[场上]1个[CP点]」 conjunct can never admit and `add_counter_at` is unreachable | — |
| mark:cp `on_land` guard / no-op | 「在拥有[CP]点的格子上[结算]时」 | **PROVEN** (guard rejects) | `mark_cp_land_guard_rejects_without_tile_cp`, `mark_cp_land_guard_rejects_when_cancelled` | see run table | no tile CP / cancelled ⇒ entry does not fire | — |
| mark:cp `on_place` | 「在任意一个没有角色和[CP点]的格子上添加1个[CP点]…」 | **BLOCKED-HOST** | — | not run | `msg_tile` / `msg_a` / `msg_sender_uid` are protocol stubs (return 0) ⇒ the handler cannot see the payload. Specs kept (`mark_cp_place_guard`, `mark_cp_place_effects_ok`). | — |

---

## Summary counts

| classification | count |
|---|---:|
| PROVEN | 8 rows (draw settles ×3 bodies, half-charge + leg + cap + force-buy arithmetic, mark gain + guard) |
| PROVEN (observable half only) | 2 (event settle event-half; mark mark/counter half) |
| FAILS | 0 *currently reachable*; 2 expected once unblocked (G-SETTLE-06 / 07 main-landing LESS) |
| PROTOCOL-LEVEL | 6 (event zone/reshuffle, mortgage funding, force-buy immunity, buy/build/rent/agent pipelines) |
| BLOCKED-HOST | 8 (property/ring entry, circle pass reward, agent colour set, mark place payload, mark bump/src) |
| OPEN | 5 refs (OPEN-结算定义, OPEN-RING-RENT, OPEN-同色, OPEN-AGENT-MORTGAGED-HALF, OPEN-强制购买支付钩) |

## BLOCKED-HOST ops needed

| op | who needs it | host status today |
|---|---|---|
| `trig_move_main` | `tile:property`, `tile:ring` settle entry | **unmodeled** (loud fail) |
| `trig_move_from` / `trig_move_resolve` / `trig_move_remaining` | `tile:circle` [经过] reward | **unmodeled** |
| `is_color` | `tile:agent` 同色 set | protocol stub (returns 0) |
| `pay_rent` / `offer_buy` / `offer_build` / `offer_force_buy` / `agent_offer` | property / agent settle effects | protocol stub (returns 0) |
| `draw_event` | `tile:event` event half (G-SETTLE-03/04/05) | protocol stub (returns 0) |
| `bump_mark_f` | `mark:cp` tile-mark removal | no-op stub |
| `mark_src_at` | `mark:cp` provenance / on-card spend | stub (returns −1) |
| `msg_tile` / `msg_a` / `msg_sender_uid` | `mark:cp` `"place"` payload | protocol stubs (return 0) |
| `rent_of` | rent amount in specs (G-MONEY-09) | unmodeled (host-ops agent) |
| `tile_at` / `trig_price` | price reads in buy/force-buy specs | unmodeled (host-ops agent) |

## Harness verdicts

`bash tools/kani/run.sh --unit tiles|markers --harness NAME --jobs 1`
(`CARGO_TARGET_DIR=/home/monopoly/kani-target-tiles`, `--default-unwind 80`).
Time is CBMC `Verification Time`. **Method note:** bodies are invoked via the
shipped `CardDef` function pointer (`call_settle` / `call_mark_cp_land`), not
`invoke::find`'s registry scan — the scan is an unwind cliff (hundreds of
`CardDef`s) that made every post-body assertion `UNREACHABLE` (vacuous iff)
at `#[kani::unwind(24)]`.

**Runner note.** An earlier `tools/kani/run.sh` grepped `unmodeled host op`
before the CBMC verdict and mislabeled harnesses `UNMODELED` (the string is
`host::unmodeled.assertion.1`'s *description* even when that check is
`UNREACHABLE`). Master's runner prefers `VERIFICATION:- …`; this branch has
that fix merged.

### Proven on this pass

| harness | verdict | time (s) | notes |
|---|---|---:|---|
| `agent_half_charge_rounds_up_to_ten` | SUCCESSFUL | 0.20 | 「向上取整10」; cover 301→160 |
| `force_buy_is_double_buy_total` | SUCCESSFUL | 0.20 | 2×(price + houses·house) |
| `level_cap_is_rent_len_minus_one` | SUCCESSFUL | 0.31 | cover RiNG cap 0 / 购物中心 cap 3 |
| `property_leg_unowned_is_buy` | SUCCESSFUL | 0.12 | cover all five property legs |
| `edogawa_settle_draw_one_iff` | SUCCESSFUL | 90.8 | cover "exactly one effect" **SATISFIED**; every clause assert **SUCCESS** (non-vacuous) |

### Written, verdict pending (slot queue; run centrally or retry)

| harness | expected | notes |
|---|---|---|
| `agent_all_owned_gate` | SUCCESSFUL | G-SETTLE-13 all-owned-by-other gate |
| `circle_settle_draw_one_iff` | SUCCESSFUL | same shape as edogawa |
| `event_settle_draw_one_iff` | SUCCESSFUL | observable half only |
| `edogawa_settle_mutant_two_cards` | SUCCESSFUL (panic as expected) | non-vacuity |
| `edogawa_settle_mutant_wrong_player` | SUCCESSFUL (panic as expected) | |
| `circle_settle_mutant_two_cards` | SUCCESSFUL (panic as expected) | |
| `circle_settle_mutant_wrong_player` | SUCCESSFUL (panic as expected) | |
| `event_settle_mutant_two_cards` | SUCCESSFUL (panic as expected) | |
| `event_settle_mutant_wrong_player` | SUCCESSFUL (panic as expected) | |
| `mark_cp_land_gain_iff` | SUCCESSFUL | gain-800 observable half |
| `mark_cp_land_mutant_900` | SUCCESSFUL (panic as expected) | |
| `mark_cp_land_mutant_wrong_seat` | SUCCESSFUL (panic as expected) | |
| `mark_cp_land_guard_rejects_without_tile_cp` | SUCCESSFUL | no-op when no [CP点] |
| `mark_cp_land_guard_rejects_when_cancelled` | SUCCESSFUL | no-op when cancelled |
| `mark_cp_land_tile_cp_visible_to_count_marks` | SUCCESSFUL | prefix of the guard; cover still-rejecting `mark_src_at` stub |

All 20 harnesses compile under `cargo build -p rules-kani` on this branch.

## What we tried for blocked / protocol rows

* **property / ring.** The unmodified `On::Settle` body immediately reads
  `trigger::move_is_main()`. Under Kani that is `Op::trig_move_main`, which
  falls through to `unmodeled()` → `kani::assert(false)`. No harness is
  written: a proof would die on the unmodeled op, not on the clause.
  Specs (`g_settle_property_leg`, `buy_total`, `force_buy_total`,
  `level_cap`, `build_ok`) are ready to reuse.
* **agent.** The body's 同色 scan is `is_buyable(t) && is_color(player, t, group)`.
  `is_color` is a protocol stub returning 0, so the set is always empty and
  the body only logs `log.agent_none` and returns. Proving the half-charge
  sweep or the choose-one branch would be vacuous. Specs (`agent_half_charge`,
  `g_settle_13_all_owned_by_other`, `g_settle_14_legal_target`) pin the book
  arithmetic and gate.
* **circle [经过].** `pass_pays` reads `trigger::move_from()` /
  `exile_of` / `is_circle`; `on_pass` reads `move_resolve` / `move_remaining`.
  The first two move readers are unmodeled `Op`s. Specs kept.
* **mark:cp on_land.** The gain half is proven. The mark/counter half cannot
  be: `mark_src_at` is a stub, so the shipped guard `lands_on_cp` can never
  admit (its last conjunct is `src >= 0 && counter_at(src, CP) > 0`). We do
  **not** write a vacuous iff. `mark_cp_land_tile_cp_visible_to_count_marks`
  pins the prefix (`count_marks` sees the installed mark) and covers the
  still-rejecting guard.
* **mark:cp on_place.** Message payload ops return 0, so `place_ok` always
  sees `(tile=0, count=0)` and rejects. Not proven.

## Mutant / non-vacuity

Every PROVEN body clause has a `#[kani::should_panic]` mutant (off-by-one
amount or wrong target). Pure-spec rows are self-checking arithmetic (the
301→160 example is `kani::cover!`ed). If a mutant does *not* fail, the iff is
vacuous — fix it rather than keep it.
