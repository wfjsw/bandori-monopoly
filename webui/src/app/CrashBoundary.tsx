// The last thing that still renders when a child blows up. React 19 unmounts
// the failed subtree and leaves a white screen with no UI of its own; this
// boundary catches the throw and swaps in `CrashPanel` -- which must not depend
// on `ModalHost` (that lives *inside* the crashed subtree).

import { Component, type ErrorInfo, type ReactNode } from "react";
import { getCrash, reportCrash, subscribeCrash, type CrashInfo } from "../core/crash";
import { CrashPanel } from "./CrashPanel";

interface Props {
  children: ReactNode;
}

interface State {
  crash: CrashInfo | null;
}

function local(message: string, stack?: string, componentStack?: string): CrashInfo {
  return {
    kind: "render",
    message: message || "render error",
    stack,
    componentStack,
    // Node-safe: a boundary should never throw while reporting a throw.
    time: Date.now(),
    url: typeof location !== "undefined" ? location.href : "",
    ua: typeof navigator !== "undefined" ? navigator.userAgent : "",
  };
}

/**
 * Catches render errors from anywhere below it, and surfaces a reported
 * `"worker"` crash (the solo engine dying does not throw into React). A stray
 * `"error"` / `"rejection"` is recorded in the store but leaves a healthy board
 * alone -- see `reportCrash`.
 */
export class CrashBoundary extends Component<Props, State> {
  state: State = { crash: null };
  private unsub: (() => void) | null = null;

  // Only enough to flip the tree to the fallback; `componentDidCatch` fills in
  // the component stack (React does not pass it here).
  static getDerivedStateFromError(error: Error): Partial<State> {
    return { crash: local(error?.message, error?.stack) };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    const crash = local(error?.message, error?.stack, info.componentStack ?? undefined);
    reportCrash(crash);
    // Prefer the catch's own record even if the store kept an earlier crash:
    // the panel must show *this* render failure, not a stale rejection.
    this.setState({ crash });
  }

  componentDidMount(): void {
    this.unsub = subscribeCrash((c) => {
      if (c.kind === "render" || c.kind === "worker") this.setState({ crash: c });
    });
    // A crash that landed before this boundary mounted (worker death during boot).
    const existing = getCrash();
    if (existing && (existing.kind === "render" || existing.kind === "worker")) {
      this.setState({ crash: existing });
    }
  }

  componentWillUnmount(): void {
    this.unsub?.();
    this.unsub = null;
  }

  render() {
    if (this.state.crash) return <CrashPanel info={this.state.crash} />;
    return this.props.children;
  }
}