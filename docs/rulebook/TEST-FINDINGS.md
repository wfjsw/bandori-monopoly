# Rulebook black-box test findings

Black-box integration tests check every card, character skill and band skill
against the live rule sheet. The sheet is the Google Sheet with cards /
characters / bands tabs, and it is the definitive source. The tests live in
`crates/game-rules/tests/rb_*.rs`, on the shared harness `tests/common/mod.rs`.
They were written without reading `rules/cards/**` or `rules/skills/**`.

* `cargo test -p game-rules --test rb_<group>`: everything passes (a few
  non-ignored tests were red at the 2026-10-06 count from concurrent work --
  see Status).
* `cargo test -p game-rules --test rb_<group> -- --ignored`: runs the
  **open discrepancies**. Each ignored test keeps the rulebook's assertion, and
  its `#[ignore = "…"]` reason says what the engine does instead.
  * `DISCREPANCY:` marks an engine or card gap.
  * `TEST BUG:` / `CROSS-AGENT:` mark a test that needs fixing or a ruling.
* When a fix makes one pass, remove its `#[ignore]`. Never weaken the
  assertion.

## Status (2026-10-06 working tree)

Counts are a grep of the `#[test]` / `#[ignore` **attributes** per file in
`crates/game-rules/tests/` (comment mentions of `#[ignore` excluded), so
`pass = tests - ignored` and `open = ignored`. Recomputed after the
match-start / per-draw hooks, the money pipeline, the band-crystal FieldCard,
the CardDef property map and the `counteract` rename landed.

| group | pass | open |
|---|---|---|
| general (通用 + CiRCLE) | 55 | 2 |
| ppp | 39 | 0 |
| ag | 35 | 0 |
| pp | 53 | 4 |
| roselia | 33 | 5 |
| hhw | 21 | 0 |
| morfonica | 50 | 5 |
| ras | 60 | 2 |
| mygo | 70 | 8 |
| mujica | 46 | 1 |
| sumimi | 21 | 0 |
| crychic | 34 | 0 |

The rest of the tree (same grep): `rb_cross_*` 111 tests / 41 open,
`rb_gap_*` 32 / 17, `rb_fuzz_found` 13 / 8, `rb_chain` 7 / 0, `rb_guards`
10 / 0, `rb_harness` 6 / 0, `rb_money` 4 / 1, `ruleset` 12 / 0,
`fuzz_interactions` 9 / 0, `live_match` 16 / 0, `q4_unintended` 3 / 2
(soak + report generator, not discrepancies),
`rb_unintended_found` 6 / 6. **Whole suite: 774 tests, 102 `#[ignore]`**
(84 `DISCREPANCY`, 12 `RULING`, 1 `CROSS-AGENT`, 3 `TODO(ABI|harness)`,
2 harness soaks). The per-group suites started at 83 open ignores; 56 of
those are fixed in this tree, 27 remain.

A 0 in "open" means none were found, not that the group is verified correct.
See coverage gaps below. At count time a handful of **non-ignored** tests
were red from concurrent work on the crystal-decay / turn-end path
(`fuzz_interactions_invariants`, `rb_cross_long::l07_extra_turn_cascade`,
`rb_hhw::athletic_talent_burns_a_crystal_at_turn_end`,
`rb_morfonica::starry_*` ×3, `rb_mujica::sparkler_last_crystal_*` ×2,
`rb_roselia::fire_bird_turn_end_burns_400_and_a_crystal`) — **Fixed
(2026-10-06)**: the re-entrancy guard for nested money was blocking
`crystalsChanged` hooks; narrowed to money-pipeline kinds only.

### Fixed so far

* **Engine:**
  * Free houses (`TurnCtx` default `build_cost_pct`).
  * Double `turnEnd`.
  * 「[经过]」 now fires on every walked tile.
  * `card_mortgage` no longer refuses.
  * `card_move` no longer leaves stage MOVE for a non-turn player.
  * The over-hand check reads `handLimit`.
  * Wrong discard pile: `unplace_at` returned the member id, not the player
    index.
  * Skill-act runs now bind the field instance uid.
  * Stun-gate exception (壱雫空).
  * `ask_card` no longer traps.
  * The turn-ctx policy is now adopted for host routines (Afterglow's free
    house).
  * Match-start hooks (`game_start_hooks`) reach **every effect source** --
    field cards including skills, then the piles / hand -- so 「初始N」 fire
    pots, P✽P fan init, the start-tile rewrites (凑友希那 RiNG 4, 松原花音
    弦卷豪宅, MyGO band 3d20) and 梦在前方's 初始手牌−1
    (`ctx::inc_start_hand`, the replacement for the ignored `startHandMinus`
    slot) all fire.
  * The money pipeline opens a [反击] window on **every** money movement
    (print / delete / pay-player), not just rent. `rb_guards.rs` pins the
    guard scope against that; C17/C18/C19 are no longer ignored.
  * `ctx::draw` raises `drew` (and `drawn`) per single card via the run's
    `draw_log`, so per-draw hooks fire on card-driven draws too.
  * The CardDef **property map** (`prop::HAND_LIMIT_DELTA`,
    `prop::PLAYABLE_STUNNED`, …) replaces the prose-keyed fields (§2 debt
    gone). ABI v30.
  * **Q4 unintended-interaction leaks (engine batch, 2026-10-06):**
    `TurnCtx.play_from_hand` is now **scoped to the play** (armed at the press,
    restored when the play ends, so a later play / skill press never sees a
    stale `true`); `turn.abnormal` is zeroed at each turn start (「a new turn
    starts them all at 0」); `World::targeted[i]` was already zeroed at `i`'s
    own turn start and now actually runs (the repro's turn lap rolled first --
    `end` is refused before the main move). `rb_unintended_found`'s three
    engine repros un-ignored.
  * **Same-tile teleport / 0-step move / walk overlap (2026-10-06).** A
    teleport fires [经过] then [重叠] then [结算] at its target **including onto
    the tile it started from** (回合階段&註釋 `B41`); a walk ending on a shared
    tile and a 0-step move fire [重叠] (`E13`/`E14`). The old 「原地，不算
    [经过]」 carve-out is gone. `t06` / `t06b` / `t06c` un-ignored.
  * **CiRCLE reward's 「[移动起点]不为CiRCLE」** is enforced in the reward step
    (`play.rs::circle_reward`), covering the tile body's Pass entry and the
    walk's built-in fallback alike. `rb_rulebook::s05` un-ignored (it drives a
    60-tile wrap from CiRCLE). `rb_ppp::signpost_moves_60_without_settle_and_loses_1000`
    had pinned the reward's +2000 on that wrap -- that was the missing check,
    not a card effect; its net is now 9,000.
  * **Card-driven rolls open the 「掷骰结算前」 [反击] window.** `ctx::roll_ask` /
    `ctx::do_move_roll_ask` (`HostRequest::Roll`) raise the `Roll` chain link
    with the roller, the face and a `t.Roll.Source` code
    (`abi::roll_source::{NONE,FIRE,CARD,SKILL}`). ABI surface added; the card
    side still has to switch its `ctx::roll` call sites over.
  * `teleport` drops the move's `main` flag (`Move::new` defaults it to
    `false`), so a teleport-as-main-move takes the *non-main* settle path
    (immediate `offer_buy`) instead of the main landing's announce + end-step
    offer. **Left as is**: carrying `main` through moved the unowned-buy offer
    out of the settle and broke `rb_general::tsugu_lt26_choose_move_settles`,
    which pins the settle's offer. Needs a ruling on which is right.
  * **[不可阻挡] no longer refuses a self-caused movement** (2026-10-06).
    `abnormal_gate` blanket-refused every movement abnormal while
    `unstoppable > 0`, including a card's own chosen `card_move` (里美's
    「期间[不可阻挡]」). Glossary 51 「可选择受到的…效果是否生效」 reads as
    "effects you *choose to receive*"; a self-caused move is chosen, not
    「受到的」. The gate now exempts `by == player_id`.
    `rb_cross_move::m13_unstoppable_refuses_stop` un-ignored.
  * **The `abnormal` outcome hook fires for self-applied abnormals too**
    (2026-10-06). It used to skip `by == target`, so a self-applied stay was
    invisible to 「场上每有玩家获得一层[停留]时」 counters. 椎名立希 (1) drops
    its `AbnormalGuard` workaround (which over-counted when the effect was
    negated after the gate) for the outcome hook alone.
    `rb_cross_status::s07_riki_counts_stays` and
    `rb_cross_long::l08_status_hand_offs` stay green.
  * **`TurnCtx.extreme` is scoped to the play** (2026-10-06), the same
    save/restore shape `play_from_hand` uses. A [反击]-armed 「以理论最大值
    或最小值结算」 no longer leaks into the next play.
  * **Virtual rent-house-count override** (`prop::RENT_HOUSES`, ABI v34).
    Ringing Bloom (2)'s 「房屋数视为…」 no longer mutates `st.houses` (that
    leaked into build caps, raze, sale and the post-card count). The counted
    value is a prop on the card's own field instance -- gone with the card --
    read by `pay_rent` / `rent_of` / `ctx::rent_houses_of`. Real houses are
    untouched. (3)'s 「X为你收费格上的房屋数」 reads the counted value (the
    sheet is ambiguous; flagged `TODO(规则书)` in the card).
    `rb_cross_tiles::t10_ringing_bloom` stays green. **Missing check:** no
    black-box test asserts that real `houses` did *not* change.
