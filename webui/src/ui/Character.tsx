// Character pieces: band filter tabs, stand-art cards, round avatars, name
// plates and band logos.

import { memo, type ReactNode } from "react";
import { bandLogo, charArt } from "../core/assets";
import { cx } from "../core/cx";
import { D } from "../core/data";
import type { PersonaData } from "../core/types";
import s from "./Character.module.css";
import { Icon } from "./Icon";
import { t as tr } from "../i18n/t";

/** Band filter tabs (CharacterGallery / character select / deck editor). */
export const BAND_TABS: { label: string; groups: string[] }[] = [
  { label: "PPP", groups: ["Poppin' Party"] },
  { label: "AG", groups: ["Afterglow"] },
  { label: "PP", groups: ["Pastel✽Palettes"] },
  { label: "Roselia", groups: ["Roselia"] },
  { label: "HHW", groups: ["Hello, Happy World!"] },
  { label: "Morfonica", groups: ["Morfonica"] },
  { label: "RAS", groups: ["RAISE A SUILEN"] },
  { label: "MyGO", groups: ["MyGO!!!!!"] },
  { label: "Mujica", groups: ["Ave Mujica"] },
];
export const tabLabels = () => [tr("common.all"), ...BAND_TABS.map((b) => b.label), tr("common.other")] as const;

export function inTab(c: PersonaData, tab: string): boolean {
  if (tab === tr("common.all")) return true;
  const hit = BAND_TABS.find((b) => b.label === tab);
  return hit ? hit.groups.includes(c.group) : !BAND_TABS.some((b) => b.groups.includes(c.group));
}

export function bandColor(band: string): string {
  return D.group(band)?.color ?? "#ED4E76";
}

export interface CharCardProps {
  c: PersonaData;
  onClick?: () => void;
  /** Like `onClick` but handed the character: lets a grid pass one stable
   *  callback to every card, so the memoized cards are not re-rendered. */
  onPick?: (c: PersonaData) => void;
  /** Faded: picked by someone else or banned. */
  dim?: boolean;
  /** Pink ring: highlighted / your own. */
  chosen?: boolean;
  /** Text band across the art (tr("select.stepBan"), a player name). */
  stamp?: string;
  stampKind?: "pink" | "gray" | "teal";
  check?: boolean;
  home?: boolean;
}

/** Stand-art card with the band-colored underline and the name. */
export const CharCard = memo(function CharCard({ c, onClick, onPick, dim, chosen, stamp, stampKind = "pink", check, home }: CharCardProps) {
  return (
    <button type="button" className={cx(s.card, dim && s.dim, chosen && s.chosen)} onClick={onPick ? () => onPick(c) : onClick}>
      <div className={s.art}>
        <img src={charArt(D.artId(c), "thumb")} alt="" loading="lazy" draggable={false} />
      </div>
      <div className={s.line} style={{ background: bandColor(c.group) }} />
      <div className={s.name}>{c.display}</div>
      {stamp && <div className={cx(s.stamp, s[stampKind])}>{stamp}</div>}
      {check && <div className={s.check}><Icon name="check" /></div>}
      {home && <div className={cx(s.check, s.home)}><Icon name="home" /></div>}
    </button>
  );
});

/** Round avatar cropped from the SD art, ringed in the character color. */
export const Avatar = memo(function Avatar({ c, size = 64, className }: { c: PersonaData | undefined; size?: number; className?: string }) {
  return (
    <div className={cx(s.avatar, !c && s.empty, className)} style={{ ["--av-border" as string]: c?.color ?? "var(--line)", width: size, height: size, borderWidth: size < 50 ? 2 : size < 70 ? 3 : 4 }}>
      {c && <img src={charArt(D.artId(c), "sdThumb")} alt="" draggable={false} />}
    </div>
  );
});

/** The character's name plate art, or a CSS plate when the art has none. */
export const NamePlate = memo(function NamePlate({ c, width = 355 }: { c: PersonaData; width?: number }) {
  const src = charArt(D.artId(c), "namePlate");
  if (src.includes("/name_")) return <img className={s.plate} style={{ width }} src={src} alt={c.display} />;
  return (
    <div className={s.cssPlate} style={{ width, ["--plate" as string]: bandColor(c.group) }}>
      <b>{c.display}</b>
      <small>{c.group}</small>
    </div>
  );
});

export const BandMark = memo(function BandMark({ band, className }: { band: string; className?: string }) {
  const src = bandLogo(band);
  return src ? <img className={cx(s.logo, className)} src={src} alt={band} /> : <div className={cx(s.logoText, className)} style={{ color: bandColor(band) }}>{band}</div>;
});

/** Grid wrapper so scenes can lay out cards in their own columns. */
export const CharGrid = memo(function CharGrid({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cx(s.grid, className)}>{children}</div>;
});
