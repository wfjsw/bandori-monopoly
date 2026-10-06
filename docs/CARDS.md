# Card & event rules

Official card behaviours are WASM card modules (the authoring choice: **wasmi +
`card-sdk`, one module per card family**), executed inside the match by
[`WasmRules`](../crates/game-rules/src/wasm_rules.rs). The engine shell
(`game-core`) never holds card text; every message is a key (see
[I18N.md](I18N.md)).

## How a card runs

A card effect is straight-line Rust. When it needs a player decision it calls an
`ask_*` helper: the host aborts the run, the engine raises the prompt like any
other (`Cx::ask`), and after the answer the run replays from the same world
snapshot -- so dice re-roll identically and the effect reaches the prompt again,
this time with an answer. No coroutines, no callbacks; the RNG lives in the
world copy the host runs against.

## Authoring template

```rust
// rules/cards/card-roselia/src/press.rs
use card_sdk::{ctx, key, CardDef, Msg, On};

/// C# `CardPress` -- 压: gain 1,000.
pub const PRESS: CardDef = CardDef::new("R:[衍生] 压", &[
    On::Play(press),
    // On::CantPlay(cant_play),                       // C# `Card.WhyNot`
    // On::Counteract(&[TriggerKind::Pay], guard, counteract),  // [反击]: kinds + guard + effect
    // On::Hook(&[TriggerKind::TurnEnd], decay),      // field hooks, auto-run in play
    // On::AtEnd(at_end),                             // scheduled turn-end body
    // On::RollPlan(roll_plan),                       // shapes the main move
]);
fn press(player_id: i32) {
    ctx::gain(player_id, 1000, &Msg::new(key!("why_press")));
}
```

