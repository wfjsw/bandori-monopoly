// Global 托管 auction policy (`docs/BOT-DRIVE.md` §2): does auto mode answer
// auctions, or are they left to the player?
//
// A settings-store key like the solo search cap (`session.ts` `botSoloCapMs`),
// not a profile field: it survives a refresh without inflating the profile,
// and the Settings popup edits it. Its own module so it can be unit-tested
// without pulling the wasm-backed session in.

import { kv } from "../core/kv.ts";

const prefs = kv("prefs");
const AUTO_BID_KEY = "autoBid";

/** On unless the player turned it off. */
export function autoBids(): boolean {
  return prefs.get(AUTO_BID_KEY) !== "off";
}

export function setAutoBids(on: boolean): void {
  prefs.set(AUTO_BID_KEY, on ? "on" : "off");
}
