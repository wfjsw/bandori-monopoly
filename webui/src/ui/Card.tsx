// Card faces (art, color strip, title, tags) and the card detail popup
// (CardDetailView). Sizing is deterministic: the `width` prop sets `--card-w`,
// which every size class reads as its default, so a caller override always wins
// regardless of stylesheet order. `InspectCard` is the one hover-preview /
// click-to-inspect behaviour every prompt and card collection shares.

import { memo, useCallback, type CSSProperties, type ReactNode } from "react";
import { cx } from "../core/cx";
import { D, cardTitle, cardText, cardBody, cardColor, cardArtUrl, skillCard, GENERAL_GROUP } from "../core/data";
import type { CardData } from "../core/types";
import { useLatestRef } from "../hooks/latest";
import { Btn } from "./Button";
import s from "./Card.module.css";
import { openModal } from "./Modal";
import { previewFrom, previewHide, stickCard, type CardPill } from "./CardPreview";
import { SkillBody } from "./SkillBody";
import { t as tr } from "../i18n/t";

/** Card category, as the deck editor labels it. Stable keys; labels are translated. */
export type CardKind = "exclusive" | "band" | "general" | "derived";
export function cardKind(c: CardData): CardKind {
  if (c.derived) return "derived";
  if (c.owner) return "exclusive";
  return c.group === GENERAL_GROUP ? "general" : "band";
}

export const CARD_KIND_LABEL: Record<CardKind, string> = {
  exclusive: "card.kind.exclusive",
  band: "card.kind.group",
  general: "card.kind.general",
  derived: "card.kind.derived",
};

const TAG_SHORT: Record<string, string> = { "手牌": "card.tag.hand", "手": "card.tag.hand", "反击": "card.tag.counter", "持续": "card.tag.cont" };

export const TagChip = memo(function TagChip({ tag }: { tag: string }) {
  const key = TAG_SHORT[tag];
  return <span className={cx(s.tag, (tag === "手" || tag === "反击") && s.red)}>{key ? tr(key) : tag}</span>;
});

export const KindChip = memo(function KindChip({ kind, className }: { kind: CardKind; className?: string }) {
  return <span className={cx(s.kind, s[`k${Object.keys(CARD_KIND_LABEL).indexOf(kind)}`], className)}>{tr(CARD_KIND_LABEL[kind])}</span>;
});

export interface CardFaceProps {
  id: string;
  size?: "hand" | "strip" | "mini" | "mid" | "pool" | "big" | "tile";
  /**
   * Force the face's width in px. Sets `--card-w`, which the size classes read
   * as their default -- so the override always wins, whatever the stylesheet
   * order. Use this instead of a className that sets `width`.
   */
  width?: number;
  on?: boolean;
  /** Blue glowing border: the card can be played right now. Unplayable faces
   *  stay ordinary -- nothing dims. */
  playable?: boolean;
  /**
   * Field-row face: the tags tuck above the title and the title + `children`
   * (the caller's marker badges) form a fixed-height bottom zone -- exactly
   * the part that peeks while the field row is retracted.
   */
  field?: boolean;
  onClick?: () => void;
  onMouseEnter?: (el: HTMLElement) => void;
  onMouseLeave?: () => void;
  onFocus?: (el: HTMLElement) => void;
  onBlur?: () => void;
  children?: ReactNode;
  className?: string;
  title?: string;
  style?: CSSProperties;
}

export const CardFace = memo(function CardFace({ id, size = "mini", width, on, playable, field, onClick, onMouseEnter, onMouseLeave, onFocus, onBlur, children, className, title, style }: CardFaceProps) {
  const c = D.card(id);
  const faceStyle = width != null ? { ...style, ["--card-w" as string]: `${width}px` } : style;
  const clickable = !!onClick;
  // Stable with `onClick`, so a re-render that keeps the same click handler
  // does not hand a fresh `onKeyDown` down the face.
  const onKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onClick?.();
    }
  }, [onClick]);
  return (
    <div
      className={cx(s.face, s[size], on && s.on, playable && s.playable, field && s.field, className)}
      style={faceStyle}
      role={clickable ? "button" : undefined}
      tabIndex={clickable ? 0 : undefined}
      onClick={onClick}
      onKeyDown={clickable ? onKeyDown : undefined}
      onMouseEnter={(e) => onMouseEnter?.(e.currentTarget)}
      onMouseLeave={onMouseLeave}
      onFocus={(e) => onFocus?.(e.currentTarget)}
      onBlur={onBlur}
      title={title}
    >
      <div className={s.art}><img src={cardArtUrl(id)} alt="" decoding="async" draggable={false} /></div>
      <div className={s.line} style={{ background: cardColor(id) }} />
      <div className={s.title}>{cardTitle(id)}</div>
      {!!c?.tags.length && <div className={s.tags}>{c.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
      {field ? <div className={s.marks}>{children}</div> : children}
    </div>
  );
});

