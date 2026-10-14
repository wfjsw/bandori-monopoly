// Match clock / timer settings popup (ClockSettingsView): seven ±5s steppers
// (0..=300 s) or read-only. Mirrors `ScoreWeights.tsx` -- same modal, same
// host-only gate. Nested under `RoomInfo.clock` / `CreateRoom.clock`.

import { useState } from "react";
import type { ClockSettings } from "../core/types";
import { Btn, RoundBtn } from "./Button";
import { Icon } from "./Icon";
import { openModal } from "./Modal";
import s from "./ScoreWeights.module.css";
import { t as tr } from "../i18n/t";

export const DEFAULT_CLOCK: ClockSettings = {
  baseTime: 15,
  bonusTime: 0,
  baseRefill: 15,
  banSeconds: 20,
  pickSeconds: 25,
  deckSeconds: 45,
  bankruptAuctionTime: 10,
};

export const CLOCK_MIN = 0;
export const CLOCK_MAX = 300;

const fmt = (v: number) => String(Math.round(v * 10) / 10);

type Key = keyof ClockSettings;

const KEYS = (): [Key, string, string][] => [
  ["baseTime", tr("clock.baseTime"), "hourglass_top"],
  ["bonusTime", tr("clock.bonusTime"), "bolt"],
  ["baseRefill", tr("clock.baseRefill"), "autorenew"],
  ["banSeconds", tr("clock.banSeconds"), "block"],
  ["pickSeconds", tr("clock.pickSeconds"), "how_to_reg"],
  ["deckSeconds", tr("clock.deckSeconds"), "style"],
  ["bankruptAuctionTime", tr("clock.bankruptAuctionTime"), "gavel"],
];

function Clocks({
  initial,
  editable,
  changed,
  hint,
}: {
  initial: ClockSettings;
  editable: boolean;
  changed?: (c: ClockSettings) => void;
  hint: string;
}) {
  const [c, setC] = useState(initial);
  const apply = (next: ClockSettings) => {
    setC(next);
    changed?.(next);
  };
  const step = (k: Key, d: number) => {
    const next = { ...c, [k]: Math.min(CLOCK_MAX, Math.max(CLOCK_MIN, c[k] + d)) };
    apply(next);
  };
  const isDefault = (Object.keys(DEFAULT_CLOCK) as Key[]).every((k) => c[k] === DEFAULT_CLOCK[k]);
  return (
    <div className={s.box}>
      {KEYS().map(([k, label, icon]) => (
        <div key={k} className={s.row}>
          <div className={s.label}>
            <Icon name={icon} />
            {label}
          </div>
          {editable && <RoundBtn icon="remove" disabled={c[k] <= CLOCK_MIN} onClick={() => step(k, -5)} />}
          <div className={s.val}>{fmt(c[k])}{tr("common.unitSec")}</div>
          {editable && <RoundBtn icon="add" disabled={c[k] >= CLOCK_MAX} onClick={() => step(k, 5)} />}
        </div>
      ))}
      <p className={s.formula}>{tr("clock.formula")}</p>
      <p className={s.hint}>
        {editable
          ? tr("clock.hintEdit")
          : (
            <>
              {tr("clock.hintRead")}
              <span className={s.pink}>{hint}</span>
            </>
          )}
      </p>
      {editable && !isDefault && (
        <Btn size="small" onClick={() => apply({ ...DEFAULT_CLOCK })}>{tr("score.reset")}</Btn>
      )}
    </div>
  );
}

export function showClockSettings(
  clock: ClockSettings,
  editable: boolean,
  changed?: (c: ClockSettings) => void,
  readOnlyHint = tr("score.onlyHost"),
): void {
  openModal(
    tr("clock.title"),
    <Clocks initial={clock} editable={editable} changed={changed} hint={readOnlyHint} />,
    { size: "mid" },
  );
}