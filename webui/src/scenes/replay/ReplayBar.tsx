// Replay transport: play / pause, speed, skip-idle, prev / next turn, the scrub
// bar with turn marks, and the perspective dropdown (any seat or spectator).
// The bar itself can be docked (compact floating, or full-width flush to the
// top / bottom edge) and hidden away behind a small handle; both choices stick
// in localStorage. Transport shortcuts (Space / arrows / H) keep working while
// the bar is hidden -- the handler lives here, not in the chrome.

import { useEffect, useState } from "react";
import { useSessionOther, useTick } from "../../core/hooks";
import { D } from "../../core/data";
import { cx } from "../../core/cx";
import type { ReplaySession } from "../../game/replay";
import { Btn } from "../../ui/Button";
import { t as tr } from "../../i18n/t";
import s from "./Replay.module.css";

/** Where the transport rides: floating over the board, or docked at an edge. */
export type BarDock = "float" | "bottom" | "top";

/** Saved transport chrome (localStorage `bm.replayBar`). */
export interface BarPrefs {
  dock: BarDock;
  hidden: boolean;
}

const BAR_KEY = "bm.replayBar";
const DEFAULT_BAR: BarPrefs = { dock: "float", hidden: false };
/** Fine keyboard seek: two seconds of the record's 50 ms tick quanta. */
const SEEK_TICKS = 40;

export function loadBarPrefs(): BarPrefs {
  try {
    const p = JSON.parse(localStorage.getItem(BAR_KEY) ?? "{}") as Partial<BarPrefs>;
    return {
      dock: p.dock === "bottom" || p.dock === "top" ? p.dock : "float",
      hidden: !!p.hidden,
    };
  } catch {
    return { ...DEFAULT_BAR };
  }
}

function saveBarPrefs(p: BarPrefs): void {
  try {
    localStorage.setItem(BAR_KEY, JSON.stringify(p));
  } catch {
    /* private mode -- the choice just does not stick */
  }
}

