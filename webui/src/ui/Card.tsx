// Card faces (art, color strip, title, tags) and the card detail popup
// (CardDetailView).

import type { CSSProperties, ReactNode } from "react";
import { cardArt } from "../core/assets";
import { cx } from "../core/cx";
import { D, cardTitle, GENERAL_BAND } from "../core/data";
import type { CardData } from "../core/types";
import { Btn } from "./Button";
import s from "./Card.module.css";
import { bandColor } from "./Character";
import { openModal } from "./Modal";
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
  size?: "hand" | "strip" | "mini" | "mid" | "pool" | "big";
  on?: boolean;
  onClick?: () => void;
  onMouseEnter?: () => void;
  onMouseLeave?: () => void;
  children?: ReactNode;
  className?: string;
  title?: string;
  style?: CSSProperties;
}

export function CardFace({ id, size = "mini", on, onClick, onMouseEnter, onMouseLeave, children, className, title, style }: CardFaceProps) {
  const c = D.card(id);
  return (
    <div className={cx(s.face, s[size], on && s.on, className)} style={style} onClick={onClick} onMouseEnter={onMouseEnter} onMouseLeave={onMouseLeave} title={title}>
      <div className={s.art}><img src={cardArt(id)} alt="" loading="lazy" draggable={false} /></div>
      <div className={s.line} style={{ background: c ? bandColor(c.band) : "#ED4E76" }} />
      <div className={s.title}>{cardTitle(id)}</div>
      {!!c?.tags.length && <div className={s.tags}>{c.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
      {children}
    </div>
  );
}

/** An empty deck slot (pink with a star). */
export function EmptySlot({ size = "strip", star }: { size?: CardFaceProps["size"]; star: string }) {
  return <div className={cx(s.face, s[size], s.empty)}><img src={star} alt="" /></div>;
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
      <div className={s.bigArt} style={{ borderColor: c ? bandColor(c.band) : "#ED4E76" }}><img src={cardArt(id)} alt="" /></div>
      <div className={s.info}>
        <div className={s.kindRow}>
          {c && <KindChip kind={cardKind(c)} />}
          {c?.band && <span>{c.owner ? `${c.band} · ${c.owner}` : c.band}</span>}
        </div>
        {!!c?.tags.length && <div className={s.tags}>{c.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
        <div className={s.text}><SkillBody text={c?.text ?? ""} /></div>
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
  return openModal(cardTitle(id), (close) => <CardDetail id={id} actions={actions} note={note} close={close} />, { size: "wide", key: "card" });
}
