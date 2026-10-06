# Rulebook black-box test findings

Black-box integration tests check every card, character skill and band skill
against the live rule sheet. The sheet is the Google Sheet with cards /
characters / bands tabs, and it is the definitive source. The tests live in
`crates/game-rules/tests/rb_*.rs`, on the shared harness `tests/common/mod.rs`.
They were written without reading `rules/cards/**` or `rules/skills/**`.

* `cargo test -p game-rules --test rb_<group>`: everything passes.
* `cargo test -p game-rules --test rb_<group> -- --ignored`: runs the
  **open discrepancies**. Each ignored test keeps the rulebook's assertion, and
  its `#[ignore = "…"]` reason says what the engine does instead.
  * `DISCREPANCY:` marks an engine or card gap.
  * `TEST BUG:` / `CROSS-AGENT:` mark a test that needs fixing or a ruling.
* When a fix makes one pass, remove its `#[ignore]`. Never weaken the
  assertion.

## Status (2026-10-06, after the engine fix, card fix and `counteract` rename)

| group | pass | open |
|---|---|---|
| general (通用 + CiRCLE) | 51 | 6 |
| ppp | 38 | 1 |
| ag | 33 | 2 |
| pp | 39 | 18 |
| roselia | 29 | 9 |
| hhw | 19 | 2 |
| morfonica | 44 | 11 |
| ras | 58 | 4 |
| mygo | 50 | 26 |
| mujica | 43 | 4 |
| sumimi | 21 | 0 |
| crychic | 34 | 0 |

The first pass found 106 discrepancies; about 25 are fixed. A 0 in "open"
means none were found, not that the group is verified correct. See coverage
gaps below.

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
* **Cards:**
  * Y.O.L.O (your own roll only).
  * Anon Tokyo (上限1 + link rent).
  * 若能再次交汇 (20-tile cap).
  * 羽丘 (strictly >20).
  * 育美 (2).
  * FEVER! (counts itself).
  * 三全音 (listened on the wrong trigger).
  * 再次闪耀 (playable from hand).
  * （祥子）斩断留恋.
* **Spec sync:**
  * `docs/rulebook/cards-sheet.csv` / `cards.json` re-extracted from the
    live sheet.
  * `data/cards.json` text updated: Y.O.L.O, Anon Tokyo, 若能再次交汇,
    （育美）.
  * `tools/rulebook/cite.py` fixed for `CardDef::new`.
* **Rename:** [反击] is `counteract` throughout. ABI v28; `ask.counteract.*`;
  `log.play_counteract`.

## Open items

### 1. Engine bugs needing a design decision

1. **Game-start hooks don't reach field (skill) cards.**
   `DeckAtGameStart` / `DeckBeforeGame` are dispatched only to ids in piles
   and hands. Blocked as a result:
   * 「初始N」 fire pots: 美竹兰, 都筑诗船, 高松灯, 千早爱音, 长崎素世,
     要乐奈.
   * P✽P fans: PP character (1) and PP band (1).
   * Start tiles: 凑友希那 RiNG 4, 松原花音 弦卷豪宅, MyGO band 3d20.
   * 梦在前方's 初始手牌−1 (`startHandMinus` is written but ignored).
2. **Pay / loss counteract windows open only for rent**, not for payments a
   card forces. Affected: （小白）, 三全音's card-driven case.
3. **Two band-crystal stores.** `bandCrystals` keyed state vs the band skill
   field card's `crystals`. Affected: PP TITLE IDOL / 同一个梦想 / 花朵,
   MyGO band (2)/(3), Mujica 会被骗着买水晶的人. Unify on the band card.
4. **「无法获取[CiRCLE奖励]」 has no suppression hook.** Affected: PPP band
   (2), PP band (3), 凑友希那's first CiRCLE pass.
5. **`ctx::draw` doesn't raise `drew`.** Per-draw hooks never fire:
   梦在前方 (1), 若宫伊芙 exclusive (1).

### 2. Design debt introduced by the engine fixes (review)

* The hand-limit cut is derived at load from the rule *prose*
  「手卡上限数量减1」 (`CardData.hand_limit_delta`).
* The stun exception keys on the prose 「可在眩晕时打出」.
* Behaviour keyed on prose silently breaks if a card is reworded. Move both
  to explicit data on the card definition.

### 3. Missing card/ABI capabilities (TODO(ABI) placed)

* 网络链接异常: cancel *one* designation. Needs a static targeting query; today
  it negates the whole multi-target card.
* 育美 (2): count gains during the turn. There is no gain event, and nothing
  observes a card while it is in hand.
* Per-uid crystal read/write (会被骗着买水晶的人 moves all crystals, not one).
* Anon Tokyo uses a field stand-in and slots, because the original
  attachment model / mark notes are not readable.

### 4. Remaining per-card discrepancies

The `#[ignore]` reasons are authoritative. Current list:

