# Negation audit — does any spend escape its effect?

**Date:** 2026-10-07 · read-only audit · no code changed

**Terminology:** "counteraction" = the [反击] mechanism (chain / ring / window).
"hook" = passive listener ([持续]/[特] / Fx). Never "reaction".

**Ruling under test (user, 2026-10-07):** there are **no activation costs**. Every
[消耗]/[支付] (and by extension any spend) written in a card/skill/event text is
**EFFECT CONTENT**: it resolves together with the rest of that effect at effect
resolution, and if the effect is negated (抵消/取消 by a counteraction), the
payment is negated together with it — nothing is paid. Unaffordable in-effect
payments follow the **shortfall path** (mortgage, then bankruptcy).

Rulebook anchors: `data/rules.txt` L13-14 ([消耗]/[支付] definitions), L16
(破产 shortfall), L76 (抵押 offer), L32 (「[反击]…结算优先于X」), L89 (ring);
支付阶段 table `docs/rulebook/rulebook-doc.md:227-235` (stage 5 「取消支付」
distinct from stage 6/7).

---

## 0. Summary

| | count |
|---|---|
| Activation paths checked | 6 (hand play, skill press, event, tile settle, counteraction link, money pipeline) |
| Paths where the whole effect is negated **before** the body | 5 of 6 — clean |
| **Violations / gaps** | **5** (V1–V5) + 1 split-shape question (V6) + 1 open ruling (V7) |
| Text-less money play gates (follow-up) | **6 cards** (§4) |
| Marker-spend question for the user | §5 |
| Spends inside a guard | **0** |
| Spends that survive a whole-effect negation at the top level | **0** (the window is pre-body) |

Top cases: **V1** hook bodies run before the counteraction window at the same
trigger; **V2** money play gates model effect-content payments as activation
costs.

---

## 1. Activation-path map (where the negation window sits)

| Path | Negation window | Body runs? | Money/marker spend in body goes through | Verdict |
|---|---|---|---|---|
| Hand card play `play_from_hand` | `card` raise, **before** `rules.play` (`play.rs:3916-3930`) | only if `!t.is_cancelled()` | `ctx::pay`/`transfer` → host request → `Cx::money` | **clean** |
| Skill press `use_skill` | `skillUsed` raise, **before** `rules.play` (`play.rs:641-656`, 花园多惠（2）「将其抵消」) | only if `!t.is_cancelled()` | same, plus `ctx::spend_fire` (direct) | **clean** for money |
| Event `draw_event` | `event` raise, **before** `rules.event` (`play.rs:4024-4042`) | only if `!t.is_cancelled()` | `ctx::pay`/`transfer`/`gain` | **clean** |
| Tile settle `settle_at` | `settle` then `settleBody` (`play.rs:1512-1531`) | only if neither cancelled | `ctx::pay_rent` / `ctx::pay` | **clean** |
| Counteraction link | `resolve_rounds` checks `link.is_cancelled()` **before** `drive(Call::Counteract)` (`wasm_rules.rs:2695-2726`) | negated → body skipped, card already spent | body's own `ctx::pay` etc. | **clean** for money |
| Any raise (`Cx::counteract`) | order at one trigger: (1) acting card's own follow-up → (2) hooks → (3) `hand_counteractions` (`wasm_rules.rs:3610-3887`) | see **V1** | | **gap** |
| Money movement `Cx::money` | `effect` declaration window **before** money moves (`play.rs:2955-2987`); `payTotalCancel` before split (`play.rs:3201`) | payment settles only if not cancelled | the payment itself | **clean** |

Chain resolution is LIFO over the answer tree; answers settle before the link
they answer, so a counter that negates an earlier link runs before that link's
body and the body never starts (`wasm_rules.rs:2681-2743`). No link's money can
be left behind by a **later** link.

---

## 2. Violations / gaps

### V1 — hook bodies run BEFORE the counteraction window at the same trigger — **FIXED 2026-10-07**

* **Where:** `crates/game-rules/src/wasm_rules.rs:3610-3887` (`CardRules::counteract`).
  Order is (1) the acting card's own `Call::Counteract` follow-up (`3610-3632`),
  (2) field/tile/event/lingering **hooks** (`3634-3879`, `drive_hook`), and only
  then (3) `hand_counteractions` (`3881-3887`). The comment at `3603-3608` still
  describes (2) as the [反击] round — stale; hooks were inserted between.
