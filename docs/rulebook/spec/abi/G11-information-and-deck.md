# ABI design G11 — Information and deck inspection

**Status: draft for user review. Design only — no source edits, no commit.**
Worktree `/home/monopoly/web/.claude/worktrees/card-rulebook-review`, branch
`worktree-card-rulebook-review`.

Baseline: the tree's `ABI_VERSION 104` (G06's turn-snapshot entry is already on
disk, so the constant reads 104 before G03/G06 land their own entries). Sibling
drafts G03/G06 are written against the same base; §8 fixes the landing order and
the renumbering rule. In the README order (G03, G08, G02, G04, G06, G11) this
doc's own line is **106 → 107** — see §8.

Closed slugs and their review rows (`CARD-REVIEW-2026-10-27.md` line ids). The
review rows are holistic ids; the authoritative sheet line in
`docs/rulebook/cards.json` is named beside every fragment in §2.

| # | slug | sev | rows | class | demote? |
|---|------|-----|------|-------|---------|
| 1 | `backstage-peek-refill` (merged from `backstage-flip-refill`) | medium | 1018 | B | card half A |
| 2 | `reveal-deck-inspection` (merged from `believe-reveal`, `guide-reveal-is-only-a-count`, `inf-possibility-deck-inspection`) | medium | 165, 170, 171, 537, 612, 615, 1092, 1400 | B | reveal-only card halves A |
| 3 | `drew-batch` (merged from `inf-possibility-drew-per-action`) | medium | 1401, 1480 | B | card half A |

Ground truth: `docs/rulebook/cards.json`. Every fragment below was re-read from
that file on 2026-10-28 against the card id and line named beside it.

"Demote to A" means: after this design's surface exists, that item or half is a
card-body edit and a pinned test, with no further ABI work. Where a row is
marked A it is still covered here so the card migration lands in one batch.

Related open work this design closes or feeds:

* `docs/rulebook/BACKLOG.md` **ABI-07** (deck-inspection hook) is closed by
  items 1–2 and §4.
* The same look idiom is also live in `C-PP-maya-dawn-02`
  (`rules/skills/skill-personas/src/maya_dawn.rs:60-97`) and
  `rules/skills/skill-personas/src/ako_cool.rs:52-144`; both get the same
  migration (§6) and both have the same peek-refill defect today.
* Open questions that stay open after this doc: OPEN-HHW-BELIEVE-CHOOSER,
  OPEN-HHW-SHORT-PILE, OPEN-mark, OPEN-div (§11).

---

## 2. Problem

### 2.1 Per item

**1. `backstage-peek-refill` — HHW:出发！后台之旅！, rows 1018
(`cards.json:33`)**

> 「看牌堆顶2张牌，选择0~2张以任意顺序放回，剩余的翻入弃牌堆，每翻入一张获得1000资金」

`backstage_tour.rs:74-76` takes both peeked cards out of the deck with
`ctx::take_card(.., CardPile::Deck)` so they sit in **no pile**. `take_card`'s
Deck branch refills as soon as the deck empties (`wasm_rules.rs:1105-1110` →
`ops.rs:970-986`), and the flip at `backstage_tour.rs:105` calls
`ctx::to_discard`, whose `World::to_discard` (`ops.rs:1048-1058`) pushes the
card and immediately refills again. With a two-card deck and an empty discard
the flipped card is shuffled straight back: the deck ends non-empty, the discard
ends empty. The existing probe (`zz_backstage_refill.rs`, quoted in the review)
shows exactly that: `draw=["HHW:微笑巡逻队"]`, `discard=["HHW:热气球演出"]` after
both cards were flipped.

Expected: a flipped peeked card **stays** in the discard. The base-rule refill
belongs to the next draw, not to a pile write made while a look is in progress.

**2. `reveal-deck-inspection` — HHW:因为我一直相信着你 (`cards.json:34`) /
PP:练习生解密指南 (`cards.json:110`) / PP:[大和麻弥]可能性为∞ (`cards.json:95`),
rows 165, 170, 171, 537, 612, 615, 1092, 1400**

> `HHW:因为我一直相信着你` 「公开你的卡组与手牌」
>
> `PP:练习生解密指南` 「然后公开[使用者]的抽卡区并根据公开卡中的颜色数量添加一个单色/双色标记」
>
> `PP:[大和麻弥]可能性为∞` 「[特]：观看卡组并观看到此卡时将此卡展示给所有玩家并将其放置在自己[场上]（次效果优先于其他后续效果，比如观看卡组后将一张牌加入手牌）。」

Three separate holes, one shared root:

* **There is no reveal output.** `believe_you.rs:59-75` logs a `Msg` whose
  locale text carries only lengths (`believe_you_reveal`: 「公开了手牌（{{hand}}
  张）和抽卡区（{{deck}} 张）」). `trainee_guide.rs:91-112` logs the distinct
  colour count only. The only public pile in `MatchState` is the discard
  (`state.rs:612`); a hand is a count (`state.rs:610`) and a draw pile is a
  count (`state.rs:611`), with the identities of the viewer's own pile shipped
  separately, sorted, in `MatchView.draw` (`engine/mod.rs:1186-1199`,
  `web-glue/src/lib.rs:760-772`). No seat other than the asker ever learns a
  revealed card name.