* **general**
  * @Tsugu ycm's −1500 buy discount is not applied.
  * FEVER! doesn't boost money a card makes you [获得].
  * 网络链接异常 negates a whole multi-target card (see 3).
  * The CiRCLE band (2) doubling is unported.
* **ppp**: band (2) no-CiRCLE-reward (see 1.4).
* **ag**: 朝同一片天空迈进 doesn't auto-play on draw.
* **pp**
  * 丸山彩 / 白鹭千圣 exclusives do nothing.
  * 白鹭千圣 (2), 大和麻弥 (2) and 冰川日菜 (2) never prompt.
  * 初次演出事故 doesn't block the (2) skills.
  * 可爱又强壮的花朵 doesn't force a stop, and its [共鸣] +1500 never opens.
  * 安可 doesn't counter 重叠的声音's self-teleport.
* **roselia**
  * 凑友希那 (1)/(3) and stop-pot are missing.
  * 白金燐子 (2) has no pre-roll window.
  * live前的准备 / 选择自己的舞台 open no window.
* **hhw**
  * 松原花音 start tile (see 1.1).
  * 黑衣人的补给 opens no window.
* **morfonica**
  * 你的光芒 doesn't move the start tile.
  * 纯真振翅 never rolls after the teleport.
  * 星月夜's reroll and even-roll clauses never fire.
  * 秘密与青春的虹彩's 1.5× case zeroes the payment instead.
  * 瑠唯's crit never fires on gains, and the 正论恶魔 X=5 doesn't apply.
  * 筑紫 (1) never prompts.
  * NNM / 广町七深 (2): marks are unobtainable.
* **ras**
  * 游击演出 lands one tile past the chosen one, and its [特] never opens.
  * 练习室里的风暴 and Repaint don't scale rent.
* **mygo**
  * 壱雫空 clears nothing and charges only affected players.
  * （立希） and （soyo）混合的颜色 don't reduce payments.
  * 哪怕这旅程 pays nothing and doesn't drain.
  * 无路矢's income is dropped.
  * 那天的雨 misses adjacent tiles.
  * （乐奈）有趣的女人 doesn't gain on pass.
  * 灯 不再迷茫 crystal-as-fire and removal don't work.
  * 要乐奈's Space teleport doesn't move the piece.
  * MyGO band (2)/(3) don't work.
  * A mid-turn [停留] doesn't stop the move.
* **mujica**
  * 骰子已经掷下 is placed twice.
  * 祥，移动 doesn't halve the payment.
  * 会被骗着买水晶的人 moves all crystals (see 3).
  * 三角初华 state 1 lacks the +1d10.

### 5. Tests needing a fix or a ruling

* `rb_general::ix_great_perfect_fever_chain` (TEST BUG): asserts the old
  FEVER! X=600. The sheet gives 400. Correct the expected money.
* `rb_pp::guide_hand_limit_and_turn_end_crystal` (TEST BUG): never sets P✽P
  fans, and only passed while `turnEnd` double-fired.
* `rb_mygo::inter_yolo_pushes_haneoka_over_20` (CROSS-AGENT): needs a ruling
  on when Y.O.L.O's +1d4 is added, now that it is own-roll-only.
* `rb_pp::accident_blocks_pp_skill_2`: uses `place_raw`, which skips the play
  body. Arrange it by playing the card.
* `rb_morfonica::tritone_countdown_discards_and_pays_back`: the harness's
  turn-end sequencing errs (`err.no_roll_now`). The card body works.

### 6. Ambiguities needing a ruling

* 黑色生日 「1000以下」: inclusive?
* 羽丘 「大于20」 on a d20: now strictly >20, so it is unreachable without
  modifiers.
* THANKS PARTY's X: with or without the user?
* 壱雫空 「每清除一种效果」: per type, per layer, or per player?
* 勇气展翅: (price+houses)/2?
* 爱心义演 「向上取整10」.
* 祥，移动 「支付价格」: does it include rent?
* Hey Kids's trigger subject.
* 秘密与青春的虹彩 grades: there is no grade data in `data/`.
* Returns: does the borrowed band card replace the PPP one, or sit beside it?
* Which of the 技能 / 「改」 / extra-column texts is current for 丸山彩,
  上原绯玛丽 and 羽泽鸫? The 技能 column is what's implemented.
* The band tab's alternate Pastel✽Palettes 技能卡/角色卡/團卡 design: is it
  adopted or just a draft?
* Sheet typos left out of `data/cards.json`:
  * “PERFECT“ / “FEVER!“ quote marks;
  * 星月夜 「的的」;
  * 游击演出 stray `\[特]`.

### 7. Coverage gaps

* No tests: AG 无论是何种颜色的夕阳, （巴）商店街的救世主.
* Marked not testable or shallow:
  * **hhw:** 笑容大游行, 怪盗hello happy, kkr, Wacha Mocha, 育美, 美咲,
    北泽育美, 濑田薰.
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
