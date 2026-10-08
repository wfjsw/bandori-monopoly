# Cross-card / cross-band test design

Black-box scenarios where effects from **different cards, character skills and
band skills meet**. Every expectation comes from the live sheet text
(`target/scratch/rb/*.md`) and the glossary / general rules in
`data/rules.txt`. The implementation goes in
`crates/game-rules/tests/rb_cross_*.rs` on the harness `tests/common/mod.rs`.

## Conventions

* `P0..P3`: seats = turn order. Use `Table::vanilla(n)` (no skills) unless a
  character or band skill is part of the scenario. In that case use
  `Table::new(&[..])` and `clean()` + `begin_turn(0)`. Everyone is human, so
  every eligible [反击] window is offered.
* Tiles are engine indices. The rulebook's 「#N格」 is index `N-1`.

  | index | tile | index | tile | index | tile |
  |---|---|---|---|---|---|
  | 0 | CiRCLE | 25 | 小豆岛 | 44 | 星之鼓动山丘 |
  | 1 | 购物中心 | 28 | 弦卷集团 (agent) | 45 | 流星堂 |
  | 2 | 天文馆 | 29 | 弦卷豪宅 | 46 | 商店街 (agent) |
  | 4 | 主要街道 (agent) | 30 | 江户川乐器店 | 47 | 羽泽咖啡厅 |
  | 8 | RiNG 1 | 31 | Live House (agent) | 48 | 山吹面包房 |
  | 13 | 月之森 | 35 | Bandori车站 | 49 | 银河拉面馆 |
  | 15 | CiRCLE 咖啡厅 | 36 | RiNG 3 | 50 | Live House Galaxy |
  | 21 | DUB | 40 | 武道馆 | 52 | 旭汤澡堂 |
  | 23 | 东京外 (agent) | 42 | Space | 53 | RiNG 4 |
* `R(t,h)` is tile `t`'s rent with `h` houses: `data/board.json` `rent[h]`.
  `P(t)` is its price and `H(t)` is its house cost. Compute expected numbers
  from the data, never hard-code them blind.
* Rules from `data/rules.txt` used throughout:
  * **32:** a [反击] resolves before the thing it answers.
  * **89:** counter windows ask from the player *after* the one who triggered
    the timing; several counters may answer one timing; counters to counters
    come after that round.
  * **Rulings:**
    * counters to the same timing resolve **newest first**;
    * one declaration per player per visit; priority passes on, and the ring
      may return to a player.
  * **43 / 86:** [经过] = every tile in the [路径], including the 终点. A pass
    effect that changes the 终点 recomputes the path, and later tiles are not
    passed.
  * **47–52:** [停留] / [晕眩] / [除外] = [无法移动]. [晕眩] and [除外] also
    mean no [主动] skills, no [手] effects, and no receiving or paying. [除外]
    also means not [指定]-able and [持续] effects pause. [不可阻挡] ignores
    stay/stun and may refuse [传送] / [强制移动] / [强制停下].
    [异常移动效果] = 传送, 停留, 晕眩, 除外, 强制移动, 强制停下, and moving
    backwards.
  * **90:** effects not written as optional are mandatory.
  * **92:** mortgage, redeem and forced-purchase money is never modified.
  * **96:** the CiRCLE reward needs a 起点 that is not CiRCLE.
  * **107:** an agent tile's settle.
* A case that depends on an **open ruling** (TEST-FINDINGS §6) is marked
  **RULING**. Implement it, then `#[ignore = "RULING: …"]` it unless the
  engine already matches the stated expectation.
* A case the engine gets wrong is marked `#[ignore = "DISCREPANCY: …"]` as in
  the rb_* suites. Never weaken an assertion.

---

## 1. Effects, counter effects and chains (`rb_cross_chain.rs`)

### C1. A single counter answers a multi-target card
* **Cards:** 通用:登上武道馆 vs AG:宣战布告. 3 players.
* **Setup:** P1 holds 宣战布告.
* **Act:** P0 plays 登上武道馆, so X = ceil10(2000÷2) = 1000.
* **Expect:**
  * The window is offered to P1 (P1 is the seat after P0).
  * P1 declares.
  * 宣战布告 resolves first (32): P0 pays P1 500, and P1 draws 1.
  * Then 武道馆 resolves: P1 and P2 each pay P0 1000.
  * Final money: P0 = 11500, P1 = 9500, P2 = 9000. P1's hand +1.

### C2. Two counters on the same timing, from different players
* **Cards:** 登上武道馆 vs 宣战布告 ×2. 3 players.
* **Setup:** P1 and P2 each hold 宣战布告.
* **Expect:**
  * The asks go P1, then P2. Each window's detail names 登上武道馆, so both
    answer X and not each other.
  * Both resolve before X: P0 pays 500 twice, and P1 and P2 each draw 1.
  * Final money: P0 = 10000 − 1000 + 2000 = 11000; P1 = 9500; P2 = 9500.

### C3. A counter to a counter cancels its designation
* **Cards:** 登上武道馆 → 宣战布告 → 通用:网络链接异常. 3 players.
* **Setup:** P1 holds 宣战布告. P0 holds 网络链接异常.
* **Act:** P0 plays 登上武道馆. P1 declares 宣战布告, then the round on X
  closes. Then a round on 宣战布告 opens, starting after P1: P2, then P0.
  P0 declares 网络链接异常. 宣战布告 is a [手] with a [指定], so clause 1
  cancels its designation of P0.
* **Expect:**
  * The 网络链接异常 window is offered only after the round on 登上武道馆
    closes.
  * P0 does not pay P1 500.
  * 武道馆 still resolves: P1 and P2 pay 1000 each.
* **RULING:** whether P1 still draws. 「被[指定]的玩家支付…且[使用者]抽1张卡」
  is one sentence. Assert the payment only; record the draw.

### C4. 网络链接异常 negates a [手] effect that has no target
* **Cards:** 通用:GREAT vs 网络链接异常. 2 players.
* **Act:** P0 plays GREAT. P1 counters with 网络链接异常 (clause 2: negate
  all of its effects).
* **Expect:**
  * P0's money is unchanged.
  * No PERFECT is added to P0's draw pile.

### C5. 网络链接异常 cancels one designation of four
* **Cards:** 登上武道馆 vs 网络链接异常. 4 players.
* **Setup:** P2 holds 网络链接异常.
* **Act:** P0 plays 登上武道馆. X = ceil10(2000÷3) = 670. P2 cancels its own
  designation.
* **Expect:**
  * P1 and P3 each pay 670.
  * P2 pays nothing.
  * P0 = 11340.

### C6. 安可 cancels a [停留] (an abnormal-move effect)
* **Cards:** 通用:雨啊，快点来吧 vs 通用:安可. 3 players.
* **Setup:** Load dice for 2d2 = 1 + 1, so X = 2.
* **Act:** P0 designates P0 and P1. P1 holds 安可 and counters.
* **Expect:**
  * P1 gets no [停留].
  * P0 gets 1 [停留].

### C7. Counters three deep: 安可 negated by 网络链接异常
* **Cards:** 雨啊 → 安可 → 网络链接异常. 3 players.
* **Act:** As in C6. Then in the round on 安可 (asks: P2, P0, P1), P2 plays
  网络链接异常. 安可 is a [手] with no target, so it is negated entirely.
* **Expect:**
  * P1 gets the [停留] after all.
  * 安可 and 网络链接异常 are both spent.

### C8. The user counters their own card's abnormal move
* **Cards:** MyGO:无路矢 vs 通用:安可, played by the same player. 2 players.
* **Act:** P0 plays 无路矢, designating P1's tile. In the round on 无路矢
  (asks: P1, then P0), P0 plays its own 安可 (「因任何原因」).
* **Expect:**
  * P0 gets no [除外].
  * P0 is not teleported later.
  * The counter window was offered to P1 before P0.

### C9. 离心力 on the second targeting between turns
* **Cards:** 登上武道馆 ×2 vs Mor:离心力，不为所动. 3 players.
* **Setup:** P1 holds 离心力.
* **Act:** P0 plays 武道馆 (first targeting of P1; 离心力 must not be
  offered). P0 then plays a second 武道馆 (second targeting; 离心力 is
  offered). P1 plays it.
* **Expect:**
  * P1 pays 1000 once in total.
  * P2 pays twice.
  * Until P1's next turn start, P1 is unaffected by anything.
  * Then end P0's turn. P2 plays any targeting card at P1; P1 is not affected.

### C10. 夏日合宿 vs a multi-target card and a single-target card
* **Cards:** Mor:夏日合宿 + 登上武道馆 + PP:找回珍妮弗. 3 players.
* **Setup:** It is P1's turn. P1 plays 夏日合宿.
* **Act:** It becomes P0's turn.
  * P0 plays 武道馆: P1 is not designated.
  * P0 plays 找回珍妮弗 at P1: refused, or no effect on P1.
* **Expect:**
  * Only P2 pays X. Whether X = ceil10(2000÷2) still counts P1 as an alive
    other is a **RULING**; assert P2 pays X and record X.
  * At P1's next turn start the card's effect ends. P1 negated something, so
    P1 does **not** draw.

### C11. EXIST redirects a single-target card
* **Cards:** RAS:EXIST + PP:找回珍妮弗. 3 players.
* **Setup:** P2 has EXIST on its field.
* **Act:** P0 plays 找回珍妮弗 targeting P1.
* **Expect:**
  * The card lands on P2's field, not P1's.
  * At P2's next turn start, EXIST goes to P2's discard and P2 draws **no**
    card, because EXIST did have an effect.

### C12. EXIST vs 夏日合宿
* **Cards:** EXIST (P2) + 夏日合宿 (P1) + 找回珍妮弗 (P0). 3 players.
* **Act:** P0 targets P1. P1 can only be designated by its own effects, and
  EXIST redirects the target to P2.
* **Expect:** The card lands on P2.
* **RULING:** whether 夏日合宿 counts as having "negated" something, for its
  draw.