/** An empty deck slot (pink with a star). */
export const EmptySlot = memo(function EmptySlot({ size = "strip", star, width, className }: { size?: CardFaceProps["size"]; star: string; width?: number; className?: string }) {
  return <div className={cx(s.face, s[size], s.empty, className)} style={width != null ? { ["--card-w" as string]: `${width}px` } : undefined}><img src={star} alt="" /></div>;
});

export interface CardAction {
  label: string;
  enabled: boolean;
  run?: () => unknown;
  kind?: "pink" | "white";
}

function CardDetail({ id, actions, note, close }: { id: string; actions: CardAction[]; note: string; close: () => void }) {
  const c = D.card(id);
  return (
    <div className={s.detail}>
      <div className={s.bigArt} style={{ borderColor: cardColor(id) }}><img src={cardArtUrl(id)} alt="" /></div>
      <div className={s.info}>
        <div className={s.kindRow}>
          {c && <KindChip kind={cardKind(c)} />}
          {c?.group && <span>{c.owner ? `${c.group} · ${c.owner}` : c.group}</span>}
          {!c && skillCard(id)?.group && <span>{skillCard(id)!.group}</span>}
        </div>
        {!!c?.tags.length && <div className={s.tags}>{c.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
        <div className={s.text}><SkillBody blocks={cardBody(id)} text={cardText(id)} /></div>
        {note && <div className={s.note}>{note}</div>}
        {actions.length > 0 && (
          <div className={s.actions}>
            {actions.map((a) => (
              <Btn key={a.label} kind={a.kind ?? "pink"} disabled={!a.enabled} onClick={async () => { await a.run?.(); close(); }}>{a.label}</Btn>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

/** CardDetailView: big art, kind + tags, effect text, optional action buttons. */
export function showCard(id: string, actions: CardAction[] = [], note = "", pills?: CardPill[]): () => void {
  // The standing card panel (board) follows every card the user opens.
  stickCard(id, note, pills);
  return openModal(cardTitle(id), (close) => <CardDetail id={id} actions={actions} note={note} close={close} />, { size: "wide", key: "card" });
}

export interface InspectCardProps extends CardFaceProps {
  /** Detail-sheet actions beyond a bare inspect (play / discard, pick this). */
  actions?: CardAction[];
  /** Note for the hover preview and the detail sheet (live state, hand notes). */
  note?: string;
  /** Owner / designee pills for the hover preview and standing panel. */
  pills?: CardPill[];
  /** Hover also reports the card id (a select grid's own detail panel). */
  onHover?: (id: string | null) => void;
  /** Skip the floating hover preview (the screen shows its own detail panel). */
  preview?: boolean;
}

/**
 * The shared card interaction: hover / focus floats the preview, click opens
 * the detail sheet. Every card shown in a prompt or a card-collection popup
 * goes through this one component -- no per-call-site wiring.
 *
 * The four face handlers are ref-indirected so their identity is stable: a
 * note / pills / actions update then re-renders `InspectCard` without dragging
 * `memo(CardFace)` (and its `<img>`) along with it.
 */
export const InspectCard = memo(function InspectCard({ id, actions, note, pills, onHover, preview = true, onClick, onMouseEnter, onMouseLeave, onFocus, onBlur, ...face }: InspectCardProps) {
  const latest = useLatestRef({ id, actions, note, pills, onHover, preview, onClick, onMouseEnter, onMouseLeave, onFocus, onBlur });
  const enter = useCallback((el: HTMLElement) => {
    const v = latest.current;
    if (v.preview) previewFrom(el, v.id, v.note, v.pills);
    v.onHover?.(v.id);
    v.onMouseEnter?.(el);
  }, [latest]);
  const leave = useCallback(() => {
    const v = latest.current;
    if (v.preview) previewHide();
    v.onHover?.(null);
    v.onMouseLeave?.();
  }, [latest]);
  const blur = useCallback(() => {
    const v = latest.current;
    if (v.preview) previewHide();
    v.onHover?.(null);
    v.onBlur?.();
  }, [latest]);
  const click = useCallback(() => {
    const v = latest.current;
    if (v.onClick) v.onClick();
    else void showCard(v.id, v.actions, v.note, v.pills);
  }, [latest]);
  return (
    <CardFace
      id={id}
      onClick={click}
      onMouseEnter={enter}
      onFocus={enter}
      onMouseLeave={leave}
      onBlur={blur}
      {...face}
    />
  );
});