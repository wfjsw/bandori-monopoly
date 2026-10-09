// The enlarged card preview that follows a hover: art, title, tags and effect
// text, pointer-transparent so it never steals the hover from the thing being
// previewed. Shared by the hand, the field cards and the prompts -- one look,
// one behaviour (hover previews, click inspects).

import { cardArt } from "../core/assets";
import { cx } from "../core/cx";
import { cardColor, cardText, cardTitle, D } from "../core/data";
import { TagChip } from "./Card";
import { SkillBody } from "./SkillBody";
import s from "./CardPreview.module.css";

export interface CardPreviewProps {
  /** The card to preview; `null` hides the panel. */
  id: string | null;
  /** Extra note under the effect text (e.g. live crystals / CP). */
  note?: string;
  /** Which side of the anchor to sit on. `auto` flips away from that edge. */
  place?: "right" | "left" | "auto";
  className?: string;
}

/**
 * A floating card detail panel. The caller anchors it (`className` moves it);
 * it is `pointer-events: none`, sized like the hand's preview (260px), and
 * clipped by nothing -- the prompt window keeps `overflow: visible` so the
 * panel can sit outside the card row without being cut.
 */
export function CardPreview({ id, note, place = "auto", className }: CardPreviewProps) {
  if (!id) return null;
  const c = D.card(id);
  return (
    <div
      className={cx(s.preview, place === "auto" && s.auto, place === "left" && s.left, className)}
      role="img"
      aria-label={cardTitle(id)}
    >
      <div className={s.art} style={{ borderColor: cardColor(id) }}>
        <img src={cardArt(id)} alt="" draggable={false} />
      </div>
      <div className={s.title}>{cardTitle(id)}</div>
      {!!c?.tags.length && <div className={s.tags}>{c.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
      <div className={s.text}><SkillBody text={cardText(id)} /></div>
      {!!note && <div className={s.note}>{note}</div>}
    </div>
  );
}