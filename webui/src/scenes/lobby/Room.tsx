// Waiting room (RoomPanelView). Reached by /room/<id>; after a refresh it
// re-attaches to the player through the server session.

import { useEffect, useState } from "react";
import { navigate } from "../../app/router";
import { charArt } from "../../core/assets";
import { sfx } from "../../core/audio";
import { cx } from "../../core/cx";
import { D } from "../../core/data";
import { useSessionOther } from "../../core/hooks";
import { getProfile } from "../../core/store";
import type { BotMentality } from "../../core/types";
import { downloadRecordPrompt } from "../../game/downloadPrompt";
import { submitRoomNonce } from "../../game/nonce";
import { endSession, matchScene, OnlineSession, resumeOnline } from "../../game/session";
import { model } from "../board/model";
import { api } from "../../net/api";
import { fmtMsg, type Msg } from "../../i18n/msg";
import { Btn } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { formula, showScoreWeights } from "../../ui/ScoreWeights";
import { DEFAULT_CLOCK, showClockSettings } from "../../ui/ClockSettings";
import { toast } from "../../ui/Toast";
import { TopBar } from "../../ui/TopBar";
import s from "./Room.module.css";
import { t as tr } from "../../i18n/t";

const report = (r: { ok: boolean; error?: Msg }) => !r.ok && toast(fmtMsg(r.error!), "error");

/** Attach to room `id` (already joined, or re-attach after a refresh). */
export function useOnline(id: string): OnlineSession | null {
  const [sess, setSess] = useState<OnlineSession | null>(null);
  useEffect(() => {
    let alive = true;
    const p = getProfile();
    const home = D.persona(p.homePersona);
    void resumeOnline(id, p.playerName, home?.name ?? "", home ? D.artId(home) : "").then((r) => {
      if (!alive) return;
      if ("k" in r) {
        toast(fmtMsg(r), "error");
        endSession();
        navigate({ name: "lobby" }, { replace: true });
      } else setSess(r);
    }, () => {
      // `ensureSession` can throw on a refused storage write, and the switch
      // path may reject on its own; never leave the "entering" screen stuck.
      if (!alive) return;
      toast(fmtMsg({ k: "err.offline" }), "error");
      endSession();
      navigate({ name: "lobby" }, { replace: true });
    });
    return () => {
      alive = false;
    };
  }, [id]);
  useSessionOther(sess);
  // Commit-reveal (`docs/FAIRNESS.md`): contribute our nonce as we enter the
  // room -- and again whenever the room shows a new commitment (the slot the
  // server rolls after every match). No window, no wait: a nonce that never
  // arrives is simply absent from the derived seed.
  // `seated` is in the deps because a spectator who later takes a free seat
  // (`join` upgrades in place) becomes a human the seed must cover.
  const commit = sess?.room?.fair?.commit ?? "";
  const seated = !!sess && !sess.spectator && !!sess.room?.members.some((m) => m.id === sess.you);
  useEffect(() => {
    const r = sess?.room;
    if (!r || !commit || r.playing) return;
    const me = r.members.find((m) => m.id === sess!.you);
    if (!me || me.bot) return;
    submitRoomNonce(r.id, commit);
  }, [sess, commit, seated]);

  useEffect(() => {
    if (sess?.dissolved) {
      toast(fmtMsg(sess.dissolved!), "error");
      endSession();
      navigate({ name: "lobby" }, { replace: true });
    }
  });
  return sess;
}