export function ReplayBar({ rs, onExit }: { rs: ReplaySession; onExit: () => void }) {
  useSessionOther(rs);
  useTick(200); // the scrub head follows `status()` while playing
  const [prefs, setPrefs] = useState<BarPrefs>(loadBarPrefs);
  const st = rs.status();
  const now = Number(st.tick) || 0;
  const total = Math.max(1, rs.totalTicks);
  const marks = rs.turns;
  const seats = rs.header.seats;

  const patch = (p: Partial<BarPrefs>) =>
    setPrefs((old) => {
      const n = { ...old, ...p };
      saveBarPrefs(n);
      return n;
    });

  const seekTo = (tick: number) => {
    rs.seek(tick);
  };

  // Transport shortcuts, alive whether or not the bar is on screen. Editable
  // fields (the console, the scrub range, the perspective select) keep their
  // own keys; a focused button keeps Space so it can be activated that way.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return;
      if (e.ctrlKey || e.metaKey || e.altKey || e.isComposing) return;
      const onButton = !!t && /^(BUTTON|A)$/.test(t.tagName);
      switch (e.key) {
        case " ":
          if (onButton) return;
          e.preventDefault();
          rs.setPlaying(!rs.playing);
          break;
        case "ArrowLeft":
          e.preventDefault();
          if (e.shiftKey) rs.prevTurn();
          else rs.seek(rs.now() - SEEK_TICKS);
          break;
        case "ArrowRight":
          e.preventDefault();
          if (e.shiftKey) rs.nextTurn();
          else rs.seek(rs.now() + SEEK_TICKS);
          break;
        case "Home":
          e.preventDefault();
          rs.seek(0);
          break;
        case "End":
          e.preventDefault();
          rs.seek(rs.totalTicks);
          break;
        case "h":
        case "H":
          if (e.repeat) return;
          e.preventDefault();
          setPrefs((old) => {
            const n = { ...old, hidden: !old.hidden };
            saveBarPrefs(n);
            return n;
          });
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [rs]);

  if (prefs.hidden) {
    // A small handle on the bar's own edge brings it back (or press H).
    return (
      <button
        type="button"
        className={cx(s.handle, prefs.dock === "top" && s.handleTop)}
        onClick={() => patch({ hidden: false })}
        title={tr("replay.barHint")}
      >
        {tr("replay.showBar")}
      </button>
    );
  }

  return (
    <div className={cx(s.bar, prefs.dock === "bottom" && s.dockBottom, prefs.dock === "top" && s.dockTop)} title={tr("replay.barHint")}>
      <div className={s.transport}>
        <Btn size="small" onClick={() => rs.setPlaying(!rs.playing)}>
          {rs.playing ? tr("replay.pause") : tr("replay.play")}
        </Btn>
        <Btn size="small" icon="chevron_left" onClick={() => rs.prevTurn()} title={tr("replay.prevTurn")}>{tr("replay.prevTurn")}</Btn>
        <Btn size="small" icon="chevron_right" onClick={() => rs.nextTurn()} title={tr("replay.nextTurn")}>{tr("replay.nextTurn")}</Btn>
        <div className={s.speeds}>
          {[1, 2, 4].map((v) => (
            <button key={v} type="button" className={cx(s.speed, rs.speed === v && s.on)} onClick={() => rs.setSpeed(v)}>{v}×</button>
          ))}
        </div>
        <label className={s.skip}>
          <input type="checkbox" checked={rs.skipIdle} onChange={(e) => rs.setSkipIdle(e.target.checked)} />
          {tr("replay.skipIdle")}
        </label>
      </div>

      <div className={s.scrub}>
        <input
          className={s.range}
          type="range"
          min={0}
          max={total}
          step={1}
          value={Math.min(now, total)}
          onChange={(e) => seekTo(Number(e.target.value))}
          aria-label={tr("replay.scrub")}
        />
        <div className={s.marks} aria-hidden>
          {marks.map((m) => (
            <button
              key={`${m.round}:${m.turn}:${m.tick}`}
              type="button"
              className={s.mark}
              style={{ left: `${(m.tick / total) * 100}%` }}
              title={tr("replay.turnMark", { round: m.round, turn: m.turn })}
              onClick={() => seekTo(m.tick)}
            />
          ))}
        </div>
        <div className={s.at}>
          {tr("replay.at", { round: rs.view?.state.round ?? 0, turn: rs.view?.state.turn ?? 0, tick: now })}
        </div>
      </div>

      <div className={s.side}>
        <label className={s.persp}>
          {tr("replay.dock")}
          <select
            value={prefs.dock}
            onChange={(e) => patch({ dock: e.target.value as BarDock })}
            aria-label={tr("replay.dock")}
          >
            <option value="float">{tr("replay.dockFloat")}</option>
            <option value="bottom">{tr("replay.dockBottom")}</option>
            <option value="top">{tr("replay.dockTop")}</option>
          </select>
        </label>
        <label className={s.persp}>
          {tr("replay.perspective")}
          <select value={rs.perspective} onChange={(e) => rs.setPerspective(Number(e.target.value))}>
            <option value={0}>{tr("replay.spectator")}</option>
            {seats.map((x) => (
              <option key={x.member} value={x.member}>
                {x.player}
                {x.character ? ` · ${D.character(x.character)?.display ?? x.character}` : ""}
                {x.bot ? ` (${x.mentality === "chaos" ? tr("solo.mentalityChaos") : x.mentality === "advanced" ? tr("solo.mentalityAdvanced") : tr("solo.mentalityStandard")})` : ""}
              </option>
            ))}
          </select>
        </label>
        <Btn size="small" icon="remove" onClick={() => patch({ hidden: true })} title={tr("replay.barHint")}>{tr("replay.hideBar")}</Btn>
        <Btn size="small" icon="home" onClick={onExit}>{tr("replay.back")}</Btn>
      </div>
    </div>
  );
}