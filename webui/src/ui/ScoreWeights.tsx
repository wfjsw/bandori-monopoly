// Score rules popup (ScoreWeightsView): three ±0.5 steppers (0..5) or read-only.

import { useState } from "react";
import type { ScoreWeights } from "../core/types";
import { Btn, RoundBtn } from "./Button";
import { Icon } from "./Icon";
import { openModal } from "./Modal";
import s from "./ScoreWeights.module.css";
import { toast } from "./Toast";
import { t as tr } from "../i18n/t";

export const DEFAULT_WEIGHTS: ScoreWeights = { money: 1, property: 1, houses: 1 };

const fmt = (v: number) => String(Math.round(v * 10) / 10);

export function formula(w: ScoreWeights): string {
  return tr("score.formula", { money: fmt(w.money), property: fmt(w.property), houses: fmt(w.houses) });
}

const KEYS = (): [keyof ScoreWeights, string, string][] => [["money", tr("score.money"), "account_balance"], ["property", tr("score.property"), "location_on"], ["houses", tr("deed.house"), "construction"]];

function Weights({ initial, editable, changed, hint }: { initial: ScoreWeights; editable: boolean; changed?: (w: ScoreWeights) => void; hint: string }) {
  const [w, setW] = useState(initial);
  const apply = (next: ScoreWeights) => {
    setW(next);
    changed?.(next);
  };
  const step = (k: keyof ScoreWeights, d: number) => {
    const next = { ...w, [k]: Math.min(5, Math.max(0, w[k] + d)) };
    if (next.money + next.property + next.houses <= 0) return toast(tr("score.zero"));
    apply(next);
  };
  const isDefault = w.money === 1 && w.property === 1 && w.houses === 1;
  return (
    <div className={s.box}>
      {KEYS().map(([k, label, icon]) => (
        <div key={k} className={s.row}>
          <div className={s.label}><Icon name={icon} />{label}</div>
          {editable && <RoundBtn icon="remove" disabled={w[k] <= 0} onClick={() => step(k, -0.5)} />}
          <div className={s.val}>×{fmt(w[k])}</div>
          {editable && <RoundBtn icon="add" disabled={w[k] >= 5} onClick={() => step(k, 0.5)} />}
        </div>
      ))}
      <p className={s.formula}>{formula(w)}</p>
      <p className={s.hint}>
        {editable
          ? tr("score.hintEdit")
          : <>{tr("score.hintRead")}<span className={s.pink}>{hint}</span></>}
      </p>
      {editable && !isDefault && <Btn size="small" onClick={() => apply({ ...DEFAULT_WEIGHTS })}>{tr("score.reset")}</Btn>}
    </div>
  );
}

export function showScoreWeights(weights: ScoreWeights, editable: boolean, changed?: (w: ScoreWeights) => void, readOnlyHint = tr("score.onlyHost")): void {
  openModal(tr("solo.scoreRules"), <Weights initial={weights} editable={editable} changed={changed} hint={readOnlyHint} />, { size: "mid" });
}
