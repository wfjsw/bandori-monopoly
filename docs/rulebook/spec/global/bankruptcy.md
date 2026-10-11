# G-BANKRUPT -- [破产] / [存活] / 离开 / 拍卖

Unit 7 owns this file and `rules/spec/src/global/bankruptcy.rs`. Append
clauses; do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

---

## G-BANKRUPT-01

* **ID:** `G-BANKRUPT-01`
* **Source:** `rulebook-doc.md:134`
* **Quote:** 「当玩家进入[破产]状态或选择离开游戏（弃权）时进行以下操作：
  1. 将其控制的所有棋子，角色卡，乐队卡，和手卡移出游戏。所有其正在生效的卡，技能效果停止生效。
  2. **其拥有的[可购买格子]变成无主状态**并抽取3张其中格子的地契进行[拍卖]（少于3则全部）。」
* **Trigger:** an observed input whose event window contains a `bankrupt`
  event for player `P` (or a leave that cashes out).
* **Condition:** `P` owned at least one buyable tile in `pre`.
* **Effect:** in `post`, no buyable tile is owned by `P`. (The auction of
  min(3, n) deeds is unit 7's separate clause; this one only checks the
  un-own step.)
* **Frame:** n/a (read of `owners`).
* **Rulings & OPEN:** 「[可购买格子]」 excludes CiRCLE / 咖啡厅 / 江户川 /
  流星堂 / [地产商] (`rulebook-doc.md:65`). Bankruptcy money has **no ledger
  legs** (the engine cashes mortgages silently and zeroes the payer) -- that
  is a documented observability gap and is why G-MONEY-01 skips `bankrupt`
  windows.
* **Executable:** `rules_spec::global::bankruptcy::g_bankrupt_01`.

---

## Open questions

* OPEN-乐队卡 -- 「乐队卡」 undefined as a card class (bankruptcy removes them).

## Findings

(Empty.)