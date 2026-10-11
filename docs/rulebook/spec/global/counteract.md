# G-COUNTER -- [反击] 窗口 / 顺序 / 无效 / 专有名词

Unit 8 owns this file and `rules/spec/src/global/counteract.rs`. Append
clauses; do not edit other areas.

Terminology: "counteraction" for [反击] windows, "hook" for passive listeners.
Never "reaction".

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

---

## G-COUNTER-01

* **ID:** `G-COUNTER-01`
* **Source:** `rulebook-doc.md:79`
* **Quote:** 「[反击]：带有“[反击]X：Y”效果在X发生时打出并触发效果Y，且结算优先于X（例：X可以是被其他玩家的卡[指定]时）。」
* **Trigger:** an observed input whose event window contains a `"card"`
  activation marked `negated` (a counteraction cancelled it).
* **Condition:** the negated activation's card body would have moved
  money / status / position (any of `pay` / `rent` / `gain` / `lose` /
  `mortgage` / `redeem` / `move` children).
* **Effect:** no such child events hang under the negated activation
  (`parent == that event's id`). The negation's own log line may.
* **Frame:** n/a (read of the event window).
* **Rulings & OPEN:** 「结算优先于X」 is why the negation can cancel X outright:
  Y resolves first, and a negating Y leaves X's body unrun. The counteraction
  *order* is `G-COUNTER-02`. A `negated` activation still flashes and logs --
  only the body is empty.
* **Executable:** `rules_spec::global::counteract::g_counter_01`.
  Scenario: `g_counter_01_negated_play_runs_no_body`
  (`通用:网络链接异常` negates `通用:10次招募（1回限定）`).

---

## G-COUNTER-02

* **ID:** `G-COUNTER-02`
* **Source:** `rulebook-doc.md:143`
* **Quote:** 「如果有多名玩家可在同一时间发动[反击]效果则从行动顺序上在触发[反击]时点的玩家的下一位玩家开始依次决定是否使用[反击]效果。」
* **Trigger:** a [反击] window opens and two or more seats hold an eligible
  counteraction to the same timing.
* **Condition:** the live prompt's ask ring (`Table::asked`). The prompt is
  **not** in `spec_kit::Step`.
* **Effect:** the seats are asked one at a time in turn order, starting from
  the player after the one who triggered the timing.