* **Current behaviour:** a spend inside a hook body (e.g. Fire bird's
  「每回合结束时失去400资金」 `card-roselia/src/fire_bird.rs:127-140`,
  `ctx::pay` on `turnEnd`) settles **before** any seat can counteract that
  `turnEnd`/`settle`/`card`/`effect` link. Cancelling the link afterwards does
  not roll the spend back — the hook's `ctx::pay` already ran `Cx::money` (and
  its own nested `effect` window opened and closed).
* **Ruling / rulebook:** rulebook L32 「[反击]…结算优先于X」 — the counteraction
  settles before X. If X includes its triggered [持续] settlement, hooks must
  run after the window. Independently, the ruling says a spend is effect content
  of the card that wrote it: a hook's payment is that [持续]'s effect content,
  and nothing in the current shape can negate *that* effect as a whole after the
  fact.
* **Nuance (not auto-wrong):** the hook's payment does open its **own** nested
  `effect` window, so a pay-stage cancel can still drop that payment. The gap is
  whole-trigger negation, and the hook/counteraction ordering question.
* **Proposed test:** `rb_chain::hook_spend_settles_before_the_counteraction_window`
  — field card 「当…[结算]时[支付]500」 on `settle`; a 网络链接异常-style counter
  cancels the settle. Assert (per the ruling you pick) either (a) the 500 is
  paid and stands (current), or (b) the 500 never moves because the counter
  settles first.
* **Fixed (2026-10-07):** `counteract()` now runs (1) the acting card's own
  follow-up, (2) the hand-counteraction [反击] round, (3) hooks -- and (3) is
  skipped entirely when the trigger is cancelled. Rulebook L32
  「[反击]…结算优先于X」. Files: `crates/game-rules/src/wasm_rules.rs`.

### V2 — money play gates model effect-content payments as activation costs — **FIXED 2026-10-07**

* **Where:** `cant_play` / `WhyNot` money checks — full list in §4.
* **Current behaviour:** the play is refused outright (`err` message) when
  `money < N`. The player can never reach the shortfall path for that effect.
  `fire_bird.rs:33-34` even comments "the pay is the card's cost" — exactly the
  model the ruling eliminates.
* **Ruling requires:** no activation cost. Play the card; the in-body
  「[消耗]/[支付]」 is effect content; if unaffordable it runs
  `raise_funds` (mortgage offer `play.rs:3298-3344`, then `bankrupt`) with
  `Pay::must = true` (`wasm_rules.rs:2022`, PIPELINE-AUDIT B1 already fixed).
* **Proposed test:** `rb_money::unaffordable_in_effect_payment_raises_funds`
  — P0 holds 0 cash + one mortgageable deed, plays 通用:10次招募. Expect the
  mortgage offer, then the draw if the payment settled, or bankruptcy with the
  bank as creditor. Today the play is refused before any of that.
* **Fixed (2026-10-07):** all six `money_of < N` gates removed. The hand-count
  [限] on 「因为我一直相信着你」 stays. In-body payments run the Q1 shortfall
  path. Tagged `TODO(规则书)` (NEGATION-AUDIT V2). Tests: `rb_money::
  gacha10_playable_while_short_then_bankrupts`,
  `fire_bird_playable_while_short_then_mortgages`,
  `crimson_soul_playable_while_short_then_bankrupts`,
  `believe_you_playable_while_short_then_bankrupts`,
  `kokoro_circle_playable_while_short_then_bankrupts`,
  `starry_night_playable_while_short_then_bankrupts`.

### V3 — marker spends have no negation window of their own (see §5, user question)

* **Where:** `ctx::spend_fire` → `Run::spend_fire` (`wasm_rules.rs:1092-1109`),
  `ctx::add_crystals`/`set_crystals`, token writes. Direct world mutation on the
  run's copy; **no** `effect`/`payTotalCancel`/`pay` window, no per-marker
  counteraction. All-or-nothing; logged to `fire_spent_log` and raised as
  `fireSpent` after commit.
* **Current behaviour:** the marker leaves when the body reaches the call. The
  only negation that removes it is the whole-effect negation **before** the body
  (`skillUsed`/`card`/`event`). A later step's negation, or a pay-stage cancel
  of an accompanying money payment, leaves the marker spent.
