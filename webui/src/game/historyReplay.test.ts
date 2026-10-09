// Unit tests for the history -> replay link. Run with:
//   node --test webui/src/game/historyReplay.test.ts

import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  recordMinutes,
  resolveHistoryReplay,
  wallMinutes,
  type HistoryReplayRow,
} from "./historyReplay.ts";
import type { RecordHeader, ReplayEntry } from "./record.ts";

// ---------------------------------------------------------------- fixtures

function header(over: Partial<RecordHeader> = {}): RecordHeader {
  return {
    engine: {
      format: 1,
      save_version: 1,
      abi: 1,
      ruleset_sha256: "stub",
      data_sha256: "stub",
      engine: "game-core",
      build: "test",
    },
    mode: 0,
    step: 0.05,
    origin: "Solo",
    created: "2026-10-07 12:00",
    seats: [
      { member: 1, player: "me", bot: false, mentality: "standard", character: "户山香澄", rank: 1, score: 300 },
      { member: 2, player: "bot", bot: true, mentality: "standard", character: "花园多惠", rank: 2, score: 100 },
    ],
    partial: false,
    ended: true,
    reason: "settle",
    rounds: 3,
    total_ticks: "120",
    ...over,
  };
}

function entry(id: string, over: Partial<RecordHeader> = {}, savedAt = 1000): ReplayEntry {
  return { id, header: header(over), savedAt };
}

function row(over: Partial<HistoryReplayRow> = {}): HistoryReplayRow {
  return { time: "2026-10-07 12:00", mode: 0, rank: 1, character: "户山香澄", ...over };
}

// ---------------------------------------------------------------- stamps

test("wallMinutes reads the profile's yyyy-MM-dd HH:mm stamp", () => {
  assert.equal(wallMinutes("2026-10-07 12:00"), Date.UTC(2026, 9, 7, 12, 0) / 60_000);
  assert.equal(wallMinutes("2026-10-07T12:00"), Date.UTC(2026, 9, 7, 12, 0) / 60_000);
  assert.equal(wallMinutes("not a time"), null);
});

test("recordMinutes takes a local stamp as-is and converts an ISO instant", () => {
  assert.equal(recordMinutes("2026-10-07 12:00"), wallMinutes("2026-10-07 12:00"));
  // An ISO `created` is an instant: compare it as the machine's wall clock,
  // which is how the history row was stamped.
  const iso = new Date(2026, 9, 7, 12, 0).toISOString();
  assert.equal(recordMinutes(iso), wallMinutes("2026-10-07 12:00"));
  assert.equal(recordMinutes("??"), null);
});

// ---------------------------------------------------------------- resolve

test("the row's own id is authoritative when the record is still stored", () => {
  const entries = [entry("a", { created: "2026-10-07 12:00" }), entry("b", { created: "2026-10-07 12:00" })];
  assert.deepEqual(resolveHistoryReplay(row({ replayId: "b" }), entries), { kind: "linked", id: "b" });
});

test("an id whose record was evicted is 'gone', not a silent re-match", () => {
  const entries = [entry("a", { created: "2026-10-07 12:00" })];
  assert.deepEqual(resolveHistoryReplay(row({ replayId: "z" }), entries), { kind: "gone", id: "z" });
});

test("a row without an id matches on time + character + rank + mode", () => {
  const entries = [entry("mine", { created: "2026-10-07 12:00" }), entry("other", { created: "2026-10-07 12:00", seats: header().seats.map((s) => ({ ...s, character: "花园多惠" })) })];
  assert.deepEqual(resolveHistoryReplay(row(), entries), { kind: "matched", id: "mine" });
});

test("the player's seat must agree -- bots never match, nor does a wrong rank", () => {
  const botSeat = header().seats.map((s) => ({ ...s, bot: true }));
  assert.equal(
    resolveHistoryReplay(row(), [entry("b", { seats: botSeat })]).kind,
    "none",
    "a bot seat is not the player's match",
  );
  assert.equal(
    resolveHistoryReplay(row({ rank: 2 }), [entry("b")]).kind,
    "none",
    "a different final rank is a different match",
  );
});

test("mode and the one-minute window both filter", () => {
  assert.equal(resolveHistoryReplay(row({ mode: 1 }), [entry("b")]).kind, "none");
  // The export stamp can land a minute either side of the history stamp.
  assert.deepEqual(resolveHistoryReplay(row(), [entry("t1", { created: "2026-10-07 12:01" })]), {
    kind: "matched",
    id: "t1",
  });
  assert.deepEqual(resolveHistoryReplay(row(), [entry("t1", { created: "2026-10-07 11:59" })]), {
    kind: "matched",
    id: "t1",
  });
  assert.equal(resolveHistoryReplay(row(), [entry("t2", { created: "2026-10-07 12:02" })]).kind, "none");
  assert.equal(resolveHistoryReplay(row(), [entry("t3", { created: "2026-10-06 12:00" })]).kind, "none");
});

test("several candidates pick the closest finish, then the newest save", () => {
  const entries = [
    entry("far", { created: "2026-10-07 12:01" }, 5),
    entry("near-old", { created: "2026-10-07 12:00" }, 1),
    entry("near-new", { created: "2026-10-07 12:00" }, 9),
  ];
  assert.deepEqual(resolveHistoryReplay(row(), entries), { kind: "matched", id: "near-new" });
});

test("an online match with no local record resolves to none", () => {
  // Casual/ranked rows are written the same way; the client keeps no local
  // `.bdrec` for them (the room offers a download instead).
  const entries = [entry("solo", { mode: 0, created: "2026-10-07 12:00" })];
  assert.equal(resolveHistoryReplay(row({ mode: 1 }), entries).kind, "none");
});