// The replay list: the last few solo records kept in IndexedDB, plus "open
// file" and drag-and-drop for a `.bdrec` from disk. Each row can be watched,
// downloaded or deleted; the badge previews whether the current engine can
// play it (`compat`'s rules over `engine_stamp` vs the record's stamp).

import { useEffect, useRef, useState } from "react";
import { navigate } from "../../app/router";
import { D } from "../../core/data";
import { cx } from "../../core/cx";
import {
  deleteReplay,
  downloadRecord,
  getReplay,
  readRecordFile,
  type Mismatch,
  type RecordHeader,
  type ReplayEntry,
  listReplays,
  REPLAY_KEEP,
} from "../../game/record";
import { queueReplayBytes, stampMismatches } from "../../game/replay";
import { modeName } from "../board/model";
import { Btn } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { TopBar } from "../../ui/TopBar";
import { toast } from "../../ui/Toast";
import { ask } from "../../ui/Modal";
import s from "./Replay.module.css";
import { t as tr } from "../../i18n/t";

const mentalityName = (m: string) => (m === "chaos" ? tr("solo.mentalityChaos") : tr("solo.mentalityStandard"));

function seatsText(h: RecordHeader): string {
  return h.seats
    .map((x) => {
      const who = D.character(x.character)?.display ?? x.player;
      return x.bot ? `${who}（${mentalityName(x.mentality)}）` : who;
    })
    .join("、");
}

function winnerText(h: RecordHeader): string {
  const w = h.seats.find((x) => x.rank === 1) ?? h.seats[0];
  return w ? (D.character(w.character)?.display ?? w.player) : "—";
}

/** `compat`'s table: format / abi fatal, the rest warnings. */
function badgeOf(h: RecordHeader): { cls: string; label: string } {
  const mis: Mismatch[] = stampMismatches(h);
  if (mis.some((x) => x.fatal)) return { cls: s.badFatal, label: tr("replay.compatFatal") };
  if (mis.length) return { cls: s.badWarn, label: tr("replay.compatWarn") };
  return { cls: s.badOk, label: tr("replay.compatOk") };
}

export function Replays() {
  const [entries, setEntries] = useState<ReplayEntry[] | null>(null);
  const [drag, setDrag] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const reload = () => {
    void listReplays()
      .then(setEntries)
      .catch((e) => {
        console.warn("replay list:", e);
        setEntries([]);
      });
  };
  useEffect(reload, []);

  const openBytes = async (bytes: Uint8Array, id = "") => {
    try {
      queueReplayBytes(bytes, id);
      navigate({ name: "replayView" });
    } catch (e) {
      console.warn(e);
      toast(tr("replay.badFile"), "error");
    }
  };
  const openFiles = async (files: FileList | null) => {
    const f = files?.[0];
    if (!f) return;
    try {
      queueReplayBytes(await readRecordFile(f));
      navigate({ name: "replayView" });
    } catch (e) {
      console.warn(e);
      toast(tr("replay.badFile"), "error");
    }
  };

  return (
    <>
      <TopBar
        section={tr("replay.section")}
        title={tr("replay.title")}
        onBack={() => navigate({ name: "menu" })}
        right={<></>}
      />
      <input
        ref={fileRef}
        type="file"
        accept=".bdrec,.json,application/json"
        style={{ display: "none" }}
        onChange={(e) => {
          void openFiles(e.target.files);
          e.target.value = "";
        }}
      />
      <div
        className={cx(s.page, drag && s.dragging)}
        onDragOver={(e) => {
          e.preventDefault();
          setDrag(true);
        }}
        onDragLeave={() => setDrag(false)}
        onDrop={(e) => {
          e.preventDefault();
          setDrag(false);
          void openFiles(e.dataTransfer.files);
        }}
      >
        {drag && <div className={s.drop}>{tr("replay.dropHint")}</div>}
        {/* The list's own header: what it holds, and where a file comes in
            (button or drop) -- next to the list it adds to. */}
        {!!entries?.length && (
          <div className={s.head}>
            <b>{tr("replay.saved", { n: entries.length, max: REPLAY_KEEP })}</b>
            <span className={s.headHint}>{tr("replay.dropHint")}</span>
            <Btn kind="pink" size="small" icon="add" onClick={() => fileRef.current?.click()}>{tr("replay.openFile")}</Btn>
          </div>
        )}
        {!entries?.length ? (
          <div className={s.empty}>
            <Icon name="hourglass" size={48} />
            <p>{tr("replay.empty")}</p>
            <Btn kind="pink" icon="add" onClick={() => fileRef.current?.click()}>{tr("replay.openFile")}</Btn>
          </div>
        ) : (
          <div className={s.list}>
            {entries.map((e) => (
              <Row
                key={e.id}
                entry={e}
                onWatch={() => void (async () => {
                  const bytes = await getReplay(e.id);
                  if (bytes) void openBytes(bytes, e.id);
                  else toast(tr("replay.loadFailed"), "error");
                })()}
                onDownload={() => void (async () => {
                  const bytes = await getReplay(e.id);
                  if (bytes) downloadRecord(bytes, `bdrec-${e.id}.bdrec`);
                  else toast(tr("replay.loadFailed"), "error");
                })()}
                onDelete={() => void (async () => {
                  if (!(await ask(tr("replay.title"), tr("replay.confirmDelete")))) return;
                  await deleteReplay(e.id);
                  reload();
                })()}
              />
            ))}
          </div>
        )}
      </div>
    </>
  );
}

function Row({ entry, onWatch, onDownload, onDelete }: { entry: ReplayEntry; onWatch: () => void; onDownload: () => void; onDelete: () => void }) {
  const h = entry.header;
  const badge = badgeOf(h);
  return (
    <div className={s.row}>
      <div className={s.when}>
        <b>{h.created.replace("T", " ").replace(/\.\d+Z$/, "")}</b>
        <small>{modeName(h.mode)} · {tr("replay.rounds", { n: h.rounds })}{h.partial ? ` · ${tr("replay.partial")}` : ""}</small>
      </div>
      <div className={s.who}>
        <small>{tr("replay.seats")}</small>
        <span>{seatsText(h)}</span>
        <small>{tr("replay.winner")}</small>
        <span>{winnerText(h)}</span>
      </div>
      <span className={cx(s.badge, badge.cls)}>{badge.label}</span>
      <div className={s.acts}>
        <Btn size="small" kind="pink" onClick={onWatch}>{tr("replay.watch")}</Btn>
        <Btn size="small" onClick={onDownload}>{tr("replay.download")}</Btn>
        <Btn size="small" onClick={onDelete}>{tr("replay.delete")}</Btn>
      </div>
    </div>
  );
}