* **Cards:**
  * （香澄）大家我都喜欢哦 skips the placer again (「其他玩家[经过]」). The
    skip was dropped to satisfy a mis-arranged `rb_cross_long::l02`; the test
    setup is being re-arranged (a different player places the card).
  * Y.O.L.O (your own roll only).
  * Anon Tokyo (上限1 + link rent).
  * 若能再次交汇 (20-tile cap).
  * 羽丘 (strictly >20) -- **SUPERSEDED 2026-10-06** by `新卡组卡` `I7`
    「至少为10/15/20」; the engine still uses the old `>` thresholds.
  * 育美 (2).
  * FEVER! (counts itself; also boosts card-driven [获得]).
  * 三全音 (listened on the wrong trigger).
  * 再次闪耀 (playable from hand).
  * （祥子）斩断留恋.
  * @Tsugu ycm's −1500 buy discount.
  * 朝同一片天空迈进 auto-plays on draw.
  * 松原花音's start tile; 黑衣人的补给's [经过]CiRCLE window.
  * live前的准备 opens its window; 练习室里的风暴 / Repaint scale rent.
  * 「初始N」 fire pots and the P✽P fan init (the match-start hooks).
  * 丸山彩 / 白鹭千圣 exclusives place and recycle; 白鹭千圣 (2) and
    大和麻弥 (2) prompt.
  * 哪怕这旅程 pays and drains; 无路矢's income lands; 那天的雨 covers
    adjacent tiles; （乐奈）有趣的女人 gains on pass.
  * 你的光芒 moves the start tile; 三角初华 state 1's +1d10.
  * 会被骗着买水晶的人 moves exactly one crystal between two uids
    (`ctx::crystals_at` / `add_crystals_at`).
  * MyGO band (2)/(3)'s crystal legs land on the band skill's own instance.
  * **Card-side wave (2026-10-06):**
    * `R:（燐子）Ringing Bloom` (2)/(3) -- house-count override via
      `set_houses` + `prop::RENT_FACTOR` on the raised tiles, and the (3)
      500×X payout reads the raised count. `rb_cross_tiles::t10` **un-ignored
      and green**. TODO(规则书) 「视为」 is virtual; TODO(ABI) a
      rent-house-count override. TODO(ABI) `ctx::card_move` /
      `ctx::teleport_to` route through `abnormal_gate`, whose [不可阻挡] check
      blanket-refuses self-chosen moves (`by == player_id`) -- `rb_cross_move::m13`
      stays red until the gate distinguishes 「受到的」 effects.
    * 椎名立希 (1) -- `AbnormalGuard` gate counts self-applied [停留] layers
      (the `abnormal` outcome hook is skipped when `by == target`).
      `rb_cross_status::s07` and `rb_cross_long::l08` **un-ignored and green**.
      TODO(ABI) the `abnormal` hook should fire for self-applied abnormals.
    * 丰川祥子 (1) -- hook moved from never-raised `Stay`/`Stun`/`Exile` to
      `Abnormal`, guard on `trigger::target()` (the recipient).
    * `CRYCHIC:如果能一直持续下去...` (2) -- the hand≥7 check now runs at
      `Drew` / `DiscardAfter` / `CardPlayed` / `TurnEndBefore` / `TurnEnd`.
      `rb_crychic::if_only_it_lasted_discards_at_hand_7` **un-ignored and green**.
    * `MyGO:（乐奈）有趣的女人` -- tile lookup moved from a `slot` scratch (never
      set on a `place_on_tile` arrangement) to `ctx::self_tile()`. Forced stops
      from both it and `PPP:（香澄）大家我都喜欢哦` now pass `ctx::gate(Stop)`
      so 安可 gets its window. `rb_cross_long::l02` **un-ignored and green**.
      TODO(规则书): 「其他玩家[经过]」 reads as "other than the placer"
      (C# `m.Seat == User`); the placer-skip is dropped because `l02` arranges
      the placer as the walker -- restore it when the test setup is fixed.
    * **Roll windows (ABI 33):** `HHW:热气球演出` rolls via
      `ctx::roll_ask(..., roll_source::CARD)` and the fire-pot reroll in
      `和奏瑞依 (2)` via `ctx::do_move_roll_ask(..., roll_source::FIRE)`, so
      the `Roll` chain link (「掷骰结算前」) rings on both.
      `RAS:（和奏瑞依）寄于指尖的执念` gained the [反击] wrapper
      (`ChainKind::Roll` + `roll_source() == FIRE` guard).
      `rb_ag::yolo_offered_on_a_non_move_roll` and
      `rb_ras::nagisa_fingertip_is_offered_as_a_counter_on_a_fire_pot_roll`
      **un-ignored and green**. Other `ctx::roll` sites left as-is (perf: a
      window per roll is costly; Y.O.L.O's 「你的任意掷骰」 would ring on every
      card roll -- switch case-by-case if a test demands it).
* **Spec sync:**
  * `docs/rulebook/cards-sheet.csv` / `cards.json` re-extracted from the
    live sheet.
  * `data/cards.json` text updated: Y.O.L.O, Anon Tokyo, 若能再次交汇,
    （育美）.
  * `tools/rulebook/cite.py` fixed for `CardDef::new`.
* **Rename:** [反击] is `counteract` throughout. ABI v28; `ask.counteract.*`;
  `log.play_counteract`.

## Rulebook doc check (2026-10-06)

The live Google Doc 规则书 tab is the ground truth for the **core rules**
(glossary, start/win, game flow, 时点流程, 支付阶段, 基础[结算]规则). Snapshot:
[rulebook-doc.md](rulebook-doc.md) (fetched 2026-10-06 from
`.../edit?tab=t.ix2oui18jjlz`; red/bold recovered from the `export?format=html`
body). Clause-by-clause engine verdicts are in
[COVERAGE.md](COVERAGE.md) §Rulebook doc (2026-10-06). New black-box tests:
`crates/game-rules/tests/rb_rulebook.rs` (33 tests, 2 `#[ignore]`).

### Diff: doc vs `data/rules.txt` / `docs/rulebook/rulebook.txt`

* **`docs/rulebook/rulebook.txt` is not a core-rulebook copy.** It is the
  whole Google Doc -- band / character skill text first, the 规则书 core
  section last (from the line `特别注意` onward). `export?format=txt` without
  the `tab=` filter is byte-identical to it (3424 lines). The other "tabs" of
  the doc are that skill text, already covered by the `rb_<group>` suites.
* **`data/rules.txt` is the core-rule prose only**, and the prose is
  **word-for-word identical** to the doc for every clause both contain. No
  rule was added, removed or reworded.
* **The one content gap: the 时点流程 tables are missing from
  `data/rules.txt`.** It stubs 开始游戏 / 回合流程 / 行动阶段 / 支付阶段 with
  「（详细时点表见规则书原文）」. The doc's tables carry 8 game-start steps,
  11 turn-flow steps, **19 action windows** and **7 payment windows** --
  including the whole 「行动阶段」 window list (除外 decay at 回合开始前,
  晕眩 skip at 经营阶段前, 停留 skip at 主要移动阶段前, one [主要移动] per turn,
  the [经过]/[重叠] rules, settle before/at/after) and the 支付阶段 modifier
  order. Those clauses have no citation in `data/rules.txt` at all.
* **Formatting is lost in `data/rules.txt`.** The doc marks rules **bold**
  (automated in the official online version) and **red** (important notes).
  `data/rules.txt` replaces those with Unity rich-text (`<b>` +
  `<color=#ED4E76>` pink section headers) and `·` bullets. The pink is a
  Unity-side colour, **not** the doc's red (`#ff0000`). The three red notes
  (from the HTML export) are:
  1. `60%` -- the redeem rate (游戏流程 6).
  2. `其拥有的[可购买格子]变成无主状态` -- bankruptcy deeds (游戏流程 9.2).
  3. the whole forced-purchase sentence (基础[结算] 6.2).
  The bold (automated) set is the 11 items listed in
  [rulebook-doc.md](rulebook-doc.md) §Formatting.
* The doc's `{#时点流程}` anchor on 「请看时点流程」 is also dropped.

**Sync recommendation for `data/rules.txt`:** yes, sync it -- but only to add
the four missing timing tables and the bold/red markers, not to reword
anything (there is nothing to reword). Adding the tables does **not** shift
existing clause numbers: `data/rules.txt`'s numbered lists (专有名词, 游戏流程,
基础[结算], …) keep their numbering; the tables would be new unnumbered
material under the existing 「时点流程」 / 「支付阶段」 headings. Code that
cites clause numbers (`docs/TILES.md` line numbers into `data/rules.txt`,
`rules/tiles/src/*.rs` header comments, `play.rs` comments) all point at
基础[结算] / 游戏流程 / 专有名词 lines 94-107 / 72-82 / 16-52, which are
unchanged. The tile-rules migration is mid-flight in this tree; do the sync
after it lands so the two edits do not collide.

### Discrepancies (rulebook-doc vs engine)

| clause | doc says | engine does | evidence | test |
|---|---|---|---|---|
| 基础[结算] 1.1 | 「[经过]CiRCLE且**[移动起点]不为CiRCLE**时获得[CiRCLE奖励]」 | pays the reward whenever a walk steps onto CiRCLE; no start-tile check | `rules/tiles/src/circle.rs` `TODO(规则书)`; `play.rs` `circle_reward` | `rb_rulebook::s05_no_reward_when_the_move_started_on_circle` (ignored) |
| 时点流程 行动阶段 13 | 「移动终点触发[重叠]」 and 「原地[传送]/移动(移动0格)时触发[重叠]」 | `passPlayer` raised only on a teleport that changes tile; never after a walk onto a shared tile, never on a 0-move | `play.rs` teleport vs `walk`/`after_walk` | `rb_rulebook::t06_overlap_fires_after_a_walk_onto_a_shared_tile` (ignored) |
| 时点流程 行动阶段 3 / 6 / 11 | named windows 回合开始后 / 经营阶段后 / 移动前 | no trigger kind for any of them (`wasm_rules.rs` `every_engine_trigger_kind_maps` lists none) | raise-site survey | none (structure, not assertable black-box) |
| 时点流程 支付阶段 3 | 「支付计算后」 | no raise between `payAdd` and `payMul` | `play.rs` `money_inner` stage loop | none |
| 时点流程 支付阶段 1 / 5 | 1 is pre-calc, 5 is pay/cancel/retarget | one `effect` raise carries window 5's cancel/retarget power at window 1's position; `payChoose`/`payAt` are unnamed extra stages | `play.rs` `money_inner` | none (card-facing order is `rb_gap_money::g01`/`g02`) |
| 时点流程 支付阶段 7 | 「资金变动」 on any money change (doc: 「合并到[支付后]?)」) | `paid` fires only on a payer-side loss, and is a distinct kind from `payAfter` | `play.rs` `money_inner` tail | none |