Every handler is a `fn(player_id: i32)` (the player the card is placed at for hooks);
`On::Counteract` carries its [反击] trigger kinds + a pure `bool` guard + the effect,
`On::Hook` the field-hook kinds (`passTile`, `payAdd`, `drawn`, `turnEnd`, ...
-- the kind names ARE the C# `Fx.*` names). An empty kind list is never
dispatched.

* `id` is the **data** card id from `data/cards.json` (Chinese; an identifier,
  not display text).
* every string a card produces is a message key: `key!("...")` namespaces it as
  `cards:<crate>.<key>`, and the translation lives in the crate's
  `locales/zh-CN.json` + `locales/en.json`. The message crosses the guest/host
  boundary as `postcard` bytes (ABI v26) -- a serde format, so neither side
  hand-writes an encoding. The engine renders it to each client in that
  client's language.
* names are arguments, not text: `Msg::new(key!("x")).player_id("who", s).tile("tile", t)`
  -- the client resolves them to display names.

## The vocabulary (`card_sdk::ctx`)

| group | functions |
|---|---|
| dice & log | `roll`, `log` |
| board | `tile_count`, `tile_named`, `tile_owner`, `player_pos`, `tile_steps_ahead`, `rent_of`, `buy_price`, `build_cost`, `mortgage_value`, `owned_tiles`, `is_buyable`, `is_shop`, `tile_group`, `tile_price`, `houses_of`, `set_houses`, `add_house`, `mortgaged_of`, `set_mortgaged`, `set_owner`, `dist`, `tile_forward`, `neighbor`, `players_on`, `is_ring`, `is_circle`, `is_live_house` |
| players & money | `player_count`, `player_out`, `others`, `money`, `gain`, `pay`, `can_pay`, `cant_move`, `character_is`, `in_band`, `turn_player`, `round_no`, `turn_key` |
| hand & deck | `draw`, `add_to_hand`, `add_to_deck`, `to_discard`, `hand_count`, `hand_size`, `discard_count`, `deck_count`, `discard_size`, `discard_from_hand`, `sweep_to_deck`, `add_to_deck_at` (`DeckPos::{Top,Bottom,Random}`), `cards_in(player_id, CardPile)` (list a pile; deck top first), `take_card(player_id, CardPile, id)` / `take_from_hand` (remove without discarding) |
| marks & tokens | `add_mark`, `count_marks`, `remove_marks`, `tok`, `set_tok`, `add_tok` |
| per-player slots | `slot`, `set_slot`, `inc_slot` |
| pots & status | `band_crystals`, `add_band_crystals`, `fire`, `fire_max`, `gain_fire`, `spend_fire`, `give_stay`, `give_stun`, `give_exile`, `give_extra_turn`, `stay_of`, `stun_of` |
| ring | `ring_multiplier`, `add_ring_bonus`, `teleport_to` |
| prompts | `ask_yes`, `ask_pick`, `ask_tile`, `ask_player`, `ask_card`, `ask_number` |
| nesting & trigger | `play_card`, `trigger::{kind, player_id, target, tile, value, step, by_card, move_roll, set_move_roll, set_pay_amount, set_pay_target, set_cancelled, cancelled, card_is, move_flags, move_is_main, move_dir}` |
| field cards | `place_card`, `place_card_at`, `unplace_card`, `is_placed`, `set_dest`, `placed_tile`, `crystals`, `set_crystals`, `add_crystals`, `decay` |

Prompts can carry an AI preference later (`H.AskXxx`'s `ai` parameter); until
then bots take the prompt fallback.

## Trigger points (`trigger::kind()`)

Every point the engine raises is a pair: a `*Before` half that fires before the
thing happens (so a counteraction can block or rewrite it) and a `*After` half that
fires after it commits. A few legacy kinds from the original port kept their
original names where the pairing is already implied (`settleBefore` →
`settle` → `settleAfter`, `pay` → `paid`, `roll` → `moveRoll`).

| pair | when |
|---|---|
| `turnStartBefore` / `turnStart` | a turn begins -- before / after the exile & stun status ticks |
| `roll` / `moveRoll` | the main roll -- before the d20 is cast / after it, before walking (a counteraction may reroll via `set_move_roll`) |
| `passBefore` / `pass` | each tile stepped over (CiRCLE and the destination) -- before / after the player arrives |
| `settleBefore` / `settle` / `settleAfter` | landing -- before resolving the tile / before its effect / after it fully resolves |
| `mortgageBefore` / `mortgage` | mortgaging a deed -- before any guard (can block) / after it applied |
| `pay` / `paid` | money leaving a player -- before the deduction / after. `player_id` = payer, `target` = payee, `value` = amount |
| `bankruptBefore` / `bankrupt` | bankruptcy -- before asset cash-in / after cash-in, before removal from the game |
| `card` / `cardAfter` | playing a card from hand -- before its `play` body / after its `Dest` handling |
| `event` / `eventAfter` | drawing an event -- before it resolves / after it is filed away |
| `buyBefore` / `buyAfter` | buying a deed -- before any guard (fires even on a no-op attempt) / after the deed changes hands |
| `buildBefore` / `buildAfter` | building a house -- before payment / after the house commits (not on the refund path) |
| `discardBefore` / `discardAfter` | discarding from hand -- before the card leaves / after it is in the pile |
| `endTurnBefore` / `endTurnAfter` | ending a turn -- the player's command / any turn end, including stun & exile auto-skips |
| `leaveBefore` / `leaveAfter` | forfeiting -- before any guard / after the player is cleared, before the game-over check |

`roll` carries `value = -1` as a "no roll yet" sentinel so roll-counteracting cards
(which match `Roll | MoveRoll`) stay dormant before the dice are cast.

## Every line cites the rule book

The rule book is the spec: `docs/rulebook/cards-sheet.csv` (one column per band,
one cell per card) exported from
[this sheet](https://docs.google.com/spreadsheets/d/1xZ3avBsNBXbl3bQ74lmPs0YQFgZkPD0Sdzmx7ZGGEDY),
plus
[this document](https://docs.google.com/document/d/1jM0aThg70XQ_9wYCubZ6oWRhS_f9SgIpmjpxvt4OlGU)
(band and character skills). `tools/rulebook/extract.py` turns the sheet into
`docs/rulebook/cards.json` (card id -> passage); list markers `（1）（2）…` each get
their own line there.

A card's file carries the passage it implements and every code line cites the
sentence it implements:

```rust
//! 规则书（docs/rulebook/cards.json, id `PPP:Popipa`）:
//! > Popipa：
//! > [手]：
//! > 获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]
...
fn popipa(player_id: i32) {
    ctx::set_dest(ctx::Dest::Banished);          // 规则书[手]: 「然后此卡[移除]」
    ctx::gain(player_id, 1000, &Msg::new(key!("why")));  // 规则书[手]: 「获得1000资金」
    ...
}
```

Rule book text that the code cannot express yet (hooks the ABI lacks) goes in as
`// TODO(规则书): …` naming the missing hook, never silently dropped.

```sh
python tools/rulebook/extract.py   # sheet -> docs/rulebook/cards.json
python tools/rulebook/cite.py      # (re)quote the passage into each card file
python tools/rulebook/check.py     # every card quotes its passage and cites it
```

## Porting checklist

`cardmap.json` (extracted from `MatchHost.cs` line ~18500) maps every C# class to
its data id: 184 cards over 176 classes. Porting a card:

1. read its C# `Play` / `Counteract` / `WhyNot` (MatchHost.cs);
2. write a `CardDef` + body against the vocabulary above;
3. add the message keys to `locales/zh-CN.json` and `locales/en.json`;
4. `node tools/build-ruleset.mjs && python tools/i18n/check.py && cargo test -p game-rules`.

`tools/i18n/check.py` verifies every `key!(...)` has a translation, so a card
cannot ship with a raw key showing to players.

## Status

* seam: card modules run inside real matches (server and browser), prompts come
  out as engine prompts and effects commit -- `crates/game-rules/tests/live_match.rs`;
* vocabulary above (ABI v26) is implemented end to end;
* **all 184 cards are ported** (`rules/cards/card-*`, one module per card family
  crate, one `.rs` per card). `python tools/rulebook/check.py` verifies every
  card quotes its passage and cites it; `node tools/build-ruleset.mjs` ships
  them as one module (`dist/cards`, 184 cards).
* [反击] cards fire through the hand-counteraction window (`WasmRules::hand_counteractions`):
  every trigger opens it, declarations resolve in reverse order, and a counteraction
  play opens a counter-window (depth-capped runaway guard at 16; the C# cuts at
  4, which is tighter than the card pool needs).
* **Ordering when several cards answer one trigger** is a LIFO stack, and the
  case that matters is a [反击] counter-card answering someone else's card play
  (the `card` → hand-counteraction window → `counteracted` chain). Declarations are
  collected in player order starting at the trigger's player and wrapping the
  table; resolution is the exact **reverse** of declaration order, so the last
  card to answer the window resolves first, and any counteraction it plays opens a
  nested counter-window before earlier declarations get their turn.
  Where this LIFO rule does **not** apply (left as-is, flagged for a later pass):
  * the acting card's own follow-up (`t.card`'s `counteract`) resolves *before* the
    window, outside the stack -- a card answering its own play is not competing
    with counteractions to it;
  * one declaration per player per window -- a player with two eligible cards picks
    one and cannot stack both;
  * nested counter-windows are separate stacks (LIFO within each, not across
    the whole chain);
  * multi-player declaration order is fixed by player position, not chosen.
* `trigger::step()` is the turn step (0/1/2/3) the trigger fired in. It is only
  informative for kinds that are not already step-specific -- `mortgage`, for
  example, can fire during both step 1 (awaiting roll) and step 3 (settling),
  and `step()` tells those apart.
* **Field-card (`Fx`) hooks** are persistent effects on a placed card. They are
  *not* [反击] points: the engine runs every placed card's `counteract` against them
  **automatically**, in placement order per player, with no player declaration.
  They use their own `TriggerKind`s (`turnEnd`, `drawn`, `passTile`, `payAfter`,
  `rollAfter`, `cardPlayed`, `targeted`, `payChoose`) so a card can tell a field
  effect from a hand counteraction by its kind alone -- `match trigger::kind()` is the
  dispatch. Every hook kind is raised except `targeted` (it waits on the
  targeting pipeline). `drawn` runs on the card just drawn (named on
  `t.card`, still in hand) rather than on placed cards. Hook-only kinds open no
  [反击] window; `turnStart` / `settleAfter` are both a hook and a [反击] point,
  so a card there tells which by `is_placed(player_id)`.
  Per-card miracle crystals (`crystals` / `set_crystals` / `add_crystals`) are
  the decay counter `DecayCard.TurnEnd` uses.
* A counteraction can reshape the trigger it answered: `set_move_roll` rewrites a
  move roll, `set_pay_amount` reduces or cancels (0) a payment, `set_pay_target`
  redirects its payee (-1 = the bank), and `set_cancelled` / `negate_effect` / `spare` shape what settles. The engine honours all four once the counteraction window closes.
* **[反击] is keyed on the effect, not on an outcome.** `ChainKind::Effect` is
  raised when an effect's recipients are named, before settlement; `ctx::effect`
  lists what that link declared (`count` / `kind` / `target` / `from` / `tile` /
  `value`), so a guard can take the whole list (「被…效果影响」 -- `effect::hits`)
  or one entry (「一次性支付5000以上」 -- `effect::has(TriggerKind::Pay)`).
  `Target` / `Abnormal` / `Pay` are settlement hooks now and cannot reconstruct
  that clause. A counteraction is a chain link: it resolves **before** the effect and
  may `set_cancelled()` (negate the activation -- the link never happened),
  `negate_effect()` (it happened, settles to nothing), or `spare(seat)` (everyone
  but that seat settles).
* `trigger::by_card()` is the player whose card caused the trigger (`None` when it
  was board-driven, e.g. rent or a buy). This is what 「来自你以外」 checks against
  (C# `H.HitByOtherCard`): `by_card().is_some_and(|by| by != player_id)`. It is
  distinct from `player_id()` -- on a `pay` trigger `player_id()` is the *payer*, not the
  card that forced the payment, so only `by_card()` can tell a card-caused
  payment from rent.
* most cards are **partial**: what the vocabulary cannot express is marked
  `TODO(规则书)` / `TODO(ABI)` naming the missing hook (251 markers remain after
  the v22-v25 waves). What is **landed** since the first cut: the `On::` handler
  form (Play / CantPlay / Counteract / Hook / AtEnd / RollPlan), the field-hook kinds
  (`PassTile`, `PayAdd/PayMul/PayChoose/PayAt/PayAfter`, `SettleAfter/Instead`,
  `TurnStart`/`TurnEnd(+Before/After)`, `Drew/Drawn`, `RollAfter`, `Reshuffled`,
  `Bought`, `Discarded`, `DeckBeforeGame`/`DeckAtGameStart`, `BeforeOut`,
  `Teleported`), per-card `Crystals`, pile enumeration/take
  (`cards_in`/`take_card`/`card_replayable`), the turn-end scheduler
  (`before_turn_end` = C# AtEnd / `at_turn_end` = AfterEnd), the abnormal gate
  (`abnormal_count`, `abnormal_kind`, `AbnormalGuard`, the `abnormal` [反击]
  window), the trigger payload (`by_card`, `pay_is_rent`, `move_flags/main/dir`,
  `step`, pay mutators, `cards()`), and the plan-shaping ops
  (`ctx::plan::*`). What **remains**, from the markers:
  * **`ctx::card_move`** (C# `H.CardMove`: run the move now as the main move) --
    the largest family (~20 markers); plan-shaping is written and waits only for
    this + the plan-readback wiring in `main_move`;
  * plan gaps: `set_start` (start-tile override), `set_teleport_to`
    (teleport-with-settle shaping), multi-die `MoveCtx.Base` tables, and
    in-flight move writeback (counteractions setting `m.Stopped`/`m.ExtraSteps`);
  * **targeting** is landed in v26 (`target`/`target_all`/`target_tile`,
    `ImmuneAll`/`Untargetable`/`Redirect`, `targeted_count`); what remains is
    the per-play `immune<p>` tags, `PlayCtx` `Effective`/`Extreme`, `t.Pay.tile`,
    `H.ExtraOf` skill attachments, the
    skill system (`TryResonance`, `SwitchState`, `SkillUsed`, ...), and
    host->guest strings (`AbName`/`EventTitle`);
  * routines (`BuyRoutine`/`BuildRoutine`/`MortgageRoutine`/`SettleAt`),
    `AgentLanding`, `RevealSeen`/`Actions`, world snapshot/restore.

Those are engine-side extensions (a nested `Cx` run inside a host call shares the
answer log; the `Fx` hook set becomes new `CardRules` methods), not rewrites of
what is here. The engine seam is co-developed with two peer sessions -- check
`ListAgents` before editing `game-core`/`game-rules`/`card-sdk`.