export function Room({ id }: { id: string }) {
  const sess = useOnline(id);
  const [armed, setArmed] = useState(false);
  /** Mentality of the next bot the host adds. */
  const [nextMentality, setNextMentality] = useState<BotMentality>("standard");
  /** Cycle the picker: standard -> chaos -> advanced -> standard. */
  const cycleMentality = () =>
    setNextMentality(nextMentality === "standard" ? "chaos" : nextMentality === "chaos" ? "advanced" : "standard");
  useEffect(() => sfx("place"), []);
  // The match started: go to it.
  useEffect(() => {
    if (!sess) return;
    return sess.subscribe((v) => {
      // A seat that forfeited / went bankrupt left the match screen on purpose;
      // do not bounce it straight back into the match. A spectator is `out` by
      // construction (`playerId` -1) but has no seat to abandon — they came to
      // watch, so send them in.
      const spectating = sess.spectator;
      if (v && matchScene(v) && sess.room?.playing && (spectating || !model(v).out)) {
        navigate({ name: "play", id }, { replace: true });
      }
    });
  }, [sess, id]);

  if (!sess?.room) return <TopBar section={tr("menu.online")} title={tr("room.entering")} onBack={() => navigate({ name: "lobby" })} />;
  const r = sess.room;
  const me = r.members.find((m) => m.id === sess.you);
  const isHost = !!me?.host;
  const spectating = sess.spectator;
  const leave = () => {
    endSession();
    navigate({ name: "lobby" });
  };
  /** Take a free seat without leaving first (`join` upgrades in place). */
  const takeSeat = async () => {
    const res = await api.join(r.id, "", false);
    if (!res.ok) return toast(fmtMsg(res.error!), "error");
    // Apply the reply now: the SSE `room` frame would get there eventually,
    // but a reconnecting stream must not leave us locked in watcher mode.
    sess.adoptSeat(res.data!.room, res.data!.you);
  };
  const start = async () => {
    const res = await api.start(r.id, armed);
    if (res.ok) return;
    if (!armed) {
      setArmed(true);
      toast(`${fmtMsg(res.error)}${tr("room.forceStartHint")}`, "error");
    } else toast(fmtMsg(res.error!), "error");
  };


  const empty = Math.max(0, r.maxPlayers - r.members.length);
  return (
    <>
      <TopBar section={tr("menu.online")} title={r.name} onBack={leave} />
      <div className={s.info}>
        <span className={cx(s.mode, r.ranked && s.ranked)}>{r.ranked ? tr("mode.ranked") : tr("mode.casual")}</span>
        <b>{tr("room.idLabel", { id: r.id })}</b>
        <span>{tr("room.players", { n: r.members.length, max: r.maxPlayers, hint: r.ranked ? tr("lobby.playersHintRanked") : tr("solo.playersHint"), locked: r.locked ? tr("room.locked") : "" })}</span>
        {spectating && <span className={s.tagWatching}>{tr("room.watching")}</span>}
        {(r.spectators?.length ?? 0) > 0 && (
          <span className={s.spectators} title={r.spectators!.map((sp) => sp.player).join(", ")}>
            <Icon name="groups" />
            {tr("room.spectators", { n: r.spectators!.length })}
            {r.spectators!.slice(0, 3).map((sp) => (
              <span key={sp.id} className={cx(s.spectator, sp.id === sess.you && s.me)}>
                {sp.player}{sp.id === sess.you && <small>{tr("common.youSuffix")}</small>}
              </span>
            ))}
            {r.spectators!.length > 3 && <span className={s.spectator}>+{r.spectators!.length - 3}</span>}
          </span>
        )}
        {!sess.connected && <span className={s.warn}>{tr("room.reconnecting")}</span>}
        <span className={s.spacer} />
        <Btn size="small" icon="leaderboard" onClick={() => showScoreWeights(r.weights, isHost && !r.playing, async (w) => report(await api.weights(r.id, w)))}>{tr("solo.scoreRules")}</Btn>
        <Btn size="small" icon="schedule" onClick={() => showClockSettings(r.clock ?? DEFAULT_CLOCK, isHost && !r.playing, async (c) => report(await api.setClock(r.id, c)))}>{tr("clock.title")}</Btn>
        <Btn size="small" icon="person_add" onClick={() => { void navigator.clipboard?.writeText(r.id); toast(tr("room.copied", { id: r.id })); }}>{tr("room.invite")}</Btn>
      </div>
      <div className={cx(s.grid, r.maxPlayers > 6 && s.dense)}>
        {r.members.map((m) => {
          const c = D.persona(m.persona);
          return (
            <div key={m.id} className={cx(s.member, m.id === sess.you && s.me)}>
              <div className={s.art}>{c ? <img src={charArt(D.artId(c), "stand")} alt="" /> : <div className={s.silhouette}><Icon name={m.bot ? "smart_toy" : "person"} /></div>}</div>
              <div className={s.tags}>
                {m.host && <span className={s.tagHost}>{tr("room.host")}</span>}
                {m.bot && <span className={s.tagBot}>{tr("solo.bot")}</span>}
                {m.bot && m.mentality === "chaos" && <span className={s.tagChaos}>{tr("solo.mentalityChaos")}</span>}
                {m.bot && m.mentality === "advanced" && <span className={s.tagBot}>{tr("solo.mentalityAdvanced")}</span>}
                {m.away && <span className={s.tagAway}>{tr("room.offline")}</span>}
              </div>
              <div className={s.text}>
                <div className={s.name}>{m.player}{m.id === sess.you && <small>{tr("common.youSuffix")}</small>}</div>
                <div className={s.chara}>{c?.display ?? "　"}</div>
                <div className={cx(s.ready, (m.ready || m.host || m.bot) && s.readyOn)}>{m.host ? tr("room.host") : m.bot || m.ready ? tr("room.ready") : tr("room.notReady")}</div>
              </div>
              {isHost && m.bot && !r.playing && (
                <button type="button" className={s.x} title={tr("common.remove")} onClick={async () => report(await api.bot(r.id, "remove", m.id))}><Icon name="close" /></button>
              )}
            </div>
          );
        })}
        {Array.from({ length: empty }, (_, i) => (
          <div key={`e${i}`} className={cx(s.member, s.empty)}>
            {isHost && !r.playing
              ? (
                <div className={s.addWrap}>
                  <button type="button" className={s.add} onClick={async () => report(await api.bot(r.id, "add", 0, nextMentality))}><Icon name="person_add" />{tr("solo.addBot")}</button>
                  <button
                    type="button"
                    className={cx(s.mentalityPick, nextMentality === "chaos" && s.mentalityChaos)}
                    title={tr("solo.mentalityHint")}
                    onClick={cycleMentality}
                  >
                    {nextMentality === "chaos"
                      ? tr("solo.mentalityChaos")
                      : nextMentality === "advanced"
                        ? tr("solo.mentalityAdvanced")
                        : tr("solo.mentalityStandard")}
                  </button>
                </div>
              )
              : <div className={s.wait}><Icon name="hourglass" />{tr("room.waitingJoin")}</div>}
          </div>
        ))}
      </div>
      <div className={s.foot}>
        <p className={s.formula}>{formula(r.weights)}</p>
        {/* The sealed record reveals every hand: only a seat may download it. */}
        {!r.playing && !spectating && (
          <Btn
            icon="download"
            onClick={async () => {
              const res = await api.record(r.id);
              if (!res.ok) {
                toast(fmtMsg(res.error), "error");
                return;
              }
              downloadRecordPrompt(res.bytes, res.filename);
            }}
          >
            {tr("room.downloadLastReplay")}
          </Btn>
        )}
        <Btn size="big" icon="logout" onClick={leave}>{tr("room.leave")}</Btn>
        {r.playing ? <Btn kind="pink" size="big" className={s.main} onClick={() => navigate({ name: "play", id })}>{tr("room.backToMatch")}</Btn>
          : spectating
            ? (r.members.length < r.maxPlayers
              ? <Btn kind="pink" size="big" className={s.main} onClick={takeSeat}>{tr("lobby.join")}</Btn>
              : <Btn kind="white" size="big" className={s.main} disabled>{tr("room.watching")}</Btn>)
            : isHost ? <Btn kind="pink" size="big" className={s.main} onClick={start}>{armed ? tr("room.forceStart") : tr("solo.start")}</Btn>
          : <Btn kind={me?.ready ? "white" : "pink"} size="big" className={s.main} onClick={async () => report(await api.ready(r.id, !me?.ready))}>{me?.ready ? tr("room.cancelReady") : tr("room.ready")}</Btn>}
      </div>
    </>
  );
}
