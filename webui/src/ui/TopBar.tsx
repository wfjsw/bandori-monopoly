// Screen chrome: square back button, breadcrumb pill (pink section label over
// the white title, "?" help), the status pill and the ☰ menu (MenuPopupView).

import type { ReactNode } from "react";
import { navigate } from "../app/router";
import { sceneImg } from "../core/assets";
import { cx } from "../core/cx";
import { D, rules } from "../core/data";
import { n0, richText } from "../core/format";
import { useProfile, useTick } from "../core/hooks";
import { hasProfile, markSeen } from "../core/store";
import { Btn } from "./Button";
import { Icon } from "./Icon";
import { openModal } from "./Modal";
import s from "./TopBar.module.css";
import { t as tr } from "../i18n/t";

export interface TopBarProps {
  section: string;
  title: string;
  onBack?: () => void;
  /** "?" on the breadcrumb; defaults to the game rules, `false` hides it. */
  help?: (() => void) | false;
  /** Replaces the status pill and ☰ (e.g. the step indicator). */
  right?: ReactNode;
  /** Narrow variant used on the board. */
  compact?: boolean;
}

export function TopBar({ section, title, onBack, help, right, compact }: TopBarProps) {
  const onHelp = help === undefined ? showRules : help;
  return (
    <header className={cx(s.bar, compact && s.compact)}>
      {onBack && (
        <button type="button" className={s.back} onClick={onBack} title={tr("common.back")}>
          <Icon name="back" />
        </button>
      )}
      <div className={s.crumb}>
        <div className={s.section}>{section}</div>
        <div className={s.title}>
          <span>{title}</span>
          {onHelp && <button type="button" className={s.help} onClick={onHelp} title={tr("topbar.help")}>?</button>}
        </div>
      </div>
      {right ?? (
        <>
          <StatusPill />
          <button type="button" className={s.menu} onClick={showMenuPopup} title={tr("topbar.menu")}>
            <Icon name="hamburger" />
          </button>
        </>
      )}
    </header>
  );
}

/** Time until the daily fire refill (local midnight), "HH:MM". */
function untilRefill(): string {
  const now = new Date();
  const next = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  const m = Math.max(0, Math.floor((next.getTime() - now.getTime()) / 60000));
  return `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;
}

function info(title: string, text: string): void {
  openModal(title, <p className={s.info}>{text}</p>, { size: "small" });
}

export function StatusPill() {
  const p = useProfile();
  useTick(30_000);
  if (!p) return null;
  const pct = Math.floor(rules.profile_level_progress(JSON.stringify(p)) * 100);
  return (
    <div className={s.status}>
      <div className={s.level}>
        <div>{tr("common.level")}</div>
        <b>{p.level}</b>
      </div>
      <div className={s.mid}>
        <div className={s.exp}>
          <div className={s.expBar}>
            <div style={{ width: `${pct}%` }} />
          </div>
          <small>EXP</small>
          <b>{pct}</b>
          <small>%</small>
        </div>
        <div className={s.row}>
          <img src={sceneImg("icon_star")} alt="" />
          <div className={s.val}>{n0(p.stars)}</div>
          <button type="button" className={s.plus} onClick={() => info(tr("topbar.stars"), tr("topbar.starsInfo"))}>
            <Icon name="add" />
          </button>
        </div>
      </div>
      <div className={s.right}>
        <div className={s.fire}>
          <img src={sceneImg("icon_fire")} alt="" />
          <b>{p.fire}</b>
          <small>/5</small>
          <span className={s.timer}>
            <small>{tr("topbar.fireRemain")}</small>
            {untilRefill()}
          </span>
          <button
            type="button"
            className={s.plus}
            onClick={() => info(tr("topbar.fire"), tr("topbar.fireInfo", { n: p.firePerGame, mult: p.firePerGame + 1 }))}
          >
            <Icon name="add" />
          </button>
        </div>
        <div className={s.row}>
          <img src={sceneImg("icon_coin")} alt="" />
          <div className={s.val}>{n0(p.coins)}</div>
        </div>
      </div>
    </div>
  );
}

export function showRules(): void {
  if (hasProfile()) markSeen("rules");
  openModal(tr("topbar.rulesTitle"), <div className={s.rules} dangerouslySetInnerHTML={{ __html: richText(D.rulesText) }} />, { size: "wide" });
}

function MenuPopup({ close }: { close: () => void }) {
  const p = useProfile();
  if (!p) {
    return (
      <div className={s.popup}>
        <p>{tr("topbar.noSave")}</p>
        <Btn kind="pink" onClick={() => { close(); navigate({ name: "menu" }); }}>{tr("topbar.createSave")}</Btn>
      </div>
    );
  }
  const need = rules.exp_to_next(p.level);
  const nav = (name: "menu" | "lobby" | "gallery", label: string, icon: string) => (
    <Btn icon={icon} className={s.nav} onClick={() => { close(); navigate({ name }); }}>{label}</Btn>
  );
  return (
    <div className={s.popup}>
      <div className={s.who}>
        <b>{p.playerName}</b>
        <small>ID {p.playerId}</small>
      </div>
      <div className={s.lv}>
        {tr("topbar.levelLine", { n: p.level })}
        <small>{need > 0 ? `EXP ${n0(p.exp)} / ${n0(need)}` : "EXP MAX"}</small>
      </div>
      <div className={s.stats}>{tr("topbar.stats", { games: p.games, solo: p.soloGames, casual: p.casualGames, ranked: p.rankedGames, wins: p.rankedWins })}</div>
      <div className={s.navs}>
        {nav("menu", tr("topbar.home"), "home")}
        {nav("lobby", tr("topbar.lobby"), "groups")}
        {nav("gallery", tr("topbar.gallery"), "kid_star")}
      </div>
    </div>
  );
}

export function showMenuPopup(): void {
  openModal(tr("topbar.menu"), (close) => <MenuPopup close={close} />, { size: "small" });
}
