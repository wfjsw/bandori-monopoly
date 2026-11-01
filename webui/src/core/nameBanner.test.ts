// Run with: node --test webui/src/core/nameBanner.test.ts
// Banner geometry: the plate must always frame its text without clipping it.

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { BANNER_HEIGHT, TITLE, bannerWidth, dotZone, pill, starPath, titleSize } from "./nameBanner.ts";

test("a short name still gets the plate's minimum length", () => {
  assert.equal(bannerWidth("奏"), 430);
  assert.equal(bannerWidth(""), 430);
});

test("a longer name gets a longer banner", () => {
  const short = bannerWidth("松原花音");
  const long = bannerWidth("高松灯（CRYCHIC）");
  const longer = bannerWidth("高松灯（CRYCHIC）Extra Long Name");
  assert.ok(short >= 430, "never below the floor");
  assert.ok(long > short, "a variant name widens the plate");
  assert.ok(longer > long, "and a longer one widens it further");
});

test("the title shrinks only when the slot cannot hold it", () => {
  assert.equal(titleSize("松原花音", 430), TITLE.size, "the design width fits at full size");
  assert.equal(titleSize("松原花音", bannerWidth("松原花音")), TITLE.size);
  assert.ok(titleSize("高松灯（CRYCHIC）", 430) < TITLE.size, "a tight slot shrinks the letters");
  assert.ok(titleSize("高松灯（CRYCHIC）", 100) >= 20, "but never past the floor");
});

test("punctuation is estimated narrower than an ideograph", () => {
  assert.ok(bannerWidth("松原花音（") < bannerWidth("松原花音灯"), "same length, one is a bracket");
});

test("the pill and its dot zone stay inside the banner", () => {
  for (const width of [430, 675, 1200]) {
    const p = pill(width);
    const d = dotZone(width, p);
    assert.equal(p.y + p.height, BANNER_HEIGHT, "the pill sits on the banner floor");
    assert.equal(p.mid, 88, "the stars sit on the pill centre");
    assert.ok(p.innerY > p.y && p.innerY + p.innerHeight < p.y + p.height, "the body is inset by the rim");
    assert.ok(d.cutBottom < width && d.cutTop < width, "the dot zone ends before the right edge");
    assert.ok(d.cutTop > d.cutBottom, "and slants in towards the top");
  }
});

test("a star is a closed ten-corner path around its centre", () => {
  const d = starPath(50, 50, 10);
  assert.ok(d.startsWith("M"), "starts with a move");
  assert.ok(d.endsWith("Z"), "is closed");
  assert.equal(d.split("Q").length - 1, 10, "one rounded corner per corner");
  for (const point of d.matchAll(/(-?[\d.]+),(-?[\d.]+)/g)) {
    const [x, y] = [Number(point[1]), Number(point[2])];
    assert.ok(Math.abs(x - 50) <= 10.01 && Math.abs(y - 50) <= 10.01, "stays within the radius");
  }
});