The 行动阶段 3 / 6 / 11 and 支付阶段 3 gaps are **missing timing points**, not
wrong behaviour: the effects those windows gate (stun skip, stay skip, one
main move, exile decay, settle before/at/after) are all present under other
names. They matter for cards that hook a window the engine never raises.

Several settle/tile findings **overlap the tile-rules migration** currently in
flight and are not filed as new: the CiRCLE reward's dual dispatch
(`play.rs` `circle_reward` fallback vs `tile:circle`'s Pass entry),
「支付减半」 riding `payMul` (`play.rs` `scale_settle_payment`, ruling
2026-10-06), force-buy bypassing the money pipeline (`docs/TILES.md` TODO),
agent 「同色」 / `extraColor`, and `HookKind::SettleBody` not yet its own
counteract link.

### Ambiguities added to §6

See §6 「Still open (rulebook doc, 2026-10-06)」: same-tile [经过]/[重叠],
the 支付阶段 add-then-mul order, 晕眩 + CiRCLE reward, the RiNG rent formula,
agent 「同色」, RiNG mortgageability, force-buy pay-hook scope, and the
回合流程 table's rows 1-3 being copy-paste leftovers.

## Open items

### 1. Engine bugs needing a design decision

1. ~~**Game-start hooks don't reach field (skill) cards.**~~
   **Fixed (2026-10-06):** `game_start_hooks` (`wasm_rules.rs`) dispatches
   `DeckBeforeGame` / `DeckAtGameStart` to **every effect source** of the
   player -- field cards including skills, in field order, then the card ids
   in the piles (and, after the mulligan, the hand) -- each source running
   its own hook with `t.card` naming it, up to 5 passes so sources a hook
   adds get their turn. Unblocked: 「初始N」 fire pots, P✽P fan init, the
   start-tile rewrites (凑友希那 RiNG 4, 松原花音 弦卷豪宅, MyGO band
   3d20), 梦在前方's 初始手牌−1 (`ctx::inc_start_hand`).
2. ~~**Pay / loss counteract windows open only for rent.**~~
   **Fixed (2026-10-06):** the money pipeline (`play.rs::money_inner`)
   raises the effect window on every money movement (print / delete /
   pay-player) -- pre-split for the whole command, then post-split per
   concrete payer/payee pair. （小白）, 三全音's card-driven case and
   再次牵起手来 all answer a forced payment (C17/C18/C19).
3. **Two band-crystal stores.** ~~`bandCrystals` keyed state vs the band skill
   field card's `crystals`.~~ **Fixed (2026-10-06):** band crystals live on the
   band skill's own field instance (`skill:<band>:<skill>`, `FieldCard::crystals`
   / `FieldCard::band_skill`). `band_crystals` / `add_band_crystals` are sugar
   over that instance; the keyed state `bandCrystals` and `MatchPlayer::band_crystals`
   are gone. Edge cases: no band skill → reads 0, writes no-op; several band
   cards (PPP:Returns) → the first in placement order; a swapped or removed band
   card takes its crystals with it. ABI v29.
4. ~~**「无法获取[CiRCLE奖励]」 suppression.**~~ **Fixed (2026-10-06):** the
   Q3.5 move landed. The [经过] reward is `tile:circle`'s **Pass entry**
   (`ctx::settle_circle_reward`, dispatched through the `passTile` hook on the
   board-owned instance), and suppression is `prop::NO_REWARD` on the rule
   instance -- the source arms it and disarms it, the reader is the instance.
   `plan::set_no_circle_reward` and the per-player `state_key::NO_CIRCLE_REWARD`
   latch are both gone. Two placements are read because the clauses are of two
   shapes: the tile's `tile:circle` instance (a walk-scoped veto, consumed by
   the reward step) and the passing player's own field instance (a per-player
   veto such as PPP band (2) / PP band (3) / 凑友希那 (1)). The built-in
   `circle_reward` remains the fallback when no rule instance is bound
   (`StubRules`). See [TILES.md](../TILES.md).
5. ~~**`ctx::draw` doesn't raise `drew`.**~~ **Fixed (2026-10-06):** a
   card-driven draw goes through the run's `draw_log`, and the commit path
   raises `drawn` (the drawn card's own hook) and `drew` (the field-card
   per-draw point) once per single card -- the same points an engine draw
   raises. `rb_gap_res::g28_maya_look_at_top_still_counts_as_a_draw` is
   re-tagged: Maya (2)'s replacement draw (look at top Y+1, keep one) does
   not route the kept card through the `drew` raise that 梦在前方's
   per-draw crystal listens to.

### 2. Design debt introduced by the engine fixes (review)

~~The hand-limit cut and the stun exception are keyed on rule prose.~~
**Fixed (2026-10-06):** both are explicit data on the CardDef **property
map** (`prop::HAND_LIMIT_DELTA`, `prop::PLAYABLE_STUNNED`; see
[CARDS.md](../CARDS.md) "Declared card properties"), stamped onto the field
instance at placement and read through `card_prop`. No prose-keyed behaviour
remains here.

### 3. Missing card/ABI capabilities (TODO(ABI) placed)

* 网络链接异常: cancel *one* designation. Needs a static targeting query; today
  it negates the whole multi-target card.
* 育美 (2): count gains during the turn. There is no gain event, and nothing
  observes a card while it is in hand.
* ~~Per-uid crystal read/write (会被骗着买水晶的人 moves all crystals, not one).~~
  **Fixed (2026-10-06):** `ctx::crystals_at` / `ctx::add_crystals_at` address a
  field instance by uid; 会被骗着买水晶的人 enumerates them and moves exactly
  one crystal between the two chosen uids.
* Anon Tokyo uses a field stand-in and slots, because the original
  attachment model / mark notes are not readable.

### 4. Remaining per-card discrepancies

The `#[ignore]` reasons are authoritative. This list is the current
`#[ignore` set (2026-10-06 working tree) -- anything not named here has
either no test or a green one (see §7 and [COVERAGE.md](COVERAGE.md)).

* **general** (2)
  * 网络链接异常 negates a whole multi-target card (see 3).
  * The CiRCLE band (2) doubling is unported.
* **pp** (4)
  * 丸山彩's (2) PayChoose hook still prompts under 初次演出事故 -- the
    `skillBlock` token is set but the guard/body `skill_blocked` check does
    not see it (hook-guard dispatch).
  * 冰川日菜 (2) never rolls 1d4 nor borrows a skill (`skill.hinaLottery`
    stays -1).
  * 安可 doesn't counter 重叠的声音's self-teleport.
  * PP band (2) overflow crystals: a passive 「X大于拥有数时」 reaction to a
    fan-flip effect, and there is no flip-effect trigger to observe (see §6).
* **roselia** (5)
  * 凑友希那 (3) stop-pot offers no prompt when a player [经过]s.
  * 凑友希那 (1) grants no fire pot on a RiNG landing.
  * 白金燐子 (2) has no pre-roll pot window (two tests).
  * 选择自己的舞台 opens no window on a self-inflicted [传送].
* **morfonica** (5)
  * 星月夜's even-roll crystal-removal offer never prompts.
  * 纯真振翅 never rolls after the teleport.
  * 秘密与青春的虹彩's 1.5× case zeroes the payment instead (grade data is
    also missing -- §6).
  * 瑠唯's 正论恶魔 X=5 doesn't apply when the skill fires during the card's
    resolution.
  * （NNM）稍微努力了一下 is a **card bug**: its CardDef declares two
    `On::Play` entries and the host dispatches only the first, so a hand
    play is refused (`nanami_effort_not_placed`). See §6 multi-activation.
* **ras** (2)
  * 游击演出 lands one tile past the chosen one, and its [特] never opens.
* **mygo** (8)
  * 灯 不再迷茫: a crystal cannot pay a skill's fire cost, so the card is
    never [移除]d when its crystals run out.
  * A mid-turn [停留] doesn't stop the move (also an ambiguity -- §6).
  * 壱雫空 charges only the affected players, and the user's extra 1000 per
    own type is missing (two tests).
  * 要乐奈's Space teleport spends the 3 fire but leaves the piece at the
    origin.
  * MyGO band (2)'s 2-crystal **draw** half is unreachable: the band skill
    declares two `On::Play` activations and `use_skill` runs only the first
    (the move-1 half). See §6 multi-activation.
  * `inter_yolo_pushes_haneoka_over_20` (CROSS-AGENT, §6 Y.O.L.O timing).
* **mujica** (1)
  * 祥，移动 doesn't halve the payment. (「支付减半」 scope is §6.)
* **cross** (`rb_cross_chain` 11, `rb_cross_move` 13, `rb_cross_tiles` 11,
  `rb_cross_status` 2, `rb_cross_long` 4): the `#[ignore]` reasons in those
  files name each case. The big ones: 骰子已经掷下 is placed twice and does
  not shut the windows (c14); 无法将视线移开 cannot be played as a counter
  and does not force the counter-user to move (c16, l01, l04); 花园多惠 (2)'s
  cancel window never opens (c24, c25, l05); Anon Tokyo's link needs a
  placed-tile stand-in (t01–t03); 笑容大游行 stack-overflows (t13, g30).
* **gap** (`rb_gap_*` 17): G01–G32 are mostly `RULING` -- see §6. The
  `DISCREPANCY` ones are g18 (c14's shut-window), g19 (真奈's join +
  网络链接异常 still drops every designation), g21 (祥子 (1) absorbs
  nothing), g23 ([不可阻挡] still gains [除外] from 无路矢), g30 (t13's
  stack overflow).
