// Screen chrome: square back button, breadcrumb pill (pink section label over
// the white title, "?" help), the status pill and the ☰ menu (MenuPopupView).
// The title can carry right-hand controls -- the board's 托管 and
// animation-speed toggles dock there (TitleToggles).

import { memo, type ReactNode } from "react";
import { navigate } from "../app/router";
import { sceneImg } from "../core/assets";
import { cx } from "../core/cx";
import { D, rules } from "../core/data";
import { n0, richText } from "../core/format";
import { useProfile, useSessionOther, useTick } from "../core/hooks";
import { hasProfile, markSeen } from "../core/store";
import type { GameSession } from "../game/session";
import { AutoToggle } from "./AutoToggle";
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
  /** Extra control docked at the right of the title pill (e.g. the speed toggle). */
  titleRight?: ReactNode;
  /** Narrow variant used on the board. */
  compact?: boolean;
}

export function TopBar({ section, title, onBack, help, right, titleRight, compact }: TopBarProps) {
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
          {titleRight}
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

/** Cycle of the single speed toggle: 1× → 2× → 4× → 1×. */
const NEXT_SPEED: Record<number, number> = { 1: 2, 2: 4, 4: 1 };

/**
 * The board's animation-speed toggle, docked at the right of the title pill.
 * One button: each click steps the speed and loops back to 1×. Hidden in a
 * read-only session (a replay drives its own speed from the transport bar).
 */
export const AnimationSpeed = memo(function AnimationSpeed({ sess }: { sess: GameSession }) {
  useSessionOther(sess);
  if (sess.readOnly) return null;
  const speed = sess.animSpeed;
  return (
    <button
      type="button"
      className={cx(s.speed, speed > 1 && s.speedOn)}
      title={tr("board.animationSpeedAt", { n: speed })}
      aria-label={tr("board.animationSpeed")}
      onClick={() => sess.setAnimSpeed(NEXT_SPEED[speed] ?? 1)}
    >
      {speed}×
    </button>
  );
});

/**
 * The title's docked toggles: the 托管 mode pill and the animation-speed pill.
 * While both are on the title they read as one segmented control -- two
 * half-pills, flat facing ends over a hairline seam -- which is narrower than
 * two free-standing pills plus their gap and leaves the title more room for
 * the round name. A lone pill keeps its full round shape: `auto` parks the
 * 托管 pill on the hand bar (compact canvas), and a read-only session hides
 * both, so the pair only ever half-rounds when it really is two.
 */
export function TitleToggles({ sess, auto }: { sess: GameSession; auto: boolean }) {
  return (
    <div className={s.duo}>
      {auto && <AutoToggle sess={sess} />}
      <AnimationSpeed sess={sess} />
    </div>
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
          <b>{p.charge}</b>
          <small>/5</small>
          <span className={s.timer}>
            <small>{tr("topbar.fireRemain")}</small>
            {untilRefill()}
          </span>
          <button
            type="button"
            className={s.plus}
            onClick={() => info(tr("topbar.fire"), tr("topbar.fireInfo", { n: p.chargePerGame, mult: p.chargePerGame + 1 }))}
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
