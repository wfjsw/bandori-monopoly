# Card & event rules

Official card behaviours are WASM card modules (the authoring choice: **wasmi +
`card-sdk`, one module per card family**), executed inside the match by
[`WasmRules`](../crates/game-rules/src/wasm_rules.rs). The engine shell
(`game-core`) never holds card text; every message is a key (see
[I18N.md](I18N.md)).

The same authoring shape covers the other rule categories: board tiles
(`rules/tiles`, [TILES.md](TILES.md)), tile marks (`rules/tile_marks`, `mark:*`
-- [TILES.md](TILES.md) → 「Board marks」) and event cards (`rules/events`,
[EVENTS.md](EVENTS.md)). An event's `CardDef` id is `event:<id>` and its body is
the same `On::` table -- `On::Play` for what happens on draw, `On::Hook` for
what runs while it is active, `On::AtEnd` for expiry.

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
    On::Play("", None, press),
    // On::Play("", Some(cant_play), press),          // C# `Card.WhyNot` gate
    // On::Counteract(&[ChainKind::Pay], "", guard, counteract),  // [反击]: kinds + condition + guard + effect
    // On::Hook(&[HookKind::TurnEnd], "", guard, decay),          // field hooks, auto-run in play
    // On::AtEnd(at_end),                             // scheduled turn-end body
    // On::RollPlan(roll_plan),                       // shapes the main move
    // On::Settle(settle),                            // a tile rule's settle body (docs/TILES.md)
]);
fn press(player_id: i32) {
    ctx::gain(player_id, 1000, &Msg::new(key!("why_press")));
}
```

Every handler is a `fn(player_id: i32)` (the player the card is placed at for hooks);
`On::Counteract` carries its [反击] trigger kinds + the condition + a pure `bool`
guard + the effect, `On::Hook` the field-hook kinds (`passTile`, `payAdd`,
`drawn`, `turnEnd`, ... -- the kind names ARE the C# `Fx.*` names) in the same
shape. An empty kind list is never dispatched.

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

## Authoring a condition (`pre`, ABI v45)

A guarded entry (`On::Counteract`, `On::Hook`, `On::Play` gates) carries a
**condition** string immediately before the residual guard -- category →
condition → guard → body, the three-layer model of
docs/[GUARDS.md](GUARDS.md) §4.3. `""` = no condition. The condition is a CEL
expression over the §4.2 window/candidate vocabulary; it is compiled **once**
at ruleset build and evaluated natively before the wasm guard is ever
instantiated.

```rust
use card_sdk::{pre, On};

// condition (`pre: &'static str`) before the guard
On::Counteract(&[ChainKind::MoveRoll], "actor == owner && move.roll != null",
               can_counteract, counteract),

// sugar: `pre::MINE` = `actor == owner`
On::Hook(&[HookKind::TurnEnd], pre::MINE, guard, decay),

// or the const builder
On::Play("", Some(cant_play), play).pre("owner.money >= 1500"),
```

**Distinct layers (nothing is rejected twice).** The category filter (the
`On::*` kind list) owns the trigger kind; the condition owns everything
expressible in the schema; the wasm guard owns only the residual. A guard must
not restate its condition. `kind` is an ordinary variable, so a multi-kind
entry may still *branch* on it -- never use it to reject what the category
filter already rejected.

Fail-closed: a parse error, an unknown variable/function, or a float literal
(the schema is int-only) fails `build-ruleset` / `RulesetBuilder::build` with
`RuleError::BadPre`. An absent condition means "no clauses beyond the guard".

The compiled lean form (`Cond::to_bytes(false)`) rides the ruleset so the
browser glue evaluates with `rules-cond`'s `runtime-only` build and never ships
the CEL parser.

## Bot-only estimated execution cost

`prop::EST_COST` (`"estCost"`) is a card's **estimated execution cost** (user
ruling 2026-10-07): what activating the card is expected to cost the player, in
资金. Read **only** by bots / autopilot as a reserve check -- never by legality.
Constant for now; an X-dependent cost may later become a `rules-cond`
expression. Declare it with `CardDef::new(...).props(&[(prop::EST_COST, N)])`.
`0` (the default) means unknown / assume free.

```rust
pub const GACHA10: CardDef = CardDef::new("通用:10次招募（1回限定）", &[On::Play("", None, gacha10)])
    .props(&[(card_sdk::abi::prop::EST_COST, 1500)]);