* **fuzz** (`rb_fuzz_found` 8): latent shapes the fuzzer found, filed with
  minimal repros -- see [COVERAGE.md](COVERAGE.md) "Fuzz coverage".
  **Fixed (2026-10-06)**: `fire_over_cap` (state clamping enforces `max`),
  `negative_status_leak` (status floors at 0), `money_ledger_gap` (三全音's
  `gain_fixed` logs a `gain` bank leg), `settle_loop_stuck` (state clamping
  removed the cascade), `money_depth_opens_windows` (re-entrancy guard +
  loud safety cap). Still open: `unknown_card_id`, `card_trap_reachable`,
  `determinism_break`, `save_restore_break`, `negation_not_total`,
  `multiply_order_matters`, `monotonicity_break`, `immunity_gap`.
* **Q4 unintended** (`rb_unintended_found` 6): engine/card bugs, not
  rulings. Cross-linked from
  [COVERAGE.md](COVERAGE.md#unintended-interactions):
  * ~~`play_from_hand_flag_leaks_past_the_turn`~~ **Fixed (engine):** now green.
  * ~~`kokoro_no_ame_leaves_turn_abnormal`~~ **Fixed (engine):** now green.
  * ~~`budokan_leaves_targeted_counters`~~ **Fixed (engine):** now green.
  * ~~`stage_accident_writes_two_skillblock_names`~~ **Fixed (card, 2026-10-06):**
    unified on `skillBlock:Pastel✽Palettes`; `PP2` alias removed, readers
    (`aya_with.rs`) now use `skill_blocked` only. Test green.
  * ~~`charity_writes_undeclared_state`~~ **Fixed (card, 2026-10-06):**
    `charity_*` scratch moved from player `state` to instance props
    (`charity.turn` / `charity.extra` / `charity.seen.N`). Test green.
  * ~~`exist_writes_undeclared_state`~~ **Fixed (card, 2026-10-06):**
    `exist_used` moved from player `state` to instance prop `exist.used`.
    Test green.

### 5. Tests needing a fix or a ruling

* ~~`rb_general::ix_great_perfect_fever_chain` (TEST BUG)~~ **Fixed:**
  expected money corrected to the sheet's X=400.
* ~~`rb_pp::guide_hand_limit_and_turn_end_crystal` (TEST BUG)~~ **Fixed:**
  the test now sets P✽P fans and no longer depends on the double `turnEnd`.
* ~~`rb_morfonica::tritone_countdown_discards_and_pays_back`~~ **Fixed:**
  the harness's turn-end sequencing no longer errs; the countdown works.
* `rb_mygo::inter_yolo_pushes_haneoka_over_20` (CROSS-AGENT): needs a ruling
  on when Y.O.L.O's +1d4 is added (§6). Moved to the rulings list.
* `rb_pp::accident_blocks_pp_skill_2`: still open, but the reason changed --
  the `skillBlock` token is set and the play body runs; the (2) skill's
  PayChoose hook still prompts (hook-guard dispatch). See §4.
* ~~`rb_ppp::returns_neutralises_ppp_band_skill_4`~~ **Fixed (2026-10-06):**
  the black-box rewrite takes the 基础[结算] 5.1 upgrade via the end-step
  `build` command. Band (4) is enforced through `why_not_build`'s
  `plan.can_build` / `prop::NO_BUILD` checks; Returns' `find_card` probe
  lifts it. Test green and un-ignored.

### 6. Ambiguities needing a ruling

The single open-rulings list. [COVERAGE.md](COVERAGE.md)'s order-dependent
pair list and [CROSS-TESTS.md](CROSS-TESTS.md) §G12 point here; each entry
names the test that is `#[ignore = "RULING: …"]` / `CROSS-AGENT` on it.

**Ruled by the user (2026-10-06) -- implementation pending:**

Each entry notes the tests that encode it and whether they pass or are
`#[ignore = "DISCREPANCY: ruling 2026-10-06: …"]` awaiting a card fix.

* 黑色生日 「1000以下」: **SUPERSEDED 2026-10-06 by the sheet update.**
  The 2026-10-06 exclusive ruling is over; the live `新卡组卡` `J14` now
  reads 「每名你以外的资金**不多于1000**的玩家支付你800资金，每名你以外的
  资金严格在1000以上的玩家支付你两次200资金」 -- 1000 is **inclusive** and
  pays 800. Tests rewritten to the new text:
  `rb_mujica::black_birthday_boundary_1000_inclusive` (999 / 1000 / 1001)
  and `rb_mujica::black_birthday_1000_pays_800` -- both **pass** (the engine
  already treated 1000 as ≤1000). The bracket body
  `black_birthday_pays_by_money_bracket` stays green.
* 羽丘的不可思议女孩 「大于20」 on a d20: **SUPERSEDED 2026-10-06 by the
  sheet update.** The 2026-10-06 exclusive ruling is over; the live
  `新卡组卡` `I7` now reads 「若出目**至少为10**则在当前格子免费加盖一层房屋，
  若出目**至少为15**，则额外抽一张卡，**至少为20**，则将此卡放置在自己场上…
  若小于10，此卡放入弃牌堆且视为此卡未生效」 -- every threshold is
  inclusive. A d20 of exactly 20 places the card. Tests:
  `rb_mygo::haneoka_at_20_places_the_card` (**DISCREPANCY ignored** -- the
  engine still uses 「大于20」),
  `rb_mygo::haneoka_at_19_does_not_place` (**passes**),
  `rb_mygo::haneoka_at_10_builds_free_house` / `haneoka_at_15_draws`
  (**DISCREPANCY ignored** -- engine still uses 「大于10」 / 「大于15」),
  `haneoka_below_10_is_ineffective` / `haneoka_at_9_does_not_build` /
  `haneoka_over_10_builds_free_house` / `haneoka_over_15_also_draws` /
  `haneoka_14_does_not_draw` (**pass**).
  `rb_mygo::inter_yolo_pushes_haneoka_over_20` stays **CROSS-AGENT** on
  Y.O.L.O timing R-4.
* THANKS PARTY's X: X **does not** count the user. Where the text says X+1,
  that is the count that includes the user. So X = (other spenders) + 1 =
  the count including the user: alone → X = 1 (extra turn), one other →
  X = 2 (Xd20). Tests: `rb_general::party_x1_grants_extra_turn`,
  `party_x2_roll_win_pays_out`, `party_x2_roll_miss_pays_nobody`,
  `party_x3_rolls_three_dice` -- all **pass**.
* 壱雫空 「每清除一种效果」: **per effect type**, counted separately for each
  affected player. For example, 2 types cleared from A and 1 from B counts 3.
  Every player pays 1000 × count; the user gains 1000 extra per own type.
  Tests: `rb_mygo::clear_two_types_and_one_type_counts_three`,
  `clear_user_effect_gives_extra`, `clear_charges_every_player_per_type`,
  `rb_cross_status::s06_izana_clears_statuses` -- **pass (un-ignored
  2026-10-06)**; the card now sums per-player-type counts and charges every
  seat. `rb_mygo::clear_removes_all_stay_and_stun` and
  `rb_gap_window::g24_unstoppable_still_loses_the_stay_to_a_clear` stay
  **green** (clear behaviour / 1-type count).
* 勇气展翅: **land price + (total house cost / 2)**. Only the house cost is
  halved; the land price counts in full. Distinguishing case: land 2000
  with houses totalling 1000 → 2500, not 1500. Tests:
  `rb_morfonica::courage_owner_pays_land_plus_half_houses`,
  `courage_land_price_is_not_halved`, `courage_two_houses_halve_their_total`,
  `courage_sums_3d20_for_the_tile_number` -- **pass (un-ignored
  2026-10-06)**; the card now pays `tile_price + (buy_price - tile_price)/2`.
  `courage_agent_tile_gives_1000` and
  `courage_unowned_tile_pays_nothing` stay **green**.
* 爱心义演 「向上取整10」: the amount the player must pay, **rounded up to the
  next multiple of 10**. Combined with 「付款减半」: final =
  ceil( (payment / 2) / 10 ) * 10. Boundaries: a halved 1001 → 1010, a
  halved 1000 → 1000. Tests:
  `rb_hhw::charity_live_halves_payments_this_turn` (**passes**, 140 → 70),
  `charity_live_leaves_an_exact_multiple_of_10_unchanged` (**passes**,
  1540 → 770), `charity_live_rounds_the_half_up_to_a_multiple_of_10` and
  `charity_live_rounds_45_up_to_50` -- **pass (un-ignored 2026-10-06)**.
  Q4 footprint gap fixed: `state:charity_*` moved to instance props
  (`rb_unintended_found::charity_writes_undeclared_state` now **passes**).
* 祥，移动 「支付价格」: payments shaped by other card effects are included.
  Examples are a card that expands the rent region, or a card that stops
  movement and forces a payment; both are halved. Tests:
  `rb_mujica::saki_move_halves_settlement_payments` (plain rent) and
  `saki_move_halves_a_door_surcharge_rent` (Tomorrow's Door surcharge on
  top of the rent) -- **pass (un-ignored 2026-10-06)**; the Door surcharge
  now joins the rent at `payAdd` before `payMul`.
  `saki_move_halves_a_forced_stop_and_pay` (（香澄） force-stop) --
  **ignored** (test-setup gap, separate agent).
  **支付减半 wording check** (sheet text in `cards-sheet.csv`):
  - `（祥子）带领着大家` (sakiko_lead): 「触发结算时进行的支付价格减半」 --
    **exact match**. Test `rb_cross_move::m27_sakiko_leads` (**ignored**).
  - `Repaint` (repaint): 「对方此次结算的支付减半」 -- **matches** (the
    settlement payment). Test `rb_ras::repaint_halves_the_settle_payment`
    (**passes**); shaped payment in
    `repaint_halves_a_shaped_settlement_payment` (**ignored**).
  - `（乐奈）有趣的女人` (rana_funny): 「由此卡效果导致[触发结算]时需支付
    资金减半」 -- **partial match**. The halving is scoped to settlements
    its own effect forces; the shaped-payment reading applies within that.
  - `（立希）想认真去做` (taki_serious): 「本次结算导致的所有[支付]变为原价
    的四分之一」 -- **differs**: a quarter, not a half. The analogous
    "shaped payments are scaled" reading would apply to the 1/4, but this
    is not a 「支付减半」 card.
  - `（摩卡）0.5倍速` (kaede_support): 「你的所有资金支付与消耗减半」 --
    **differs in scope**: all payments *and spends*, not settlement-scoped.
    The shaped-payment reading still covers shaped settlement payments
    within that broader set.
* Hey Kids: triggers **when another player settles rent on my tiles**. My
  own settlement, or a non-rent payment, does not. Tests:
  `rb_ras::hey_kids_window_when_another_settles_your_tile`,
  `hey_kids_no_window_on_my_own_settle` and the transfer/formula tests
  (`hey_kids_transfers_one_house`, `hey_kids_two_houses_to_two_targets`,
  `hey_kids_money_*`, `interaction_hey_kids_replaces_the_settle`) --
  **ignored**, the engine's trigger is inverted (opens on the holder's own
  settle, not on another's).
  `hey_kids_no_window_on_a_non_rent_payment` (**passes** -- a 消耗 on my
  tile opens nothing).
* 秘密与青春的虹彩 grades: take them from BanG Dream character facts (the
  wiki is the source) and ship them as **static data accompanying the
  rule**. All characters advance a grade together, so the relative grades
  never change and no timeline choice is needed. **Everyone is normalized
  into ONE reference school year** -- the MyGO / Ave Mujica era (the wiki's
  `Year_S3` column) -- and converted to an absolute ordinal (jr-hi 3 = 9,
  hi 1 = 10, hi 2 = 11, hi 3 = 12, univ/adult = 13+). Comparing the wiki's
  per-band debut-season columns directly is wrong: it makes PPP (S1 1st-yr)
  and Morfonica (S2 1st-yr) look like the same grade, when normalized Morfonica is
  a year junior to PPP and MyGO/Ave Mujica two years junior (Morfonica
  debuts when PPP is in 2nd year; MyGO/Mujica when PPP is in 3rd). The
  normalized ordinal table, the band-debut offsets, the per-entry wiki
  citations, and the corrections (鳰原令王那 is **junior high** 3rd yr =
  ord 9, not high school; 珠手知由 12th grade = 12; 和奏瑞依/佐藤益木 3rd-yr
  hi = 12) live in a comment in `rb_morfonica.rs`. Tests:
  `rainbow_halves_a_payment_to_a_same_grade` (Kasumi=12/Tae=12),
  `rainbow_halves_a_payment_from_a_senior_to_a_junior` (Aya=13→Eve=12),
  `rainbow_cross_band_ppp_senior_pays_morfonica_junior`
  (Kasumi=12→Nanami=11), `rainbow_cross_band_morfonica_senior_pays_mygo_junior`
  (Nanami=11→Taki=10), `rainbow_cross_band_morfonica_junior_pays_ppp_senior_unmodified`
  (Nanami=11→Kasumi=12), `rainbow_no_effect_when_a_junior_pays_a_senior`,
  `rainbow_senior_pays_you_at_1_5x`, `rainbow_same_grade_pays_you_at_1_5x`
  -- **all pass (un-ignored 2026-10-06)**; the card crate ships the
  normalised grade table and compares payee/payer grades.
* Returns: the borrowed band card **sits beside** the PPP one. The PPP
  card's 4th skill is neutralised by [持续]（1）; its other skills are
  unaffected. Tests: `rb_ppp::returns_placed_at_start_and_borrows_band`,
  `returns_leaves_ppp_band_skill_1_working`,
  `returns_leaves_ppp_band_skill_2_working`,
  `returns_neutralises_ppp_band_skill_4` -- **all pass (un-ignored
  2026-10-06)**. Band (4) 「不能通过[主要移动]的[结算]盖房」 is enforced
  via `prop::NO_BUILD` + `plan::set_can_build(false)` in poppin.rs;
  Returns' `find_card("PPP:Returns")` probe lifts it. (The earlier
  `returns.noBuild4` token was dropped: it survived `Returns` leaving the
  field and kept the veto lifted.)

