# Purchasing in the rules crates — design (2026-10-07)

Status: **P0–P5 implemented, including the P5 deferred work** (2026-10-07).
ABI 39 → 40 (surface) → 41 (removals). SAVE_VERSION 3 → 4.

* The hook-aware `WasmRules::buy_quote` is wired: it runs `BuyGate` /
  `BuyAdd` → `BuyMul` → `BuySet` in **pure guard mode** against a world copy
  (the `cant_play` shape — no mutation, no prompt) only when a hooking instance
  exists; the commit re-quotes, so quote == charge. Gates (`buyable_here`,
  `why_not_act "buy"`), the view (`st.buy_price`), the AI (`ai_wants_buy` /
  `ai_agent_choice`) and the autopilot all read that quote.
* The three cards that used the `TurnCtx` flags are ported — see §Card
  migrations. `buy_discount` / `free_buy` / `raze_on_buy` and their `set_*`
  APIs, `st.tile_colors` and `key::EXTRA_COLOR` are **deleted** (ABI 41).
* The colour skills (朝日六花 / Roselia (1) / 游击演出 / soyo_clear / （soyo）
  混合的颜色) write the `colorFor:<p>` / `ANY_COLOR` tile props.
* Dealer buy/build options are not filtered by current cash. After selecting
  one, a short player can mortgage deeds to cover the final payment amount,
  or cancel without a partial charge or bankruptcy. The deed being upgraded
  is excluded from funding. Ordinary land buy/build gates and buttons also
  count mortgageable deeds, and use the same optional funding path; normal
  rent/auction funding stays mandatory.

User request: tile purchasing and agent purchasing — what can pay, how much,
how ownership is assigned — are handled by the `rules/` crates.

## Today (bugs this fixes)

* **Property buy** goes through `play.rs::buy()`:
  1. `buyBefore`;
  2. `buy_price` (`tile.price + houses*tile.house`);
  3. the `TurnCtx` flags `buy_discount` / `free_buy`;
  4. `money()` with kind `buy` (`payAdd` / `payMul` apply);
  5. the `owners[t]` write and mortgage clear;
  6. `raze_on_buy`;
  7. `bought` → `buyAfter`.
* **Gates and AI read the base price, not the price actually charged.** That
  covers `buyable_here`, `can_buy_here`, `why_not_act "buy"`, `ai_wants_buy`
  and `ai_agent_choice`. A player who can afford only the discounted price is
  refused (`err.buy_poor`).
* **Agent:** the `tile:agent` body's 同色 set (`ctx::is_color`) reads the
  board `group` plus `prop::ANY_COLOR` / `colorFor:<p>`, so （soyo）混合的颜色's
  「其他颜色的地产商」 can fire. An `ANY_COLOR`-only member is offered for buy
  but never for build (soyo 「该格本身不可因自有以外的颜色的地产商盖房」).
* **Force-buy:** moves money directly at 2×(price + houses). The deed stays
  mortgaged and no buy hooks are raised.
* **Auction:** moves money directly, clears the mortgage, and raises only
  `bought`.
* **Dead gates:**
  * `plan.no_buy` (rana_parking) is never read.
  * Poppin's `lock_hill` cancels `buyBefore`, but `buy()` ignores the
    cancellation, and the lock never covers force-buy.
* **Unwritten view field:** `st.buy_price` is always -1, so `autopilot.ts`
  recomputes the price itself.

## Responsibilities

**The rules crate** (`rules/tiles` plus card / skill hooks) decides:

* **Eligibility:**
  * the tile prop `BUYABLE` (「可购买格子」, rules.txt line 19);
  * `HookKind::BuyGate` (Poppin hill lock, parking no-buy, 「不可被抵押双倍支付购买」).
* **Price:**
  * the base from tile props;
  * then the stages `BuyAdd` (fixed ±) → `BuyMul` (×) → `BuySet` (free /
    fixed), each floored at 0, mirroring 支付阶段 2 / 4 / 5.
* **Payer and payee:**
  * `seller` is the bank (-1) for land / agent / card / auction buys, and the
    owner for force-buy and 收购;
  * `direct` (bypass the pipeline) is set by the tile prop `FORCE_FIXED`
    (line 106 「不受任何资金变动效果影响…」, line 92).
* **Assignment:**
  * `HookKind::BuyAssign` runs at commit, before `bought`;
  * hooks rewrite `owner_after`, `houses_after` (raze) and `mortgaged_after`;
  * the defaults come from tile props. A land buy clears the mortgage;
    `FORCE_STAYS_MORTGAGED` (「获得的地契仍为抵押状态」) keeps it.
