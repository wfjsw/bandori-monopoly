// Replay transport: play / pause, speed, skip-idle, prev / next turn, the scrub
// bar with turn marks, and the perspective dropdown (any seat or spectator).

import { useSessionOther, useTick } from "../../core/hooks";
import { D } from "../../core/data";
import { cx } from "../../core/cx";
import type { ReplaySession } from "../../game/replay";
import { Btn } from "../../ui/Button";
import { t as tr } from "../../i18n/t";
import s from "./Replay.module.css";

export function ReplayBar({ rs, onExit }: { rs: ReplaySession; onExit: () => void }) {
  useSessionOther(rs);
  useTick(200); // the scrub head follows `status()` while playing
  const st = rs.status();
  const now = Number(st.tick) || 0;
  const total = Math.max(1, rs.totalTicks);
  const marks = rs.turns;
  const seats = rs.header.seats;

  const seekTo = (tick: number) => {
    rs.seek(tick);
  };

  return (
    <div className={s.bar}>
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
          {tr("replay.perspective")}
          <select value={rs.perspective} onChange={(e) => rs.setPerspective(Number(e.target.value))}>
            <option value={0}>{tr("replay.spectator")}</option>
            {seats.map((x) => (
              <option key={x.member} value={x.member}>
                {x.player}
                {x.character ? ` · ${D.character(x.character)?.display ?? x.character}` : ""}
                {x.bot ? ` (${x.mentality === "chaos" ? tr("solo.mentalityChaos") : tr("solo.mentalityStandard")})` : ""}
              </option>
            ))}
          </select>
        </label>
        <Btn size="small" icon="home" onClick={onExit}>{tr("replay.back")}</Btn>
      </div>
    </div>
  );
}