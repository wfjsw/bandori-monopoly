# Bots drive like a client

Status: landed (2026-10-28). Supersedes the "policy lives in `game-core`"
arrangement (`engine/ai.rs`'s turn code, `Match::bot_action`,
`Cx::ai_step`, the `tick_live` scheduler).

All four staging steps are in. The decision code lives in
`bot-core/src/policy.rs`; every seat that has no human actively answering it --
a bot, a 托管 human, a timed-out or gone one -- is driven over the human
interface by its host. What the engine keeps is *simulation prefill*, not
decision: the prompt's default answer stamped at prompt-open (`Ask::ai` /
`ai_picked` / `worth`, `fill_ai`, `ai_answer_for_entry`) and the `aiAnswer`
frame hint the prompt force path and the search's priors read.

## 1. The rule

**A bot seat is a client.** Every bot command is produced the way a human's
is: somebody reads `view(member)`, decides, and sends `act(member, cmd)`. The
engine never decides for a seat; it only applies the commands it is given.

Consequences, all deliberate:

* `game-core` carries **no policy**. It keeps the seat-view gates
  (`playable` / `estCost` / `skills`), `SeatView` accessors, `Match::act`,
  `Match::tick` and the public `Cx` queries. `ai.rs`'s turn code
  (`bot_turn_action`, `ai_card_choice` / `ai_skill_choice` /
  `ai_redeem_choice`, `ai_step`), `Match::bot_action` and `compute_ai_answer`
  are gone.
* The **policy is `bot-core`'s** (`policy.rs`), next to the determinizer, the
  extras and the search that already consume it. It takes `&mut Cx` (a
  throwaway world: a determinized fork for a view, a clone for a live match)
  and returns a `NetMessage` / an answer index.
* Nobody decides on hidden information the client cannot see (§1.1 of
  `BOT.md` is the audit; the framing stays authoritative).
* `player.ai` marks a **human** seat under machine control -- a clock
  time-out, or a leave -- never a bot one (a bot starts with `ai` off). It
  keeps its clock / afk semantics and the `board.afk` tag, and those seats are
  driven by the same driver as a bot's.
* `Ask::ai` / `ai_picked` / `worth` stay, but only as the **prefill**: the
  answer a hostless seat (or a driver that went quiet) is forced onto, and the
  `aiAnswer` hint the search ranks with. Nobody schedules them any more.
* Solo stays self-contained: the page drives its bot seats through the
  `bot-glue` worker (`driveSeat`), exactly as the server drives online seats
  through `bot-service`. A recording is unaffected -- the commands are what
  get recorded.

## 2. Drivers

The match publishes **`MatchState::drive`**: the members a driver must move
right now -- every bot seat, every machine-held human, and the acting seat once
its own clock is spent. Empty in Solo (the page is the host there) and before
play unless a prompt is open. Hosts probe exactly that list.

| host | driver | note |
|---|---|---|
| online (`server`) | `claim_drive` -> `spawn_probe` for every member on the drive list | the view is the exact client frame; a standard / chaos seat answers with the local policy, an `Advanced` one searches |
| solo (`webui`) | `botSeats` -> `driveSeat` for every bot seat, standard/chaos included | the worker's `act`; no clock in solo |
| `rules/tests`, `rules-spec` | `Table` pumps `bot-core::sim::pump_bots` inside `settle` / `tick_until` | host-only; `rules-spec` depends on `bot-core` |
| `game-core` tests | `common::bot_msg` / `common::drive` over `drive_members` (the drive list, or the bots in Solo, where the list is empty), reached through the `common::Pump` trait's `pump_tick`; a record test pumps through `RecordedMatch::act` so the commands land in the record | |
| `bot-core::sim` rollouts | `policy::act_for` on the fork (`pump_bots`, `next_decision`) | |

Fallbacks: the service being absent or slow is the *host's* problem. The
server keeps its local `bot-core` policy answer; the page falls back to
`autopilot.ts`'s table; the harness never falls back. A driver that stops
answering entirely can only stall its own seat: a prompt is forced onto its
prefill once the seat's window is spent, and a turn is re-claimed by the
server through the drive list.

## 3. Staging (each step green on its own)

1. **Policy out of `game-core`.** Create `bot-core/src/policy.rs` with the
   turn and prompt decisions moved verbatim; add `Match::bot_cx()` and
   `MatchState::played_count`; `view_extra` loses `auto_action`; every caller
   (`extras`, `sim`, the server fallback, `web-glue`'s frame, the page) now
   goes through `bot-core`. **Done.**
2. **Drive online.** `claim_advanced` → every bot seat; the page drives every
   bot seat too; `tick_ask` stops answering bot seats. **Done.**
3. **Delete the engine drive.** `Routine::Ai`, `Cx::ai_step`, the `ai_*` turn
   choice fns, `bot_action`, `compute_ai_answer`, the `tick_live` answer
   schedule (`ai_at`, instant mulligan fill, auction bids); `MatchState::drive`
   and the server claim carry the reset seats instead; the prompt force path
   keeps the prefill. **Done.**
4. **Harness + docs.** Pump in every harness, `BOT.md` / `SERVER.md` /
   `REPLAY.md` rewrites, the parity suite re-pointed at the view entry.
   **Done.**

## 4. What must not regress

* A solo match still advances with zero host help beyond the page it already
  has (the driver is part of the page).
* `determinize` + extras stay the only view→world path; no new hidden-state
  reads.
* The replay format does not change: commands are recorded (format 4), and a
  format-4 replay never asks a bot anything. A format-3-and-older log carries
  no bot commands -- those were engine decisions -- so its replay re-arms the
  seats and runs a host-installed policy per tick
  (`game_core::record::set_machine_pump`; the browser glue and the
  `rules-worker` replay op both install `bot_core::sim::pump_bots`). Without a
  pump it replays only up to the first bot decision.
* Nothing hangs when a driver goes quiet: the prompt force (prefill) and the
  server's clock claim are the two backstops. A bot seat has no clock, so its
  own commands must be ones the engine accepts — the policy picks roll vs end
  by the engine's gates (`can_roll_here` / `can_end_here`, mirrored in
  `policy::turn_action`), never by the `skip_move` latch.
* Every frame carries the turn's **public play history** (`played` /
  `cardsPlayed`). The determinized fork is the policy's only world, so without
  it the max-plays cap and the skill de-dup see an empty turn and a machine
  seat re-presses the same skill forever.
* A machine seat answers in milliseconds and so spends no recovery pool on a
  prompt countdown (`tick_live` skips it, `grant_auction_extend` too). Letting
  it burn the pool would shrink its later windows and its turn clock.