* **Agent offers:** the `tile:agent` body decides:
  * the 同色 set: own `group` + `prop::ANY_COLOR` + per-player `colorFor:<p>`;
  * the all-owned → half-charge branch;
  * which tiles are offered for Buy vs Build. An `ANY_COLOR`-only member is
    never buildable, per soyo 「该格本身不可因自有以外的颜色的地产商盖房」.

**The engine keeps the generic mechanics:**

* prompt / answer plumbing;
* `money()`, or a direct transfer;
* one ownership primitive `World::assign_deed(t, owner, houses, mortgaged)`;
* the view preview (`st.buy_price`, prompt prices).

`CardRules::buy_quote`'s default is the plain rulebook formula; that is what
StubRules and the sim run.

## ABI surface

* **`TriggerKind` / `HookKind`:**
  * `BuyGate`, `BuyAdd`, `BuyMul`, `BuySet`, `BuyAssign` (the next free values);
  * wire names `buyGate`, `buyAdd`, `buyMul`, `buySet`, `buyAssign`.
* **`BuyKind`:** `{ Land, Agent, Card, Force, Acquire, Auction }`.
* **New props:** `BUYABLE`, `BUY_HOUSES`, `FORCE_MULT` (milli), `FORCE_FIXED`,
  `FORCE_STAYS_MORTGAGED`, `ANY_COLOR`, and the prefix `colorFor:`.
* **Retired props (deleted at ABI 41):** `BUY_DISCOUNT`, `FREE_BUY`,
  `RAZE_ON_BUY`. The `TurnCtx` flags `buy_discount` / `free_buy` /
  `raze_on_buy` and their `set_*` APIs, `st.tile_colors` and `key::EXTRA_COLOR`
  go with them.
* **Trigger payload:** `buy_kind`, `seller`, `price` / `set_price`,
  `deal_owner` / `set_deal_owner`, `deal_houses` / `set_deal_houses`,
  `deal_mortgaged` / `set_deal_mortgaged`. `BuyGate` uses `set_cancelled` plus
  a reason.
* **`ctx`:**
  * `buy_quotes(player, kind, &[tile])` — batched, one `HostRequest::Quote`;
  * `buy(player, tile, kind) -> bool` — replaces `card_buy`;
  * `acquire(player, from, tile, price)` — 收购: pipeline pay, then assign →
    `bought` → `buyAfter`;
  * `agent_offer(...)`;
  * `linger(player, expires)` — binds the running card's own def as a
    turn-scoped instance in `TurnCtx.lingering`. It is cleared at turn start
    and carried across NeedHost by `adopt_turn_policy`. It is the hand-card
    home for 「本回合」 effects: the def's own `BuyAdd` / `BuyMul` / `BuySet` /
    `BuyAssign` hooks reach the buy pipeline, and a `ctx::set_prop` made before
    the call lands on the instance's props (e.g. `prop::NO_BUILD`). The
    lingering instances hear the same field-hook dispatch as placed cards and
    board rules. It replaces the retired `buy_discount` / `free_buy` /
    `raze_on_buy` flags and the per-player `noBuild` scratch key.
  * `ctx::buy_price(t)` stays as the deed's base value.
* **`CardRules::buy_quote(&self, w: &World, data: &GameData, q) -> Quote`:**
  `WasmRules` clones the world only when a hooking instance exists, and runs
  the hooks in pure guard mode, like `cant_play` (a throwaway copy — no world
  mutation, and a hook that would prompt contributes nothing to the preview).
  The commit re-quotes, so quote == charge. Takes the world by reference so the
  view's `st.buy_price` preview asks without cloning.

## Card migrations

| source | new | status |
|---|---|---|
| @Tsugu ycm (3) | linger + `BuyAdd` −1500 | **done** |
| Roselia band (1)/(2) | `BuyMul` ½ (live-house / first non-LH); `Bought` one-shot unchanged | **done** |
| 迷宫般的仓库 | linger + `BuySet` 0 + `BuyAssign` houses 0 | **done** |
| Poppin (3) hill lock | `BuyGate`, every kind, Force included | **done** |
| rana_parking | native gate reads `plan.no_buy` for Land | **done** |
| 学生会的检查 noBuild | a linger instance carrying `prop::NO_BUILD` | **done** |
| （soyo）混合的颜色 | tile prop `ANY_COLOR` | **done** |
| 朝日六花 / Roselia (1) / 游击演出 / soyo_clear | tile prop `colorFor:<p>`; remove `st.tile_colors` and `key::EXTRA_COLOR` | **done** |
| 巴 收购 | `ctx::acquire` | **done** |
| Afterglow / chuchu / asahi | `Bought`, unchanged | unchanged |

## AI and autopilot

