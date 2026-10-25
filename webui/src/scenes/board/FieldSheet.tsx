// The cards in play, docked at the top edge of the centre column -- the mirror
// of the hand's bottom fan. A straight overlapping row of full card faces (no
// circular sector), retracted upward behind the top edge so each face's bottom
// zone -- the name and the live markers -- peeks below it. Hovering / focusing
// the row lowers it into view and it retracts again after a short idle;
// hovering one card drops that one further and brings it to the front. Per-card
// info is the shared one: the owner's colour on the frame, the standing hover
// preview, click-to-inspect.
//
// Touch splits that in two (the same hold-vs-tap table the draw-pile chip
// uses): a short tap presents the card out, highlights it and feeds the
// standing preview -- it never lights tiles; a press-and-hold on a card that
// actually binds tiles (「指定」) lights those and fades the rest, and lifting
// the finger is not a tap. A card with no tile binding has no hold effect.

import { memo, useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { useDockRaise } from "../../hooks/dock";
import { holdPeek } from "../../hooks/hold";
import { PICK_ATTR, usePickDeselect } from "../../hooks/pick";
import { sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D } from "../../core/data";
import { namesOf } from "../../core/names";
import { fmtMsg } from "../../i18n/msg";
import type { Model } from "./model";
import { InspectCard, showCard } from "../../ui/Card";
import { pickCard, previewFrom, previewHide, type CardPill } from "../../ui/CardPreview";
import { Avatar } from "../../ui/Character";
import { showEvent } from "./Popups";
import { setHoverCardTiles, useHoverSeat } from "./hoverSeat";
import { designatedTiles, eventTiles, fieldCardTiles } from "./fieldTiles";
import { t as tr } from "../../i18n/t";
import { designBox } from "../../app/stageScale";
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
    const ro = new ResizeObserver(() => setWidth(designBox(el).width));
    ro.observe(el);
    setWidth(designBox(el).width);
    return () => ro.disconnect();
  }, [any]);
  // Touch screens: a short tap presents the card out, highlights it and feeds
  // the standing preview (like the hand's first click). A second tap on the
  // same card opens its detail popup. A tap never lights tiles. Clicking
  // anywhere else drops the pick again -- it is a lens, not a mode.
  const [picked, setPicked] = useState<string | null>(null);
  const unpick = useCallback(() => setPicked(null), []);
  usePickDeselect(picked, unpick);
  // Skill rules (`skill:`) are engine plumbing on the field, not cards in play.
  const rows = S.players
    .map((x, i) => [i, (x.field ?? []).filter((c) => !c.card.startsWith("skill:"))] as const)
    .filter(([, f]) => f.length);
  const events = S.eventActive ?? [];
  /** Hover (mouse) or press-and-hold (touch) lights the card's tiles; a tap does not. */
  const tileHold = (tiles: readonly number[]) =>
    tiles.length
      ? holdPeek(
          () => setHoverCardTiles(new Set(tiles)),
          () => setHoverCardTiles(null),
        )
      : {};
  /** The coarse-pointer first tap: present, highlight, preview. */
  const present = (key: string, id: string, note: string, pills?: CardPill[]) => {
    setPicked(key);
    pickCard(id, note, pills);
  };
  const coarse = () => window.matchMedia("(pointer: coarse)").matches;
  const total = rows.reduce((a, [, f]) => a + f.length, 0) + events.length;
  if (!total) return null;
  // A picked card keeps the row out from under the top edge until it is unpicked.
  return (
    <div ref={sheetRef} className={cx(s.sheet, (dock.raised || picked) && s.up)} {...dock.hover}>
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
          // Hold-to-highlight only when this instance actually designates tiles.
          const tiles = designatedTiles(S.marks, fieldCardTiles(fc.uid));
          const key = `c${fc.uid}`;
          return (
            <div key={fc.uid} {...{ [PICK_ATTR]: key }} className={cx(s.slot, picked === key && s.picked, pulse?.uid === fc.uid && s.pulse, hoverSeat >= 0 && s[owner === hoverSeat ? "hot" : "dim"])} {...tileHold(tiles)} style={{ ["--own" as string]: m.colorOf(owner), ...(pulse?.uid === fc.uid ? { ["--pulse-ms" as string]: `${pulse.ms}ms` } : {}) }}>
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
                  on={picked === key}
                  onClick={() => {
                    if (coarse() && picked !== key) return void present(key, fc.card, live, pills);
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
          const tiles = designatedTiles(S.marks, eventTiles(e.id));
          const key = `e${e.id}`;
          return (
            <div
              key={e.id}
              {...{ [PICK_ATTR]: key }}
              className={cx(s.slot, picked === key && s.picked, k === 0 && s.sep, hoverSeat >= 0 && s[e.playerId === hoverSeat ? "hot" : "dim"])}
              {...tileHold(tiles)}
              style={{ ["--own" as string]: e.playerId >= 0 ? m.colorOf(e.playerId) : "var(--purple)" }}
            >
              {e.faceDown ? (
                <div className={s.back} title={tr("board.faceDown")}><img src={sceneImg("card_back")} alt="" /></div>
              ) : (
                <button type="button" className={cx(s.event, picked === key && s.picked)} title={tr("board.activeEvents")} onClick={() => {
                    if (coarse() && picked !== key) return void present(key, e.id, note);
                    setPicked(null);
                    showEvent(e.id, note);
                  }}
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
