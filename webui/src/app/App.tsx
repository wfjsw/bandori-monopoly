// App shell: boot gate, History API routes with the scene fade (0.14 s each way), BGM per screen, popups and toasts.

import { type ReactNode, Suspense, useEffect, useMemo, useState } from "react";
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
import { LazyScene } from "./scenes";

const BACKDROP: Record<Route["name"], string> = { menu: "bg_common", lobby: "bg_live", room: "bg_live", play: "bg_band", gallery: "bg_band", deck: "bg_band", replay: "bg_common", replayView: "bg_common" };
const BGM: Record<Route["name"], string> = { menu: "menu", lobby: "lobby", room: "lobby", play: "select", gallery: "gallery", deck: "deck", replay: "menu", replayView: "board" };

function scene(r: Route): ReactNode {
  switch (r.name) {
    case "menu": return <Menu />;
    case "gallery": return <LazyScene name="gallery" />;
    case "deck": return <LazyScene name="deck" />;
    case "lobby": return <Lobby />;
    case "room": return <Room id={r.id} />;
    case "play": return <Play id={r.id} />;
    case "replay": return <LazyScene name="replay" />;
    case "replayView": return <LazyScene name="replayView" />;
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

  // Dissolve to the next scene once its art is loaded. The preload is capped so
  // a slow network delays the swap by at most 450ms.
  const { shown, fading, dimmed } = useCrossfade(ready ? target : null, {
    eq: (a, b) => href(a) === href(b),
    outMs: 140,
    inMs: 140,
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
      <Stage transitioning={fading}>
        {/* Remount on language change so every already-rendered string is re-read. */}
        <div key={lang} style={{ position: "absolute", inset: 0 }}>
          {/* `dimmed` fades this layer out to the backdrop on the fallback path. */}
          <div style={{ position: "absolute", inset: 0, opacity: dimmed ? 0 : 1, transition: "opacity 0.14s" }}>
            {booted ? shown && <div key={href(shown)}><Suspense fallback={null}>{scene(shown)}</Suspense></div> : <Boot resume={!!target} onReady={() => setReady(true)} />}
          </div>
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