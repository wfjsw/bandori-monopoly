// The global `autoBid` setting: 托管 bids by default, and remembers "off".

import { strict as assert } from "node:assert";
import { test } from "node:test";

import { autoBids, setAutoBids } from "./autoBid.ts";

test("autoBid: on unless the player turned it off", () => {
  setAutoBids(true);
  assert.equal(autoBids(), true);
  setAutoBids(false);
  assert.equal(autoBids(), false, "asks left to the player");
  setAutoBids(true);
  assert.equal(autoBids(), true);
});
