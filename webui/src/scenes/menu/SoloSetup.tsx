// SoloSetupView: you + 2..9 bots named from SoloMatch.BotNames, a character
// per seat (or 随机), score rules, and each bot's mentality (标准 / 混沌). A
// solo match left running (or saved before a refresh) can be continued.

import { useMemo, useState } from "react";
import { navigate } from "../../app/router";
import { cx } from "../../core/cx";
import { D, rules } from "../../core/data";
import { getProfile } from "../../core/store";
import type { BotMentality, CharacterData, ScoreWeights } from "../../core/types";
import { SoloSession, botSoloCapMs, discardSolo, resumeSolo, setBotSoloCapMs, startSolo } from "../../game/session";
import { Btn } from "../../ui/Button";
import { Avatar, CharCard, inTab, tabLabels } from "../../ui/Character";
import { Chips } from "../../ui/Chips";
import { Icon } from "../../ui/Icon";
import { openModal } from "../../ui/Modal";
import { DEFAULT_WEIGHTS, showScoreWeights } from "../../ui/ScoreWeights";
import s from "./SoloSetup.module.css";
import { t as tr } from "../../i18n/t";

const BOTS_KEY = "bm.soloBots";
const MENT_KEY = "bm.soloMentality";
/** Your last explicit character pick (absent = start on 随机). */
const CHAR_KEY = "bm.soloChar";

function botNames(player: string, n: number): string[] {
  const out: string[] = [];
  for (let i = 0; i < n; i++) out.push(rules.bot_name(JSON.stringify([player, ...out])));
  return out;
}

const MENTALITIES = ["standard", "chaos", "advanced"] as const;
const label = (m: BotMentality) =>
  m === "chaos" ? tr("solo.mentalityChaos") : m === "advanced" ? tr("solo.mentalityAdvanced") : tr("solo.mentalityStandard");
const other = (m: BotMentality): BotMentality => {
  const i = MENTALITIES.indexOf(m as (typeof MENTALITIES)[number]);
  return MENTALITIES[(i + 1) % MENTALITIES.length];
};

