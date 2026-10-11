# G-MONEY -- 资金变动与账本

Unit 1 owns this file and `rules/spec/src/global/money.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

---

## G-MONEY-01

* **ID:** `G-MONEY-01`
* **Source:** `rulebook-doc.md:60`
* **Quote:** 「[支付]：“A[支付]BX资金”的效果为玩家A向玩家B支付X资金。」
* **Trigger:** any observed input whose event window contains money moves.
* **Condition:** the window's only money-moving events are `pay` / `rent` legs
  with no `gain` modifier (a one-sided print / delete is a different clause; a
  modified transfer is a different clause).
* **Effect:** for each such leg of X from A to B: `money(A)` fell by X,
  `money(B)` rose by X, and every other player's money is unchanged.
* **Frame:** only `money:*` may move on a pure-transfer step.
* **Rulings & OPEN:** a `gain` arg on the log line (`log.with_received`) means
  a card raised the amount the payee received; that is not a bare
  「A[支付]BX」 and is out of scope here. Bankruptcy and auction windows move
  money without ledger legs (documented observability gap) and are skipped.
* **Executable:** `rules_spec::global::money::g_money_01`.

---

## Open questions

* OPEN-分摊 (分摊 undefined) -- a 分摊 clause cannot be written yet.

## Findings

(Empty.)