# ABI design G11 — Information and deck inspection

**Status:** implemented (ABI **114**, SAVE **17**, kinds 132–133, ops 451–454).
**Worktree:** `.claude/worktrees/g11-design` (branch `worktree-g11-design`).
**Group:** G11, family "Information and deck inspection".
**Ground truth:** `docs/rulebook/cards.json`; every book fragment below is
verbatim from it, re-read on 2026-11-04 at the card id named beside it.
**Row ids** are the review-sheet row ids (`CARD-REVIEW-2026-10-27.md`); they are
not `cards.json` indices.

Baseline: the tree's `ABI_VERSION 113`, `SAVE_VERSION 16`,
`RECORD_VERSION 4`, engine kinds ending at 131, Kani `Op` ending at 442 (G07's
declared band 441–450, two used). G09 and G10 are designed but not landed;
under the README landing order this group lands twelfth, so its row is
`ABI_VERSION` **113 → 114**, `SAVE_VERSION` **16 → 17**, engine kinds
**132–133** (the README band, after G10's 131). Op: G09's row is still `—` and
claims 411–430 in its document, G10 claims 431–449, and G07 holds up to 450,
so this group takes **451–454** — four consecutive values after the highest
claim. §8 restates the renumbering rule.

Terminology: **look** / **inspection** = turning cards of a draw pile over
into an explicit frame. **Reveal** = publishing card identities to the table.
**Hook** = a passive listener (`On::Action`), never "reaction".

**Closed slugs and rows:** all three items are covered; two card halves are
demoted to A once the surface exists.

| # | slug | severity | rows | verdict |
|---|---|---|---|---|
| 1 | `backstage-peek-refill` (`backstage-flip-refill`) | medium | 1018 | B; the card half is **demote to A** |
| 2 | `reveal-deck-inspection` (`believe-reveal`, `guide-reveal-is-only-a-count`, `inf-possibility-deck-inspection`) | medium | 165, 170, 171, 537, 612, 615, 1092, 1400 | B; the two reveal-only bodies are **demote to A** |
| 3 | `drew-batch` (`inf-possibility-drew-per-action`) | medium | 1401, 1480 | B; the card half is **demote to A** |

No item is dropped. "Demote to A" means: after §4 exists, that half is a
card-body edit and a pinned test, with no further ABI work. The item stays B
because the possibility card needs both the look point and the batch point.

Related work this design closes or feeds:

* `docs/rulebook/BACKLOG.md` **ABI-07** (deck-inspection hook) is closed by
  items 1–2 and §4.
* The same look idiom is live in `C-PP-maya-dawn-02`
  (`rules/skills/skill-personas/src/maya.rs:83-111`) and
  `rules/skills/skill-personas/src/ako.rs:77-124`; both get the same migration
  (§6.4) and both carry the same refill defect today. Neither is a group item;
  they move in the same batch.
* Open questions that stay open after this doc: OPEN-HHW-BELIEVE-CHOOSER,
  OPEN-HHW-SHORT-PILE, OPEN-mark, OPEN-div (§11).

---

## 1. Problem, per item

### 1.1 `backstage-peek-refill` (row 1018)

Book (`HHW:出发！后台之旅！`, `cards.json:33`):

> 看牌堆顶2张牌，选择0~2张以任意顺序放回，剩余的翻入弃牌堆，每翻入一张获得1000资金

Current (`rules/cards/card-hhw/src/backstage_tour.rs`): the body reads the top
two into a local (`:56-60`), removes both from the deck with
`ctx::take_card(.., CardPile::Deck)` (`:64`) so they sit in **no pile at all**,
asks over the local, and files a flipped card with `ctx::to_discard` (`:90`).
Both writes fire the base-rule refill when the deck empties:

* `HostCtx::take_card`'s Deck branch calls `world.refill_draw_pile`
  (`crates/game-rules/src/wasm_rules.rs:1198-1225`);
* `World::to_discard` calls it after the push (`crates/game-core/src/engine/ops.rs:1226-1238`).

With a two-card deck and an empty discard, removing both empties the deck with
nothing to refill; the first flip then pushes to the discard and immediately
refills, shuffling the flipped card straight back. The deck ends non-empty and
the discard empty — the code's own TODO names it (`backstage_tour.rs:87-89`).

Expected: a flipped peeked card **stays** in the discard. Empty-deck
maintenance belongs to the next draw, not to a pile write made while a look is
in progress.

### 1.2 `reveal-deck-inspection` (rows 165, 170, 171, 537, 612, 615, 1092, 1400)

Book fragments:

> `HHW:因为我一直相信着你` (`cards.json:34`): 公开你的卡组与手牌
>
> `PP:练习生解密指南` (`cards.json:110`): 然后公开[使用者]的抽卡区并根据公开卡中的颜色数量添加一个单色/双色标记
>
> `PP:[大和麻弥]可能性为∞` (`cards.json:95`): [特]：观看卡组并观看到此卡时将此卡展示给所有玩家并将其放置在自己[场上]（次效果优先于其他后续效果，比如观看卡组后将一张牌加入手牌）。

Three separate holes, one shared root:

* **There is no reveal output.** `believe_you.rs:51-62` reads the two piles
  into locals and logs one line whose locale text carries only lengths
  (`believe_you_reveal`); `trainee_guide.rs:87-113` logs one line **per card**
  through `ctx::log` and a distinct-colour count, but a log line is not a
  table-visible zone. The only public pile in `MatchState` is the discard
  (`state.rs:647`); a hand is a count (`state.rs:645`) and a draw pile is a
  count (`state.rs:646`). The viewer's own draw pile ships sorted, separately,
  in `MatchView.draw` (`engine/mod.rs:1307-1314`, `web-glue/src/lib.rs:770`);
  no other seat ever learns a revealed card id, and a sorted list cannot show
  the order a look must preserve.
* **There is no deck-inspection point.** Nothing in the vocabulary observes a
  look. `infinite_possibility.rs:46` says so in a TODO and registers only
  `On::Play` (the [持续]（2） press) and `On::Action(Action::Drew)` (`:26`);
  because `can_swap` only accepts `is_placed()`, and nothing can place the card
  out of a deck, both [持续] clauses are unreachable in real play.
* **A look is spelled as "snapshot + take later".** Every looker
  (`backstage_tour.rs:56-64`, `believe_you.rs:91-109`,
  `maya.rs:83-111`, `ako.rs:77-124`) reads a pile into a local, prompts, then
  removes by id from the **live** pile. Any pile write in between (including a
  refill — item 1) invalidates the snapshot, and duplicate ids are re-resolved
  by name against a pile that may have changed.

Expected: every seat sees a revealed card; a look can show and place
可能性为∞ **before** the look continues.

### 1.3 `drew-batch` (rows 1401, 1480)

Book (`PP:[大和麻弥]可能性为∞`, `cards.json:95`):

> （1）[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4则为此卡添加1个[奇迹水晶]，否则移除此卡的[奇迹水晶]并为[拥有者]的Pastel✽Palettes乐队卡添加1个[奇迹水晶]。

Current: `ctx::draw(player, n)` calls the `draw` import once **per card**
(`crates/card-sdk/src/ctx.rs:885-916`), and each call ends in
`draw_cards_with_hooks`, which raises `drew` inside the per-card loop with
`value = 1` and `cards = [card]` (`crates/game-core/src/engine/play.rs:5543-5552`).
`abi.rs:2418-2421` documents exactly that. The card keys
`Action::Drew("… value > 0", …)` at `infinite_possibility.rs:26`, so a
two-card draw makes two decisions: two crystals, and at the cap a
clear-then-refill-then-grow in one action.

The clause wants **one decision per action**.

### 1.4 A further defect on the same body: double consumption

`backstage_tour.rs` removes the peeked cards with `take_card(Deck)` and files a
flip with `to_discard`, whose hand-then-push behaviour
(`ops.rs:1226-1236`) removes a same-named **hand** copy first. If the player
already holds a copy of the flipped card, the hand copy is consumed and the
peeked copy stays — one card too many in hand, one missing in the deck. A peek
needs a pile-of-origin move, not a name-addressed hand write.

### 1.5 Shared root causes

1. **A look has no state.** "The top two are being looked at" is a local
   variable, so the engine cannot know the deck is temporarily short, cannot
   show the cards, and cannot dispatch anything to them.
2. **Reveal is a log line, not an output.** A `Msg` is display text with
   single-card args (`msg.rs:101`); a pile reveal needs ids plus a client
   surface.
3. **`drew` is a per-card detail point.** There is no action-keyed after-draw
   point, and the guest draw wrapper has no action boundary the host can see.

---

## 2. Capability as an invariant

**An inspection is an explicit, owned frame: the cards it turns over leave the
deck into the frame, the frame keeps their order, nothing refills an emptied
deck while a frame is open, and every held card leaves through exactly one
release. A reveal is a table-visible output and never a pile write. One draw
instruction is one draw action, and its after-point names the batch.**

Invariants (each has a §9 test or Kani obligation):

* **I1 Held, not copied.** `inspection_begin(looker, owner, count, public)`
  moves `min(count, deck length)` cards from the deck **top** into a new frame,
  top-first. They are readable (`cards_in(Inspection)`) and takeable
  (`take_card(.., Inspection)`); a card is never in the frame and in a pile at
  once, so filing it anywhere is `take_card(Inspection)` + a dest.
* **I2 No refill while open.** While any frame has `deck_owner == p`, the four
  eager pile-write refills (`to_discard`, `add_to_dest(Graveyard)`,
  `take_card(Deck)`, `discard_from_hand`) do not fire. `draw` still refills: a
  draw during a look must find cards.
* **I3 One release per card.** A held card leaves by `take_card(Inspection)`
  followed by exactly one dest. `inspection_end` releases whatever is left, in
  reverse (top restored), or shuffled per its flag; it returns the count.
  `take_card` returning `false` for a card no longer held is the signal a body
  already released it.
* **I4 Frame exactly once.** The resumed `inspection_begin` adopts its own
  open, unfinished frame (same looker, same deck) instead of moving a second
  prefix, and continues at the first not-yet-revealed entry. Every entry
  records `revealed`.
* **I5 Order.** Frame order is deck order, top-first. `inspection_end(false)`
  restores the original prefix exactly; `inspection_end(true)` returns the
  leftovers shuffled (`DeckRandom` semantics) and is only for clauses that say
  「洗回卡组」.
* **I6 Reveal is output.** `reveal_pile` / `reveal_card` publish ids to every
  seat through an event and a public state overlay and mutate no pile. A
  public inspection reveals each card as it is revealed; a private one reveals
  nothing.
* **I7 Look point.** `inspected` is raised for every card that enters a frame,
  in reveal order, **before `inspection_begin` returns**, so a card that
  places itself out of the frame is gone before the body's next statement
  (「优先于其他后续效果」). Own-card dispatch only; no field walk.
* **I8 Batch.** `drawAfter` is raised exactly once per draw instruction, after
  the last card and before the over-hand sweep, iff at least one card was
  drawn; `value` is the batch length and `cards` the batch. `drew` stays per
  card.
* **I9 Totality.** Every import exists on wasm32, native and Kani; none asks a
  player (the hooks they raise may); reveal imports are pure with respect to
  piles.
* **I10 Off-board and empty.** An out seat's deck yields an empty frame; a
  `count` of 0 is a legal, empty look; `inspection_end` with no frame of the
  caller is a no-op returning 0.

---

## 3. Non-goals

* No revealing of the *event* deck (a separate pile with its own public tail).
* No "look at the top and choose the order" primitive beyond `inspection_end`'s
  two release modes (bodies can always take and re-add individually).
* No change to the base rule's eager refill outside a frame (alternative 2,
  §10).
* No hidden-information audit beyond the looker's own frame: what a client may
  see is the seat's `MatchView` (`docs/FAIRNESS.md`).

---

## 4. Surface

### 4.1 `CardPile` (extended) — `crates/card-sdk/src/abi.rs`

```rust
pub enum CardPile {
    Hand = 0,
    Graveyard = 1,
    /// The draw pile, **top card first**.
    Deck = 2,
    /// Cards placed on the player's field, in placement order.
    Field = 3,
    /// The cards open inspections have turned over, in reveal order
    /// (top-first). Not a pile: only `inspection_begin` fills it, and a
    /// leftover returns to the deck. Addressed by **deck owner**.
    Inspection = 4,
}
```

`from_i32` gains `4`. `add_to_dest` does **not** take an `Inspection` dest:
take the card first. `cards_in` and `take_card` accept it;
`distinct_cards_in` composes. `take_card` removes from the innermost frame
that holds the card and returns `false` when none does.

### 4.2 Imports (new; `ruleset_*`)

| import | args | returns | pause | why it is not a decision |
|---|---|---|---|---|
| `ruleset_inspection_begin` | `(looker, owner, count, public)` | held count | Sentinel; raises `inspected` per card, each may pause | moves cards + raises |
| `ruleset_inspection_end` | `(player, shuffle)` | released count | none | releases leftovers |
| `ruleset_reveal_pile` | `(player, pile, reason_ptr, reason_len)` | — | none | publishes the pile's ids |
| `ruleset_reveal_card` | `(player, card_ptr, card_len, reason_ptr, reason_len)` | — | none | publishes one id |

`count` is clamped to the deck length. `public` is `0/1`.

`reason` is the `log.*` line shown with the reveal (a postcard `Msg`, the same
encoding `place_card`'s note uses). A **public inspection's** auto-reveals use
the fixed key `log.inspection_reveal` (args: `card`, `who` = looker): they are
child events of the caller's activation header, which already carries why.

### 4.3 Output state

* **Public overlay**, derived at `public_state`, never authored by a card:

```rust
// MatchPlayer (public state)
#[serde(default, skip_serializing_if = "Option::is_none")]
pub reveal: Option<RevealView>,

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RevealView {
    /// The reveal's reason (`log.*` key plus args).
    pub reason: Msg,
    /// Publicly revealed hand ids, in hand order.
    pub hand: Vec<String>,
    /// Publicly revealed draw-pile ids, top-first. A public inspection's held
    /// cards count as revealed draw-pile ids.
    pub deck: Vec<String>,
    /// Publicly revealed discard ids (that pile is already public in
    /// `MatchPlayer.discard`; carried for the reveal's own emphasis).
    pub discard: Vec<String>,
}
```

* **Event `"reveal"`** — one per `reveal_card` (`card` set) and one per
  `reveal_pile` (`value` = count, `to` = the pile code, `msg` = reason). Add
  the case to the client switch (`webui/src/scenes/board/anim.ts:809`); it is
  a card row, not a draw.
* **Viewer-only inspection** — `MatchView.inspection:
  Option<InspectionView>` (`{ cards: Vec<String>, public: bool }`), the
  caller's innermost open frame, top-first. Filled in both view builders
  (`web-glue::frame` at `crates/web-glue/src/lib.rs:751`, the worker's `view`
  arm at `crates/rules-worker/src/lib.rs:348`) from a new
  `Match::inspection_of(member)`; `crates/server/src/api.rs:578` re-types it
  with `serde(default)`.

The overlay is built in `World::public_state`
(`crates/game-core/src/engine/world.rs:1544-1570`) from that player's
`RevealRecord` plus the held cards of that player's **public** frames. A deck
reveal made while such a frame is open already lists the held cards, so the
builder drops that copy (one occurrence per held id) before splicing the held
cards in front: each card shows once, the frame still reads as the true top.
It is cleared at the end of the outermost routine (§5.6), so it lives exactly
as long as the clause that produced it.

Client: the reveal overlay renders as a card row under the seat's name plate
(the same `InspectCard` face the graveyard hover peek uses — those cards are
already a plain id list, `webui/src/scenes/board/Players.tsx:167-235`), stays
while the seat's `reveal` is set, and the `"reveal"` event just flashes it. The
viewer's own `inspection` renders as a fan above the draw-pile badge, marked
当公开 with the `ui.json` key `ui.inspection.public`. Neither surface is a
decision point: both are read-only.

### 4.4 Hook kinds (new; `crates/game-kinds/src/lib.rs`)

| id | wire | class | filter | hook-only | payload |
|---:|---|---|---|---|---|
| 132 | `inspected` | Action | no | yes | `player_id` = deck owner, `target` = owner, `card` = the revealed id, `cards` = `[that id]`, `value` = reveal position (1-based), `by_card` = looker. Dispatched to the revealed card itself. |
| 133 | `drawAfter` | Action | no | yes | `player_id` = drawer, `value` = batch length, `cards` = the batch. Field-card dispatch. |

Both are **hook-only**: no counteraction window opens at either. `inspected`
joins `is_own_card_kind` (`crates/game-rules/src/wasm_rules.rs:12556-12560`)
and therefore the own-card branch of the dispatch
(`wasm_rules.rs:12195-12229`) — unlike `Discarded` its field walk is skipped.
`drawAfter` stays a normal field hook.

`ctx::trigger` readers: `cards()` already exists (its doc gains `inspected`
beside `drew`) and `card_is(id)` already exists. No new reader.

### 4.5 `ctx` wrappers (sugar before the ABI) — `crates/card-sdk/src/ctx.rs`

```rust
/// Turn up to `count` cards of `owner`'s draw pile into a new inspection frame
/// for `looker`. `public` reveals each card as it is turned. Returns how many
/// were held. Raises `inspected` per card **now**; pauses on a hook that
/// prompts. The body must `?` it.
#[must_use]
pub fn inspection_begin(
    looker: i32,
    owner: i32,
    count: i32,
    public: bool,
) -> Result<i32, Prompt>;

/// Close `player`'s own innermost frame (the caller as **looker**, whatever
/// seat owns the deck). Leftovers return to the deck top in their original
/// order, or shuffled when `shuffle`. Returns the count returned.
pub fn inspection_end(player: i32, shuffle: bool) -> i32;

/// 「公开…」 -- publish a pile. Hand and draw pile are the meaningful ones.
/// A draw-pile reveal includes the open frames on that seat, innermost first.
pub fn reveal_pile(player: i32, pile: CardPile, reason: &Msg);

/// 「展示给所有玩家」 -- publish one card.
pub fn reveal_card(player: i32, card: &str, reason: &Msg);
```

`inspection_cards(player)` is not a wrapper: `ctx::cards_in(player,
CardPile::Inspection)` is the list, and `ctx::distinct_cards_in` works on it.

### 4.6 Seat addressing and nesting

The `Inspection` pile is addressed by **deck owner**, never by looker:
`cards_in(owner, Inspection)` and `take_card(owner, card, Inspection)` reach
the frames on that seat's deck whoever is looking. Only `inspection_begin`'s
`looker` and `inspection_end`'s argument name the acting seat.

Frames on one deck stack (innermost last, each with its looker). `cards_in`
lists held cards innermost-frame-first and **revealed entries only**;
`take_card` removes from the innermost frame holding the card;
`inspection_end` pops the caller's own innermost frame.

### 4.7 Wire-breaking vs additive

`CardPile` gains a wire code, the import set grows, and the four refill sites
change semantics around a frame — so `ABI_VERSION` bumps (§8). No existing
kind id, signature or wire field moves. The two kinds, the reveal event, the
overlay and the view field are additive.

---

## 5. Engine wiring

### 5.1 Frame storage

`World` (saved state), with new fields added to the initializer at
`crates/game-core/src/engine/world.rs:1180-1210`:

```rust
/// Open inspection frames, innermost last. Ids come from `next_inspection_id`
/// so the commit net can name what belongs to a drive.
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub inspection_frames: Vec<InspectionFrame>,
#[serde(default)]
pub next_inspection_id: i32,
/// Public reveal records, cleared at the outermost routine's end.
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub reveals: Vec<RevealRecord>,

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectionFrame {
    pub id: i32,
    pub looker: i32,
    pub deck_owner: i32,
    pub public: bool,
    /// Top-first. `revealed` is false for entries a not-yet-finished
    /// `inspection_begin` has moved but not raised `inspected` for.
    pub cards: Vec<InspectionCard>,
    /// The begin that owns the frame has returned. A resumed begin (its first
    /// pass suspended on an `inspected` hook, so no answer was recorded)
    /// adopts the newest unfinished frame of its looker and deck instead of
    /// moving a second prefix; a later begin of the same body never adopts a
    /// finished one.
    pub finished: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectionCard {
    pub id: String,
    pub revealed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevealRecord {
    pub player: i32,
    pub reason: Msg,
    pub hand: Vec<String>,
    pub deck: Vec<String>,
    pub discard: Vec<String>,
}
```

`Hidden` keeps no field: the frame owns the cards, so "held" is one list in one
place.

Frame helpers on `World`:

```rust
fn refill_after_pile_write(&mut self, player: usize) -> bool {
    if self.inspection_frames.iter().any(|f| f.deck_owner as usize == player) {
        return false;
    }
    self.refill_draw_pile(player)
}

/// Release one frame's leftovers, in place: back to the deck top in their
/// original order, or shuffled into the deck.
fn release_frame(&mut self, index: usize, shuffle: bool) -> i32 {
    let frame = self.inspection_frames.remove(index);
    let owner = frame.deck_owner.max(0) as usize;
    let count = frame.cards.len() as i32;
    let mut deck = std::mem::take(&mut self.hidden[owner].deck);
    // `deck.push` is the top; reverse iteration puts the first card on top.
    for entry in frame.cards.into_iter().rev() {
        deck.push(entry.id);
    }
    // `DeckRandom`'s semantics: push, then shuffle the whole pile.
    if shuffle {
        self.rng.shuffle(&mut deck);
    }
    self.hidden[owner].deck = deck;
    count
}
```

### 5.2 `inspection_begin` — HostRequest and arm

New `HostRequest::InspectionBegin { looker, owner, count, public }` answered
through `request_or_pause` (`hostfns.rs` wrapper). Like `Draw`,
`Run::apply` returns `None` for it, so the drive takes the NeedHost path, runs
the `PileCheckpoint` sync, and replays past the call (§5.5).

The arm:

```
on entry:
  frame = the innermost open, unfinished frame with
          (looker, deck_owner) == (looker, owner) -> adopt it,
          and push its id to `run.adopted` (the commit net, §5.4)
  else create
  on create:
      move min(count, deck length) deck-top cards into frame.cards,
      every entry revealed = false; frame.finished = false
  loop: first entry with revealed != true
      mark revealed
      if frame.public: emit_reveal_card(owner, id, log.inspection_reveal, looker)
      raise "inspected"(player_id = owner, target = owner, card = id,
                        cards = [id], value = position, by_card = looker)
      if the raise removed the entry, continue at the same cursor
  frame.finished = true
  return frame.cards.len()
```

`emit_reveal_card(owner, id, key, looker)` is the internal helper §5.6's third
step describes (record + `"reveal"` event); the public `reveal_card` import is
the same emission with a caller-supplied reason.

The `revealed` flag makes the replayed pass idempotent: a card already raised
is not raised again, a card a hook placed is simply gone. The arm never reads
the discard and never calls `refill_draw_pile`.

Adoption only matters when the first pass **suspended**: a pass that completed
the call consumes the recorded answer at `request_or_pause`'s cursor and never
reaches the commit pass's begin, so the live world keeps exactly one move.
`finished` is what separates two looks in one body — the first is finished, and
the second therefore creates rather than adopting it. A nested drive's own
frame is likewise either finished (gone) or, in the rare same-deck-same-looker
case, innermost; adopting it only continues the reveal cursor it is at.

### 5.3 Refill suppression (I2)

`World::refill_draw_pile` (`ops.rs:1148-1166`) stays as it is. The four
pile-write sites call `refill_after_pile_write` instead:

* `World::to_discard` (`ops.rs:1236`);
* the Graveyard branch of `Run::add_to_dest` (`wasm_rules.rs:1151-1160`);
* the Deck branch of `Run::take_card` (`wasm_rules.rs:1210`);
* `Run::discard_from_hand` (`wasm_rules.rs:2251`).

`play::refill_draw_pile` (`play.rs:5470`) and therefore every draw keep
calling the direct function, so a draw during a look still refills and still
raises `reshuffled` on commit.

### 5.4 `inspection_end` and the commit net

`HostRequest::InspectionEnd { player, shuffle }` is applied **inline** (no
hooks, and `Run::apply` still gives it the pile sync before and a run-copy
re-seed after, §5.5). It finds the caller's innermost frame (`looker ==
player`), calls `release_frame(shuffle)`, logs `log.inspection_returned` with
the count when a body leaked, and returns the count. No frame is a no-op
returning 0.

The commit net runs in `commit_after` (`wasm_rules.rs:5494`) after
`cx.swap_shared(after.world)` and releases, top-down, every frame that is
**not** in the committing run's `frames_at_drive_start` **or** that the run
adopted. That covers all three leaks: a body that returns without
`inspection_end` (not in base), a frame opened inside a cancelled hook (not in
base), and a frame carried across a suspension (in base, but only for the drive
that resumed it, which is the one that commits). `frames_at_drive_start` is the
list of frame ids present when the `Run` was built; `adopted` is filled by
`inspection_begin`. A nested drive's base contains the outer drive's in-flight
frame and the nested run does not adopt it, so the nested commit leaves it
alone.

### 5.5 Replay plumbing

* `Run` gains `draw_after_log: Vec<(i32, Vec<String>)>`, `frames_at_drive_start:
  Vec<i32>` (initialized at every `Run` construction site from
  `world.inspection_frames` ids) and `adopted: Vec<i32>` (empty on
  construction, filled by `inspection_begin`).
* `Run::apply` (`wasm_rules.rs:3170`) returns `None` for
  `HostRequest::InspectionBegin`, beside `Draw`.
* The NeedHost arm's pile sync (`wasm_rules.rs:4241-4255`) includes
  `InspectionBegin` beside `Draw`; the `PileCheckpoint` records the
  pre-request run copy, so a replay prefix is reconstructed instead of being
  lost.
* For the inline `InspectionEnd` / `RevealPile` / `RevealCard` requests,
  `Run::apply` performs the same `PileCheckpoint::apply` + `cx.swap_world`
  preamble before `apply_host_request` — the guest's preceding pile writes are
  made live first, and they are already inside `cx` — and, for
  `InspectionEnd` only, copies the live world back into the run afterwards, so
  the guest's next read sees the frame gone and the released cards back in the
  deck.
* `visible_host_request` (`wasm_rules.rs:108-133`) gains all three visible
  arms (`"inspection"` for begin/end, `"reveal"` for the reveal pair).
* The three non-pausing routines do **not** build a request today: their
  `hostfns` apply straight through `c.st_mut().w()` (`Run::inspection_end` /
  `Run::reveal_pile` / `Run::reveal_card`), so a caller that ends on one of
  them never loses the guest's call. The request arms above are the pause
  fallback and are kept in step with the `Run` methods; a request is only
  built if a future caller routes one through `request_or_pause`.

### 5.6 Reveal emission and lifetime

`reveal_pile` / `reveal_card`:

1. resolve the ids (`reveal_pile`: hand order / deck top-first **plus the open
   frames on that seat, innermost first** / discard order; `reveal_card`: the
   one id);
2. write them into the player's `RevealRecord` (`reveal_pile` replaces that
   pile's vector, so two reveals show as one overlay; create the record on the
   first reveal);
3. post a `"reveal"` event — `post_log_value("reveal", player_id, reason,
   card, value)` with `card` empty and `value` = count for a pile, `card` = the
   one id and `value` = 1 for a card. `post_log_value` does not set the event's
   `to`, so the arm also stamps it on the just-posted id through a new
   `PlayCtx::set_event_to(id, to) -> bool` (`to` = the `CardPile` code; the
   helper finds the event in `w.recent` and returns `false` on a truncated
   tail, which is harmless because the overlay still carries the pile).

Lifetime: `Match::execute` gains an `outermost` flag — `start` passes
`!self.flushing_deferred`, `complete_prompt` passes the value stored on
`Pending` (`Pending` gains `#[serde(default)] outermost: bool`), and `direct`
clears on its own. The success arm clears `w.reveals` when `outermost`; an
Ask keeps them (the overlay must survive the pause the reveal usually causes).
A nested settle's commit inside the outermost routine does not clear.

### 5.7 Draw batching

`ctx::draw` becomes one instruction:

```rust
pub fn draw(player_id: i32, n: i32) -> Result<i32, Prompt> {
    crate::pure::refuse_if_filter("ctx::draw");
    #[cfg(kani)]
    { /* unchanged: the four unrolled single-card calls, assert n <= 4 */ }
    #[cfg(not(kani))]
    {
        let got = asked(unsafe { sys::draw(player_id, n.max(0)) })?;
        Ok(got)
    }
}
```

The engine loop already handles replacement, refill and pausing for `n` cards
(`draw_cards_with_hooks`), so one call is the whole action. In the `Run::draw`
fixture path (`wasm_rules.rs:1112-1146`) the `hand[before..]` slice is the
batch;
after `drawn` / `drew` entries are pushed for it, push
`(player_id, batch)` to `draw_after_log` when the batch is non-empty.

`draw_cards_with_hooks` collects the cards the instruction actually added in
a local, in draw order, and raises the point after the loop and the `log.draw`
block, before the over-hand sweep (`play.rs:5563-5580`):

```rust
if hooks && !drawn.is_empty() {
    let count = drawn.len() as i32;
    raise!(self, "drawAfter", i, value = count, cards = drawn)?;
}
```

The local is used rather than `hand[hand_before..]`: a `drewBefore` hook that
moves an **older** hand card shifts a positional slice, so the batch would
name a card that was never drawn and omit the one that was. A `drewBefore`
replacement (a card the hook added) is still recorded, since it goes through
the same `drawn.push`.

`drew` and `drawn` are untouched. A `drewBefore` replacement counts toward the
same batch (the replacement *is* the draw, `abi.rs:2264-2271`); a cancelled
draw with nothing added counts for nothing. The fixture/positive-reply path
(`Run::draw`, `wasm_rules.rs:1112`) pushes the same `draw_after_log` entry, and
the commit loop raises `drawAfter` after the last `drew` of the drive
(`wasm_rules.rs:5693-5705`). `check_silent_body` and `drive_should_flash` gain
the new log.

Kani: `ctx::draw`'s Kani branch is **unchanged** — it still issues
single-card calls (the finite unroll), so `arm_draw`
(`rules/kani/src/host.rs:2181`) keeps recording `Effect::Draw { n: 1 }` and
every existing spec's assertion holds. The model's draw path does not dispatch
hooks, so the batching difference is unobservable inside a proof.

### 5.8 Kani model

`rules/kani/src/world.rs`: `inspection_pile: [u16; MAX_CARDS]`,
`inspection_n: u8`, `inspection_owner: [bool; MAX_PLAYERS]` (which seat holds
the deck open), `inspection_public: bool` (one flag: the model keeps a single frame and does
not exercise nesting visibility), a parallel `inspection_revealed:
[bool; MAX_CARDS]` and `inspection_frame_n: u8`. `MAX_CARDS` (8) bounds the
frame.

`rules/kani/src/host.rs` arms:

```
inspection_begin => move min(count, deck_n) from the deck's front
                    (`deck_pile[0..]`, the order `arm_cards_in` reads as
                    top-first) into the inspection pile (revealed = true),
                    mark the owner, push Effect::InspectionHold; return the
                    moved count (the `inspected` raises are not modelled --
                    Kani dispatches no hooks -- so a proof asserts on the move)
inspection_end   => put the leftovers back at the deck's front in their
                    original order (or through the existing shuffle stub),
                    clear the owner flag, push Effect::InspectionReturn;
                    return the count
reveal_pile      => push Effect::Reveal { player, pile }; touch no pile
reveal_card      => as reveal with the one-card pile code
```

`arm_cards_in` gains `a1 == 4` (the revealed frame entries) and
`arm_take_card` gains `pile == 2` (deck) and `pile == 4`; today the model has
**no refill** at all (`arm_draw` and `arm_to_discard` just push), so it gains
one bounded stub — `refill_if_empty(owner)`: when the owner's deck is empty
and its discard is not, move the discard top into the deck — called by
`arm_to_discard` and by the `pile == 2` branch of `arm_take_card`, gated on
`!w.inspection_owner[owner]`. `arm_draw` stays refill-free (a draw in the
model always finds a card). Existing specs that push to the discard on an empty
deck will see the new card in the deck instead; they are the ones the gate
should keep green.

New `Op` values `451-454` go in `crates/card-sdk/src/kani_host.rs`,
`ctx/sys_kani.rs` and the `build.rs` allowlist; the `dispatch` match stays
exhaustive.

---

## 6. Card usage — before / after

All card bodies are A-class after §4; they land in the same batch.

**6.1 `backstage_tour.rs`** (item 1; card half A)

```rust
// before: cards_in(Deck) snapshot, take_card(Deck) ×2, asks, to_discard (refills)
// after
ctx::inspection_begin(player_id, player_id, 2, false)?;
let top: Vec<NameId> = ctx::cards_in(player_id, CardPile::Inspection);
// … the same asks over `top` …
// flip:
ctx::take_card(player_id, id.as_str(), CardPile::Inspection);
ctx::add_to_dest(player_id, id.as_str(), ctx::Dest::Graveyard);   // no refill
ctx::gain(player_id, 1000, …)?;
// keepers (chosen order: `under` added before `over`, so `over` is top):
ctx::take_card(player_id, id.as_str(), CardPile::Inspection);
ctx::add_to_dest(player_id, id.as_str(), ctx::Dest::DeckTop);
ctx::inspection_end(player_id, false);
```

The `cant_play` gate and the short-pile pre-shuffle branch stay. The
`to_discard` name-addressed double consumption (1.4) goes away with the
`take_card` + `add_to_dest(Graveyard)` pair.

**6.2 `believe_you.rs`** (item 2; A)

```rust
ctx::reveal_pile(player_id, CardPile::Hand, &Msg::new(key!("believe_you_reveal_hand")));
let count = ctx::deck_count(player_id);
ctx::inspection_begin(player_id, player_id, count, true)?;   // public whole-deck look
// … rival asks unchanged; the two picks read the frame and move to hand …
ctx::take_card(player_id, id, CardPile::Inspection);
ctx::add_to_dest(player_id, id, ctx::Dest::Hand);
ctx::inspection_end(player_id, false);          // leftovers back in order
// … then shuffle_into_deck as today
```

`reveal_pile(Hand)` is the 「公开你的手牌」 half; the public inspection is the
「公开你的卡组」 half and makes the possibility [特] fire **before** the picks.
The old count-only log and its hand/deck sampling TODO go.

**6.3 `trainee_guide.rs`** (item 2; A)

```rust
let count = ctx::deck_count(player_id);
ctx::inspection_begin(player_id, player_id, count, true)?;
let deck = ctx::cards_in(player_id, CardPile::Inspection);
// … the same band-colour fold over `deck` …
ctx::inspection_end(player_id, false);
// … marks, resonance, as today
```

The per-card `trainee_guide_reveal_card` log loop goes (the public overlay now
carries the ids); the distinct-colour count line stays.

**6.4 Same-family lookers** (not group items, same batch)

`maya.rs:83-111` and `ako.rs:77-124` change their `cards_in(Deck)` snapshot and
post-prompt `take_card(Deck)` into `inspection_begin` / `take_card(Inspection)`.
maya's tail becomes `inspection_end(player, true)` (「剩余观看的牌洗回卡组」);
ako releases each surplus card with `take_card(Inspection)` + `place_card`
(paid) or `take_card(Inspection)` + `add_to_dest(Graveyard)`, then
`inspection_end(false)`. Both are private looks and reveal nothing. In ako the
`trigger::set_cancelled()` replacement still names the pick as the reward.

**6.5 `infinite_possibility.rs`** (items 2 + 3; B — the capability consumer)

```rust
On::Action(Action::Inspected("", None, seen)),
On::Action(Action::DrawAfter("actor == owner && card.placed && value > 0", None, draw_after)),

fn seen(player_id: i32) -> Asked {
    let Some(id) = ctx::trigger::cards().first().copied() else { return Ok(()) };
    ctx::reveal_card(player_id, id.as_str(), &Msg::new(key!("infinite_possibility_seen")));
    if ctx::take_card(player_id, id.as_str(), CardPile::Inspection) {
        ctx::place_card(player_id, id.as_str(), &Msg::new(key!("infinite_possibility_note")));
    }
    Ok(())
}
```

`drew` renames to `draw_after` and its body is unchanged (it now runs once per
action). No `set_dest` (not a hand play). The [持续]（2） press and `can_swap`
stay. The `infinite_possibility.rs:46` TODO goes.

---

## 7. Save / record / replay

* **`SAVE_VERSION` 16 → 17** (last SAVE row in the landing order; take the
  next free one at landing). A save may be taken while a look is paused: the
  frame holds cards in no pile, so dropping it loses them. `inspection_frames`,
  `next_inspection_id` and `reveals` are saved; `#[serde(default)]` is shape
  tolerance, not a substitute for the bump — the exact-version check makes the
  bump the honest marker. `MatchPlayer.reveal` and `MatchView.inspection` are
  projections, not saved inputs.
* **`RECORD_VERSION` stays 4.** No input shape changes: prompts opened from an
  `inspected` hook are recorded as ordinary answers, the frame is rebuilt by
  the re-run, and `inspection_begin` uses no RNG. Only
  `inspection_end(shuffle)` draws, on the recorded RNG stream like every other
  shuffle. `EngineStamp` carries `save_version`, which moves with the bump.
* **Replay.** Deck order, frame order and reveal records re-derive. The client
  needs no archive change. Older engine bundles never emit the kinds or events
  and are unaffected: both are additive.

---

## 8. Versioning

`ABI_VERSION` **113 → 114** (insert immediately below the constant and above
the newest note, matching the tree's newest-first block):

```
/// v114: **deck inspection, public reveal, and the draw-action point**.
///    `CardPile::Inspection = 4` (cards an open look has turned over and not
///    yet released); `cards_in` / `take_card` accept it and the empty-deck
///    refill is suppressed on the pile writes while a look holds that seat's
///    deck open. New imports
///    `ruleset_inspection_begin(looker, owner, count, public)`,
///    `ruleset_inspection_end(player, shuffle)`,
///    `ruleset_reveal_pile(player, pile, reason)` and
///    `ruleset_reveal_card(player, card, reason)`; guest wrappers
///    `ctx::inspection_begin` / `inspection_end` / `reveal_pile` /
///    `reveal_card`. New hook-only Action kinds `inspected` (a card entered a
///    look; dispatched to the revealed card, raised before the look
///    continues) and `drawAfter` (one raise per draw instruction, `value` =
///    batch length, `cards` = the batch; `drew` stays per card). New public
///    `MatchPlayer.reveal` overlay, `MatchView.inspection` for the caller's
///    own frame, and the `"reveal"` event type. SAVE_VERSION 16 -> 17 (open
///    frames are saved); RECORD_VERSION unchanged.
```

**Renumbering rule.** Sibling drafts share a base; each reserves its row in
the README. Under the README order this group lands twelfth, after G09
(126–130) and G10 (131), so its rows are kinds 132–133 and ops 451–454. If it
lands earlier, take the next free numbers and shift later groups; if later,
the numbers are higher. Text unchanged.

Why a bump at all: the import set grows (instantiation needs the new imports),
`CardPile` gains a wire code, and `take_card` / the refill sites change
semantics around the frame. No existing signature or kind id moves.
`ManifestOn` is unchanged; the kind pin (`kind_table_pin.rs`'s `named` row
list, `kinds.rs` action list, `on_parse.rs`) and the game-kind table
(`ENGINE_KINDS`, `wire` / `from_wire`) get 132/133.

---

## 9. Tests and Kani obligations

Black-box tests go through real plays; card wasm must be rebuilt first
(`build-ruleset.mjs` — a Rust change under `rules/cards` is otherwise
invisible to `ruleset-tests`).

| item | test |
|---|---|
| 1 | `rules/tests/tests/hhw.rs::backstage_flip_stays_in_the_discard_on_a_two_card_deck` — deck exactly 2, discard empty, flip both: deck 0, both ids in the discard, money +2000; a following draw then reshuffles |
| 1 | `hhw.rs::backstage_flip_does_not_consume_a_hand_copy` — a same-named hand copy stays in hand, the peeked copy is the one filed (1.4) |
| 1 | `hhw.rs::backstage_kept_cards_return_in_the_chosen_order` — both kept, pick the top: the picked one ends on top |
| 1 | `hhw.rs::backstage_does_not_refill_mid_peek_with_a_fat_discard` — 2-card deck, 5-card discard: after the flip discard 6, deck 0 |
| 2 | `hhw.rs::believe_reveals_hand_and_deck_to_every_seat` — the state's `players[0].reveal.hand` and `.deck` hold the piles; a `reveal` event names the hand count; a third seat sees the same state |
| 2 | `pp.rs::trainee_guide_publishes_the_deck_while_it_counts_colours` — the public overlay equals the deck read before the play |
| 2 | `pp.rs::possibility_places_itself_the_moment_it_is_looked_at` — deck `[possibility, X]`, public whole-deck look: placed before the body's next statement, gone from the look list, one `reveal` event names it |
| 2 | `pp.rs::possibility_fires_on_a_private_peek_by_its_owner` — a backstage-style private begin still places it |
| 2 | `pp.rs::possibility_does_not_re_place_from_the_hand` — the copy in hand never fires `inspected` |
| 3 | `pp.rs::possibility_grows_once_per_draw_action` — 0 crystals, draw 2: exactly 1 crystal; from 3 crystals the same action clears once (0 crystals, +1 band crystal) |
| 3 | `pp.rs::draw_after_names_the_batch_and_drew_stays_per_card` — one `drawAfter` with `value == 2` and both ids; two `drew` |
| 1/2 | `pp.rs::maya_rest_shuffle_returns_the_unpicked_cards` and `ako.rs::ako_flip_release_does_not_refill` — same-family migration (maya's tests live in `pp.rs`, ako's in `ako.rs`) |
| 2 | `pp.rs::trainee_guide_publishes_the_deck_while_it_counts_colours` also asserts the overlay is gone after `drain` — it lives exactly as long as the clause |
| 2 | `live_match.rs::a_deck_reveal_during_a_public_look_lists_each_card_once` — a deck reveal inside a public frame shows the held card once (the overlay drops the duplicate copy) |
| 1 | `live_match.rs::a_leaked_frame_is_released_by_the_commit_net` — a body that returns without `inspection_end`: one `log.inspection_returned` with the count, the cards back in order, nothing published |

ABI level: `crates/game-rules/tests/kind_table_pin.rs` (132/133 in its
`named` list), `crates/game-rules/tests/live_match.rs` (`inspected` raised
top-first **before** `inspection_begin` returns; `drawAfter` exactly once,
after the last card), `rules/tests/tests/split_hook_entries.rs` (`Inspected`
dispatches to a deck card, not the field), `rules/kani/tests/on_parse.rs`
(`Inspected` / `DrawAfter` parse as Action entries),
`crates/rules-native/tests/abi_link_names.rs` (the four new link names).

**Kani obligations** (`rules/kani/src/proofs/g11_inspection.rs` / `hhw.rs` /
`pp.rs`):

* `inspection_begin_holds_at_most_count_and_preserves_deck_order` — the held
  prefix equals the old top in order; the remainder is untouched.
* `inspection_end_restores_the_deck` — begin(n) then end(false) puts the deck
  back exactly (order and all).
* `inspection_end_shuffle_keeps_the_multiset` — end(true) restores the
  multiset.
* `peek_flip_leaves_the_card_in_the_discard` — a 2-card deck, empty discard:
  begin 2, discard one, the card is in the discard and the deck 0.
* `refill_is_suppressed_iff_a_frame_holds_the_seat_open`.
* `reveal_touches_no_pile` — reveal calls keep deck / discard / hand / frame
  counts and order unchanged.
* `possibility_grows_exactly_once_per_action_iff_below_cap` — spec pairs
  (draw 2, crystals 0 → 1), (draw 2, crystals 3 → 0 crystals + one band
  crystal); a non-zero `drawAfter` is the only producer. The clause body is
  called directly (`draw_after_kani`); Kani does not dispatch hooks.
* Stub totality: every new `Op` has an arm and a `build.rs` allowlist entry.

---

## 10. Alternatives

1. **A `defer_refill` boolean on the pile writers.** Smallest change, but every
   looker threads a flag through every call, a missed call silently restores
   the bug, and the looked cards still have no identity to show. Rejected; the
   frame is the state the flag was approximating.
2. **Defer all refills to the next draw, globally.** Closest to the base rule,
   but it changes `reshuffled` timing for every card that relies on the eager
   refill mid-effect (discard-then-draw loops, the possibility swap's
   "never empty during the swap" comment) — a far wider regression surface.
   Rejected for this batch; worth a later ABI.
3. **Log-only reveal.** Cheaper, but another seat cannot see a still-open hand
   or deck once the log scrolls and the client cannot render art from a `Msg`
   arg list; the [特] self-reveal would be one line among many. Rejected.
4. **Log-only inspection with a pending list on `Match`.** A `Match` field is
   not saved with the world, so a pause + save would strand the cards.
   Rejected.
5. **`drew` gains an action id and every action-keyed card latches on
   `index == 0`.** No new kind, but the point still fires inside the action,
   the latch is per card (every future card repeats it), and the book says
   后. Rejected.
6. **A hidden `DeckOf`-style zone per player instead of a frame stack.** One
   list cannot express two nested looks or their lookers. Rejected.
7. **Frame + reveal together.** Chosen: the refill defect, the visibility
   defect and the look point share one mechanism, and the possibility card
   consumes all three.

---

## 11. Open questions

1. **Is a private peek a 「观看卡组」 for 可能性为∞?** The [特] says only
   「观看卡组」. This design fires on every `inspection_begin`, private or
   public, by owner or opponent. Confirm — otherwise the hook must test
   `frame.public`.
2. **How literal is 「优先于其他后续效果」?** The design places the card before
   the look body's next statement, therefore before any "choose N" that
   follows in the same body. If the ruling is only "before the *next effect of
   the look*", the observable difference is a later pick offering that card.
   Confirm.
3. **Does a flipped backstage card become a `discarded` hook immediately (at
   commit) or after the 1000 gain?** The design files it with
   `take_card(Inspection)` + `add_to_dest(Graveyard)`, so it never passes
   through the hand; the gain is logged between. Cards listening to
   `Discarded` see it in arrival order. Confirm.
4. **Does a `drewBefore` replacement belong to the same action's batch?**
   Design: yes — the replacement **is** the draw (`abi.rs:2479-2486`). A
   replacement from a different player's effect is still the drawer's action
   and raises on the drawer.
5. **When does a hand reveal end?** Clear at the outermost routine's end, i.e.
   after the rival's pick and the rest of the clause. If it should end at the
   pick, an explicit clear import is needed.
6. **OPEN-HHW-SHORT-PILE** (fewer than 2 cards) is untouched: the pre-shuffle
   stays a card-side decision; only the refill defect under it is closed.
7. **OPEN-HHW-BELIEVE-CHOOSER / OPEN-mark / OPEN-div** stay C-path card
   questions and are unaffected by the surface.
8. **A seat leaving mid-look.** Nothing about a leave runs a commit, so a frame
   on an out seat's deck would stay open and its cards out of the deck until
   the match ends. The design lets it stay (the cards are not lost; the seat's
   deck is simply short until something closes it). If a leave should close
   every frame it owns, add that to the leave path.

---

## 12. Landing checklist

| area | files |
|---|---|
| card-sdk | `abi.rs` (`CardPile`, `TriggerKind` / `ActionKind` consts, v114 note), `kinds.rs` (action list), `ctx.rs` (wrappers, one-call `draw`), `ctx/sys.rs` + `ctx/sys_kani.rs`, `kani_host.rs` (`Op` 451–454) |
| game-core | `state.rs` (`RevealView` / `InspectionView`, `MatchPlayer.reveal`), `engine/world.rs` (fields, helpers, `public_state`, initializer), `engine/mod.rs` (`Pending.outermost`, execute flag, `inspection_of`, reveal clear), `engine/play.rs` (`drawAfter` raise) |
| game-rules | `host.rs` (`HostRequest` variants, linker registration, `CardWorld` default methods for the four imports, `NullWorld`), `hostfns.rs`, `world.rs` (trait methods), `wasm_rules.rs` (arms, run fields, apply/commit, own-card kind, refill sites, `check_silent_body` / `drive_should_flash`), `native_shims.rs` (regenerate with `target/scratch/gen_native_shims3.py`) |
| game-kinds | `lib.rs` (rows 132–133, `wire` / `from_wire`, hook arrays) |
| rules | `card-hhw/backstage_tour.rs`, `card-hhw/believe_you.rs`, `card-pp/trainee_guide.rs`, `card-pp/infinite_possibility.rs`, `skill-personas/maya.rs`, `skill-personas/ako.rs` |
| views / client | `server/api.rs`, `rules-worker/src/lib.rs`, `web-glue/src/lib.rs`, `webui/src/core/types.ts`, `webui/src/scenes/board/anim.ts`, a reveal / inspection component, i18n `game.json` (`log.inspection_reveal`, `log.inspection_returned`) + `ui.json` + the cards' own keys |
| kani | `world.rs`, `host.rs`, `build.rs`, `proofs/hhw.rs`, `proofs/pp.rs`; `rules/kani/tests/conf_model.rs` (`ConfWorld` impls for the four imports, so conformance fills the model rather than falling to the defaults) |
| tests | §9 rows |
| docs | `README.md` rows and amendment, `CARD-REVIEW` findings closed, `docs/CARDS.md` interface table |

