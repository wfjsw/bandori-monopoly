import { useSyncExternalStore } from "react";
import { showInstallHint } from "./installGate";
import { PHONE_QUERY } from "./router";

// "Add to home screen" gate for the cover ahead of the rotate cover (see
// InstallHint.tsx). Phones only, and only while the page is still a browser
// tab: an installed PWA already runs `display-mode: standalone` (or the iOS
// home-screen shell) and needs no nudge. An explicit override records the
// choice so the cover never comes back in this browser profile.
//
// The decision itself is pure (installGate.ts); this file only gathers the
// browser facts and keeps the dismiss flag.

const DISMISS_KEY = "bm.installHint.dismissed";

/** iOS home-screen PWA (`navigator.standalone`); absent in every other shell. */
type StandaloneNavigator = Navigator & { standalone?: boolean };

/** Captured `beforeinstallprompt` so the cover's button can raise the sheet. */
interface InstallPromptEvent extends Event {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: "accepted" | "dismissed" }>;
}

// A bump per state change; `useInstallHint` re-reads the decision on it.
let version = 0;
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((cb) => cb());

export function subscribeInstallHint(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/** Store revision; re-renders subscribers when it moves. */
export function installHintVersion(): number {
  return version;
}

/** Is the page already running as an installed home-screen app? */
export function isStandalone(): boolean {
  return window.matchMedia("(display-mode: standalone)").matches
    || window.matchMedia("(display-mode: fullscreen)").matches
    || (navigator as StandaloneNavigator).standalone === true;
}

/**
 * A handset in either orientation. The phone query, not `isPhone()`, because
 * that one also matches an installed iOS PWA at any size -- and this cover is
 * only for the browser-tab case. Desktops and tablets fall outside it.
 */
export function isPhoneDevice(): boolean {
  return window.matchMedia(PHONE_QUERY).matches;
}

/** Has the user overridden the install cover ("continue in the browser")? */
export function isInstallHintDismissed(): boolean {
  try {
    return localStorage.getItem(DISMISS_KEY) === "1";
  } catch {
    return true;
  }
}

/** The override: never show the install cover again in this browser profile. */
export function dismissInstallHint(): void {
  try {
    localStorage.setItem(DISMISS_KEY, "1");
  } catch {
    // Private mode: still hide it for this page's lifetime.
  }
  version++;
  emit();
}

/** The live install-cover decision (re-renders on override / install). */
export function useInstallHint(): boolean {
  useSyncExternalStore(subscribeInstallHint, installHintVersion);
  return showInstallHint({
    phone: isPhoneDevice(),
    standalone: isStandalone(),
    dismissed: isInstallHintDismissed(),
  });
}

// `beforeinstallprompt` is Chrome/Android only; iOS Safari has no sheet and
// the cover falls back to the share-menu steps. Capture it eagerly so the
// button is armed by the time the cover paints.
let deferred: InstallPromptEvent | null = null;

export function hasInstallPrompt(): boolean {
  return deferred !== null;
}

/** Show the browser's install sheet, if it offered one. */
export async function runInstallPrompt(): Promise<void> {
  if (!deferred) return;
  const evt = deferred;
  deferred = null;
  await evt.prompt();
  void evt.userChoice.then(() => {
    version++;
    emit();
  });
  version++;
  emit();
}

if (typeof window !== "undefined") {
  window.addEventListener("beforeinstallprompt", (e) => {
    e.preventDefault();
    deferred = e as InstallPromptEvent;
    version++;
    emit();
  });
  window.addEventListener("appinstalled", () => {
    deferred = null;
    version++;
    emit();
  });
}