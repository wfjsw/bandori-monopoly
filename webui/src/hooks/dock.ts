// The docked card rows (the match hand at the screen's bottom edge, the
// cards-in-play row at the top of the centre column). One raise / retract
// contract for both, so the two docks cannot drift apart.

import { useRef, useState } from "react";
import { useCloseWhen, useMountEffect } from "./mount.ts";

export interface DockRaise {
  raised: boolean;
  setRaised: (v: boolean) => void;
  raise: () => void;
  retract: () => void;
  /** Spread on the dock: enter / focus raise, leave / blur retract. */
  hover: {
    onMouseEnter: () => void;
    onMouseLeave: () => void;
    onFocus: () => void;
    onBlur: (e: { currentTarget: { contains: (node: Node | null) => boolean }; relatedTarget: EventTarget | null }) => void;
  };
}

/**
 * Raise / retract a row docked to a screen edge: hover or focus raises it at
 * once, leaving retracts it only after `holdMs` so moving between cards (or to
 * the dock's own bar and back) never drops it out from under the pointer.
 * `blocked` keeps it down (a prompt sheet owns the edge) and drops it the
 * moment it becomes true.
 */
export function useDockRaise(opts: { holdMs?: number; blocked?: boolean } = {}): DockRaise {
  const { holdMs = 200, blocked = false } = opts;
  const [raised, setRaised] = useState(false);
  const timer = useRef(0);
  const raise = () => {
    window.clearTimeout(timer.current);
    if (blocked) return;
    setRaised(true);
  };
  const retract = () => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setRaised(false), holdMs);
  };
  useMountEffect(() => () => window.clearTimeout(timer.current));
  useCloseWhen(blocked, () => setRaised(false));
  return {
    raised,
    setRaised,
    raise,
    retract,
    hover: {
      onMouseEnter: raise,
      onMouseLeave: retract,
      onFocus: raise,
      onBlur: (e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) retract();
      },
    },
  };
}