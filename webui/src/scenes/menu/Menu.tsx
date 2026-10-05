// Main menu (MainMenuController): home character with a dialogue box on the
// left, mode tiles on the right.

import { useMemo, useState } from "react";
import { navigate } from "../../app/router";
import { sceneImg } from "../../core/assets";
import { playVoice } from "../../core/audio";
import { cx } from "../../core/cx";
import { D } from "../../core/data";
import { useProfile } from "../../core/hooks";
import { hasNew, settings } from "../../core/store";
import { SoloSession } from "../../game/session";
import { Icon } from "../../ui/Icon";
import { Live2DStand } from "../../ui/Live2DStand";
import { showRules, TopBar } from "../../ui/TopBar";
import s from "./Menu.module.css";
import { showFire, showHistory, showSettings } from "./MenuPopups";
import { showSoloSetup } from "./SoloSetup";
import { t as tr } from "../../i18n/t";

function randomLine(cnId: string) {
  const lines = D.voiceLines.find((v) => v.id === cnId)?.lines.filter((l) => l.text && l.voice) ?? [];
  return lines.length ? lines[Math.floor(Math.random() * lines.length)] : undefined;
}

export function Menu() {
  const p = useProfile()!;
  const c = D.character(p.homeCharacter) ?? D.characters[0];
  const first = useMemo(() => randomLine(c.cnId), [c.cnId]);
  const [line, setLine] = useState(first?.text.replace(/\n/g, "") ?? tr("menu.talk"));
  const [bounce, setBounce] = useState(0);
  const badge = (on: boolean) => on && <img className={s.badge} src={sceneImg("badge_new")} alt="NEW" />;

  const talk = () => {
    const l = randomLine(c.cnId);
    if (!l) return;
    setLine(l.text.replace(/\n/g, ""));
    setBounce((b) => b + 1);
    if (settings().voice > 0) playVoice(c.cnId, l.voice);
  };

  return (
    <>
      <TopBar section={tr("menu.section")} title={tr("menu.selectMode")} />
      <Live2DStand key={D.artId(c)} id={D.artId(c)} className={s.stand} pulse={bounce} alt={c.display} onClick={talk} zoom={1.36} focusTop={0.14} headroom={0.025} />
      <div className={s.dialog}>
        <div className={s.tag}>
          <span>{D.homeTag(c)}</span>
          <img src={sceneImg("deco_star_stripes")} alt="" />
        </div>
        <div className={s.box}>{line}</div>
      </div>

      <button type="button" className={cx(s.small, s.history)} onClick={showHistory}><Icon name="history" />{tr("menu.history")}{badge(hasNew("history"))}</button>
      <button type="button" className={cx(s.small, s.settings)} onClick={showSettings}><Icon name="chevrons" />{tr("menu.settings")}</button>
      <button type="button" className={cx(s.small, s.fire)} onClick={showFire}>
        <div className={s.ribbon}>{tr("menu.fireRibbon", { n: p.firePerGame })}</div>
        <img src={sceneImg("icon_fire")} alt="" />{tr("menu.settings")}
      </button>
      <button type="button" className={cx(s.big, s.story)} disabled title={tr("menu.comingSoon")}>
        <img className={s.pic} src={sceneImg("pic_story")} alt="" />
        <div className={s.soon}>{tr("menu.comingSoon")}</div>
        <div className={s.label}>{tr("menu.story")}</div>
      </button>
      <button type="button" className={cx(s.big, s.solo)} onClick={showSoloSetup}>
        <img className={s.pic} src={sceneImg("pic_single")} alt="" />
        {SoloSession.hasSave() && <div className={s.resume}>{tr("menu.resume")}</div>}
        <div className={s.label}>{tr("menu.solo")}</div>
      </button>
      <button type="button" className={cx(s.big, s.online)} onClick={() => navigate({ name: "lobby" })}>
        <div className={s.wins}><img src={sceneImg("icon_medal")} alt="" /><span>{tr("menu.rankFirst")}</span><b>{p.rankedWins}</b></div>
        <img className={s.pic} src={sceneImg("pic_group")} alt="" />
        <div className={s.modes}>{tr("menu.onlineModes")}</div>
        <div className={s.label}>{tr("menu.online")}</div>
      </button>
      <button type="button" className={cx(s.foot, s.rules)} onClick={showRules}><img className={s.logo} src={sceneImg("logo_game")} alt="" />{tr("menu.tileRules")}{badge(hasNew("rules"))}</button>
      <button type="button" className={cx(s.foot, s.deck)} onClick={() => navigate({ name: "deck" })}><Icon name="playing_cards" />{tr("menu.tileDeck")}{badge(hasNew("deck"))}</button>
      <button type="button" className={cx(s.foot, s.gallery)} onClick={() => navigate({ name: "gallery" })}><Icon name="kid_star" />{tr("menu.tileGallery")}{badge(hasNew("gallery"))}</button>
    </>
  );
}
