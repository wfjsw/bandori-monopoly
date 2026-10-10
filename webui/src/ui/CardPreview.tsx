// The enlarged card preview that follows a hover: art, title, tags and effect
// text, pointer-transparent so it never steals the hover from the thing being
// previewed. Shared by the hand, the field cards and the prompts -- one look,
// one behaviour (hover previews, click inspects). The store below is that one
// behaviour: `previewFrom` / `previewHide` from `InspectCard` (or a detail
// block) and `InspectPreviewHost` draws the panel inside the stage.

import { useSyncExternalStore, type CSSProperties, type ReactElement } from "react";
import { useMountEffect } from "../hooks/mount";
import { cardArt } from "../core/assets";
import { cx } from "../core/cx";
import { cardColor, cardText, cardTitle, D } from "../core/data";
import { t as tr } from "../i18n/t";
import { SkillBody } from "./SkillBody";
import s from "./CardPreview.module.css";

const TAG_SHORT: Record<string, string> = { "手牌": "card.tag.hand", "手": "card.tag.hand", "反击": "card.tag.counter", "持续": "card.tag.cont" };

/** The tag chip, restyled locally so this module never imports Card (cycle). */
function PreviewTag({ tag }: { tag: string }) {
  const key = TAG_SHORT[tag];
  return <span className={cx(s.tagChip, (tag === "手" || tag === "反击") && s.tagRed)}>{key ? tr(key) : tag}</span>;
}

export interface CardPreviewProps {
  /** The card to preview; `null` hides the panel. */
  id: string | null;
  /** Extra note under the effect text (e.g. live crystals / CP). */
  note?: string;
  /** Which side of the anchor to sit on. `auto` flips away from that edge. */
  place?: "right" | "left" | "auto";
  className?: string;
  style?: CSSProperties;
  /** A docked, interactive panel (the standing preview): the text takes keyboard
   *  focus so arrows / PageUp / PageDown scroll it. */
  scrollable?: boolean;
}

/**
 * A floating card detail panel. The caller anchors it (`className` / `style`
 * move it); it is `pointer-events: none`, sized like the hand's preview (260px),
 * and clipped by nothing -- the prompt window keeps `overflow: visible` so the
 * panel can sit outside the card row without being cut.
 */
export function CardPreview({ id, note, place = "auto", className, style, scrollable }: CardPreviewProps) {
  if (!id) return null;
  const c = D.card(id);
  return (
    <div
      className={cx(s.preview, place === "auto" && s.auto, place === "left" && s.left, className)}
      style={style}
      role={scrollable ? "region" : "img"}
      aria-label={cardTitle(id)}
    >
      <div className={s.art} style={{ borderColor: cardColor(id) }}>
        <img src={cardArt(id)} alt="" draggable={false} />
      </div>
      <div className={s.title}>{cardTitle(id)}</div>
      {!!c?.tags.length && <div className={s.tags}>{c.tags.map((t) => <PreviewTag key={t} tag={t} />)}</div>}
      <div className={s.text} tabIndex={scrollable ? 0 : undefined}><SkillBody text={cardText(id)} /></div>
      {!!note && <div className={s.note}>{note}</div>}
    </div>
  );
}

// ---- The one shared hover-preview store -------------------------------------

interface Anchor {
  id: string;
  note?: string;
  /** The hovered element; the host maps it into the stage's design pixels. */
  el: HTMLElement;
}

let anchor: Anchor | null = null;
/** The card the standing panel shows: set on hover, click and flash, and it
 *  stays until another card replaces it (hover-out never clears it). */
let stick: { id: string; note?: string } | null = null;
/** The match screen prefers its standing panel to floating popups. */
let standing = false;
const listeners = new Set<() => void>();

function emit(): void {
  listeners.forEach((cb) => cb());
}

/** Float the shared preview beside `el` (hover / keyboard focus). */
export function previewFrom(el: HTMLElement, id: string, note?: string): void {
  anchor = { el, id, note };
  stick = { id, note };
  emit();
}

/** Hide the floating preview (the pointer left the card). The standing panel
 *  keeps showing the last card. */
export function previewHide(): void {
  if (!anchor) return;
  anchor = null;
  emit();
}

/** Pin the standing card panel: a click, a card flash, a log reference. */
export function stickCard(id: string, note = ""): void {
  stick = { id, note };
  emit();
}

/** Turn the floating hover popups off while a standing panel is on screen. */
export function useStandingPreviewOn(): void {
  useMountEffect(() => {
    standing = true;
    emit();
    return () => {
      standing = false;
      emit();
    };
  });
}

/** True while the match screen's standing panel is up (floaters stay off). */
export function useStandingMode(): boolean {
  return useSyncExternalStore(subscribe, () => standing);
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/** The card the standing panel should show (sticky across hover-outs). */
export function useStickyCard(): { id: string; note?: string } | null {
  return useSyncExternalStore(subscribe, () => stick);
}

/** The docked variant of this panel (board `CardStand`): full-height, no
 *  floating chrome. Exported here so it overrides this module's own layout. */
export const cardStandClass: string = s.stand;

/**
 * The preview host. Drawn inside the stage (so it scales with it) as an
 * absolutely positioned panel beside the hovered card -- `position: absolute`
 * on the stage, which is the containing block even under the stage's scale
 * transform, so the panel escapes any modal body's `overflow: auto`. The
 * match screen turns it off: its standing panel (board `CardStand`) is the one
 * card detail there.
 */
export function InspectPreviewHost(): ReactElement | null {
  const cur = useSyncExternalStore(subscribe, () => (standing ? null : anchor));
  if (!cur) return null;
  const host = cur.el.closest("[data-stage]") as HTMLElement | null;
  const place = host ? stagePlace(host, cur.el) : { left: 0, top: 0 };
  return (
    <CardPreview
      id={cur.id}
      note={cur.note}
      className={s.hosted}
      style={{ left: place.left, top: place.top }}
    />
  );
}

/**
 * Where to put the preview beside `el`, in the stage's design pixels, flipping
 * to the card's left when the stage's right edge would cut it off.
 */
function stagePlace(stage: HTMLElement, el: HTMLElement): { left: number; top: number } {
  const sr = stage.getBoundingClientRect();
  const er = el.getBoundingClientRect();
  const scale = sr.width / (stage.offsetWidth || 1);
  const x = (er.left - sr.left) / scale;
  const y = (er.top - sr.top) / scale;
  const w = (er.width / scale) || 96;
  const gap = 14;
  const previewW = 260;
  const stageW = stage.offsetWidth || sr.width / scale;
  const flip = x + w + gap + previewW > stageW;
  return { left: flip ? x - gap - previewW : x + w + gap, top: Math.max(0, y) };
}