// The replay screen: the unchanged Board fed by a `ReplaySession`, the
// transport bar over it, and the two interruptions a record can earn --
// a compatibility dialog up front, and the divergence banner mid-play.

import { useState } from "react";
import { navigate } from "../../app/router";
import { useSessionOther } from "../../core/hooks";
import { cx } from "../../core/cx";
import {
  clearPendingReplay,
  endReplay,
  openPending,
  type Opened,
  type ReplaySession,
  startReplay,
} from "../../game/replay";
import { Board } from "../board/Board";
import { Btn } from "../../ui/Button";
import { ReplayBar } from "./ReplayBar";
import s from "./Replay.module.css";
import { t as tr } from "../../i18n/t";

export function ReplayPlayer() {
  // `openPending` is memoized on the queued record, so StrictMode's double
  // initializer gets the same instance instead of loading it twice.
  const [loaded, setLoaded] = useState<Opened | { phase: "error"; message: string }>(
    () => openPending() ?? { phase: "error", message: tr("replay.noReplay") },
  );

  const back = () => {
    clearPendingReplay();
    endReplay();
    navigate({ name: "replay" });
  };

  if (loaded.phase === "error") {
    return (
      <div className={s.error}>
        <p>{tr("replay.loadFailed")}</p>
        <p className={s.detail}>{loaded.message}</p>
        <Btn kind="pink" onClick={back}>{tr("replay.back")}</Btn>
      </div>
    );
  }
  if (loaded.phase === "warn") {
    const { mismatches, m } = loaded;
    return (
      <div className={s.error}>
        <p>{tr("replay.compatWarnText")}</p>
        <ul className={s.mismatch}>
          {mismatches.map((x) => (
            <li key={x.field}>
              <b>{x.field}</b>: {x.want} → {x.got}
            </li>
          ))}
        </ul>
        <div className={s.btns}>
          <Btn onClick={() => { m.free(); back(); }}>{tr("common.cancel")}</Btn>
          {/* The mismatch is acknowledged; the instance built with `force`
              is the one that plays. */}
          <Btn kind="pink" onClick={() => setLoaded({ phase: "ready", rs: startReplay(m, "") })}>{tr("replay.continueAnyway")}</Btn>
        </div>
      </div>
    );
  }
  return <PlayerLive rs={loaded.rs} />;
}

function PlayerLive({ rs }: { rs: ReplaySession }) {
  useSessionOther(rs);
  return (
    <div className={s.player}>
      {/* Remount the Board on every seek: the animator's log and token
          positions are re-seeded from the restored frame. */}
      <Board key={rs.epoch} sess={rs} />
      {rs.divergence && (
        <div className={cx(s.banner, s.diverge)} role="alert">
          <span>{tr("replay.diverged", { round: rs.divergence.round, turn: rs.divergence.turn })}</span>
          <div className={s.btns}>
            <Btn size="small" kind="pink" onClick={() => rs.ackDivergence(true)}>{tr("replay.divergedContinue")}</Btn>
            <Btn size="small" onClick={() => rs.ackDivergence(false)}>{tr("replay.divergedStop")}</Btn>
          </div>
        </div>
      )}
      <ReplayBar rs={rs} onExit={() => { clearPendingReplay(); endReplay(); navigate({ name: "replay" }); }} />
    </div>
  );
}