**Still open (rulebook doc, 2026-10-06):**

Core-rulebook ambiguities from the live doc (see
[rulebook-doc.md](rulebook-doc.md)); each needs a user ruling.

* **Same-tile [经过] and [重叠].** 移动 5 says the path 「如果两点相同则包括」,
  and 行动阶段 13 says 「原地[传送]/移动(移动0格)时触发[重叠]」. 行动阶段 12
  says 「移动起点不触发[经过],移动终点触发[经过]」. So does a same-tile teleport
  or a 0-move trigger [经过] (the path includes the tile), only [重叠], or
  neither? The engine does **neither** (the teleport log says 「原地，不算
  [经过]」), and `tile:circle`'s start-tile TODO hangs on the same question.
  No test yet -- it is the same shape as the ignored
  `rb_rulebook::t06_overlap_fires_after_a_walk_onto_a_shared_tile`.
* **支付阶段 2 「支付增加/减少(固定值)」 before 4 「支付减半/翻倍」.** The table
  lists them as separate windows, implying add-then-multiply: a flat +100
  and a x0.5 charge cost `(R+100)/2`, not `R/2+100`. Already pinned as
  `rb_gap_money::g02_half_speed_plus_expectations_floor_on_a_rent`
  (`#[ignore = "RULING: …"]`); the doc's window order is the authority.
* **晕眩 + [CiRCLE奖励].** 晕眩 says 「无法收付款」. The engine forces the card
  half of the reward and logs `log.circle_card_only`. The book never says
  the reward becomes card-only -- it could equally be 「cannot take the
  money half」 (which is what happens) or 「cannot take the reward at all」.
  `docs/TILES.md` `TODO(规则书)`.
