# Held-token `markerChanged` event (HAG-01)

A **held token** is a player counter bound to the instance of the card that
created it (`MARKERS.md`; `bind_held` stamps the creating uid). Until this
item the only way a rule could react to one being written was to re-read the
count at its own tick: a hand-written sweep after every write site, or a
turn-end catch-all. Every such rule duplicates the write list, and a new
write verb (the fire kernel's) reaches none of the copies.

HAG-01 adds the missing observation point: an engine `markerChanged` hook
raised after **every** committed bound held write. The reference clause is
北泽育美:

> 其他角色拥有4个可乐饼时自动移除所有可乐饼停留一回合。

A standing 「has N」 clause hangs off the event, filtered by name and holder,
instead of off each writer.

## Kind

| | |
|---|---|
| kind | `markerChanged` |
| namespace | engine, id **143** (appended; ids are wire-stable) |
| shape | `On::Action(Action::MarkerChanged(condition, guard, body))` |
| hook-only | yes -- no hand-counteraction window |
| `ActionKind` | `ActionKind::MarkerChanged` = `Self::engine(143)` |
| CEL spelling | `trig::MarkerChanged` / `kind == 'markerChanged'` |

`TriggerKind::MarkerChanged` is the same engine id. Append-only: nothing is
renumbered, so a recorded match keeps its meaning.

## Payload

| field | meaning |
|---|---|
| `player_id` | the token **holder** (the write's subject seat) |
| `value` | the signed delta the write applied |
| `name` | the counter name |
| `instance` | the creating instance uid, `-1` when unknown |
| `by_card` | the seat whose drive caused the write |

The count *after* the write is not carried -- a body reads it with
`held_count`. A body's own `player_id` is the card owner (the field walk's
seat), not the actor; the holder is `trigger::player_id()`. The condition's
`actor` is that same raise seat and `owner` is the card owner, so
`actor != owner` filters "another player's write".

## Raise sites

One raise is logged per bound write, inside the writing run, and raised by
`commit_after` **after** the write committed:

| verb | logged |
|---|---|
| `bind_held` | yes (`add_held_by_name` is a bind + add) |
| `add_held_by_name` | yes, `(holder, got, ...)` -- `got` is capped |
| `set_held` | yes, `(holder, v - was, ...)`, including delta 0 (a re-stamp) |
| `spend_marker` | yes on a successful spend, `(holder, -n, ...)` |
| `move_units` | both player legs: `(from, -got, ...)` and `(to, got, ...)` |

The fire kernel reaches `ctx::set_held` / `ctx::spend_marker`, so pot writes
raise too.

The raise sits in the normal commit net, next to `marker_spent_log`, and
runs on the accumulated `cur` world exactly like every other post-commit
event.

## Recursion guard

A handler that writes held tokens would otherwise chain forever. `PlayCtx`
carries `marker_raise_depth`; `commit_after` raises only at depth 0 and
bumps the depth around the dispatch. A nested drive's own `markerChanged`
is therefore suppressed, and an unconditional writer cannot loop.

The consequence is that **a write under an open dispatch is not re-raised**.
A rule that needs a catch-all for those keeps its own tick -- hagumi's
turn-end sweep stays as that backstop and is cheap (a count read per other
seat).

## Card usage (hagumi)

```rust
On::Action(Action::MarkerChanged(
    "actor != owner && counter_is('可乐饼')",
    None,
    on_marker_changed,
)),
```

`on_marker_changed` sweeps `trigger::player_id()` (the holder). The manual
sweep re-runs at the pass write sites are gone; the turn-end hook remains
for depth-guarded writes.

## Tests

`rules/tests/tests/ab_tokens_trap.rs`:

* a transfer to another player trips the sweep exactly once (recipient
  emptied, one [停留] layer, one outcome line);
* the owner's own gain to 4 does not sweep.

Fixtures: `TEST:hagumiGive` (move the whole 可乐饼 pool to the other seat),
`TEST:hagumiGain` (own +4).

## Save / record impact

None beyond the log: no new `MatchState` field, no `SAVE_VERSION` or
`ABI_VERSION` change. A pre-143 ruleset simply never declares the kind and
nothing raises.

## Open questions

* `set_held` at delta 0 raises. That keeps cap restatements (`fire::declare`)
  observable, but the silent-body debug check counts only a nonzero delta
  and skips `deckAtGameStart` -- a setup write is not a clause event.
