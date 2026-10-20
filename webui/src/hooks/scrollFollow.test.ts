// Run with: node --test webui/src/hooks/scrollFollow.test.ts
// The pure tail-tracking state machine behind `useScrollFollow` (match log,
// console output): when does new content re-pin the scroller, and what does a
// `scroll` event do to that decision.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { atScrollEnd, END_SLACK, scrollFollowStep, shouldPinToBottom, type ScrollFollow } from "./pure.ts";

const ev = (scrollTop: number, gap: number, visible = true) => ({ scrollTop, gap, visible });

test("a gap still inside the slack counts as the end", () => {
  assert.equal(atScrollEnd(0), true);
  assert.equal(atScrollEnd(END_SLACK), true, "the boundary itself is at the end");
  assert.equal(atScrollEnd(END_SLACK + 1), false);
  // A wheel notch / a trackpad rest / a fractional scrollTop rounding.
  assert.equal(atScrollEnd(3.5), true);
  // One log line arriving mid-gesture (14px / 1.55 ≈ 22px).
  assert.equal(atScrollEnd(22), true);
});

test("the slack is about a line and a half, not a whole screen", () => {
  assert.ok(END_SLACK >= 22, "covers one line landing on the gesture");
  assert.ok(END_SLACK <= 48, "still opts out when the user scrolls up to read");
});

test("a user scroll re-flags follow when they reach the end", () => {
  const up: ScrollFollow = { atEnd: false, pinned: null };
  assert.deepEqual(scrollFollowStep(up, ev(400, 200)), { atEnd: false, pinned: null });
  assert.deepEqual(scrollFollowStep(up, ev(600, 0)), { atEnd: true, pinned: null });
});

test("a few px short of the bottom still resumes follow", () => {
  // The reported bug: scroll the log to the bottom, auto scroll stays dead.
  const up: ScrollFollow = { atEnd: false, pinned: null };
  assert.equal(scrollFollowStep(up, ev(597, 3)).atEnd, true);
  assert.equal(scrollFollowStep(up, ev(597, END_SLACK)).atEnd, true);
});

test("the echo of our own pin is not the user scrolling away", () => {
  // We pin to the old bottom; the echo is dispatched later.
  const pinned: ScrollFollow = { atEnd: true, pinned: 600 };
  assert.deepEqual(scrollFollowStep(pinned, ev(600, 0)), { atEnd: true, pinned: 600 });
});

test("a pin echo that lands after more content arrived keeps follow alive", () => {
  // THE race: pin to the bottom of the content as it was, then another line
  // renders before the echo. The echo measures the gap of that new line -- if
  // that gap is taken as a user scroll, follow dies while they sit at the
  // bottom and the next line never pins.
  const pinned: ScrollFollow = { atEnd: true, pinned: 600 };
  const echo = scrollFollowStep(pinned, ev(600, 22));
  assert.equal(echo.atEnd, true, "the new line must not read as a scroll-up");
  assert.equal(echo.pinned, 600);
});

test("a real scroll up clears both the flag and the pin mark", () => {
  const pinned: ScrollFollow = { atEnd: true, pinned: 600 };
  assert.deepEqual(scrollFollowStep(pinned, ev(500, 122)), { atEnd: false, pinned: null });
});

test("a hidden pane's events say nothing", () => {
  const s: ScrollFollow = { atEnd: true, pinned: 600 };
  assert.deepEqual(scrollFollowStep(s, ev(0, 0, false)), s);
  const up: ScrollFollow = { atEnd: false, pinned: null };
  assert.deepEqual(scrollFollowStep(up, ev(0, 0, false)), up);
});

test("burst of lines: follow survives every pin echo and re-pins", () => {
  // Walk the reported failure end to end: several log lines land in a row
  // while the user is at the bottom. Each pin's echo arrives after the next
  // line is already in the DOM.
  let s: ScrollFollow = { atEnd: true, pinned: null };
  let scrollTop = 0;
  let gap = 0;
  const pin = () => {
    // `useScrollFollow`'s layout effect: pin to the bottom and remember where.
    scrollTop = scrollTop + gap; // == scrollHeight - clientHeight
    gap = 0;
    s = { atEnd: true, pinned: scrollTop };
    return scrollTop;
  };
  const appendLine = (px: number) => {
    gap += px;
  };

  // Line 1 arrives at the end: pin to 0 (short content).
  pin();
  // Line 2 renders before the echo of that pin, then the echo fires.
  appendLine(22);
  s = scrollFollowStep(s, ev(scrollTop, gap));
  assert.equal(s.atEnd, true);
  pin();
  // Line 3, same interleaving.
  appendLine(22);
  s = scrollFollowStep(s, ev(scrollTop, gap));
  assert.equal(s.atEnd, true);
  pin();
  assert.equal(gap, 0, "left pinned to the newest line");
});

test("scrolling up mid-burst stops the follow, scrolling back resumes it", () => {
  let s: ScrollFollow = { atEnd: true, pinned: 800 };
  s = scrollFollowStep(s, ev(700, 100));
  assert.deepEqual(s, { atEnd: false, pinned: null }, "a real scroll up clears the pin mark");
  s = scrollFollowStep(s, ev(800, 0));
  assert.deepEqual(s, { atEnd: true, pinned: null }, "back at the bottom resumes follow");
});

test("new content re-pins at the end, and near it -- never while reading", () => {
  assert.equal(shouldPinToBottom({ atEnd: true, pinned: 900 }, 22, true), true);
  assert.equal(shouldPinToBottom({ atEnd: true, pinned: 900 }, 500, true), true);
  // The tail landed mid-gesture: the last `scroll` saw a bigger gap than the
  // one they stopped on. Catching up by a line or so still follows.
  assert.equal(shouldPinToBottom({ atEnd: false, pinned: null }, 22, true), true);
  assert.equal(shouldPinToBottom({ atEnd: false, pinned: null }, END_SLACK, true), true);
  // Scrolled up to read: leave them alone.
  assert.equal(shouldPinToBottom({ atEnd: false, pinned: null }, END_SLACK + 1, true), false);
  assert.equal(shouldPinToBottom({ atEnd: false, pinned: null }, 200, true), false);
  // A hidden pane's gap is unknown (not 0): a follow the user opted out of
  // must not be re-armed just because the log tab was behind the card tab.
  assert.equal(shouldPinToBottom({ atEnd: false, pinned: null }, Infinity, true), false);
  assert.equal(shouldPinToBottom({ atEnd: true, pinned: 900 }, Infinity, true), true);
  // Off (the console's follow checkbox) never pins.
  assert.equal(shouldPinToBottom({ atEnd: true, pinned: 900 }, 0, false), false);
});