### C13. R.I.O.T. and 宣战布告 on the same timing
* **Cards:** RAS:R. I. O. T. (P1) + 宣战布告 (P2) vs 登上武道馆. 3 players.
* **Setup:** Hands: P0 = 武道馆 plus 2 fillers; P1 = R.I.O.T. plus 1 filler;
  P2 = 宣战布告 plus 3 fillers. Draw piles are big enough for the draws.
* **Expect:**
  * Resolution is newest first: 宣战布告 (P2) first, then R.I.O.T. (P1).
  * After R.I.O.T., every player has discarded their hand and redrawn the same
    count. P1 draws 1 extra.
  * Hand sizes after everything are: P0 = 2 (武道馆 has already left the
    hand); P1 = 1 + 1; P2 = 3 + 1 from 宣战布告, redrawn to the same count.
  * Then 武道馆 pays out.
  * Assert the hand sizes and money.

### C14. 骰子已经掷下 shuts the counter windows for the turn
* **Cards:** Mujica:骰子已经掷下 + 登上武道馆 vs 宣战布告. 2 players.
* **Act:** P0 plays 骰子已经掷下, and P1 lets it resolve. Then P0 plays
  武道馆.
* **Expect:**
  * No [反击] window is offered to P1 for 武道馆.
  * After P0's turn ends, the card is in P0's discard.

### C15. 骰子已经掷下 can itself be countered
* **Act:** P0 plays 骰子已经掷下. P1 counters with 网络链接异常 (no target,
  so it is negated).
* **Expect:**
  * Then P0's 武道馆 *does* offer P1 the 宣战布告 window.

### C16. 无法将视线移开 answers a counter
* **Cards:** 登上武道馆 → 宣战布告 → Mujica:无法将视线移开. 2 players.
* **Act:** P1 counters 武道馆 with 宣战布告. In the round on 宣战布告, P0
  plays 无法将视线移开 and chooses forward 2.
* **Expect:**
  * P1 moves 2 tiles and settles there.
  * Then 宣战布告 resolves.
  * Then 武道馆 resolves.

### C17. 再次牵起手来's [持续] vs a card-forced payment
* **Cards:** Mor:再次牵起手来 + AG:宣战布告 vs 登上武道馆. 3 players.
* **Setup:** P1 already has 再次牵起手来 on its field (placed earlier via its
  [反击] after P0 paid rent). P1 also holds 宣战布告.
* **Act:** P0 plays 武道馆. P1 counters with 宣战布告.
* **Expect:**
  * P0 pays P1 500.
  * P1's 1000 payment to P0 is cancelled by 再次牵起手来, which goes to P1's
    discard.
  * P2 pays 1000.

### C18. 三全音 vs a card-forced payment
* **Cards:** Mor:迷茫之蝶们的三全音 vs 登上武道馆. 2 players.
* **Act:** P1 counters P0's 武道馆 payment with 三全音. With 2 players, X =
  ceil10(2000÷1) = 2000.
* **Expect:**
  * P1 immediately gains 2000, then pays 2000. Net 0 now.
  * The card is on P1's field with 3 crystals.
  * After 3 of P1's turn ends it is discarded, and P1 pays 2000.
* **RULING:** who receives that payback, the bank or the original payee.

### C19. （小白） vs a card-forced payment
* **Cards:** Mor:（小白） (P1 is 仓田真白) vs 登上武道馆. 2 players.
* **Expect:** With 2 players, X = 2000.
  * P1 loses 2000 instead of paying it, so P0 receives nothing from P1.
  * P0 loses 1000.
  * P0 = 9000; P1 = 8000.
* **Doc note:** an earlier draft assumed X = 1000. The implemented test
  (`c19_shiroko_vs_card_payment`) asserts P0 = 9000 and P1 = 8000, the
  sheet's reading. It was verified against the sheet and passes.

### C20. 主唱太拼命了 plus a halving modifier
* **Cards:** CRYCHIC:主唱太拼命了 + AG:（摩卡）0.5倍速. 2 players.
* **Setup:** P0 is 青叶摩卡 with 0.5倍速 on its field. P1 owns tile `t`,
  whose rent `R` is between 5000 and 9999.
* **Act:** P0 lands on `t`.
* **Expect:** 0.5倍速 halves the payment to R/2.
* **RULING:** whether 主唱太拼命了 (「一次性…5000以上」) checks the pre- or
  post-modifier amount. Assert that the window is offered only when R/2 ≥
  5000, or record what happens.

### C21. Here the world catches a second card in one turn
* **Cards:** Sumimi:Here the world vs any two plays. 2 players.
* **Act:** P0 plays R:[衍生] 压, then 10次招募. P1 counters the second play
  with Here the world.
* **Expect:**
  * The card sits on P0's field.
  * P0's next draw goes face-down onto it with 3 crystals.
  * At each of P0's turn starts it loses a crystal.
  * At 0 the drawn card joins P0's hand, and P1 draws 1.

### C22. （真奈）歌唱大赛5连冠 with nobody joining
* **Cards:** Sumimi:（真奈）歌唱大赛5连冠 (P1 is 纯田真奈) vs 登上武道馆. 3
  players.
* **Act:** P1 counters. Nobody else plays a counter in its window.
* **Expect:**
  * P1 draws 1.
  * 武道馆 resolves normally.

### C23. （真奈）歌唱大赛5连冠 with a joiner
* **Setup:** As C22, but P2 holds 宣战布告 and plays it in 真奈's window.
* **Expect:**
  * P2 is offered 宣战布告.
  * Record how its self-effect is redirected.
* **RULING:** the redirection of self-effects (「针对打出玩家自身的效果改为你」).

### C24. 花园多惠 (2) cancels a [手] effect with 4 fire
* **Cards:** 花园多惠 skill + 登上武道馆. 2 players.
* **Setup:** P1 is 花园多惠 with 4 fire.
* **Act:** P0 plays 武道馆.
* **Expect:**
  * P1 may spend 4 fire to cancel it: nobody pays X.
  * P1 +500, P0 +2000.
  * P1's fire = 0.

### C25. 花园多惠 (2) vs a skill's [主] effect
* **Cards:** 花园多惠 vs 户山香澄 (2).
* **Act:** P0 (香澄, 1 fire, owns a tile) uses (2) to teleport. P1 (多惠, 4
  fire) cancels it.
* **Expect:**
  * P0 does not move.
  * P0 +2000; P1 +500.
  * P0's fire spent: **RULING**.

### C26. Card immunity vs a crystal mover
* **Cards:** PP:不要背负期待 (「此卡不受任何其他效果影响」) vs
  Mujica:会被骗着买水晶的人. 2 players.
* **Setup:** P1 has 不要背负期待 on its field. P1 also has a crystal-holding
  card with crystals.
* **Act:** P0's 会被骗着买水晶的人 cannot choose 不要背负期待 as the source or
  destination.
* **Expect:** Only legal cards are offered.

### C27. 欢迎来到ave mujica的世界 (2) on a state switch
* **Cards:** Mujica:欢迎来到ave mujica的世界 vs a state switch. 3 players.
* **Setup:** P1 is an Ave Mujica character. P0 plays (1) to switch P1's
  state. P2 holds 欢迎来到 and declares (2).
* **Expect:**
  * P2 and every player who switched state this turn switch once more.
  * P1 is back where it started.
* **RULING:** whether a player with no state 2 is unaffected.

---

## 2. Moves and counter-moves (`rb_cross_move.rs`)

### M1. Y.O.L.O works only on your own roll
* **Cards:** AG:Y.O.L.O. 2 players.
* **Setup:** P0 and P1 both hold Y.O.L.O.
* **Act:** P0 rolls a loaded 5.
* **Expect:**
  * Only P0 is offered Y.O.L.O.
  * P0 plays it with 1d4 = 3 and moves 8.
  * P1's copy is never offered on P0's roll.

### M2. Two counters on one roll from the same player
* **Cards:** AG:（绯玛丽）如果并非没问题 + Y.O.L.O. P0 is 上原绯玛丽.
* **Act:** P0's main roll is a loaded 3. The ring is P1, then P0. P0 declares
  one card. The ring returns and P0 declares the other.
* **Expect:**
  * The second declaration is possible only on a new visit.
* **RULING:** whether 绯玛丽's 「小于6」 is checked at declaration (the move
  is always 3 + 1 + 1d4) or at resolution (with newest first, a Y.O.L.O that
  resolves first may push the roll ≥ 6 and void the +1).

### M3. 若宫伊芙 (2) before the roll, Y.O.L.O after
* **Skills/cards:** 若宫伊芙 (2) + Y.O.L.O. P0 is 若宫伊芙 with 5 face-up fans.
* **Act:** Before the roll, P0 flips Y = 2 fans for +2d4 (loaded 1, 1). The
  d20 shows 4. Then Y.O.L.O adds 1d4 = 2.
* **Expect:**
  * Move = 4 + 2 + 2 = 8.
  * Fans: 3 face-up and 2 face-down.

