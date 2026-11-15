// Run with: node --test webui/src/console/find.test.ts
// The console's own find: every match in the whole snapshot, marked by range.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { findHits, highlightSegments } from "./find.ts";

test("matches are case-insensitive and carry line + range", () => {
  assert.deepEqual(findHits(["Alpha beta", "beta BETA", "none"], "beta").hits, [
    { line: 0, start: 6, end: 10 },
    { line: 1, start: 0, end: 4 },
    { line: 1, start: 5, end: 9 },
  ]);
});

test("an empty query finds nothing", () => {
  assert.deepEqual(findHits(["anything"], ""), { hits: [], truncated: false });
});

test("the limit stops the scan and reports truncation", () => {
  const result = findHits(["aaaa", "aaaa"], "a", 3);
  assert.equal(result.hits.length, 3);
  assert.equal(result.truncated, true);
});

test("highlights split the line around every hit", () => {
  const spans = [{ line: 1, start: 2, end: 4 }, { line: 1, start: 6, end: 8 }];
  assert.deepEqual(highlightSegments("xxABCDyy", spans), [
    { text: "xx", hit: false, current: false },
    { text: "AB", hit: true, current: false },
    { text: "CD", hit: false, current: false },
    { text: "yy", hit: true, current: false },
  ]);
});

test("only the cursor hit is current; an empty line stays whole", () => {
  const spans = [{ line: 0, start: 0, end: 2 }];
  assert.deepEqual(highlightSegments("ab", spans, spans[0]), [{ text: "ab", hit: true, current: true }]);
  assert.deepEqual(highlightSegments("", []), []);
});