```

Filled for cards with fixed payments (the six formerly money-gated ones plus
the other flat `ctx::pay(N)` bodies); leave it at `0` for computed costs.

## Declared card properties (`CardDef::props`)

Some behaviours are **static properties of the card rule**, not effects that
run at a trigger. They are a generic **property map** -- named `key -> i32`
values declared on the `CardDef`, keyed by constants so the names are never
stringly scattered:

```rust
use card_sdk::abi::prop;
// 规则书[持续]（1）: 「手卡上限数量减1」 -- C# `Card.HandLimitDelta`
pub const CUT: CardDef = CardDef::new("PP:不要背负期待", &[...])
    .props(&[(prop::HAND_LIMIT_DELTA, -1)]);
// 规则书: 「（此卡可在眩晕时打出）」 -- C# `Card.PlayableStunned`
pub const H: CardDef = CardDef::new("MyGO:壱雫空", &[...])
    .props(&[(prop::PLAYABLE_STUNNED, 1)]);
```

The keys the engine reads live in **one place** each side (the two crates
cannot share a definition, so they are mirrored -- keep them in step):

* `card_sdk::abi::prop` -- what a card rule declares against;
* `game_core::state::prop` -- what the engine reads back.

| key | constant | rulebook clause | C# | what the engine does with it |
|---|---|---|---|---|
| `handLimitDelta` | `prop::HAND_LIMIT_DELTA` | 「手卡上限数量减1」 (`-1`) | `Card.HandLimitDelta` | stamped onto the field instance at placement (`FieldCard::props`) and summed into the owner's hand limit while it sits there; gone with the card. A `place_raw` test arrangement sees it too. |
| `playableStunned` | `prop::PLAYABLE_STUNNED` | 「（此卡可在眩晕时打出）」 (`1`) | `Card.PlayableStunned` | the card skips the stun gate in `cannot_play`. The exile and no-hand gates have no such exception in the pool. |

**Tile-rule properties** (the same map, on a *board-owned* tile rule instance --
[TILES.md](TILES.md)): `price` / `house` / `group` / `buildMax` / `rentLen` /
`rent:N` / `ringMult` are the tile's data, stamped at `bind_tiles`; `noReward` /
`rentFactor` / `payFactor` / `buyDiscount` / `freeBuy` / `razeOnBuy` / `noBuild`
are the modifiers a card writes in place of the engine flags those used to be.
Two write surfaces, matching the two clause shapes (ABI v32):

* `ctx::prop` / `ctx::set_prop` -- the **running** instance (a per-player veto
  riding a placed card: PPP band (2) 「无法获取[CiRCLE奖励]」, 梦在前方's
  「[拥有者]不可盖房」). Read back by scanning the passing / building player's
  field.
* `ctx::tile_prop` / `ctx::set_tile_prop` -- the rule instance(s) governing a
  **tile** (a tile-scoped or walk-scoped veto: 「[经过]CiRCLE时不获得[CiRCLE奖励]」
  arms `prop::NO_REWARD` on CiRCLE's `tile:circle` instance).

`prop::NO_REWARD` is the CiRCLE-reward veto (`tile:circle`'s Pass entry reads
and consumes the tile-scoped one); `prop::NO_BUILD` gates `why_not_build_on`;
`prop::RENT_FACTOR` / `PAY_FACTOR` scale a settlement payment at the money
pipeline's `payMul` stage. A rule body's own instance is the one running
(`ctx::self_tile()` names its tile).

**Defined default: `0`** for every key -- a card that does not declare a
property reads as `0`, and `!= 0` is how a boolean-ish key like
`playableStunned` is tested.

Path: `CardDef::props` → `ManifestEntry.props` (a key-sorted
`Vec<(String, i32)>`, so the wire bytes are deterministic; ABI v30) →
`CardInfo::props` (a `BTreeMap<String, i32>` -- not a `HashMap`, so iteration
and serialization stay deterministic) → `CardRules::card_props(card)` /
`::card_prop(card, key)` → stamped onto `FieldCard::props` at placement.

Each declaration cites its clause like any other line (see below). The current
holders: `PP:不要背负期待`, `PP:梦在前方，结彩当下`, `PP:练习生解密指南` and
`Sumimi:#L12` declare `(prop::HAND_LIMIT_DELTA, -1)`; `MyGO:壱雫空` declares
`(prop::PLAYABLE_STUNNED, 1)`.

## The vocabulary (`card_sdk::ctx`)