* **There is no deck-inspection point.** Nothing in the vocabulary observes a
  look. `infinite_possibility.rs:26,51-55` says so in a TODO and registers only
  `On::Play` (the [持续]（2） press) and `On::Action(Drew)`; because the only
  gate `can_swap` accepts is `is_placed()`, and nothing can place the card out
  of a deck, both [持续] clauses are unreachable in real play.
* **A look is spelled as "snapshot + take later".** Every looker
  (`backstage_tour.rs:65-76`, `believe_you.rs:107-127`, `maya_dawn.rs:60-97`,
  `ako_cool.rs:52-144`) reads `cards_in(Deck)` into a local, then removes by
  card id from the live deck after a prompt. Any pile write in between
  (including a refill, item 1) invalidates the snapshot, and the ids are
  re-resolved by name against a pile that may have changed.

Expected: every seat sees the revealed cards; a look can show and place
可能性为∞ before the look continues.

**3. `drew-batch` — PP:[大和麻弥]可能性为∞, rows 1401, 1480
(`cards.json:95`)**

> 「（1）[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4则为此卡添加1个[奇迹水晶]，否则移除此卡的[奇迹水晶]并为[拥有者]的Pastel✽Palettes乐队卡添加1个[奇迹水晶]。」

`play.rs:4623-4637` raises `drew` **inside the per-card loop**, with
`value = 1` and `cards = vec![card]`; `abi.rs:2003-2006` documents exactly
that. The card keys `Action::Drew("… value > 0", …)` at
`infinite_possibility.rs:31`, so a two-card draw makes two decisions: two
crystals, and at the cap a clear-then-refill-then-grow in one action.

The sheet keys on the **draw action** (「进行抽卡动作后」), not on a card, and
the C# `CardInfinitePossibility.Drew` was a batch point (`value` = count, the
source comment at `infinite_possibility.rs:57-59` records the difference). The
clause wants one decision per action.

### 2.2 Shared root causes

1. **A look has no state.** "The top two are being looked at" is a local
   variable, so the engine cannot know the deck is temporarily short, cannot
   show the cards, and cannot dispatch anything to them.
2. **Reveal is a log line, not an output.** A `Msg` is localized display text
   with single-card args (`msg.rs:101`); a pile reveal needs an id list plus a
   client surface.
3. **`drew` is a per-card detail point.** There is no action-keyed after-draw
   point, so an action-keyed clause must fake one with a latch.

---

## 3. Capability as an invariant

**An inspection is an explicit, owned frame: the cards it has turned over leave
the deck into the frame, the frame keeps their order, nothing refills an emptied
deck while a frame is open, and every held card leaves through exactly one
release. A reveal is a table-visible output and never a pile write. A draw
action is one event with a card batch.**

Invariants (each gets a §9 test or Kani obligation):

* **I1 Held, not copied.** `inspect_begin(owner, n)` moves `min(n, deck_len)`
  cards from the deck top into the frame, top-first. They are readable
  (`cards_in(Inspect)`) and takeable (`take_card(.., Inspect)`); a card is
  never in the frame and in a pile at once, so filing it anywhere is
  take + `add_to_dest`.
* **I2 No refill while open.** While any frame has `owner == p`, the pile
  writes that today refill eagerly (`to_discard`, `add_to_dest(Graveyard)`,
  `take_card(Deck)`, `discard_from_hand`) do **not** refill. `draw` still does:
  a draw during a look must find cards.
* **I3 One release per card.** A held card leaves by `take_card(Inspect)`
  followed by exactly one fate (`add_to_dest` / `place_card` / `to_discard`
  once it is out). `inspect_end` releases whatever is left in the frame, in order
  (top restored) or shuffled, per its `shuffle` flag; it returns the count.
  Exactly one release applies — `take_card` returning `false` for a card no
  longer held is the signal a body already released it.
* **I4 Frame exactly once.** `inspect_begin` is re-entrant after a pause: the
  frame records which entries it has already revealed, so a resume continues
  instead of re-revealing. `inspect_end` pops the innermost frame of that
  owner. A frame still open at commit is closed (leftovers returned in order)
  and logged.
* **I5 Order.** Frame order is deck order, top-first. `inspect_end(shuffle =
  false)` restores the original prefix exactly; `shuffle = true` returns the
  leftovers shuffled (`Dest::DeckRandom` semantics) and is only for clauses
  that say 「洗回卡组」.
* **I6 Reveal is output.** `reveal_card` / `reveal_pile` publish card ids to
  every seat through an event and a state overlay and mutate no pile. A public
  inspection (`public = true`) reveals each card as it is revealed; a private
  one reveals nothing.
