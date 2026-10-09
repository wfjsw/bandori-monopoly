// Card faces (art, color strip, title, tags) and the card detail popup
// (CardDetailView). Sizing is deterministic: the `width` prop sets `--card-w`,
// which every size class reads as its default, so a caller override always wins
// regardless of stylesheet order. `InspectCard` is the one hover-preview /
// click-to-inspect behaviour every prompt and card collection shares.

import type { CSSProperties, ReactNode } from "react";
import { cardArt } from "../core/assets";
import { cx } from "../core/cx";
import { D, cardTitle, cardText, cardColor, skillCard, GENERAL_BAND } from "../core/data";
import type { CardData } from "../core/types";
import { Btn } from "./Button";
import s from "./Card.module.css";
import { openModal } from "./Modal";
import { previewFrom, previewHide, stickCard } from "./CardPreview";
import { SkillBody } from "./SkillBody";
import { t as tr } from "../i18n/t";

/** Card category, as the deck editor labels it. Stable keys; labels are translated. */
export type CardKind = "exclusive" | "band" | "general" | "derived";
export function cardKind(c: CardData): CardKind {
  if (c.derived) return "derived";
  if (c.owner) return "exclusive";
  return c.band === GENERAL_BAND ? "general" : "band";
}

export const CARD_KIND_LABEL: Record<CardKind, string> = {
  exclusive: "card.kind.exclusive",
  band: "card.kind.band",
  general: "card.kind.general",
  derived: "card.kind.derived",
};

const TAG_SHORT: Record<string, string> = { "手牌": "card.tag.hand", "手": "card.tag.hand", "反击": "card.tag.counter", "持续": "card.tag.cont" };

export function TagChip({ tag }: { tag: string }) {
  const key = TAG_SHORT[tag];
  return <span className={cx(s.tag, (tag === "手" || tag === "反击") && s.red)}>{key ? tr(key) : tag}</span>;
}

export function KindChip({ kind, className }: { kind: CardKind; className?: string }) {
  return <span className={cx(s.kind, s[`k${Object.keys(CARD_KIND_LABEL).indexOf(kind)}`], className)}>{tr(CARD_KIND_LABEL[kind])}</span>;
}

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

export function CardFace({ id, size = "mini", width, on, onClick, onMouseEnter, onMouseLeave, onFocus, onBlur, children, className, title, style }: CardFaceProps) {
  const c = D.card(id);
  const faceStyle = width != null ? { ...style, ["--card-w" as string]: `${width}px` } : style;
  const clickable = !!onClick;
  return (
    <div
      className={cx(s.face, s[size], on && s.on, className)}
      style={faceStyle}
      role={clickable ? "button" : undefined}
      tabIndex={clickable ? 0 : undefined}
      onClick={onClick}
      onKeyDown={clickable ? (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onClick(); } } : undefined}
      onMouseEnter={(e) => onMouseEnter?.(e.currentTarget)}
      onMouseLeave={onMouseLeave}
      onFocus={(e) => onFocus?.(e.currentTarget)}
      onBlur={onBlur}
      title={title}
    >
      <div className={s.art}><img src={cardArt(id)} alt="" loading="lazy" draggable={false} /></div>
      <div className={s.line} style={{ background: cardColor(id) }} />
      <div className={s.title}>{cardTitle(id)}</div>
      {!!c?.tags.length && <div className={s.tags}>{c.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
      {children}
    </div>
  );
}

/** An empty deck slot (pink with a star). */
export function EmptySlot({ size = "strip", star, width }: { size?: CardFaceProps["size"]; star: string; width?: number }) {
  return <div className={cx(s.face, s[size], s.empty)} style={width != null ? { ["--card-w" as string]: `${width}px` } : undefined}><img src={star} alt="" /></div>;
}

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
      <div className={s.bigArt} style={{ borderColor: cardColor(id) }}><img src={cardArt(id)} alt="" /></div>
      <div className={s.info}>
        <div className={s.kindRow}>
          {c && <KindChip kind={cardKind(c)} />}
          {c?.band && <span>{c.owner ? `${c.band} · ${c.owner}` : c.band}</span>}
          {!c && skillCard(id)?.band && <span>{skillCard(id)!.band}</span>}
        </div>
        {!!c?.tags.length && <div className={s.tags}>{c.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
        <div className={s.text}><SkillBody text={cardText(id)} /></div>
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
export function showCard(id: string, actions: CardAction[] = [], note = ""): () => void {
  // The standing card panel (board) follows every card the user opens.
  stickCard(id, note);
  return openModal(cardTitle(id), (close) => <CardDetail id={id} actions={actions} note={note} close={close} />, { size: "wide", key: "card" });
}

export interface InspectCardProps extends CardFaceProps {
  /** Detail-sheet actions beyond a bare inspect (play / discard, pick this). */
  actions?: CardAction[];
  /** Note for the hover preview and the detail sheet (live state, hand notes). */
  note?: string;
  /** Hover also reports the card id (a select grid's own detail panel). */
  onHover?: (id: string | null) => void;
  /** Skip the floating hover preview (the screen shows its own detail panel). */
  preview?: boolean;
}

/**
 * The shared card interaction: hover / focus floats the preview, click opens
 * the detail sheet. Every card shown in a prompt or a card-collection popup
 * goes through this one component -- no per-call-site wiring.
 */
export function InspectCard({ id, actions, note, onHover, preview = true, onClick, onMouseEnter, onMouseLeave, onFocus, onBlur, ...face }: InspectCardProps) {
  const enter = (el: HTMLElement) => {
    if (preview) previewFrom(el, id, note);
    onHover?.(id);
    onMouseEnter?.(el);
  };
  const leave = (after?: () => void) => {
    if (preview) previewHide();
    onHover?.(null);
    after?.();
  };
  return (
    <CardFace
      id={id}
      onClick={onClick ?? (() => void showCard(id, actions, note))}
      onMouseEnter={enter}
      onFocus={enter}
      onMouseLeave={() => leave(onMouseLeave)}
      onBlur={() => leave(onBlur)}
      {...face}
    />
  );
}