### M4. 不要背负期待 −2 plus Y.O.L.O
* **Cards:** PP:不要背负期待 (P0's field) + Y.O.L.O.
* **Act:** The main roll is 5. Y.O.L.O adds 1d4 = 1.
* **Expect:** The move is 5 + 1 − 2 = 4.
* **Edge case:** with a roll of 1 and no Y.O.L.O, the result is clamped to 0
  and P0 does **not** settle.

### M5. Repaint plus （摩卡）0.5倍速
* **Cards:** RAS:Repaint (P1) vs a main move + （摩卡）0.5倍速 (P0 is 青叶摩卡,
  with the card on its field). 2 players.
* **Setup:** P1 owns 2 tiles on P0's expected path.
* **Act:** P0 rolls 10.
* **Expect:**
  * P1 is offered Repaint after P0's move roll.
  * The move is reduced by X = 2.
  * The settle payment is halved twice (Repaint and 0.5倍速).
* **RULING:** whether 0.5倍速 halves before or after Repaint's −X, which gives
  4 or 3 tiles.

### M6. 若能再次交汇 and its 20-tile cap
* **Cards:** MyGO:若能再次交汇. 2 players.
* **Setup:** P0 is at 0 and P1 is at 30.
* **Act:** P0 rolls 3, then plays the card. The loaded re-rolls are 5, 5, 5,
  5, 5.
* **Expect:**
  * P0 keeps rolling until it has passed P1 (index 30) or gone more than 20
    past the original end at 3, whichever comes first.
  * Here that is 3 + 5k > 23 → the cap applies. Assert P0 stops by the cap
    rule.
  * Run a second variant with P1 at 10: P0 stops on passing P1.

### M7. 安可 vs 祥，移动
* **Cards:** Mujica:祥，移动 vs 通用:安可. 2 players.
* **Act:** P1 forces P0 to move 3 forward and settle. P0 counters with 安可.
* **Expect:** P0's position is unchanged, and P0 does not settle.

### M8. （兰）像往常一样 undoes a forced move at turn end
* **Cards:** 祥，移动 vs AG:(兰) 像往常一样. P1 is 美竹兰.
* **Act:** On P0's turn, P0 forces P1 3 tiles onto P0's owned tile, where P1
  pays rent. P1 counters with 像往常一样.
* **Expect:**
  * Before that turn ends, P1 is back at its start tile and every effect it
    received is cancelled: the rent is refunded and nothing settles.
* **RULING:** whether "取消所有受到的效果" refunds money already paid.
  Assert the position, and record the money.

### M9. RAS band (1): only the first abnormal-move effect of the turn
* **Skills/cards:** RAS band (1) + 祥，移动 + 雨啊.
* **Setup:** P0 is a RAS character.
* **Act:** On P1's turn, P1 plays 祥，移动 on P0, then 雨啊 designating P1 and
  P0 (loaded 2d2 = 1, 1).
* **Expect:**
  * P0 may decline the second effect: P0 was moved 3 but has no [停留].

### M10. 白金燐子 (2) while stunned
* **Skills:** 白金燐子 (2) while stunned.
* **Setup:** P0 is 白金燐子 with 2 fire and 1 [晕眩].
* **Act:** P0 uses (2) with X = 2.
* **Expect:**
  * P0 moves exactly 12, despite the stun (「可在[晕眩]状态下使用」 and
    「本次移动不受异常移动效果影响」).
  * fire = 0.

### M11. 白金燐子 (2) plus Y.O.L.O
* **Act:** P0 fixes the result to 12, then plays Y.O.L.O (+1d4 = 2).
* **RULING:** whether 「固定为X*6」 overrides later additions. Assert 12 or 14
  and record.

### M12. 凑友希那 (3) stop-pot vs 安可
* **Skills/cards:** 凑友希那 (3) stop-pot vs 安可.
* **Setup:** P0 is 凑友希那, standing on RiNG 3 (36) with 1 fire.
* **Act:** P1 walks past 36. P0 spends 1 fire to force P1 to stop there.
* **Expect (variant a):** P1 stops at 36 and settles.
* **Expect (variant b):** P1 counters with 安可 (强制停下 is abnormal). P1
  continues to its rolled destination, and P0's fire is spent: **RULING**.

### M13. [不可阻挡] may refuse a forced stop
* **Skills/cards:** 凑友希那 (3) vs PPP:（里美）我的心就像巧克力螺's [不可阻挡].
* **Act:** P1 (牛込里美) moves up to 4 tiles via the card, passing P0. P0 tries
  to stop P1.
* **Expect:** P1 is offered whether to accept the stop, per 51. Choose
  refuse: P1 reaches the chosen tile.

### M14. （香澄）大家我都喜欢哦 on the hill, with 安可
* **Setup:** P0 is 户山香澄 (PPP) with the card on 44. P1 is a non-PPP player.
* **Act:** P1 rolls past 44 to 48.
* **Expect (variant a):**
  * P1 is forced to stop at 44.
  * The card goes to P0's discard, and P0's band card gets +1 crystal.
  * P1's rent at 44 is half the normal rent. Under the PPP band (3), it is
    split among the PPP players.
* **Expect (variant b):** P1 answers the forced stop with 安可. P1 reaches 48,
  and the card stays on 44.

### M15. The first forced stop on the path wins (rule 86)
* **Cards:** （香澄） card on 44 + MyGO:（乐奈）有趣的女人 on 48 with 5 crystals.
* **Act:** P1 rolls from 40 to 50.
* **Expect:**
  * P1 stops at 44.
  * 48 is never passed: the 乐奈 card's crystals and the 抹茶芭菲 choice are
    unchanged.

### M16. live前的准备 shortens the path past Bandori车站 (rule 86)
* **Cards:** R:live前的准备 vs RAS:（LOCK）追逐梦想的步伐 (2). P0 is 朝日六花,
  with LOCK on its field.
* **Act:** P0 rolls from 25 to 38, which would pass 30 and 35. At 30, P0 plays
  live前的准备.
* **Expect:**
  * P0 stops at 30 and settles (draws a card).
  * 35 was not passed, so LOCK stays on the field and P0 is not sent to 52.

### M17. LOCK (2) on its own
* **Act:** As M16, without live前的准备.
* **Expect:**
  * P0's 终点 becomes 52 before settling.
  * LOCK is removed.

### M18. 普通与理所当然 after a forced move
* **Cards:** MyGO:普通与理所当然 after 祥，移动.
* **Setup:** P0's last non-teleport main move was 7.
* **Act:** On P1's turn, P1 forces P0 3 tiles. P0 counters afterwards with
  普通与理所当然.
* **Expect:** On P0's next turn, with a loaded d20 of 15, P0 moves 7.

### M19. 你的光芒将照亮前路 plus Y.O.L.O
* **Setup:** P0 is at 20, within 20 of 13.
* **Act:** P0 plays the card, rolls 4, and plays Y.O.L.O (+1d4 = 2).
* **Expect:** The walk starts from 13 and ends at 19. 月之森 itself does not
  settle as the start.

### M20. Backwards past CiRCLE: 仓田真白 vs 美竹兰
* **Variant a:** P0 is 仓田真白, with a loaded 2d20 = 3 + 3, starting at 4.
  * P0 walks backwards past CiRCLE to 58.
  * P0 **does** get the CiRCLE reward (96; the start is not CiRCLE).
* **Variant b:** P0 is 美竹兰 and uses (2) to go backwards past CiRCLE.
  * P0 gets **no** reward (the skill's text says so).

### M21. 宇田川巴 (2): shifting the start toward the ramen shop
* **Setup:** P0 is 宇田川巴 at 45 with 1 fire.
* **Act:** P0 uses (2). 49 is closer forward, so the start shifts 10 forward,
  past 49, to 55.
* **Expect:**
  * The fire is spent and refunded: 「超过了…补充一个火罐」.
  * The move then starts from 55.

### M22. MyGO band (2) move-1 vs CRYCHIC:想要成为人类
* **Setup:** P0 is a MyGO character whose band card has 1 crystal, with
  想要成为人类 (X = 10) on its field.
* **Act:** P0 replaces its roll with a 1-tile move.
* **Expect:**
  * P0 moves 1.
  * 想要成为人类 gains no crystal (there was no move roll).
  * The band crystal is spent.

### M23. 想要成为人类's boosted move hit by an abnormal effect
* **Setup:** P0 has 想要成为人类 (X = 15, 2 crystals) on its field.
* **Act:** At P0's turn start the crystals clear, and the next roll gets +5.
  During that move, P1 (凑友希那, 1 fire, on a RiNG on the path) forces P0 to
  stop.
* **Expect:**
  * P0 stops at the RiNG.
  * The card gains 2 crystals.

### M24. 人偶的箱庭
* **Cards:** Mujica:人偶的箱庭. 3 players.
* **Act:** P1 chooses to roll 6 and move (no settle). P2 chooses to pay X×20,
  where X = its normal distance to P0. Then P0 is forced to move 6.
* **Expect:** This counts as P0's main move. Assert the positions and P2's
  payment.

### M25. 人偶的箱庭 with a player who can't move
* **Setup:** As M24, but P2 has [停留].
* **Expect:** P2 must pay.

### M26. 一人两个甜甜圈
* **Cards:** Sumimi:一人两个甜甜圈. 2 players.
* **Setup:** P0 is at 10.
* **Act:** P0 plays it and gets [除外].
* **Expect:**
  * On P1's turn P1 walks from 5 past 10 to 15.
  * After P1 settles, P0 may teleport to any tile from 11 to 15 and settle.
  * Both players gain 2 fire, or 500 per pot over the cap.

### M27. （祥子）带领着大家 (CRYCHIC)
* **Setup:** P0 is 丰川祥子（CRYCHIC） on the same tile as P1.
* **Act:** P0 plays the card at turn start, then rolls 6.
* **Expect:**
  * P0 and P1 both move 6.
  * P0 settles first, then P1.
  * Each settlement's payment is halved: put P0 on P2's tile, so the rent paid
    is R/2.

### M28. 是我自己的问题 vs a forced move
* **Cards:** CRYCHIC:是我自己的问题 vs 祥，移动.
* **Act:** P0's main move ends next to P1. P0 plays the card: P0 moves 1 tile
  further away from P1.
* **Variant:** P0 was force-moved this turn, which is an abnormal effect. The
  card has no effect, per 「若受到[异常移动效果]影响，此卡不生效」.

### M29. Who gets the CiRCLE reward
* **Setup:** 4 players.
  * P0 is 户山香澄 (PPP band (2): no reward).
  * P1 is 弦卷心 ((2): +1500 extra).
  * P2 is 丸山彩 (PP band (3): no reward) with FEVER! on its field.
  * P3 is vanilla.
* **Act:** Each walks past CiRCLE from a non-CiRCLE start.
* **Expect:**
  * P0: no reward prompt.
  * P1: reward + 1500.
  * P2: no reward, so FEVER! adds nothing.
  * P3: the normal reward.

### M30. 不可阻挡 walks through a [停留]
* **Setup:** P0 is 三角初华 in state 2 ([不可阻挡]) with 1 [停留].
* **Expect:** P0 still rolls and moves (51).

---

## 3. Connected tiles, their effects and counter-effects (`rb_cross_tiles.rs`)

### T1. Anon Tokyo links two tiles
* **Cards:** MyGO:[千早爱音]Anon Tokyo.
* **Setup:** P0 is 千早爱音, owns 1 and 2, and stands on 1.
* **Act:** P0 links 1–2.
* **Expect:**
  * P0 spends max(P(1), P(2)) / 2.
  * P1 lands on 1 and pays R(1,0) + R(2,0)/2 to P0.
  * A second Anon Tokyo on the same pair adds no second link (上限1).

### T2. Anon link plus Fire bird
* **Setup:** As T1, and P0 also has R:Fire bird on its field (1.5×).
* **Expect:** P1 pays 1.5 × (R(1) + R(2)/2).
* **RULING:** whether the linked half uses the base rent or the boosted rent,
  1.5×R(2)/2. Assert at least 1.5 × R(1), and record.

### T3. Anon link plus Repaint
* **Act:** P1's settlement on a linked tile is halved by Repaint, which P0
  played on P1's roll.
* **Expect:** The total is (R(1) + R(2)/2) / 2.

### T4. Anon link with a mortgaged partner tile
* **Setup:** Tile 2 is mortgaged.
* **Expect:** Landing on 1 charges R(1) plus 0, because a mortgaged tile
  collects nothing.
* **RULING:** whether the link still adds half of 2's "收费" when 2 is
  mortgaged.

### T5. ONE OF US shares settlement income
* **Cards:** AG:ONE OF US.
* **Setup:** P0 is Afterglow and owns 47. P1 owns 48 (商店街 colour).
* **Act:** P0 designates its 47 and P1's 48.
* **Expect:**
  * P2 lands on 48 and pays R(48). P1 receives it, then P0 gets half from P1:
    「先在原主人方结算完成，之后被分享方资金直接增加」.
  * A FEVER! on P0's field does **not** boost the shared half (「不受其他任何效果
    影响」).

### T6. Tomorrow's Door's surcharge
* **Cards:** PPP:Tomorrow's Door.
* **Setup:** The card is in P0's play area, and P0 owns tiles with 2 houses
  on 44.
* **Act:** P1 settles on P0's tile.
* **Expect:**
  * P1 pays R + 200.
  * Also when P1 settles on 梦开始的地方 (41).

### T7. Tomorrow's Door walks its sequence
* **Act:** P0 passes the tile the card is on: it moves to the next tile in the
  sequence (流星堂 → 花咲川 → Space → …).
* **Expect:** After 大阪中之岛公园, it goes to P0's play area.

### T8. 练习室里的风暴 forces a remote settle
* **Cards:** RAS:练习室里的风暴.
* **Setup:** The card is on P0's livehouse 50 with 2 crystals.
* **Act:** P1 settles on 52 (X = 2).
* **Expect:**
  * The card goes to the discard.
  * P1 also settles 50, at (4 − 2) / 4 of the normal rent.
  * With X = 3 (more than the crystals), nothing happens.

### T9. Change the world
* **Cards:** RAS:Change the world.
* **Setup:** P0 owns 50 with 1 house. The card goes on it, and P0's roll
  becomes 3d20.
* **Act:** P0 passes 2 livehouse tiles it doesn't own (y = 2).
* **Expect:**
  * The next charge on 50 is R(50,1) + 50 × 3 × 1.
  * P0 also gains that same extra.
  * The card goes to P0's discard.

### T10. Ringing Bloom raises house counts
* **Cards:** R:（燐子）Ringing Bloom.
* **Setup:** P0 is 白金燐子 and owns 1 (3 houses) and 2 (0 houses).
* **Act:** P1 lands on 2.
* **Expect:**
  * Tile 2 is treated as having 3 houses, with the rent halved: R(2,3)/2.
  * After that non-RiNG collection the card goes to the discard, and P0 gains
    500 × 3.

### T11. Fire bird plus HHW band (1) doubling
* **Setup:** P1 is an HHW character landing on P0's tile while P0 has Fire bird.
* **Act:** P1 chooses double.
* **Expect:**
  * P1's band card gets 1 crystal.
  * P0 is recorded.
* **RULING:** the order of 1.5× and 2×; both orders give 3×R. Assert
  3 × R(t) and the crystal.

### T12. 学生会的检查 on a tile
* **Cards:** R:学生会的检查.
* **Setup:** P0's move ends within 3 tiles of its owned tile `t`.
* **Act:** P0 plays the card before settling and pays H(t)/2 to place it on
  `t`. P0 can't build this turn, and a 压 is added to P0's draw pile.
* **Expect:**
  * Next, P1 walks past `t` (end ≠ t): P1 is forced to stop at `t` and pays
    R(t)/2.
* **Variant:** P1 counters the forced stop with 安可. P1 continues, and the
  card stays.

### T13. 笑容大游行 swaps a tile with 弦卷集团
* **Cards:** HHW:笑容大游行.
* **Act:** P0 passes 28 and places the card there with 3 crystals. That move's
  终点 counts as the 弦卷集团 agent.
* **Expect:**
  * Later, after P0 settles at its end tile `e`, P0 moves the card onto `e` and
    clears its crystals.
  * `e` now counts as swapped with 28.
  * The next settle on `e`, by anyone, discards the card.
* Assert the placement, the crystals, and the discard. Record the swap
  behaviour.

### T14. 黑衣人的补给 turns 弦卷集团 into a CiRCLE
* **Cards:** HHW:黑衣人的补给.
* **Act:** P0 passes CiRCLE and counters: 28 gets a crystal.
* **Expect:**
  * P1 passing 28 gets the CiRCLE reward.
  * P0 passing 28 removes the crystal.
* **Cross:** a PPP-band player passing 28 while it has a crystal still gets no
  reward.

### T15. （kkr）前往笑容集结的地方！ on CiRCLE, plus 爱心义演
* **Setup:** P0 is 弦卷心 and pays 10000 to put the card on CiRCLE.
* **Expect:**
  * P1 settles at CiRCLE, i.e. ends its move there, and pays P0 6000.
  * P0's 爱心义演 counts CiRCLE as P0's tile, so P0 passing CiRCLE adds +2 to
    its move.

### T16. （soyo）混合的颜色 and an agent tile
* **Setup:** P0 is 长崎素世. The card is on P0's tile `s`, which now counts as
  every colour.
* **Act:** P1 settles on another colour's agent tile and chooses `s`.
* **Expect:** P1 pays R(s)/2/2. A normal agent half-rent on a non-soyo tile is
  R/2.

### T17. 那天的雨 colour zone
* **Cards:** MyGO:那天的雨.
* **Act:** A loaded 1d10 = 1 selects the first agent's colour (excluding
  东京外).
