// SoloSetupView: you + 2..9 bots named from SoloMatch.BotNames, score rules,
// and each bot's mentality (标准 / 混沌). A solo match left running (or saved
// before a refresh) can be continued.

import { useMemo, useState } from "react";
import { navigate } from "../../app/router";
import { rules } from "../../core/data";
import { getProfile } from "../../core/store";
import type { BotMentality, ScoreWeights } from "../../core/types";
import { SoloSession, discardSolo, resumeSolo, startSolo } from "../../game/session";
import { Btn } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { openModal } from "../../ui/Modal";
import { DEFAULT_WEIGHTS, showScoreWeights } from "../../ui/ScoreWeights";
import s from "./SoloSetup.module.css";
import { t as tr } from "../../i18n/t";

const BOTS_KEY = "bm.soloBots";
const MENT_KEY = "bm.soloMentality";

function botNames(player: string, n: number): string[] {
  const out: string[] = [];
  for (let i = 0; i < n; i++) out.push(rules.bot_name(JSON.stringify([player, ...out])));
  return out;
}

const label = (m: BotMentality) => (m === "chaos" ? tr("solo.mentalityChaos") : tr("solo.mentalityStandard"));
const other = (m: BotMentality): BotMentality => (m === "chaos" ? "standard" : "chaos");

function SoloSetup({ close }: { close: () => void }) {
  const player = getProfile().playerName;
  const [bots, setBots] = useState(() => Math.min(9, Math.max(2, Number(localStorage.getItem(BOTS_KEY) ?? 3) || 3)));
  const [weights, setWeights] = useState<ScoreWeights>({ ...DEFAULT_WEIGHTS });
  /** The "all bots" default; a per-name `overrides` entry beats it. */
  const [mentality, setMentality] = useState<BotMentality>(() =>
    localStorage.getItem(MENT_KEY) === "chaos" ? "chaos" : "standard"
  );
  const [overrides, setOverrides] = useState<Record<string, BotMentality>>({});
  const names = useMemo(() => botNames(player, bots), [player, bots]);
  const set = (n: number) => setBots(Math.min(9, Math.max(2, n)));
  const mentalFor = (n: string) => overrides[n] ?? mentality;
  /** One choice for every bot (the default), clearing per-bot overrides. */
  const setAll = (m: BotMentality) => {
    setMentality(m);
    setOverrides({});
    localStorage.setItem(MENT_KEY, m);
  };

  const start = () => {
    localStorage.setItem(BOTS_KEY, String(bots));
    close();
    startSolo(
      player,
      names.map((name) => ({ name, mentality: mentalFor(name) })),
      weights
    );
    navigate({ name: "play", id: "solo" });
  };

  return (
    <div className={s.setup}>
      <div className={s.top}>
        <p className={s.desc}>
          {tr("solo.desc")}
        </p>
        <Btn icon="leaderboard" className={s.score} onClick={() => showScoreWeights(weights, true, setWeights)}>{tr("solo.scoreRules")}</Btn>
      </div>
      <div className={s.mentalityRow}>
        <span className={s.mentalityLabel}>{tr("solo.mentality")}</span>
        {(["standard", "chaos"] as const).map((m) => (
          <button
            key={m}
            type="button"
            className={`${s.mentality} ${m === mentality ? s.mentalityOn : ""}`}
            onClick={() => setAll(m)}
            title={tr("solo.mentalityHint")}
          >
            {label(m)}
          </button>
        ))}
      </div>
      <div className={s.rows}>
        <div className={`${s.row} ${s.you}`}>
          <span className={s.ic}><Icon name="person" /></span>
          <b>{player}</b>
          <span className={`${s.tag} ${s.youTag}`}>{tr("common.you")}</span>
          <span className={s.xSpace} />
        </div>
        {names.map((n, i) => (
          <div key={n} className={s.row}>
            <span className={s.ic}><Icon name="smart_toy" /></span>
            <b>{n}</b>
            <button
              type="button"
              className={`${s.tag} ${s.mentalityTag} ${mentalFor(n) === "chaos" ? s.mentalityTagChaos : ""}`}
              title={tr("solo.mentalityHint")}
              onClick={() => setOverrides((o) => ({ ...o, [n]: other(mentalFor(n)) }))}
            >
              {label(mentalFor(n))}
            </button>
            {i === bots - 1 && bots > 2
              ? <button type="button" className={s.x} onClick={() => set(bots - 1)} title={tr("common.remove")}><Icon name="close" /></button>
              : <span className={s.xSpace} />}
          </div>
        ))}
      </div>
      <div className={s.foot}>
        <Btn icon="add" className={s.add} disabled={bots >= 9} onClick={() => set(bots + 1)}>{tr("solo.addBot")}</Btn>
        <div className={s.count}><b>{tr("common.playersN", { n: bots + 1 })}</b><small>{tr("solo.playersHint")}</small></div>
        <Btn kind="pink" size="big" className={s.go} onClick={start}>{tr("solo.start")}</Btn>
      </div>
    </div>
  );
}

function Resume({ close }: { close: () => void }) {
  return (
    <div className={s.resume}>
      <p>{tr("solo.resumeText")}</p>
      <div className={s.resumeBtns}>
        <Btn onClick={() => { close(); discardSolo(); showSoloSetup(); }}>{tr("solo.resumeDrop")}</Btn>
        <Btn kind="pink" onClick={() => { close(); if (resumeSolo()) navigate({ name: "play", id: "solo" }); }}>{tr("menu.resume")}</Btn>
      </div>
    </div>
  );
}

export function showSoloSetup(): void {
  if (SoloSession.hasSave()) {
    openModal(tr("menu.solo"), (close) => <Resume close={close} />, { size: "mid" });
    return;
  }
  openModal(tr("menu.solo"), (close) => <SoloSetup close={close} />);
}