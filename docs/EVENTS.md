# Event card rules (事件卡)

Standardize event-card behaviour the same way card, skill and tile rules are
implemented. Where [CARDS.md](CARDS.md) covers what a *card* does,
[TILES.md](TILES.md) covers what a *board tile* does and [ENGINE.md](ENGINE.md)
covers the match shell, this doc covers what an **event card** does when it is
drawn, while it is in play, and when it expires.

Status: **the category is in** (`rules/events`, 28 events, one `CardDef` each).
The engine keeps only the generic mechanics. What each body still owes the
sheet text is tagged `TODO(规则书)` in the bodies and listed under
[Left short](#left-short).

## Why

The rulebook says an event is drawn, made public, takes effect immediately and
then goes to the discard (`data/rules.txt`, 「基础[结算]规则」, lines 97–99):

> · CiRCLE咖啡厅和流星堂的的[结算]是：抽取一张手卡，然后抽取一个事件卡。
> 　· 抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。
> 　· 事件结算后进入事件弃卡区。如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。

and the per-event text lives in `data/events.json` (the sheet's 中立事件 tab).
The engine's job is the deck and the filing; **what the event does is content**,
the same split [TILES.md](TILES.md) draws for tiles. Until now the engine had
the deck and a `CardRules::event` hook but no rules crate, so every event fell
through `StubRules::event` (「还没有移植」) and went straight to the discard --
`MatchState::event_active` existed and was never written.

## The crate: `rules/events`

One crate, one `.rs` per event, one `CardDef` each -- the same authoring shape
as `rules/cards/card-*`, `rules/skills/*` and `rules/tiles`:

| id | file | kind | sheet |
|---|---|---|---|
| `event:对邦` | `duobang.rs` | one-shot | 中立事件 A2 |
| `event:弦卷集团地产开发` | `tsurumaki_estate.rs` | one-shot | A3 |
| `event:很噜的感觉` | `lulu.rs` | one-shot | A4 |
| `event:前往哈比内尔王国旅游` | `habinel.rs` | one-shot | A5 |
| `event:这只手我不会放开` | `this_hand.rs` | one-shot | A6 |
| `event:PICO灵魂交换` | `pico_swap.rs` | one-shot | A7 |
| `event:协助CiRCLE重建` | `circle_rebuild.rs` | active | A8 |
| `event:前场队还是后场队？` | `front_or_back.rs` | one-shot | A9 |
| `event:EX任务挑战` | `ex_quest.rs` | one-shot | A10 |
| `event:发送熊饼表情` | `bear_cookie.rs` | one-shot | A11 |
| `event:飞鸟山之战` | `asukayama.rs` | one-shot | A12 |
| `event:幻觉来了` | `hallucination.rs` | active | A13 |
| `event:卡池BUG` | `pool_bug.rs` | active | A14 |
| `event:A！A！O！` | `a_a_o.rs` | active | A15 |
| `event:麻里奈小姐的礼物箱` | `marina_box.rs` | active | A16 |
| `event:元祖！邦多利酱` | `bangdream_chan.rs` | active | A17 |
| `event:超燃甩头` | `heat_head.rs` | active | A18 |
| `event:意外的对邦` | `surprise_duel.rs` | active | A19 |
| `event:Forbidden Moca` | `forbidden_moca.rs` | active | A20 |
| `event:上学时间` | `school_time.rs` | one-shot | A21 |
| `event:泪水的含义` | `tears.rs` | one-shot | A22 |
| `event:Kizuna Music` | `kizuna_music.rs` | one-shot | A23 |
| `event:冲榜` | `chart_rush.rs` | derived one-shot | A24 |
| `event:修复公告` | `fix_note.rs` | active | A25 |
| `event:迷子的追逐` | `lost_chase.rs` | derived one-shot | A26 |
| `event:让我来结束一切` | `end_it_all.rs` | derived, permanent | A27 |
| `event:一切不会结束` | `never_ends.rs` | derived one-shot | A28 |
| `event:火种燃尽之后会怎么样呢？` | `embers.rs` | active | A29 |

The crate is a **rule crate**, not a card crate: its ids are `event:*`, not
`data/cards.json` ids. `tools/rules-aggregate.mjs` scans `rules/events` as a
fourth root (besides `rules/cards`, `rules/skills` and `rules/tiles`) so
`card-all` links it; `tools/build-ruleset.mjs` runs that aggregate first.

Each body quotes the event text from `data/events.json` and cites it per line,
exactly like a card (`docs/CARDS.md` → 「Every line cites the rule book」).
`python tools/rulebook/check.py` requires the quote on `rules/events/*` the way
it does on `rules/cards/*` and `rules/tiles/*` -- the passage is the event's
`text` field, since `docs/rulebook/cards.json` has no `event:*` ids.

Log lines are `log.event.*` keys in `webui/src/i18n/locales/<lang>/game.json`,
the same namespace the tile bodies use (`log.land_*`).

## The engine keeps the generic mechanics

`draw_event` (`crates/game-core/src/engine/play.rs`) is the whole of the
engine's event logic, and it is deliberately per-event-free:

```
draw the top of event_deck            (an empty deck takes the shuffled event_discard
                                       once the draw has fully resolved, and before a
                                       draw as a fallback: World::refill_event_deck)
log the public reveal (log.event)
raise `event`                         -- the [反击] window on the draw
bind the rule instance on BOARD_OWNER -- bind_event, one per event id
rules.event(...)                      -- the body's On::Play
if the body did not stay:             -- one-shot / negated
    expire_event(id, derived?)        -- unbind + file away
raise `eventAfter`
```

The event-deck reshuffle waits for everything to resolve (user ruling
2026-10-07: "Wait for everything to resolve, only then reshuffle the event
deck back"), which is also the rulebook's shape (`data/rules.txt` 基础[结算]
2.2: 「事件结算后进入事件弃卡区。如果没有事件可抽取则将事件弃卡区洗切并当作新的
事件卡堆来抽取。」). The hand-card draw pile is the opposite: it refills as
soon as it empties (「当抽卡区抽光时将弃卡区洗卡并放回抽卡区」, 规则书 游戏流程 2).

The event tile's hand draw runs through the engine immediately, including
empty-deck maintenance and per-card hooks, before the event draw can prompt.
Card-module replay restores the pre-draw piles and adopts each completed host
request at its original statement, so discard/redraw effects do not repeat
their pile writes on the newly drawn hand.

Everything else is the rule's:

| mechanic | where |
|---|---|
| what happens on draw | the body's `On::Play` |
| ongoing effects | `On::Hook` entries, dispatched like a tile instance's |
| counters | the instance's `crystals` / `props` (view mirrors them into `ActiveEvent`) |
| expiry | `ctx::event_expire(id, removed)` -- 「放入事件弃牌」 / 「永久移除」 |
| derived-on-deck-top | `ctx::event_deck_push(id, face_down)` -- 「背面朝上放置于事件牌堆顶部」 |
| permanent removal | `ctx::event_banish(id)` -- 「从所有非衍生事件中选择3个移除」 |

**`StubRules` is the built-in fallback**, the same shape as `land_at_built_in`:
with no `event:*` rule in the ruleset, `bind_event` binds nothing,
`StubRules::event` logs `log.event_not_ported` and returns "does not stay", and
the engine files the card away. `crates/game-core/examples/sim.rs` runs
`StubRules`, so it keeps drawing events (no effect) at the same cost as before.

## Binding

When an event is drawn and its rule exists, the engine places **one rule
instance on the neutral board owner** ([`BOARD_OWNER`] = `-1`):

* **Storage** is `World::board_field`, the same `FieldCard` list tile rules
  use -- but with `tile = -1`, because an event governs a *card in play*, not a
  board square. `card = event:<id>`.
* **`bind_event`** runs from `draw_event`, next to the raise, and is idempotent
  per event id. The body's `On::Play` therefore runs against a live instance
  (`is_placed` / `crystals` / `props` work).
* **`expire_event`** (or `unplace_self` from the body) drops the instance and
  the `ActiveEvent` row together, then files the card to `event_discard` or
  `event_removed`.
* **Hook dispatch** visits event instances the same way it visits tile
  instances (`docs/TILES.md`): a tile-carrying trigger reaches the tile's own
  rule instances *plus* every active event instance (they are not
  tile-governed); a trigger with no tile reaches the board list only when some
  `tile:*` or `event:*` rule declares the hook.

### The active list is a view

`MatchState::event_active` (`ActiveEvent { id, playerId, counter, counter2,
note, faceDown }`) is the public view of what is in play. The **source of truth
is the rule instance**: `public_state` mirrors `crystals` into `counter`,
`props["count2"]` into `counter2`, and the instance's `note` / `faceDown` into
the row. A body that counts down with `ctx::decay()` or `set_crystals` shows it
for free.

## ABI and SAVE_VERSION

Checked: `card_sdk::abi::ABI_VERSION = 39`,
`game_core::engine::mod::SAVE_VERSION = 3` (the match save),
`game_core::profile::SAVE_VERSION = 3` (the player profile -- untouched here).

| bump | when | why |
|---|---|---|
| **ABI → 37** | the migration | `ctx::event_expire` / `event_is_active` / `event_deck_push` / `event_banish`, and the `event:*` rule-id convention |
| **ABI → 39** | the discrepancy pass | `plan::set_roller` (幻觉来了's roll substitution) and `prop::NO_BUILD_ABOVE` (卡池BUG's build gate) |
| **SAVE_VERSION** | unchanged | `MatchState::event_active` and `World::board_field` were already in the v3 save. A v2 save is still rejected; a v3 save with an empty active list still loads (it simply has no event in play). |

## Conventions

* The rulebook / `data/events.json` decides *what*; the C# port is not a spec.
* No raw money pokes, no side-channel flags, no prose-keyed behaviour.
* 「…时」 clauses go in event handlers on that state's change, not inline
  checks at each spend site.
* Anything left short of the text gets a `TODO(规则书)` naming the gap.
* Commit nothing.

## Left short

The category is in and every event has a body. What the bodies still owe the
sheet text, by kind of gap (each tagged `TODO(规则书)` at the clause):

* **「可[消耗]」 default polarity** (麻里奈小姐的礼物箱): the pass opens an
  optional prompt, but its fallback (the default) is 「消耗」 rather than
  「不消耗」 -- two of the behaviour tests are written against `drain()`
  auto-paying. Flipping the default needs those expectations updated.
* **No auction primitive** (弦卷集团地产开发): the body runs a sealed high-bid
  over `ask_pick` (narrowed candidates), not the engine's raise-and-bend
  auction. A tie goes to the earliest seat.
* **No global hand-effect veto** (A！A！O！'s 「不能使用卡牌的[手]效果」).
* **No clock surface** (元祖！邦多利酱's 免费时间 / 恢复时间) -- host clocks.
* **No skin change** (EX任务挑战) -- cosmetic only, so the body logs eligibility.
* **优先权 claim** (协助CiRCLE重建's 「该改变[触发结算]的效果优先于其他任何
  改变[触发结算]的效果」): instances run in placement order, not by priority.
  The CiRCLE settle replacement, the cafe override (「CiRCLE原本的所有效果
  迁移至CiRCLE咖啡厅」 -- including the [经过] reward) are implemented.
* **「减少1d6」 as a subtraction** (超燃甩头 phase 2) rather than a negative die
  in the pool -- the dice plan has no negative-die op.
* **Song-card count** (Kizuna Music's X) is a duplicated id table (from
  `data/song_cards.json` + `data/cards.json`) counted from each player's draw
  pile; `GameData.song_cards` is not reachable from `ctx`.
* **Non-derived event id table** (让我来结束一切's 3 picks) is duplicated from
  `data/events.json` (`derived` is not reachable from `ctx`).
* **「累计在上述格子上加盖后」** (修复公告) read as one build per designated tile
  (3 total); the text does not say how many.
* **飞鸟山之战's B12 「衍生迷子的追逐」** is a sheet annotation the event text
  does not carry -- not triggered.
* **「下2回合开始时」** (意外的对邦) read as the start of the drawer's 2nd turn.
* **很噜的感觉's shout** is out-of-game flavour, logged only.

## Per-step results

| step | build-ruleset | game-core | game-rules | rulebook | notes |
|---|---|---|---|---|---|
| events category | ok (285 rules) | green (incl. 7 new `tests/events.rs`) | green, intentional ignores only | 28 events, 0 problems | ABI v37; sim on StubRules **261.6 ms/game** (budget ≤ 323), shell event counts identical to baseline (rent 19,612 / build 4,641 / buy 2,908 / forcebuy 415 / circle money 5,915 / agent half rent 4,221) |

`cargo test -p game-rules --no-fail-fast` was green apart from three
`cp_*` tests in `rb_general.rs` and one `rules/tiles/src/cp.rs` quote problem
in `check.py` -- both the concurrent [CP点] tile-mark batch, not this migration.

Events with no dedicated test are listed in [TEST-FINDINGS.md](rulebook/TEST-FINDINGS.md)
(all 28 of them). A black-box `rb_events.rs` agent can be added later; the
engine tests here cover activate → effect → expire for the generic mechanics.
