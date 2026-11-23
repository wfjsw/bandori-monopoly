// Should the room screen follow the session into the match?
//
// The room is reached two ways: a match actually starts while the table waits
// (the seats enter), and the results screen sends a player back to the room
// afterwards. Only the first pull belongs to this predicate. `playing` may
// still be true for the tick between the seal and the server's reset, so the
// frame's own phase decides: a finished match is not entered again.
//
// A seat that forfeited / went bankrupt left the match screen on purpose and
// is not bounced back. A spectator is `out` by construction (`playerId` -1)
// but has no seat to abandon -- they came to watch, so they go in.

export type RoomScene = "select" | "deck" | "board" | null;

export function shouldEnterMatch(o: {
  /** `matchScene(view)` of the latest frame. */
  scene: RoomScene;
  /** `view.state.phase`. */
  phase: string | undefined;
  /** `session.room.playing` -- the server's own flag. */
  playing: boolean;
  spectator: boolean;
  /** `model(view).out`: the seat forfeited or went bankrupt. */
  out: boolean;
}): boolean {
  if (!o.scene || o.phase === "ended" || !o.playing) return false;
  return o.spectator || !o.out;
}
