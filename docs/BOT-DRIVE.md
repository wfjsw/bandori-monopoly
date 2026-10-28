# Bots drive like a client

Status: design + staging (2026-10-28). Supersedes the "policy lives in
`game-core`" arrangement (`engine/ai.rs`, `Match::bot_action`,
`Cx::ai_answer_for_entry`, `fill_ai`).

Landed: step 1 (`bot-core/src/policy.rs`, `Match::bot_cx`,
`MatchState::played_count`, the published `Cx` helpers, `policy::act_for_view`,
plus `rules/tests/tests/policy_parity.rs` pinning the module to the engine's
old answers) and step 2 (server probe on every bot seat, the page driving every
bot seat, `tick_ask` no longer answering one). **The engine still runs its own
copy for the seats that are not bots** -- a human under 托管 / timed out / left,
which `ai` marks -- and for setup, so the two copies exist side by side until
step 3 deletes the engine one; the parity suite is what keeps that honest.

## 1. The rule

**A bot seat is a client.** Every bot command is produced the way a human's
is: somebody reads `view(member)`, decides, and sends `act(member, cmd)`. The
engine never decides for a seat; it only applies the commands it is given.

Consequences, all deliberate:

* `game-core` carries **no policy**. It keeps the seat-view gates
  (`playable` / `estCost` / `skills`), `SeatView` accessors, `Match::act`,
  `Match::tick` and the public `Cx` queries. `engine/ai.rs`'s decision code,
  `Match::bot_action`, `compute_ai_answer` and `ai_answer_for_entry` leave.
* The **policy is `bot-core`'s** (`policy.rs`), next to the determinizer, the
  extras and the search that already consume it. It takes `&mut Cx` (a
  throwaway world: a determinized fork for a view, a clone for a live match)
  and returns a `NetMessage` / an answer index.
* Nobody decides on hidden information the client cannot see (§1.1 of
  `BOT.md` is the audit; the framing stays authoritative).
* `player.ai` is a **human** seat under the machine now -- 托管, a clock
  time-out, or a leave -- never a bot one (a bot starts with `ai` off). It
  keeps its clock / afk semantics, the `board.afk` tag and the replay seat
  kind. For the time being it still makes the engine act; step 3 moves those
  seats onto the same drivers too.
* `Ask::ai` stops being the default answer. The driver answers prompts like a
  human; a timed-out seat is driven by the same driver.
* Solo stays self-contained: the page drives its bot seats through the
  `bot-glue` worker (`driveSeat`, already used for 进阶), exactly as the
  server drives online seats through `bot-service`. A recording is unaffected
  -- the commands are what get recorded.

## 2. Drivers

| host | driver | note |
|---|---|---|
| online (`server`) | the existing probe loop, widened from `Advanced` seats to every bot seat; every seat whose clock expired joins it in step 3 | the view is the exact client frame; a standard / chaos seat answers with the local policy, an `Advanced` one searches |
| solo (`webui`) | `driveSeat` for every bot seat, standard/chaos included | the worker's `decide`; no clock in solo |
| `rules/tests` | a `Table`-side pump that decides for bot seats on the surfaces `tick_until` would have let the engine answer | host-only, `rules-spec` may depend on `bot-core` |
| `game-core` tests | `common::engine_bots` turns the engine's in-flight copy back on for a harness that only ticks; a record test instead pumps through `RecordedMatch::act` so the commands land in the record | the copy goes away with step 3, the record pump stays |
| `bot-core::sim` rollouts | direct `policy` calls on the fork | already the shape (`heuristic_message_with`) |

Fallbacks: the service being absent or slow is the *host's* problem. The
server keeps its local `bot-core` policy answer; the page falls back to
`autopilot.ts`'s table; the harness never falls back.

## 3. Staging (each step green on its own)

1. **Policy out of `game-core`.** Create `bot-core/src/policy.rs` with the
   turn and prompt decisions moved verbatim; add `Match::bot_cx()` and
   `MatchState::played_count`; `view_extra` loses `auto_action`; every caller
   (`extras`, `sim`, the server fallback, `web-glue`'s frame, the page) now
   goes through `bot-core`. **Done.**
2. **Drive online.** `claim_advanced` → every bot seat (`claim_bots`); the
   page drives every bot seat too; `tick_ask` stops answering bot seats.
   **Done**, except that timed-out human seats still ride the engine copy.
3. **Delete the engine drive.** `Routine::Ai`, `Cx::ai_step`, `fill_ai`,
   `Ask::ai` / `ai_picked` / `ai_at`, the `ai_*` choice fns, `bot_action`,
   `compute_ai_answer`; the reset seats above join the drivers; setup's
   remaining policy bits (`is_chaos`, the deck book) move to `bot-core` or stay
   as plain engine data.
4. **Harness + docs.** `Table` pump, `BOT.md` / `SERVER.md` rewrites, the
   `bot_pass_reward` and `compute_ai_answer` suites ported to `bot-core`.

## 4. What must not regress

* A solo match still advances with zero host help beyond the page it already
  has (the driver is part of the page).
* `determinize` + extras stay the only view→world path; no new hidden-state
  reads.
* The replay format does not change: commands are recorded, and a replay never
  asks a bot anything.
