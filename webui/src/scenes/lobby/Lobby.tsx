// Online lobby (LobbyController): room list with mode filters, join by room
// id, quick join, create room.

import { useEffect, useState } from "react";
import { navigate } from "../../app/router";
import { sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D } from "../../core/data";
import { getProfile } from "../../core/store";
import type { RoomInfo, ScoreWeights } from "../../core/types";
import { OnlineSession, session, startOnline } from "../../game/session";
import { api, ensureSession } from "../../net/api";
import { fmtMsg } from "../../i18n/msg";
import { Btn, RoundBtn } from "../../ui/Button";
import { Avatar } from "../../ui/Character";
import { Chips } from "../../ui/Chips";
import { Center, Form, FormRow, TextInput } from "../../ui/Form";
import { Icon } from "../../ui/Icon";
import { openModal } from "../../ui/Modal";
import { DEFAULT_WEIGHTS, showScoreWeights } from "../../ui/ScoreWeights";
import { toast } from "../../ui/Toast";
import { TopBar } from "../../ui/TopBar";
import s from "./Lobby.module.css";
import { t as tr } from "../../i18n/t";

type LobbyFilter = "all" | "ranked" | "casual" | "joinable";
const FILTERS: LobbyFilter[] = ["all", "ranked", "casual", "joinable"];
const filterLabel = (f: LobbyFilter) => ({ all: tr("common.all"), ranked: tr("lobby.filterRanked"), casual: tr("lobby.filterCasual"), joinable: tr("lobby.filterJoinable") }[f]);
const joinable = (r: RoomInfo) => !r.playing && r.members.length < r.maxPlayers;

/** Make sure this tab has a server session for the current profile. */
export async function connect(): Promise<boolean> {
  const p = getProfile();
  const home = D.character(p.homeCharacter);
  return ensureSession(p.playerName, home?.name ?? "", home ? D.artId(home) : "");
}

function enter(room: RoomInfo, you: number): void {
  startOnline(room, you);
  navigate({ name: "room", id: room.id });
}

async function join(id: string, locked: boolean, password = ""): Promise<void> {
  if (locked && !password) {
    openModal(tr("lobby.passwordTitle"), (close) => <PasswordForm onDone={(pw) => { close(); void join(id, true, pw); }} />, { size: "small" });
    return;
  }
  const r = await api.join(id, password);
  if (!r.ok) return toast(fmtMsg(r.error!), "error");
  enter(r.data!.room, r.data!.you);
}

function PasswordForm({ onDone }: { onDone: (pw: string) => void }) {
  const [pw, setPw] = useState("");
  return (
    <Form onSubmit={() => onDone(pw)}>
      <TextInput placeholder={tr("lobby.roomPassword")} value={pw} onChange={(e) => setPw(e.target.value)} autoFocus />
      <Center><Btn kind="pink" type="submit" wide>{tr("lobby.join")}</Btn></Center>
    </Form>
  );
}

