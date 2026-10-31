// Face-down cards parked on a field: still hand cards, only out of reach for a
// while (宇田川亚子's skill puts them down). They therefore sit as one dense
// stack at the right end of the hand -- the mirror of the skill stack on its
// left -- and rise / retract with the hand, never in the cards-in-play row.
//
// Everywhere else's face-down convention holds: everyone but the owner sees the
// card back; the owner keeps the face, the 盖着 tag and a back that blends over
// it now and then. Hover / focus shows the shared preview; a press-and-hold on
// one that binds tiles lights them; a click opens its detail sheet.

import { memo, useCallback, useState, type CSSProperties } from "react";
import { sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D } from "../../core/data";
import { namesOf } from "../../core/names";
import { holdPeek } from "../../hooks/hold";
import { PICK_ATTR, usePickDeselect } from "../../hooks/pick";
import { fmtMsg } from "../../i18n/msg";
import { t as tr } from "../../i18n/t";
import { Avatar } from "../../ui/Character";
import { pickCard, type CardPill } from "../../ui/CardPreview";
import { setHoverCardTiles, useHoverSeat } from "./hoverSeat";
import { FieldFace } from "./FieldSheet";
import { designatedTiles, fieldCardTiles } from "./fieldTiles";
import { fieldMarkers, markerNote } from "./markers";
import type { Model } from "./model";
import { SKILL_CARD_W, SKILL_FAN_GAP } from "../../styles/layout";
import fd from "./FaceDown.module.css";
import fs from "./FieldSheet.module.css";
import s from "./Side.module.css";

/** The widest step between the faces' left edges and the stack's widest span:
 *  the dense overlap keeps a long stack inside the column. */
export const DOWN_STEP_MAX = 28;
export const DOWN_MAX = 240;

/** The step between the faces' left edges for `n` cards (0 for a lone one). */
export const downStep = (n: number) => (n > 1 ? Math.min(DOWN_STEP_MAX, (DOWN_MAX - SKILL_CARD_W) / (n - 1)) : 0);

/** The width the stack takes for `n` cards, for the hand's spread budget. */
export const downStackWidth = (n: number) => (n ? SKILL_CARD_W + (n - 1) * downStep(n) + SKILL_FAN_GAP : 0);

export const FaceDownStack = memo(function FaceDownStack({ m, pulse }: { m: Model; pulse?: { uid: number; id: number; ms: number } | null }) {
  const S = m.S;
  const names = namesOf(S);
  const hoverSeat = useHoverSeat();
  const [picked, setPicked] = useState<string | null>(null);
  const unpick = useCallback(() => setPicked(null), []);
  usePickDeselect(picked, unpick);
  const present = useCallback((key: string, id: string, note: string, pills?: CardPill[]) => {
    setPicked(key);
    pickCard(id, note, pills);
  }, []);
  const dismiss = useCallback(() => setPicked(null), []);
  const mine = (ownerSeat: number) => ownerSeat >= 0 && ownerSeat === m.playerId;
  const down = S.players.flatMap((x, seat) => (x.field ?? []).filter((fc) => fc.faceDown).map((fc) => ({ seat, fc })));
  if (!down.length) return null;
  const step = downStep(down.length);
  return (
    <div className={s.downStack} style={{ ["--down-step" as string]: `${step.toFixed(1)}px` }} title={tr("board.faceDownStack")}>
      {down.map(({ seat, fc }) => {
        // The frame takes the owner's colour, like every field instance's.
        const owner = fc.owner >= 0 ? fc.owner : seat;
        const own = mine(owner);
        // A card parked on someone else's field names the seat it sits on.
        const designee = seat !== owner ? seat : -1;
        const markers = fieldMarkers(fc);
        const note = markerNote(fc.note ? fmtMsg(fc.note, names) : "", markers);
        const tiles = designatedTiles(S.marks, fieldCardTiles(fc.uid));
        const key = `c${fc.uid}`;
        const name = D.card(fc.card)?.name;
        return (
          <div
            key={key}
            {...{ [PICK_ATTR]: key }}
            className={cx(s.downSlot, pulse?.uid === fc.uid && s.pulse, hoverSeat >= 0 && s[owner === hoverSeat ? "hot" : "dim"])}
            {...(tiles.length ? holdPeek(() => setHoverCardTiles(new Set(tiles)), () => setHoverCardTiles(null)) : {})}
            style={{ ["--own" as string]: m.colorOf(owner), ...(pulse?.uid === fc.uid ? { ["--pulse-ms" as string]: `${pulse.ms}ms` } : {}) } as CSSProperties}
          >
            {own ? (
              <>
                <span className={cx(fd.tag, s.downTagRight)}>{tr("board.faceDownTag")}</span>
                <FieldFace
                  slotKey={key}
                  card={fc.card}
                  title={`${name ?? fc.card} · ${tr("board.faceDown")}`}
                  note={note}
                  markers={markers}
                  owner={owner}
                  designee={designee}
                  m={m}
                  on={picked === key}
                  present={present}
                  dismiss={dismiss}
                />
                <div className={fd.blend} title={tr("board.faceDown")}><img src={sceneImg("card_back")} alt="" /></div>
              </>
            ) : (
              <div className={s.downBack} title={`${m.nameOf(seat)} · ${tr("board.faceDown")}`}><img src={sceneImg("card_back")} alt="" /></div>
            )}
            {designee >= 0 && <Avatar c={m.charOf(designee)} size={28} className={fs.holder} />}
          </div>
        );
      })}
    </div>
  );
});