* **View:** `st.buy_price` / `st.build_cost` are written from the quote at
  routine commit on END, and -1 otherwise.
* **Prompts:** `MatchPrompt.price` (buy / force_buy) and `prices[]` (agent),
  serde-defaulted.
* **`ai.rs`:** uses the quoted prices.
* **`autopilot.ts`:** reads `S.buyPrice`, `p.price` and `p.prices[k]`.

## Phases (each gated: build-ruleset, game-core, game-rules, fuzz, i18n, check.py, sim)

* **P0 — surface, no behaviour change.**
  * sdk kinds / props / payload / ctx;
  * new file `engine/play/purchase.rs` (`BuyKind`, `BuyQuery`, `Quote`,
    `Deal`, `quote_native`, `assign_deed`, commit);
  * the `buy_quote` default;
  * the host functions;
  * test: `quote_native` == today's prices on every tile.
* **P1 — property buy.**
  * quote → commit;
  * honour the `buyBefore` cancel;
  * the `why_not_act` gate;
  * the `st.buy_price` write;
  * `MatchPrompt.price`;
  * tile props in `property.rs` / `ring.rs`;
  * AI and autopilot;
  * sim event counts identical (buy 2908, forcebuy 415, auctions 264).
* **P2 — agent.** `agent.rs` owns the set / branch / offers;
  `ctx::agent_offer`; `prices[]`; soyo tests.
* **P3 — force-buy and 收购.** Kind Force through the quote (Poppin lock
  included); direct transfer; `FORCE_STAYS_MORTGAGED`; no buy hooks until
  ruling 6; `ctx::acquire` and the tomoe_savior port.
* **P4 — auction.** `BuyGate` filters bidders; the quote base sets AI worth;
  the commit goes through assign + `bought`; money stays direct until ruling 7.
* **P5 — flags, cards, colours.** `linger`; the card ports; delete the
  `TurnCtx` flags and the `set_*` APIs, `st.tile_colors` and
  `key::EXTRA_COLOR`; SAVE_VERSION bump; docs.

## Risks

* **Perf.** The sim runs only the native path (expected under 1%). Time
  fuzz / web-glue before and after as well (+10% budget).
* **Replay.** `st.buy_price` and the prompt fields enter `save()`, so old
  recordings fail compat on ABI / format; that is acceptable.
* **`linger`** must cross `adopt_turn_policy`.
* **Flag-internal tests, rewritten to assert behaviour:**
  * `rb_ras::lock_skill_colors_the_first_deed_like_a_live_house` asserted
    `extraColor:1`; it now asserts the observable consequence (the deed counts
    as a Live House for that player).
  * `q4/snap.rs`, `q4/expiry.rs` recorded `turn.buy_discount` etc.; they now
    record `turn.lingering` (the mechanism those flags became) and treat it as
    turn-scoped.
  * `q4/names.rs`'s `extraColor:` filter goes with the key.
* **The price stages run for every `BuyKind`.** `BuyAdd` / `BuyMul` / `BuySet`
  are the engine's generic quote stages, so a lingering 「本回合购买格子…」
  discount also shapes a Force / Acquire / Auction quote. Before the migration
  the flags only applied to `buy()`. Whether 强制购买 / an auction win is a
  「购买」 for those clauses is rulings 6 / 7 — **not decided here**; if a ruling
  says they are not, the cards' hooks must scope on `trigger::buy_kind()`.
  `bought` / `buyAfter` still do not fire for Force or Auction (unchanged).

## Rulings (proposed defaults in brackets)

1. Buy-price stage order: add → mul → set? [yes]
2. Does a buy's [消耗] also run the pay pipeline after the price stages? [yes,
   as today]
3. May a player mortgage to fund a buy (line 76)? [ordinary land and dealer
   buys/builds allow optional mortgage funding; force-buy/card acquisition
   offers keep their existing gates]
4. Agent pick: a full [结算] of the chosen tile, or a direct buy / build? Does
   「玩家拥有的」 include other players' tiles?
5. Does soyo's 「所有颜色」 count toward another colour's all-owned
   half-charge, and is it offered for purchase? Do the per-player 「视为live
   house」 effects widen the agent set?
6. Is 强制购买 a 「购买」 for 「购买…时」 listeners? [no hooks today]
7. Auction: is a win a 「购买格子」 for discounts? Does the auction [消耗] go
   through the pipeline?
8. 巴's 「常规收购价一半」: what is the base (land / land + houses / 2× force)?
9. 迷宫般的仓库 raze + Afterglow free house: raze first, so 1 house remains?
   [yes]
10. Do 「本回合」 linger effects end at this turn's end even in an extra turn,
    or during another player's turn?
11. Poppin hill co-ownership needs a multi-owner model — out of scope.