* **Examples:** 「你可以消耗一个火罐指定…」 (`himari_step.rs:63-80` — pick
  first, then `spend_fire`, then `plan::set_parity`; if the move is later
  cancelled the fire stands); 「消耗7个[火罐]」 (`kaoru_prince.rs:128`);
  「消耗所有火罐」 (`misaki_card.rs:71`, inside a counteract body).
* **Ruling names payments.** Whether 「消耗N个火罐」/「移除[奇迹水晶]」 is a cost
  or effect content is **the question for you** (§5).

### V4 — `gain_fixed` bypasses the whole payment pipeline — **FIXED 2026-10-07**

* **Where:** `crates/game-core/src/engine/ops.rs:517-531`; only call site
  `rules/cards/card-morfonica/src/tritone.rs:51` (三全音's
  「立刻获得此次失去的资金金额」).
* **Current behaviour:** money moves immediately on the world copy; `payAdd` /
  `payMul` / `payChoose` / `effect` / `payTotal*` / `pay` never see it (by
  design, "skills and crits may not bend the figure"). `payAfter`/`paid` are
  raised only after the body commits (`wasm_rules.rs:1570-1611`).
* **Gap:** a counteraction cannot cancel or reshape this money movement. For a
  pure 「获得」 that is arguably correct (「固定」), but it is still money movement
  outside the 支付阶段 windows the rulebook tables. If 「获得」 is also effect
  content under the ruling, this is a hole.
* **Fixed (2026-10-07):** `ctx::gain_fixed` is `gain` under a name the cards
  already use -- Tritone's 「立刻获得此次失去的资金金额」 goes through the money
  pipeline (the `effect` [反击] window, the modifier stages, `pay` / `payAfter`).
  The C# `fixedAmount` bypass is gone. Files: `crates/game-rules/src/hostfns.rs`,
  `rules/card-sdk/src/ctx.rs`, `rules/cards/card-morfonica/src/tritone.rs`.

### V5 — the `pay` settlement window ignores `is_cancelled()` — **FIXED 2026-10-07**

* **Where:** `play.rs:3070-3107`. The `pay` trigger is raised, but only
  `amount <= 0` stops settlement — `t.is_cancelled()` is never read (the comment
  "Cancelled: nothing moves" refers to the zero-amount case).
* **Current behaviour:** a counteraction that calls `trigger::set_cancelled()`
  on the `pay` link does **not** stop the payment unless it also sets the value
  to 0. (No card does this today — 秘密与青春的虹彩 reshapes the amount; the
  rulebook timing table's 「取消支付」 is carried by the `effect` /
  `payTotalCancel` windows instead. See PIPELINE-AUDIT **B2**: window-5 cancel
  power sits at window-1 position.)
* **Gap:** the API invites a counter body to cancel `pay` and get silence.
* **Proposed test:** `rb_money::pay_window_set_cancelled_stops_the_payment` —
  a counter body that `set_cancelled()`s a `pay` link; expect 0 moved. Today it
  moves the full amount.
* **Fixed (2026-10-07):** `money_inner` treats `t.is_cancelled()` on the `pay`
  raise as a stop (settle 0, no reversal). Files:
  `crates/game-core/src/engine/play.rs`.

---

## 3. Partial negation — what is all-or-nothing and what is not

| Gesture | Where | Scope | Payment consequence |
|---|---|---|---|
| Whole-effect negation (`set_cancelled` / `negate_effect` on the `card`/`skillUsed`/`event`/`settle` link) | pre-body window | all-or-nothing | body never runs — **no payment left behind** |
| Pay-stage cancel (`effect` cancelled, or `payTotalCancel`) | inside `Cx::money`, before money moves | **this payment only** | that payment returns `Paid::default()`; the body resumes with 0 |
| Per-recipient (`Trigger::spare`, `ctx::cancel_designation`) | `play.rs:2974` `settles_for`, `net_error.rs:78-93` | one seat | that seat's leg skipped, the rest lands |
| Mid-body later-step negation | a nested window (e.g. `target`, `drew`) | that step | **earlier settled payments in the same body stand** |

The last row is the "all-or-nothing?" question. Example: 黑色生日
(`card-mujica/src/black_birthday.rs:46-55`) settles up to four `ctx::transfer`s
in one body; each opens its own `effect` window. A pay-stage cancel of the
second transfer does not roll back the first. Per the rulebook's explicit
partial gestures (stage 5 「取消支付」, 「取消其对目标之一的[指定]」) this looks
intended; per the ruling's 「resolves together with the rest of that effect」 you
may want effect-level atomicity (a cancelled leg voids the whole body, or the
reverse: the rest still resolves). **Needs your call** — no code change assumed.

