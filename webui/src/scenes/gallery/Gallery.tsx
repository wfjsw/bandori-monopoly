// Character gallery (CharacterGalleryController + CharacterDetailView).

import { useState } from "react";
import { navigate } from "../../app/router";
import { charArt } from "../../core/assets";
import { skillText } from "../../core/data";
import { useMountEffect } from "../../hooks/mount";
import { SkillBody } from "../../ui/SkillBody";
import { Live2DStand } from "../../ui/Live2DStand";
import { playVoice } from "../../core/audio";
import { cx } from "../../core/cx";
import { D } from "../../core/data";
import { useProfile } from "../../core/hooks";
import { markSeen, patchProfile } from "../../core/store";
import type { PersonaData } from "../../core/types";
import { Btn } from "../../ui/Button";
import { InspectCard } from "../../ui/Card";
import { BandMark, CharCard, inTab, NamePlate, tabLabels } from "../../ui/Character";
import { Chips } from "../../ui/Chips";
import { Icon } from "../../ui/Icon";
import { openModal } from "../../ui/Modal";
import { toast } from "../../ui/Toast";
import { TopBar } from "../../ui/TopBar";
import s from "./Gallery.module.css";
import { t as tr } from "../../i18n/t";

export function Gallery() {
  useMountEffect(() => markSeen("gallery"));
  const p = useProfile()!;
  const [tab, setTab] = useState<string>(tr("common.all"));
  const list = D.personas.filter((c) => inTab(c, tab));
  return (
    <>
      <TopBar section={tr("topbar.gallery")} title={tr("gallery.title")} onBack={() => navigate({ name: "menu" })} />
      <div className={s.tabs}>
        <Chips items={tabLabels()} on={tab as (ReturnType<typeof tabLabels>)[number]} onPick={setTab} />
        <div className={s.count}>{tab === tr("common.all") ? tr("gallery.countAll", { n: list.length }) : tr("gallery.countTab", { tab, n: list.length })}</div>
      </div>
      <div className={s.panel}>
        <div className={s.grid}>
          {list.map((c) => <CharCard key={c.name} c={c} home={c.name === p.homePersona} onClick={() => showDetail(c, list)} />)}
        </div>
      </div>
    </>
  );
}

function Detail({ list, start }: { list: PersonaData[]; start: number }) {
  const [i, setI] = useState(start);
  const p = useProfile()!;
  const c = list[i];
  const band = D.group(c.group);
  const isHome = p.homePersona === c.name;
  const lines = D.voiceLines.find((v) => v.id === c.cnId)?.lines.filter((l) => l.text && l.voice) ?? [];
  const exclusive = D.cards.filter((x) => x.owner === c.name && !x.derived);
  const step = (d: number) => setI((i + d + list.length) % list.length);
  return (
    <div className={s.detail} style={{ ["--accent" as string]: c.color }}>
      <div className={s.left}>
        <div className={s.glow} />
        <Live2DStand key={D.artId(c)} id={D.artId(c)} className={s.stand} headroom={0.103} />
        <div className={s.group}><BandMark band={c.group} /></div>
        <div className={s.plate}><NamePlate c={c} /></div>
        <button type="button" className={cx(s.nav, s.prev)} onClick={() => step(-1)} title={tr("gallery.prev")}><Icon name="chevron_left" /></button>
        <button type="button" className={cx(s.nav, s.next)} onClick={() => step(1)} title={tr("gallery.next")}><Icon name="chevron_right" /></button>
      </div>
      <div className={s.right}>
        <div className={s.head}><b>{c.display}</b><span className={s.swatch} style={{ background: c.color }} /><span>{tr("gallery.homeColor", { color: c.color })}</span><span className={s.muted}>{c.group}</span></div>
        <div className={s.skill}><span className={s.skillTag}>{tr("select.skillChar")}</span><b>{c.skill || tr("select.skillTbd")}</b><SkillBody className={s.skillBody} text={skillText(c)} /></div>
        <div className={cx(s.skill, s.groupSkill)}><span className={s.skillTag}>{tr("select.skillBand")}</span><b>{band?.skill || tr("select.bandSkillTbd")}</b><SkillBody className={s.skillBody} text={skillText(band)} /></div>
        <div className={s.sub}>{tr("gallery.exclusives")}</div>
        {exclusive.length ? <div className={s.cards}>{exclusive.map((x) => <InspectCard key={x.id} id={x.id} />)}</div> : <p className={s.muted}>{tr("gallery.noExclusives")}</p>}
        {lines.length > 0 && <div className={s.sub}>{tr("gallery.voice")}</div>}
        {lines.length > 0 && (
          <div className={s.lines}>
            {lines.map((l) => <button key={l.voice} type="button" className={s.line} onClick={() => playVoice(c.cnId, l.voice)}><Icon name="volume_up" />{l.text.replace(/\n/g, " ")}</button>)}
          </div>
        )}
        <div className={s.foot}>
          <div className={s.sds}><img src={charArt(D.artId(c), "sdThumb")} alt="" /><img src={charArt(D.artId(c), "sdHappyThumb")} alt="" /></div>
          <Btn kind={isHome ? "white" : "pink"} icon="home" disabled={isHome} onClick={() => { patchProfile({ homePersona: c.name }); toast(tr("gallery.homeToast", { who: c.display })); }}>
            {isHome ? tr("gallery.home") : tr("gallery.setHome")}
          </Btn>
        </div>
      </div>
    </div>
  );
}

function showDetail(c: PersonaData, list: PersonaData[]): void {
  openModal(tr("gallery.detail"), <Detail list={list} start={Math.max(0, list.indexOf(c))} />, { size: "xl", className: s.popup });
}
