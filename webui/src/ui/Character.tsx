// Character pieces: band filter tabs, stand-art cards, round avatars, name
// plates and band logos.

import type { ReactNode } from "react";
import { bandLogo, charArt } from "../core/assets";
import { cx } from "../core/cx";
import { D } from "../core/data";
import type { CharacterData } from "../core/types";
import s from "./Character.module.css";
import { Icon } from "./Icon";
import { t as tr } from "../i18n/t";

/** Band filter tabs (CharacterGallery / character select / deck editor). */
export const BAND_TABS: { label: string; bands: string[] }[] = [
  { label: "PPP", bands: ["Poppin' Party"] },
  { label: "AG", bands: ["Afterglow"] },
  { label: "PP", bands: ["Pastel✽Palettes"] },
  { label: "Roselia", bands: ["Roselia"] },
  { label: "HHW", bands: ["Hello, Happy World!"] },
  { label: "Morfonica", bands: ["Morfonica"] },
  { label: "RAS", bands: ["RAISE A SUILEN"] },
  { label: "MyGO", bands: ["MyGO!!!!!"] },
  { label: "Mujica", bands: ["Ave Mujica"] },
];
export const tabLabels = () => [tr("common.all"), ...BAND_TABS.map((b) => b.label), tr("common.other")] as const;

export function inTab(c: CharacterData, tab: string): boolean {
  if (tab === tr("common.all")) return true;
  const hit = BAND_TABS.find((b) => b.label === tab);
  return hit ? hit.bands.includes(c.band) : !BAND_TABS.some((b) => b.bands.includes(c.band));
}

export function bandColor(band: string): string {
  return D.band(band)?.color ?? "#ED4E76";
}

export interface CharCardProps {
  c: CharacterData;
  onClick?: () => void;
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
export function CharCard({ c, onClick, dim, chosen, stamp, stampKind = "pink", check, home }: CharCardProps) {
  return (
    <button type="button" className={cx(s.card, dim && s.dim, chosen && s.chosen)} onClick={onClick}>
      <div className={s.art}>
        <img src={charArt(D.artId(c), "thumb")} alt="" loading="lazy" draggable={false} />
      </div>
      <div className={s.line} style={{ background: bandColor(c.band) }} />
      <div className={s.name}>{c.display}</div>
      {stamp && <div className={cx(s.stamp, s[stampKind])}>{stamp}</div>}
      {check && <div className={s.check}><Icon name="check" /></div>}
      {home && <div className={cx(s.check, s.home)}><Icon name="home" /></div>}
    </button>
  );
}

/** Round avatar cropped from the SD art, ringed in the character color. */
export function Avatar({ c, size = 64, className }: { c: CharacterData | undefined; size?: number; className?: string }) {
  return (
    <div className={cx(s.avatar, !c && s.empty, className)} style={{ ["--av-border" as string]: c?.color ?? "var(--line)", width: size, height: size, borderWidth: size < 50 ? 2 : size < 70 ? 3 : 4 }}>
      {c && <img src={charArt(D.artId(c), "sdThumb")} alt="" draggable={false} />}
    </div>
  );
}

/** The character's name plate art, or a CSS plate when the art has none. */
export function NamePlate({ c, width = 355 }: { c: CharacterData; width?: number }) {
  const src = charArt(D.artId(c), "namePlate");
  if (src.includes("/name_")) return <img className={s.plate} style={{ width }} src={src} alt={c.display} />;
  return (
    <div className={s.cssPlate} style={{ width, ["--plate" as string]: bandColor(c.band) }}>
      <b>{c.display}</b>
      <small>{c.band}</small>
    </div>
  );
}

export function BandMark({ band, className }: { band: string; className?: string }) {
  const src = bandLogo(band);
  return src ? <img className={cx(s.logo, className)} src={src} alt={band} /> : <div className={cx(s.logoText, className)} style={{ color: bandColor(band) }}>{band}</div>;
}

/** Grid wrapper so scenes can lay out cards in their own columns. */
export function CharGrid({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cx(s.grid, className)}>{children}</div>;
}