The mirror shape is already coded as a **payment gate on the remainder**:
bodies that do `if paid < N { return }` (fire_bird:44, believe_you:43,
kokoro_circle:48, starry_night:60, to_the_peak:152, council_check:90,
one_of_us:173, dream_return:94) or `if paid > 0 { … }` (gacha10:30,
crimson_soul:66) drop the rest of the effect when the payment is cancelled or
reduced. A pure 「取消支付」 therefore cancels the whole remainder — which is one
reading of 「resolves together」, but it also means a **modifier-reduced** payment
(e.g. payAdd −600 on fire_bird's 1600) kills the placement even though the
payment did settle. Worth a ruling.

Whole-effect negation at the top level is genuinely all-or-nothing and never
leaves a payment behind: the `card`/`skillUsed`/`event`/`settle` window is
strictly pre-body, and a nested window can only negate its own link (the parent
`card` link is already settled and unreachable from a child `effect` trigger).

---

## 4. Money-gate list (follow-up: remove text-less play gates) — **DONE 2026-10-07**

All six are `cant_play` / `WhyNot` refusals on `money_of < N`. None of the card
texts writes a [限] on money (rulebook 「带有"[限]X"的卡只有在X成立时才可从手中
打出」) — the amount appears only as in-body [消耗]/[支付] effect content, so the
gate is **text-less**. Tag each `TODO(规则书)`.

| File:line | Gate | Card text (abbrev.) | Body spend |
|---|---|---|---|
| `rules/cards/card-general/src/gacha10.rs:17-21` | `>= 1500` | 「[消耗]1500资金，抽1张卡」 | `ctx::pay(1500)` then draw if `paid > 0` |
| `rules/cards/card-roselia/src/fire_bird.rs:32-36` | `< 1600` | 「支付1600资金将此卡放置…」 | `ctx::pay(1600)`, abort remainder if short |
| `rules/cards/card-ag/src/crimson_soul.rs:41-43` | `< 500` | 「选择[消耗]1到5次500资金」 | `ctx::pay(500 * n)`, crystals only if `paid > 0` |
| `rules/cards/card-hhw/src/believe_you.rs:31-32` | `< 800` | 「至少有另一张手牌时可发动，消耗800资金」 (hand-count is a real [限]; money is not) | `ctx::pay(800)`, abort if short |
| `rules/cards/card-hhw/src/kokoro_circle.rs:31-32` | `< 10000` | 「支付10000资金（视为买地花费）」 | `ctx::pay(10000)`, abort if short |
| `rules/cards/card-morfonica/src/starry_night.rs:36` | `< 1000` | 「支付X次1000的资金」 | `ctx::pay(1000 * x)`, abort if short |

**Not play gates** (body-internal branches — leave alone): `black_birthday.rs:39`
(bracket latch), `thanks_party.rs:24` (who is asked for an optional 500),
`guerrilla.rs:103` / `sweet_escape.rs:147` / `tsugu_ycm.rs:105` /
`tomoe_savior.rs:34` (price vs money inside a body), `starry_night.rs:196`
(roller affordability). Event-side affordability checks
(`events/src/lulu.rs:31`, `events/src/marina_box.rs:70`) gate an **optional**
spend; `lulu.rs:28-30` already carries a `TODO(规则书)` on the shortfall
question for a chosen teleport.

---

## 5. Marker spends — **answered 2026-10-07**

The ruling names payments (资金). 「消耗N个火罐」 / 「移除[奇迹水晶]」 / token
spends are handled as **raw state writes** (`ctx::spend_fire` all-or-nothing,
`ctx::add_crystals(-n)`, `ctx::add_tok`) with no counteraction window. Skill
texts use 「你可以消耗一个火罐…」 / 「消耗N个[火罐]」 wording (e.g.
`himari_step.rs:5`, `kaoru_prince.rs`, `arisa_bonsai.rs:7`
「使用2个[火罐]为自己的团卡添加1个[奇迹水晶]」).

**Ruled 2026-10-07:**

1. Marker spends are **effect content** -- spent as the effect resolves,
   nothing spent if the effect is negated (the whole-effect window already
   covers that). The press checks (「消耗7个[火罐]」) stay activation guards.
2. They get **their own counteraction window** (`markerSpend` / `markerGain`,
   TriggerKind 92/93) opened **before** the markers move. No shipped card
   listens yet; `rules/fixtures/test-cards`'s `TEST:markerDeny` pins the shape.

Exception: HHW:（美咲） may counteract with 0 火罐 -- 「消耗所有火罐」 is
vacuous when the pot is empty.

Related pre-gates: `can_use` fire checks (`himari_step.rs:56`, `kaoru_prince.rs:88`
`FIRE < 7`, `kiritani_zenith.rs:70`, `extraordinary_star.rs:77`, `uika_idol.rs`,
`anon_restart.rs`, `lisa_goddess.rs`, `mana_donut.rs`, `moca_self.rs`,
`tomoe_ramen.rs`, `tsugumi_plain.rs`, `rinko_1cm.rs`, `sato_red.rs`,
`misaki_other.rs`, `tukushi_try.rs`) refuse the press when the pot is short —
the same activation-cost shape as §4, if markers are effect content.

---

## 6. Counteraction cards themselves

Declaration (`build_round`, `wasm_rules.rs:2637-2642`) removes the card from the
hand **immediately**. Resolution (`resolve_rounds:2695-2726`) skips the body when
the link was negated, and the card is still spent ("the declaration and the
spend stand" — `TEST-FINDINGS.md:676`). That is the card-as-activation act
(「打出此卡」), not a [消耗]/[支付] in the text, so it is out of the ruling's
scope — but under a strict "the activation never happened"
(`Negation::Activation`) reading you may want the card returned to hand. YGO
keeps it spent. **Flagged, not changed.**

Money inside a counteraction body runs at that link's resolution (post-order
LIFO), so a later link that negates it prevents its payment entirely. Clean.

---

## 7. Bot / AI / autopilot

* `crates/game-core/src/engine/ai.rs` — `wants_buy` / `wants_build` /
  `wants_redeem` / `bot_wants_*` are **reserve policies for voluntary board
  offers** (buy / build / redeem / force-buy / auction bid). They pre-check money
  against a real quoted price, not against a card's effect-content payment. No
  pre-deduction.
* `webui/src/game/autopilot.ts:41-44` — explicitly: "Card plays and counteracts
  are unrestricted — the view carries no visible card cost to check against it."
  Consistent with the ruling. Only the §4 `cant_play` gates need removing for
  the follow-up; no autopilot change is implied.

---

## 8. Clean list (for the record)

* Hand play / skill press / event / tile settle: negation window strictly before
  the body. Payments inside the body cannot precede it.
* No spend in any `cant_play` / `can_counteract` / `hook_guard` (≈120 guards
  checked; all pure reads).
* `ctx::pay` / `transfer` / `pay_total` / `pay_leg` / `pay_rent` / `gain` all go
  through `Cx::money` (host request → `wasm_rules.rs:1971-2033`), so every
  money movement opens the `effect` declaration window before any money moves,
  and `Pay::must = true` routes shortfall through `raise_funds` → `bankrupt`.
* `payTotalCancel` drops the whole command before any 「[分摊]」 leg splits
  (`play.rs:3201`, `ctx::split_pay`).
* Chain LIFO: a counter that negates an earlier link runs first; the negated
  link's body (and its payments) never start.

---

## 9. Open rulings carried forward

| id | question | where |
|---|---|---|
| N1 | Hook vs counteraction order at one trigger (V1) — does 「结算优先于X」 put hooks before or after the [反击] round? | `wasm_rules.rs:3603-3887` |
| N2 | Marker spends (V3 / §5): effect content or cost? own window? | `spend_fire` / crystals / tokens |
| N3 | Effect-level atomicity (§3): does a pay-stage cancel void the rest of the body, and does a later-step negation void earlier settled payments? | `black_birthday.rs`, `if paid` gates |
| N4 | 「取消所有受到的效果」 refunds money already paid? | 像往常一样 `ran_as_usual.rs:90-110`, Sumimi不会解散哦 `no_breakup.rs:111` — already open in `TEST-FINDINGS.md:590-593` |
| N5 | `Negation::Activation` returns the counteraction card to hand? | `resolve_rounds:2705-2708` |
| N6 | Is 「获得」 (including `gain_fixed`) in the ruling's spend scope? | `ops.rs:517`, `tritone.rs:51` |