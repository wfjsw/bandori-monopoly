# Rulebook black-box test findings (2026-10-06)

Black-box integration tests checking every card, character skill and band skill
against the live rule sheet (the Google Sheet, cards / characters / bands tabs).
They live in `crates/game-rules/tests/rb_*.rs`, on the shared harness
`tests/common/mod.rs`. The tests were written without reading `rules/cards/**`
or `rules/skills/**`.

* `cargo test -p game-rules --test rb_<group>`: everything passes.
* `cargo test -p game-rules --test rb_<group> -- --ignored`: runs the
  **discrepancies**. Each ignored test keeps the rulebook's assertion, and its
  `#[ignore = "DISCREPANCY: …"]` says what the engine does instead.

| group | pass | discrepancies |
|---|---|---|
| general (通用 + CiRCLE) | 50 | 7 |
| ppp | 38 | 1 |
| ag | 33 | 2 |
| pp | 36 | 21 |
| roselia | 28 | 10 |
| hhw | 17 | 4 |
| morfonica | 41 | 14 |
| ras | 54 | 8 |
| mygo | 45 | 31 |
| mujica | 39 | 8 |
| sumimi | 21 | 0 |
| crychic | 34 | 0 |

Coverage is uneven:
* sumimi / crychic / hhw / roselia have several shallow tests ("is a counter",
  "skill is bound") and cards marked not testable.
* AG: 无论是何种颜色的夕阳 and （巴）商店街的救世主 have no tests.
* A 0 in the discrepancy column means "none found", not "verified correct".

## Spec drift (sheet vs repo copies)

* `docs/rulebook/cards-sheet.csv` / `cards.json` are behind the live sheet on
  three cards:
  * Y.O.L.O now reads 「**你的**掷骰」.
  * Anon Tokyo adds 「（上限1）」.
  * 若能再次交汇 adds a 20-tile cap.
* `data/cards.json` `text` for **HHW:（育美）** is an old version: the sheet
  adds the >60 branch and a (2) [反击].
* Character tab:
  * 丸山彩 has a different 「改」 revision.
  * 上原绯玛丽 and 羽泽鸫 carry alternate text in an unnamed extra column.
* Band tab: three extra rows (Pastel✽Palettes 技能卡/角色卡/團卡) describe a
  different P✽P fan design.

## Cross-cutting engine findings

Each of these explains several discrepancies at once.

1. **Free houses.** `next_turn` builds `TurnCtx { .., ..Default::default() }`.
   The derived `Default` makes `build_cost_pct = 0` (only serde defaults it to
   100), so `build` (play.rs) charges 0 for every house in real games.
2. **「初始N」 fire pots.** Skills set the cap but the starting count stays 0
   (美竹兰, 都筑诗船, 高松灯, 千早爱音, 长崎素世, 要乐奈, …).
3. **Game-start hooks on skills never run.** Examples:
   * 5 P✽P fans per PP character, 1 face-down fan for non-PP players;
   * 凑友希那 starting on RiNG 4;
   * 松原花音 starting on 弦卷豪宅;
   * the MyGO band skill's 3d20 start tile.

   `DeckAtGameStart` is dispatched only to cards in piles/hands, not to field
   (skill) cards.
4. **「[经过]X」 only fires for CiRCLE or the move's end tile.** Tiles merely
   walked through don't count. Affected: 高松灯 (RiNG), 佐藤益木, 奥泽美咲.
5. **「无法获取CiRCLE奖励」 is not honoured**, for the PPP band (2) and the PP
   band (3).
6. **Wrong discard pile.** Several placed cards go to another player's discard
   (the next player, previous player or payer): 骰子已经掷下, 燃尽前的线香花火,
   #J11, FEVER!, EXIST, Change the world.
7. **Removed from the game instead of discarded**: 蝴蝶飞舞的星月夜,
   再次牵起手来.
8. **Field-card turn-end decay runs twice per turn end** (`end_turn` raises
   `turnEnd` twice). Affected: Fire bird (800 / 2 crystals),
   那天的雨 (5→3), chuchu (3→1), #J11.
9. **Two band-crystal stores.** `bandCrystals` state vs the band skill field
   card's `crystals`. Cards add to one, the band skill spends the other
   (PP TITLE IDOL / 同一个梦想 / 花朵, Mujica 会被骗着买水晶的人).
