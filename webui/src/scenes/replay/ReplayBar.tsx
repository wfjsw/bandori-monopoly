// Replay transport: play / pause, speed, skip-idle, prev / next turn, the scrub
// bar with turn marks, and the perspective dropdown (any seat or spectator).
// The bar itself can be docked (compact floating, or full-width flush to the
// board's bottom edge, or a second chrome row under the TopBar) and hidden;
// both choices stick in localStorage. The hide control is the strip's leftmost
// item in every dock mode, and the show handle takes that same slot when the
// strip is gone -- toggle without moving the pointer. Transport shortcuts
// (Space / arrows / H) keep working while the bar is hidden -- the handler
// lives here, not in the chrome.

import { useRef, useState } from "react";
import { useSessionOther, useTick } from "../../core/hooks";
import { useHotkeys } from "../../hooks/dom";
import { useResizeObserver } from "../../hooks/measure";
import { D } from "../../core/data";
import { cx } from "../../core/cx";
import type { ReplaySession } from "../../game/replay";
import { Btn } from "../../ui/Button";
import { TOPBAR_H } from "../../styles/layout";
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

  // The transport's measured height becomes edge insets on the player root
  // (the common ancestor of this bar and the Board). Bottom-docked: bottom-
  // edge chrome (hand dock, peek, skill stack, sheet strip, actions, player
  // list, log) rides up by the bar's height. Top-docked: the bar rides *under*
  // the TopBar, so the inset is `--topbar-h` + the bar -- the whole board body
  // drops by that, taking the field row, zoom control and card preview with
  // it. Floating or hidden: both insets stay 0, nothing has to make room.
  const barRef = useRef<HTMLDivElement>(null);
  useResizeObserver(barRef, (el) => {
    const root = el.parentElement;
    if (!root) return;
    const h = prefs.hidden ? 0 : el.offsetHeight;
    root.style.setProperty("--replay-bar-h", `${h}px`);
    root.style.setProperty("--replay-bar-bottom", !prefs.hidden && prefs.dock === "bottom" ? `${h}px` : "0px");
    root.style.setProperty(
      "--replay-bar-top",
      !prefs.hidden && prefs.dock === "top" ? `${TOPBAR_H + h}px` : "0px",
    );
  }, [prefs.hidden, prefs.dock]);

  // Transport shortcuts, alive whether or not the bar is on screen. Editable
  // fields (the console, the scrub range, the perspective select) keep their
  // own keys; a focused button keeps Space so it can be activated that way.
  useHotkeys([
    {
      key: " ",
      onControl: false,
      run: (e) => {
        e.preventDefault();
        rs.setPlaying(!rs.playing);
      },
    },
    {
      key: "ArrowLeft",
      run: (e) => {
        e.preventDefault();
        if (e.shiftKey) rs.prevTurn();
        else rs.seek(rs.now() - SEEK_TICKS);
      },
    },
    {
      key: "ArrowRight",
      run: (e) => {
        e.preventDefault();
        if (e.shiftKey) rs.nextTurn();
        else rs.seek(rs.now() + SEEK_TICKS);
      },
    },
    {
      key: "Home",
      run: (e) => {
        e.preventDefault();
        rs.seek(0);
      },
    },
    {
      key: "End",
      run: (e) => {
        e.preventDefault();
        rs.seek(rs.totalTicks);
      },
    },
    {
      key: ["h", "H"],
      once: true,
      run: (e) => {
        e.preventDefault();
        setPrefs((old) => {
          const n = { ...old, hidden: !old.hidden };
          saveBarPrefs(n);
          return n;
        });
      },
    },
  ]);

  // One strip in every dock mode. The toggle lives in the leftmost slot in
  // both states -- hide at the head of the bar, the show handle in that same
  // slot once the strip is gone -- so the pointer does not have to travel
  // between the two. Hidden, the strip keeps its box (the rest goes
  // `visibility: hidden`) so the handle lands exactly on the hide button.
  return (
    <div
      ref={barRef}
      className={cx(
        s.bar,
        prefs.dock === "bottom" && s.dockBottom,
        prefs.dock === "top" && s.dockTop,
        prefs.hidden && s.barHidden,
      )}
      title={tr("replay.barHint")}
    >
      {prefs.hidden ? (
        <Btn
          size="small"
          icon="add"
          className={s.handle}
          onClick={() => patch({ hidden: false })}
          title={tr("replay.barHint")}
        >
          {tr("replay.showBar")}
        </Btn>
      ) : (
        <Btn size="small" icon="remove" onClick={() => patch({ hidden: true })} title={tr("replay.barHint")}>{tr("replay.hideBar")}</Btn>
      )}

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
        <Btn size="small" icon="home" onClick={onExit}>{tr("replay.back")}</Btn>
      </div>
    </div>
  );
}