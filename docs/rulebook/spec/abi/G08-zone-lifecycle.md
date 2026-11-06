# G08 — Zone, attachment and creator lifecycle

Status: **draft for review, design only** (2026-11-03 revision of the
2026-10-28 draft). No source is edited by this document.
Worktree `/home/monopoly/web/.claude/worktrees/g08-design-review`, branch
`worktree-g08-design-review`. Fits **on top of ABI v112** (G01 through G06 have
landed; G07 is the next group and lands first). Group: **G08**. Family:
the one writer for every card move between zones, the facts a move carries
(origin, destination, cause, waste, count, play outcome, departure cause), the
instance edges that make a card die with its host and its creator, and the
per-player deed-acquisition order.

Names are spelled out (G03 R25's rule, G06's header): a **zone** is one of the
places a card can be; a **zone transition** is any move between two zones,
applied as one **zone transaction**; a **leave** is a transition out of
`Field` or `Attached`; an **attachment** is a card physically on another card;
a **creator edge** is the record that one instance made another; a
**departure** is a card's exit from play, and a player's departure from the
game is an **out**; a **hook** is a passive listener; the engine-side carrier
of one raise is a **trigger**, its module-side mirror a **link**.

The instance field is the repo's existing `uid` (`FieldCard.uid`,
`Trigger.instance`); prose says **instance** and code says `uid` where the
field already does.

Names are also split by crate (`bandori-std/src/lib.rs:3-9`): `card-sdk`
holds only generalized runtime machinery, and **this game's vocabulary lives
in `bandori_std`**. The zone enum, the cause enums and the payload are generic
mechanism and stay in `game-kinds` / `card-sdk`; the prop keys the engine
reads, the `zones` sugar and the card-side predicates are `bandori_std::zones`.

Ground truth is `docs/rulebook/cards.json`; every quoted fragment below was
re-read there and is identified by its card id beside it. The **row ids are
the triage sheet rows** — the same ids `BACKLOG.md` section B uses. Reading
order: G01 (instance identity and the per-instance store), G06 §6.1 (the kind
contract), `docs/rulebook/spec/cards/mujica.md:364-392` (C-MUJICA-喵梦-01..03),
`docs/rulebook/spec/cards/ag.md:118-190` (ONE OF US), `docs/rulebook/spec/
cards/sumimi.md` (Here the world), and `docs/rulebook/BACKLOG.md:1930-1945`
(C-Q49, Here the world's own leave timing; C-Q50, ONE OF US's deed
placement).

## 0. Review record

This revision came out of a full review of the 2026-10-28 draft, followed by a
naming pass against `bandori_std`'s crate contract. Every cited site was
re-read against master, every claim about the engine was re-derived from the
code, and the band arithmetic was re-run from the groups that landed before
this one. What the draft got wrong:

