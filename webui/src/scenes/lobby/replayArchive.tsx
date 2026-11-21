// The room's durable replay archive (`docs/SERVER.md` §Match records): every
// finished match the caller sat in, newest first, each downloadable. The
// store's "last replay" button is the shortcut; this is where the older ones
// live.

import { useEffect, useState } from "react";
import { downloadRecordPrompt } from "../../game/downloadPrompt";
import { fmtMsg } from "../../i18n/msg";
import { t as tr } from "../../i18n/t";
import { api, type ArchivedMatch } from "../../net/api";
import { Btn } from "../../ui/Button";
import { openModal } from "../../ui/Modal";
import { toast } from "../../ui/Toast";
import s from "./Room.module.css";

/** Open the archive list for `room`. */
export function showReplayArchive(room: string): void {
  openModal(tr("room.replayArchive"), (close) => <Archive room={room} close={close} />, {
    size: "mid",
    key: "replay-archive",
  });
}

function Archive({ room, close }: { room: string; close: () => void }) {
  const [items, setItems] = useState<ArchivedMatch[] | null>(null);
  const [why, setWhy] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    void api.records(room).then((r) => {
      if (!alive) return;
      if (r.ok) setItems(r.data!.records);
      else {
        setItems([]);
        setWhy(fmtMsg(r.error!));
      }
    });
    return () => {
      alive = false;
    };
  }, [room]);

  const download = async (m: ArchivedMatch) => {
    const r = await api.archivedRecord(room, m.id);
    if (!r.ok) return toast(fmtMsg(r.error!), "error");
    downloadRecordPrompt(r.bytes, r.filename);
    close();
  };

  return (
    <div className={s.archive}>
      {items === null && <p className={s.archiveNote}>{tr("room.replayLoading")}</p>}
      {items?.length === 0 && <p className={s.archiveNote}>{why ?? tr("room.replayArchiveEmpty")}</p>}
      {items?.map((m) => (
        <div key={m.id} className={s.archiveRow}>
          <div className={s.archiveHead}>
            <b>{m.created}</b>
            <span>{tr("room.replayRounds", { n: m.rounds })}</span>
          </div>
          <div className={s.archiveSeats}>
            {m.seats.map((seat) => (
              <span key={seat.member}>
                {seat.player}
                <small>{seat.score}</small>
              </span>
            ))}
          </div>
          <Btn size="small" icon="download" onClick={() => void download(m)}>
            {tr("replay.download")}
          </Btn>
        </div>
      ))}
    </div>
  );
}