| group | functions |
|---|---|
| dice & log | `roll`, `log` |
| board | `tile_count`, `tile_named`, `tile_owner`, `player_pos`, `tile_steps_ahead`, `rent_of`, `buy_price`, `build_cost`, `mortgage_value`, `owned_tiles`, `is_buyable`, `is_shop`, `tile_group`, `tile_price`, `houses_of`, `set_houses`, `add_house`, `mortgaged_of`, `set_mortgaged`, `set_owner`, `dist`, `tile_forward`, `neighbor`, `players_on`, `is_ring`, `is_circle`, `is_live_house` |
| players & money | `player_count`, `player_out`, `others`, `money`, `gain`, `pay`, `can_pay`, `cant_move`, `character_is`, `in_band`, `turn_player`, `round_no`, `turn_key`, `gains_this_turn` (「当前回合内你每获得过一次资金」 -- the engine's per-turn money-in counter, reset at each turn start) |
| targeting | `target`, `target_all`, `target_tile`, `targeted_count`, `designations` (the **static targeting query** -- which players the play being resolved designates, C# `H.Db.Card(id).Targeting` + `H.Others`; empty when it names nobody), `cancel_designation` / `designation_cancelled` (per-pair cancel, C# `play.Tags["immune"+seat]` -- one designation drops, the rest land) |
| hand & deck | `draw`, `add_to_hand`, `add_to_deck`, `to_discard`, `hand_count`, `hand_size`, `discard_count`, `deck_count`, `discard_size`, `discard_from_hand`, `sweep_to_deck`, `add_to_deck_at` (`DeckPos::{Top,Bottom,Random}`), `cards_in(player_id, CardPile)` (list a pile; deck top first), `take_card(player_id, CardPile, id)` / `take_from_hand` (remove without discarding) |
| marks & tokens | `place_mark`, `place_mark_new`, `count_marks`, `bump_mark`, `remove_marks`, `mark_src_at`, `count_held`, `add_held`, `move_units`, `tok`, `set_tok`, `add_tok` -- every mark / token is a unit of the creating instance's named counter (`MarkFilter` selects by kind / category / owner / src / instance); see 「Named counters and bound units」 |
| named counters (ABI v50) | `counter` / `add_counter` / `set_counter` (this instance), `counter_at` / `add_counter_at` (by uid), `card_counter` / `add_card_counter` (a placed card by id). Wire names `cp` / `crystals` map to `FieldCard::cp` / `FieldCard::crystals`; anything else lives in `FieldCard::counters`. The old CP helpers (`place_cp` / `count_cp` / `clear_cp` / `add_cp` / `cp_attached`) are **gone** -- use the mark / counter API, and see 「Cross-card messages」 for the 该清CP了 ↔ `mark:cp` shape |
| per-player slots | `slot`, `set_slot`, `inc_slot` |
| pots & status | `band_crystals`, `add_band_crystals` (「乐队卡 / 团卡」 crystals = the band-skill field instance's `crystals`, i.e. its named `'crystals'` counter), `fire`, `fire_max`, `gain_fire`, `spend_fire`, `give_stay`, `give_stun`, `give_exile`, `give_extra_turn`, `stay_of`, `stun_of` |
| skills & band attachments (ABI v35) | `band_skill` / `character_skill` (the bound rule id, C# `H._fx[i].bands` / `.skill`), `band_skills` (every band attachment as `(uid, id, extra)`), `add_band_skill` (C# `H.MakeBand`; `extra` = 「拿取」 copy: 「相同乐队技能卡的效果不可叠加」 / 「不视为那个乐队的角色」), `invoke_skill` (run a skill rule's press entry `On::Play` for a player -- 「立即执行乐队技能的（2）效果」 / `SkillPareo -> Offer()`. No `skillUsed`: that is the player's own press (`use_skill`).) |
| ring | `ring_multiplier`, `add_ring_bonus`, `teleport_to` |
| prompts | `ask_yes`, `ask_pick`, `ask_tile`, `ask_tiles` (labels + prices + AI hint), `ask_player`, `ask_card`, `ask_number` |
| nesting & trigger | `play_card`, `invoke_skill`, `raise_bought` (C# `f.Bought(i, t)` -- a card that handed a deed over announces it), `trigger::{kind, player_id, target, tile, value, step, by_card, move_roll, set_move_roll, set_pay_amount, set_pay_target, set_cancelled, cancelled, card_is, move_flags, move_is_main, move_dir}` |
| purchase (ABI v40/41) | `buy_quotes(player, kind, &[tile])` (batched quote), `buy(player, tile, kind)` (replaces `card_buy`), `acquire(player, from, tile, price)` (「收购」: pipeline pay, then assign → `bought` → `buyAfter`), `agent_offer`, `linger(player, expires)` (bind the running def as a turn-scoped instance in `TurnCtx.lingering` -- the hand-card home for 「本回合」 effects; a `set_prop` before it lands on the instance's props), `buy_price(t)` (the deed's base value). `trigger::{buy_kind, seller, price, set_price, deal_owner, set_deal_owner, deal_houses, set_deal_houses, deal_mortgaged, set_deal_mortgaged, set_reason}` carry the `BuyGate` / `BuyAdd` → `BuyMul` → `BuySet` / `BuyAssign` payload (`docs/PURCHASE.md`). |
| field cards | `place_card`, `place_card_at`, `unplace_card`, `is_placed`, `set_dest`, `placed_tile`, `crystals`, `set_crystals`, `add_crystals` (sugar over this instance's named `'crystals'` counter), `decay` |
| cross-card messages (ABI v51) | `send(Target, name, &Message)`, `On::Message(names, pre, guard, body)`, `ctx::message::{sender_uid, sender_seat, name, a, b, c, tile, seat, text, reply}` -- see 「Cross-card messages」 |
| tile rules | `self_tile`, `prop`, `set_prop`, `tile_prop`, `set_tile_prop`, `prop_at`, `set_prop_at`, `draw`, `draw_event`, `pay_rent`, `offer_buy`, `offer_build`, `offer_force_buy`, `buy`, `buy_quotes`, `card_build`, `ask_tiles`, `ai_agent_choice`, `is_color`, `raise`, `gain_typed`, `exile_of`, `stun_of`, `card_settle_at` (the settle / pass primitives; `docs/TILES.md`) |
| event rules | `event_expire`, `event_is_active`, `event_deck_push`, `event_banish` (the active-list / deck handles; `docs/EVENTS.md`) |

Prompts can carry an AI preference (`ask_tiles`'s `ai` hint, or the prompt
fallback when it is -1).

## Named counters and bound units (ABI v51)

Every tile mark and player token is a **unit of some card instance's named
counter**, bound to a tile or a holder; destroying the instance destroys its
units (user ruling 2026-10-10). There is one model, not two:

* **On-card pool** -- the count on the instance itself. The two known names
  keep their wire names and storage: `counter::CP` (`"cp"`) → `FieldCard::cp`
  (「自己[场上]N个[CP点]」), `counter::CRYSTALS` (`"crystals"`) →
  `FieldCard::crystals` (奇迹水晶). Anything else lives in
  `FieldCard::counters[name]`.
* **Tile marks** (`TileMark`) -- units bound to a tile. `TileMark.instance` is
  the owning counter's card instance; `TileMark.src` stays **provenance**
  (which card instance placed it). A mark dies with its instance, not with its
  placer.
* **Held tokens** (`Counter`) -- units bound to a holder (a player).
  `Counter.instance` is the owning counter's card instance.

The guest surface is the generic verb set:

```rust
// on-card named counter of this instance
ctx::counter("cp");
ctx::add_counter("cp", 6, 0)?;          // max 0 = uncapped; raises CounterChanged
ctx::set_counter("cp", 0);
// another instance / a placed card by id
ctx::counter_at(uid, "cp");
ctx::add_counter_at(uid, "cp", -1, 0);
ctx::card_counter(player, "通用:该清CP了", "cp");

// tile marks, selected by MarkFilter (kind / category / owner / src / instance;
// empty / mark::ANY = any)
ctx::place_mark(tile, "mark:cp", "cp", -1, self_uid(), 1, &note);
ctx::count_marks(tile, &MarkFilter::any().category("cp"));
ctx::bump_mark(tile, &filter, -1);
ctx::remove_marks(tile, &filter);
ctx::mark_src_at(tile, &filter);        // provenance (TileMark.src)

// holder-bound units of this instance's counter
ctx::count_held("抹茶芭菲", player);
ctx::add_held("抹茶芭菲", player, 1, 0)?;
ctx::move_units("抹茶芭菲", -1, other, space, -1, 1);
```

`tok` / `add_tok` / `set_tok` are the holder-binding verbs of the creating
instance's counter (`add_tok` = `add_held`). `crystals` / `set_crystals` /
`add_crystals` are sugar over this instance's named `'crystals'` counter;
`band_crystals` / `add_band_crystals` are sugar over the band-skill field
instance's. The CP helpers (`place_cp` / `count_cp` / `clear_cp` / `add_cp` /
`cp_attached`) are **gone**.

SAVE_VERSION 5 → 6: `TileMark.instance` / `Counter.instance` are new
(serde default `-1` for legacy rows; a row whose creator cannot be determined
is kept, never auto-purged). `Match::restore` attaches CP marks to the standing
`mark:cp` and other rows to the creating rule's field instance when it can be
found.

One `CounterChanged` hook (wire names `counterChanged`|`crystalsChanged`|
`cpChanged`) replaces `CpChanged` / `CrystalsChanged`; it carries
`Trigger.name` = the counter name and `Trigger.value` = the signed delta.
「…时」 clauses on a count (「此卡上不再拥有[奇迹水晶]时」, 通用:该清CP了's
graveyard rule) live in one event handler on that hook, not at each spend site.

## Cross-card messages (ABI v51)

`ctx::send(Target, name, &Message)` is the generic cross-card channel.
`On::Message(names, pre, guard, body)` is the answering entry -- same
condition / residual guard / body layering as `On::Hook`. The handler reads
the sender and payload from `ctx::message::*` and returns a reply value via
`ctx::message::reply(v)`; the sender's `ctx::send` returns it (0 when no
receiver admitted the message). Handling a message does not flash the card.

Addressing (`abi::Target`): `Uid(i32)` (a specific field instance),
`Card { player, card }` (the instance of a card id on a seat), or
`Board(String)` (a standing board-owned pseudo card, e.g. `mark:cp`).
Multiple receivers of a name: the first admitted one in field order answers;
a handler whose `pre` / guard rejects is not a receiver.

`Message` is a few typed ints plus an optional name -- `name`, `a`, `b`, `c`,
`tile`, `seat`, `text`. `ctx::message::{sender_uid, sender_seat, name, a, b,
c, tile, seat, text}` read them; `Trigger.name` is the message name (the same
field a `CounterChanged` hook uses for the counter name).

### 该清CP了 ↔ `mark:cp` (before / after)

通用:该清CP了 seeds tile [CP点] and an on-card stock; `mark:cp` owns the tile
marks and the landing clause that spends both kinds. The card files are
mid-migration (`rules/cards/card-general/src/clear_cp.rs` already uses the new
API; `rules/tile_marks/src/cp.rs` still calls the old CP helpers). The shape:

**Before** -- 该清CP了 called the CP helpers directly and listened to
`HookKind::CpChanged`:

```rust
// 通用:该清CP了
On::Play("", Some(cant_play), clear_cp),
On::Hook(&[HookKind::CpChanged], "", Some(cp_empty_guard), on_cp_empty),

fn clear_cp(player_id: i32) -> Asked {
    // ...
    ctx::place_cp(tile);            // tile [CP点], stamped onto this instance
    ctx::add_cp(6, 0);              // on-card [CP点] stock
    // ...
}
fn cp_empty_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::card_is(ID) && ctx::cp_attached() == 0
        && trigger::value() <= 0
}
```

**After** -- 该清CP了 `ctx::send`s a message to `Target::Board("mark:cp")`
asking it to place CP units; `mark:cp`'s `On::Message` handler runs
`ctx::place_mark`. The landing hook spends the placer's on-card CP via
`ctx::add_counter_at`. The graveyard rule is an `On::Hook` on
`HookKind::CounterChanged` filtering `counter_is('cp')` /
`card.counter('cp') == 0`:

```rust
// 通用:该清CP了
On::Play("", Some(cant_play), clear_cp),
On::Hook(&[HookKind::CounterChanged], "", Some(cp_empty_guard), on_cp_empty),

fn clear_cp(player_id: i32) -> Asked {
    // ...
    place_tile_cp(tile);                        // the message, below
    ctx::add_counter(counter::CP, 6, 0)?;       // on-card [CP点] stock
    // ...
}

/// Ask the `mark:cp` standing rule to put `count` [CP点] on `tile`.
fn place_tile_cp(tile: i32) -> Asked {
    ctx::send(
        &Target::Board("mark:cp".into()),
        "place",
        &Message {
            name: "place".into(),
            tile,
            a: 1,                               // 「添加1个[CP点]」
            ..Default::default()
        },
    )?;
    Ok(())
}

fn cp_empty_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::card_is(ID)
        && trigger::name() == counter::CP     // or counter_is('cp') in `pre`
        && ctx::counter(counter::CP) == 0
        && trigger::value() <= 0
}

// rules/tile_marks/src/cp.rs
On::Message(&["place"], "", Some(places_cp), place_cp),
On::Hook(&[HookKind::SettleBody], "", Some(lands_on_cp), on_land),

fn place_cp(_owner: i32) -> Asked {
    let tile = ctx::message::tile();
    let src = ctx::message::sender_uid();       // provenance 「此卡在格子上添加的」
    ctx::place_mark(
        tile, mark::CP_KIND, mark::CP_CATEGORY,
        /* owner */ -1, src, ctx::message::a().max(1), &Msg::new("log.cp_placed"),
    );
    ctx::message::reply(1);
    Ok(())
}

fn on_land(_owner: i32) -> Asked {
    let seat = trigger::player_id();
    let tile = trigger::tile();
    let src = ctx::mark_src_at(
        tile,
        &MarkFilter::any().category(mark::CP_CATEGORY),
    );
    ctx::bump_mark(
        tile,
        &MarkFilter::any().category(mark::CP_CATEGORY),
        -1,
    );
    // 「自己[场上]1个[CP点]」 -- the placer's on-card stock
    if src >= 0 {
        ctx::add_counter_at(src, counter::CP, -1, 0);
    }
    ctx::gain(seat, 800, &Msg::new("log.cp_clean").player_id("who", seat))?;
    Ok(())
}
```

The landing clause (settle on a CP tile: remove the tile unit + one on-card
CP of the placer, gain 800) lives on `mark:cp`; the spread clause (「此卡在
格子上添加的[CP点]及其产物」) keys on `TileMark.src` provenance the same way
it does today.

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
| `passBefore` / `pass` / `passTile` | each tile stepped over (CiRCLE and the destination) -- before / after the player arrives. `passTile` is the [经过] step (行动阶段 12); a 「被[经过]」 clause reads the tile being entered. `passPlayer` is the end-tile [重叠] only |
| `moveBefore` / `moveAfter` | the move head / tail (ABI v43, `SETTLE-STAGES.md` §7). `moveBefore` fires once the plan is fixed and before the first step / the teleport -- counteractions that cancel or alter the move go here. `moveAfter` is 「移动后」/「主要移动结束时」/「[移动终点]」: after `passPlayer`, before `settleBefore`, for every completed move including a 「不触发结算」 one |
| `settleBefore` / `settle` / `settleBody` / `settleAfter` | landing -- before resolving the tile / before its effect / the settle's effect list (a field card may replace it with 「将本次结算改为…」) / after it fully resolves |
| `mortgageBefore` / `mortgage` | mortgaging a deed -- before any guard (can block) / after it applied |
| `pay` / `paid` | money leaving a player -- before the deduction / after. `player_id` = payer, `target` = payee, `value` = amount. `paid` is the [反击] window and opens only on a payer-side loss (再次牵起手来 / 游击演出 are 「[消耗]或[支付]」 / 「被…收取资金」). `payAfter` is the 「资金变动」 hook (rulebook 支付阶段 7, 「合并到[支付后]」) and fires on **any** money change, a print (`gain`) and a `gain_fixed` included |
| `payTotalAdd` / `payTotalMul` / `payTotalCancel` | the **command-wide pre-split** stage (「分摊前」, `PIPELINE-AUDIT` Q2): `payTotalAdd` (fixed ±) → `payTotalMul` (×) → `payTotalCancel` (drop the whole command) shape the figure **before** any 「[分摊]」 divides it into shares. A single-pair payment's command total is its own amount, so the three run there too. `trigger::value()` / `set_pay_amount` rewrite the total; `set_cancelled` on `payTotalCancel` drops the command. The per-share counterparts are `payAdd` / `payMul` / `payChoose` / `payAt` |
| `tileResolved` / `moveResolved` / `bankruptResolved` | terminal points (「结算完成时」, `PIPELINE-AUDIT` Q6). `tileResolved` fires after `settleAfter` (and after a cancelled settle); `moveResolved` after a move and any settle it asked for; `bankruptResolved` after the seat is cleared and the leftover auctions finish. `payAfter` / `buyAfter`+`bought` / `eventAfter` are already the terminals of their pipelines |
| `bankruptBefore` / `bankrupt` | bankruptcy -- before asset cash-in / after cash-in, before removal from the game. The seat is marked **dead before `bankruptBefore`** (B3), so the dying player's own sources cannot join that window; other players' hooks still fire and may inspect the remaining state. `bankruptResolved` is the terminal |
| `card` / `cardAfter` | playing a card from hand -- before its `play` body / after its `Dest` handling |
| `event` / `eventAfter` | drawing an event -- before it resolves / after it is filed away |
| `buyBefore` / `buyAfter` | buying a deed -- before any guard (fires even on a no-op attempt) / after the deed changes hands |
| `buyGate` / `buyAdd` / `buyMul` / `buySet` / `buyAssign` | the purchase surface (`docs/PURCHASE.md`). `buyGate` (an `On::Gate`, runs for **every** `BuyKind` -- Force included) refuses a buy with `set_cancelled` + `set_reason`. `buyAdd` (fixed ±) → `buyMul` (×) → `buySet` (free / fixed) are the quote's price stages, each floored at 0, rewriting the running `trigger::price()`. `buyAssign` runs at commit and rewrites `deal_owner` / `deal_houses` / `deal_mortgaged` before the one ownership write. `bought` / `buyAfter` still do not fire for Force or Auction (rulings 6 / 7, undecided). |
| `buildBefore` / `buildAfter` | building a house -- before payment / after the house commits (not on the refund path) |
| `discardBefore` / `discardAfter` | discarding from hand -- before the card leaves / after it is in the pile |
| `endTurnBefore` / `endTurnAfter` | ending a turn -- the player's command / any turn end, including stun & exile auto-skips |
| `leaveBefore` / `leaveAfter` | forfeiting -- before any guard / after the player is cleared, before the game-over check |
| `deckBeforeGame` / `deckAtGameStart` | the two match-start points. `deckBeforeGame` = **before match start**, before the opening hands are drawn: start positions and the authoritative initial hand size (`ctx::inc_start_hand`, default 2, minimum 0) are decided here. `deckAtGameStart` = **after match start**, after the opening deal and mulligan: initial tokens/resources (fire pots 「初始N」, P✽P fans). Both dispatch to **every effect source** of the player they are raised for -- field cards including skills (per player, in field order) and the card ids in that player's piles/hands -- each source running its own hook with `t.card` naming it. There is no separate "match started" point; nothing in the pool wants "after positions are set, before the deal". |
| `drewBefore` / `drawn` + `drew` | the per-draw points, **one raise per single card** (an N-card draw is N iterations, each payload naming one card). `drewBefore` is the *replacement* point: `t.card` is the deck's top, and a hook that replaces the draw calls `trigger::set_cancelled()` and performs its own look/pick -- whatever it adds to the hand is the replacement draw (「此次加手视为抽卡动作」) and the after points fire for it. After the card is in hand: `drawn` runs on the **drawn card itself** (C# `AfterDraw`), `drew` on the **field cards** (crystal-per-draw effects). Opening hands raise **none** of the three -- 「抽卡」 means a draw during play, and 朝同一片天空迈进's 「（开局时抽到此卡洗回）」 is a `deckAtGameStart` clause for exactly that reason. |

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
* vocabulary above is implemented end to end (ABI v35 as of 2026-10-06 --
  `card_sdk::abi::ABI_VERSION`; the wire format note above still says v26
  because that is when `postcard` was adopted);
* **all 184 cards are ported** (`rules/cards/card-*`, one module per card family
  crate, one `.rs` per card). `python tools/rulebook/check.py` verifies every
  card quotes its passage and cites it; `node tools/build-ruleset.mjs` ships
  them as one module (`dist/cards`, 184 cards).
* [反击] cards fire through the hand-counteraction window (`WasmRules::hand_counteractions`):
  every trigger opens one **round per timing** (rulebook 32 / 89), the counters
  settle newest-first before the timing they answer, and a counter is itself a
  new timing that may be answered (depth-capped runaway guard at 16; the C# cuts
  at 4, which is tighter than the card pool needs).
* **Ordering when several cards answer one trigger** (rulebook 89): the ring
  starts at the seat after the player the timing belongs to and runs forward in
  turn order, so that player is asked **last**; each visit a responder declares
  one eligible hand card answering **that timing** or passes (several players may
  counter the same effect, each answering X rather than each other); a
  declaration advances priority to the next responder and the round closes after
  a full consecutive pass. Once the round closes the declared counters become
  new timings, taken **newest first**, each with its own round starting after its
  declarer (clause 89's 「…后可对新的时点发动[反击]」). Resolution is LIFO over the
  resulting answer tree: a counter's own answers settle before it, sibling
  counters settle newest first, and every counter settles before the timing it
  answers. Where this rule does **not** apply (left as-is, flagged for a later
  pass):
  * the acting card's own follow-up (`t.card`'s `counteract`) resolves *before* the
    window, outside the tree -- a card answering its own play is not competing
    with counteractions to it;
  * one declaration per player per visit -- a player with two eligible cards picks
    one and may declare the other when the ring comes back;
  * multi-player declaration order is fixed by player position, not chosen.
* `trigger::step()` is the turn step (0/1/2/3) the trigger fired in. It is only
  informative for kinds that are not already step-specific -- `mortgage`, for
  example, can fire during both step 1 (awaiting roll) and step 3 (settling),
  and `step()` tells those apart.
* **Field-card (`Fx`) hooks** are persistent effects on a placed card. They are
  *not* [反击] points: the engine runs every placed card's `counteract` against them
  **automatically**, in placement order per player, with no player declaration.
  They use their own `TriggerKind`s (`turnEnd`, `drewBefore`, `drew`,
  `passTile`, `payAfter`,
  `rollAfter`, `cardPlayed`, `targeted`, `payChoose`) so a card can tell a field
  effect from a hand counteraction by its kind alone -- `match trigger::kind()` is the
  dispatch. Every hook kind is raised except `targeted` (it waits on the
  targeting pipeline). `drawn` runs on the card just drawn (named on
  `t.card`, still in hand) rather than on placed cards; `drewBefore` / `drew`
  are the per-draw field points (one raise per single card). Hook-only kinds open no
  [反击] window; `turnStart` / `settleAfter` are both a hook and a [反击] point,
  so a card there tells which by `is_placed(player_id)`.
  Per-card miracle crystals (`crystals` / `set_crystals` / `add_crystals`) are
  the decay counter `DecayCard.TurnEnd` uses -- sugar over this instance's
  named `'crystals'` counter (`FieldCard::crystals`). **Band-card (「乐队卡 /
  团卡」) crystals are one pool with them**: they live on the band skill's own
  field instance (`skill:<band>:<skill>`), so `band_crystals` /
  `add_band_crystals` are sugar over that instance's count. No keyed-state
  side store. Edge cases: a player with no band skill reads 0 and every write
  is a no-op; several band cards (PPP:Returns) target the first in placement
  order; a swapped or removed band card takes its crystals with it. The
  empty-out clause (「此卡上不再拥有[奇迹水晶]时」) is an `On::Hook` on
  `HookKind::CounterChanged` filtering `counter_is('crystals')`, not a check
  at each spend site.
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
  `TODO(规则书)` / `TODO(ABI)` naming the missing hook (78 markers remain as of
  the v35 wave -- 2 of them `TODO(ABI)`, and those two are only the convention
  notes in `skill-bands` / `skill-characters`'s `lib.rs`; the card pool's
  `TODO(ABI)` list is empty). What is **landed** since the first cut: the `On::` handler
  form (Play / CantPlay / Counteract / Hook / AtEnd / RollPlan), the field-hook kinds
  (`PassTile`, `PayAdd/PayMul/PayChoose/PayAt/PayAfter`, `SettleAfter` /
  `SettleBody` -- `SettleBody` replaced `SettleInstead` --,
  `TurnStart`/`TurnEnd(+Before/After)`, `Drew/Drawn`, `RollAfter`, `Reshuffled`,
  `Bought`, `Discarded`, `DeckBeforeGame`/`DeckAtGameStart`, `BeforeOut`,
  `Teleported`), per-card `Crystals`, pile enumeration/take
  (`cards_in`/`take_card`/`card_replayable`), the turn-end scheduler
  (`before_turn_end` = C# AtEnd / `at_turn_end` = AfterEnd), the abnormal gate
  (`abnormal_count`, `abnormal_kind`, `AbnormalGuard`, the `abnormal` [反击]
  window), the trigger payload (`by_card`, `pay_is_rent`, `move_flags/main/dir`,
  `step`, pay mutators, `cards()`), and the plan-shaping ops
  (`ctx::plan::*`), and (v35) the **skill / band / follow** surface:
  `band_skill` / `character_skill` / `band_skills` / `add_band_skill`
  (C# `H._fx[i].bands` / `.skill` / `H.MakeBand`; `extra` = 「拿取」 copy),
  `invoke_skill` (run a skill's press entry `On::Play` -- 「立即执行乐队技能的
  （2）效果」, `SkillPareo -> Offer()`), `raise_bought` (C# `f.Bought(i, t)`,
  a card-driven hand-over announcing the acquisition), and
  `plan::add_follower` (「使你的下次主要移动结果对那些玩家一起执行」 -- the
  engine replays the move's result for each follower after the mover settles,
  in the recorded order, carrying the same plan / `pay_factor`).
  What **remains**, from the markers:
  * **`ctx::card_move`** (C# `H.CardMove`: run the move now as the main move) --
    the largest family (~20 markers); plan-shaping is written and waits only for
    this + the plan-readback wiring in `main_move`;
  * plan gaps: `set_start` (start-tile override), `set_teleport_to`
    (teleport-with-settle shaping), multi-die `MoveCtx.Base` tables, and
    in-flight move writeback (counteractions setting `m.Stopped`/`m.ExtraSteps`);
  * **targeting** is landed in v26 (`target`/`target_all`/`target_tile`,
    `ImmuneAll`/`Untargetable`/`Redirect`, `targeted_count`) and, since the
    2026-10-06 group-A wave, the **static targeting query** (`designations`,
    C# `H.Db.Card(id).Targeting` + `H.Others`, declared via
    `prop::DESIGNATES`) and **per-pair cancel** (`cancel_designation` /
    `designation_cancelled`, C# `play.Tags["immune"+seat]` -- one designation
    drops, the rest land). What remains is `PlayCtx` `Effective`/`Extreme`,
    `t.Pay.tile`, the skill system (`TryResonance`, `SwitchState`, ...), and
    host->guest strings (`AbName`/`EventTitle`);
    (`H.ExtraOf` skill attachments landed in v35 as `band_skills`' `extra`);
  * routines (`BuyRoutine`/`BuildRoutine`/`MortgageRoutine`/`SettleAt`),
    `AgentLanding`, `RevealSeen`/`Actions`, world snapshot/restore.

Those are engine-side extensions (a nested `Cx` run inside a host call shares the
answer log; the `Fx` hook set becomes new `CardRules` methods), not rewrites of
what is here. The engine seam is co-developed with two peer sessions -- check
`ListAgents` before editing `game-core`/`game-rules`/`card-sdk`.