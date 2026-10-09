// The cards in play, docked at the top edge of the centre column -- the mirror
// of the hand's bottom fan. A straight overlapping row of full card faces (no
// circular sector), retracted upward behind the top edge so each face's bottom
// zone -- the name and the live markers -- peeks below it. Hovering / focusing
// the row lowers it into view and it retracts again after a short idle;
// hovering one card drops that one further and brings it to the front. Per-card
// info is the shared one: the owner's colour on the frame, the standing hover
// preview, click-to-inspect.

import { type CSSProperties } from "react";
import { useDockRaise } from "../../hooks/dock";
import { sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D } from "../../core/data";
import { namesOf } from "../../core/names";
import { fmtMsg } from "../../i18n/msg";
import type { Model } from "./model";
import { InspectCard } from "../../ui/Card";
import { showEvent } from "./Popups";
import { t as tr } from "../../i18n/t";
import s from "./FieldSheet.module.css";

/**
 * The straight row spread (`--row-slot`: the px step between card centres),
 * tightening with the count so the cards always fit the centre column. The
 * field row has no arc -- unlike the hand's `fanArc`.
 */
export function rowSpread(n: number): Record<string, string> {
  const slot = n > 1 ? Math.min(112, 480 / (n - 1)) : 0;
  return { "--row-slot": `${slot.toFixed(1)}px` };
}

export function FieldSheet({ m }: { m: Model }) {
  const S = m.S;
  const names = namesOf(S);
  const dock = useDockRaise();
  // Skill rules (`skill:`) are engine plumbing on the field, not cards in play.
  const rows = S.players
    .map((x, i) => [i, (x.field ?? []).filter((c) => !c.card.startsWith("skill:"))] as const)
    .filter(([, f]) => f.length);
  const events = S.eventActive ?? [];
  const total = rows.reduce((a, [, f]) => a + f.length, 0) + events.length;
  if (!total) return null;
  return (
    <div className={cx(s.sheet, dock.raised && s.up)} {...dock.hover}>
      <div className={s.row} style={rowSpread(total) as CSSProperties}>
        {rows.flatMap(([i, f]) => f.map((fc) => {
          // The frame takes the owner's colour (`fc.owner`), not the field it
          // happens to sit on -- a borrowed card still reads as its owner's.
          const owner = fc.owner >= 0 ? fc.owner : i;
          const note = fc.note ? fmtMsg(fc.note, names) : "";
          const live = [
            tr("board.fieldOwner", { who: m.nameOf(owner) }),
            note,
            fc.crystals ? tr("board.crystals", { n: fc.crystals }) : "",
            fc.cp > 0 ? tr("board.cp", { n: fc.cp }) : "",
          ].filter(Boolean).join(" · ");
          return (
            <div key={fc.uid} className={s.slot} style={{ ["--own" as string]: m.colorOf(owner) }}>
              {fc.faceDown ? (
                <div className={s.back} title={`${m.nameOf(i)} · ${tr("board.faceDown")}`}><img src={sceneImg("card_back")} alt="" /></div>
              ) : (
                <InspectCard
                  id={fc.card}
                  size="mini"
                  field
                  className={s.card}
                  title={D.card(fc.card)?.name}
                  note={live}
                  actions={[]}
                >
                  {fc.crystals > 0 && <span className={s.crystal}>◆{fc.crystals}</span>}
                  {fc.cp > 0 && <span className={s.cp} title={tr("board.cp", { n: fc.cp })}>CP{fc.cp}</span>}
                  {!!note && <span className={s.noteDot} title={note} />}
                </InspectCard>
              )}
            </div>
          );
        }))}
        {/* Events in effect ride the row's end as a separated group, with the
            same face shape (art, name, counters) and the same peek. */}
        {events.map((e, k) => {
          const ev = D.event(e.id);
          const note = e.note?.k ? fmtMsg(e.note, names) : "";
          return (
            <div
              key={e.id}
              className={cx(s.slot, k === 0 && s.sep)}
              style={{ ["--own" as string]: e.playerId >= 0 ? m.colorOf(e.playerId) : "var(--purple)" }}
            >
              {e.faceDown ? (
                <div className={s.back} title={tr("board.faceDown")}><img src={sceneImg("card_back")} alt="" /></div>
              ) : (
                <button type="button" className={s.event} title={tr("board.activeEvents")} onClick={() => showEvent(e.id, note)}>
                  <div className={s.eventArt}><img src={sceneImg("card_back")} alt="" /></div>
                  <div className={s.eventName}>{ev?.name ?? e.id}</div>
                  <div className={s.marks}>
                    {e.counter > 0 && <span className={s.count}>×{e.counter}</span>}
                    {e.counter2 > 0 && <span className={s.count}>×{e.counter2}</span>}
                    {!!note && <span className={s.noteDot} title={note} />}
                  </div>
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}