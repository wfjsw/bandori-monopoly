# event_pairs.json — mapping notes

One entry per `### ` title in `events.md`'s `## Cards` section (28), keyed by
the local rule id, valued by the exact library title.

- Local ids are `event:<name>`; the `[衍生]` marker exists only on the library
  side, so it is stripped from the key and kept in the value (`event:冲榜` ->
  `[衍生]冲榜`). The metric therefore compares normalized titles with the
  derived flag ignored, and `event_full` equals `event_synced` (events carry
  no tags).
- `display.json` keys `events` by the **bare** event name (`对邦`), not the
  `event:` rule id; the metric strips the prefix for lookups and re-adds it
  for `event_unpaired_local`.
- Checked at write time: ids unique, 0 library titles unmapped, 0 display
  events without a pair.
