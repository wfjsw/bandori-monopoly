// One-shot mount actions, named as such so the "why is this an effect?"
// question is answered at the call site. Everything else should be a more
// specific hook (subscription, timer, listener, resource).

import { useEffect } from "react";

/**
 * Run `fn` once when the component mounts (loading a screen, marking a screen
 * seen, kicking off a one-time fetch). The cleanup runs on unmount -- and on
 * React StrictMode's simulated remount, so `fn` must tolerate being torn down
 * and run again in development. Prefer a more semantic hook when one fits;
 * this is the last resort.
 */
export function useMountEffect(fn: () => void | (() => void)): void {
  useEffect(() => fn(), []);
}

/**
 * Close an overlay as soon as `shouldClose` holds (the prompt was answered,
 * the turn moved on, the deck was picked). One contract for every
 * self-closing modal in the game.
 */
export function useCloseWhen(shouldClose: boolean, close: () => void): void {
  useEffect(() => {
    if (shouldClose) close();
  }, [shouldClose, close]);
}