| # | Draft defect | Correction |
|---|---|---|
| R1 | The header named worktree `card-rulebook-review` and no base version. Master is ABI 112 / SAVE 16 / RECORD 4 / the last Kani `Op` 432, and G07 has not landed yet. | Header and §11 carry the real base and the landing band (§11 records the final landing: ABI 118, SAVE 20, kinds 113–119 + 140, ops 467–486). |
| R2 | Nearly every engine citation was stale by 300–1100 lines (`wasm_rules.rs:5067-5071` for the face-down guard, `ops.rs:1285` for `set_field_owner`, `ctx.rs:1838` for `ctx::raise`, `ctx.rs:1989` for `Dest`, `rules.rs:291` for `Trigger::new`); `ops.rs` has since moved from `game-rules` to `game-core/src/engine/`. | Every site is re-cited against master: the face-down guard `wasm_rules.rs:5950-5952` and `:6097-6099`, the owner write `wasm_rules.rs:2101`, `Dest` `ctx.rs:2416`, `Trigger::new` `rules.rs:507`. |
| R3 | The payload was addressed by `Trigger.uid`, "the identity discriminator", with `uid = -1` on instance-less events. | `Trigger.uid` is already the **raise identity** (`ctx.rs:3666`, `rules.rs:439`); the subject instance is `Trigger.instance` (`ctx.rs:3674`, `rules.rs:444`), which G01 added and the dispatch's subject pass already drives (`wasm_rules.rs:5975-6005`). §6 reuses it and adds no field. |
| R4 | §"No transaction raises events" claimed the bridge can raise after the ctx call returns "so no `HostRequest` round-trip is needed". | A before-window must run **before** the write, may prompt (喵梦's clause 1 is a choice), and may rewrite the destination. A plain hostfn cannot pause; the writer is reached through `request_or_pause` with a `HostRequest` (§7.4). |
| R5 | `I1`-`I9` enumerated the two kind sets per transaction but had no order invariant, so a leave hook could not know whether the attachment cascade had already run. | §4 fixes the cascade order (attachments, then created cards, then the leaver) and states it as I5. |
| R6 | The draft introduced `cardLeaveBefore`, `cardLeft`, `cardRemoved`, `cardFlipBefore`, `cardFlipped`, `cardRehomed` and six kinds besides; `cardRemoved` and `cardRehomed` duplicate `to = Banished` and `cause = Rehome`. | Eight kinds only, seven of them in the band (§6.2); `Banished` and `Rehome` are payload values, so a listener sees them without a second kind table, and `zoneRoundTripped` is dropped for want of a reader. |
| R7 | The enum was `ZoneCause` and §"MoveDest" was `MoveDest`; the draft also wrote `ZoneTxn`, `tx` and `entry` for three different things. | `Zone`, `ZoneTransition` (the transaction), `ZoneChangeCause` (§5.1); code is `transition`, prose is "the transaction". |
| R8 | §"`add_to_hand(..., counted)`" added a second hand import although `add_to_dest` already files every pile. | `sys::add_to_dest` gains the `counted` argument (`ctx::add_to_dest` keeps its sugar name); no `add_to_hand` (§5.5). |
| R9 | `set_field_owner` was named after a *tile* concept while it takes an instance. | `set_instance_owner` / `ctx::set_instance_owner` (§5.4); the tile writer keeps its own name. |
| R10 | `destroy_instance_units` was described as "the removal cleanup"; the instance edges, the attachment row and the acquisition sequence need their own cleanup points. | §7.6 extends `destroy_instance_units` for the edges and adds the detach path; the acquisition sequence is a ledger, not instance state (§7.7). |
| R11 | `record_acquisition` was invented as an import; the engine can record it at its own ownership write. | The engine records inside `assign_deed` / `set_owner` / the bankruptcy release; the surface keeps only the `earliest_owned_tile` read (§5.4, §7.7). |
| R12 | The board-owned attachment cascade said only "detaches to `Nowhere`"; a board rule's attachment must not vanish. | A board-owned host's attachments follow the host's destination, which for a board rule is `Graveyard` (the board rule row is re-placed, not destroyed) (§7.5). |
| R13 | `self_face_down` was treated as unaddressed; it is already the running instance's (`wasm_rules.rs:1359`). | The flip surface keeps `self_face_down` and only adds the cause-carrying addressed form (§5.5). |
| R14 | Item 10 was "demoted to A" with "no ABI surface", yet its pitfall (the `firstTile` slot is stale after a sale) needs an engine rebuild hook and a read for any card. | Kept demoted **as an item**; the engine-side ledger and rebuild are part of the group, and one read import lands (§5.4, §7.7). |
| R15 | §"face-down cards" said the leave window skips face-down cards "as the field walk does". | The **subject pass** does not apply the face-down guard (`wasm_rules.rs:5975`), which is exactly why 喵梦's own hook must run there (§7.3). The field walk keeps the guard. |
| R16 | The draft had no necessity section, no alternatives and no clause match (G06/G07 added them after review). | §12, §13 and §15 exist. |
| R17 | §"Rules" made `to = Attached` legal only from `Hand`/`Nowhere`, but Here the world holds a card drawn from hand and 灯's card attaches from hand too; nothing could re-attach a detached card. | From is open; the writer takes the row wherever it is and only rejects a self-host and a cycle (§7.4). |
| R18 | The draft put `uid = -1` on the hand event and called the counter a `counted` bit "on `handChanged`" without saying who sets it. | `counted` is set by the **draw routine** only, so a tutor / bounce / skill add is `counted = 0` by construction (§7.2). |
| R19 | `wasted` was described as a log string; the draft left `miracle`'s `miracle_wasted` line in place as "sufficient". | `set_ineffective_at` is a saved instance bit; the log line stays as display but the hook reads the bit (§7.2, §8.4). |
| R20 | The draft's Kani note did not say the model world lacks `on` and `created_by`, so a proof would compile against a field-less model. | §10.3 adds `on` and `created_by` to `rules/kani/src/world.rs`'s `FieldCard` and the arms. |
| R21 | §"eleven new imports go through the three layers … exactly like the existing zone helpers they extend" undercounted: the helper set is 14 new exports plus four re-signed ones. | §5.5 lists all of them and marks which are re-signed; §11 counts 14 new ops. |
| R22 | The card usage for Here the world kept the prop-store hold ("we only take it out and remember the id"). | A held card is a real attachment — face-down on the host, with the crystals on the *held* instance — per 「将那张卡背面朝上放置于此卡上」 (§8.3). |
| R23 | The card usage for `black_suits` (`TODO(G08) created_by`) said the interim `unplace_at` "is not a replacement" but did not say what replaces it. | The borrowed instance is a created child of the marker owner with `created_by`; its creator leaving drops it (§8.5). |
| R24 | The draft had no `RECORD_VERSION`/replay section claim beyond "unchanged", although the payload is in the event log. | §9 states why RECORD stays 4 (the payload is derived at re-raise, not stored) and what the replay re-derives. |

## 1. Items

| # | slug(s) | sev | rows | disposition |
|---|---------|-----|------|-------------|
| 1 | `nyamu-face-flip-trigger` | high | 347, 348, 1043, 1272 | B: two kinds, one re-signed import, a guest getter |
| 2 | `one-of-us-rehome` — merged `one-of-us-cause`, `created-by-lifetime` (from G01) | medium | 1107, 1124, 283, 487, 488, 490, 1029, 1555 | B: the rehome request, the instance-owner surface, the creator edge |
| 3 | `same-sky-mulligan-order` | low | 32 | B: `openingHandDealt` kind and a counted draw |
| 4 | `cardplayed-payload` | medium | 34, 677, 680 | B: `play_outcome` on the play outcome |
| 5 | `hand-changed-event` | high | 57, 58, 197, 642, 1688 | B: `handChanged` kind and the `counted` bit |
| 6 | `wasted-flag` | medium | 510, 522, 523, 1396, 1701 | B: `set_ineffective_at` and `wasted` on `discarded` |
| 7 | `card-on-card` | high | 1075 | B: attachment surface and the host cascade |
| 8 | `here-the-world-leave-hook` | high | 639, 645, 867, 875, 1074, 1252, 1474, 1488, 1514, 1515, 1689, 1695 | B: the leave kinds and `prop::LEAVE_DEST` |
| 9 | `tomori-earliest-owned` | medium | 188, 994 | **demote to A** on the card side: the engine keeps `firstTile` current; one read import stays in the band |

Items 2, 7 and 8 are one mechanism: a card can be *on* another card, the host
can leave, and a card that leaves must be able to react first. Items 3, 5 and
6 are all "the transition carries a fact the listener needs"; they share the
`ZoneTransition` payload and the writer. Item 1 is the same writer's
in-place (no-zone) sibling. Item 4 rides the play outcome the writer already
knows. Item 9 is engine bookkeeping: the card itself needs no new behaviour
once the slot is right, but the slot has to be right from game start, which
only the engine can do.

Group coverage against `BACKLOG.md` section B: all eleven original slugs are
covered, with `one-of-us-cause` and `created-by-lifetime` merged into item 2
because the departure cause and the creator cascade are the same edge set.
`one-of-us-cause` is split across item 2 (player departure cause on
`beforeOut`) and item 8 (`HostLeft`/`Bankruptcy`/`Forfeit` zone causes).

## 2. Problem, per item

Each entry names the verbatim fragment, the current site, what the book
requires and the root cause. Root causes are **C1-C7** defined in §3.

### 2.1 `nyamu-face-flip-trigger` — Mujica:（喵梦）, rows 347, 348, 1043, 1272

Book:

> 每当此卡将从正面翻至背面时可选择失去2个火罐。
> 每当此卡将从背面翻至正面时此卡拥有者抽一张卡。

Current site: `rules/cards/card-mujica/src/nyamu_card.rs:26-31` registers one
`TurnEnd` hook, and `:57-73` samples `ctx::self_face_down()` against a
`nyamu.wasDown` slot:

```rust
// TODO(ABI) face-flip hook: no per-card "about to flip" / "flipped" trigger
// exists, so a flip is only sampled from the face-down flag at the owner's
// TurnEnd -- and a face-down placed card's `On::Action` hook is skipped by the
// dispatch, so the sample never runs while the card is down (`WAS_DOWN` stays
// 0) and the draw below never fires in the ordinary path.
```

Two separate holes. No flip event exists (C2); and the only sampler is
skipped while the card is face-down, because the field walk's guard is
`!world.is_face_down_at(uid)` (`wasm_rules.rs:5950-5952`, `:6097-6099`), so
the sample and the flip it waits for never meet (C7). The book's clauses are
「将从…翻至」 — strictly **before** the face changes — and clause 1 is a
player choice, so the window must prompt. `ctx::self_face_down` already
addresses the running instance (`wasm_rules.rs:1359`); nothing reads a cause.

### 2.2 `one-of-us-rehome` — AG:ONE OF US, rows 1107 (with 1124, 283, 487, 488, 490, 1029, 1555)

Book:

> 当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有此地契

Current site: `rules/cards/card-ag/src/one_of_us.rs:294-331`. The whole
transfer is one tile-owner rewrite, `ctx::set_owner(t, survivor)` at `:319`,
with a `TODO(规则书)` at `:312` recording that the book parks the deeds **on
the card** and the card itself moves:

```rust
// TODO(规则书)…: the book parks 「两张」 designated deeds on the card
// (「放置在该卡上」), moves the card into the survivor's area and only deems
// them the owner (「视为拥有」) -- we park nothing, rewrite the real owner, …
```

The card then `remove_from_game` clears the bankrupt seat's field
(`play.rs:5158`), so ONE OF US dies with the bankrupt side it was supposed to
survive (C4, C1). 「视为拥有」 is not the real owner either: a later sale must
follow the survivor's deed, and 「该方视为拥有」 only changes who is asked.
The same entry needs to know *why* the player is out (bankruptcy, forfeit or
game removal) to tell the survivor's hook apart (C2).

### 2.3 `same-sky-mulligan-order` — AG:朝同一片天空迈进, row 32

Book:

> 开局与换牌后，若此卡在你的手牌中，则将此卡打出。

Current site: `rules/cards/card-ag/src/same_sky.rs:15-19,33-50,72-75`. The
`DeckAtGameStart` return is hand-rolled at `:35-50` because the opening deal
runs `draw(i, n, false)` in one batch (`play.rs:548`) and raises nothing per
card; the mulligan redraw (`play.rs:634`) has the same gap, so the 「开局时
抽到此卡洗回」 half cannot run at the deal and the copy stays visible through
the mulligan (`:15-18`). `:72-75` carries a `TODO(ABI)` for the missing
`cardPlayed` on the hand-carried auto-play that answers 「若此卡在你的手牌
中，则将此卡打出」. A card dealt at opening has no event that says so (C2),
and it is dealt *unwatched*, so no arrival fact exists either (C6).

### 2.4 `cardplayed-payload` — PPP:Returns and the play outcome, rows 34, 677, 680

Book (PPP:Returns（1）):

> 每当一张[使用]的卡被[抵消]时…

Current site: `rules/cards/card-ppp/src/returns.rs:31-37`. The card reads
`CardPlayed` and has no way to ask whether the play it heard was cancelled;
`play_from_hand` raises `discarded` and the two play announcements from one
place (`play.rs:5781-5814`) and knows `t.is_cancelled()` at `:5762`, but every
`raise!` builds a **fresh** `Trigger::new` (`rules.rs:680-685`) and does not
copy the `card` link's negation, so `trigger::cancelled()` is false on
`cardAfter`/`cardPlayed` even for a negated play (C2). `one_of_us`'s
counter-half needs the same fact sideways.

### 2.5 `hand-changed-event` — rows 57, 58, 197, 642, 1688

Book (multiple): 「若…加入手牌时」 (相信你 tutors, bounce returns, Here the
world's return) and 「每次你的手牌数从4增加至5」
(CRYCHIC band （3）, `rules/skills/skill-groups/src/crychic.rs:102-112`).

Current site: there is no arrival event at all. Every hand entry is a bare
`hidden[i].hand.push(..)` (`play.rs:5731`, `:5781`; `ops.rs:1207`; the Hand
branch of `add_to_dest` `wasm_rules.rs:1180-1195` raises only under
`DrewBefore`), and the one game card that must see its own arrival says so
with a `TODO(ABI)` (`card-ppp/src/auction_pulled.rs:21-22`). `forever.rs:26-34`
compensates by checking hand size at four other entry points. Arrivals also
differ by **how** they happened: a drawn card is already watched
(`draw_cards_with_hooks`, `play.rs:5483`) and a tutor's add is not, but the
count-based clauses must not treat them alike (C6).

### 2.6 `wasted-flag` — MyGO band （3）, 奇迹水晶, rows 510, 522, 523, 1396, 1701

Book (MyGO!!!!! band （3）):

> 每次你的卡在未生效的情况下进入弃牌堆时，为此卡添加一个可超出上限的[奇迹水晶]

Book (MyGO:MIRACLE （3）):

> 若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效。

Current site: `rules/skills/skill-groups/src/mygo.rs:32-40` matches bare
`Discarded` with only an actor guard, and its `TODO(规则书)` says every owner
discard counts; `rules/cards/card-mygo/src/miracle.rs:171-177` logs
`miracle_wasted` but marks nothing. `discarded` is raised from four unrelated
sites (`play.rs:5781`, `:5596`, `wasm_rules.rs:5632`, the `to_discard` path)
and none can say whether the card took effect (C2, C6). The `Discarded`
condition also cannot see card identity, so a `wasted` bit is the only
carrier that survives to the listener.

### 2.7 `card-on-card` — Here the world, rows 1075

Book (Sumimi:Here the world （2）):

> 将那张卡背面朝上放置于此卡上

Current site: `rules/cards/card-sumimi/src/here_the_world.rs:148`:

```rust
// TODO(规则书)（将那张卡背面朝上放置于此卡上）: 书里把它背面朝上放在此卡上 -- 我们只把它从手牌取出、把 id 记在本卡上，卡本身不入场也不翻面。
```

The held card is a string in a prop (`store_held`) while its crystals sit on
the *host* instance, and a second copy of Here the world reads the prop as if
it were its own. The card is not on the board at all, so nothing can find it,
flip it, or move it with its host (C4). 灯's card attaches to a tile rather
than an instance (`tomori_inner_shout.rs:129-133`, `place_card_on`); the
`tile` field doubles as "on". Nothing records which instance is the host
(C4, C1).

### 2.8 `here-the-world-leave-hook` — rows 639, 645, 867, 875, 1074, 1252, 1474, 1488, 1514, 1515, 1689, 1695

Book (multiple, e.g. Morfonica:夏日合宿, Roselia:Resolve, R:):

> 当此卡效果结束…
> 此卡[移除]时获得1000资金。
> [移除]此卡。

Current sites: `rules/cards/card-morfonica/src/summer_camp.rs:25-33` ends only
at the owner's turn start and says so:

```rust
// TODO(ABI) leave-field hook: the end path is the owner's turn start only
//   -- if the card leaves the field earlier, 「当此卡效果结束」 never runs
```

`card-roselia/src/resolve.rs:31-34` pays only on its own play ("only this play
pays; no removal event exists to key on"), `card-ras/src/guerrilla.rs:236-239`
never clears `colorFor:<seat>`, `card-mygo/src/soyo_colors.rs:103-105` never
disarms `ANY_COLOR`, and Here the world's held card is orphaned when the host
goes. A leave has no event (C2), no destination the card can set (C3), and no
cascade that would carry attachments and children along (C4).

### 2.9 `tomori-earliest-owned` — CRYCHIC:（灯）内心的呐喊, rows 188, 994

Book:

> 放置到你的乐队技能最早拥有的格子上

Current site: `rules/cards/card-crychic/src/tomori_inner_shout.rs:113-140`
takes `owned.min()`, with `firstTile` honoured only if already written, and
the comment at `:105-112` explains that "earliest" needs a deed-acquisition
order written from game start. Nothing is written (C5): `owners` is a plain
tile→owner vector (`state.rs:215`) with no sequence, and a sale leaves no
trace of the order.

## 3. Shared root causes

* **C1 — no zone.** A card's place is a `Vec` membership (`s.field`,
  `s.hand`, `s.graveyard`, …) and a `tile`; leaving is `s.field.remove`
  (`ops.rs:1726-1727`), arriving is `push`. No routine knows the pair.
* **C2 — events are split by caller.** `discarded`, `drawn` and the two play
  announcements are raised only by the few routines that happen to run them
  (`play.rs:5596`, `:5781`, `:5806`; `wasm_rules.rs:5632`); any other caller
  is silent, and the caller's own knowing (cancelled? wasted? counted?) is
  not passed on.
* **C3 — no destination fact.** The pending destination is one field on the
  run (`set_dest`), not a property of the instance, so a leave elsewhere
  cannot carry its own destination — which is why `here_the_world`,
  `summer_camp` and `guerrilla` each hard-code a default and `resolve` pays
  only where it was played.
* **C4 — no instance edges.** `FieldCard` has `tile` but no host and no
  creator (`state.rs:1135-1175`), so nothing can follow a host or a creator.
* **C5 — no acquisition order.** `owners` (`state.rs:215`) is unordered.
* **C6 — no count/waste bits.** Nothing distinguishes a drawn arrival from a
  tutor add, and nothing records that a card entered the discard unused.
* **C7 — face-down is dead.** The field walk's guard skips every face-down
  card (`wasm_rules.rs:5950-5952`, `:6097-6099`), so the card that must react
  to its own flip is skipped exactly then.

## 4. Capability as an invariant

> **Every move of a card between zones is one `ZoneTransition`, applied by one
> writer, which raises the before hooks, lets them rewrite the destination,
> cascades the instance edges, then commits the row change and raises the
> after hooks.**

* **I1 (single writer).** No site outside `Run::zone_commit` may remove a
  field row or push a pile row on behalf of a card move; every such site
  becomes a transition.
* **I2 (pair).** Every transition carries the instance, `from`, `to`, the
  holder and the cause; `from`/`to` are always zones (`Zone::Nowhere` off the
  board), never `-1` sentinels.
* **I3 (before then commit).** `Zone::Field` or `Zone::Attached` on the `from`
  side raises exactly one `zoneLeft` on the subject instance **before** the
  row changes, and exactly one `zoneLeftAfter` **after** it. A
  `Zone::Hand`-side `to` is the mirror: `handChanged` after only. A `from`
  with no rule surface (a pile) raises nothing per card.
* **I4 (destination is a fact, the card's write is a default).** A `zoneLeft`
  responder may rewrite the pending destination by writing `prop::LEAVE_DEST`
  (the `bandori_std::zones::set_leave_destination` sugar); the writer re-reads
  it after the window. The write stands only while the pending destination is
  the ordinary `Graveyard` one, or when the body keeps the card on the field
  (a re-home never really leaves: ONE OF US crosses to the survivor on a
  bankruptcy `Banished`). A destination the caller named -- a bounce to the
  hand (Here the world returning a held card), a bankruptcy `Banished` banish
  -- wins over the card's hard-coded filing.
  The prop is the *pending* destination: the writer clears it before the
  window, so only a write made in that window is a rewrite and a value left
  behind by an earlier leave can never override a destination this caller
  named. The starting value is the caller's explicit argument, else whatever
  `prop::LEAVE_DEST` already holds, else the cause's default (`Graveyard`,
  except `Rehome`/`HostLeft`, which keep the row's place).
* **I5 (cascade order).** A transition out of `Field`/`Attached` processes the
  leaver's attachments first (each as its own transition, cause `HostLeft`),
  then its created children (cause `CreatorLeft`), then the leaver. A child
  that is itself a host recurses depth-first; the cycle guard is the notice
  that the child is already in the current cascade.
* **I6 (attachment invariant).** `on >= 0` holds exactly while the row is in
  `Zone::Attached`; `on < 0` in every other zone. An attachment shares its
  host's `tile`, so `tile_at` keeps answering; it is excluded from
  `field_instances` and from the field walk's ordinary pass.
* **I7 (creator invariant).** `created_by >= 0` implies the creator instance
  exists and is not the child. A child is never reused for another creator.
* **I8 (waste).** `set_ineffective_at` stamps `effective = false` on the
  instance; a transition into `Graveyard` reads it into the transition's
  `wasted` bit and then clears it. The read happens **after** `zoneLeft` (the
  `zoneLeft` payload carries the pre-hook bit, the `discarded` raise the
  post-hook one), because marking is exactly what that body is for. A card
  played from the hand has no instance yet: `set_ineffective_at(-1, true)`
  stamps the play being resolved (`Turn.play_ineffective`) and the play's own
  destination carries it. A card's own normal play never sets it.
* **I9 (count).** `counted` is true **iff** the draw routine performs the
  transition into `Hand`; every other caller leaves it false.
* **I10 (play outcome).** Both play announcements fire exactly once per
  committed play, carrying `play_outcome` = `Cancelled` iff the link was
  cancelled.
* **I11 (out cause).** `beforeOut` carries the cause (`Bankrupt`, `Forfeit`,
  `Removed`); the bankruptcy field clear runs the writer seat by seat with
  cause `Bankruptcy`, so every field card hears one `zoneLeft`.
* **I12 (dead instance).** `destroy_instance_units` also clears the row's
  `on`, detaches its attachments and drops its children first; a dead
  instance can never be a host or a creator.
* **I13 (order).** `earliest_owned_tile(player)` answers the smallest
  acquisition sequence among the tiles the player currently owns, or `-1`.
  The ledger never rewrites an existing sequence; a release frees the tile,
  a re-acquisition takes a new sequence.
* **I14 (no write without hooks).** Every state change that a rule can
  observe (zone, face, attachment, waste, count, outcome, out) is a
  transition or a named write with its hook; nothing observable is a silent
  field write.

## 5. Surface

Everything is generic mechanism in the SDK; this game's words are in
`bandori_std`. No display text, no player decision inside an import.

### 5.1 `card_sdk::abi` — the vocabulary

```rust
/// Where a card instance is. `Nowhere` is off the board (mid-transition or a
/// dead instance). Wire `i32`; an unknown value reads `Nowhere`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum Zone {
    #[default]
    Nowhere = 0,
    Deck = 1,
    Hand = 2,
    Field = 3,
    /// The owner's discard heap (「弃牌堆」; `Dest::Graveyard` names the same
    /// place from the play side).
    Graveyard = 4,
    Banished = 5,
    EventDeck = 6,
    EventDiscard = 7,
    EventRemoved = 8,
    /// On another instance (`FieldCard.on`).
    Attached = 9,
}

/// Why a transition happened. Wire `i32`; an unknown value reads `Other`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum ZoneChangeCause {
    Draw = 0,
    /// The play's own hand → destination transition.
    Play = 1,
    /// A deferred destination applied at run commit.
    #[default]
    Resolve = 2,
    /// An import called by a card body.
    Effect = 3,
    Cost = 4,
    HandLimit = 5,
    Mulligan = 6,
    Opening = 7,
    /// Cascade from the host leaving.
    HostLeft = 8,
    /// Cascade from the creator leaving.
    CreatorLeft = 9,
    Bankruptcy = 10,
    Forfeit = 11,
    /// The row moved between seats unchanged (`rehome_card`).
    Rehome = 12,
    /// A lingering or event instance expiring.
    Expire = 13,
    Attach = 14,
    Detach = 15,
    Setup = 16,
    Other = 17,
}

/// How a play ended. Wire `i32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PlayOutcome {
    Resolved = 0,
    Cancelled = 1,
}

/// Why a player left the game. Wire `i32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum OutCause {
    Bankrupt = 0,
    Forfeit = 1,
    Removed = 2,
}

/// The transition a zone hook is about. Postcard `Option<ZoneChange>` through
/// `sys::trigger_zone`, exactly like `RollPayload` through `trigger_roll`.
pub struct ZoneChange {
    pub from: Zone,
    pub to: Zone,
    pub cause: ZoneChangeCause,
    /// The seat whose area it lands in; `-1` = the owner.
    pub holder: i32,
    /// A discard entry that never took effect (I8).
    pub wasted: bool,
    /// The transition came from the draw routine (I9).
    pub counted: bool,
}
```

`ctx.rs:14` re-exports these beside `AbKind, CardPile`. `PlayOutcome` rides the
existing `card` / `cardAfter` / `cardPlayed` kinds and `OutCause` the existing
`beforeOut`; neither gets a kind of its own.

### 5.2 `card_sdk::abi::prop` — the leave destination

```rust
/// A `Dest` code the card wants when it leaves the field, read at `zoneLeft`
/// commit time. Absent = `Graveyard`. Mirrored in `game_core::state::prop`.
pub const LEAVE_DEST: &str = "leaveDest";
```

### 5.3 `card_sdk::ctx::trigger` — the readers

```rust
/// The transition this raise is about, `None` outside the zone family.
pub fn zone() -> Option<ZoneChange>;

/// The instance (or seat) the raise's subject sits on; `-1` = untouched. The
/// land seat is `zone().holder` on a zone raise.
pub fn instance() -> i32;                                  // exists
/// A `PlayOutcome` code; `-1` outside `card` / `cardAfter`.
pub fn play_outcome() -> i32;
/// An `OutCause` code; `-1` outside `beforeOut`.
pub fn out_cause() -> i32;
```

### 5.4 `card_sdk::ctx` — queries and plain writes

```rust
/// The zone of an instance; `Zone::Nowhere` for a dead `uid`.
pub fn zone_of(instance: i32) -> Zone;
/// The instance `instance` sits on, `-1` when it is not attached.
pub fn attachment_host(instance: i32) -> i32;
/// The instances attached to `host`, in creation order.
pub fn attached_cards(host: i32) -> Vec<i32>;
/// The instances `creator` made, in creation order.
pub fn created_cards(creator: i32) -> Vec<i32>;
/// The instance's real owner (`FieldCard.owner`), `-1` when dead.
pub fn instance_owner(instance: i32) -> i32;
/// Set the real owner without moving the row (「视为拥有」).
pub fn set_instance_owner(instance: i32, owner: i32) -> bool;
/// Record that `instance` was made by `creator` (`-1` clears).
pub fn set_created_by_at(instance: i32, creator: i32) -> bool;
/// Mark the instance as not having taken effect, and read the bit back; the
/// writer also reads it at the next Graveyard entry (I8). `instance = -1`
/// marks the play being resolved instead (a hand card has no instance).
pub fn set_ineffective_at(instance: i32, ineffective: bool) -> bool;
pub fn is_ineffective_at(instance: i32) -> bool;
/// The player's earliest-owned tile under the acquisition order, `-1` if none.
pub fn earliest_owned_tile(player_id: i32) -> i32;
/// Put a card (`card` is a name) onto `host`, face-down when `face_down`.
/// A live instance of the name is re-parented; a pile copy is filed fresh.
/// The attached instance's uid, `-1` when refused (cycle, dead host, no copy).
pub fn attach_card(host: i32, card: &str, cause: ZoneChangeCause, face_down: bool) -> i32;
/// Take `instance` off its host into `dest` (`-1` = its leave destination).
pub fn detach_card(instance: i32, dest: Dest, cause: ZoneChangeCause) -> i32;
/// Move the deed row on `tile` to `owner` unchanged.
pub fn rehome_card(tile: i32, owner: i32) -> bool;
/// Remove `card` from `player`'s hand and run its play now.
pub fn play_hand_card(player: i32, card: &str) -> bool;
/// Suppress the arrival hook for the next `set_dest` (a batch's own moves).
pub fn set_dest_unwatched(dest: Dest);
```

### 5.5 Imports, and the ones re-signed

New exports (`sys` extern, `sys_kani` stub, `hostfns`, `native_shims`,
`func_wrap`), 15:

| # | import | args | pause |
|---|--------|------|-------|
| 467 | `zone_of` | `instance` | no |
| 468 | `attachment_host` | `instance` | no |
| 469 | `attached_cards` | `host, buf, cap` | no |
| 470 | `created_cards` | `creator, buf, cap` | no |
| 471 | `earliest_owned_tile` | `player` | no |
| 472 | `instance_owner` | `instance` | no |
| 473 | `attach_card` | `host, card, cause, face_down` | yes |
| 474 | `detach_card` | `uid, dest, cause` | yes |
| 475 | `set_instance_owner` | `instance, owner` | no |
| 476 | `set_created_by_at` | `instance, creator` | no |
| 477 | `set_ineffective_at` | `instance, on` | no |
| 478 | `is_ineffective_at` | `instance` | no |
| 479 | `rehome_card` | `tile, owner` | yes |
| 480 | `play_hand_card` | `player, card` | yes |
| 481 | `set_dest_unwatched` | `dest` | no |

Re-signed, no new id: `unplace_at(uid)` → `unplace_at(uid, destination,
cause)`; `set_face_down_at(uid, on)` → `set_face_down_at(uid, on, cause)` and
its self wrapper `set_self_face_down(on)` likewise;
`add_to_dest(player, card, dest)` → `add_to_dest(player, card, dest, counted)`;
`set_dest(dest)` → `set_dest(dest, cause)`; `send_to_dest` / `transfer_to_dest`
take a cause. `move_now` folds into the writer.

The six pausing imports (`unplace_at`, `set_face_down_at`, `attach_card`,
`detach_card`, `rehome_card`, `play_hand_card`) use
`request_or_pause(.., Pause::Trap)` and are called with `?` at every site;
`HostRequest` gains `Leave`, `Flip`, `Attach`, `Detach`, `Rehome` and
`PlayHand` (§7.4). All state a request needs is captured **before** the pause
(the run's world copy is discarded on pause, `hostfns.rs:2377-2380`).

### 5.6 `bandori_std::zones` — the game layer

```rust
// rules/bandori-std/src/zones.rs
pub use card_sdk::abi::{Zone, ZoneChangeCause};

/// The field-leave destination a card sets from its own `zoneLeft` body.
pub fn set_leave_destination(dest: card_sdk::ctx::Dest);
pub fn leave_destination() -> card_sdk::ctx::Dest;

/// 「未生效」 sugar over `ctx::set_ineffective_at(ctx::self_uid(), …)`.
pub fn set_ineffective();
pub fn is_ineffective() -> bool;

/// 「同一回合内打出两张」-style card-id match over `trigger::card()`.
pub fn played_card_is(id: &str) -> bool;
```

### 5.7 Not changed

`field_instances`, `cards_in`, `hand_size`, `deck_count`, `discard_size`,
`self_face_down`, `place_card`, `place_card_on`, `play_card`, `invoke_entry`
keep their signatures. `field_instances` keeps answering unattached cards only
(I6).

### 5.8 Classification

`Zone`, `ZoneChangeCause`, `PlayOutcome`, `OutCause` are wire values and live
in `game-kinds::vocab` beside `Seat` / `Payee` (one definition shared with
the guest); `card_sdk::abi` re-exports them and defines the `ZoneChange`
payload. The prop key is engine vocabulary and mirrors in
`game_core::state::prop`; everything game-named is `bandori_std::zones`.

## 6. The trigger payload

### 6.1 Payload, not eight fields

`Trigger` gains:

```rust
/// The transition this raise is about. `None` outside the zone family
/// (`zoneLeft`, `zoneLeftAfter`, `handChanged`, `cardAttached`,
/// `cardDetached`, `discarded`).
pub zone: Option<ZoneChange>,
/// The playing card's outcome on `card` / `cardAfter`; `None` elsewhere.
pub play_outcome: Option<PlayOutcome>,
/// Why the player left on `beforeOut`.
pub out_cause: Option<OutCause>,
```

`Trigger::new` defaults all three to `None` (`rules.rs:507`). The guest mirror
(`world.rs:35`) and `bridge_trigger` (`wasm_rules.rs:9074`) follow the
`roll`/`settle_tile` pattern, and `sys::trigger_zone` writes the postcard
`Option<ZoneChange>` (`hostfns.rs:211-235`). The subject instance is
`Trigger.instance`, already dispatched (`wasm_rules.rs:5975-6005`).

### 6.2 Kinds (engine ids 113–120)

| id | wire | hook-only | payload |
|---|---|---|---|
| 113 | `zoneLeft` | no | `zone` (from/to/cause/holder) |
| 114 | `zoneLeftAfter` | yes | `zone` |
| 115 | `handChanged` | yes | `zone` (`counted`) |
| 116 | `cardFaceFlipped` | no | `value` = the face it turns to |
| 117 | `cardFaceFlippedAfter` | yes | `value` |
| 118 | `cardAttached` | no | `zone` (`to = Attached`) |
| 119 | `cardDetached` | yes | `zone` (`from = Attached`) |
| 140 | `openingHandDealt` | yes | `card`, `value` = deal order |

Seven of the band's eight ids are used (113–119; **120** stays free), because
the draft's `zoneRoundTripped` earned no reader and is dropped (§13 Q6).
`openingHandDealt` takes the next free id above the live table — **140**;
the band 113–120 is the block still free between 112 and 121 (G06's
`abnormalRefused`), and the README amendment records 140 on G08's row. It is
hook-only and carries no window.

`discarded` (existing kind 58) keeps its wire and gains the `wasted` bit in its
`zone` payload. `cardAfter` (32) / `cardPlayed` (50) gain `play_outcome` --
the `card` (12) window is still open when the cancellation lands, so kind 12
cannot carry the outcome of its own window. `beforeOut` (65) gains
`out_cause`.

### 6.3 What each raise carries

| raise | instance | zone payload |
|---|---|---|
| `zoneLeft` / `zoneLeftAfter` | the leaver | from, to, cause, holder |
| `handChanged` | `-1` (a hand card has no instance) | to = `Hand`, counted |
| `cardFaceFlipped` / `After` | the flipped instance | — (`value` only) |
| `cardAttached` | the **attached** card | to = `Attached`, holder = host's seat |
| `cardDetached` | the detached card | from = `Attached` |
| `openingHandDealt` | `-1` | `card` |

## 7. Engine wiring

### 7.1 `ZoneTransition` and the writer

`game_core::engine::world`:

```rust
/// One card move, fully described before any row changes.
pub struct ZoneTransition {
    pub uid: i32,
    pub from: Zone,
    pub to: Zone,
    pub cause: ZoneChangeCause,
    /// The seat whose area it lands in; `-1` = the row's owner.
    pub holder: i32,
    /// The host for `to = Attached`.
    pub host: i32,
    /// The arrival face for `to = Attached` (the book puts held cards down).
    pub face_down: bool,
    /// Raised only when the caller is the draw routine.
    pub counted: bool,
    /// Already inside a cascade (the cycle guard).
    pub cascaded: bool,
}
```

The split follows the existing `CardWorld` boundary: `game_core::World`
owns the pure row mechanics (`World::resolve_zone_transition`,
`World::apply_zone_row`, `World::drop_instance_edges`), and `Run`'s `CardWorld`
impl (`wasm_rules.rs`) owns the raise and therefore the sequencing.
`Run::zone_prepare(&mut self, transition) -> Option<ZoneTransition>` resolves
the pair (I2, I4), rejects a self-host or a cycle, and returns the effective
transition; `Run::zone_commit(&mut self, transition)` is the only row writer
(I1). It:

1. reads the row, refuses a stale `uid` or a dead instance;
2. when `from` is `Field`/`Attached`, clears `prop::LEAVE_DEST`, raises
   `zoneLeft` on the instance, and adopts the destination only if the body
   rewrote it (I4);
3. cascades attachments, then children (I5), each through
   `zone_prepare`/`zone_commit` with `cascaded = true`;
4. applies the row through `World::apply_zone_row`: detaches `on`, clears
   `tile`/`face_down` per the destination, removes the row from its vector
   and pushes it into the destination vector (`hidden[holder].graveyard` /
   `.hand`, `event_discard`, …), or inserts it into the host's vector with
   `on = host` and `tile = host.tile`;
5. raises `zoneLeftAfter`, then `handChanged` / `cardAttached` /
   `cardDetached` as the destination dictates (I3, I6, I8);
6. on a Graveyard arrival, raises `discarded` with `wasted` and clears it.

The host fixture (`host.rs`) and the Kani model implement the two trait
methods the same way (§10.3), so a card's body sees one shape in every build.

`move_now` (`wasm_rules.rs:709`), `unplace_card` (`ops.rs:1988`),
`unplace_at` (`ops.rs:1714`), `take_card`'s Field branch (`wasm_rules.rs:1198`),
`to_discard` (`:1233`), `apply_dest` (`:6990`), the `play_from_hand`
destination match (`play.rs:5781-5790`), the bankruptcy clear (`play.rs:5158`)
and `remove_from_game` (`:5121`) all become callers. `ops.rs:1726-1727`'s
`s.field.remove` and `:1431`'s `s.field.push` move into the writer.

### 7.2 The bits the writer reads

* `counted`: only `draw_cards_with_hooks` (`play.rs:5483`) and the opening deal
  set it; the opening deal (`play.rs:548-550`) also raises `openingHandDealt`
  after each single deal (replacing the batch `draw(i, n, false)`), and the
  mulligan redraw (`:634`) is counted with cause `Mulligan` — a `handChanged`
  only, not a second `openingHandDealt` (§13 Q8).
* `wasted`: `FieldCard.effective` defaults true; `set_ineffective_at` clears it
  (or `Turn.play_ineffective` for a `-1` mark); a Graveyard arrival reads it
  after the leave hooks (I8). `discarded` carries it.
* `play_outcome`: `play_from_hand` already knows `t.is_cancelled()`
  (`play.rs:5762`); both announcements read it (I10). `cardAfter`/`cardPlayed`
  fire even for a cancelled play — that is what PPP:Returns negates.

### 7.3 Dispatch

The two dispatch shapes already exist and are reused as they are:

* **Own-card kinds** (`is_own_card_kind`, `wasm_rules.rs:12476-12480`): the
  raise names the card on `t.card`, the card's own def runs whatever its zone
  (this is how `Drawn` reaches a card still in hand, `:5901-5914`). The
  arriving card of a `handChanged` and of an `openingHandDealt` has no
  instance, so both are own-card kinds and their instance is `-1`.
  `is_own_card_kind` gains `HandChanged` and `OpeningHandDealt`.
* **Instance kinds**: the subject pass (`:5975-6005`) runs the raise's own
  instance whether it is still placed or already gone, and the ordinary field
  walk runs every other placed card. `zoneLeft`, `zoneLeftAfter`,
  `cardFaceFlipped*`, `cardAttached` and `cardDetached` are instance kinds;
  `discarded` keeps its existing own-card **and** field-walk shape.
* The face-down guard (`wasm_rules.rs:5950-5952`, `:6097-6099`) **stays** for
  the field walk. The subject pass has no such guard, so the flipping /
  leaving instance itself always hears its own hook (R15).
* The field walk skips an attached row (`f.on >= 0`) and a dead seat (existing
  `world.out` guard, `:5935`).

### 7.4 Host requests

```rust
pub enum HostRequest {
    /// Take the instance off play into `destination` (`-1` = the card's own
    /// leave destination, I4). Answers the departed owner, `-1` when dead.
    Leave { uid: i32, destination: i32, cause: ZoneChangeCause },
    /// Flip the instance; `face_down` is the target face, `cause` why.
    Flip { uid: i32, face_down: bool, cause: ZoneChangeCause },
    /// File a new instance of `card` onto `host` (§7.5). Answers the new uid.
    Attach { host: i32, card: String, cause: ZoneChangeCause, face_down: bool },
    /// Take the instance off its host into `destination` (§7.5).
    Detach { uid: i32, destination: i32, cause: ZoneChangeCause },
    /// Move the deed row on `tile` to `owner` unchanged.
    Rehome { tile: i32, owner: i32 },
    /// Remove `card` from `player`'s hand and run its play now.
    PlayHand { player: i32, card: String },
}
```

Each is handled in `apply_host_request` (`wasm_rules.rs:4501`) exactly like
`Target`/`Move`: resolve against `cx`'s world copy, run the window (which may
`?` into a prompt), commit, answer an `i32`. `PlayHand` re-enters
`play_from_hand` (`play.rs:5676`) — the whole pipeline (declare, `card`, body,
`discarded`, `cardAfter`, `cardPlayed`), so `same_sky` sees a real play,
`Returns` sees its outcome, and the hand-size clamp of `forever` still runs.

### 7.5 Attachment rules

* `attach_card(host, card, cause, face_down)`: `card` is a **name**. A copy in
  the caller's hand is taken first (a card in a pile has no instance): the
  writer files a new row (`from = Nowhere`, a fresh uid, `owner` = the pile's
  seat — not the host's; Here the world holds *its target's* card). That order
  keeps a caller that just drew the copy on the copy it means. Only a name
  with **no hand copy** falls back to the caller's first live instance, which
  is re-parented as it stands: its uid, counters and creator edges ride along,
  `from` is its real zone, and the cycle guard walks the host chain (I6) —
  which is what makes "A on B, then B on A" a refusal rather than a second
  stack. Either way `on = host`, `tile = host.tile` and the face is the
  argument's.
* `detach_card(uid, dest, cause)`: `dest` `-1` means the row's own
  `prop::LEAVE_DEST`, else `Hand`; the name is pushed into the destination
  pile and the row is dropped.
* A host transition cascades its attachments with cause `HostLeft`: a player
  host's attachments follow the host's destination; a **board-owned** host
  (a tile rule) does not leave, so its attachments fall to `Graveyard`
  (R12).
* A board-owned row (`owner = BOARD_OWNER`) has no seat to file into: its
  name, and a created child's, goes to the neutral `MatchState::board_discard`
  instead of player 0's graveyard. Player 0 is not a part of the board, and its
  reshuffle would otherwise deal rule names back as dead hand cards (review
  finding 4).
* Attachment chain depth is unbounded but cycle-free (I6); `attached_cards`
  answers direct children in creation order.

### 7.6 Edges

`FieldCard` gains:

```rust
/// The instance this one sits on, `-1` when not attached.
#[serde(default = "minus_one")]
pub on: i32,
/// The instance that made this one, `-1` when none. A child dies with its
/// creator (I7).
#[serde(default = "minus_one")]
pub created_by: i32,
/// Did this card take effect? Cleared by `set_ineffective_at` (I8).
#[serde(default = "truth")]
pub effective: bool,
```

`destroy_instance_units` (`ops.rs:2381`) calls `World::drop_instance_edges`
first — it clears the row's `on`, detaches its attachments and drops its
children (I12) — before clearing the counters and marks it already clears;
`field_by_uid` (`ops.rs:1443`) resolves either vector.

### 7.7 Ownership order

`MatchState` gains:

```rust
/// Per tile: when the current owner took it. `-1` = nobody / not held.
#[serde(default)]
pub owned_seq: Vec<i32>,
#[serde(default)]
pub next_owned_seq: i32,
```

The owner write moves off `Run::set_owner` (`wasm_rules.rs:2101`) into one
`game_core::World::set_tile_owner(tile, owner)` that also writes the pair: a
tile changing to a player with no sequence gets `next_owned_seq` and it
increments; a release to `-1` clears it. Every writer calls it — the guest
import, `assign_deed` (`play/purchase.rs:318`), the forced-acquire paths and
the bankruptcy release (`play.rs:5174`). A write that removes a player's
earliest tile rewrites that player's `firstTile` slot from
`earliest_owned_tile` (I13). `earliest_owned_tile` is the read in §5.4.

## 8. Card usage

### 8.1 Mujica:（喵梦） (`nyamu_card.rs`)

Replace the `TurnEnd` sampler with two field hooks on the instance:
`CardFaceFlipped` (before) asks the owner whether to lose 2 火罐 and, on yes,
adjusts the counters; `CardFaceFlippedAfter` draws when the new face is up.
The face-down guard no longer matters (the subject pass runs), so both work
while the card is down. `WAS_DOWN` is deleted.

### 8.2 AG:ONE OF US (`one_of_us.rs`)

`before_out`: `ctx::rehome_card(t, survivor)` for each live designated deed and
`ctx::set_instance_owner(instance, survivor)`, with `trigger::out_cause()`
telling bankruptcy from forfeit; the card itself leaves with the bankrupt seat
and its own `zoneLeft` (cause `Bankruptcy`) rehomes if `before_out` could not.
The `MINE`/`THEIRS` marks become the attachments of the card (item 7) and die
with it.

### 8.3 Sumimi:Here the world (`here_the_world.rs`)

The hold is `ctx::attach_card(ctx::self_uid(), id, Effect, true)`, whose answer
is the held instance; the crystals go on the host instance (`其` is 此卡)
(`ctx::add_counter_at`); the return is `ctx::detach_card(held, Dest::Hand,
Effect)`; the turn-start tick counts the host's crystals. A
`zoneLeft` body on the host
returns the held card and sets `prop::LEAVE_DEST` to `Graveyard`
(`bandori_std::zones::set_leave_destination`). Both `TODO(规则书)` and the
C-Q49 "when does it leave" note get their answer.

### 8.4 MyGO:MIRACLE (`miracle.rs`) and MyGO band （3） (`mygo.rs`)

`miracle` marks itself from its own `zoneLeft` entry: when the transition's
`to` is `Graveyard` and crystals remain, it calls
`ctx::set_ineffective_at(ctx::self_uid(), true)` — the writer's post-hook read
(I8) then puts the bit on `discarded`, wherever the discard came from (its own
pass clause, another card's dump, a cascade). The old
mark-before-`set_dest` placement in the pass body missed every other path.
`mygo`'s `Discarded` condition becomes `trigger::zone().is_some_and(|z|
z.wasted)` and the band （3） gains exactly the cards that entered unused.
`haneoka`'s clause is the hand-play form: the card is discarded while still a
hand card, so the mark goes on the play (`ctx::set_ineffective_at(-1, true)`,
`bandori_std::zones::set_ineffective`), not on an instance.

### 8.5 Alice/CiRCLE (`black_suits.rs`) and 日菜 (`hina_sound.rs`)

The borrowed `tile:circle` instance is stamped
`ctx::set_created_by_at(instance, ctx::self_uid())` instead of the interim
`unplace_at`; when the marker glow goes, the child drops. `hina_sound`'s
substitute takes `set_created_by_at(instance, -1)` (adopted from the stand-in)
rather than keeping the edge on the banished stand-in.

### 8.6 AG:朝同一片天空迈进 (`same_sky.rs`) and PPP:拍卖撤下来了 (`auction_pulled.rs`)

`same_sky` splits its two halves. Its `OpeningHandDealt` body (own-card, so it
runs at the deal while the copy is in hand) takes the copy and files it back
with `add_to_dest(DeckRandom)` — 「开局时抽到此卡洗回」. Its `Mulligan`
`HandChanged` body then plays whatever copy is still in hand after the
initial redraw with `ctx::play_hand_card` — 「换牌后，若此卡在你的手牌中，
则将此卡打出」. Both are real events, so the `DeckAtGameStart` fallback and
the `TODO(ABI)` at `:72-75` go. `auction_pulled` keys on `HandChanged` with
`trigger::zone().counted == false`.

### 8.7 `forever`, `fate_together`, `tomori_inner_shout`, `summer_camp`, `resolve`, `guerrilla`, `soyo_colors`

`forever` collapses its four backstops onto `HandChanged` (hand >= 7) plus the
existing boundaries. `fate_together`'s return reads `HandChanged` so its
「不触发乐队技能的（3）效果」 comment is expressed by `counted = false`.
`tomori_inner_shout` uses `earliest_owned_tile`. `summer_camp`, `resolve` and
`guerrilla`/`soyo_colors` get their `zoneLeft` / `zoneLeftAfter` bodies and
`prop::LEAVE_DEST`; the four `TODO(规则书)` leave notes (morfonica, roselia,
ras, mygo) drop to the sheet's own timing.

## 9. Save, record and replay

* Save: the three new `FieldCard` fields are `#[serde(default)]` (I6-I8), so
  SAVE 16 states load with `on = -1`, `created_by = -1`, `effective = true`.
  `SAVE_VERSION` moves to **17** because the holder/attachment shape is a
  persisted relation, not a repaired scalar.
* Record: `RECORD_VERSION` stays **4**. The event log stores the raised link;
  `zone` is derived when the link is re-raised from the same transition, not
  stored, so an old record replays through the archived bundle (which carries
  its own host). The one thing a record does store is the `uid`, already
  stable through the save (`MatchState.trigger_uid`, `state.rs:206`).
* Replay: the re-run recomputes the same transitions in the same order from
  the same pre-action state, so the same uids and the same destinations;
  `owned_seq` is in the save, so `earliest_owned_tile` is stable.
* A pause inside a cascade records its `HostRequest` in the answer log like
  every other `Pause::Trap` site, so a re-run consumes the same answers.
* A re-run re-executes the body from its top, so the reads **before** the
  replayed request must see the entry state, not the request's result. The
  drive puts the rows a routine took out back at its loop top (the row
  overlays keep the pre-overlay row) and hands the reinserted uids to the
  pass: a uid the pass's replay takes out again is gone, a uid it leaves is
  dropped from the committed world (a nested hook's leave the enclosing
  routine never re-issues). The commit pass's staging overlay leaves the rows
  in place for the same reason -- that pass replays every request itself --
  but leaves a row the body **minted** out: the replay re-places it at the
  same call (the placement ledger pins the uid), so a body that reads the
  field before and after a placement (the stand-in cards) must not find its
  own row already there. Piles follow the same rule one level up: the pile
  checkpoint that re-supplies an answered request's pile half is captured
  *before* the body's own pile writes are overlaid on the live world, or a
  body that discards and then asks (the [共鸣] fuel) would find the fuel gone
  on every re-run and skip its own branch.
* A replayed `Leave` of the running instance re-posts the
  `log.outcome.unplace` line the original `unplace_card` posted. The commit
  pass replaces the learn pass's line stream in place, so every line the learn
  pass wrote at a pause must be written again at the same point or the stream
  shifts. A replayed `Detach` posts none (the live `zone_detach` writes no
  such line).

## 10. Tests and Kani obligations

### 10.1 Tests — `rules/tests/tests/g08_zone_lifecycle.rs`

One named file per group (G06/G07 rule). Fixtures in
`rules/fixtures/test-cards/src/lib.rs` (`TEST:zoner` with hooks on every zone
kind, `TEST:holder` with an attachment, `TEST:creator` with a child):

1. `leave_raises_before_and_after_with_cause` — one `zoneLeft` then one
   `zoneLeftAfter`, the payload pair reversed on the second.
2. `leave_destination_rewrite_wins` — the body sets `Hand` and the caller
   named the default `Graveyard`; the row lands in the hand, not the graveyard.
   A caller that named `Hand`/`Banished` itself keeps it (I4: the write is a
   default).
2b. `stale_leave_destination_does_not_override_the_caller` — after an earlier
   `Hand` rewrite, a second leave whose body writes nothing and whose caller
   names `Banished` lands banished: the prop is pending, not persistent.
3. `attachment_cascades_before_host` — host leaves; the attachment's
   `zoneLeft` precedes the host's, both carry `HostLeft`.
4. `creator_child_dies_with_creator` — the child row is gone, cause
   `CreatorLeft`; a grandchild recurses.
5. `attach_cycle_rejected` — A on B, then B on A refuses.
6. `hand_changed_counted_only_for_draws` — a draw is `counted`, a tutor add is
   not, a bounce is not; the count-based band clause fires only on the draw.
7. `wasted_only_when_marked` — `set_ineffective_at` then discard gives
   `wasted = true`; a plain discard is false.
8. `play_outcome_cancelled` — a countered play still raises the two
   announcements with `Cancelled`.
9. `opening_deal_events` — one `openingHandDealt` per dealt card, in order,
   before the mulligan asks.
10. `flip_window_prompts_and_face_down_self_runs` — the flipping card hears its
    own hook while down.
11. `bankruptcy_field_clear_is_per_card` — one `zoneLeft` per field card,
    cause `Bankruptcy`, before the seat's `out` flips.
12. `rehome_keeps_row_identity` — the card keeps its `uid`, counters and
    crystals, changes seat, and stays attached.
13. `earliest_owned_tile_follows_acquisition` — buy B then A then sell B; the
    answer is A, not B.
14. `board_host_attachment_falls_to_graveyard`.
15. Each card's own test in its existing suite file (`rules/tests/tests/
    mujica.rs`, `ag.rs`, `sumimi.rs`, `mygo.rs`, `ppp.rs`, `morfonica.rs`):
    喵梦's two clauses, ONE OF US's transfer, Here the world's hold and
    return, MIRACLE's waste, `auction_pulled`'s entry, `summer_camp`'s early
    leave.

### 10.2 Kani specs

`rules/kani/src/spec/g08_zone_lifecycle.rs` models `prepare`/`commit` on the
model world and proves (each with a mutant harness under `proofs/`):

* `transition_pair_iff` — `from`/`to` are the row's real pair for arbitrary
  targets; a mutant keeping the old `tile = -1` sentinel answer fails.
* `cascade_order_iff` — attachments then children then the leaver, exactly
  once each; a mutant reversing the order fails.
* `counted_iff` — true iff the caller is the draw routine.
* `wasted_iff` — true iff the effective bit was clear at a Graveyard entry,
  and the bit is cleared by the entry.
* `attachment_cycle_iff` — an attach is admitted iff the host is not the card
  nor one of its descendants.
* `earliest_owned_iff` — the sequence-minimum over the currently owned set.
* `destroy_clears_edges_iff` — after `destroy_instance_units` no live row
  names the uid in `on` or `created_by`.

### 10.3 Kani model changes

`rules/kani/src/world.rs`'s `FieldCard` (`:615-639`) gains `on`, `created_by`,
`effective`; `MAX_FIELD` (`:54`) holds attachments; `field_instances`
(`host.rs:2193`) skips attached rows; `arm_unplace_at` and a new
`arm_attach_card` / `arm_detach_card` / `arm_trigger_zone` follow the real
writer. `world::bound_uid` and `field_by_uid` resolve both edges.

### 10.4 Card rows

The nine 喵梦 / ONE OF US / Here the world / MIRACLE / `auction_pulled` /
`summer_camp` / `resolve` / `guerrilla` / `soyo_colors` rows stop being
PROTOCOL-LEVEL where the hook is now a modelled host query; the band-（3）
crystal row stays PROTOCOL-LEVEL (it reads `wasted`, which the model proves
separately).

## 11. Versioning and band arithmetic

`ABI_VERSION` 113 → **118** (the batch landed after G12, G14, G11 and G09, so
it takes the next free version and 114–117 are already spent), entry inserted
below the v117 contract block:

```
/// v118: **zone, attachment and creator lifecycle**. Every card move between
///    zones is one `ZoneTransition` through one writer; `zoneLeft` /
///    `zoneLeftAfter` fire on the leaving instance, `handChanged` on a hand
///    arrival (`counted` only from the draw routine), `discarded` carries
///    `wasted`, `cardAfter`/`cardPlayed` carry `play_outcome`, `beforeOut` carries
///    `out_cause`. New kinds 113-119 and `openingHandDealt`; new payload
///    `abi::ZoneChange` through `ctx::trigger::zone()`. `FieldCard.on` /
///    `created_by` / `effective`; `ctx::attach_card` / `detach_card` /
///    `attached_cards` / `attachment_host` / `rehome_card` / `play_hand_card`
///    / `zone_of` / `instance_owner` / `set_instance_owner` /
///    `set_created_by_at` / `set_ineffective_at` / `is_ineffective_at` /
///    `set_dest_unwatched` / `earliest_owned_tile` / `is_own_placement`; `unplace_at` /
///    `set_face_down_at` / `add_to_dest`
///    / `set_dest` / `send_to_dest` / `transfer_to_dest` gain a cause/flag
///    argument. SAVE_VERSION 19 -> 20 (the edges and the held-card relation);
///    RECORD_VERSION unchanged.
```

Band arithmetic from the README: the G08 row is `ABI_VERSION` 105→106 /
SAVE 12→13 / kinds 113–120 / ops 351–370, written against the original base
and never rebased since. At the current base the row must be re-derived: G01
took 104→105 and kinds 111–112; G02–G06 landed; G07 is the pending
owner of 112→113 and ops 441–450. The kinds **113–120 remain free in
`ENGINE_KINDS`** (live ids run 1–112 and 121–131, so the G08 row's kind band
is still exactly right and needs no shift). Its ABI and SAVE cells and its op
band shift by the landed groups: this document takes **ABI 117→118**, **SAVE
19→20**, and **ops 467–486** (nineteen of the reserved twenty; 486 stays free for
review additions). The one extra kind (`openingHandDealt`) takes the next free
id above the live table, **140**; G10's kind row shifts to 141.

## 12. Alternatives

**A1 — no transaction; raise from each call site.** Keeps the current split,
but the pair, the cascade and the "let the card rewrite its destination"
ordering would have to be repeated at every one of the nine callers, which is
exactly how the current disagreement grew. Rejected.

**A2 — attachments as a tile sentinel (`tile = -2`, host in a prop).** No new
field, but `tile_at` lies, the cascade has no cheap lookup, and a card attached
to a card attached to a tile cannot be expressed. Rejected; `on` is one field.

**A3 — a separate `attachments: Vec<FieldCard>`.** Cleaner scans, but needs a
second uid space or cross-vector lookups, and `destroy_instance_units` must
walk both. Rejected; the `on` bit plus a helper keeps one vector.

**A4 — the held card keeps its prop-store representation and only gains a
leave hook.** Cheaper, but the crystals then sit on the host, a second copy
reads them, and the card never flips or leaves with its host. Rejected.

**A5 — a flip as a counteraction window (a `counteract` entry on the flip
link).** Gives the [反击] texture for free, but the book's flip clauses are
plain 「时」 hooks (a responder could not tell it apart from a real counter),
and it would open a whole chain per flip. Rejected; the two flat kinds keep
the distinction.

**A6 — a dedicated `cardRemoved` kind instead of `to = Banished`.** A listener
would not need the payload; but every other destination would still need one,
so the kind table would double. Rejected.

**A7 — `handChanged` only in `add_to_dest`'s Hand branch.** That is today's
shape (raise only when `DrewBefore`), and it cannot see the direct pushes in
`play_from_hand` (`play.rs:5731`, `:5781`) or the opening deal. Rejected; the
push sites all become the writer.

**A8 — `created_by` as a prop key.** Free storage, but a scan is a full field
walk, and a prop outlives what it should (the same hazard `black_suits` has
today). Rejected; the edge is a column.

**A9 — one `cardFiled` event for every zone entry.** Half the kinds, but the
listener then branches on `to` for a job it could have been dispatched for,
and `handChanged`'s count semantics are the only case that needs it.
Rejected.

**A10 — rehome as `unplace` + `place`.** Needs both a leave and an enter for
a move that never leaves play, and loses the row's identity (counters,
crystals, effective card). Rejected; `rehome_card` moves the row itself.

**A11 — `play_hand_card` as a body-only `play_card` + manual hand removal.**
That is `same_sky`'s current fallback; it skips `declare_activation`, the
`card` window and `cardPlayed`, which is exactly what the sheet item wants.
Rejected.

**A12 — the acquisition sequence as a per-player list of tiles.** Needs
insertion, removal and duplicate handling; the tile-indexed `owned_seq`
answers the only query (the minimum for *this* player) without a list.
Rejected.

## 13. Open questions

1. **Attachment destination default.** A held card sets its own destination;
   what about an attachment that has none (a marker-like card)? Design
   defaults it to `Graveyard`. Confirm.
2. **Attachment visibility.** Excluding attachments from `field_instances` is
   what keeps `miracle`'s crystal sum right, but a card that should count
   attached cards must now call `attached_cards`. Confirm the split.
3. **Board host.** A tile rule never leaves, so a `HostLeft` cascade from it
   is really "the host expired". Should an event-rule expiry (`Expire`) also
   drop its attachments to `Graveyard`, or to `Banished` for a face-down
   held card?
4. **Rehome and `tile`.** The row keeps its `tile` (the deed is a tile), but a
   non-deed card moved by `rehome_card` has no tile; is `-1` the right
   placeholder, or must rehome keep the caller's tile?
5. **Creator edge on a pile card.** A created card normally has an instance;
   a created *card name* in a pile has none. Should `created_by` survive into
   the pile row (a pile row is a string), or is an instance-only edge enough?
   Design: instance-only.
6. **A leave-and-return in one run.** The draft's `zoneRoundTripped` had no
   reader and is dropped. Should a summer-camp-style self-bounce instead be
   one event, or do listeners read the two transitions (with `HandChanged`'s
   `counted = false`)? Design: two transitions for now; id 120 stays free for
   a round-trip kind if a card needs it.
7. **`counted` on a forced draw.** A draw the engine performs inside an
   effect (a 「抽N」 body) is still `counted`; is that the intended meaning
   (「抽」) or should only turn-draws count?
8. **`openingHandDealt` and the mulligan redraw.** The mulligan's redraw is
   cause `Mulligan`; does it also raise `openingHandDealt`? Design: no — the
   deal alone is `openingHandDealt`, the redraw is `handChanged(Mulligan)`, so
   朝同一片天空迈进 can tell 「开局时抽到」 (return it) from 「换牌后」 (play
   it).
9. **Out cause on `remove_from_game`.** A match-end removal is `Removed`; does
   it run `beforeOut` at all today? Design: it runs, with `Removed`, so ONE OF
   US still transfers (C-Q50).
10. **Waste through an attachment.** A card discarded while attached is a
    `wasted` discard only if it never took effect; attached cards have not.
    Confirm the bit is still per instance and not inferred.
11. **`firstTile` slot versus `earliest_owned_tile`.** The slot is a cache;
    should the engine keep writing it, or should 灯's card call the query and
    drop the slot? Design: the engine rebuilds it (so any future card may
    read it) and the query is the authority.
12. **C-Q50(a): the owner write or a view.** 「视为拥有」 is normally a
    continuous view; design writes the real instance owner because the card
    itself moves and its `owner` is what the seat tests read. The rest of
    C-Q50 stays: the two deeds are
    attachments (not a marker on the tiles), a sold deed leaves the set, and
    the pair is cleared on the way out.
13. **Attach a card that is already attached.** Design makes it
    `detach` + `attach`: a fresh instance, counters not carried, no
    `cardDetached` suppression. A future card that needs counters to survive
    would take a re-parent form of `attach_card`.

## 14. Necessity

This section answers three questions per item, in order: **does the book
actually print the clause**, **is the surface its minimum**, and **why can it
not wait**. It is written item by item and surface by surface so a reviewer
can strike one piece without having to strike the group. Every claim of the
form "today X" cites the site where X happens, and every "if removed" names
the card test that fails without it.

Each surface carries one of three classes:

* **forced** — no alternative satisfies the clause; the surface is the
  minimum.
* **forced-now** — an alternative exists but is mistimed or not
  composition-safe; the surface is the smallest correct shape *in this
  group*.
* **convenience** — a named wrapper over a mechanism already present; it can
  be struck with a mechanical rewrite and no behaviour change. Conveniences
  are listed so they are not mistaken for load-bearing.

### 14.1 The group: two facts and one order

The group is not eleven independent fixes. It is **two facts that do not
exist** and **one ordering that follows from them**:

1. *A card can be in a named place, and a move between places is one
   describable event.* Today the place is a vector membership (`s.field`,
   `s.hand`, `s.graveyard`) and a `tile`; the pair is never named, leaves are
   `s.field.remove` (`ops.rs:1726-1727`), arrivals are `push`
   (`ops.rs:1431`), and the only event a few callers happen to raise
   (`discarded`, `drawn`, `card`/`cardAfter`) is raised without the pair or
   the reason (C1, C2).
2. *A card can sit on another card and can have been made by another card.*
   `FieldCard` has `tile` and nothing else, so the two relations have no home
   (C4), and six cards hard-code a destination or a string that should be a
   row (C3).
3. *Order follows:* a host and a creator must be left before the card, and a
   listener must run before the row changes. That ordering is the whole
   reason a host cascade cannot be written after the fact.

Items 1, 3, 4, 5 and 6 are all instances of fact 1 — they are the reason
(cause), the count (drawn vs added), the waste bit, the outcome, and the
before/after pair. Items 2, 7 and 8 are fact 2 and its cascade. Item 9 is the
one piece of bookkeeping (C5) that has no place to live until the writer
exists to record it.

The book prints all of them; none is speculative. `docs/rulebook/cards.json`
is the ground truth and §15 quotes each sentence.

### 14.2 Per item

**1 — face flip (rows 347, 348, 1043, 1272).** Clause 1 「每当此卡将从正面
翻至背面时可选择失去2个火罐」 is **not implemented at all**; clause 2 「每当此卡
将从背面翻至正面时此卡拥有者抽一张卡」 is implemented by a sampler that is
skipped exactly when it fires (C7): the field walk's guard skips every
face-down card (`wasm_rules.rs:5950-5952`, `:6097-6099`), so `WAS_DOWN` stays
`0` and the draw never happens in the ordinary path (`nyamu_card.rs:57-73`).

*Smallest alternative:* A1-style sampling at more kinds — every kind a flip
can happen at (play, counteraction, status exit, Untargetable expiry) would
need its own copy of the sample, and the sample still cannot run while down.
*Class:* forced. Two kinds and one re-signed import, because clause 1 is a
choice (a prompt) and 「将从…翻至」 is strictly before the face changes.
*Fails without it:* `nyamu`'s two clauses, both silent.

**2 — rehome, owner, creator (rows 1107, 1124; adopted 283, 487, 488, 490,
1029, 1555).** The book says 「将两张被指定地契放置在该卡上并转移到存活方的
游戏区，该方视为拥有此地契」. Today the whole hand-over is one tile-owner
rewrite (`one_of_us.rs:319`): nothing is parked on the card, the card itself
stays on the bankrupt row and is cleared with it (`play.rs:5158`), and the
survivor is not distinguished from a bankruptcy or a forfeit.

*Smallest alternative:* A10 (`unplace` + `place`) gets the card to the
survivor but loses the row's identity — counters, crystals and the effective
card — and needs a leave/enter pair for a move that never leaves play. A
separate `set_instance_owner` write is unavoidable either way because
「视为拥有」 is a fact about the *card*.
*Class:* forced for the request and the owner write; forced-now for the
creator edge (the two cards that need it, `black_suits` and `hina_sound`,
both carry `TODO(G08)`).
*Fails without it:* ONE OF US's transfer, its cause test, the borrowed
`tile:circle`, and 日菜's substitute.

**3 — opening deal (row 32).** 「开局时抽到此卡洗回」 and 「换牌后，若此卡在
你的手牌中，则将此卡打出」 are split by the deal, and the deal is one batch
`draw(i, n, false)` (`play.rs:548`) that raises nothing. The card answers
`DeckAtGameStart` (which fires after the mulligan, `same_sky.rs:15-18`) and
hand-rolls the return; the play half is hand-carried and carries a
`TODO(ABI)` (`:72-75`).

*Smallest alternative:* reusing `Drawn` — it is exactly the event the deal
does not raise, and the `hooks=false` batch exists to keep the deal quiet.
A11 (body-only play) would skip `declare_activation`, the `card` window and
`cardPlayed`, which is the half the sheet asks for.
*Class:* forced (`openingHandDealt`), forced-now (`play_hand_card`,
`set_dest_unwatched`).
*Fails without it:* the card is swept too late and its auto-play is not a
real play.

**4 — play outcome (rows 34, 677, 680).** 「每当一张[使用]的卡被[抵消]时」.
The caller holds `t.is_cancelled()` (`play.rs:5762`), but each `raise!` builds
a fresh trigger (`rules.rs:680-685`) and copies neither the negation nor the
link, so the announcement hooks see `cancelled == false` on a negated play.

*Smallest alternative:* reading `trigger::negation` on the hook — impossible
for the same reason (fresh trigger, `Negation::None`); re-raising through the
original link would change the hook's other payload. A second kind
(`cardPlayCancelled`) doubles the announcement surface.
*Class:* forced. One optional enum on three existing kinds.
*Fails without it:* PPP:Returns counts every play as resolved.

**5 — hand arrival (rows 57, 58, 197, 642, 1688).** Five clauses key on
「…加入手牌时」 or on a hand-size crossing. Every hand entry is a bare
`push` (`play.rs:5731`, `:5781`; `ops.rs:1207`; the Hand branch of
`add_to_dest`, `wasm_rules.rs:1180-1195`, raises only when the caller passed
`DrewBefore`), and `auction_pulled` says so with `TODO(ABI)`
(`auction_pulled.rs:21-22`).

*Smallest alternative:* A7 (raise only in `add_to_dest`) is today's shape and
cannot see the two direct pushes or the deal. A *condition* on hand size at
some other kind re-derives what a listener could have been told.
*Class:* forced for the event; forced for `counted` (the draw/added
distinction is the difference between five clauses' meanings);
convenience for nothing.
*Fails without it:* the tutor, the bounce, `fate_together`'s return,
`forever`'s clamp and the CRYCHIC band counter.

**6 — waste (rows 510, 522, 523, 1396, 1701).** 「未生效的情况下进入弃牌堆」
and 「视为此卡未生效」. `discarded` cannot say, the log line is display, and
`miracle` writes only the line (`miracle.rs:171-177`).

*Smallest alternative:* a `cond` over a card name for the specific cards —
possible for MIRACLE, impossible for the band clause (「你的卡」, any card) and
for any future card. A condition also cannot read a per-entry fact after the
row moved.
*Class:* forced (`wasted`, `set_ineffective_at`), convenience
(`is_ineffective_at`, so a card can check its own flag without a prop).
*Fails without it:* the band （3） crystal and MIRACLE's clause.

**7 — attachment (row 1075).** 「将那张卡背面朝上放置于此卡上」. The held
card is a string in a prop while its crystals sit on the host
(`here_the_world.rs:148-179`); a second copy reads the same prop.

*Smallest alternative:* A2 (tile sentinel + host in a prop) lies in `tile_at`
and cannot express a chain; A3 (a second vector) needs a second uid space and
every scan written twice. A4 (keep the prop hold and add only a leave hook)
leaves the held card unobservable, unflippable and unable to follow its host.
*Class:* forced (`on`, `attach_card`, `detach_card`, `attached_cards`,
`attachment_host`).
*Fails without it:* Here the world's hold, the host cascade, ONE OF US's
「放置在该卡上」.

**8 — leave (rows 639, 645, 867, 875, 1074, 1252, 1474, 1488, 1514, 1515,
1689, 1695).** Twelve rows across four cards. The event does not exist, so
`summer_camp` reports its end only at the owner's turn start
(`summer_camp.rs:25-27`), `resolve` pays only where it was played
(`resolve.rs:31-34`), and `guerrilla`/`soyo_colors` never clear their tile
grant (`guerrilla.rs:236-239`, `soyo_colors.rs:103-105`).

*Smallest alternative:* a per-card scan at every kind — the four cards would
each need the same five-sample ladder, and a leave caused by another card
(e.g. a counterspell) is not any of those kinds.
*Class:* forced (`zoneLeft`/`zoneLeftAfter`, `prop::LEAVE_DEST`); forced-now
(`set_dest_unwatched`).
*Fails without it:* all four cards' leave timing, and the cascade's trigger.

**9 — order (rows 188, 994).** 「最早拥有的格子」. The card takes `owned.min()`
(`tomori_inner_shout.rs:113-140`), because nothing records acquisition.

*Smallest alternative:* the card writing its own list at play time — too
late, since every deed taken before the play is already unordered; a list
written by whichever card is in play is absent until that card is played and
goes stale on a sale.
*Class:* forced for the ledger (engine-side); demoted on the card side,
which needs only the read.
*Fails without it:* 灯's card places on the lowest board index, not the first
acquisition.

### 14.3 What cannot be split

The landing order in §11 already says which items compile alone. The
converse matters for necessity: some items cannot be *struck* without
striking another, because they share one fact.

| item | needs from | because |
|---|---|---|
| 2 | 7 | 「放置在该卡上」 is an attachment |
| 2 | 8 | the card must leave with its seat and still carry the pair |
| 7 | 8 | a host that goes must raise before its attachments move |
| 8 | 1, 3, 5 | `counted`, `wasted` and the flip ride the same payload |
| 5 | 3 | the deal is a hand arrival |
| 9 | — | independent, but only the ledger; the read is convenience |

Striking 7 while keeping 2 leaves ONE OF US's deeds in a prop; striking 8
while keeping 7 leaves attachments with no cascade trigger; striking 1/3/5
while keeping 8 makes the payload carry two of five fields and gives the
writer a shape it will have to break. Those are the couplings a reviewer
must keep in one head.

### 14.4 Surface, field by field

| surface | class | forced by | if removed |
|---|---|---|---|
| `Zone` | forced | 2, 5, 6, 7, 8 | the pair cannot be named |
| `ZoneChangeCause` | forced | 2, 8 | a listener cannot tell bankruptcy from a cost discard |
| `PlayOutcome` | forced | 4 | Returns counts negated plays as resolved |
| `OutCause` | forced | 2 | ONE OF US cannot tell bankruptcy from removal |
| `ZoneChange` payload | forced | 1, 3, 5, 6, 7, 8 | six separate field pairs and two payload shapes |
| `prop::LEAVE_DEST` | forced | 8 | the four cards keep a hard-coded default |
| `trigger::zone` | forced | all | the payload is unreachable |
| `trigger::play_outcome` / `out_cause` | forced | 4 / 2 | same |
| `zone_of`, `attachment_host`, `attached_cards`, `created_cards` | forced | 7, 8 | a full scan per question, and no `attached_cards` answer at all |
| `instance_owner` | forced | 2 | the owner is only a gate-side comparison (`actor == owner`) |
| `set_instance_owner` | forced | 2 | 「视为拥有」 has no writer |
| `set_created_by_at` | forced | 2 | `black_suits` keeps its interim `unplace_at` and the child leaks |
| `set_ineffective_at` | forced | 6 | MIRACLE keeps only a log line |
| `is_ineffective_at` | convenience | 6 | a card using a prop key instead |
| `rehome_card` | forced | 2 | A10's identity loss |
| `play_hand_card` | forced | 3 | A11's skipped pipeline |
| `set_dest_unwatched` | convenience | 3, 8 | every batch move pays a full hook set |
| `earliest_owned_tile` | forced | 9 | the card keeps `owned.min()` |
| `FieldCard.on` / `created_by` / `effective` | forced | 7 / 2 / 6 | the relation has no home |
| `owned_seq` / `next_owned_seq` | forced | 9 | no order |
| `set_dest(cause)`, `send_to_dest`, `transfer_to_dest`, `unplace_at`, `set_face_down_at`, `add_to_dest(counted)` | forced | 1, 5, 8 | an uncaused move reads as `Other`; a flip cannot say why |
| `HostRequest::{Leave,Flip,Attach,Detach,Rehome,PlayHand}` | forced | 1, 2, 3, 7, 8 | a plain hostfn cannot pause or prompt |
| kinds 113–119, 140 (`openingHandDealt`) | forced | 1, 3, 5, 6, 7, 8 | no window, no hook |

Every forced surface has a test in §10.1 that reads it. There is no field
without a reader and no test without a field.

### 14.5 The cost, priced against the workarounds

* **ABI 114.** Fifteen new imports and six re-signed signatures. The
  alternative is per-card workarounds: five cards each re-deriving a hand
  arrival, four each guessing a destination, two each writing their own
  borrowed-instance cleanup, and `same_sky` keeping a hand-rolled play. That
  is more code than the surface, in places no later group can fix centrally.
* **SAVE 17.** Three `#[serde(default)]` fields still move the version,
  because the attachment relation must survive a save for replay stability.
  The alternative — attachments reloaded from a prop — is exactly the state
  we are leaving.
* **No display text, no decision inside an import.** Every decision stays
  `ask_*` in a body; the requests carry no text.
* **No game word in the SDK.** The zone family is generic; the keys and sugar
  are `bandori_std::zones`.
* **Landing independence.** Items 1, 3, 4, 5, 6 and 9 land alone on the
  writer; 2, 7 and 8 land as one piece. Nothing needs a later group.

### 14.6 Why now, and not after another group

Three reasons, each checkable:

1. **Live, not prospective.** Every one of the nine items has a `TODO` at the
   site or a fallback §2 shows to be observably wrong — and two of them (ONE
   OF US's dead hand-over, 喵梦's dead draw) sit on heavily played cards.
2. **The writer is a precondition for later groups.** G09's build/pass hooks,
   G10's settle facts and G11's deck frames all move cards or read where a
   card is; today each would have to grow its own move helper, which is how
   `unplace_at`, `move_now` and `apply_dest` came to disagree.
3. **The order is cheapest here.** The cascade order (§7.5) is only cheap
   while there is one writer and no other caller; introducing attachments
   later would mean auditing every existing removal again.

### 14.7 What would falsify the necessity

* If the book printed no clause that reads an attachment, `on` and the
  cascade would be overkill (A4 would do). It prints three.
* If no leave clause needed to choose a destination, `prop::LEAVE_DEST` could
  go — four cards need exactly that, all of them with the leak today.
* If a hand arrival were always a draw, `counted` would be redundant — the
  tutor, the bounce and the deal are the counter-examples.
* If every play announcement already carried the negation, item 4 would be a
  release note rather than a field. Section §2.4 shows the fresh-trigger
  reason it does not.

None of these holds. The group is necessary, and each surface is the
minimum that makes its clause true.

## 15. Clause match

Every clause below is quoted verbatim from `docs/rulebook/cards.json`. The
last column says what in §5–§8 makes the sentence true and what it still
cannot express.

| card id | clause | satisfied by | residual |
|---|---|---|---|
| Mujica:（喵梦） | 每当此卡将从正面翻至背面时可选择失去2个火罐 | `cardFaceFlipped` (before, prompts) + counter write | none |
| Mujica:（喵梦） | 每当此卡将从背面翻至正面时此卡拥有者抽一张卡 | `cardFaceFlippedAfter` + subject pass (face-down included) | none |
| AG:ONE OF US | 将两张被指定地契放置在该卡上 | `attach_card` from `before_out` | the two `MarkRef`s become the attachments (item 7) |
| AG:ONE OF US | 并转移到存活方的游戏区 | `rehome_card` | none |
| AG:ONE OF US | 该方视为拥有此地契 | `set_instance_owner` | a real owner write, not a view (C-Q50(a)) |
| Sumimi:Here the world | 将那张卡背面朝上放置于此卡上 | `attach_card(host, card, Cause, true)` | none |
| Sumimi:Here the world | 当奇迹水晶数为0时，那名玩家将那张卡加入手牌 | `detach_card` + `add_to_dest(Hand)` | C-Q49 (own leave timing) stays open |
| MyGO:MIRACLE | 若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效 | `set_ineffective_at` before the destination | none |
| MyGO!!!!! band（3） | 每次你的卡在未生效的情况下进入弃牌堆时… | `discarded` + `wasted` | none |
| PPP:拍卖撤下来了 | 此卡加入手牌时… | `handChanged`, `counted = false` | none |
| PPP:Returns（1） | 每当一张[使用]的卡被[抵消]时… | `cardPlayed` + `play_outcome = Cancelled` | none |
| AG:朝同一片天空迈进 | 开局时抽到此卡洗回 | `openingHandDealt` (own-card, at the deal) | none |
| AG:朝同一片天空迈进 | 换牌后，若此卡在你的手牌中，则将此卡打出 | `HandChanged` (cause `Mulligan`) + `play_hand_card` | none |
| CRYCHIC:（灯）内心的呐喊 | 放置到你的乐队技能最早拥有的格子上 | `earliest_owned_tile` | none |
| CRYCHIC band（3） | 每当你的手牌数从4增加至5 | `handChanged` with the count | `counted` distinction (Q7) |
| Morfonica:夏日合宿 | 当此卡效果结束… | `zoneLeft` | none |
| Roselia:Resolve | 此卡[移除]时获得1000资金 | `zoneLeft` any cause | none |
| RAS:游击演出 | 使其对你视为live house格子 | `zoneLeftAfter` clears `colorFor` | none |
| MyGO:（soyo）混合的颜色 | 该格获得所有颜色 | `zoneLeftAfter` clears `ANY_COLOR` | none |
| HHW:黑色西服 | 该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果 | `created_by` child on the marker's owner | none |
| PP:（日菜） | 将本卡的效果改变为… | `set_created_by_at(instance, -1)` on the substitute | OPEN-substitution-effect stays |
| band skill, `Action::Drawn` fallbacks | 若…抽到 | `handChanged` + `counted` | none |
