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
// rules/cards/official-basic/src/lib.rs
use card_sdk::{ctx, key, CardDef, Msg};

/// C# `CardPress` -- 压: gain 1,000.
const PRESS: CardDef = CardDef { id: "R:[衍生] 压", play: Some(press), can_react: None, react: None };
fn press(seat: i32) {
    ctx::gain(seat, 1000, &Msg::new(key!("why_press")));
}

card_sdk::bandori_ruleset!(&[PRESS, /* ... */]);
```

* `id` is the **data** card id from `data/cards.json` (Chinese; an identifier,
  not display text).
* every string a card produces is a message key: `key!("...")` namespaces it as
  `cards:<crate>.<key>`, and the translation lives in the crate's
  `locales/zh-CN.json` + `locales/en.json`. The message crosses the guest/host
  boundary as `postcard` bytes (ABI v5) -- a serde format, so neither side
  hand-writes an encoding. The engine renders it to each client in that
  client's language.
* names are arguments, not text: `Msg::new(key!("x")).seat("who", s).tile("tile", t)`
  -- the client resolves them to display names.

## The vocabulary (`card_sdk::ctx`)

| group | functions |
|---|---|
| dice & log | `roll`, `log` |
| board | `tile_count`, `tile_named`, `tile_owner`, `seat_pos`, `tile_steps_ahead`, `rent_of`, `buy_price`, `build_cost`, `mortgage_value`, `owned_tiles`, `is_buyable`, `is_shop`, `tile_group`, `tile_price`, `houses_of`, `set_houses`, `add_house`, `mortgaged_of`, `set_mortgaged`, `set_owner`, `dist`, `tile_forward`, `neighbor`, `seats_on` |
| seats & money | `seat_count`, `seat_out`, `others`, `money`, `gain`, `pay`, `can_pay`, `character_is`, `in_band`, `turn_seat`, `round_no`, `turn_key` |
| hand & deck | `draw`, `add_to_hand`, `add_to_deck`, `to_discard`, `hand_count`, `discard_count`, `deck_count`, `discard_size`, `discard_from_hand`, `sweep_to_deck` |
| field cards | `place_card`, `place_card_at`, `unplace_card`, `is_placed`, `set_dest` |
| marks & tokens | `add_mark`, `count_marks`, `remove_marks`, `tok`, `set_tok`, `add_tok` |
| per-seat slots | `slot`, `set_slot`, `inc_slot` |
| pots & status | `band_crystals`, `add_band_crystals`, `fire`, `fire_max`, `gain_fire`, `spend_fire`, `give_stay`, `give_stun`, `give_exile`, `give_extra_turn`, `stay_of`, `stun_of` |
| ring | `ring_multiplier`, `add_ring_bonus`, `teleport_to` |
| prompts | `ask_yes`, `ask_pick`, `ask_tile`, `ask_seat`, `ask_card`, `ask_number` |
| nesting & trigger | `play_card`, `trigger::{kind, seat, target, tile, value, move_roll, set_move_roll, card_is}` |

Prompts can carry an AI preference later (`H.AskXxx`'s `ai` parameter); until
then bots take the prompt fallback.

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
fn popipa(seat: i32) {
    ctx::set_dest(ctx::Dest::Banished);          // 规则书[手]: 「然后此卡[移除]」
    ctx::gain(seat, 1000, &Msg::new(key!("why")));  // 规则书[手]: 「获得1000资金」
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

1. read its C# `Play` / `React` / `WhyNot` (MatchHost.cs);
2. write a `CardDef` + body against the vocabulary above;
3. add the message keys to `locales/zh-CN.json` and `locales/en.json`;
4. `node tools/build-ruleset.mjs && python tools/i18n/check.py && cargo test -p game-rules`.

`tools/i18n/check.py` verifies every `key!(...)` has a translation, so a card
cannot ship with a raw key showing to players.

## Status

* seam: card modules run inside real matches (server and browser), prompts come
  out as engine prompts and effects commit -- `crates/game-rules/tests/live_match.rs`;
* vocabulary above (ABI v7) is implemented end to end;
* **all 184 cards are ported** (`rules/cards/card-*`, one module per card family
  crate, one `.rs` per card). `python tools/rulebook/check.py` verifies every
  card quotes its passage and cites it; `node tools/build-ruleset.mjs` ships
  them as one module (`dist/cards`, 184 cards).
* [反击] cards fire through the hand-reaction window (`WasmRules::hand_reactions`):
  every trigger opens it, declarations resolve in reverse order, and a reaction
  play opens a counter-window (depth-capped runaway guard at 16; the C# cuts at
  4, which is tighter than the card pool needs).
* most cards are **partial**: what the vocabulary cannot express is marked
  `TODO(规则书)` / `TODO(ABI)` naming the missing hook. The remaining plumbing
  wave, from those markers:
  * **movement routines** (`H.CardMove` / `H.Walk` / `H.ForceWalk` with settle,
    main-move consumption, move-plan fields) -- the big one;
  * **persistent `Fx` hooks** (per-card instances + `Mem`/`Crystals`):
    `PassTile`, `PayChoose/PayAdd/PayMul/PayAfter/PayAt`, `SettleAfter/Instead`,
    `TurnStart`/`TurnEnd(+After)`, `Drew/Drawn`, `RollAfter`, `BuildCost/Built`,
    `HandLimitDelta`, `Actions`, ...;
  * **`CardDef` hooks**: `why_not` / `note_text` / `ai_play` (playability gates),
    which need a `CardDef` field (deferred while the card crates were being
    written);
  * targeting (`H.Target`/`PickTarget`/`TargetAll`, `t.ByCard`, `PlayCtx`
    cancel/`Effective`/tags) and `H.ExtraOf` skill attachments;
  * a few odds: `H.BuyRoutine`/`BuildRoutine`, deck-top inspection,
    play/reaction history, world snapshot/restore.

Those are engine-side extensions (a nested `Cx` run inside a host call shares the
answer log; the `Fx` hook set becomes new `CardRules` methods), not rewrites of
what is here.