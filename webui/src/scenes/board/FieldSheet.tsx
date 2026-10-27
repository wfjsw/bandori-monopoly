// The cards in play -- field cards and events in effect as one stack -- docked
// at the top edge of the centre column: the mirror of the hand's bottom fan. A
// straight overlapping row of full card faces (no circular sector), retracted
// upward behind the top edge so each face's bottom zone -- the name and the
// live markers -- peeks below it. Hovering / focusing the row lowers it into
// view and it retracts again after a short idle; hovering one card drops that
// one further and brings it to the front. Every face wears the same on-field
// card style: an event is just another face in the stack, not a separate
// group. Per-card info is the shared one: the owner's colour on the frame, the
// standing hover preview, click-to-inspect.
//
// Touch splits that in two (the same hold-vs-tap table the draw-pile chip
// uses): a short tap presents the card out, highlights it and feeds the
// standing preview -- it never lights tiles; a press-and-hold on a card that
// actually binds tiles (「指定」) lights those and fades the rest, and lifting
// the finger is not a tap. A card with no tile binding has no hold effect.

import { memo, useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { useDockRaise } from "../../hooks/dock";
import { holdPeek } from "../../hooks/hold";
import { PICK_ATTR, usePickDeselect } from "../../hooks/pick";
import { useLatestRef } from "../../hooks/latest";
import { sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D } from "../../core/data";
import { namesOf } from "../../core/names";
import { fmtMsg } from "../../i18n/msg";
import type { Model } from "./model";
import { InspectCard, showCard, type CardAction } from "../../ui/Card";
import { pickCard, type CardPill } from "../../ui/CardPreview";
import { Avatar } from "../../ui/Character";
import { setHoverCardTiles, useHoverSeat } from "./hoverSeat";
import { designatedTiles, eventTiles, fieldCardTiles } from "./fieldTiles";
import { eventMarkers, fieldMarkers, markerNote, type Marker } from "./markers";
import { t as tr } from "../../i18n/t";
import { designBox } from "../../app/stageScale";
import s from "./FieldSheet.module.css";

/** Card face width (px), the widest step, and the row's horizontal breathing
 *  room. `PAD_X` keeps in sync with FieldSheet `.row`'s `padding-inline`: the
 *  owner ring (2.5px) and the holder avatar (`right: -6px`) hang past a face,
 *  and a picked / pulsed card scales up -- without the pad the last face's end
 *  is clipped at the sheet edge on a phone. */
const CARD_W = 120;
const SLOT_MAX = 112;
const PAD_X = 14;

/** Inspect-only field / event faces take no detail-sheet actions. */
const NO_ACTIONS: CardAction[] = [];

const coarse = () => window.matchMedia("(pointer: coarse)").matches;

/**
 * The straight row spread (`--row-slot`: the px step between card centres),
 * tightening with the count so the stack fits the sheet's `width` less `PAD_X`
 * on each side. Grown past that the faces overlap tighter -- the skill stack's
 * dense overlap -- and the stack still fills the usable width edge to edge
 * (maximum space, never a sideways scroll). `SLOT_MAX` keeps a near-full step
 * while they still fit. The field row has no arc -- unlike the hand's `fanArc`.
 */
export function rowSpread(n: number, width = 480 + CARD_W): Record<string, string> {
  const inner = width - 2 * PAD_X;
  const fit = n > 1 ? (inner - CARD_W) / (n - 1) : 0;
  const slot = n > 1 ? Math.min(SLOT_MAX, Math.max(0, fit)) : 0;
  return { "--row-slot": `${slot.toFixed(1)}px` };
}

/**
 * One field / event face: its own memo boundary, so a seat-hover dim, a pick
 * flip on a neighbour, or the sheet's width measure leaves the other faces
 * (and their `<img>`s) alone. Pills are derived here; the note and marker
 * chips come pre-built (`markers.ts`) because several surfaces print them.
 */
const FieldFace = memo(function FieldFace({ slotKey, card, title, note, markers, owner, designee, m, on, present, dismiss }: {
  /** The row's pick identity (`e…` / `c…`), shared with the wrapper div. */
  slotKey: string;
  /** The card / event id whose art this face shows. */
  card: string;
  title: string | undefined;
  note: string;
  /** The face's marker chips, in print order (`markers.ts`). */
  markers: Marker[];
  owner: number;
  /** Seat this instance is parked on for someone else, or -1. */
  designee: number;
  m: Model;
  on: boolean;
  present: (key: string, id: string, note: string, pills?: CardPill[]) => void;
  dismiss: () => void;
}) {
  const pills = useMemo((): CardPill[] => {
    const out: CardPill[] = [];
    if (owner >= 0) out.push({ label: tr("board.pillOwner", { who: m.nameOf(owner) }), color: m.colorOf(owner), char: m.charOf(owner) });
    if (designee >= 0) out.push({ label: tr("board.pillDesignee", { who: m.nameOf(designee) }), color: m.colorOf(designee), char: m.charOf(designee) });
    return out;
  }, [m, owner, designee]);
  const latest = useLatestRef({ slotKey, card, note, pills, on, present, dismiss });
  const onClick = useCallback(() => {
    const v = latest.current;
    // The coarse-pointer first tap: present, highlight, preview. A second tap
    // on the same card opens its detail popup. A tap never lights tiles.
    if (coarse() && !v.on) return void v.present(v.slotKey, v.card, v.note, v.pills);
    v.dismiss();
    void showCard(v.card, NO_ACTIONS, v.note, v.pills);
  }, [latest]);
  return (
    <InspectCard
      id={card}
      size="mini"
      field
      className={s.card}
      title={title}
      note={note}
      pills={pills}
      actions={NO_ACTIONS}
      on={on}
      onClick={onClick}
    >
      {markers.map((mk) => <span key={mk.key} className={s[mk.cls]} title={mk.note}>{mk.chip}</span>)}
    </InspectCard>
  );
});

export const FieldSheet = memo(function FieldSheet({ m, pulse }: { m: Model; pulse?: { uid: number; id: number; ms: number } | null }) {
  const S = m.S;
  const names = namesOf(S);
  const dock = useDockRaise();
  const hoverSeat = useHoverSeat();
  // The sheet's real width: on a phone the centre column is far narrower than
  // a desktop one, so the row spread follows it (and overlaps tighter when still too long).
  const sheetRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(480 + CARD_W);
  // The sheet stays mounted even when the row is empty, so the observer is
  // armed once -- not toggled with the stack's occupancy.
  useEffect(() => {
    const el = sheetRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => setWidth(designBox(el).width));
    ro.observe(el);
    setWidth(designBox(el).width);
    return () => ro.disconnect();
  }, []);
  // Touch screens: a short tap presents the card out, highlights it and feeds
  // the standing preview (like the hand's first click). A second tap on the
  // same card opens its detail popup. A tap never lights tiles. Clicking
  // anywhere else drops the pick again -- it is a lens, not a mode.
  const [picked, setPicked] = useState<string | null>(null);
  const unpick = useCallback(() => setPicked(null), []);
  usePickDeselect(picked, unpick);
  // Stable identities for the faces below: a pick flip then only repaints the
  // two faces whose `on` actually moved.
  const present = useCallback((key: string, id: string, note: string, pills?: CardPill[]) => {
    setPicked(key);
    pickCard(id, note, pills);
  }, []);
  const dismiss = useCallback(() => setPicked(null), []);
  // One stack, cards in play and events in effect alike. Skill rules (`skill:`)
  // are engine plumbing on the field, not cards in play.
  const stack = [
    ...S.players.flatMap((x, i) =>
      (x.field ?? [])
        .filter((c) => !c.card.startsWith("skill:"))
        .map((fc) => ({ kind: "card" as const, seat: i, fc })),
    ),
    ...(S.eventActive ?? []).map((e) => ({ kind: "event" as const, e })),
  ];
  /** Hover (mouse) or press-and-hold (touch) lights the card's tiles; a tap does not. */
  const tileHold = (tiles: readonly number[]) =>
    tiles.length
      ? holdPeek(
          () => setHoverCardTiles(new Set(tiles)),
          () => setHoverCardTiles(null),
        )
      : {};
  const total = stack.length;
  // Keep the sheet mounted through a transient empty row (the mirror of
  // SkillStack's cliff): an empty `.row` is zero-height and draws no phantom
  // bar, but unmounting the whole sheet mid-match would drop every face's
  // <img> and flash the field art the next time a card lands.
  //
  // A picked card keeps the row out from under the top edge until it is unpicked.
  return (
    <div ref={sheetRef} className={cx(s.sheet, (dock.raised || picked) && s.up)} {...dock.hover}>
      <div className={s.row} style={rowSpread(total, width) as CSSProperties}>
        {stack.map((it) => {
          if (it.kind === "event") {
            const e = it.e;
            const markers = eventMarkers(e);
            const note = markerNote(e.note?.k ? fmtMsg(e.note, names) : "", markers);
            const tiles = designatedTiles(S.marks, eventTiles(e.id));
            const key = `e${e.id}`;
            // An event's frame is its player's colour; a face with no player
            // (a neutral one) falls back to the event purple.
            const owner = e.playerId >= 0 ? e.playerId : -1;
            return (
              <div key={key} {...{ [PICK_ATTR]: key }} className={cx(s.slot, picked === key && s.picked, hoverSeat >= 0 && s[e.playerId === hoverSeat ? "hot" : "dim"])} {...tileHold(tiles)} style={{ ["--own" as string]: owner >= 0 ? m.colorOf(owner) : "var(--purple)" }}>
                {e.faceDown ? (
                  <div className={s.back} title={tr("board.faceDown")}><img src={sceneImg("card_back")} alt="" /></div>
                ) : (
                  <FieldFace
                    slotKey={key}
                    card={e.id}
                    title={D.event(e.id)?.name ?? e.id}
                    note={note}
                    markers={markers}
                    owner={owner}
                    designee={-1}
                    m={m}
                    on={picked === key}
                    present={present}
                    dismiss={dismiss}
                  />
                )}
              </div>
            );
          }
          const { seat, fc } = it;
          // The frame takes the owner's colour (`fc.owner`), not the field it
          // happens to sit on -- a borrowed card still reads as its owner's.
          const owner = fc.owner >= 0 ? fc.owner : seat;
          const markers = fieldMarkers(fc);
          const live = markerNote(fc.note ? fmtMsg(fc.note, names) : "", markers);
          // A card on its owner's own field designates no one else: only a card
          // parked on another player's field has a designee.
          const designee = seat !== owner ? seat : -1;
          // Hold-to-highlight only when this instance actually designates tiles.
          const tiles = designatedTiles(S.marks, fieldCardTiles(fc.uid));
          const key = `c${fc.uid}`;
          return (
            <div key={key} {...{ [PICK_ATTR]: key }} className={cx(s.slot, picked === key && s.picked, pulse?.uid === fc.uid && s.pulse, hoverSeat >= 0 && s[owner === hoverSeat ? "hot" : "dim"])} {...tileHold(tiles)} style={{ ["--own" as string]: m.colorOf(owner), ...(pulse?.uid === fc.uid ? { ["--pulse-ms" as string]: `${pulse.ms}ms` } : {}) }}>
              {fc.faceDown ? (
                <div className={s.back} title={`${m.nameOf(seat)} · ${tr("board.faceDown")}`}><img src={sceneImg("card_back")} alt="" /></div>
              ) : (
                <FieldFace
                  slotKey={key}
                  card={fc.card}
                  title={D.card(fc.card)?.name}
                  note={live}
                  markers={markers}
                  owner={owner}
                  designee={designee}
                  m={m}
                  on={picked === key}
                  present={present}
                  dismiss={dismiss}
                />
              )}
              {designee >= 0 && <Avatar c={m.charOf(designee)} size={28} className={s.holder} />}
            </div>
          );
        })}
      </div>
    </div>
  );
});