* **I7 Look point.** `inspected` is raised for every card that enters a frame,
  in reveal order, **before `inspect_begin` returns** — so a card that places
  itself out of the frame is gone before the look body's next statement
  (「优先于其他后续效果」). Own-card dispatch only (the revealed card's entry);
  the field walk is skipped.
* **I8 Batch.** `drawAfter` is raised exactly once per draw action, after the
  last card and before the over-hand sweep, iff at least one card was drawn;
  `value` is the count and `cards` the batch. `drew` stays per card.
* **I9 Totality.** Every new import exists in wasm32, native and Kani; none
  asks a player (the raised hooks may); the reveal imports are pure with
  respect to piles.

---

## 4. Surface

### 4.1 `CardPile` (extended)

```rust
pub enum CardPile {
    Hand = 0,
    Graveyard = 1,
    Deck = 2,
    Field = 3,
    /// The cards an open inspection has turned over, in reveal order
    /// (top-first). Not a pile: nothing can be added to it except by
    /// `inspect_begin`, and a card left at `inspect_end` goes back to the
    /// deck. `cards_in` / `take_card` accept it; `distinct_cards_in` composes.
    Inspect = 4,
}
```

`CardPile::from_i32` gains `4`. `add_to_dest` does **not** take an
`Inspect` dest (the many 4/5/6 codes are `Dest`'s); a body that wants a held
card moved takes it first. `take_card(Inspect)` returns `false` for a card not
held; on success it removes the frame entry. `to_discard` keeps its current
hand-then-push behaviour unchanged, so a held card is discarded as
`take_card(Inspect)` + `add_to_dest(Graveyard)` — which also routes the
`discarded` raise and the refill guard through one place.

### 4.2 Imports (new; `ruleset_*`)

| import | args | returns | pause | why it is not a decision |
|---|---|---|---|---|
| `ruleset_inspect_begin` | `(looker, owner, n, public)` | held count | Sentinel (raises `inspected` per card; each raise may pause) | moves cards + raises |
| `ruleset_inspect_end` | `(player, shuffle)` | returned count | none | releases leftovers |
| `ruleset_reveal` | `(player, pile, msg_ptr, msg_len)` | — | none | publishes the pile's ids |
| `ruleset_reveal_card` | `(player, card_ptr, card_len, msg_ptr, msg_len)` | — | none | publishes one id |

`looker` is the acting seat (`by_card` on the raise), `owner` the deck's seat;
all shipped uses pass `looker == owner`, but the C# `H.Present` path and
possibility's own wording (it fires for an opponent's look too) keep the two.
`n` is clamped to the deck size. `public` `0/1`.

`msg` is the reason line (`log.*` key) shown with the reveal; it is display
text supplied by the caller, never interpreted by the host. A **public
inspection's** auto-reveals use the fixed key `log.inspect_reveal` — they are
child events of the caller's activation header, which already carries why.

### 4.3 Output events

* **State overlay** — public, derived, never authored by a card:

```rust
// MatchPlayer (public state; filled by `World::public_state`)
#[serde(default, skip_serializing_if = "Option::is_none")]
pub reveal: Option<RevealView>,

pub struct RevealView {
    /// The reveal's reason (`log.*` key plus its args).
    pub reason: Msg,
    /// Publicly revealed hand ids, when a hand was revealed.
    pub hand: Vec<String>,
    /// Publicly revealed draw-pile ids, top-first (a public inspection's held
    /// cards count as revealed draw-pile ids).
    pub deck: Vec<String>,
    /// Publicly revealed discard ids (that pile is already public in
    /// `MatchPlayer.discard`; carried for the reveal's own emphasis).
    pub discard: Vec<String>,
}
```

* **Event type `"reveal"`** — one per `reveal_card` (with `card` set) and one
  per `reveal_pile` (`value` = count, `to` = the pile code, `msg` = reason).
  Added to the client's event switch (`webui/src/scenes/board/anim.ts:803`) and
  the event-type documentation beside it; it animates as a card row, not as a
  draw.
* **Viewer-only inspection** — `MatchView.inspect: Option<InspectView>`
  (`{ cards: Vec<String>, public: bool }`), the caller's own open frame,
  top-first. Filled in both view builders (`web-glue::frame`,
  `rules-worker`'s `"view"` arm) from a new `Match::inspect_of(member)`; the
  server's typed `MatchView` (`api.rs:578`) re-types it with `serde(default)`.

The overlay is derived from `World` in `public_state` (`world.rs:988-1014`) and
cleared at run commit, so it lives exactly as long as the paused look that
produced it.

### 4.4 Hook kinds (new; `crates/game-kinds/src/lib.rs`)

| id | wire | class | filter | hook-only | meaning |
|---:|---|---|---|---|---|
| 132 | `inspected` | Action | no | yes | a card entered an inspection frame. Payload: `player_id` = deck owner, `target` = owner, `card` = the revealed card, `cards` = `[that card]`, `value` = reveal index, `by_card` = looker. Dispatched to the revealed card itself. |
| 133 | `drawAfter` | Action | no | yes | one draw action finished. Payload: `player_id` = drawer, `value` = cards drawn, `cards` = the batch. |

These are this group's **README band** (132/133). Every draft group has its own
consecutive band in landing order instead of all reaching for 111; both kinds
move with the band if the order changes. Retired 104 is unrelated to
`ABI_VERSION`.

`inspected` joins `is_own_card_kind` (`wasm_rules.rs:9788-9792`) and both
dispatch sites' own-card branch (`wasm_rules.rs:5027-5044`,
`wasm_rules.rs:9566-9583`); unlike `Discarded` it does **not** fall through to
the field walk. No counteraction window opens at either kind
(`is_hook_only`), so neither can be answered.

`ctx::trigger` readers: `cards()` already exists (its doc gains `inspected`
beside `drew`: the revealed card) and `card_is(id)` already exists; `cards()`
also returns the batch on `drawAfter`. No new reader is required.

### 4.5 `ctx` wrappers (guest side; sugar before the ABI)

```rust
/// Turn up to `n` cards of `owner`'s draw pile into a new inspection frame for
/// `looker`. `public` reveals each card to the table as it is turned. Returns
/// how many were held. Raises `inspected` per card *now*; pauses on any hook
/// that prompts. The body must `?` it.
#[must_use]
pub fn inspect_begin(
    looker: i32,
    owner: i32,
    n: i32,
    public: bool,
) -> Result<i32, Prompt>;

/// Close `player_id`'s own innermost frame (the caller as **looker**, whatever
/// seat owns the deck). Leftovers go back to the deck **top in their original
/// order**; `shuffle` sends them back shuffled instead (`Dest::DeckRandom`).
/// Returns the count returned.
pub fn inspect_end(player_id: i32, shuffle: bool) -> i32;

/// 「公开…」 -- publish a pile. Hand and draw pile are the meaningful ones.
pub fn reveal_pile(player_id: i32, pile: CardPile, reason: &Msg);

/// 「展示给所有玩家」 -- publish one card.
pub fn reveal_card(player_id: i32, card: &str, reason: &Msg);
```

`inspect_cards(player_id)` is not a wrapper: `ctx::cards_in(player_id,
CardPile::Inspect)` is the list, and `ctx::distinct_cards_in` works on it.

**Seat addressing.** The `Inspect` pile is addressed by **deck owner**, not by
looker: `cards_in(owner, Inspect)` and `take_card(owner, card, Inspect)` reach
the open frames on that seat's deck whatever seat is looking. Only
`inspect_begin`'s `looker` and `inspect_end`'s argument name the acting seat.
Frames on one deck stack (innermost last, `looker` recorded); `cards_in` lists
the held cards innermost-frame-first, `take_card` removes from the innermost
frame that holds the card, and `inspect_end` pops the caller's own innermost
frame. That is what lets a third-party look (the kept C# `Present` path) pick
out of another seat's deck without confusing the two seats.

---

## 5. Engine wiring

### 5.1 Frame storage

`World` (saved state):

```rust
/// Open deck-inspection frames, innermost last.
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub inspections: Vec<InspectCtx>,
/// Public reveal overlay, cleared at run commit.
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub reveals: Vec<RevealRecord>,

pub struct InspectCtx {
    pub looker: i32,
    pub deck_owner: i32,
    pub public: bool,
    /// Top-first. `revealed` is false for entries a suspended `inspect_begin`
    /// has moved but not yet raised `inspected` for.
    pub cards: Vec<InspectCard>,
    /// The pending request (`n`), so a resume continues instead of re-moving.
    pub pending: i32,
    /// The activation that opened the frame (commit-time safety net).
    pub activation: i32,
}

pub struct InspectCard { pub id: String, pub revealed: bool }

pub struct RevealRecord {
    pub player: i32,
    pub hand: Vec<String>,
    pub deck: Vec<String>,
    /// The public discard is already in `MatchState`; kept here only so a
    /// `reveal_pile(Graveyard)` has somewhere to land and the event can name it.
    pub discard: Vec<String>,
    pub reason: Msg,
}
```

`Hidden` keeps no field: the frame owns the cards, so "held" is one list in one
place. `public_state` builds each `MatchPlayer.reveal` from that player's
`RevealRecord` and appends the held cards of that player's **public** frames to
`deck`.

### 5.2 `inspect_begin` (HostRequest + arm)

New `HostRequest::InspectBegin { looker, owner, n, public }`, answered through
the normal `request_or_pause` path (`hostfns.rs` wrapper). `current` is the
enclosing drive's activation id (`run.own_activation`). The arm:

```
on entry:
  frame = this activation's open frame for `owner` by this `looker`
          (same activation && same looker && pending > 0); create one when absent
  if creating:  move min(n, deck_len) deck-top cards into frame.cards,
                set pending = n, activation = current
  loop: take the first entry with revealed == false
        mark it revealed
        if frame.public: publish reveal_card(owner, id, log.inspect_reveal)
        raise "inspected"(card = id, cards = [id], value = index,
                          by_card = looker)                              // may pause
        if the raise removed the entry, continue at the same cursor
  return frame.cards.len()
```

The `revealed` flag is what makes the replay after a pause idempotent: a card
already raised is not raised again, and a card the hook placed is simply gone
from `frame.cards`. The activation id is stamped so the commit net only closes
frames the finishing run opened.

`inspect_begin` never reads the discard and never calls `refill_draw_pile`.
When the reveal loop finishes it clears `pending`, so a second look in the same
activation creates a second frame instead of resuming the first.

### 5.3 Refill suppression (I2)

`World::refill_draw_pile` stays as it is. A new helper is used at the four
pile-write sites instead:

```rust
/// Refill after a pile write unless a look holds this seat's deck open.
fn refill_after_write(&mut self, i: usize) -> bool {
    if self.inspections.iter().any(|f| f.deck_owner as usize == i) {
        return false;
    }
    self.refill_draw_pile(i)
}
```

Call sites: `World::to_discard` (`ops.rs:1055`), `HostCtx::add_to_dest`'s
Graveyard branch (`wasm_rules.rs:1052-1054`), `HostCtx::take_card`'s Deck branch
(`wasm_rules.rs:1105-1110`), `HostCtx::discard_from_hand`
(`wasm_rules.rs:2061-2063`). `play::refill_draw_pile` and
`draw_cards_with_hooks` keep calling the direct function, so a draw during a
look still refills and still raises `reshuffled`.

`World::to_discard` is unchanged: a held card is filed with `take_card(Inspect)`
+ `add_to_dest(Graveyard)`, so nothing can be double-filed and the `discarded`
raise keeps its single producer. (The Graveyard branch of `add_to_dest` is one
of the guarded refill sites.)

### 5.4 `inspect_end`

Pops the caller's own innermost frame (`looker == player_id`; the frame's
`deck_owner` is the seat it was opened on). For each leftover
in reverse (so the original first card is restored to the top) — or, when
`shuffle`, into a `DeckRandom` push — re-insert. Logs `log.inspect_returned` when a frame
actually had leftovers (a body that leaked), with the count. Returns the
number returned. An `inspect_end` with no open frame is a no-op returning 0.

The commit safety net runs the same release for every frame whose `activation`
is the finishing run's `own_activation` (`commit_after`,
`wasm_rules.rs:4626`), so a body that returns without `inspect_end` — or a
frame opened inside a hook that was cancelled — never strands a card.

