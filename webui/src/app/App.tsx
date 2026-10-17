// App shell: boot gate, History API routes with the scene fade (0.14 s each way), BGM per screen, popups and toasts.

import { lazy, type ReactNode, Suspense, useEffect, useMemo, useState } from "react";
import { playSceneBgm, sfx, unlockAudio } from "../core/audio";
import { useLangVersion, useProfile } from "../core/hooks";
import { preloadRoute, warmAllWhenIdle, within } from "../core/preload";
import { useCrossfade } from "../hooks/scene";
import { Boot } from "../scenes/boot/Boot";
import { Lobby } from "../scenes/lobby/Lobby";
import { Room } from "../scenes/lobby/Room";
import { Menu } from "../scenes/menu/Menu";
import { Play } from "../scenes/Play";
import { closeAllModals, ModalHost } from "../ui/Modal";
import { InspectPreviewHost } from "../ui/CardPreview";
import { ToastHost } from "../ui/Toast";
import { href, navigate, parse, type Route, usePath } from "./router";
import { setBackdrop, Stage } from "./Stage";
import { RotateHint } from "./RotateHint";
import { Console } from "../console/Console";

// Rarely visited scenes load as separate chunks (core/preload.ts warms them).
const Deck = lazy(() => import("../scenes/deck/Deck").then((m) => ({ default: m.Deck })));
const Gallery = lazy(() => import("../scenes/gallery/Gallery").then((m) => ({ default: m.Gallery })));
const Replays = lazy(() => import("../scenes/replay/Replays").then((m) => ({ default: m.Replays })));
const ReplayPlayer = lazy(() => import("../scenes/replay/ReplayPlayer").then((m) => ({ default: m.ReplayPlayer })));

const BACKDROP: Record<Route["name"], string> = { menu: "bg_common", lobby: "bg_live", room: "bg_live", play: "bg_band", gallery: "bg_band", deck: "bg_band", replay: "bg_common", replayView: "bg_common" };
const BGM: Record<Route["name"], string> = { menu: "menu", lobby: "lobby", room: "lobby", play: "select", gallery: "gallery", deck: "deck", replay: "menu", replayView: "board" };

function scene(r: Route): ReactNode {
  switch (r.name) {
    case "menu": return <Menu />;
    case "gallery": return <Gallery />;
    case "deck": return <Deck />;
    case "lobby": return <Lobby />;
    case "room": return <Room id={r.id} />;
    case "play": return <Play id={r.id} />;
    case "replay": return <Replays />;
    case "replayView": return <ReplayPlayer />;
  }
}

// No page zoom: iOS Safari ignores `user-scalable=no`, so cancel its pinch
// gestures and multi-touch moves here (the board's own map pinch uses pointer
// events on a `touch-action: none` element and is unaffected).
for (const g of ["gesturestart", "gesturechange", "gestureend"]) {
  document.addEventListener(g, (e) => e.preventDefault(), { passive: false });
}
document.addEventListener("touchmove", (e) => { if (e.touches.length > 1) e.preventDefault(); }, { passive: false });

// TapFeedback: unlock audio on the first gesture, a tap sound on buttons.
document.addEventListener("pointerdown", () => unlockAudio(), { capture: true });
document.addEventListener("click", (e) => {
  if ((e.target as Element).closest("button:not(:disabled)")) sfx("tap", 40);
});

export function App() {
  const [ready, setReady] = useState(false);
  const profile = useProfile();
  const lang = useLangVersion();
  const path = usePath();
  // Parsed once per path: `useCrossfade` keys on this object, and a fresh one
  // every render would re-run its effect (and cancel its timers) on every
  // profile bump under a live match.
  const target = useMemo(() => parse(path), [path]);

  // Unknown or empty routes go to the menu.
  useEffect(() => {
    if (ready && profile && !target) navigate({ name: "menu" }, { replace: true });
  }, [ready, profile, target]);

  // Warm every screen's art and code once the app is up.
  useEffect(() => {
    if (ready) warmAllWhenIdle();
  }, [ready]);

  // Fade out while the next scene's art loads, swap (backdrop + BGM) with the
  // new scene already decoded, then fade in. The preload is capped so a slow
  // network delays the swap by at most 450ms.
  const { shown, fading } = useCrossfade(ready ? target : null, {
    eq: (a, b) => href(a) === href(b),
    outMs: 140,
    inMs: 0,
    prepare: (r) => within(preloadRoute(r), 450),
    onSwap: (r) => {
      closeAllModals();
      setBackdrop(BACKDROP[r.name]);
      playSceneBgm(BGM[r.name]);
    },
  });

  const booted = ready && profile;
  return (
    <>
      <Stage fading={fading}>
        {/* Remount on language change so every already-rendered string is re-read. */}
        <div key={lang} style={{ position: "absolute", inset: 0 }}>
          {booted ? shown && <div key={href(shown)}><Suspense fallback={null}>{scene(shown)}</Suspense></div> : <Boot resume={!!target} onReady={() => setReady(true)} />}
        </div>
        <ModalHost />
        <InspectPreviewHost />
        <ToastHost />
      </Stage>
      <RotateHint />
      <Console />
    </>
  );
}