export function Lobby() {
  const [status, setStatus] = useState<"connecting" | "ok" | "offline">("connecting");
  const [rooms, setRooms] = useState<RoomInfo[]>([]);
  const [error, setError] = useState("");
  const [filter, setFilter] = useState<LobbyFilter>("all");
  const [code, setCode] = useState("");

  useEffect(() => {
    let alive = true;
    let t = 0;
    void (async () => {
      if (!(await connect())) return alive && setStatus("offline");
      if (!alive) return;
      // Still seated in a room (e.g. came back with the browser's back button): return to it.
      if (session instanceof OnlineSession && !session.dissolved) return navigate({ name: "room", id: session.id }, { replace: true });
      setStatus("ok");
      const refresh = async () => {
        const r = await api.rooms();
        if (!alive) return;
        setError(r.ok ? "" : fmtMsg(r.error ?? { k: "err.offline" }));
        setRooms(r.data ?? []);
      };
      await refresh();
      t = window.setInterval(refresh, 3000);
    })();
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, []);

  const shown = rooms.filter((r) => filter === "all" || (filter === "ranked" ? r.ranked : filter === "casual" ? !r.ranked : joinable(r)));
  const quickJoin = () => {
    const r = rooms.find((x) => joinable(x) && !x.locked);
    if (r) return void join(r.id, false);
    toast(tr("lobby.noJoinable"));
    showCreateRoom();
  };

  return (
    <>
      <TopBar section={tr("menu.online")} title={tr("lobby.title")} onBack={() => navigate({ name: "menu" })} />
      {status === "offline" ? (
        <div className={s.area}><Empty title={tr("lobby.offlineTitle")} text={tr("lobby.offlineText")} /></div>
      ) : (
        <>
          <div className={s.area}>
            <div className={s.filters}>
              <span className={s.listLabel}><Icon name="groups" />{tr("lobby.roomList")}</span>
              <Chips items={FILTERS.map(filterLabel)} on={filterLabel(filter)} onPick={(l) => setFilter(FILTERS.find((f) => filterLabel(f) === l) ?? "all")} className={s.chips} />
            </div>
            {error ? <Empty title={tr("lobby.offlineTitle")} text={error} />
              : shown.length ? <div className={s.grid}>{shown.map((r) => <RoomCard key={r.id} r={r} />)}</div>
              : <Empty title={status === "connecting" ? tr("lobby.loading") : tr("lobby.emptyTitle")} text={tr("lobby.emptyText")} />}
          </div>
          <div className={s.foot}>
            <form className={s.joinPill} onSubmit={(e) => { e.preventDefault(); if (code.trim()) void join(code.trim().toUpperCase(), false); }}>
              <span>{tr("lobby.roomId")}</span>
              <TextInput placeholder={tr("lobby.roomIdPlaceholder")} maxLength={6} value={code} onChange={(e) => setCode(e.target.value)} className={s.joinInput} />
              <Btn kind="pink" type="submit" className={s.joinBtn}>{tr("lobby.join")}</Btn>
            </form>
            <button type="button" className={cx(s.bigTile, s.blue)} onClick={quickJoin}><img src={sceneImg("pic_single")} alt="" /><span>{tr("lobby.quickJoin")}</span></button>
            <button type="button" className={cx(s.bigTile, s.pink)} onClick={showCreateRoom}><img src={sceneImg("pic_group")} alt="" /><span>{tr("lobby.createRoom")}</span></button>
          </div>
        </>
      )}
    </>
  );
}

function Empty({ title, text }: { title: string; text: string }) {
  return (
    <div className={s.empty}>
      <img src={sceneImg("pic_group")} alt="" />
      <b>{title}</b>
      <span>{text}</span>
    </div>
  );
}

function RoomCard({ r }: { r: RoomInfo }) {
  const host = r.members.find((m) => m.host);
  return (
    <button type="button" className={cx(s.card, r.ranked && s.ranked, r.playing && s.playing)} onClick={() => void join(r.id, r.locked)}>
      <div className={s.cardHead}>
        <span className={cx(s.mode, r.ranked && s.modeRanked)}>{r.ranked ? tr("common.rankShort") : tr("common.casualShort")}</span>
        <b>{r.name}</b>
        {r.locked && <Icon name="lock" />}
      </div>
      <div className={s.members}>
        {r.members.slice(0, 6).map((m) => <Avatar key={m.id} c={D.character(m.character)} size={40} />)}
        {r.members.length > 6 && <span>+{r.members.length - 6}</span>}
      </div>
      <div className={s.cardFoot}>
        <span>{tr("lobby.host", { who: host?.player ?? "—", id: r.id })}</span>
        <b>{r.members.length}/{r.maxPlayers}</b>
        <span className={s.state}>{r.playing ? tr("lobby.playing") : tr("lobby.waiting")}</span>
      </div>
    </button>
  );
}

function CreateRoom({ close }: { close: () => void }) {
  const p = getProfile();
  const [name, setName] = useState(tr("lobby.roomNameDefault", { who: p.playerName }));
  const [ranked, setRanked] = useState(false);
  const [max, setMax] = useState(6);
  const [pw, setPw] = useState("");
  const [weights, setWeights] = useState<ScoreWeights>({ ...DEFAULT_WEIGHTS });
  const [lo, hi] = ranked ? [5, 6] : [3, 10];
  const submit = async () => {
    const r = await api.createRoom({ name, ranked, maxPlayers: max, password: pw, weights });
    if (!r.ok) return toast(fmtMsg(r.error!), "error");
    close();
    enter(r.data!.room, r.data!.you);
  };
  return (
    <Form onSubmit={submit}>
      <FormRow label={tr("lobby.roomName")}><TextInput value={name} maxLength={24} onChange={(e) => setName(e.target.value)} /></FormRow>
      <FormRow label={tr("lobby.mode")}>
        <Chips items={[tr("mode.casual"), tr("mode.ranked")] as const} on={ranked ? tr("mode.ranked") : tr("mode.casual")} onPick={(m) => { const rk = m === tr("mode.ranked"); setRanked(rk); if (rk) setMax(Math.min(6, Math.max(5, max))); }} />
      </FormRow>
      <FormRow label={tr("lobby.maxPlayers")}>
        <span className={s.stepper}>
          <RoundBtn icon="remove" disabled={max <= lo} onClick={() => setMax(max - 1)} />
          <b>{tr("common.playersN", { n: max })}</b>
          <RoundBtn icon="add" disabled={max >= hi} onClick={() => setMax(max + 1)} />
          <small>{ranked ? tr("lobby.playersHintRanked") : tr("solo.playersHint")}</small>
        </span>
      </FormRow>
      <FormRow label={tr("lobby.password")}><TextInput placeholder={tr("lobby.passwordPlaceholder")} maxLength={16} value={pw} onChange={(e) => setPw(e.target.value)} /></FormRow>
      <FormRow label={tr("common.score")}><span><Btn size="small" icon="leaderboard" onClick={() => showScoreWeights(weights, true, setWeights)}>{tr("solo.scoreRules")}</Btn></span></FormRow>
      <Center><Btn kind="pink" type="submit" wide>{tr("lobby.create")}</Btn></Center>
    </Form>
  );
}

function showCreateRoom(): void {
  openModal(tr("lobby.createRoom"), (close) => <CreateRoom close={close} />, { size: "mid" });
}