### 5.5 Reveal emission

`reveal_pile` / `reveal_card`:

1. resolve the list (`reveal_pile`: the pile's ids — hand order / deck
   top-first / discard order / frame order; `reveal_card`: the one id);
2. write it into the player's `RevealRecord` (`reveal_pile` replaces that
   pile's vector, so two reveals show as one overlay with both halves); create
   the record on the first reveal of the run;
3. log the `"reveal"` event with the reason and the count (or the card).

`reveals` is cleared in `commit_after` (`wasm_rules.rs:4626`) once the run that
produced it finishes; a paused run keeps it, which is exactly the interval the
clauses need.

### 5.6 Draw batching

In `draw_cards_with_hooks` (`play.rs:4581-4669`), accumulate every card that
actually entered the hand:

```rust
let mut batch: Vec<String> = Vec::new();
// … per-card body: batch.push(card.clone()) where `got += 1` happens …
if hooks && got > 0 {
    raise!(self, "drawAfter", i, value = got, cards = batch)?;
}
// … then the over-hand sweep
```

Placement is after the last card and **before** the over-hand discard, so the
hook sees the hand holding everything the action drew. `drew` and `drawn` are
untouched. A `drewBefore` replacement counts toward the same batch (the
replacement *is* the draw, `abi.rs:2064-2071`); a cancelled draw with nothing
added counts for nothing.