* **Expect:**
  * Players on that colour's tiles **and their adjacent tiles** gain 1 [停留].
  * Players elsewhere don't.
  * Then, each of P0's turn ends removes 1 crystal.

### T18. （乐奈）有趣的女人 builds up, then stops someone
* **Cards:** MyGO:（乐奈）有趣的女人.
* **Setup:** The card is on tile `t`.
* **Act:** Players pass `t` without settling there 5 times, so it has 5
  crystals. Then P1 passes `t`.
* **Expect:**
  * P1 chooses: lose a 抹茶芭菲, or stop.
  * With no 芭菲, P1 must stop. It settles `t`, paying half, and the card is
    shuffled into the discard.

### T19. （沙绫）总有一天要给这片天空命名 relocates itself
* **Act:** P0 (山吹沙绫) places the card on 48. P0 passes 48.
* **Expect:**
  * At turn end P0 gains 1 fire.
  * The card moves to the tile given by a loaded 3d20 (e.g. 5 + 5 + 5 → tile
    #15 = index 14).

### T20. 该清CP了 CP points spread
* **Cards:** 通用:该清CP了.
* **Act:** P0 puts a CP point on tile `t`, with 6 on P0's field.
* **Expect:**
  * At P0's next 2 turn starts, a CP point spreads to the adjacent CP-less
    tiles.
  * P0 settling on a CP tile removes the tile's CP point and 1 field point,
    and gains 800.

### T21. （立希）想认真去做 plus an Anon link
* **Setup:** P1 has [停留] and stands next to P0's linked tile 1.
* **Act:** P0 (椎名立希) plays the card and chooses tile 1 for P1.
* **Expect:** P1 settles there and pays (R(1) + R(2)/2) / 4.

### T22. 星之鼓动山丘 co-ownership and Tomorrow's Door
* **Setup:** P0 and P1 are PPP. P2 is not.
* **Act:** P2 settles on 44.
* **Expect:**
  * The rent is split between P0 and P1 (PPP band (3)).
  * If P0 also has Tomorrow's Door in its area: whether 44 counts as P0's
    owned tile for the surcharge is a **RULING**.

### T23. 狂乱Hey Kids!! replaces a settlement
* **Cards:** RAS:狂乱Hey Kids!!.
* **Setup:** P0 owns `t` with 1 house and an empty buildable tile `u`.
* **Act:** P0 settles on `t` and counters.
* **Expect:**
  * The house moves to `u`.
  * The money follows the card's formula. Record it; the formula is in
    TEST-FINDINGS.

### T24. Parking Space plus 要乐奈 (2)
* **Setup:** P0 is 都筑诗船 and has placed Parking Space on Space (42).
* **Act:** P1 (要乐奈, 3 fire) uses (2) to teleport to 42.
* **Expect:**
  * Settling there is replaced by 1 [停留] at P1's turn end.
  * While P1 is on 42, P1 cannot use character or band skills.

### T25. 高贵的微蓝 vs 即使夕阳落山
* **Setup:** P0 has used 高贵的微蓝 on its own tile `t`, so `t` is marked.
* **Act:** P1 plays AG:即使夕阳落山 designating its own tiles. Only the user's
  own tiles are designated, so this is no conflict.
* **Expect:** Assert instead that a tile-designating effect can't pick `t`.
  Use Mujica:无路矢 by P1, designating P0's tile while P0 stands on the marked
  `t`: `t` must not be offered.

### T26. 可爱又强壮的花朵 stops only its user
* **Cards:** PP:可爱又强壮的花朵.
* **Setup:** The card is on P0's tile `t`.
* **Expect:**
  * P1 walking past `t` is not stopped.
  * P0 walking past `t` is forced to stop and settle there, and the card goes
    to P0's discard.

### T27. （育美） marks
* **Cards:** HHW:（育美）.
* **Setup:** A mark is on tile `t`.
* **Expect:**
  * P0 passing `t` may stop there and gain 2000; the mark is removed.
  * P1 passing `t` is not offered anything.

### T28. 花园多惠 (1) rabbits
* **Act:** P0 (花园多惠) takes the CiRCLE reward. A loaded 3d20 = 2 + 2 + 2
  puts a rabbit on #6, index 5.
* **Expect:** P0 later passes 5: the rabbits are removed, and P0 gains that
  much fire (cap 4).
* **Variant:** a 3d20 result pointing at 星空齿科 (#7) places nothing.

### T29. 北泽精肉店 croquettes (北泽育美 (1))
* **Act:** A croquette spawns on 51 each turn. P1 passes 51 and takes it.
* **Expect:** P1 pays P0 50 × (P1's croquettes).

---

## 4. Status interplay (`rb_cross_status.rs`)

### S1. An exiled player can't be designated
* **Setup:** P1 is exiled. 3 players.
* **Act:** P0 plays 武道馆.
* **Expect:** P1 is not designated (50), so only P2 pays.
* **RULING:** whether X counts P1 as alive (it does: 存活 = not bankrupt).
  Assert X = ceil10(2000÷2).

### S2. A stunned player can't pay
* **Setup:** P1 is stunned.
* **Act:** P0 plays 武道馆.
* **Expect:** P1 pays nothing (「无法收付款」).

### S3. A stunned player can't receive rent
* **Setup:** P1 is stunned, and P0 lands on P1's tile.
* **Expect:** P0 pays nothing.

### S4. 初演大成功 keeps money from falling
* **Cards:** CRYCHIC:初演大成功.
* **Act:** P0 plays it, then lands on P1's tile.
* **Expect:**
  * P0's money doesn't fall.
  * P1 still receives R: **RULING**. Record it.
  * At turn end P0 gains 1 [眩晕] and its band card gets +1 crystal.

### S5. CRYCHIC band (1) at 6 or more hand cards
* **Setup:** P1 is CRYCHIC with 6 hand cards.
* **Act:** P0 plays 武道馆.
* **Expect:**
  * P1 neither pays nor receives.
  * P1 can't play a hand card.
  * On a CiRCLE pass, P1 must take the draw.

### S6. 壱雫空 clears stays and stuns
* **Cards:** MyGO:壱雫空.
* **Setup:** P1 has [停留] and P2 has [晕眩]. P0 is stunned too.
* **Act:** P0 plays it, which is allowed while stunned.
* **Expect:** All stays and stuns are cleared.
* **RULING:** who pays what.

### S7. 椎名立希 (1) counts stays from other players' cards
* **Setup:** P0 is 椎名立希 with 0 fire.
* **Act:** P1 plays 雨啊 designating P1 and P2.
* **Expect:** P0 gains 2 fire, one per layer granted.
* **Cross:** a stay from MyGO:轮符雨 on P1 also adds 1.

### S8. 丰川祥子 (1) absorbs statuses as she passes
* **Setup:** P0 is 丰川祥子 in state 1. P1 has 2 [停留].
* **Act:** P0 walks past P1.
* **Expect:**
  * P0 may take P1's stays: P1 = 0, P0 = 2, P0 +3000.
  * P0's fire +2, one per layer received.

### S9. Ave Mujica band at 1.5× plus （小白）
* **Setup:** P0 is an Ave Mujica character in state 2, paying rent R to P1.
* **Act:** P0 counters with （小白）. P0 needs to be 仓田真白 for that, so use
  Mujica:（睦/mortis） to copy the card, or record it as not testable.
* **Expect:** The amount P0 loses is 1.5 × R, and P1 loses half of that.

### S10. 若麦 state 1 raises payments to her
* **Setup:** P1 is 祐天寺若麦 in state 1 with 2 fire.
* **Act:** P0 pays P1 rent R.
* **Expect:**
  * P1's fire becomes 3, and the payment rises by 3 × 100.
  * The increase is X×100 with X = the fire held after the gain: **RULING**.
    Record which.

---

## 5. Long chains with in-chain interactions (`rb_cross_long.rs`)

Each long chain asserts **every intermediate step**: the order of the asks,
which link each window answers (the prompt detail), the order bodies resolve
in, and the money / position / hand after each resolution where the harness can
observe it. Don't assert only the end state.

### L1. A counter war five links deep
* **Players:** 4, `vanilla(4)`.
* **Setup:**
  * Hands: P1 = 宣战布告; P2 = 宣战布告, 网络链接异常; P3 = 网络链接异常;
    P0 = 登上武道馆, 网络链接异常.
  * Each draw pile holds ≥ 3 fillers.
* **Chain:**
  1. P0 plays 登上武道馆 (X). X = ceil10(2000÷3) = 670.
  2. **Round on X.** The asks go P1, P2, P3, P0, P1, … until a full quiet lap.
     * P1 declares 宣战布告 (A).
     * P2 declares 宣战布告 (B).
     * P3 passes, P0 passes, P1 has nothing left, P2 passes: closed.
     * Assert that P2 was asked right after P1 declared (priority passes on).
  3. **Counters to counters, newest first: round on B** (asks start at P3).
     * P3 passes.
     * P0 declares 网络链接异常 (C) answering B. B is a [手] with a [指定], so
       C cancels B's designation of P0.
  4. **Round on C** (asks start at P1). P2 declares 网络链接异常 (D) answering
     C. C is a [手] with no [指定], so D negates all of C.
  5. **Round on D:** quiet.
  6. **Round on A** (asks start at P2). P3 declares its 网络链接异常 (E) answering
     A, which cancels A's designation of P0.
* **Resolution:** post-order, newest first.
  * D resolves: C is negated, so C's body never runs.
  * B resolves: P0 pays P2 500, and P2 draws 1.
  * E resolves: A's designation of P0 is cancelled.
  * A resolves: P0 pays nothing. Whether P1 draws is a **RULING** (see C3).
  * X resolves: P1, P2 and P3 pay P0 670 each.
* **Expect:**
  * P0 = 10000 − 500 + 2010 = 11510.
  * P1 = 9330, P2 = 9830, P3 = 9330.
  * Every declared card is in its owner's discard. C is spent even though its
    body was negated.

### L2. One walk through several pass effects
* **Players:** 3.
* **Setup:**
  * P1 is 户山香澄, with （香澄）大家我都喜欢哦 on 44.
  * P2 is 要乐奈, with （乐奈）有趣的女人 on 48 holding 5 crystals.
  * P0 is vanilla at 38. P0 holds 通用:安可 and PPP:仓库里的Random Star
    (already played earlier, so on P0's field), plus 2 星星贴纸 tokens.
  * P0 has no 抹茶芭菲.
  * P2 is also 凑友希那-free. Keep it simple: no other passers.
* **Chain:**
  1. P0 rolls a loaded 12, so the path is 39 → 50.
  2. **At 44.** 香澄's card forces P0 to stop. P0 answers the 强制停下 with 安可:
     the stop and everything it would cause are void, so the card stays on 44,
     P1's band card gets no crystal, and nothing settles at 44.
  3. **At 45 (流星堂).** Random Star (2) offers to spend 2 stickers to stop and
     settle. P0 declines.
  4. **At 48.** 乐奈's card has ≥ 5 crystals, so P0 must choose: lose a 抹茶芭菲
     (it has none), or stop. P0 stops. The path is recomputed (rule 86), so 49
     and 50 are not passed. P0 settles 48 at half price (「由此卡效果导致
     [触发结算]时需支付资金减半」), and the card is shuffled into P2's discard.
* **Expect:**
  * P0 ends on 48.
  * P0 pays R(48)/2 to its owner. Arrange that P1 owns 48.
  * The 44 card is still on 44.
  * P0 still has 2 stickers.
  * 安可 is in P0's discard.

### L3. A rent payment through every modifier
* **Players:** 3.
* **Setup:**
  * P1 is 千早爱音. P1 owns 1 and 2, linked by Anon Tokyo. P1 has R:Fire bird
    and 通用:[衍生]FEVER! on its field.
  * P0 is 青叶摩卡, with （摩卡）0.5倍速 on its field. P0 holds
    Mor:迷茫之蝶们的三全音 and CRYCHIC:主唱太拼命了.
* **Chain:**
  1. P0 lands on 1. The base charge is R(1) + R(2)/2 (the link).
  2. Fire bird multiplies it by 1.5.
  3. 0.5倍速 halves P0's payment.
  4. FEVER! adds X to what P1 receives. P1 has 2 field cards, so X = 600 − 2×200
     = 200.
  5. P0's [反击] window opens on the payment. P0 declares 三全音: it gains the
     final amount now, and the card goes to its field with 3 crystals.
     主唱太拼命了 is offered only if the amount is ≥ 5000.
* **Expect:**
  * Assert the windows offered and 三全音's placement.
  * Assert that P0's net change now is 0.
  * Assert that P1's receipt = the pipeline result + 200.
* **RULING:** the order of the add and multiply modifiers, and whether the
  linked half is boosted. Assert the amounts the sheet fixes; record the rest.

### L4. A forced move into a remote settle on a linked tile
* **Players:** 3.
* **Setup:**
  * P0 is 千早爱音. P0 owns 49 and 50, linked. RAS:练习室里的风暴 sits on 50
    with 2 crystals, and P0 holds Mujica:无法将视线移开.
  * P1 is at 48 and holds 宣战布告.
* **Chain:**
  1. P0 plays 登上武道馆.
  2. P1 counters with 宣战布告 (A).
  3. In the round on A, P0 counters with 无法将视线移开 (B) and forces P1 4
     forward, to 52.
  4. B resolves first. P1 walks 49 → 52 and settles at 52 (unowned; P1
     declines to buy).
  5. 风暴 triggers: X = |52 − 50| = 2 ≤ 2 crystals. The card goes to P0's
     discard, and P1 settles 50 at (4 − 2)/4 of the normal charge.
  6. Then A resolves: P0 pays P1 500, and P1 draws.
  7. Then 武道馆 resolves: P1 and P2 pay.
* **Expect:**
  * Every step's money.
  * P1 is on 52.
  * 风暴 is in P0's discard.
* **RULING:** whether the link's extra half applies inside the scaled
  settlement.

### L5. One skill use, several counteractions
* **Players:** 4.
* **Setup:**
  * P0 is 户山香澄 with 1 fire and owns tile 10.
  * P1 is 广町七深 with 2 fire.
  * P2 is 青叶摩卡 with 1 fire.
  * P3 is 花园多惠 with 4 fire.
* **Chain:**
  1. P0 uses (2): a teleport to its own tile.
  2. The counteraction opportunities go in clause-89 order, P1 → P2 → P3:
     * P1's (2): pay 1 fire for a 香澄 character mark.
     * P2's (2): spend 1 fire to teleport to P0's tile, without settling.
     * P3's (2): spend 4 fire to cancel the use.
* **Expect:**
  * The use is cancelled: P0 doesn't move, P0 gains 2000, and P3 gains 500.
  * P2 is on P0's **pre-skill** tile.
  * P1 holds the mark.
  * Each spender's fire drops by its cost.
* **RULING:**
  * whether P1 still gets the mark and P2 still teleports when the use is
    cancelled;
  * whether P0's fire is refunded.

### L6. One draw meeting several draw effects
* **Players:** 2.
* **Setup:**
  * P0 is 大和麻弥 with 3 face-up fans.
  * P0 has PP:梦在前方，结彩当下 on its field with 0 crystals.
  * P0's draw pile, top first: filler, AG:朝同一片天空迈进, filler.
  * P0's hand is 3 fillers.
  * P1 has already placed Sumimi:Here the world on P0's field (P0 played two
    cards in one turn earlier).
* **Chain:**
  1. P0 takes the CiRCLE reward's draw.
  2. 大和麻弥 (2), Y = 2: look at the top 3 cards and pick 朝同一片天空迈进.
     That counts as a draw.
  3. Here the world wants the drawn card face-down on itself. 朝同一片天空迈进
     auto-plays when drawn: hand ≥ 3, so it pays hand × 600. 梦在前方 gains 1
     crystal for the draw.
* **Expect:**
  * 2 fans are flipped.
  * 梦在前方 has 1 crystal.
  * The two filler cards not picked are shuffled back into the deck.
* **RULING:** whether Here the world captures the card before 朝同一片天空
  can auto-play. Record the observed outcome.

### L7. An extra-turn cascade with decaying field cards
* **Players:** 2.
* **Setup:**
  * P0 has Mujica:燃尽前的线香花火 (2 crystals) and HHW:运动的天赋 (3
    crystals) on its field.
  * P1 holds MyGO:壱雫空.
* **Chain:**
  1. P0 turn A ends. 线香花火 → 1 crystal, and P0 gets an extra turn.
     运动的天赋 → 2.
  2. Extra turn B. P0's roll pays 运动的天赋's 700, then 600 on its re-roll(s).
     It re-rolls until the roll is ≥ 10.
  3. B ends. 线香花火's last crystal goes: card to P0's discard, P0 gains 2
     [眩晕], and P0 gets another extra turn (「…并使你获得一个额外回合」 —
     whether the last removal also grants an extra turn is a **RULING**).
     运动的天赋 → 1.
  4. On P1's turn, P1 plays 壱雫空: P0's stun is cleared. The payments follow
     the ruling on 壱雫空.
  5. P0's next turn: P0 moves normally. 运动的天赋 → 0, so it goes to the
     discard.
* **Expect:**
  * The turn order, A → B → (C?) → P1.
  * The crystal counts after each turn end.
  * The stun before and after 壱雫空.
  * The money from 运动的天赋 per roll.

### L8. Status hand-offs
* **Players:** 4.
* **Setup:**
  * P0 is 丰川祥子 in state 1, with 0 fire.
  * P2 is 椎名立希 with 0 fire.
  * P1 and P3 are vanilla.
* **Chain:**
  1. P1 plays 雨啊 (2d2 = 2 + 1, so X = 3). It must include its user, so it
     designates P1, P3 and P0.
     - P2 gains 3 fire: one per [停留] layer (立希 (1)).
     - P0 gains 1 fire: it received a [停留] (祥子 (1)).
  2. On P0's turn, P0 can't move, because of its [停留].
  3. Variant: P0 instead walks past P1 and P3 and absorbs their stays.
     - Each layer gives P0 +1500 and +1 fire. P0 reaches the cap of 3, so it
       may enter state 2 at its next turn start.
     - On entering state 2, P0 keeps at most 1 hand card and discards the rest.
     - From then on, each turn start auto-plays the top card of the draw pile.
  4. P1 plays 壱雫空 (copy in hand): every stay goes, which affects 祥子's
     transferred stays.
* **Expect:**
  * Fire counts after each step.
  * Money.
  * The state-2 entry and the auto-play of a known top card (put 压 on top:
    +1000).

## 6. Coverage-gap cases

Cases for the **missing** pairs in `docs/rulebook/COVERAGE.md`'s
surface x category-pair matrix. Ids `G01`…, file `rb_cross_gaps.rs`
(or folded into the existing `rb_cross_*.rs` per the surface). Same
conventions as §1–§5: `R(t,h)`, loaded dice, `// 规则书:` quotes,
`RULING` markers where the sheet is silent.

### G1. Money pipeline

#### G01. multiply x multiply: Fire bird 1.5x and Ave Mujica 1.5x on one rent
* **Cards / skills:** `R:Fire bird` (P0 field) + `skill:Ave Mujica:假面之下的真实`
  (P0, state 2) vs a rent on P1's tile.
* **Setup:** P0 owns a tile with a Fire bird; P0 is in Ave Mujica state 2
  (use `world_mut` to set state and crystals). P1 owns tile `t` with 0 houses.
  P0 lands on `t`.
* **Act:** settle; P0 pays P1 `R(t,0)`.
* **Expect (book):** each modifier scales the amount. Sheet does not fix the
  order. Two candidates: `R(t,0) * 1.5 * 1.5` vs a sequential add-then-multiply.
* **RULING:** the order of two multipliers on one amount. Record which the
  engine does; the sheet does not settle it. Assert the money delta either way
  and mark the order in the ignore reason.
* **Loaded dice:** none (landing is arranged by `set_pos`).

#### G02. multiply x clamp-floor: 摩卡 half-pay under 不要背负期待's +100
* **Cards:** `AG:（摩卡）0.5倍速` (P0 field) + `PP:不要背负期待` (P0 field)
  vs a rent of `R(t,h)`.
* **Act:** P0 settles on P1's tile.
* **Expect:** 不要背负期待 (2) raises a [支付] by 100 (pay +100), 摩卡 (2)
  halves every pay. Sheet: 「所有资金支付与消耗减半」 and 「[支付]资金时金额
  提高100」. Composition is unstated.
* **RULING:** is the +100 added before the halving (so `(R+100)/2`) or after
  (`R/2 + 100`)? The sheet's payment-stage description (AddDiff → MultDiff)
  from the ileuxali notes suggests add-then-multiply, i.e. `(R+100)/2`.
  Assert that; mark **RULING**.

#### G03. cancel-one x split-share: 网络链接异常 drops one leg of a 分摊
* **Cards:** `PP:[白鹭千圣]微笑的铁假面` (P0) played against P1+P2+P3, vs
  `通用:网络链接异常` (P2).
* **Act:** P0 plays the exclusive; others [分摊] 2000. P2 counters, cancelling
  its own designation.
* **Expect:** P1 and P3 each pay ceil-share of 2000; P2 pays 0.
* **Note:** currently blocked by `TODO(ABI)` (no static targeting query).
  Write the test anyway and `#[ignore = "TODO(ABI): …"]` it, matching
  `rb_money::per_pair_cancel_drops_one_designation`.
* **RULING:** does X (the share) recompute after the drop? Assert the
  pre-drop share; record the alternative.

#### G04. multiply x cancel-one: a halved rent that then loses one payer
* **Cards:** `RAS:Repaint` (halves the settlement payment) + `通用:网络链接异常`
  on `通用:登上武道馆`.
* **Act:** P0 plays 武道馆 (3 others, X = ceil10(2000/3) = 670). P1 counters
  with Repaint-style halving is not applicable (Repaint is move-only), so use
  `CRYCHIC:（祥子）带领着大家`'s 「触发结算时进行的支付价格减半」 or
  `Mujica:祥，移动#3` instead: force-move a payer onto a rent tile and halve.
* **Simpler arrangement:** `HHW:爱心义演` (halves P0's payments this turn)
  + `通用:网络链接异常` cancelling one of P0's two payers... (the user is the
  payer). Use `Mujica:祥，移动` to force P1 onto P0's tile with the halving
  clause, and `通用:网络链接异常` to drop one of the two forced players.
* **Expect:** the surviving payer pays `ceil10(R/2)`; the cancelled one pays 0.
* **RULING:** the halving is 「触发结算时进行的支付价格减半」 — does it apply
  before or after a cancel-one?

#### G05. replace-loss x split-share: 三全音 against a 分摊
* **Cards:** `Mor:迷茫之蝶们的三全音` (P1) vs `PP:[白鹭千圣]微笑的铁假面`
  played by P0 (others [分摊] 2000).
* **Act:** P0 plays the exclusive; P1 holds 三全音 and counters when the
  分摊 hits.
* **Expect:** 三全音 「立刻获得此次失去的资金金额」 — P1 gains their share and
  does not lose it; the other designated players still pay. Three turns later
  P1 pays the accumulated amount back.
* **Note:** `c18_tritone_vs_card_payment` passes (2026-10-06): the payment
  still happens and the payee receives, so P1 nets 0 now. This adds the
  split-share leg.

#### G06. clamp-floor x split-share: 不要背负期待's +100 on a 分摊 leg
* **Cards:** `PP:不要背负期待` (P1 field, so P1 is the one whose pays rise)
  + `PP:[白鹭千圣]微笑的铁假面` (P0).
* **Expect:** P1's leg of the 2000 分摊 is `ceil(2000/n) + 100`, floored at 0.
  Assert the +100 lands on a split leg.

#### G07. negator x redirect: EXIST retargeting a card that 安可 then negates
* **Cards:** `RAS:EXIST` (P2) + `通用:安可` (P2) vs `PP:找回珍妮弗` (P0 → P1).
* **Act:** P0 targets P1. EXIST retargets to P2. P2 holds 安可 and counters the
  retargeted landing.
* **Expect:** the card does not land on P2's field; P2's EXIST still checks
  「若在此期间此卡没有造成影响」 and P2 draws 1 (it redirected nothing in the
  end).
* **RULING:** does a negated redirect count as "had an effect"? Assert
  "draws 1"; record the alternative.

### G2. Roll

#### G08. dice-set x dice-reroll: 星月夜's re-roll on a 3d20 set
* **Cards:** `Mor:蝴蝶飞舞的星月夜` (P0 field) + `RAS:（MASKING）CRUSH ON THE
  DRUM!!!` (adds Xd20 to the move roll).
* **Act:** P0 plays CRUSH (discard has 2 cards, so +2d20), then moves. 星月夜
  (2) lets P0 abandon the first result and re-roll.
* **Expect:** the re-roll is of the whole 3d20+2d20 set (5 dice), not just the
  base d20. Assert `dice_left() == 0` after and that the movement equals the
  second sum.
* **RULING:** is 「放弃第一次的结果重骰一次」 a re-roll of the full set, or a
  re-roll of only the base die? Assert full set.

#### G09. dice-reroll x dice-fix: 白金燐子's fixed X*6 plus a re-roll
* **Cards:** `skill:白金燐子:即使1cm也要前进` (2) + `Mor:蝴蝶飞舞的星月夜` (P0).
* **Act:** P0 spends 3 fire to fix the move at 18. 星月夜 (2) offers a re-roll.
* **Expect:** once the roll is fixed to X*6 there is nothing to re-roll; the
  re-roll prompt does not appear (or, if it does, the result stays 18).
* **RULING:** which wins when both are offered? Assert no re-roll; record.

#### G10. dice-set x dice-result: Eve's +Yd4 on a 2d20 move
* **Cards:** `skill:仓田真白:向后全速前进` (move = 2d20) + `skill:若宫伊芙:天下统一` (2).
* **Act:** P0 (Mashiro) with Eve's borrowed (2), or arrange Eve and give her
  Mashiro's roll via `skill:若叶睦（CRYCHIC）:精致的人偶`'s copy. Simplest:
  P0 is Eve, P1 is Mashiro, and P0 uses 精致的人偶 to copy Mashiro's 2d20;
  then Eve's (2) adds Yd4.
* **Expect:** the move is `2d20 + Yd4`, backwards (Mashiro's (2)).
* **RULING:** does a dice-set rewrite (2d20) happen before or after the
  additive modifier?

### G3. Path

#### G11. forced-stop x end-rewrite: a forced stop on a path that LOCK rewrites
* **Cards:** `RAS:（LOCK）追逐梦想的步伐` (2) + `R:live前的准备` or
  `MyGO:（乐奈）有趣的女人` (forced stop).
* **Setup:** P0 walks from 30 past Bandori车站 (35) toward 40; P1 holds live前的准备
  and forces a stop at 江户川乐器店 (30)... (that is the start). Use 乐奈's card
  on tile 33 instead.
* **Act:** P0's walk would end at 40, passing 35 (LOCK rewrites the end to
  旭汤澡堂 52) and 33 (乐奈 forces a stop).
* **Expect (rule 86):** the first forced stop on the path wins; the path is
  recomputed only when an effect changes the 终点. Order of the two hooks is
  per-tile: 33 comes before 35, so the stop at 33 lands first and LOCK's
  rewrite never happens.
* **RULING:** if LOCK's rewrite is processed first (it is on `SettleBefore`),
  does the stop at 33 still count? Assert stop-at-33; record.

#### G12. forced-stop x reverse: a stop while the move is reversed
* **Cards:** `skill:美竹兰:叛逆的红挑染` (2) (move backward) + `PPP:（香澄）大家我都喜欢哦`
  (forced stop on the hill).
* **Act:** P0 moves backward from 50 past 44 (星之鼓动山丘) toward 40.
* **Expect:** 「[路径]」 for a backward move includes the tiles walked in that
  direction; the hill's force-stop fires when 44 enters the path. P0 stops at
  44 and settles.
* **RULING:** is a backward walk's [路径] defined the same way? Glossary 42
  says the path is from start to end in the direction of travel.

#### G13. end-rewrite x reverse: 普通与理所当然 after a reversed move
* **Cards:** `MyGO:普通与理所当然` + `skill:羽泽鸫:伟大的平凡` (2).
* **Act:** P0's main move is reversed (say 8 backward). Later P0 is hit by an
  abnormal move and plays 普通与理所当然, which copies 「最近一次非传送的主要
  移动的移动格数」.
* **Expect:** the distance is 8, and the new move is forward 8 (the card only
  copies the distance, not the direction). Assert pos += 8.
* **RULING:** does 「移动格数」 include the sign? Assert unsigned; record.

#### G14. reverse x teleport: 仓田真白's 2d20 backwards into a teleport start
* **Cards:** `skill:仓田真白:向后全速前进` + `AG:商店街的青梅竹马`.
* **Act:** P0's main move is the 2d20 backwards; before settling, P0 plays
  商店街的青梅竹马, which teleports and 「视为你的主要移动」.
* **Expect:** the teleport replaces the main move entirely; the backward 2d20
  is discarded. P0 lands on the shop-street tile and settles there.
* **RULING:** can a 「视为主要移动」 teleport replace an already-started walk?
  Assert replacement.

### G4. Settlement

#### G15. replace-settle x extra-settle: Parking Space's stay-settle plus 轮符雨's extra
* **Cards:** `通用:[都筑诗船]Parking Space` on Space (42) + `MyGO:轮符雨`.
* **Act:** P0 plays 轮符雨 (stay + extra settle at turn end) and ends the main
  move on Space.
* **Expect:** Space's settle is replaced by 「turn end: +1 [停留]」; 轮符雨
  then adds an extra [触发结算] at turn end. Do both fire? The replaced
  settle is not a settle any more, so the extra settle has nothing to
  re-settle; P0 gets 2 stays (one from each).
* **RULING:** does 「额外进行一次[触发结算]」 re-run a *replaced* settle?
  Assert 2 stays; record.

#### G16. skip-settle x remote-settle: 人偶的箱庭 (no settle) into 练习室里的风暴
* **Cards:** `Mujica:人偶的箱庭` + `RAS:练习室里的风暴` (P1's tile).
* **Act:** P0 plays 人偶的箱庭; P1 chooses to move (no settle) onto the tile
  `storm` is on... (storm's hook is on a settle X tiles away). Arrange:
  P1's no-settle move ends within `storm`'s X range of the storm tile.
* **Expect:** storm's (2) fires 「[结算]时」 — a no-settle move does not
  trigger it. P1 does not force-settle the storm tile.
* **RULING:** does a forced remote settle count as a settle for storm's own
  trigger (recursion)? Assert no.

#### G17. remote-settle x extra-settle: 立希's quarter-rent plus 轮符雨
* **Cards:** `MyGO:（立希）想认真去做` + `MyGO:轮符雨` (P1 has a stay).
* **Act:** P0 plays 立希; P1 settles on P0's tile at quarter rent. P1 also has
  轮符雨's extra settle pending at their turn end.
* **Expect:** the extra settle is a normal settle (full rent), not the
  quarter-rent one. P1 pays `R(t,h)` at turn end.

### G5. Counteract windows

#### G18. negate-one x shut-window: 骰子已经掷下 while a multi-target pay is live
* **Cards:** `Mujica:骰子已经掷下` (P0) + `通用:登上武道馆` (P0) +
  `通用:网络链接异常` (P2).
* **Act:** P0 plays 骰子已经掷下 (let it resolve). Then P0 plays 武道馆.
* **Expect:** no counter window opens for P2, so 网络链接异常 cannot drop a
  target. All three others pay.
* **Note:** `c14` currently shows 骰子已经掷下 does not shut the windows;
  this case adds the negate-one leg once it does.

#### G19. negate-one x join-window: 真奈 lets a third party drop one designation
* **Cards:** `Sumimi:（真奈）歌唱大赛5连冠` (P0) + `通用:登上武道馆` (P1) +
  `通用:网络链接异常` (P3).
* **Act:** P1 plays 武道馆 targeting P0, P2, P3. P0 counters with 真奈.
  Under 真奈's (2) 「场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡」,
  P3 may play 网络链接异常 as if they were the designated one, dropping their
  own designation.
* **Expect:** P3 pays 0; P0 and P2 pay X. P3 draws 1 (真奈's 「若以此种方式
  使你免于受到该影响，打出那张[反击]卡的玩家可抽一张卡」).

#### G20. shut-window x join-window: 真奈 during 骰子已经掷下
* **Cards:** `Mujica:骰子已经掷下` (P1) + `Sumimi:（真奈）歌唱大赛5连冠` (P0).
* **Act:** P1 plays 骰子已经掷下; P0 would like to counter with 真奈.
* **Expect:** 「本回合内所有其他玩家无法从手牌中使用[反击]」 — P0 cannot.
  Assert no window is offered.
* **RULING:** does 「无法使用[反击]」 also block a counter that another card
  (真奈) invites? Assert yes; record.

### G6. Targeting

#### G21. immunity x absorb: 夏日合宿 plus 祥子 (1) status absorption
* **Cards:** `Mor:夏日合宿` (P1) + `skill:丰川祥子:请把你们的人生交给我` (1) (P2).
* **Act:** P0 plays `MyGO:那天的雨`, which gives [停留] to a colour zone.
  P1 is in the zone; P2 passes P1 and would absorb P1's stay.
* **Expect:** 夏日合宿 「你只会被自己发动的效果指定」 — P1 is not designated,
  so there is no stay for P2 to absorb. P2 gains 0 fire.
* **Note:** `s08` is currently DISCREPANCY (祥子 absorbs nothing).

#### G22. redirect x absorb: EXIST retargets a status card onto a 祥子
* **Cards:** `RAS:EXIST` (P2, 祥子) + `MyGO:轮符雨` (P0, self-target) is
  self-only; use `通用:雨啊，快点来吧` instead (designates X players).
* **Act:** P0 designates P1; EXIST retargets to P2. P2 is 祥子 and is now the
  designated one.
* **Expect:** P2 gains the stay (EXIST retargets the designation, it does not
  absorb). 祥子 (1) is a *pass* trigger, not a designation shield.
* **RULING:** none; this is a sanity check that redirect and absorb are
  different categories.

### G7. Status

#### G23. exile x unstoppable: 初华 state 2 under 无路矢's exile
* **Cards:** `MyGO:无路矢` (P0 targets P1's tile) + `skill:三角初华:Imprisoned XII`
  state 2 (P1 has [不可阻挡]).
* **Act:** P0 plays 无路矢, giving P1 2 [除外].
* **Expect:** glossary 51: [不可阻挡] 「无法获得新的[除外]层数」. P1 gains 0
  exile and is not teleported later.
* **RULING:** 无路矢's exile is the cost of the teleport, not just a status.
  If the exile is refused, is the whole card negated? Assert the card is
  negated; record the alternative (P1 is teleported with no exile).

#### G24. unstoppable x clear-status: 壱雫空 vs a [不可阻挡] holder
* **Cards:** `MyGO:壱雫空` (P0) + `skill:三角初华:Imprisoned XII` state 2 (P1,
  already holding 1 [停留] from before entering state 2).
* **Act:** P0 plays 壱雫空.
* **Expect:** [不可阻挡] 「[停留]，[晕眩]，[除外]在适当时机依旧掉层」 — a
  *clear* is not a new layer, so the stay is cleared. P1 pays 1000 per
  cleared type (see the `clear_charges_every_player_per_type` DISCREPANCY).

### G8. Crystals and fire

#### G25. crystal-add x fire-spend: TITLE IDOL crystals vs a fire-paid skill
* **Cards:** `PP:TITLE IDOL` (adds crystals to every crystal card) +
  `skill:要乐奈:投币式停车场的猫` (2) (spend 3 fire to teleport to Space).
* **Act:** P0 plays TITLE IDOL; a card of P0's that mentions crystals gains 1.
  Then P0 uses Rana's (2).
* **Expect:** the crystals added by TITLE IDOL are not fire pots. P0's fire
  is unchanged; the skill still needs 3 fire.
* **Note:** a category-boundary sanity check. `light_crystal_pays_skill_fire_cost`
  is the *legitimate* crystal-as-fire case (MyGO:（灯）不再迷茫 (2)) and is
  currently DISCREPANCY.

#### G26. crystal-spend x fire-add: Popipapapipopa's spend vs a fire gain on the same settle
* **Cards:** `PPP:[衍生]Popipapapipopa` (spend crystals, -150 each) +
  `skill:户山香澄:非凡之星` (1) (another settling on the hill gives +1 fire).
* **Act:** P1 settles on P0's 星之鼓动山丘 tile with the Popipapapipopa card
  on P0's field; P0 is Kasumi.
* **Expect:** P1's rent is reduced by 150 per crystal P0 spends; P0 gains 1
  fire from the hill settle. The two are independent.

#### G27. crystal-move x fire-spend: 会被骗着买水晶的人 while someone spends fire
* **Cards:** `Mujica:会被骗着买水晶的人` + `skill:佐藤益木:与燃烧的红色一起驰骋` (2).
* **Act:** P0 moves a crystal from one card to another during P1's turn, while
  P1 is spending fire for +Xd10.
* **Expect:** the crystal move does not touch fire pots and does not interrupt
  the dice-set. Assert P1's fire drops by X and the roll is `d20 + Xd10`.

### G9. Draw

#### G28. draw-before x draw-replace: 梦在前方's crystal vs Maya's look-at-top
* **Cards:** `PP:梦在前方，结彩当下` (1) (+1 crystal per draw) +
  `skill:大和麻弥:朝阳照耀的片刻` (2) (replace a draw with a look-at-top).
* **Act:** P0 is Maya with 梦在前方 on field; P0 draws, using (2) to look at
  Y+1 and keep 1.
* **Expect:** the kept card 「此次加手视为抽卡动作」, so 梦在前方 (1) still
  adds 1 crystal. The other looked-at cards going back to the deck do not.
* **Note:** the harness gap is closed -- `ctx::draw` now raises `drew` (and
  `drawn`) per single card. `rb_gap_res::g28_maya_look_at_top_still_counts_as_a_draw`
  still carries a stale `TODO(harness)` ignore; drop it (or re-ignore with a
  real reason) when this case is next touched.

#### G29. draw-replace x draw-curve: Maya's look-at-top inside 春日影's draw-to-6
* **Cards:** `CRYCHIC:春日影` (1) + `skill:大和麻弥:朝阳照耀的片刻` (2).
* **Act:** P0 triggers 春日影 (draw until hand is 6) with Maya's (2) active.
* **Expect:** each iteration of the curve is a draw, so each may be replaced
  by a look-at-top. The curve stops when the hand reaches 6.

### G10. Field / tiles

#### G30. tile-swap x field-place: 笑容大游行's swap plus a field card
* **Cards:** `HHW:笑容大游行` (3) (tile swaps with 弦卷集团) +
  `通用:[都筑诗船]Parking Space` on Space.
* **Act:** 大游行 is placed on Space; the swap makes Space behave as 弦卷集团
  and vice versa. Parking Space is still on the (now doubly labelled) tile.
* **Expect:** the swap changes what the tile *is*, not what is on it. Parking
  Space's (1) still replaces the settle. 大游行's (3) then fires after a
  settle there and goes to discard.
* **Note:** `t13_smile_parade` is DISCREPANCY (stack overflow); this adds the
  field-place leg once it lands.

### G11. Fans and tokens

#### G31. fan-flip x mark-generic: Aya's fan spend while holding 兔子
* **Cards:** `skill:丸山彩:With~` (2) + `skill:花园多惠:花园警察，出警！` (1)
  (多惠兔子 marks).
* **Act:** P0 is Aya with 3 fans and 2 兔子 on tiles; P0 pays a rent and flips
  Y=2 fans to reduce it by 200.
* **Expect:** the fan flip does not touch the 兔子 marks. The rent drops by
  200. The 兔子 still convert to fire when passed.

#### G32. fan-spend x mark-generic: Eve's +Yd4 while holding a 抹茶芭菲
* **Cards:** `skill:若宫伊芙:天下统一` (2) + `skill:要乐奈:投币式停车场的猫` (3)
  (抹茶芭菲 marks).
* **Act:** P0 is Eve holding 2 fans and 1 抹茶芭菲; P0 rolls for a move,
  flipping 2 fans for +2d4.
* **Expect:** the 抹茶芭菲 is untouched and still spendable at a RiNG for 400.
  The roll is `d20 + 2d4`.

### G12. Order-dependence candidates (carry-forward RULINGs)

These are the pairs whose outcome depends on resolution order and that the
sheet does not settle. Each is a candidate Q for the next ruling round. The
consolidated open-rulings list (these R-ids plus the sheet-text ambiguities
and the gap `RULING` tests) is
[TEST-FINDINGS.md](TEST-FINDINGS.md) §6; the bullets below stay as the
design record and the suggested order:

* **R-1 (money multiply x multiply):** G01. Fire bird 1.5x vs Ave Mujica 1.5x
  vs 摩卡 half vs HHW band double-pay. Suggest: AddDiff, then each MultDiff in
  field-card age order (oldest first), then band, then character, then clamp.
* **R-2 (money flat-add x multiply):** FEVER!'s +X vs a rent multiplier. Does
  +X land on the base rent or on the multiplied rent?
* **R-3 (money cancel-one x split-share):** G03. Does X recompute after a
  designation is dropped?
* **R-4 (roll additive order):** Y.O.L.O +1d4 vs 不要背负期待 −2 vs Eve +Yd4
  vs 沙绫 −1d10. The existing `inter_yolo_pushes_haneoka_over_20` is already
  `CROSS-AGENT` on this. Suggest: dice-set, then additive modifiers in
  declaration order, then clamp-floor.
* **R-5 (path hook order):** G11. A per-tile walk fires pass hooks in tile
  order; a `SettleBefore` end-rewrite is a *later* timing than the pass hooks
  on the original path. Assert the original path's hooks win.
* **R-6 (counteract close vs join):** G19/G20. Once a round on X closes, may a
  真奈-style joiner still enter it? Suggest: no — 「所有玩家同意所有对一个
  时点的[反击]已发动后可对新的时点发动[反击]」 closes the round.
* **R-7 (extra-turn x until-turn):** an effect lasting 「until your next turn
  start」 while extra turns are granted. Suggest: it ends at the first turn
  start after it was created, including an extra turn's.

---

## Implementation notes

* One test fn per case or variant: `c01_…`, `m14b_…`, `t08_…`, `s05_…`.
  Quote the clause(s) in a `// 规则书:` comment.
* Use loaded dice for every roll. Assert `dice_left() == 0` where it matters.
* Cases marked **RULING** get `#[ignore = "RULING: <question>"]` unless the
  engine already does exactly what the case expects. Engine mismatches get
  `#[ignore = "DISCREPANCY: …"]`.
* If a case can't be arranged through the harness, add a minimal local helper
  in the test file (`t.m.world_mut()`), or report it as not testable. Never
  read card or skill rule code.