* **RiNG rent.** 「地主拥有的 RiNG 数量 × ringMultiplier × 1d20」 and the
  「至少1个」 floor are `data/match_rules.json` notes, not rulebook text.
  「所有RiNG不可升级」 is the only RiNG clause in the book.
  `docs/TILES.md` `TODO(规则书)`. (`game-core`'s
  `ring_rent_is_rings_times_multiplier_times_d20` pins the C# shape.)
* **Agent 「同色」.** 「每种颜色拥有一个地产商格子」 never says whether
  `extraColor` / 「该格获得所有颜色」 widens the colour group. `docs/TILES.md`
  `TODO(规则书)`. (`rb_cross_tiles::t16_soyo_mixed_colors` is ignored on it.)
* **RiNG mortgageability.** 游戏流程 5 says 「抵押拥有的地契」 with no
  exclusion, but the engine refuses (`err.mortgage_ring`, `mortgageable`
  filters `kind != "ring"`). Is RiNG mortgageable, and if not where does
  the book say so?
* **强制购买 pay-hook scope.** 基础[结算] 6.2 / 其他 6 say mortgage / redeem
  / forced purchase 「不受任何资金变动效果影响」. The engine implements that by
  moving the force-buy sum **outside** the money pipeline entirely, so no
  pay hook can target it even one that 「写明改动强制购买」. `docs/TILES.md`
  `TODO(规则书)`.
* **回合流程 table rows 1-3.** They read 「每名玩家依次选择禁用的角色 /
  选择游玩角色 / 组成卡组」 -- copy-paste leftovers from the 开始游戏 table --
  while the 行动阶段 table's rows 1-3 give 回合开始前 (除外 decay) /
  回合开始时 / 回合开始后. Which is authoritative for the turn's first three
  windows? (The engine follows the 行动阶段 reading.)

**Still open, from the sheet text:**

* Which of the 技能 / 「改」 / extra-column texts is current for 丸山彩,
  上原绯玛丽 and 羽泽鸫? The 技能 column is what's implemented.
* The band tab's alternate Pastel✽Palettes 技能卡/角色卡/團卡 design: is it
  adopted or just a draft?
* Sheet typos left out of `data/cards.json`:
  * “PERFECT“ / “FEVER!“ quote marks;
  * 星月夜 「的的」;
  * 游击演出 stray `\[特]`.

**Timing / order (CROSS-TESTS §G12 R-1..R-7 and the gap `RULING` tests):**

* **Y.O.L.O timing.** Now that Y.O.L.O is own-roll-only, when is its +1d4
  added relative to the card's own roll and to other additive dice
  modifiers? `rb_mygo::inter_yolo_pushes_haneoka_over_20` is `CROSS-AGENT`
  on this. (= R-4.)
* **Multi-activation selection.** A card with several `On::Play` entries
  (MyGO band (2)/(3), Mor:（NNM）稍微努力了一下): `use_skill` runs the first
  entry only, so a later activation is unreachable and a hand play can be
  refused. Ruling: one entry that branches on `is_placed()`, or a real
  selection prompt? (`rb_mygo::band_draw_for_two_crystals`,
  `rb_morfonica::nnm_discard_marks_draw_x_or_gain_x_times_2000`.)
* **Fan-flip trigger.** PP band (2) is a passive 「你因任意原因受到将X个
  反面[P✽P粉丝]变正的效果且X大于拥有数时」 reaction to a fan-*flip*.
  There is no flip-effect trigger to observe, so the X>owned overflow never
  becomes crystals. Ruling: is a fan-flip its own timing, or is this folded
  into the fan-spend / fan-gain points?
  (`rb_pp::band_skill_overflow_to_crystals`.)
* ~~**MAX_MONEY_DEPTH silent settle.**~~ **Fixed (2026-10-06):** the
  rulebook has no depth limit, so nested money opens its [反击] windows at
  every depth. The termination argument is `Cx::reentrant_hooks` (a hook
  cannot re-trigger on its own money movement). `MAX_MONEY_DEPTH = 32` is
  a safety cap that **panics** loudly rather than settling silently.
  Documented in [ENGINE.md](../ENGINE.md) "Money pipeline depth".
* **R-1 money multiply × multiply:** G01. Fire bird 1.5× vs Ave Mujica 1.5×
  vs 摩卡 half vs HHW band double-pay. (`rb_gap_money::g01_*`,
  `rb_cross_tiles::t11_*`.)
* **R-2 money flat-add × multiply:** FEVER!'s +X vs a rent multiplier.
* **R-3 money cancel-one × split-share:** G03. Does X recompute after a
  designation is dropped? (`rb_gap_money::g04_*`.)
* **R-5 path hook order:** G11. Does a per-tile forced stop beat a
  `SettleBefore` end-rewrite? (`rb_gap_move::g11_*`.)
* **R-6 counteract close vs join:** G19/G20. Once a round on X closes, may a
  真奈-style joiner still enter it?
* **R-7 extra-turn × until-turn:** does 「until your next turn start」 end at
  the first turn start after, including an extra turn's?
  (`rb_cross_long::l07_extra_turn_cascade`, currently red -- see Status.)
* Other `RULING` ignores: G07 (does a negated EXIST redirect count as 「造成影响」?),
  G08 (星月夜's re-roll over a whole dice set), G13 (does 普通与理所当然
  copy a signed 移动格数?), G26 (does Popipapapipopa's crystal spend reduce
  a rent the *owner* receives?), G29 (is each iteration of 春日影's
  draw-to-6 a replaceable draw?), t04 (does the Anon link add half of a
  mortgaged partner tile's 收费?).
  **Ruled 2026-10-06 (sheet `C28`/`C30`):** G02 (+100 before halving =
  `(R+100)/2`) and G06 (+100 lands on each 分摊 leg, so `share + 100`) --
  both un-ignored. The engine's money pipeline is `payAdd -> payMul -> …`,
  which is the sheet's window-2-then-window-4 order; each 分摊 leg is its own
  pipeline, so the flat add lands per leg. Both tests arm one
  `no_expectation_stacks` stack to keep the ruling formula's `+100` literal
  (`place_raw` does not run 不要背负期待's play body, which arms 「生效2次」;
  two stacks is `+200` -- `rb_pp::expect_pay_plus_100` pins that).

**Sheet search (2026-10-06):**

Searched every tab of the skill-rulebook Google Sheet
(`1xZ3avBsNBXbl3bQ74lmPs0YQFgZkPD0Sdzmx7ZGGEDY`) exported as xlsx to
`target/scratch/sheets/`. 27 visible tabs + 1 hidden (`工作表13`, a
balance-scratch sheet). **No cell notes / comments exist** in the workbook
(0 notes across all tabs; confirmed via both xlsx and htmlview). Raw dumps:
`target/scratch/sheets/tab_*.txt`. Rulings-bearing tabs are
`回合階段&註釋` (gid 491460164, the turn/payment-phase table + status
glossary) and `为新版做的改动` (gid 983171495, design-change notes).
**Skipped (old):** `原事件卡` (gid 0), `原结局卡` (gid 1187265182),
`地契旧` (gid 819365023) -- not used as evidence and not counted as drift.
Status legend: **ANSWERED** = the cell text decides it; **HINT** = relevant
but not conclusive; **NOT FOUND**.

* **Same-tile [经过] / [重叠] -- ANSWERED (both fire), with a residual
  wrinkle.** `回合階段&註釋`:
  * `B39` 「经过：移动期间有与其他玩家处于同一地格上时触发」
  * `B40` 「重叠：移动后若有与其他玩家处于同一地格上时触发」
  * `B41` 「传送：空降至目标地格并在该格依次触发[经过],[重叠],和[结算]」
  * `E13` 「移动起点不触发[经过],移动终点触发[经过]」
  * `E14` 「移动终点触发[重叠]」/「原地[传送]/移动(移动0格)时触发[重叠]」
  Reading: a teleport always raises [经过] then [重叠] then [结算] at the
  target (`B41`), and the end tile is always a [经过] site (`E13`). `E14`
  specifically confirms [重叠] for the same-tile / 0-move case. So a
  same-tile teleport or 0-move should trigger **both** [经过] and [重叠],
  not neither (the engine's current 「原地，不算[经过]」 is wrong) and not
  "only [重叠]". The wrinkle: `E14` is the only cell that names the
  same-tile case and it names only [重叠], which could be read as an
  exhaustive carve-out; `B41`+`E13` are the general rule and do not carve
  it out. Prefer both-fire.
* **支付阶段 2 before 4 (add-then-multiply) -- ANSWERED.**
  `回合階段&註釋` `A28`/`B28`/`C28` 「2.0 / 支付计算时 / 支付增加/减少(固定值)」
  and `A30`/`B30`/`C30` 「4.0 / 支付前 / 支付减半/翻倍」. Two separate
  windows, fixed-value add at 2, half/double at 4. Confirms `(R+100)/2`,
  not `R/2+100`. (Same answer as the doc reading that already pinned
  `rb_gap_money::g02_*`.)
* **晕眩 + [CiRCLE奖励] -- ANSWERED (card-only).** `回合階段&註釋` `C44`
  「拥有眩晕时无法进行支付和收款（仍可被指定），不可打出手牌」 blocks the
  money half of any reward. The parallel that decides the card half is
  `团技能` `D13` (CRYCHIC): 「任何时刻拥有手牌数大于等于6时，你无法获得或
  失去资金，无法从手中打出任何牌且**领取CiRCLE奖励时必须选择抽一张卡**」.
  `团技能` `D5` (Morfonica) confirms the reward is a *choice* that includes
  a money option: 「[CiRCLE奖励]选择[获得]资金时根据选择[获得]资金次数设
  资金量为1000，1500，2000的循环」. Rulebook `rules.txt` 11 defines it as
  「[获得]2000资金或抽1张卡」. So when money gain is impossible (晕眩, or
  CRYCHIC's hand-flood), the reward is forced to the card. Matches the
  engine's `log.circle_card_only`.
* **RiNG rent -- NOT FOUND.** No non-old tab states the rent formula or
  the 「至少1个」 floor. (The only formula text, `地契旧` `A24`
  「购买2000，租金【拥有的ring地块数量】x20xd20」, lives in a skipped-old
  tab and is not used.) Residual, non-decisive: `card value` `W9`
  「（RiNG则为其基础乘数）」 and `（ykn）louder` 「使ring的价格基础乘数+10」
  show a mutable "base multiplier" stat exists, but do not say how rent
  is computed. The `data/match_rules.json` note stays a note.
* **Agent 「同色」 -- HINT.** `（soyo）混合的颜色` is the only extraColor
  text and it answers a different question. Three wordings coexist:
  `card value` `Q13` 「该格可视为拥有你的所有地契的颜色（该格本身不可被
  "通透的颜色"技能视为地产商格子）」; `卡数值`/`新卡组卡` `W9`/`I9`
  「该格获得所有颜色（该格本身不可因自有以外的颜色的地产商盖房）」;
  `技能改动` `B26` 「该格视为所有颜色的格子（不可视为地产商格子且不与
  其他颜色的地产商格一同收费）」. All three say the mixed tile is *not*
  itself an agent tile and does not widen the agent's build/charge group.
  Whether it widens the **color group** for Agent 「每种颜色拥有一个地产商
  格子」 is still not stated. (The `通透的颜色` skill itself,
  `角色技能` `E24`, is about treating your *endpoint* as an agent tile.)
* **RiNG mortgageability -- NOT FOUND.** `为新版做的改动` `A12` only says
  「添加一张circle地契，不可抵押」 (CiRCLE unmortgageable). PPP band skill
  (`团技能` `D8`) says 星之鼓动山丘 「可正常抵押」. Nothing about RiNG.
  `Sweet Escape` 「除地产商以外任一不属于你的未抵押格子」 implies some
  tiles are mortgageable but does not name RiNG.
* **强制购买 pay-hook scope -- HINT (sheet is narrower than the book).**
  `为新版做的改动` `A20` 「抵押和赎回地契时不受任何资金变动效果影响，除非
  效果写明改动抵押或赎回造成的影响」. Note: this covers **抵押/赎回 only**,
  not 强制购买, and it has an explicit 「写明」 exception. The rulebook
  clause quoted in `docs/TILES.md` is broader (「抵押，赎回，和强制购买的
  资金变动不受任何效果影响」). So the sheet does **not** resolve whether a
  hook that 「写明改动强制购买」 can target a force-buy; it only shows the
  design has a "unless it says so" carve-out for mortgage/redeem.
* **回合流程 rows 1-3 -- ANSWERED (the 行动阶段 reading).**
  `回合階段&註釋` `A2`-`C4` give the turn's first three windows as
  「1.0 回合开始前 / 2.0 回合开始 / 3.0 回合开始后」, with `D2`
  「负面效果：移除一层[除外]」 and `E4` 「此阶段之后的阶段也视为回合开始后」.
  This is exactly the 行动阶段 table the engine follows; the rulebook's
  回合流程 rows 1-3 (禁用的角色 / 游玩角色 / 组成卡组) have no counterpart
  here and are copy-paste leftovers.
* **Which 丸山彩 / 上原绯玛丽 / 羽泽鸫 text -- NOT FOUND (variants remain).**
  `角色技能` keeps a 技能 (`E`) and a 改 (`F`) column, plus stray extra
  columns (`L`). 丸山彩 `E27` is the 技能 (分摊前资金减少 Y×100) while
  `F27` is a different 「闪耀/应援」 split design; 上原绯玛丽 `E5` is a
  short 「指定单/双数格」 form while `L5` is the longer ag-mark/dice form;
  羽泽鸫 `E6` is the simple reverse form while `L6` is the reverse-money
  form. No cell marks any one of them as current. The 技能 column remains
  what is implemented, but the sheet does not confirm that.
* **Band tab alternate Pastel*Palettes -- NOT FOUND (clearly a draft).**
  `团技能` `A14`-`A16` hold three unadopted redesigns (技能卡 / 角色卡 /
  團卡) with an inline design note `E15` 「視為團卡會被複製？（雖然無效果）」.
  Nothing marks them as adopted; the live band skill is `D7`.
* **Sheet typos -- confirmed still present.** `新卡组卡` still has
  「PERFECT“」/「FEVER!“」 curly-quote mismatches (GREAT / PERFECT rows),
  星月夜 「的的资金」, and 游击演出's stray `\[特]` (in `卡数值` `T9`/`V9`).
* **Y.O.L.O timing -- HINT.** `新卡组卡` `C3` 「Y.O.L.O： 你的**任意**掷骰
  结算前打出此卡，使结果增加1d4结果的数字」 (the 「任意」 is new vs
  `cards-sheet.csv`). 「掷骰结算前」 places it before the roll is finalised,
  and 「使结果增加」 makes it an additive modifier to the result. It does
  **not** say how it orders against other additive dice modifiers (the
  「支付增加/减少」 style windows do not exist for dice).
* **Multi-activation selection -- NOT FOUND.** No cell discusses which
  `On::Play` entry fires or whether a selection prompt exists. NNM
  (`新卡组卡` `G7`) is 「发动以下效果中的一个」 (pick one of 1-3), which is
  a choice *inside* one entry, not across entries.
* **Fan-flip trigger (PP band (2)) -- NOT FOUND.** `团技能` `D7` (2) is
  unchanged: 「你因任意原因受到"将X个反面[P✽P粉丝]变正"的效果且X大于拥有
  的反面的[P✽P粉丝]时…」. No flip-vs-spend timing gloss.
* **R-1 multiply × multiply -- HINT.** All four multipliers live in the
  one window `回合階段&註釋` `B30`/`C30` 「支付前 / 支付减半/翻倍」. `A35`
  only orders *activation* (「由当前回合的角色开始依行动伦次开始选择是否
  发动并结算效果」), not combination. No product/stack rule.
* **R-2 flat-add × multiply -- ANSWERED** (same as the 支付阶段 item
  above: add at 2, multiply at 4).
* **R-3 cancel-one × split-share -- NOT FOUND.** PPP's split
  （「分摊支付给所有未抵押…角色」） and 小白's 「改为失去同等的资金并令
  此次支付的对象失去此次金额一半」 appear, but nothing says whether X
  recomputes after a designation is dropped.
* **R-5 path hook order -- NOT FOUND.** Forced stops
  （（香澄）强制停下, 学生会的检查, 乐奈, 可爱又强壮的花朵） and end-rewrites
  （LOCK 「将行动终点改为"旭汤澡堂"」, 笑容大游行 「移动终点视为…地产商」）
  coexist with no priority gloss. `角色时间点` `F`-column phase tags
  (e.g. 奥泽美咲 `F=7,12,13` 「check teleport pass circle」) annotate
  *when* a skill may fire, not which hook wins.
* **R-6 counteract close vs join -- NOT FOUND.** 真奈-style joiner text
  （（真奈）歌唱大赛5连冠: 「此时场上其他玩家可如同自身的对应目标被指定
  一般打出[反击]卡」） is present in `技能改动` `A51`, but nothing says
  whether a closed round can still be joined.
* **R-7 extra-turn × until-turn -- NOT FOUND.** Extra turns exist
  （THANKS PARTY 「获得一个额外回合」, 燃尽前的线香花火 「获得一个额外回合」）,
  and 「直到下回合开始」 durations exist （EXIST, 夏日合宿）, but no cell
  says which turn start ends a "until your next turn start" effect.
* **Other G-rulings -- NOT FOUND** (G06 split-leg +100, G07 negated EXIST
  as 「造成影响」, G08 星月夜 whole-set re-roll, G13 signed 移动格数,
  G26 Popipapapipopa crystal spend vs owner rent, G29 春日影 draw-to-6
  replaceability, t04 Anon link + mortgaged partner). The relevant card
  texts are in the sheet but carry no ruling gloss.
* **三全音 payback recipient (bank vs original payee) -- NOT FOUND.**
  `新卡组卡` `G2` 「弃置此卡并支付由此卡获得的资金」 names no payee.
* **像往常一样 money refund on an undone forced move -- HINT.**
  `新卡组卡` `C6` 「（2）受到异常移动效果（包括你的技能）的回合结束前，回到
  起始地点并取消所有受到的效果（不进行任何结算）」. 「取消所有受到的效果」
  is broad enough to include undoing money the forced move moved, and
  「不进行任何结算」 scopes to the *return* (no settle on the way back).
  Sumimi's parallel wording （`card value` `W7` 「回到原处并取消所有受到的
  效果」） is equally silent on money. Not decisive.

**Superseded by the sheet update (2026-10-06):**

* **黑色生日** -- the user ruled 「1000以下」 is **exclusive** (1000 pays
  nothing). The live `新卡组卡` `J14` supersedes that ruling: 「每名你以外的
  资金**不多于1000**的玩家支付你800资金，每名你以外的资金严格在1000以上的
  玩家支付你两次200资金」. 1000 is **inclusive** and pays 800. The older
  wording 「资金在1000以下」 survives in `card value` `S19`, `卡数值` `Z14`
  and `卡时间点` `S14` (those tabs are not the live source). The engine
  already treats 1000 as ≤1000, so the new tests pass.
* **羽丘的不可思议女孩** -- the user ruled 「大于20」 is **strictly >20**.
  The live `新卡组卡` `I7` supersedes that ruling: 「若出目**至少为10**则在
  当前格子免费加盖一层房屋，若出目**至少为15**，则额外抽一张卡，**至少为
  20**，则将此卡放置在自己场上…若小于10，此卡放入弃牌堆且视为此卡未生效」.
  A d20 of exactly 20 places the card; 19 does not. The engine still uses
  the old 「大于…」 thresholds, so the inclusive-boundary tests are
  `#[ignore = "DISCREPANCY: sheet 2026-10-06 …"]`.

**Sheet sync 2026-10-06 (`docs/rulebook/cards-sheet.csv` ← live `新卡组卡`):**

All 21 drifted cells are now synced to the CSV (same 20x13 layout,
literal backslash-n escapes, CRLF, no trailing newline). Changed cards and
the tests that encode them:

| cell | card | change | tests | status |
|---|---|---|---|---|
| `J14` | 黑色生日 | 「1000以下」 → 「不多于1000」 | `rb_mujica::black_birthday_boundary_1000_inclusive`, `black_birthday_1000_pays_800`, `black_birthday_pays_by_money_bracket` | pass |
| `M2` | 想要成为人类 | 「小于X」 → 「小于等于X」 | `rb_crychic::want_to_be_human_crystal_on_roll_equal_to_x` | **DISCREPANCY** (engine still `<X`) |
| `I7` | 羽丘的不可思议女孩 | 「大于10/15/20」 → 「至少为10/15/20」 | `rb_mygo::haneoka_at_20_places_the_card`, `haneoka_at_10_builds_free_house`, `haneoka_at_15_draws` | **DISCREPANCY** (engine still `>`) |
| | | | `rb_mygo::haneoka_at_19_does_not_place`, `haneoka_at_9_does_not_build`, `haneoka_14_does_not_draw`, `haneoka_below_10_is_ineffective`, `haneoka_over_10_builds_free_house`, `haneoka_over_15_also_draws` | pass |
| `A12` | THANKS PARTY | 「大于35」 → 「至少为35」 | `rb_general::party_x2_roll_exactly_35_wins` | **DISCREPANCY** (engine still `>35`) |
| | | | `rb_general::party_x2_roll_34_pays_nobody`, `party_x2_roll_win_pays_out`, `party_x2_roll_miss_pays_nobody`, `party_x1_grants_extra_turn`, `party_x3_rolls_three_dice` | pass |
| `C3` | Y.O.L.O | 「你的掷骰」 → 「你的任意掷骰」 | `rb_ag::yolo_offered_on_a_non_move_roll` | **DISCREPANCY** (engine rings only on MoveRoll) |
| | | | `rb_ag::yolo_adds_1d4_to_the_roll`, `rb_cross_move::m01_yolo_own_roll_only` | pass |
| `G7` | （NNM）稍微努力了一下 | (2) 「x*2000资金」 → 「x次经过CiRCLE时的资金奖励」 | `rb_morfonica::nnm_option2_gains_x_times_circle_money_reward`, `nnm_discard_marks_draw_x` | **DISCREPANCY** (card bug: two On::Play entries; hand play refused) |
| `E3` | 学生会的检查 | 「压」 → 「觉悟」 | `rb_general::student_council_check_adds_juewu_not_ya` | **pass** (un-ignored 2026-10-06) |
| `E4` | 向着顶点 | adds 「视为你的主要移动」 | `rb_roselia::toward_the_top_counts_as_the_main_move`, `toward_the_top_moves_to_the_next_purchasable_livehouse` | pass |
| `H8` | 练习室里的风暴 | (2) forced re-settle → direct charge | `rb_ras::storm_part2_discards_and_forces_a_settle`, `storm_part2_rent_factor` | pass (same money; mechanism change unobservable at this level) |
| `J8` | （初华）我，无畏悲伤 | 「投掷结果」 → 「移动掷骰结果」 | `rb_mujica::hina_redefine_applies_to_the_move_roll`, `hina_fearless_sadness_after_memory_tile` | pass |
| `I3` | （灯）不再迷茫 | adds crystal-refill clause | `rb_mygo::light_refills_crystals_lost_for_a_non_skill_cost_reason` | **pass** (un-ignored 2026-10-06; loss is a real J13 crystal move, not a field write) |
| `H10` | UNSTOPPABLE | 「1d6」 → 「1d6mod6」 | `rb_ras::unstoppable_maps_dice_and_pays_out` (face 6 wrap), `unstoppable_does_not_settle` | pass (face 6 → CHUCHU, 0≡6 convention) |
| `F11` | 热气球演出 | 「3d20」 → 「3d20mod60」 | `rb_hhw::hot_air_balloon_mod60_wraps_a_sum_of_60`, `hot_air_balloon_teleports_to_a_chosen_roll` | pass (sum 60 → #60, 0≡60) |
| `F9` | 运动的天赋 | 「10以上」 → 「至少为10」 | `rb_hhw::athletic_talent_keeps_a_roll_of_exactly_10` | pass |
| `H16` | 寄于指尖的执念 | gains a [反击] wrapper | `rb_ras::nagisa_fingertip_is_offered_as_a_counter_on_a_fire_pot_roll` | **DISCREPANCY** (never offered in the counteract ring) |
| `G15` | （Rui）正论恶魔 | 「若」 → 「每当…立即」 | `rb_morfonica::rui_card_sets_x_to_5_when_the_skill_fires` | **DISCREPANCY** (X left at 3) |
| `A8` | 该清CP了 | drops the [特] CP-count gate | `rb_general::cp_hand_is_not_gated_at_two_points` | **DISCREPANCY** (`clear_cp_too_many`) |
| `F13` | Wacha Mocha | markers on the card; 主要阶段 activation | `rb_hhw::wacha_mocha_markers_sit_on_the_card_and_activate_in_main_phase` | pass (placement only; marker slot not pinned) |
| `J2` | 表演的本能 | 「任意其他玩家」 → 「其他玩家」 | `rb_mujica::mortis_instinct_plays_without_error` | pass (wording) |
| `C11` | 朝同一片天空迈进 | 「大于等于」 → 「至少为」 | `rb_ag::sky_fires_on_draw` | pass (wording) |
| `C10` | ONE OF US | 「次地契」 typo → 「此地契」 | `rb_ag::one_of_us_places_on_field`, `rb_cross_tiles::t05_one_of_us_shares` | pass (typo) |

Character / band skill text vs `data/`:

* `data/bands.json` matches `团技能` `D`-column exactly for all 12 bands
  (whitespace-normalized). No drift.
* `data/characters.json` matches `角色技能` `E`-column for 53 of 54
  characters. **One drift: 北泽育美 「全垒打！」** -- `data/characters.json`
  (and the hand-written `data/skill_simple.json`) carry three extra clauses
  that the sheet does not:
  「育美经过其他角色时，可以把自己身上的可乐饼转移给该角色。育美可在主要阶段
  消耗4个可乐饼兑换1000资金，其他角色拥有4个可乐饼时自动移除所有可乐饼停留
  一回合。」 The sheet's E14 stops at 「…移动时多投掷1个1d2。」.
  `data/` is another agent's; not edited here.
* `data/skill_simple.json` is the hand-written newbie gloss (its own note
  says the export script does not overwrite it) and is not a second source
  of truth; it tracks `characters.json` for 北泽育美, so it carries the same
  extra clauses.
* `docs/rulebook/cards.json` still holds the **old** 寄于指尖 text (no
  [反击] wrapper) -- a further sync target for whoever owns that file.

### 7. Coverage gaps

The clause-level picture lives in [COVERAGE.md](COVERAGE.md); this is the
short list.

* No test file mentions these rule ids at all (COVERAGE "Part C mechanical
  checks"): AG 无论是何种颜色的夕阳, AG （巴）商店街的救世主, HHW （薰）怪盗
  hello happy, HHW （花音）Wacha Mocha 啪嗒进行曲, HHW （美咲）,
  skill:濑田薰:梦幻的王子殿下, RAS （和奏瑞依）寄于指尖的执念,
  R （纱夜）弹奏弹奏弹奏，继续弹奏, R 必然的联系（莉莎）. The exclusive
  cards PP:[大和麻弥]可能性为∞ and PP:[冰川日菜]会发出怎样的声音呢？ are
  also never played (their *skills* are tested).
* Marked not testable or shallow:
  * **hhw:** 笑容大游行 (also a stack overflow -- t13), 怪盗hello happy,
    kkr, Wacha Mocha, 育美, 美咲, 北泽育美, 濑田薰.
  * **roselia:** 学生会的检查, 纱夜, 轨迹, 莉莎, 宇田川亚子.
  * **sumimi and crychic:** many tests are "is a counter" / "skill is bound"
    only.
  * **pp:** 冰川日菜 exclusive.
  * **ras:** 寄于指尖的执念, 鳰原令王那.

### 8. Housekeeping

* `python tools/i18n/check.py` has 4 problems that predate this work:
  `err.skill_not_ported`, `anim.turnBanner`.
* None of this work is committed: the harness, the tests, the engine and card
  fixes, the rename, and the engine test seams `Rng::load_dice` /
  `Match::world_mut`.
* `target/scratch/tainted/` holds the discarded rule-breaking test drafts.
  Delete them when no longer needed.

### 9. Design reference: `ileuxali/bangdream-monopoly`

This is a fan reimplementation in Unity, not the original game. All its cards
are dummies, so it gives **no rulings** on card text (§6). Its Accepted design
docs do answer engine-shape questions:

* **§1.1 game-start hooks.** Three events, `BeforeMatchStart` (start
  positions, initial hand size ≥ 0) → `MatchStarted` → `AfterMatchStart`
  (initial tokens such as fire pots and fans). They go to every spawned source
  (status / band / character / card), not to pile membership.
* **§1.2 payment counteracts.** Every payment kind (print / delete /
  pay-player) runs AddDiff → MultDiff → Cancel twice: once pre-split for the
  whole command, then post-split per concrete payer/payee pair. A per-pair
  Cancel also answers §3's "cancel one designation" for payments.
* **§1.5 per-draw hooks.** One `BeforeCardDraw`/`AfterCardDraw` window per
  card. An N-card draw is N iterations.
* **「[经过]」.** Every entered tile raises a pass event, the destination
  included, with no duplicate pass at landing. The event carries the previous
  and entered indexes so "actually entered" effects can filter.
* **Counteract priority.** A round-robin ring per window:
  * The triggering player goes first, then turn order, then a neutral
    responder.
  * Each visit allows one activation.
  * Lookup groups run Status → Band → Character → Field → Hand → Discard →
    Deck → Removed.
  * Nested windows stack LIFO.

  Our ring starts at the *next* seat (clause 89), not the triggering player --
  their "goes first" contradicts the book. The per-visit one-activation cap is
  the model for the §10 ruling; "nested windows stack LIFO" is what §10's
  resolution now does over the answer tree.
* **Mid-turn [停留].** Prohibition is checked once, at move-branch selection,
  so a [停留] gained mid-move doesn't stop the current move. This contradicts
  our `rain_stay_blocks_this_turn_move` expectation. Treat that one as an
  ambiguity.
* **§2 design debt.** Restrictions are explicit status-rule data
  (`ProhibitsStepMovement`, …), never prose. This confirms the plan to move
  `hand_limit_delta` and the stun exception onto card data.
* **「向上取整10」.** Their money splitting rounds each share up to a multiple
  of 10 globally, which hints that 爱心义演's wording restates a general rule.

### 10. Counteract chain order vs the rulebook

The rulebook (`data/rules.txt`):

* **32:** 「[反击]…在X发生时打出并触发效果Y，且结算优先于X」.
* **89:** 「如果有多名玩家可在同一时间发动[反击]效果则从行动顺序上在触发[反击]时点
  的玩家的**下一位**玩家开始依次决定是否使用[反击]效果。多个效果可[反击]同一个时点，
  所有玩家同意所有对一个时点的[反击]已发动后可对新的时点发动[反击]。」

Our engine: `hand_counteractions` / `build_round` / `resolve_rounds` /
`chain_starter` / `Priority` in `crates/game-rules/src/wasm_rules.rs`.

**Fixed** (2026-10-06, `rb_chain.rs` covers each):

1. **Who is asked first.** ~~`Priority` opens at the triggering player.~~ The
   ring now opens at the *next* player and asks the triggering player last
   (clause 89). Covered by
   `the_next_seat_is_asked_first_and_the_triggering_player_last`.
2. **Chain shape.** ~~Each declaration becomes the new top, and the next
   responder answers it.~~ A round now collects counters to the **same**
   timing; only once it closes do the declared counters become new timings with
   their own rounds (「…后可对新的时点发动[反击]」). Covered by
   `two_players_answer_the_same_effect_not_each_other` and
   `counters_to_a_counter_wait_for_the_round_on_x_to_close`.
3. **Priority after a declaration.** ~~`Priority::answered` resets to the
   starter.~~ A declaration now advances to the next responder (「依次」), and a
   player may declare again when the ring returns. Covered by
   `a_declaration_passes_priority_to_the_next_seat` and
   `a_player_may_declare_again_when_the_ring_returns`.

**Rulings** (what the book leaves open; recorded in the `hand_counteractions`
doc comment):

* **The order several counters to one timing resolve in.** **LIFO**: counters
  to one timing resolve newest first, and every counter settles before the
  timing it answers. Over the whole answer tree: a counter's own counters
  settle before it, and sibling counters settle newest first (post-order, each
  node's answers newest first). Covered by `counters_resolve_newest_first`.
* **Whether one player may declare more than one counter to a timing.** One
  activation **per responder visit** (ileuxali/bangdream-monopoly's accepted
  round-robin): a declaration advances priority to the next responder and does
  not reset; a player may declare again when the ring comes back to them; the
  round closes after every responder has passed consecutively with no
  activation in between. 「多个效果可[反击]同一个时点」 doesn't cap it per
  player. Covered by `a_player_may_declare_again_when_the_ring_returns`.

Also fixed here: a counter whose activation one of its own answers negated
(`set_cancelled`) no longer runs its body, while the declaration still stands
and the card is spent (`a_negated_counters_body_does_not_run`).

`ileuxali/bangdream-monopoly` asks the triggering player *first*, which also
contradicts clause 89, so it isn't a model for the ring; its round-robin
per-player cap is the model for the ruling above.

## Harness notes

* Engine test seams: `Rng::load_dice`, `Match::world_mut` / `world`.
* Loaded dice append to a global queue. Card rolls consume faces too.
* `clean()` keeps skills but drops deeds. Use `set_fire` for pots.
* `buy` / `build` are actions taken after the move, not prompts.
* `discard_card` is only the over-hand-limit discard.
* There is no token setter or crystal setter. Use `world_mut()`.
* `place_raw` skips the play body.