10. **`card_mortgage` always refuses** (`why_not_mortgage(.., asking = true)`
    returns busy). So （祥子）斩断留恋 does nothing.
11. **`card_move` leaves stage MOVE** when it moves a player whose turn it
    isn't.
12. **Hand limit −1 effects don't apply.** The over-hand check uses the
    constant `HAND_LIMIT` (不要背负期待, 梦在前方).
13. **Pay-modifying counters only see rent.** They never fire on payments a
    card forces (（小白）, 三全音).
14. **Placed cards' [持续] activations are refused via `skill`.** `current_uid`
    is -1, so `is_placed`/`crystals` read 0 (即使迷茫着 (2), MyGO band (2)/(3)).

## Per-card discrepancies

The `#[ignore]` reasons in each `rb_*.rs` are the authoritative list.
Highlights:

* **general**
  * 网络链接异常 negates a whole multi-target card instead of one target.
  * @Tsugu ycm's −1500 buy discount is not applied.
  * FEVER! doesn't count itself in X.
  * FEVER! doesn't boost money a card makes you [获得].
  * The CiRCLE band (2) doubling is unported.
* **ag**: 朝同一片天空迈进 doesn't auto-play on draw.
* **pp**
  * 再次闪耀 can't be placed from hand.
  * 丸山彩/白鹭千圣 exclusives do nothing.
  * 白鹭千圣 (2), 大和麻弥 (2) and 冰川日菜 (2) never prompt.
  * 初次演出事故 doesn't block the (2) skills.
  * 可爱又强壮的花朵 doesn't force a stop.
* **roselia**
  * 凑友希那 (1)/(3) and 白金燐子 (2) are missing.
  * live前的准备 / 选择自己的舞台 open no window.
* **hhw**
  * 因为我一直相信着你 traps.
  * 黑衣人的补给 opens no window.
  * 运动的天赋 pays nothing.
* **morfonica**
  * 三全音's body does nothing.
  * 你的光芒 doesn't move the start tile.
  * 纯真振翅 never rolls after the teleport.
  * 星月夜's reroll and even-roll clauses never fire.
  * 秘密与青春的虹彩's 1.5× case zeroes the payment instead.
  * 瑠唯's crit never fires on gains.
  * 筑紫 (1) never prompts.
* **ras**
  * 游击演出 lands one tile past the chosen one.
  * 练习室里的风暴 and Repaint don't scale rent.
  * 佐藤益木 (1) only counts the end tile (see 4).
* **mygo**
  * 壱雫空 clears nothing and can't be played while stunned.
  * Anon Tokyo's link adds no rent and ignores 上限1.
  * （立希）/（soyo） don't reduce payments.
  * 哪怕这旅程 pays nothing and doesn't drain.
  * 无路矢's income is dropped.
  * [停留] gained mid-turn doesn't stop the move.
* **mujica**
  * 骰子已经掷下 is placed twice.
  * 祥，移动 doesn't halve the payment.
  * 会被骗着买水晶的人 moves all crystals, not one.
  * 三角初华 state 1 moves exactly 10 (+1d10 missing).

## Ambiguities worth a ruling

* 黑色生日 「1000以下」 (inclusive?).
* 羽丘 「大于20」 on a d20.
* THANKS PARTY's X (with or without the user).
* 壱雫空 「每清除一种效果」 (per type / layer / player).
* 勇气展翅 (price+houses)/2.
* 爱心义演 「向上取整10」.
* 祥，移动 「支付价格」 (rent too?).
* Hey Kids's trigger subject.
* 秘密与青春的虹彩 grades: there is no grade data in `data/`.
* Returns: does the borrowed band card replace the PPP one, or sit beside it?
* Which of the 技能 / 改 / extra-column texts is current for 丸山彩,
  上原绯玛丽 and 羽泽鸫.

## Harness notes

* Engine test seams: `Rng::load_dice` and `Match::world_mut` / `world`.
* Loaded dice append to a global queue. Card rolls consume faces too.
* `clean()` keeps skills but drops deeds; use `set_fire` for pots.
* `buy`/`build` are actions taken after the move, not prompts.
* `discard_card` is only the over-hand-limit discard.
* There is no token setter; use `world_mut().st.players[who].tokens`.

The earlier rb_morfonica / rb_ras / rb_mygo / rb_pp attempts broke the
black-box rule and were rewritten from scratch. The discarded copies are in
`target/scratch/tainted/`.
