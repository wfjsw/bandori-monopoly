// One commit-reveal nonce per room slot (`docs/FAIRNESS.md`).
//
// The client draws 256 bits with `crypto.getRandomValues` and posts them the
// moment it enters the room (create / join / rejoin) and again whenever the
// room shows a **new** commitment -- the slot the server rolls after every
// match. There is no window and no wait: a nonce that never arrives is simply
// absent from the derived seed.

import { api } from "../net/api";
import { fmtMsg } from "../i18n/msg";
import { toast } from "../ui/Toast";

/** Nonces already posted for `roomId`'s slot `commit` this page load. */
const sent = new Set<string>();

/** 32 random bytes as lower-case hex. */
export function randomNonce(): string {
  return Array.from(crypto.getRandomValues(new Uint8Array(32)), (x) =>
    x.toString(16).padStart(2, "0"),
  ).join("");
}

/**
 * Post a fresh nonce for the room's current slot. Exactly one attempt per
 * `(room, commit)` per page load; a failed attempt retries on the next room
 * update. `err.fair.closed` (the slot was already consumed by a match) is not
 * an error to the player -- their next chance comes with the next commit.
 */
export function submitRoomNonce(roomId: string, commit: string): void {
  if (!roomId || !commit) return;
  const key = `${roomId}:${commit}`;
  if (sent.has(key)) return;
  sent.add(key);
  void api.nonce(roomId, randomNonce()).then((res) => {
    if (res.ok) return;
    if (res.error?.k === "err.fair.closed") return;
    sent.delete(key);
    toast(fmtMsg(res.error ?? { k: "err.unknown_act" }), "error");
  });
}