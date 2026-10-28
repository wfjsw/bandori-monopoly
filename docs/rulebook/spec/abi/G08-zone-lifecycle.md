# ABI design G08 — Zone and attachment lifecycle

**Status:** draft for user review · design only, no source touched.
**Worktree:** `.claude/worktrees/card-rulebook-review` (branch `worktree-card-rulebook-review`).
**Group:** G08, family "Zone and attachment lifecycle".
**Ground truth:** `docs/rulebook/cards.json`; every fragment below is verbatim from it.

Terminology: **attachment** = a card instance placed on another card instance
(`FieldCard.on`), never a "token"; **leave** = any move out of `Field` or
`Attached`; **zone transaction** (`ZoneTxn`) = the one kernel move this design
introduces. "Hook" = a passive listener, as in G03/G06.

**Closed slugs and rows:** all 10 items are covered.

| # | slug | severity | rows |
|---|---|---|---|
| 1 | `nyamu-face-flip-trigger` | high | 347, 348, 1043, 1272 |
| 2 | `one-of-us-rehome` | medium | 1107 |
| 3 | `one-of-us-cause` | low | 1124 |
| 4 | `same-sky-mulligan-order` | low | 32 |
| 5 | `cardplayed-payload` | medium | 34, 677, 680 |
| 6 | `hand-changed-event` | high | 57, 58, 197, 642, 1688 |
| 7 | `wasted-flag` | medium | 510, 522, 523, 1396, 1701 |
| 8 | `card-on-card` | high | 1075 |
| 9 | `here-the-world-leave-hook` | high | 639, 645, 867, 875, 1074, 1252, 1474, 1488, 1514, 1515, 1689, 1695 |
| 10 | `tomori-earliest-owned` | medium | 188, 994 |

**Demotions:** item 10 is **demoted to A** (engine-maintained `firstTile`, no ABI
surface). Items 4 and 7, and the cancelled-flag half of item 5, also have
A-shaped fallbacks but are kept in B because the A shape is either mistimed (4)
or not composition-safe (5, 7); each is stated inline.

---

## 1. Problem, per item

### 1.1 `nyamu-face-flip-trigger` (rows 347, 348, 1043, 1272)

Book (`Mujica:（喵梦）`):

> 1. 每当此卡将从正面翻至背面时可选择失去2个火罐。
> 2. 每当此卡将从背面翻至正面时此卡拥有者抽一张卡。

No trigger exists at a flip. Clause 1 is not implemented at all; clause 2 is
sampled from `self_face_down()` in a `TurnEnd` hook (`nyamu_card.rs:63-73`).
That sample cannot run while the card is down: the field-hook walk skips every
face-down instance (`wasm_rules.rs:5067-5071`, `!world.card_face_down(...)`),
so `WAS_DOWN` stays `0` and the draw is dead in the ordinary path. The pre/post
split is also load-bearing: clause 1 is 「将从…翻至」 (before the state changes),
clause 2 「将从背面翻至正面时」 is the moment of the flip-up.

### 1.2 `one-of-us-rehome` (row 1107)

Book (`AG:ONE OF US`):

> 并转移到存活方的游戏区

`before_out` (`one_of_us.rs:313-354`) rewrites **tile** ownership only. The card
itself stays on the bankrupt side's field row and `remove_from_game` clears that
row (`play.rs:4291-4293`), so the card and its two deed marks vanish. The
suggested workaround `transfer_to_dest(Dest::Field)` is a no-op for a placed
instance: `move_now` (`wasm_rules.rs:671-685`) has no `Field` branch, so the card
is unplaced and dropped. `set_field_owner` (`ops.rs:1285`) stamps `owner`/`user`
but does not move the instance between players' field vectors.

### 1.3 `one-of-us-cause` (row 1124)

Book:

> 当其中一方破产时

`BeforeBankrupt` is an alias of `BeforeOut` (`abi.rs`, kind 65) and is raised by
both `bankrupt()` (`play.rs:4205`) and `forfeit()` (`play.rs:4239`). The trigger
carries no cause, and `ctx::player_out` answers only "gone", so a forfeit runs
the bankruptcy hand-over. The two raise sites are the only place the fact exists;
it has to enter the payload there.

### 1.4 `same-sky-mulligan-order` (row 32)

Book (`AG:朝同一片天空迈进`):

> （开局时抽到此卡洗回）

The clause is an opening-hand-entry clause. The opening deal ends at
`play.rs:411-414`; `DeckAtGameStart` is raised only after the mulligan barrier
(`play.rs:481-483`). The copy is therefore visible through the 初始重抽 window.
The deal itself raises nothing (the doc comment at `play.rs:395-399` says so),
so there is no point a card can key on. `DeckAtGameStart` cannot simply be moved
earlier: `PPP:Returns`' [特] setup and every other 游戏开始时 effect belong to
时点流程 step 7, after step 6's mulligan.

### 1.5 `cardplayed-payload` (rows 34, 677, 680)

Book (`AG:朝同一片天空迈进`):

> [反击]抽出此卡时立刻打出

`cardPlayed` is raised unconditionally at `play.rs:4896-4904`, after the cancel
branch at `4852-4861`, so a counteracted play still announces as if it resolved.
It carries `card` / `cards` / `by_card` but no outcome. Separately, a hand
[反击] resolved through the counteraction tree (`hand_counteractions` →
`resolve_rounds`) raises `card`/`cardUsed` but never `cardPlayed`, and
`same_sky.rs:61-80` bypasses the whole pipeline with `discard_from_hand`, so its
auto-play announces nothing either. `ctx::raise` (`ctx.rs:1838`) carries only a
value, so a card cannot synthesise the payload itself.

### 1.6 `hand-changed-event` (rows 57, 58, 197, 642, 1688)

Book (`CRYCHIC` band skill (3)):

> （不触发乐队技能的（3）效果）

Three cards need "a card arrived in a hand":

* `skill:CRYCHIC:美好的往日幻影` (3) is a 4→5 hand-size crossing
  (`crychic.rs:102-112`); it rides `Drew`, so a tutor, bounce or hand counter
  return is invisible.
* `forever.rs:30-49` 「此卡立即置入弃牌堆」 is a standing `hand >= 7` condition;
  it re-checks only on draw/discard/play/turn boundaries, never on a plain add.
* `auction_pulled.rs:21-26,47-58` 「此卡加入手牌时」 rides `Drawn` with a
  `trigger::card_is(ID)` guard; a tutor such as 相信你 (`believe_you.rs:101-103`)
  calls `add_to_dest(Hand)` and never places it.