### 5.7 Kani model

`rules/kani/src/world.rs` gains `inspect_pile: [u16; MAX_CARDS]`,
`inspect_n: u8`, `inspect_owner: [bool; MAX_PLAYERS]`-style ownership and the
per-entry `revealed` bit as a parallel array; `MAX_CARDS` (8) bounds the frame.
`rules/kani/src/host.rs` arms:

```
inspect_begin  => move min(n, deck_n) from deck_pile (top end) into inspect_pile,
                  push Effect::InspectHold; return the moved count (no raise
                  modelling: the guest replay drives the same ops)
inspect_end    => move leftovers back in order (or shuffled via the existing
                  shuffle stub); push Effect::InspectReturn; return the count
reveal         => push Effect::Reveal { player, pile }; touch no pile
reveal_card    => as reveal with the one-card list
```

`arm_cards_in` gains an `a1 == 4` branch (the frame, revealed entries first —
the ship path only ever reads it after the begin's raises have run);
`arm_take_card` gains `src == 4`. `refill` in the model is gated on
`inspect_n == 0` at the same four sites. Every new op is listed in
`rules/kani/build.rs`'s import allowlist and the `Op` match stays exhaustive.

---

## 6. Card usage — before / after

**1. `backstage_tour.rs`** (B core; card half A)

```rust
// before
let top: Vec<NameId> = ctx::cards_in(player_id, CardPile::Deck)…;   // snapshot
for id in &top { ctx::take_card(player_id, id.as_str(), CardPile::Deck); }
// … asks over `top` …
ctx::to_discard(player_id, id.as_str());                            // refills
// after
ctx::inspect_begin(player_id, player_id, 2, false)?;
let top: Vec<NameId> = ctx::cards_in(player_id, CardPile::Inspect);
// … the same asks over `top` …
ctx::take_card(player_id, id.as_str(), CardPile::Inspect);
ctx::add_to_dest(player_id, id.as_str(), ctx::Dest::Graveyard);   // no refill
// keepers:
ctx::take_card(player_id, id.as_str(), CardPile::Inspect);
ctx::add_to_dest(player_id, id.as_str(), ctx::Dest::DeckTop);
ctx::inspect_end(player_id, false);             // nothing left normally
```

The `cant_play` gate and the short-pile pre-shuffle branch stay.

**2a. `believe_you.rs`** (A once §4 ships)

```rust
// before: cards_in(Deck) + counts log + later picks from the live deck
// after
let n = ctx::deck_count(player_id);
ctx::reveal_pile(player_id, CardPile::Hand, &Msg::new(key!("believe_you_reveal")));
ctx::inspect_begin(player_id, player_id, n, true)?;   // public whole-deck look
// … rival picks one from cards_in(Hand) (unchanged) …
// the two picks read cards_in(Inspect) and release with
ctx::take_card(player_id, id, CardPile::Inspect);
ctx::add_to_dest(player_id, id, ctx::Dest::Hand);
ctx::inspect_end(player_id, false);                   // leftovers back in order
// … then shuffle_into_deck as today
```

`reveal_pile(Hand)` is the 「公开你的手牌」 half; the public inspection is the
「公开你的卡组」 half and makes the possibility [特] fire before the picks.

**2b. `trainee_guide.rs`** (A once §4 ships)

```rust
// before: cards_in(Deck) + colour count + a count-only log
// after
let n = ctx::deck_count(player_id);
ctx::inspect_begin(player_id, player_id, n, true)?;
let deck = ctx::cards_in(player_id, CardPile::Inspect);
// … the same band-colour fold over `deck` …
ctx::inspect_end(player_id, false);
// … then the marks, resonance, as today
```

**2c. `infinite_possibility.rs`** (B; the capability consumer)

```rust
On::Action(Action::Inspected("", None, seen)),

fn seen(player_id: i32) -> card_sdk::Asked {
    // The hook is dispatched to the revealed copy; `cards()` is that one card.
    let Some(id) = ctx::trigger::cards().first().copied() else {
        return Ok(());
    };
    ctx::reveal_card(player_id, id.as_str(), &Msg::new(key!("infinite_possibility_seen")));
    if ctx::take_card(player_id, id.as_str(), CardPile::Inspect) {
        ctx::place_card(player_id, id.as_str(), &Msg::new(key!("infinite_possibility_note")));
    }
    Ok(())
}
```

No `set_dest` (not a hand play). The [持续]（2） press and `can_swap` stay.

**3. `infinite_possibility.rs` clause (1)** (B core; card half A)

Entry `Action::Drew` → `Action::DrawAfter`; the guard `actor == owner &&
card.placed && value > 0` stays; the body is unchanged (it now runs once per
action). The header comment about C#'s batch shape is resolved.

**4. Same-family lookers (not group items, same migration).**
`maya_dawn.rs:60-97` and `ako_cool.rs:52-144` change their `cards_in(Deck)`
snapshot plus post-prompt `take_card(Deck)` into `inspect_begin` /
`take_card(Inspect)`; maya's tail becomes `inspect_end(player, true)`
(「剩余观看的牌洗回卡组」), ako's per-card release either places
(`take_card(Inspect)` + `place_card`) or discards (`take_card(Inspect)` +
`add_to_dest(Graveyard)`), then `inspect_end(false)` for anything left. Both are private looks and reveal
nothing.

---

## 7. Save / record / replay

* **SAVE_VERSION 17 → 18** (last SAVE bump in the landing order; take the next
  free one). A save may be taken while a look is paused: the open frame holds
  cards that are in no pile, so dropping it loses them. `inspections` and
  `reveals` are saved (`#[serde(default)]` for shape tolerance;
  `#[serde(default)]` is *not* the reason no bump would be needed -- an at-rest
  save with no frame would default fine, but a mid-look save restored into a
  build without the fields would silently delete the held cards, and the
  exact-version check makes the bump the honest marker).
  `MatchPlayer.reveal` itself is a `public_state` projection (below), not saved
  state; the saved inputs are the frames and the reveal records.
* **RECORD_VERSION stays 3.** No input shape changes: prompts opened from an
  `inspected` hook are recorded as ordinary answers, the frame is rebuilt by
  the re-run, and `inspect_begin` uses no RNG (only `inspect_end(shuffle)`
  does, and its draw is on the recorded RNG stream like every other shuffle).
  `EngineStamp` carries `save_version`, which moves with `SAVE_VERSION`.
* **Replay.** Deck order, frame order and the reveal records are re-derived.
  The client needs no archive change: `revealed` overlays are read from the
  state each frame, and older engine bundles (which never emit `inspected` or
  the reveal events) are unaffected because both kinds and both event shapes
  are additive.

---

## 8. Versioning

`ABI_VERSION` **114 → 115** (entry inserted immediately below the `ABI_VERSION`
constant and above the newest note, following the tree's existing newest-first
block):

```
/// v115: **deck inspection, public reveal, and the draw-action point**.
///    `CardPile::Inspect = 4` (the cards a look has turned over and not yet
///    released); `cards_in` / `take_card` accept it and the empty-deck refill
///    is suppressed on the pile writes while a look holds that seat's deck
///    open. New imports
///    `ruleset_inspect_begin(looker, owner, n, public)`,
///    `ruleset_inspect_end(player, shuffle)`, `ruleset_reveal(player, pile,
///    msg)` and `ruleset_reveal_card(player, card, msg)`; guest wrappers
///    `ctx::inspect_begin` / `inspect_end` / `reveal_pile` / `reveal_card`.
///    New hook-only Action kinds `inspected` (a card entered a look,
///    dispatched to the revealed card itself, raised before the look
///    continues) and `drawAfter` (one raise per draw action, `value` =
///    cards drawn, `cards` = the batch; `drew` stays per card). New public
///    `MatchPlayer.reveal` overlay, `MatchView.inspect` for the caller's own
///    frame, and the `"reveal"` event type. SAVE_VERSION 17 -> 18 (open
///    frames are saved); RECORD_VERSION unchanged.
```

**Renumbering rule.** Several sibling drafts are written against the same base
and each proposes its own entry. Under the README order this group lands eleventh, so the
constant reads 114 before it and this doc's paragraph is written as 114 → 115. If G11 lands earlier, take the next free
number and renumber the groups after it; if it lands last, the number is
higher still. The constant and each doc's line move, text unchanged.

Why a bump at all: the import set grows (instantiation needs the new imports —
the wasm cannot load on an older host), `CardPile` gains a wire code, and
`take_card` / the refill sites change semantics around the frame. No existing
signature or kind id moves. `ManifestOn` is unchanged; the kind pins
(`kind_table_pin.rs`, `kinds.rs` `KINDS`, `on_parse.rs`) and the CEL listener
list get 132/133 (G08 through G09 take 114-130 ahead of this group, G10 131).

---

## 9. Tests and Kani obligations

Black-box tests go through real plays; card wasm must be rebuilt first
(`build-ruleset.mjs` — a Rust change under `rules/cards` is otherwise invisible
to `ruleset-tests`).

| item | test |
|---|---|
| 1 | `rules/tests/tests/hhw.rs::backstage_flip_stays_in_the_discard_on_a_two_card_deck` — deck exactly 2, discard empty, flip both: deck 0, discard both ids, money +2000; a following draw then reshuffles |
| 1 | `hhw.rs::backstage_kept_cards_return_in_the_chosen_order` — both kept, pick the top: deck order is the picked one first |
| 1 | `hhw.rs::backstage_does_not_refill_mid_peek_with_a_fat_discard` — 2-card deck, 5-card discard: after the flip the discard is 6, the deck 0 |
| 2 | `hhw.rs::believe_reveals_hand_and_deck_to_every_seat` — after the play the state's `players[0].reveal.hand` and `.deck` hold the piles; a `reveal` event with the hand count was logged; a third seat sees the same state |
| 2 | `pp.rs::trainee_guide_publishes_the_deck_while_it_counts_colours` — the public overlay equals the deck read before the play |
| 2 | `pp.rs::possibility_places_itself_the_moment_it_is_looked_at` — deck [possibility, X], public whole-deck look: possibility is placed before the body's next statement, the remaining look list no longer holds it, one `reveal` event names it |
| 2 | `pp.rs::possibility_fires_on_a_private_peek_by_its_owner` — backstage-style private begin still places it |
| 2 | `pp.rs::possibility_does_not_re_place_from_the_hand` — the copy in hand never fires `inspected` |
| 3 | `pp.rs::possibility_grows_once_per_draw_action` — 0 crystals, draw 2: exactly 1 crystal; then a second 2-card action from 3 crystals clears once (0 crystals, +1 band crystal) |
| 3 | `pp.rs::draw_after_names_the_batch_and_drew_stays_per_card` — an event-log probe: one `drawAfter` with `value == 2` and both ids, two `drew` |
| 1 | `rules/tests/tests/skill_personas.rs::maya_rest_shuffle_returns_the_unpicked_cards` and `pp.rs::ako_flip_release_does_not_refill` — the same-family migration |

ABI-level: `crates/game-rules/tests/kind_table_pin.rs` (132/133),
`crates/game-rules/tests/live_match.rs` (`inspected` is raised top-first *before*
`inspect_begin` returns; `drawAfter` exactly once, after the last card),
`rules/tests/tests/split_hook_entries.rs` (the `Inspected` entry is dispatched
to a deck card, not to the field), `rules/kani/tests/on_parse.rs` (`Inspected`
parses as an Action entry).

**Kani obligations** (`rules/kani/src/proofs/hhw.rs` / `pp.rs`):

* `inspect_begin_holds_at_most_n_and_preserves_deck_order` — the held prefix
  equals the old deck top in order; the remainder is untouched.
* `inspect_end_is_a_permutation` — `begin(n)` then `end(false)` restores the
  deck multiset and order exactly; `end(true)` restores the multiset.
* `peek_flip_leaves_the_card_in_the_discard` — model a 2-card deck, empty
  discard: begin 2, discard one, the card is in the discard and the deck is 0.
* `refill_is_suppressed_iff_a_frame_is_open_for_the_seat`.
* `possibility_grows_exactly_once_per_action_iff_below_cap` — spec pairs
  (draw 2, crystals 0 → 1), (draw 2, crystals 3 → 0 crystals + one band
  crystal), and a non-zero `drawAfter` is the only producer.
* `reveal_touches_no_pile` — `reveal` / `reveal_card` keep `deck_n`,
  `discard_n`, `hand_n` and the frame counts unchanged.
* Stub totality: every new `Op` has an arm (the `kani_host` match is
  exhaustive) and a `build.rs` allowlist entry.

---

## 10. Alternatives

1. **A `defer_refill` boolean on `take_card` / `to_discard` / `add_to_dest`.**
   Smallest possible change, but every looker must thread the flag through
   every call, a missed call silently restores the bug, and the looked cards
   still have no identity to show. Rejected; the frame is the state the flag
   was approximating.
2. **Defer all refills to the next draw, globally.** Closest to the base rule
   and removes the special case, but it changes `reshuffled` timing for every
   card that relies on the eager refill mid-effect (discard-then-draw loops,
   the possibility swap's "never empty during the swap" comment), and it is a
   far wider regression surface than one group. Rejected for this batch;
   worth a later ABI if the eager sites keep needing exceptions.
3. **Log-only reveal (no overlay, no `MatchView.inspect`).** Cheaper, but
   another seat cannot see a still-open revealed hand/deck once the log
   scrolls, and the client cannot render art from a `Msg` arg list; the [特]
   self-reveal would also be one line among many. The overlay is the minimum
   for 「公开」 to mean "seen by the table". Rejected.
4. **`drew` gains an `action` id and every action-keyed card latches on
   `index == 0`.** No new kind, but the point still fires *inside* the action,
   the latch is per card (every future card repeats it), and the book says
   「后」. Rejected.
5. **Make `drew` the batch point (C# shape) and move the per-card consumers to
   `drawn`.** Closest to C#, but it silently changes 梦在前方, Here the world,
   若宫伊芙, the CRYCHIC band and 如果能一直持续下去… in one batch, and `drawn`
   is the drawn card's own hook, not a field point. Keeping `drew` and adding
   `drawAfter` is additive. Rejected.
6. **Put the held cards in `Hidden` instead of a frame stack.** Fine for
   `looker == owner`, but a look of another seat's deck (the [特] wording and
   the C# `Present` path allow it) needs the owner recorded, and nested looks
   need a stack. The frame carries both. Chosen.

---

## 11. Open questions

1. **Is a private peek an 「观看卡组」 for 可能性为∞?** The [特] says only
   「观看卡组」. This design fires on every `inspect_begin`, private or public,
   by owner or opponent. Confirm — otherwise the hook must test `frame.public`.
2. **Does the [特] fire when the owner looks, not only a third party?**
   (Same wording; design: yes.) The C# path is a `Present`/`Fx` hook with no
   actor restriction, but this is a reading.
3. **How literal is 「优先于其他后续效果」?** The design places the card
   before the look body's next statement and therefore before any "choose N
   from the deck" that follows in the same body. If the ruling is only "before
   the *next effect of the look*", the observable difference is a later pick
   offering that card. Confirm.
4. **Does a flipped backstage card become a `discarded` trigger immediately
   (at commit) or after the 1000 gain?** The design drops it with
   `take_card(Inspect)` + `add_to_dest(Graveyard)`, so it never passes through
   the hand and cannot consume a same-named hand copy; the gain is logged per
   card in between. Cards listening to `Discarded` (RAS 游击演出, MyGO (3)) see
   it in arrival order. Confirm this is intended for 「翻入弃牌堆」.
5. **Is the opening deal a 「抽卡动作」 for `drawAfter`?** Design: no (the
   opening draws with `hooks = false`, as today, and C# raises nothing there).
   If it should, the possibility's `card.placed` guard cannot hold anyway
   (it is placed by its [特], after the deal).
6. **Does a `drewBefore` replacement belong to the same action's batch?**
   Design: yes — the replacement **is** the draw (`abi.rs:2064-2071`). A
   replacement from a *different* player's effect (a redirect) still belongs to
   the drawer's action and raises on the drawer.
7. **When does a hand reveal end?** The overlay clears at run commit, i.e.
   after the rival's pick and the rest of the clause. If the reveal should end
   at the pick, the card's `inspect_end`/reveal bookkeeping needs an explicit
   clear import.
8. **`OPEN-HHW-SHORT-PILE`** (fewer than 2 cards) is untouched by this batch:
   the pre-shuffle stays a card-side decision; only the refill defect under it
   is closed.
