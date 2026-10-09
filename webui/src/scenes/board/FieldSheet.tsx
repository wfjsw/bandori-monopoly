// The cards in play, as a top sheet at the top of the centre column -- the
// mirror of the prompt sheet's bottom one. A slim strip (title + count) by
// default so it never covers the board; hovering / clicking / focusing unfolds
// the card row over the board's upper edge and it folds away again after a
// short idle. Per-card info is the shared one: owner colour ring, crystal /
// CP badges, the live note, the floating hover preview and click-to-inspect.

import { useState } from "react";
import { useTimeout } from "../../hooks/timers";
import { sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D } from "../../core/data";
import { namesOf } from "../../core/names";
import { fmtMsg } from "../../i18n/msg";
import type { Model } from "./model";
import { InspectCard } from "../../ui/Card";
import { showEvent, showField } from "./Popups";
import { t as tr } from "../../i18n/t";
import s from "./FieldSheet.module.css";

/**
 * A fold-away panel: opens on hover / focus / click and folds after a short
 * idle (or when the pointer leaves). The one behaviour the field strip needs,
 * in a named hook instead of scattered timeouts.
 */
function useFoldPanel(leaveMs = 220, holdMs = 2400): {
  open: boolean;
  toggle: () => void;
  hover: { onMouseEnter: () => void; onMouseLeave: () => void; onFocus: () => void; onBlur: () => void };
} {
  // `closeIn` is the fold delay to arm (null = park open). The timer is keyed
  // off `open`, so every reopen arms afresh.
  const [plan, setPlan] = useState<{ open: boolean; closeIn: number | null }>({ open: false, closeIn: null });
  useTimeout(() => setPlan((p) => (p.open ? { ...p, open: false } : p)), plan.open ? plan.closeIn : null);
  const openFor = (ms: number | null) => setPlan({ open: true, closeIn: ms });
  return {
    open: plan.open,
    toggle: () => setPlan((p) => (p.open ? { open: false, closeIn: null } : { open: true, closeIn: holdMs })),
    hover: {
      onMouseEnter: () => openFor(null),
      onMouseLeave: () => openFor(leaveMs),
      onFocus: () => openFor(null),
      onBlur: () => openFor(leaveMs),
    },
  };
}

export function FieldSheet({ m }: { m: Model }) {
  const S = m.S;
  const names = namesOf(S);
  const panel = useFoldPanel();
  // Skill rules (`skill:`) are engine plumbing on the field, not cards in play.
  const rows = S.players
    .map((x, i) => [i, (x.field ?? []).filter((c) => !c.card.startsWith("skill:"))] as const)
    .filter(([, f]) => f.length);
  const fieldCount = rows.reduce((a, [, f]) => a + f.length, 0);
  const events = S.eventActive ?? [];
  const total = fieldCount + events.length;
  return (
    <div className={cx(s.sheet, panel.open && s.open)} {...panel.hover}>
      {/* One drawer: the card tray unfolds above its own handle row, so the
          strip and the cards read as a single sheet pulled down from the top. */}
      <div className={s.drawer}>
      <div className={s.panel}>
        {total > 0 ? (
          <div className={s.cards} data-vp-scroll>
            {rows.flatMap(([i, f]) => f.map((fc) => {
              const note = fc.note ? fmtMsg(fc.note, names) : "";
              const color = m.colorOf(i);
              const live = [
                tr("board.fieldOwner", { who: m.nameOf(i) }),
                note,
                fc.crystals ? tr("board.crystals", { n: fc.crystals }) : "",
                fc.cp > 0 ? tr("board.cp", { n: fc.cp }) : "",
              ].filter(Boolean).join(" · ");
              return fc.faceDown ? (
                <div key={fc.uid} className={s.back} style={{ ["--own" as string]: color }} title={`${m.nameOf(i)} · ${tr("board.faceDown")}`}><img src={sceneImg("card_back")} alt="" /></div>
              ) : (
                <InspectCard
                  key={fc.uid}
                  id={fc.card}
                  size="hand"
                  className={s.card}
                  style={{ ["--own" as string]: color }}
                  note={live}
                  title={D.card(fc.card)?.name}
                  actions={[]}
                >
                  {fc.crystals > 0 && <span className={s.crystal}>◆{fc.crystals}</span>}
                  {fc.cp > 0 && <span className={s.cp} title={tr("board.cp", { n: fc.cp })}>CP{fc.cp}</span>}
                  {!!note && <span className={s.noteDot} />}
                </InspectCard>
              );
            }))}
            {events.map((e) => {
              const ev = D.event(e.id);
              const note = e.note?.k ? fmtMsg(e.note, names) : "";
              return e.faceDown ? (
                <div key={e.id} className={s.back} title={tr("board.faceDown")}><img src={sceneImg("card_back")} alt="" /></div>
              ) : (
                <button key={e.id} type="button" className={s.event} title={tr("board.activeEvents")} onClick={() => showEvent(e.id, note)}>
                  <img src={sceneImg("card_back")} alt="" />
                  <span>{ev?.name ?? e.id}</span>
                  {e.counter > 0 && <span className={s.crystal}>×{e.counter}</span>}
                </button>
              );
            })}
          </div>
        ) : (
          <div className={s.empty}>{tr("common.empty")}</div>
        )}
      </div>
      <div className={s.strip}>
        <button type="button" className={s.toggle} onClick={panel.toggle} aria-expanded={panel.open} title={tr("board.field")}>
          <b>{tr("board.field")}</b>
          <small>×{total}</small>
          <span className={cx(s.chev, panel.open && s.chevOpen)} aria-hidden />
        </button>
        {panel.open && <button type="button" className={s.more} onClick={() => showField(m)}>{tr("common.all")}</button>}
      </div>
      </div>
    </div>
  );
}