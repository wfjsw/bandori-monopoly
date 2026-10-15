// The cards in play, docked at the top edge of the centre column -- the mirror
// of the hand's bottom fan. A straight overlapping row of full card faces (no
// circular sector), retracted upward behind the top edge so each face's bottom
// zone -- the name and the live markers -- peeks below it. Hovering / focusing
// the row lowers it into view and it retracts again after a short idle;
// hovering one card drops that one further and brings it to the front. Per-card
// info is the shared one: the owner's colour on the frame, the standing hover
// preview, click-to-inspect.

import { memo, useEffect, useRef, useState, type CSSProperties } from "react";
import { useDockRaise } from "../../hooks/dock";
import { sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D } from "../../core/data";
import { namesOf } from "../../core/names";
import { fmtMsg } from "../../i18n/msg";
import type { Model } from "./model";
import { InspectCard, showCard } from "../../ui/Card";
import { previewFrom, previewHide } from "../../ui/CardPreview";
import { Avatar } from "../../ui/Character";
import { showEvent } from "./Popups";
import { setHoverCardTiles, useHoverSeat } from "./hoverSeat";
import { t as tr } from "../../i18n/t";
import s from "./FieldSheet.module.css";

/** Card face width (px) and the step bounds of the overlapped row. */
const CARD_W = 120;
const SLOT_MAX = 112;
const SLOT_MIN = 44;

/**
 * The straight row spread (`--row-slot`: the px step between card centres),
 * tightening with the count so the cards fit the sheet's `width`. Below
 * `SLOT_MIN` the cards stop overlapping further (they would be too thin to tap)
 * and the sheet scrolls sideways instead. The field row has no arc -- unlike
 * the hand's `fanArc`.
 */
export function rowSpread(n: number, width = 480 + CARD_W): Record<string, string> {
  const fit = n > 1 ? (width - CARD_W) / (n - 1) : 0;
  const slot = n > 1 ? Math.max(SLOT_MIN, Math.min(SLOT_MAX, fit)) : 0;
  return { "--row-slot": `${slot.toFixed(1)}px` };
}

export const FieldSheet = memo(function FieldSheet({ m, pulse }: { m: Model; pulse?: { uid: number; id: number; ms: number } | null }) {
  const S = m.S;
  const names = namesOf(S);
  const dock = useDockRaise();
  const hoverSeat = useHoverSeat();
  // The sheet's real width: on a phone the centre column is far narrower than
  // a desktop one, so the row spread follows it (and scrolls when still too long).
  const sheetRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(480 + CARD_W);
  const any = S.players.some((x) => (x.field ?? []).length) || (S.eventActive ?? []).length > 0;
  useEffect(() => {
    const el = sheetRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => setWidth(el.clientWidth));
    ro.observe(el);
    setWidth(el.clientWidth);
    return () => ro.disconnect();
  }, [any]);
  // Touch screens: the first tap only highlights a card (like the hand); a
  // second tap on the same card opens its detail popup.
  const [picked, setPicked] = useState<number | null>(null);
  // Skill rules (`skill:`) are engine plumbing on the field, not cards in play.
  const rows = S.players
    .map((x, i) => [i, (x.field ?? []).filter((c) => !c.card.startsWith("skill:"))] as const)
    .filter(([, f]) => f.length);
  const events = S.eventActive ?? [];
  // Hovering a card that designates tiles lights those and fades the rest.
  const hoverTiles = (uid: number) => {
    const ts = (S.marks ?? []).filter((x) => x.category === "designate" && (x.src === uid || x.instance === uid)).map((x) => x.tile);
    setHoverCardTiles(ts.length ? new Set(ts) : null);
  };
  const total = rows.reduce((a, [, f]) => a + f.length, 0) + events.length;
  if (!total) return null;
  return (
    <div ref={sheetRef} className={cx(s.sheet, dock.raised && s.up)} {...dock.hover}>
      <div className={s.row} style={rowSpread(total, width) as CSSProperties}>
        {rows.flatMap(([i, f]) => f.map((fc) => {
          // The frame takes the owner's colour (`fc.owner`), not the field it
          // happens to sit on -- a borrowed card still reads as its owner's.
          const owner = fc.owner >= 0 ? fc.owner : i;
          const note = fc.note ? fmtMsg(fc.note, names) : "";
          const live = [
            note,
            fc.counters["crystals"] ?? 0 ? tr("board.crystals", { n: fc.counters["crystals"] ?? 0 }) : "",
            fc.counters["cp"] ?? 0 > 0 ? tr("board.cp", { n: fc.counters["cp"] ?? 0 }) : "",
          ].filter(Boolean).join(" · ");
          // A card on its owner's own field designates no one else: only a card
          // parked on another player's field has a designee.
          const designee = i !== owner ? i : -1;
          const pills = [
            { label: tr("board.pillOwner", { who: m.nameOf(owner) }), color: m.colorOf(owner), char: m.charOf(owner) },
            ...(designee >= 0 ? [{ label: tr("board.pillDesignee", { who: m.nameOf(designee) }), color: m.colorOf(designee), char: m.charOf(designee) }] : []),
          ];
          return (
            <div key={fc.uid} className={cx(s.slot, pulse?.uid === fc.uid && s.pulse, hoverSeat >= 0 && s[owner === hoverSeat ? "hot" : "dim"])} onMouseEnter={() => hoverTiles(fc.uid)} onMouseLeave={() => setHoverCardTiles(null)} style={{ ["--own" as string]: m.colorOf(owner), ...(pulse?.uid === fc.uid ? { ["--pulse-ms" as string]: `${pulse.ms}ms` } : {}) }}>
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
                  pills={pills}
                  actions={[]}
                  on={picked === fc.uid}
                  onClick={() => {
                    if (window.matchMedia("(pointer: coarse)").matches && picked !== fc.uid) return void setPicked(fc.uid);
                    setPicked(null);
                    void showCard(fc.card, [], live, pills);
                  }}
                >
                  {(fc.counters["crystals"] ?? 0) > 0 && <span className={s.crystal}>◆{fc.counters["crystals"] ?? 0}</span>}
                  {(fc.counters["cp"] ?? 0) > 0 && <span className={s.cp} title={tr("board.cp", { n: fc.counters["cp"] ?? 0 })}>CP{fc.counters["cp"] ?? 0}</span>}
                </InspectCard>
              )}
              {designee >= 0 && <Avatar c={m.charOf(designee)} size={28} className={s.holder} />}
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
              className={cx(s.slot, k === 0 && s.sep, hoverSeat >= 0 && s[e.playerId === hoverSeat ? "hot" : "dim"])}
              style={{ ["--own" as string]: e.playerId >= 0 ? m.colorOf(e.playerId) : "var(--purple)" }}
            >
              {e.faceDown ? (
                <div className={s.back} title={tr("board.faceDown")}><img src={sceneImg("card_back")} alt="" /></div>
              ) : (
                <button type="button" className={s.event} title={tr("board.activeEvents")} onClick={() => showEvent(e.id, note)}
                  onMouseEnter={(x) => previewFrom(x.currentTarget, e.id, note)} onFocus={(x) => previewFrom(x.currentTarget, e.id, note)}
                  onMouseLeave={previewHide} onBlur={previewHide}>
                  <div className={s.eventArt}><img src={sceneImg("card_back")} alt="" /></div>
                  <div className={s.eventName}>{ev?.name ?? e.id}</div>
                  <div className={s.marks}>
                    {e.counter > 0 && <span className={s.count}>×{e.counter}</span>}
                    {e.counter2 > 0 && <span className={s.count}>×{e.counter2}</span>}
                  </div>
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});