* **Frame:** n/a (read of the live prompt).
* **Rulings & OPEN:** TEST-FINDINGS §10 records a 2026-10-07 user ruling that
  starts the ring at the **initial user** instead of 「下一位」, and lets a
  seat exhaust its counteractions before priority moves on. The book text is
  what this clause states. When the trigger's own link cannot be self-answered
  (a card guard such as `通用:网络链接异常` on one's own play), the first
  *eligible* seat agrees under both readings -- see the scenarios.
* **Executable:** `rules_spec::global::counteract::check_ask_ring`
  (**scenario-tested**, not in `CLAUSES` -- the prompt is not in `Step`).
  Scenarios: `g_counter_02_ask_ring_starts_after_trigger_in_turn_order`,
  `g_counter_02_trigger_seat_not_offered_own_link`.

---

## G-COUNTER-03

* **ID:** `G-COUNTER-03`
* **Source:** `rulebook-doc.md:143`
* **Quote:** 「多个效果可[反击]同一个时点，所有玩家同意所有对一个时点的[反击]已发动后可对新的时点发动[反击]。」
* **Trigger:** an observed input whose window holds a non-negated `"card"`
  activation and one or more `"counter"` activations answering it.
* **Condition:** both the counters and X are in this step's event window.
* **Effect:** every counter body settles before X's body -- each
  `kind == "counter"` event id is less than every body child of X. The
  "new timing only after this one closes" half is scenario-tested: while the
  ask ring is still on X, the prompt's detail still names X, not a declared
  counter.
* **Frame:** n/a (read of the event window).
* **Rulings & OPEN:** LIFO among same-timing counters is an engine ruling
  (TEST-FINDINGS §10), not book text -- this clause only orders counters
  against X.
* **Executable:** `rules_spec::global::counteract::g_counter_03`.
  Scenarios: `g_counter_03_two_counters_one_timing_settle_before_x`,
  `g_counter_03_new_timing_waits_for_the_round_to_close`.

---

## G-COUNTER-04

* **ID:** `G-COUNTER-04`
* **Source:** `rulebook-doc.md:142`
* **Quote:** 「[指定]多名玩家的效果从发动的玩家开始依次对行动顺序上的被[指定]玩家进行判定。」
* **Trigger:** one card activation whose text has a `[指定]` and whose body
  applies the same effect type to two or more distinct players (a
  multi-player [指定] fan-out onto the user).
* **Condition:** the children of that activation, in event order.
* **Effect:** the distinct subjects appear as a forward walk of the
  turn-order ring starting at the user. The user is first when designated.
* **Frame:** n/a (read of the event window).
* **Rulings & OPEN:** a card body that writes its own order is a 特别注意 3
  override and is out of scope. The check is narrowed to fan-outs whose
  `pay`/`rent` legs all name the user as counterparty, so a card's own
  bespoke sequence is not mistaken for a bare multi-[指定].
* **Executable:** `rules_spec::global::counteract::g_counter_04`.
  Scenarios: `g_counter_04_multi_designation_pays_in_turn_order` (exact
  numbers: 3 × 670 with 4 players),
  `g_counter_04_multi_designation_forall_money` (forall over pre-moneys).

---

## G-COUNTER-05

* **ID:** `G-COUNTER-05`
* **Source:** `rulebook-doc.md:144`
* **Quote:** 「非写明可选择的效果在可发动时必须发动。」
* **Trigger:** a non-negated `"card"` activation whose printed body is not
  written optional (no 「可选择」 / 「可以」 / 「可打出」 / 「可使用」 / a
  remaining 「可」).
* **Condition:** the activation is in this window and the card's body text
  (live sheet) is mandatory.
* **Effect:** the body ran (an event, a non-bookkeeping world delta, or a
  field placement).
* **Frame:** n/a.
* **Rulings & OPEN:** a [反击] *declaration* is itself a choice (the window
  offers play/skip); this clause is about the **body** of an activation that
  already happened. The general "every mandatory hook must fire" reading is
  not per-act-testable without a hook inventory, and a mandatory body can
  settle with **no log line of its own** (a fan-counter rewrite), so the
  general per-act face is **not** in `CLAUSES` (it false-positives on the
  soak). The determined case -- a body with two printed halves must run
  both -- is scenario-tested.
* **Executable:** `rules_spec::global::counteract::g_counter_05`
  (helper). Scenario:
  `g_counter_05_mandatory_vow_body_runs_both_halves`
  (`AG:宣战布告`'s 「[支付]500资金」 and 「抽1张卡」 both run).

---

## G-COUNTER-06

* **ID:** `G-COUNTER-06`
* **Source:** `rulebook-doc.md:145`
* **Quote:** 「对乐队或角色生效的效果根据玩家当前拥有的乐队卡和角色卡进行判定且玩家执行效果。」
* **Trigger:** an effect that names a band or a character as its subject
  filter (e.g. 「非Pastel✽Palettes角色获得…」).
* **Condition:** the player's **currently held** 乐队卡 / 角色卡 (the
  identity cards dealt at setup).
* **Effect:** the effect applies to (and is executed by) exactly the players
  whose held cards match. A hook's `move` body does not move another seat.
* **Frame:** n/a.
* **Rulings & OPEN:** OPEN-乐队卡 -- the book never defines 乐队卡 as a card
  *class*. The determined reading is the identity card each player holds
  (setup: 「拿取选择角色的角色卡和对应的乐队卡」). Which players match is
  scenario-tested (丸山彩 (1)'s 「非Pastel✽Palettes玩家」 fan-out).
* **Executable:** `rules_spec::global::counteract::g_counter_06`.
  Scenario: `g_counter_06_band_filter_uses_held_band_cards`.

---

## G-COUNTER-07

* **ID:** `G-COUNTER-07`
* **Source:** `rulebook-doc.md:51`
* **Quote:** 「技能和卡的效果优先于规则书中的所有规则。如果技能或卡的效果与规则书有冲突则执行技能或卡上的效果。」
* **Trigger:** any transition where a printed card / skill body conflicts
  with a base rule.
* **Condition:** the card / skill text names an outcome the base rules would
  not produce.
* **Effect:** the printed outcome is what lands.
* **Frame:** n/a.
* **Rulings & OPEN:** meta-rule; no per-act predicate can see "conflict with
  the rulebook". Not in `CLAUSES`.
* **Executable:** scenario `g_counter_07_card_overrides_base_payment`
  (`CRYCHIC:主唱太拼命了` cancels a 6680 rent the base rules would complete).

---

## G-COUNTER-08

* **ID:** `G-COUNTER-08`
* **Source:** `rulebook-doc.md:79`
* **Quote:** 「[反击]：带有“[反击]X：Y”效果在X发生时打出并触发效果Y…」
* **Trigger:** a `"card"` activation with `kind == "counter"`.
* **Condition:** the activation is in this window.
* **Effect:** the declarer (`player_id`) is a seat of the match. The X half
  (what it answers) is read off the live prompt's
  `ask.counteract.detail.*` and is scenario-tested.
* **Frame:** n/a.
* **Rulings & OPEN:** none.
* **Executable:** `rules_spec::global::counteract::g_counter_08`.

---

## G-COUNTER-09

* **ID:** `G-COUNTER-09`
* **Source:** `rulebook-doc.md:73`
* **Quote:** 「[指定]：指定技能或卡的效果进行效果结算的目标。」
* **Trigger:** an activation whose text has a `[指定]`.
* **Condition:** the activation's body children.
* **Effect:** the children's subjects are real seats (the designated
  targets). The set itself is the card's own choice.
* **Frame:** n/a.
* **Rulings & OPEN:** multi-[指定] *order* is `G-COUNTER-04`.
* **Executable:** `rules_spec::global::counteract::g_counter_09`.

---

## G-COUNTER-10

* **ID:** `G-COUNTER-10`
* **Source:** `rulebook-doc.md:71`
* **Quote:** 「[使用者]：发动卡的玩家。」
* **Trigger:** a hand play or skill press (`StepAct::Act` of `play`/`skill`)
  that produced a `"card"` activation.
* **Condition:** the act is `play` / `skill` and the window holds a
  non-negated activation of that card.
* **Effect:** the activation's `player_id` is the seat of the act
  (`Step::player_of_act`).
* **Frame:** n/a.
* **Rulings & OPEN:** a [反击] declaration's user is the declarer
  (`G-COUNTER-08`); the act there is an Answer, not a play.
* **Executable:** `rules_spec::global::counteract::g_counter_10`.
  Scenario: `g_counter_10_and_15_hand_play_user_and_body`.

---

## G-COUNTER-11

* **ID:** `G-COUNTER-11`
* **Source:** `rulebook-doc.md:72`
* **Quote:** 「[拥有者]：[场地]存在此卡的玩家。」
* **Trigger:** a `"card"` hook (`kind == "hook"`) of a placed `[持续]` card.
* **Condition:** the card is on some player's field in `pre` or `post`.
* **Effect:** the hook's `player_id` is that owner.
* **Frame:** n/a.
* **Rulings & OPEN:** board-owned instances (an event card on the board) have
  no player [场地] and are skipped. A mid-window place/remove is
  undetermined from the snapshot alone and is skipped.
* **Executable:** `rules_spec::global::counteract::g_counter_11`.

---

## G-COUNTER-12

* **ID:** `G-COUNTER-12`
* **Source:** `rulebook-doc.md:80`
* **Quote:** 「[持续]：卡在任何玩家的[场地]或格子上时持续生效的效果。」
* **Trigger:** a `"card"` activation with `kind == "hook"` whose printed text
  carries `[持续]`.
* **Condition:** the card is on some player's field (or the board) in `pre`
  or `post`, or a place event in the window.
* **Effect:** a [持续] effect cannot fire for a card that is not placed.
* **Frame:** n/a.
* **Rulings & OPEN:** 「[场地]」 and 「格子上」 are both homes; the engine's
  `FieldCard` / `board_field` split is the observability. The engine also
  tags some non-[持续] listeners `kind == "hook"`; those are out of scope.
* **Executable:** `rules_spec::global::counteract::g_counter_12`.

---

## G-COUNTER-13

* **ID:** `G-COUNTER-13`
* **Source:** `rulebook-doc.md:81`
* **Quote:** 「[特]：卡在特殊情况下生效的效果（例：卡在游戏开始前生效的效果）。」
* **Trigger:** n/a -- **OPEN**.
* **Condition:** n/a.
* **Effect:** n/a.
* **Frame:** n/a.
* **Rulings & OPEN:** OPEN-特 -- 「特殊情况」 is undefined in the book (only
  the parenthetical 「例：卡在游戏开始前生效的效果」 is given). No predicate
  can be written without inventing the set of special situations. Cards that
  print their own [特] window are covered by the card unit.
* **Executable:** none (doc-only).

---

## G-COUNTER-14

* **ID:** `G-COUNTER-14`
* **Source:** `rulebook-doc.md:82`
* **Quote:** 「[限]：带有“[限]X”的卡只有在X成立时才可从手中打出。」
* **Trigger:** a hand play of a card whose text carries a `[限]` / `【限】`
  play gate.
* **Condition:** the play is observed; the gate text is read from the live
  sheet.
* **Effect:** a play of a [限] card either produces no activation (the gate
  refused it) or produces its play activation (the gate held). The gate's
  own truth value is scenario-tested.
* **Frame:** n/a.
* **Rulings & OPEN:** some sheet texts reuse the `[限]` marker for a
  continuous rider (e.g. `AG:即使夕阳落山`'s 「第100轮次开始前…」) rather
  than a play gate. Those are not 「只有在X成立时才可从手中打出」 and are out
  of scope. The *timing* of hand plays (运营 only) belongs to G-CARDS.
* **Executable:** `rules_spec::global::counteract::g_counter_14`.
  Scenario: `g_counter_14_limit_gate_refuses_off_gate_play`
  (`MyGO:[千早爱音]Anon Tokyo`, X = 「自己所在的格子是[可购买格子]」).

---

## G-COUNTER-15

* **ID:** `G-COUNTER-15`
* **Source:** `rulebook-doc.md:78`
* **Quote:** 「[手]：卡从手中打出所触发的效果。」
* **Trigger:** a hand play of a card whose text has a `[手]` / `【手】`
  section.
* **Condition:** the play produced a non-negated `"card"` activation of that
  card with `kind == "play"`.
* **Effect:** the [手] body ran (an event, a non-bookkeeping world delta, or
  a field placement). A negated play is `G-COUNTER-01`. A window that stopped
  on a prompt (`post_prompt_open`) is still mid-body and is skipped.
* **Frame:** n/a.
* **Rulings & OPEN:** the *timing* of [手] (「非[反击]的[手]效果只能在运营
  阶段使用」, `rulebook-doc.md:128`) belongs to G-CARDS; this clause only
  defines what [手] *is*.
* **Executable:** `rules_spec::global::counteract::g_counter_15`.
  Scenario: `g_counter_10_and_15_hand_play_user_and_body`.

---

## Open questions

* **OPEN-乐队卡** (already in the README index) -- 乐队卡 is undefined as a
  card class. G-COUNTER-06 only specifies the determined reading (the
  identity card each player holds at setup).
* **OPEN-特** -- 「特殊情况」 (`rulebook-doc.md:81`) is undefined. G-COUNTER-13
  is doc-only. The parenthetical 「例：卡在游戏开始前生效的效果」 is the only
  anchor.
* **OPEN-自我反击** -- may a player counteract their own link? The book does
  not forbid it (TEST-FINDINGS §10). Under `通用:网络链接异常` the engine does
  not offer the trigger seat its own [手] play; whether that is a card guard
  or a general engine rule is undetermined from the sheet text alone. The
  ask-ring start (G-COUNTER-02) cannot be distinguished from the §10
  ruling's "initial user first" while the trigger seat is not able.

## Findings

(None -- every scenario that follows from a quote is green. G-COUNTER-02's
book start seat and the §10 ruling's start seat are **not** distinguishable
with the real cards this unit can drive; see OPEN-自我反击. A failing test on
that distinction needs a counteraction whose text allows answering one's own
link.)