function SoloSetup({ close }: { close: () => void }) {
  const player = getProfile().playerName;
  const [bots, setBots] = useState(() => Math.min(9, Math.max(2, Number(localStorage.getItem(BOTS_KEY) ?? 3) || 3)));
  const [weights, setWeights] = useState<ScoreWeights>({ ...DEFAULT_WEIGHTS });
  /** The "all bots" default; a per-name `overrides` entry beats it. */
  const [mentality, setMentality] = useState<BotMentality>(() => {
    const raw = localStorage.getItem(MENT_KEY);
    return raw === "chaos" || raw === "advanced" ? raw : "standard";
  });
  /** 进阶 per-decision search cap, seconds (`docs/BOT.md` §3.6 user ruling). */
  const [capSec, setCapSec] = useState(() => Math.round(botSoloCapMs() / 1000));
  const [overrides, setOverrides] = useState<Record<string, BotMentality>>({});
  /**
   * One character per seat, seat 0 first: a name, or `""` for 随机. Bots start
   * on 随机; your own seat remembers the last pick (`CHAR_KEY`), falling back
   * to 随机 when there is none.
   */
  const [chars, setChars] = useState<string[]>(() => {
    const saved = localStorage.getItem(CHAR_KEY);
    const you = saved && D.character(saved) ? saved : "";
    return [you, ...Array(bots).fill("")];
  });
  const names = useMemo(() => botNames(player, bots), [player, bots]);
  /** Seat labels for the picker title / taken stamps: "you", then the bots. */
  const seats = useMemo(() => [tr("common.you"), ...names], [names]);
  const set = (n: number) => {
    const k = Math.min(9, Math.max(2, n));
    setBots(k);
    setChars((cs) => {
      const out = cs.slice(0, k + 1);
      while (out.length < k + 1) out.push("");
      return out;
    });
  };
  const mentalFor = (n: string) => overrides[n] ?? mentality;
  /** One choice for every bot (the default), clearing per-bot overrides. */
  const setAll = (m: BotMentality) => {
    setMentality(m);
    setOverrides({});
    localStorage.setItem(MENT_KEY, m);
  };

  /** Characters named by other seats -- the picker disables those. */
  const takenFor = (seat: number) => {
    const taken = new Map<string, string>();
    chars.forEach((c, i) => {
      if (i !== seat && c) taken.set(c, seats[i] ?? "");
    });
    return taken;
  };

  const pick = (seat: number, c: CharacterData | null) => {
    const name = c?.name ?? "";
    setChars((cs) => cs.map((x, i) => (i === seat ? name : x)));
    if (seat === 0) {
      // Only an explicit pick is remembered; 随机 is the no-history default.
      if (name) localStorage.setItem(CHAR_KEY, name);
      else localStorage.removeItem(CHAR_KEY);
    }
  };

  const openPicker = (seat: number) => {
    const who = seats[seat] ?? "";
    openModal(
      tr("solo.charPick", { who }),
      (cl) => (
        <CharPicker
          current={chars[seat] ?? ""}
          taken={takenFor(seat)}
          onPick={(c) => pick(seat, c)}
          close={cl}
        />
      ),
      { size: "xl", key: "solo-char" }
    );
  };

  const start = () => {
    localStorage.setItem(BOTS_KEY, String(bots));
    close();
    startSolo(
      player,
      chars[0] ?? "",
      names.map((name, i) => ({ name, mentality: mentalFor(name), character: chars[i + 1] ?? "" })),
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
        {MENTALITIES.map((m) => (
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
      {mentality === "advanced" || Object.values(overrides).includes("advanced") ? (
        <div className={s.mentalityRow}>
          <span className={s.mentalityLabel}>{tr("solo.botCap")}</span>
          <input
            type="range"
            min={1}
            max={8}
            step={1}
            value={capSec}
            onChange={(e) => {
              const sec = Math.min(8, Math.max(1, Number(e.target.value) || 3));
              setCapSec(sec);
              setBotSoloCapMs(sec * 1000);
            }}
            title={tr("solo.botCapHint")}
          />
          <span className={s.mentalityLabel}>{tr("solo.botCapSeconds", { n: capSec })}</span>
        </div>
      ) : null}
      <div className={s.rows}>
        <SeatRow
          name={player}
          seatLabel={tr("common.you")}
          character={chars[0] ?? ""}
          you
          onPick={() => openPicker(0)}
        />
        {names.map((n, i) => (
          <SeatRow
            key={n}
            name={n}
            seatLabel={n}
            character={chars[i + 1] ?? ""}
            mentality={mentalFor(n)}
            onMentality={() => setOverrides((o) => ({ ...o, [n]: other(mentalFor(n)) }))}
            onPick={() => openPicker(i + 1)}
            onRemove={i === bots - 1 && bots > 2 ? () => set(bots - 1) : undefined}
          />
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

/** One seat: the character button (avatar or 随机), the name, and its tags. */
function SeatRow({
  name,
  seatLabel,
  character,
  you,
  mentality,
  onMentality,
  onPick,
  onRemove,
}: {
  name: string;
  seatLabel: string;
  character: string;
  you?: boolean;
  mentality?: BotMentality;
  onMentality?: () => void;
  onPick: () => void;
  onRemove?: () => void;
}) {
  const c = D.character(character);
  return (
    <div className={cx(s.row, you && s.you)}>
      <button
        type="button"
        className={s.charBtn}
        title={tr("solo.charPick", { who: seatLabel })}
        onClick={onPick}
      >
        {c ? <Avatar c={c} size={44} /> : <span className={s.randomAv}><Icon name="cyclone" /></span>}
      </button>
      <div className={s.nameCol}>
        <b>{name}</b>
        <small className={cx(s.charName, !c && s.charRandom)}>{c ? c.display : tr("solo.charRandom")}</small>
      </div>
      {you ? (
        <span className={`${s.tag} ${s.youTag}`}>{tr("common.you")}</span>
      ) : (
        <button
          type="button"
          className={`${s.tag} ${s.mentalityTag} ${mentality === "chaos" ? s.mentalityTagChaos : ""} ${mentality === "advanced" ? s.mentalityTagAdvanced : ""}`}
          title={tr("solo.mentalityHint")}
          onClick={onMentality}
        >
          {label(mentality ?? "standard")}
        </button>
      )}
      {onRemove
        ? <button type="button" className={s.x} onClick={onRemove} title={tr("common.remove")}><Icon name="close" /></button>
        : <span className={s.xSpace} />}
    </div>
  );
}

/** Band chips + a 随机 tile + the character grid, one per free character. */
function CharPicker({
  current,
  taken,
  onPick,
  close,
}: {
  current: string;
  taken: Map<string, string>;
  onPick: (c: CharacterData | null) => void;
  close: () => void;
}) {
  const [tab, setTab] = useState<string>(tr("common.all"));
  return (
    <div className={s.picker}>
      <Chips items={tabLabels()} on={tab as (ReturnType<typeof tabLabels>)[number]} onPick={setTab} />
      <div className={s.pickerGrid}>
        <button
          type="button"
          className={cx(s.randomCard, !current && s.randomOn)}
          onClick={() => { close(); onPick(null); }}
        >
          <span className={s.randomFace}><Icon name="cyclone" size={44} /></span>
          <div className={s.randomName}>{tr("solo.charRandom")}</div>
        </button>
        {D.characters.filter((c) => inTab(c, tab)).map((c) => {
          const who = taken.get(c.name);
          return (
            <CharCard
              key={c.name}
              c={c}
              dim={!!who}
              chosen={c.name === current}
              stamp={who}
              stampKind="gray"
              check={c.name === current}
              onClick={() => {
                if (who) return;
                close();
                onPick(c);
              }}
            />
          );
        })}
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