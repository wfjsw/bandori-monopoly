// The replay screen: the unchanged Board fed by a `ReplaySession`, the
// transport bar over it, and the two interruptions a record can earn --
// a load failure up front (missing bundle, unreadable file, fatal ABI/format)
// and the divergence banner mid-play. The engine that plays is the one that
// wrote the record (`docs/REPLAY.md` §9): the page's own wasm for a record
// made by this build, otherwise the archived bundle, booted in a worker.

import { useEffect, useState } from "react";
import { navigate } from "../../app/router";
import { useSessionOther } from "../../core/hooks";
import { cx } from "../../core/cx";
import {
  clearPendingReplay,
  endReplay,
  openPending,
  type Opened,
  type ReplaySession,
} from "../../game/replay";
import { Board } from "../board/Board";
import { Btn } from "../../ui/Button";
import { ReplayBar } from "./ReplayBar";
import s from "./Replay.module.css";
import { t as tr } from "../../i18n/t";
import type { VerifyReport } from "../../game/record";
import { bindConsoleSession } from "../../console/context";

export function ReplayPlayer() {
  // `openPending` is memoized on the queued record and async (an archived
  // bundle boots in a worker), so this is a loading state first.
  const [loaded, setLoaded] = useState<Opened | { phase: "loading" }>({ phase: "loading" });

  useEffect(() => {
    const p = openPending();
    if (!p) {
      setLoaded({ phase: "error", message: tr("replay.noReplay") });
      return;
    }
    let live = true;
    void p.then((o) => {
      if (live) setLoaded(o);
    });
    return () => {
      live = false;
    };
  }, []);

  const back = () => {
    clearPendingReplay();
    endReplay();
    navigate({ name: "replay" });
  };

  if (loaded.phase === "loading") {
    return (
      <div className={s.error}>
        <p>{tr("replay.loadingEngine")}</p>
        <Btn onClick={back}>{tr("replay.back")}</Btn>
      </div>
    );
  }
  if (loaded.phase === "error") {
    return (
      <div className={s.error}>
        <p>{tr("replay.loadFailed")}</p>
        <p className={s.detail}>{loaded.message}</p>
        <Btn kind="pink" onClick={back}>{tr("replay.back")}</Btn>
      </div>
    );
  }
  return <PlayerLive rs={loaded.rs} />;
}

function PlayerLive({ rs }: { rs: ReplaySession }) {
  useEffect(() => bindConsoleSession(rs), [rs, bindConsoleSession]);
  useSessionOther(rs);
  // The fairness check (`docs/FAIRNESS.md`): recompute the commitment, derive
  // the match seed and re-run the record on this very engine.
  const [verify, setVerify] = useState<{ run: true } | { run: false; rep: VerifyReport } | null>(null);
  const runVerify = () => {
    setVerify({ run: true });
    rs.verifyFair()
      .then((json) => setVerify({ run: false, rep: JSON.parse(json) as VerifyReport }))
      .catch((e) =>
        setVerify({
          run: false,
          rep: {
            ok: false,
            present: true,
            steps: [{ step: "commit", ok: false, note: String(e?.message ?? e) }],
          },
        }),
      );
  };
  // The replay runs on the bundle that wrote it, so a checkpoint mismatch is
  // engine drift in the record itself -- not "this is a different build".
  const stampNote = rs.stampMismatches.length
    ? tr("replay.divergedStamp", { fields: rs.stampMismatches.map((x) => x.field).join(", ") })
    : rs.engineKind === "worker"
      ? tr("replay.divergedArchived", { bundle: rs.engineBundle.slice(0, 12) })
      : "";
  return (
    <div className={s.player}>
      {/* Remount the Board on every seek: the animator's log and token
          positions are re-seeded from the restored frame. */}
      <Board key={rs.epoch} sess={rs} />
      {/* A portable record carries its own engine -- say so, rather than
          leaving the user to wonder which build is playing. */}
      {rs.engineSource === "embedded" && (
        <div className={s.banner} role="note">
          <span>{tr("replay.embeddedEngine", { bundle: rs.engineBundle.slice(0, 12) })}</span>
        </div>
      )}
      {rs.divergence && (
        <div className={cx(s.banner, s.diverge)} role="alert">
          <span>
            {tr("replay.diverged", { round: rs.divergence.round, turn: rs.divergence.turn })}
            {stampNote ? ` — ${stampNote}` : ""}
          </span>
          <div className={s.btns}>
            <Btn size="small" kind="pink" onClick={() => rs.ackDivergence(true)}>{tr("replay.divergedContinue")}</Btn>
            <Btn size="small" onClick={() => rs.ackDivergence(false)}>{tr("replay.divergedStop")}</Btn>
          </div>
        </div>
      )}
      {verify && (
        <div className={cx(s.banner, verify.run || verify.rep.ok ? s.verifyOk : s.diverge)} role="status">
          <span>
            {verify.run
              ? tr("replay.verifyRunning")
              : !verify.rep.present
                ? tr("replay.verifyNoMaterial")
                : verify.rep.ok
                  ? tr("replay.verifyPass")
                  : tr("replay.verifyFail")}
            {!verify.run &&
              verify.rep.steps
                .filter((st) => !st.ok || st.note)
                .map((st) => ` — ${st.step}: ${st.ok ? "✓" : "✗"} ${st.note}`)
                .join("")}
          </span>
          <div className={s.btns}>
            <Btn size="small" onClick={runVerify} disabled={verify.run}>
              {tr("replay.verify")}
            </Btn>
          </div>
        </div>
      )}
      {!verify && (
        <div className={s.btns} style={{ position: "absolute", right: "0.6em", top: "0.6em", zIndex: 5 }}>
          <Btn size="small" onClick={runVerify}>
            {tr("replay.verify")}
          </Btn>
        </div>
      )}
      <ReplayBar rs={rs} onExit={() => { clearPendingReplay(); endReplay(); navigate({ name: "replay" }); }} />
    </div>
  );
}