Today `add_to_dest(Hand)` raises `drew` only inside a `DrewBefore` replacement
(`wasm_rules.rs:1042-1055`), and `take_card` plus plain adds raise nothing.
`fate_together.rs:145-158` returns itself with `Dest::Hand` and must *not* count
for band (3) (the book's own parenthetical), so the event needs a counted flag,
not just a kind.

### 1.7 `wasted-flag` (rows 510, 522, 523, 1396, 1701)

Book (`MyGO!!!!!` band skill (3)):

> 每次你的卡在未生效的情况下进入弃牌堆时

and (`MyGO!!!!!` `[奇迹水晶]` card (3)):

> 若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效。

`miracle.rs:172-197` reads its crystals, skips the payoff and logs
`miracle_wasted`; nothing on the resulting discard says "ineffective". Band (3)
(`mygo.rs:32-40`) rides bare `Discarded` + `pre::MINE`, so every owner discard
grants a crystal, and a counteracted play also counts. The fact must ride the
discard transition, because only the transition knows both the cause and the
per-play effectiveness.

### 1.8 `card-on-card` (row 1075)

Book (`Sumimi:Here the world` (2)):

> 将那张卡背面朝上放置于此卡上并为其放置3个奇迹水晶

The held card is a card id packed into sixteen i32 props plus a length counter
(`here_the_world.rs:24-85`). It is not an instance, cannot be flipped, cannot be
addressed, does not sit in any zone, and is stranded when the host leaves
(nothing returns or discards it). The placement the book prints — a face-down
card *on a card* — has no representation at all.

### 1.9 `here-the-world-leave-hook` (rows 639, 645, 867, 875, 1074, 1252, 1474, 1488, 1514, 1515, 1689, 1695)

Book (`Mor:夏日合宿`):

> 当此卡效果结束

and the same point is read by five more cards:

* `summer_camp.rs:30-33` ends only at the owner's turn start; an earlier leave
  loses the draw and never resets the negation count.
* `auction_pulled.rs:60-62` writes `fire::add_max` and never reverts it.
* `guerrilla.rs:219-271` writes a `colorFor` prop and never clears it.
* `soyo_colors.rs:105-110` arms `ANY_COLOR` and never disarms it.
* `resolve.rs:30-33` pays 1000 only on its own play, not on any removal.
* `here_the_world`'s held card is stranded (1.8).

`unplace_at` (`ops.rs:1511-1529`) raises nothing; `apply_dest`
(`wasm_rules.rs:5847-5881`) raises `discarded` or logs a removal, and the
bankruptcy purge (`remove_from_game`, `play.rs:4255-4310`) clears vectors
directly. Only a **player** departure raises `leaveAfter` (`play.rs:4243`), which
is why the family is grouped under this slug. A card-level leave point plus a
zone-agnostic removal point is the generic capability.

### 1.10 `tomori-earliest-owned` (row 188, 994) — **demote to A**

Book (`CRYCHIC:（灯）内心的呐喊` (3)):

> 打出此卡时将其放置于你最早拥有的格子上

`tomori_inner_shout.rs:133-140` uses `owned.iter().min()`, the lowest board
index. `ctx::slot("firstTile")` is read (C# `H.V(i, "firstTile") - 1`) but never
written. Acquisition order is not stored anywhere: `owners` is tile→owner only.

**A shape:** the engine stamps a monotonic acquisition counter per tile in
`MatchState`, and maintains each seat's `firstTile` slot as the still-owned tile
with the smallest stamp on every acquisition and every loss. No import, no
manifest change; the card already reads the slot. This is the recommended fix
and is what section 5.9 specifies. The ordered-list variant is alternative A2.

---

## 2. Shared root causes

1. **No choke point.** A card changes zone through `add_to_dest`
   (`wasm_rules.rs:1041`), `take_card` (`:1092`), `to_discard` (`:1128`),
   `move_now` (`:671`), `apply_dest` (`:5847`), `unplace_at` (`ops.rs:1511`),
   `unplace_card` (`ops.rs:1784`), the draw push (`play.rs:4609-4612`), the
   play remove/dest pair (`play.rs:4787`, `4865-4883`), the hand-counter remove
   and dest (`wasm_rules.rs:6128-6134`, `6400-6421`) and the purge
   (`play.rs:4266-4310`). Every site has its own event/log behaviour.
2. **Zone is implicit.** `Dest` (`ctx.rs:1989`) is a fate the caller names, not a
   transition record: no from-zone, no cause, no holder, no applied result. The
   same `Dest` value means different things from hand and from field.
3. **A hand card has no identity.** Hand/deck piles are `String` vectors, so a
   "the card that just arrived" fact can only be a name; `HandChanged` must be
   dispatched to the card's def with uid `-1`.
4. **Attachment has no address.** `FieldCard` has `tile` but no host; the only
   "placed on" relationship is name lookup or packed props.
5. **Face-down is a dispatch kill switch**, not a scoped property: the single
   `!card_face_down` guard at `wasm_rules.rs:5069-5071` removes every hook,
   including the hooks about the instance's own lifecycle.
6. **Two out causes, one trigger.** `BeforeOut` is shared by bankruptcy and
   forfeit; the fact is live only at the two raise sites.
7. **Ownership has no history.** Tile→owner overwrite loses acquisition order.

The fix is one transition kernel plus a small, typed event set; every listed
site is rewritten to go through it, so no site can forget a step.

---

## 3. Capability as an invariant

> **Every zone change is one `ZoneTxn` applied by one kernel, and it emits
> exactly one primary typed transition with cause and destination. Leaving the
> field is total and observable. Attachment placement is a first-class state of
> the placed instance, and an attachment never outlives its host silently.
> A hand arrival is a typed transition, witnessed or unwitnessed. A discard
> records whether it took effect. Out has a cause. Face-down suppresses
> continuous behaviour, not lifecycle hooks.**

Formally, for every instance `u` and every applied transaction `tx`:

* **I1 (totality):** `tx.from ∈ {Field, Attached}` ⇒ exactly one `cardLeft` with
  `uid = u`, `from = tx.from`, `to = tx.to`, `cause = tx.cause`, after exactly
  one `cardLeaveBefore` with the same payload.
* **I2 (removal):** `tx.to = Banished` ⇒ exactly one `cardRemoved` with `uid`,
  `from`, `cause`, in addition to I1 when applicable.
* **I3 (attachment):** `f.on = Some(h)` ⇒ `h` exists and is in `Field` or
  `Attached`; the chain is acyclic; `attached_cards(h)` is exactly
  `{u | f_u.on = Some(h)}` in attach order.
* **I4 (cascade):** if `u` leaves with attachments `a₁…aₖ`, exactly `k`
  `HostLeft` transactions occur, one per attachment, each from `Attached`, each
  to the attachment's own destination (default `Hand`), all before `u`'s
  commit; after them `attached_cards(u) = ∅`.
* **I5 (flip):** `set_face_down_at(u, d)` changes state **iff** it raises
  exactly one `cardFlipBefore` then exactly one `cardFlipped`, both with `uid = u`;
  a no-op flip raises neither.
* **I6 (hand):** every commit into `Hand` raises exactly one `handChanged` with
  `uid = -1`, `card`, `to = Hand`, `cause`, `counted = tx.counted`, dispatched
  to the arriving card's def and the field/lingering walk.
* **I7 (wasted):** `Discarded.wasted = true` iff the play context called
  `set_ineffective()` or the play outcome is `Cancelled`; false otherwise.
* **I8 (out cause):** exactly one `beforeOut` per departure, with
  `out_cause = Bankrupt` from `bankrupt()` and `Forfeit` from `forfeit()`.
* **I9 (play announce):** exactly one `cardPlayed` per hand play, raised at its
  commit, `play_outcome` equal to the actual outcome (including hand counters).

Kani obligations in section 9 are the machine check of I1-I9.

---

## 4. Surface

Everything is generic mechanism: no display text, no player decision inside an
import. The enums live in `crates/game-kinds/src/vocab.rs` (the `Payee` pattern),
are re-exported through `card_sdk::abi` at the existing line 1534
`pub use game_kinds::{…}`. `ctx.rs:14` re-exports only `AbKind, CardPile`, so
the enum joins that line (not the `Dest` site at 1989) and cards compare
`ctx::OutCause::Bankrupt`, never a literal. `Zone` / `ZoneCause` join it too.

### 4.1 Vocabulary

```rust
/// A zone a card instance (or a named card without an instance) can be in.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq,
         serde::Serialize, serde::Deserialize)]
pub enum Zone {
    /// Not instantiated yet (place_card / attach_card creation).
    #[default]
    Nowhere = 0,
    Deck = 1,
    Hand = 2,
    Field = 3,
    Discard = 4,
    Banished = 5,
    /// The event deck (event-rule instances).
    EventDeck = 6,
    /// On another instance (`FieldCard.on`).
    Attached = 7,
}

/// Why a transition happened. Stable wire codes.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq,
         serde::Serialize, serde::Deserialize)]
pub enum ZoneCause {
    Draw = 0,
    /// The play's own hand → fate transition.
    Play = 1,
    /// A deferred `Dest` applied at run commit.
    #[default]
    Resolve = 2,
    /// An import called by a card body (`add_to_dest`, `send_to_dest`, …).
    Effect = 3,
    /// Discarded as a cost.
    Cost = 4,
    HandLimit = 5,
    Mulligan = 6,
    /// Opening deal / opening return.
    Opening = 7,
    /// Cascade from the host leaving.
    HostLeft = 8,
    Bankruptcy = 9,
    Forfeit = 10,
    /// The whole field row moved between seats (`rehome_card`).
    Rehome = 11,
    /// A lingering / event instance expiring.
    Expire = 12,
    Attach = 13,
    Detach = 14,
    /// Game / deck setup.
    Setup = 15,
    Other = 16,
}

/// How a hand play ended.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq,
         serde::Serialize, serde::Deserialize)]
pub enum PlayOutcome { Resolved = 0, Cancelled = 1 }

/// Why a player left the game (`beforeOut` / `leaveAfter` payload).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq,
         serde::Serialize, serde::Deserialize)]
pub enum OutCause { Bankrupt = 0, Forfeit = 1 }
```

### 4.2 Engine kinds

Eight engine ids appended to the positional table, which currently ends at 110
(`None = 0`, `PaySplitMulBefore = 110`); 111-113 belong to G01, which lands
first, so the rows are `114-121` (this group's README band). Each gets an
`ActionKind` constant (`abi.rs:1762+`), a `TriggerKind::engine(n)` const
(`:2396+`), a matching `Action::…` const and an `action_kinds!` row
(`crates/card-sdk/src/kinds.rs:638`); all are `hook_only` (passive — none opens
a counteraction window):

| id | wire string | const | `hook_only` | payload |
|---|---|---|---|---|
| 114 | `cardLeaveBefore` | `CardLeaveBefore` | yes | uid, from, to, cause |
| 115 | `cardLeft` | `CardLeft` | yes | uid, from, to, cause |
| 116 | `cardRemoved` | `CardRemoved` | yes | uid, from, cause |
| 117 | `handChanged` | `HandChanged` | yes | card, cards, to=Hand, cause, counted |
| 118 | `cardFlipBefore` | `CardFlipBefore` | yes | uid, value = face it is about to become |
| 119 | `cardFlipped` | `CardFlipped` | yes | uid, value = face it became |
| 120 | `openingHandDealt` | `OpeningHandDealt` | yes | card, player, value = deal index |
| 121 | `cardRehomed` | `CardRehomed` | yes | uid, from-seat, to-seat |

Faces are the existing booleans flattened to `value` (`0` = face-up,
`1` = face-down) on the two flip kinds; `from`/`to` of a zone kind ride new
`Trigger` fields (4.3), not `value`.

### 4.3 Trigger payload (`game-core/src/engine/rules.rs`, `Trigger`)

```rust
pub struct Trigger {
    // …existing fields…
    /// The instance this transition is about, or -1 (a hand/deck card, a skill).
    pub uid: i32,
    pub from: Zone,
    pub to: Zone,
    pub cause: ZoneCause,
    /// `Discarded` only: the card did not take effect (I7).
    pub wasted: bool,
    /// `HandChanged` only: counts for crossing watchers (I6).
    pub counted: bool,
    /// `CardPlayed` only.
    pub play_outcome: Option<PlayOutcome>,
    /// `BeforeOut` / `LeaveAfter` only.
    pub out_cause: Option<OutCause>,
    /// `CardLeft` / `CardRemoved` only: the host it was attached to, or -1.
    pub host: i32,
}
```

`Trigger` exists twice: the engine struct here and the **guest mirror**
`game-rules/src/world.rs` (what `ctx::trigger` reads through
`st().wr().trigger()`). Both get every field below, and `bridge_trigger`
(`wasm_rules.rs:7270-7331`) copies each one -- a getter that reads a field the
bridge never fills returns the default forever.

`Trigger` is not serialized; every new field is initialised in `Trigger::new`
(`rules.rs:291`): `uid = -1`, `from = to = Zone::Nowhere`, `cause = Other`,
`host = -1`, flags false, options `None`. New getters in `ctx::trigger` (the
existing module, `ctx.rs:2962`):

```rust
pub fn uid() -> i32;                                  // -1 none
pub fn from_zone() -> Option<Zone>;                     // Nowhere ⇒ None
pub fn to_zone() -> Option<Zone>;                       // Nowhere ⇒ None
pub fn cause() -> ZoneCause;                          // Other when unset
pub fn wasted() -> bool;
pub fn counted() -> bool;
pub fn play_outcome() -> Option<PlayOutcome>;
pub fn out_cause() -> Option<OutCause>;
pub fn host() -> Option<i32>;
```

`trigger::uid()` is the identity discriminator; hooks that mean "this
instance" compare it with `ctx::self_uid()` in a residual, never a name (two
copies share a name).

### 4.4 New imports (`card_sdk::ctx`)

```rust
/// Attach `card` to a placed instance, face-down or not. `holder` is the
/// attached instance's owner (its pile is where a cascade defaults to).
/// Returns the attached uid, or -1 when the host is gone.
pub fn attach_card(holder: i32, host_uid: i32, card: &str, face_down: bool,
                   note: &Msg) -> i32;
/// Detach `uid` to `dest` for its owner; one `cardLeft` with cause `Detach`.
pub fn detach_card(uid: i32, dest: Dest) -> bool;
/// Attached instance uids of `host_uid`, in attach order.
pub fn attached_cards(host_uid: i32) -> Vec<i32>;
/// The host of an attached instance.
pub fn attach_host(uid: i32) -> Option<i32>;
/// The owner of the running instance (uid-addressed, unlike a name lookup).
/// Needed where the book names 拥有者 while the hook's actor is someone else.
pub fn self_owner() -> i32;
/// Move the whole field instance `uid` (props, counters, marks, face state,
/// `on`, tile) to `seat`'s field; owner/user become `seat`.
/// One `cardRehomed`; no leave. False when the instance or seat is gone.
pub fn rehome_card(uid: i32, seat: i32) -> bool;
/// Owner-stamp only: `owner`/`user = seat` without moving the row (a card
/// parked on another player's field). False when the instance is gone.
pub fn set_field_owner(uid: i32, seat: i32) -> bool;
/// Put one named card into a hand. `counted` is the `HandChanged.counted`
/// flag (I6); plain `add_to_dest(Hand)` is `counted = true`.
pub fn add_to_hand(player_id: i32, card: &str, counted: bool) -> bool;
/// The next `Dest::Hand` of this run lands `counted = false`
/// (「不触发乐队技能的（3）效果」).
pub fn set_dest_unwatched();
/// This card / instance did not take effect; its `Discarded` is `wasted` (I7).
pub fn set_ineffective();
/// `ctx::play_card(id, player_id)` already runs a card play (its argument order
/// is id first); this wrapper is the hand-source spelling with the seat first
/// and `Dest` parity. Use `play_card` when the card need not come from the
/// hand.
/// Run `card`'s hand play through the real pipeline from inside a body
/// (auto-play): remove from hand, declaration, counter window, body, `Dest`,
/// `discarded`, `cardAfter`, `cardPlayed`. The declaration prompt is the
/// card's own existing pipeline, not a new decision this import invents.
/// Returns its `Dest`.
pub fn play_hand_card(player_id: i32, card: &str) -> Result<Dest, Prompt>;
```

### 4.5 Extended imports / wrappers

| import | before | after |
|---|---|---|
| `unplace_at(uid)` | removes the row, raises nothing | full transaction: cascade, `cardLeaveBefore`, `cardLeft`, returns owner |
| `set_face_down_at(uid, on)` | writes the bool | flip kernel, I5 (pre/post kinds) |
| `set_card_face_down(player, card, down)` | first name match, writes the bool | resolves the first match's **uid**, calls the flip kernel |
| `self_face_down()` | reads the name match | `uid = self_uid()`, uid read (two copies stay distinct) |
| `add_to_dest(…, Hand)` | conditional `drew` | `handChanged(counted = true)`, cause `Tutor`; `drew` compatibility kept |
| `take_card(_, _, Field)` | drops the row | remove transition, `cardRemoved`, cause `Effect` |
| `send_to_dest` / `transfer_to_dest` | `move_now` has no `Field` arm | all `Dest` arms incl. `Field` (rehome self), `DeckTop/Bottom/Random` |
| `set_dest(Banished)` commit | log only | `cardRemoved` |
| `set_dest(Hand)` commit | silent push | `handChanged`, cause `Return`, honoured `set_dest_unwatched` |

`place_card` / `place_card_on` create instances `Nowhere → Field`; no enter
event is introduced (nothing in G08 needs one).

### 4.6 State

```rust
// FieldCard (game-core/src/state.rs:1096)
/// Host uid when this instance is attached (`Zone::Attached`); attached
/// instances carry `tile = -1` and `face_down = true` unless a card says
/// otherwise. One level of meaning: `attached_cards` is derived from this.
#[serde(default)]
pub on: Option<i32>,
```

`MatchState` gains, for item 10's A fix:

```rust
/// Monotonic acquisition stamp per tile; 0 = never owned.
#[serde(default)]
pub acquired_at: Vec<u32>,
#[serde(default)]
pub acquired_seq: u32,
```

No `MatchPlayer` vector is needed (alternative A2 keeps one).

### 4.7 Manifest / entry contract

`ManifestOn` shape is unchanged. The wire break is the new **kind ids** carried
in each entry's `triggers` list plus the new import table: a ruleset built
against v106 references 114-121 and imports a v104 host does not have.
`CardDef` sugar is table-only (`Action::CardLeft(…)` etc. write the ids), so
nothing is translated at runtime.

### 4.8 Classification

| item | NEW | EXTENDED | WIRE-BREAK | NO-WIRE |
|---|---|---|---|---|
| 1 | `CardFlipBefore/Flipped`, `set_face_down_at` semantics | two face flip wrappers | kind 115/116 | |
| 2 | `rehome_card`, `set_field_owner`, `CardRehomed` | `move_now` `Field` arm | kind 118 | |
| 3 | `OutCause`, `out_cause` payload/getter | `beforeOut` raise sites | `Trigger` out_cause | |
| 4 | `OpeningHandDealt` | opening deal | kind 117 | |
| 5 | `PlayOutcome`, `play_hand_card`, hand-counter `cardPlayed` raise | `cardPlayed` site | `Trigger.play_outcome` | |
| 6 | `HandChanged`, `add_to_hand`, `set_dest_unwatched` | `add_to_dest(Hand)`, `apply_dest` | kind 114 | |
| 7 | `set_ineffective`, `wasted` getter | `Discarded` raise sites | `Trigger.wasted` | |
| 8 | attachment imports, `CardLeft` cascade | `FieldCard.on`, `place_card*` | `on` is save-only | |
| 9 | `CardLeaveBefore/Left/Removed` | `unplace_at`, `take_card`, purge | kinds 114-116 | |
| 10 | — (A) | acquisition stamp + `firstTile` | | yes |

The wire break is the new kind ids and import names; `FieldCard.on` and the
stamps are save-shape only.

---

## 5. Engine wiring

### 5.1 Kernel

New module `crates/game-core/src/engine/zone.rs`:

```rust
pub struct ZoneTxn {
    pub card: String,
    pub uid: i32,       // -1 for a named card with no instance
    pub from: Zone,
    pub to: Zone,
    pub holder: i32,    // receiving seat; -1 = owner
    pub host: i32,      // -1 none
    pub cause: ZoneCause,
    pub counted: bool,  // HandChanged only
    pub wasted: bool,   // Discarded only
}

pub struct ZoneOut {
    pub ok: bool,
    pub owner: i32,             // the seat it left (-1 none)
    pub attached: Vec<i32>,     // its attachments, in order (pre snapshot)
    pub was_face_down: bool,
}
```

Two halves so the bridge can announce between them:

```rust
impl World {
    /// Validate + snapshot: source row, owner, attachments, face state.
    pub fn zone_pre(&mut self, tx: &ZoneTxn) -> ZoneOut;
    /// Mutate: unplace, flip-up, file, update `on`, `holder`, order guards.
    pub fn zone_commit(&mut self, tx: ZoneTxn) -> ZoneOut;
}
```

`zone_commit` is the only writer to a pile vector, a field vector or
`FieldCard.on`. Rules:

* `to = Attached` requires `from = Hand` (or `Nowhere` for `attach_card`) and a
  live, non-self, non-cycle-forming host; sets `on`, clears `tile`, sets the face.
* any move out of `Attached` clears `on`; a move to `Hand`/`Deck*`/`Discard`
  clears `face_down`; a `Banished` move clears both.
* `holder = -1` resolves to the owner.
* A transaction into the owner's hand while the owner is out is a no-op.
* No transaction raises events — the bridge (`wasm_rules.rs`) does, because
  events need the ruleset. A raise can pause, and pausing from a hostfn is the
  existing pattern: the bridge runs after the ctx call returns into host code,
  so no `HostRequest` round-trip is needed for it (unlike `card_move`, which is
  a guest-initiated multi-step operation). The one thing that cannot pause is
  the store write itself.

`game-rules/src/world.rs` adds `zone_pre`/`zone_commit` (default impls that
delegate to the existing helpers can come first, then every helper is deleted);
`wasm_rules.rs`'s `World` impl, `host.rs`'s fixture, `rules/kani/src/world.rs`
and the native host implement them. The eleven new imports go through the three
layers in order -- `ctx/sys.rs` externs, `ctx/sys_kani.rs` stubs,
`native_shims.rs` `op_*` + `host.rs` `func_wrap` -- exactly like the existing
zone helpers they extend.

### 5.2 Transition sites rewritten

| site | replacement |
|---|---|
| `add_to_dest` `wasm_rules.rs:1041` | `tx` (Discard = cause from caller; Hand = Tutor) |
| `take_card` `:1092` | `tx` to `Banished`, cause `Effect`; Field source also `cardRemoved` |
| `to_discard` `:1128` | `tx` to `Discard`, cause `Effect` |
| `move_now` `:671` | `tx` with all arms; `Field` = `rehome_card(self)` |
| `apply_dest` `:5847` | `tx` from the run's source zone, cause `Resolve`, `wasted` = run flag |
| hand-counter remove/dest `:6128-6134`, `:6400-6421` | `tx` Hand→Discard, cause `Play` / `Resolve`; `play_outcome` at commit |
| draw push `play.rs:4609-4612` | `tx` Deck→Hand, cause `Draw` |
| `discard()` `:4672`, hand limit `:4661` | `tx` Hand→Discard, cause `Cost` / `HandLimit` |
| `play_from_hand` remove/dest `:4787`, `:4865-4883` | `tx` Hand→dest, cause `Play` |
| `unplace_at` `ops.rs:1511` | `tx` to `Banished` unless the caller files after |
| `unplace_card` `ops.rs:1784` | `tx` |
| `remove_from_game` `play.rs:4255-4310` | per collected uid: `tx` Field/Attached→Banished, cause `Bankruptcy`/`Forfeit` |
| `shuffle_into_deck` | `tx` per card, cause `Effect` |

`set_owner` (`wasm_rules.rs:1970`) and `release_deed` keep writing `owners`;
both also call `record_acquisition` (5.9).

### 5.3 Leave dispatch

`drive_hook` is called from a dedicated `leave_dispatch` helper:

1. `cardLeaveBefore` to: the instance itself (`self_uid` = the leaving uid), the
   owner's field + lingering rows, then every other seat's field + lingering
   rows. A residual is the listener's job (`trigger::uid() == self_uid()`).
2. `zone_commit`.
3. `cardLeft` to the same set; then `cardRemoved` if `to = Banished`, with the
   removed card's own def added first (uid `-1` when it has no instance, e.g.
   a hand card like `R:[衍生] 觉悟`).

Face-down allowlist: `wasm_rules.rs:5069-5071` becomes

```rust
let face_ok = !world.card_face_down(player_id, &id)
    || lifecycle_kind(kind); // CardLeaveBefore/CardLeft/CardRemoved/
                             // CardFlipBefore/CardFlipped
```

— I1/I2/I5 are about the instance itself, so its face state must not suppress
them. All other kinds keep the existing inertness rule.

The out guard at `:5055-5057` is bypassed by `leave_dispatch` when the leaving
holder is out: by then `remove_from_game` has cleared the row, so the listener
set is built from the **snapshot taken in `zone_pre`**, not from a fresh field
scan. Tiles (`BOARD_OWNER`) leave with cause `Expire`; the board field is its own
vector and the same rules apply.

Segments (hanging `&self`/`&mut self` borrows after `world_copy`) mean the
dispatch takes its listener list before any drive, exactly like the existing
walk. A hook that prompts pauses the run between `cardLeaveBefore` and
`zone_commit`; the re-run from its snapshot repeats the pre. The pre body's
host calls (a fire spend, an ask) are ordinary replayable run steps, the same
as any hook body.

### 5.4 Attachment cascade

Inside the leave of `u`, after `cardLeaveBefore(u)` and before
`zone_commit(u)`:

```rust
for a in out.attached {           // attach order, from zone_pre
    let dest = w.attach_dest_of(a).unwrap_or(Dest::Hand);
    leave(a, dest, ZoneCause::HostLeft);   // recursive: a may itself host
}
```

`attach_dest_of` is a prop `prop::ATTACH_DEST` (i32 `Dest` code, default 0 =
`Hand`, alternative A4) so a card can name another end; `here_the_world` sets it
to `Hand` explicitly at attach time. Each cascade is a full transaction and
raises its own `cardLeft` with `host = Some(u)`, so a listener can tell a host
cascade from a normal leave. A cycle is prevented at attach: the host must not
be (transitively) attached below the new card.

### 5.5 Flip

`ops::flip_card(uid, down) -> FlipOut`:

```rust
pub enum FlipOut { Missing, Same, Changed { from_down: bool, owner: i32, host: i32 } }
```

Bridge: `Changed` → `cardFlipBefore` (dispatch to the instance's own def + field
walk) → `zone_commit`-level write → `cardFlipped`. `set_card_face_down` and
`set_self_face_down` resolve a uid first; two copies of a name resolve to the
first, as documented. Attach/detach face changes are silent (the card is not on
the field as a face-up card in between).

### 5.6 Hand changes and the opening deal

* `add_to_hand` and the `Dest::Hand` commit raise `handChanged` (cause
  `Tutor`/`Resolve`, `counted` from the import/`set_dest_unwatched`). The
  listener set is the arriving card's def (uid `-1`) plus the placed + lingering
  walk; `Drawn` remains the drawn card's own point and its walk skip is
  unchanged.
* `handChanged` fires **after** the push. `trigger::value()` is the new hand
  size, so a crossing check is one read.
* Opening deal (`play.rs:411-414`): the batch `draw(i, n, false)` becomes a
  loop of single draws so each card is known; after each push,
  `raise!(openingHandDealt, i, card, value = index)`. It fires before the
  mulligan ask at `:431`; the mulligan re-draw raises normal `handChanged` with
  cause `Mulligan` (counted `true`) and **no** `openingHandDealt`. If a
  returned copy shrinks the hand below `n`, the deal does not refill (open
  question 5).
* `remove_from_game`'s hand clear raises no `handChanged` (arrival-only event).

### 5.7 `cardPlayed`

* `play_from_hand`: one raise at the existing site (`play.rs:4896-4904`), after
  `cardAfter`, with `play_outcome` from `t.is_cancelled()`; the cancellation
  branch now only sets `wasted` and the dest.
* `resolve_rounds` hand-counter commit: one raise after the dest application
  (`wasm_rules.rs:6400-6421`), `play_outcome = Cancelled` when that link was
  negated (`:6333`), else `Resolved`, plus `cardAfter` in the same order. The
  link's `wasted = true` on cancel.
* `play_hand_card`: the nested pipeline raises it once by construction.
* Exactly one per hand play, never at declaration.

### 5.8 Wasted

`TurnCtx.play_wasted: Option<(i32, String)>` (not saved, like
`play_from_hand`/`extreme`): cleared at `play_from_hand` entry and at each
hand-counter link; `set_ineffective()` stamps `(self_uid, running card)`. The
`Discarded` raise sets `wasted` iff the stamped pair matches the discarded
card/uid, so an unrelated limit discard inside the same play is not tarred. The
effective value is `matched || outcome == Cancelled` (I7), and a cancelled play
stamps itself at the cancel branch rather than relying on the guest.
`trigger::wasted()` is the only reader.

### 5.9 Rehome and out cause

* `rehome_card(uid, seat)`: find the instance anywhere, remove the row, push it
  onto `seat`'s field, owner/user = seat; `on`/tile/props/marks/face unchanged;
  one `cardRehomed` (payload uid, from-seat, to-seat). The derived `attached`
  rows follow by uid, so a rehomed host keeps its attachments.
  `set_field_owner(uid, seat)` is exposed as the owner-stamp-only primitive.
* `bankrupt()` sets `out_cause = Bankrupt` on its `bankruptBefore` /
  `beforeOut` / `bankrupt` / `leaveAfter` payloads (`play.rs:4168-4243`);
  `forfeit()` sets `Forfeit`.
* `remove_from_game` runs one kernel leave per collected uid (cause
  `Bankruptcy`/`Forfeit`) **before** the direct `field.clear()` at
  `play.rs:4292`, which stays as a fallback for a row the kernel already moved
  (it is then empty); I1/I4 hold on the purge path too. The dispatch bypasses
  the out guard from 5.3 and uses the `zone_pre` snapshot for its listeners.
  Rows are processed in field (placement) order, so a host is always reached
  before its attachments; the host's cascade moves them and the later direct
  `zone_pre` for those uids answers "not present".

### 5.10 Acquisition order (item 10, A)

`assign_deed` (`purchase.rs:311`), the `ctx::set_owner` host
(`wasm_rules.rs:1970`) and `remove_from_game`'s deed free (`play.rs:4307-4310`)
call:

```rust
fn record_acquisition(st: &mut MatchState, t: usize, from: i32, to: i32) {
    if to >= 0 && st.acquired_at[t] == 0 {
        st.acquired_seq += 1;
        st.acquired_at[t] = st.acquired_seq;
    } // keep the stamp on sale so a re-buy does not jump the queue
    rebuild_first_tile(st, from);
    rebuild_first_tile(st, to);
}
```

Callers read the old owner before writing and pass it as `from`.

`rebuild_first_tile(st, p)` writes `state_set(p, "firstTile", min stamp among
p's owned tiles + 1, or 0)`; every call with `owner = -1` (sale, mortgage-free
loss, purge) rebuilds it for the seat that lost the tile. `acquired_at` is
sized with `owners` at match setup. The guest stays
unchanged (`ctx::slot("firstTile") - 1`, falling back to `.min()`). No import:
this item is A.

### 5.11 Kani model

`rules/kani/src/world.rs` implements the same `ZoneTxn` on its compact instance
vector (`uid, card, owner, zone, face_down, on`), so the proofs in section 9
exercise the real dispatch/kernel contract; `rules/kani/build.rs`'s `ARRAY_OPS`
gains the eleven new import names and `rules/kani/src/host.rs` gets the dispatch
arms (symbolic index, bounded vectors). New `#[cfg(kani)]` direct-call exports
follow the existing `ppcost` pattern for every edited card
(`nyamu_card`, `one_of_us`, `same_sky`, `returns`, `fate_together`, `forever`,
`auction_pulled`, `miracle`, `here_the_world`, `summer_camp`, `guerrilla`,
`soyo_colors`, `resolve`, `tomori_inner_shout`).

---

## 6. Card usage

### 6.1 nyamu (item 1)

Before: one `TurnEnd` sampler; dead while down.

After:

```rust
On::Action(Action::CardFlipBefore("", Some(is_self), pay_fire)),   // value == 1
On::Action(Action::CardFlipped("", Some(is_self), owner_draws)),    // value == 0
```

```rust
fn is_self(_: i32) -> bool { trigger::uid() == ctx::self_uid() }
```

`pay_fire` early-outs unless `trigger::value() == 1` (about to become face-down)
and asks 「失去2个火罐？」 before `fire::spend(ctx::self_owner(), 2, …)` — the
book's 拥有者, not the flipping actor; `owner_draws` early-outs unless
`trigger::value() == 0` and draws for `ctx::self_owner()`. Guards are
uid-addressed, never name lookups; `WAS_DOWN` and the `TurnEnd` entry are
deleted. (Leave hooks later read `trigger::from_zone()` the same way.)

### 6.2 one_of_us (items 2, 3)

Before: `set_owner(t, survivor)` on each mark; the card is left to the purge.

After, at the two mark sites: `ctx::set_owner(t, survivor)` (already there) and,
once, before the pair is cleared:

```rust
if out == owner && trigger::out_cause() == Some(ctx::OutCause::Bankrupt) {
    ctx::rehome_card(ctx::self_uid(), survivor);
}
```

`before_out_applies` gains `trigger::out_cause() == Some(ctx::OutCause::Bankrupt)`,
so a forfeit runs neither half.

### 6.3 same_sky (items 4, 5)

Before: `DeckAtGameStart` return after the mulligan; `discard_from_hand` +
manual body for the auto-play.

After:

```rust
On::Action(Action::OpeningHandDealt("trigger_card == card.id", None, return_at_opening)),
On::Action(Action::Drawn("trigger_card == card.id", None, auto_play)),
On::Play("", None, hand_effect),

/// [反击]: 「抽出此卡时立刻打出」 -- only the play is started here; the
/// clause below is the card's own hand effect.
fn auto_play(player_id: i32) -> Asked {
    ctx::play_hand_card(player_id, ID)?;
    Ok(())
}

fn hand_effect(player_id: i32) -> Asked {
    let n = ctx::hand_size(player_id);        // the card already left the hand
    if n >= 3 { ctx::gain(player_id, n * 600, …)?; } else { ctx::draw(player_id, 1)?; }
    Ok(())
}
```

The full play gives same_sky the counter window and the commit events `Drawn`
never had; a negation cancels `hand_effect` exactly like any other play.

`return_at_opening` is `take_card` + `add_to_dest(DeckRandom)` per copy (no
`handChanged`: the destination is the deck), unchanged otherwise.

### 6.4 returns (item 5)

Before: `CardPlayed(pre::MINE, Some(on_played_is_general), on_played)`.

After: add the outcome residual so a cancelled play does not pay:

```rust
fn resolved(_: i32) -> bool {
    ctx::trigger::play_outcome() == Some(ctx::PlayOutcome::Resolved)
}
```

and OR it into the guard. The hand-counter `cardPlayed` raise (5.7) makes a
通用 [反击] pay without a card change.

### 6.5 fate_together / crychic band / forever / auction_pulled (item 6)

* fate's return: `ctx::set_dest(ctx::Dest::Hand); ctx::set_dest_unwatched();`
  (`fate_together.rs:149-154`).
* band (3): `Action::Drew` → `Action::HandChanged(pre::MINE, Some(counted),
  on_drawn)`; `fn counted(_) { ctx::trigger::counted() }`. `on_drawn` keeps the
  `SEEN` crossing check but reads the new `trigger::value()` (hand size).
* forever: add `Action::HandChanged("card.placed && owner.hand >= 7", None,
  on_drew)`; the other five entries stay as backstops.
* auction: `Action::Drawn("", None, hook)` → `Action::HandChanged("", None,
  hook)`; the body's `trigger::card_is(ID)` + `take_card` still guard.

### 6.6 miracle / mygo band (item 7)

* miracle: at `miracle.rs:186-197`, keep the `left` read, call
  `ctx::set_ineffective()` when true, keep the log.
* band (3): `Action::Discarded(pre::MINE, Some(ineffective), on_discarded)`,
  `fn ineffective(_) { ctx::trigger::wasted() }`.

### 6.7 here_the_world (item 8)

Before: packed `HELD_*` props, no instance.

After: `HELD_LEN`/`held_key`/`store_held`/`load_held`/`clear_held` are deleted;
a single prop `held_uid` replaces them.

```rust
fn drew(player_id: i32) -> Asked {
    if ctx::prop(HELD_UID) > 0 { return Ok(()); }
    let Some(&id) = trigger::cards().first() else { return Ok(()); };
    if !ctx::take_card(player_id, id.as_str(), ctx::CardPile::Hand) { return Ok(()); }
    let held = ctx::attach_card(player_id, ctx::self_uid(), id.as_str(), true,
                                &Msg::new(key!("here_the_world_held")));
    ctx::set_prop(HELD_UID, held);
    ctx::set_crystals(3);
    Ok(())
}

fn turn_start(player_id: i32) -> Asked {
    let held = ctx::prop(HELD_UID);
    if held <= 0 { return Ok(()); }
    if ctx::add_crystals(-1, 0)? > 0 { return Ok(()); }
    ctx::detach_card(held, ctx::Dest::Hand);   // one cardLeft, cause Detach
    ctx::set_prop(HELD_UID, 0);
    let user = ctx::prop(USER_PROP) - 1;
    if user >= 0 && !ctx::player_out(user) { ctx::draw(user, 1)?; }
    ctx::set_dest(ctx::Dest::Graveyard);       // host leaves, cascade is empty
    Ok(())
}
```

An early host leave is handled by the kernel cascade alone (default `Hand`);
no extra hook body is needed beyond the `HELD_UID` read on the next turn start.
`set_prop(HELD_UID, 0)` in the host's own `CardLeft` hook is optional cleanup
(the instance is gone).

### 6.8 summer_camp / guerrilla / soyo_colors / resolve / auction (item 9)

* summer_camp: `Action::CardLeft("", Some(is_self), end)` with the shared
  `fn is_self(_: i32) -> bool { trigger::uid() == ctx::self_uid() }`; `end`
  runs the draw/slot-reset that `turn_start` currently does and sets
  `Dest::Graveyard`; the `TurnStart` entry calls the same `end`.
* guerrilla: `CardLeft` residual `uid == self_uid()` clears the `colorFor` prop.
* soyo_colors: `CardLeft` residual self → disarm `ANY_COLOR` (prop 0).
* resolve: `On::Action(Action::CardRemoved("trigger_card == card.id", None,
  payoff))`; the `On::Play` body stops paying and only sets `Dest::Banished`.
* auction: `CardLeft` residual self → `fire::add_max(player_id, -1)`.

### 6.9 tomori (item 10, A)

No card edit. `firstTile` is now maintained; the existing
`slot("firstTile") - 1` branch succeeds, and `.min()` remains the empty-slot
fallback.

---

## 7. Save / record / replay

* **SAVE_VERSION 12 → 13** (third group in the landing order). `FieldCard.on` and `MatchState.acquired_at /
  acquired_seq` are durable and saved; both are `#[serde(default)]`
  (`on = None`, stamps `0`), so a v12 save loads with no attachments and an
  empty order (tomori falls back to `.min()`).
* **RECORD_VERSION stays 3.** No input-stream shape changes; the record's
  `EngineStamp.save_version` moves with `SAVE_VERSION`.
* **In-flight state:** `play_wasted` lives on `TurnCtx` like `extreme` and is
  restored by the re-run from its snapshot, not serialized separately. Chains
  (where hand counters live) are already not saved (STACK-01).
* **Replays** ride archived engine bundles; a bundle built at ABI 104 keeps its
  old cards and old dispatch. New bundles record `ABI_VERSION = 106` in the
  stamp and refuse an older host.

---

## 8. Versioning

`ABI_VERSION` 105 → **106** (second group in landing order), entry inserted immediately below the constant
(newest first):

```
/// v106: **zone transactions, attachments, lifecycle and hand events,
///    play outcome and wasted discards, out cause**. Every zone change goes
///    through `zone_pre`/`zone_commit` and raises exactly one primary typed
///    transition (`cardLeft` + `cardRemoved`, `handChanged`, `cardFlipBefore`
///    + `cardFlipped`); eight new hook-only engine kinds 114-121 are therefore
///    a manifest wire change. `FieldCard.on` makes an attachment a placed
///    instance; `attach_card` / `detach_card` / `attached_cards` /
///    `attach_host` / `rehome_card` / `set_field_owner` / `self_owner`
///    address it. New imports `add_to_hand`, `set_dest_unwatched`,
///    `set_ineffective`, `play_hand_card` and `trigger::{uid,from_zone,
///    to_zone,cause,wasted,counted,play_outcome,out_cause,host}`. `Trigger` gains those payload
///    fields; `CardPlayed` is raised once at every hand play's commit
///    (including hand counters) with `play_outcome`, `Discarded` carries
///    `wasted`. `beforeOut` / `leaveAfter` carry `out_cause`. Face-down
///    instances now hear the five lifecycle kinds. `unplace_at`,
///    `set_face_down_at`, `set_card_face_down`, `add_to_dest(Hand)`,
///    `take_card(Field)` and the `send_to_dest` / `transfer_to_dest` fates
///    are extended in place; `Dest::Field` now files. SAVE_VERSION 12 → 13
///    (`FieldCard.on`, acquisition stamps); RECORD_VERSION unchanged.
```

SAVE_VERSION reason is stated in 7; the RECORD_VERSION decision is "unchanged,
no input-shape change".

---

## 9. Tests and Kani obligations

### 9.1 Per item (`rules/tests/tests/`)

| item | test | assertion |
|---|---|---|
| 1 | `g08_flip_down_offers_the_fire_cost` | face-up nyamu flipped down → ask; yes spends 2 fire, no does not |
| 1 | `g08_flip_up_draws_for_the_owner` | face-down nyamu flipped up → owner draws 1 |
| 1 | `g08_flip_hooks_run_only_for_the_flipped_instance` | second copy untouched, no draw |
| 1 | `g08_flip_hooks_run_while_face_down` | a post-flip-down hook on a down card runs |
| 2 | `g08_one_of_us_rehomes_the_card_and_marks` | bankruptcy → card with both marks on the survivor's field |
| 2 | `g08_rehome_preserves_props_counters_face` | prop/counter/face survive; `CardRehomed` once |
| 3 | `g08_before_out_cause_bankrupt` | `bankrupt()` → `out_cause == Bankrupt` |
| 3 | `g08_one_of_us_skips_a_forfeit` | `forfeit()` → card not rehomed, deeds untouched |
| 4 | `g08_same_sky_returns_during_the_opening_deal` | copy out of the opening hand before the mulligan prompt |
| 4 | `g08_same_sky_absent_from_the_mulligan_view` | the mulligan keeps the smaller hand |
| 5 | `g08_card_played_after_effects` | zero `cardPlayed` during the body, one after commit |
| 5 | `g08_card_played_flags_cancelled` | counteracted play → one `play_outcome = Cancelled` |
| 5 | `g08_hand_counter_raises_card_played` | resolved hand [反击] → one `Resolved` |
| 5 | `g08_returns_skips_a_cancelled_general` | 通用 play counteracted → no sticker |
| 6 | `g08_hand_changed_on_tutor` | `add_to_hand` → one event, `counted = true`, receiver seat |
| 6 | `g08_hand_changed_on_hand_counter_return` | bounced card returning → event |
| 6 | `g08_fate_return_not_counted` | `set_dest_unwatched` → `counted = false`, band (3) silent |
| 6 | `g08_forever_fires_on_a_tutor_add` | hand reaches 7 by tutor → card discards itself |
| 6 | `g08_auction_places_on_a_tutor_add` | 相信你 tutors it → placed |
| 7 | `g08_discarded_wasted_iff_crystals` | miracle with crystals → `wasted = true`; without → false |
| 7 | `g08_band3_ignores_an_effective_discard` | no crystal |
| 7 | `g08_band3_grants_on_a_cancelled_play` | counteracted owner card → crystal |
| 8 | `g08_here_the_world_attaches_face_down` | held card is an instance with `on = host`, `face_down` |
| 8 | `g08_here_the_world_completion_detaches_to_hand` | at 0 crystals the held card reaches hand, user draws |
| 8 | `g08_attachment_cascades_to_hand_on_host_leave` | host leaves early → card in owner's hand, one `HostLeft` |
| 9 | `g08_unplace_raises_card_left_once` | one before, one after, payload uid/from/to/cause |
| 9 | `g08_summer_camp_ends_on_early_leave` | draw fires, negation count reset |
| 9 | `g08_guerrilla_color_cleared_on_leave` | `colorFor` gone |
| 9 | `g08_soyo_any_color_disarmed_on_leave` | `ANY_COLOR` off |
| 9 | `g08_resolve_pays_on_removal_from_any_zone` | 1000 from a hand removal and from a field removal |
| 9 | `g08_auction_fire_cap_reverted_on_leave` | cap back by 1 |
| 9 | `g08_bankruptcy_purge_runs_leave_hooks` | a placed summer_camp in the purge hears its own leave |
| 9 | `g08_leave_hook_unplacing_another_card_is_not_double_fired` | nested leave → one event each |
| 10 | `g08_tomori_places_on_the_first_acquired_tile` | not the lowest index |
| 10 | `g08_tomori_skips_a_lost_first_tile` | earliest still-owned |

### 9.2 Kani obligations (machine-checked, exact iff)

`rules/kani/src/proofs/events.rs` (new proofs use the `host_subset!` harness):

1. `face_down_lifecycle_dispatch_iff` — a face-down instance's
   leave/flip hooks run; its other hooks do not.
2. `leave_totality` — for every `Field`/`Attached` commit, `cardLeaveBefore` and
   `cardLeft` counts are exactly 1; `Banished` adds exactly 1 `cardRemoved`.
3. `cascade_exactness` — `k` attachments ⇒ exactly `k` HostLeft commits, all
   before the host commit, all to `Hand` absent a prop; `attached_cards` empty
   after.
4. `attachment_host_invariant` — I3 over arbitrary attach/detach/rehome
   sequences, including host leaves.
5. `flip_iff` — `Changed` iff exactly one pre and one post; `Same`/`Missing`
   raise none.
6. `hand_change_symmetry` — every into-hand commit raises exactly one
   `HandChanged`; `counted` matches the transaction; opening deal raises one per
   card with `counted = false`.
7. `wasted_iff` — `Discarded.wasted` iff `set_ineffective` or `Cancelled`.
8. `out_cause_iff` — one `beforeOut` per departure; cause matches the entry.
9. `no_double_card_played` — exactly one `cardPlayed` per hand play of either
   path, at commit.
10. `first_tile_order` — `firstTile` is the min-stamped owned tile on an
    arbitrary acquire/lose sequence, and -1/0 when none.

`kbuild`/`run-kani` gates the same files; a failing proof fails the batch
(formal verification is exact, no `#[ignore]`).

---

## 10. Alternatives

**A1 — no kernel; raise at each site.** Add the kinds and raise them from every
`unplace_at`/`to_discard`/purge site by hand. Smaller diff, but the twelfth site
will forget, and the from-zone/cause pair is duplicated fourteen times. Rejected.

**A2 — ordered `MatchPlayer.deeds_acquired` + `earliest_owned_tile` import.**
More general than 5.10 (answers "nth-earliest" for future cards) but needs a
save field, a new import and Kani surface for a single current card. Kept as the
extension if another card needs order; 5.10 is the A fix.

**A3 — attachments as a `tile` sentinel** (`tile = -2 - host_uid`). No
`FieldCard.on`, no save shape change. Loses the distinction between "on the
board" and "on a card", makes every `tile` reader decode, and a nested host
chain is unreadable. Rejected.

**A4 — attachment destination as a second field** (`FieldCard.attach_dest`)
instead of `prop::ATTACH_DEST`. Typed, but only ever set through a prop today and
another save field; the prop is enough and is instance-scoped already.

**A5 — cancel the flip in `cardFlipBefore`.** Treat the pre as a counteraction
window so a card may stop the flip. The book's 「可选择失去2个火罐」 is the
owner's own choice, not a counter; `hook_only` keeps it out of the chain. Future
cards that replace a flip use `set_dest`-like replacement, out of scope.

**A6 — one `CardPlayed` + one `CardPlayCancelled` kind.** Clearer at the listen
site, but every existing listener doubles and `cards`/`card` are identical.
Outcome field chosen.

**A7 — outcome on `Drawn`-style event for hand counters only.** Does not help
same_sky (still no payload) and splits the meaning; rejected.

---

## 11. Open questions

1. **Host-leave destination default.** This design sends an attachment to its
   owner's hand (C# `Held` semantics). Is that the ruling for a hold whose host
   leaves before the hold ends, including no "user draws" payoff? (`C-Q50`
   family.)
2. **Attached card counting as 「场上的卡」.** It is an instance with
   `zone = Attached`, so field scans (`field_crystals`, auras, 「场上」 guards)
   see it. Does the book's count exclude an attached card? `OPEN-场上`.
3. **Face state on return.** `detach_card(to Hand)` clears `face_down`
   silently. Is a detach that is *not* a hold-return (a future card) allowed to
   keep it face-down in hand? `OPEN-C-SUMIMI-heretheworld-facdown`.
4. **Early host leave of the here-the-world hold** — hand (design) or discard,
   and does the user still draw? Book silent.
5. **same_sky opening hand size.** The return shrinks the hand and the deal does
   not refill. C# `FixOpeningHand` behaviour is the reference; confirm no refill
   (`OPEN-相同天空`).
6. **`CardLeft` dispatch to other seats.** The design broadcasts (any card may
   hear any leave). Does that leak hidden information for a hand-/deck-sourced
   `cardRemoved`? (`OPEN-信息`.)
7. **`handChanged` for a card added to an out player's hand.** Design: the event
   still raises but no placed listener runs (all their hooks are skipped). Ok?
8. **`cardRehomed` and the borrowed-band abilities.** A rehomed card keeps
   `user = survivor`; should a card whose `user` differs from `owner` (here the
   world on an opponent's field) survive `remove_from_game` without a rehome
   call? It does not today; item 2 only covers the owner side.
