// Menu popups: match history, settings, fire per game.

import { useState } from "react";
import { navigate } from "../../app/router";
import { useMountEffect } from "../../hooks/mount";
import { applyVolumes, sfx } from "../../core/audio";
import { cx } from "../../core/cx";
import { D, rules } from "../../core/data";
import { n0 } from "../../core/format";
import { useProfile, useSettings } from "../../core/hooks";
import { deleteProfile, getProfile, markSeen, profileJson, saveSettings, updateProfile } from "../../core/store";
import type { SoundSettings } from "../../core/types";
import { resolveHistoryReplay, type HistoryReplayLink } from "../../game/historyReplay";
import { getReplay, listReplays, type ReplayEntry } from "../../game/record";
import { queueReplayBytes } from "../../game/replay";
import { Btn } from "../../ui/Button";
import { TextInput } from "../../ui/Form";
import { Icon } from "../../ui/Icon";
import { ask, closeAllModals, openModal } from "../../ui/Modal";
import { toast } from "../../ui/Toast";
import s from "./MenuPopups.module.css";
import { t as tr } from "../../i18n/t";
import { LANGS, LANG_LABEL, setLang, savedLang, type Lang } from "../../i18n";

const modeLabel = (mode: number) => ([tr("mode.shortSolo"), tr("common.casualShort"), tr("common.rankShort")][mode] ?? "");

/** Watch a history row's match: the same path the replay list opens a record. */
async function openHistoryReplay(link: HistoryReplayLink): Promise<void> {
  if (link.kind === "none") return;
  const bytes = await getReplay(link.id).catch(() => null);
  if (!bytes) {
    // Evicted from the local store (it keeps 10), or never written.
    toast(tr("history.replayGone"), "error");
    return;
  }
  try {
    queueReplayBytes(bytes, link.id);
    closeAllModals();
    navigate({ name: "replayView" });
  } catch (e) {
    console.warn("could not open the replay:", e);
    toast(tr("replay.badFile"), "error");
  }
}

function History() {
  const p = useProfile()!;
  const rows = [...p.history].reverse();
  // Headers only, to decide per row whether a replay exists (and which one).
  const [replays, setReplays] = useState<ReplayEntry[] | null>(null);
  useMountEffect(() => {
    void listReplays()
      .then(setReplays)
      .catch((e) => {
        console.warn("replay list:", e);
        setReplays([]);
      });
  });
  return (
    <div className={s.history}>
      <div className={s.sum}>{tr("topbar.stats", { games: p.games, solo: p.soloGames, casual: p.casualGames, ranked: p.rankedGames, wins: p.rankedWins })}</div>
      {rows.length === 0 ? <div className={s.empty}>{tr("history.empty")}</div> : rows.map((r, i) => {
        const link = replays ? resolveHistoryReplay(r, replays) : null;
        const tip = link?.kind === "gone" ? tr("history.replayGone") : link?.kind === "none" ? tr("history.replayNone") : "";
        return (
          <div key={i} className={s.histRow}>
            <div className={cx(s.rank, r.rank === 1 && s.first)}>{r.rank}<small>/{r.players}</small></div>
            <div className={s.who}><b>{D.character(r.character)?.display ?? r.character}</b><small>{modeLabel(r.mode)} · {r.time}</small></div>
            <div className={s.gain}>+{n0(r.exp)} EXP{r.fireUsed > 0 && <small>{tr("history.fireUsed", { n: r.fireUsed })}</small>}</div>
            <div className={s.gain}>{r.coins ? tr("history.gainCoins", { coins: (r.coins > 0 ? "+" : "") + n0(r.coins) }) : ""}{r.stars > 0 && <small>{tr("history.starsGained", { n: r.stars })}</small>}</div>
            {/* 「回放」 -- open this match's `.bdrec` in the player. No link
                (an online match, or one the store no longer holds) disables
                it and says why on hover. */}
            <div className={s.act} title={tip}>
              <Btn
                size="small"
                kind="pink"
                className={link && link.kind !== "none" ? undefined : s.actOff}
                disabled={!link || link.kind === "none"}
                onClick={() => {
                  if (link) void openHistoryReplay(link);
                }}
              >{tr("history.replay")}</Btn>
            </div>
          </div>
        );
      })}
    </div>
  );
}

export function showHistory(): void {
  markSeen("history");
  openModal(tr("menu.history"), <History />, { size: "wide" });
}

function Fire({ close }: { close: () => void }) {
  const p = useProfile()!;
  return (
    <div className={s.fire}>
      <p>{tr("fire.info")}</p>
      <div className={s.fireOpts}>
        {[0, 1, 2, 3].map((n) => (
          <button key={n} type="button" className={cx(s.fireOpt, p.firePerGame === n && s.on)} onClick={() => {
            updateProfile(rules.profile_set_fire_per_game(profileJson(), n));
            close();
            toast(n ? tr("fire.setToast", { n, mult: n + 1 }) : tr("fire.setNoneToast"));
          }}>
            <b>{n === 0 ? tr("fire.none") : tr("fire.count", { n })}</b>
            <small>{tr("fire.perMatch", { n: n + 1 })}</small>
          </button>
        ))}
      </div>
    </div>
  );
}

export function showFire(): void {
  openModal(tr("fire.title"), (close) => <Fire close={close} />, { size: "mid" });
}

function Settings({ close }: { close: () => void }) {
  const st = useSettings();
  const [name, setName] = useState(getProfile().playerName);
  const update = (patch: Partial<SoundSettings>) => {
    saveSettings({ ...st, ...patch });
    applyVolumes();
  };
  const slider = (label: string, key: "bgm" | "voice" | "se", icon: string) => (
    <label className={s.setRow}>
      <span className={s.setLabel}><Icon name={icon} />{label}</span>
      <input className={s.range} type="range" min={0} max={10} step={0.1} value={st[key]} onChange={(e) => { update({ [key]: Number(e.target.value) }); if (key === "se") sfx("tap"); }} />
      <span className={s.setVal}>{st[key]}</span>
    </label>
  );
  const toggle = (label: string, key: "skipLine" | "greet" | "idleTalk" | "keepAwake") => (
    <label className={s.setRow}>
      <span className={s.setLabel}>{label}</span>
      <input className={s.switch} type="checkbox" checked={st[key]} onChange={(e) => update({ [key]: e.target.checked })} />
    </label>
  );
  return (
    <div className={s.settings}>
      {slider(tr("settings.bgm"), "bgm", "music_note")}
      {slider(tr("settings.voice"), "voice", "chat")}
      {slider(tr("settings.se"), "se", "volume_up")}
      {toggle(tr("settings.skipLine"), "skipLine")}
      {toggle(tr("settings.greet"), "greet")}
      {toggle(tr("settings.keepAwake"), "keepAwake")}
      <div className={s.setRow}>
        <span className={s.setLabel}>{tr("settings.language")}</span>
        <div className={s.langs}>
          {LANGS.map((l) => (
            <button key={l} type="button" className={l === savedLang() ? s.langOn : s.lang} onClick={() => {
              // Reopen this popup after switching so its own title follows too.
              void setLang(l as Lang).then(() => {
                close();
                showSettings();
              });
            }}>
              {LANG_LABEL[l as Lang]}
            </button>
          ))}
        </div>
      </div>
      <div className={s.setRow}>
        <span className={s.setLabel}><Icon name="edit" />{tr("settings.name")}</span>
        <TextInput className={s.grow} value={name} maxLength={16} onChange={(e) => setName(e.target.value)} />
        <Btn size="small" onClick={() => {
          if (!name.trim()) return;
          updateProfile(JSON.stringify({ ...getProfile(), playerName: name.trim() }));
          toast(tr("settings.renamed"));
        }}>{tr("settings.rename")}</Btn>
      </div>
      <div className={s.end}>
        <Btn size="small" className={s.danger} onClick={async () => {
          if (await ask(tr("settings.deleteSave"), tr("settings.deleteAsk"), tr("settings.delete"))) {
            deleteProfile();
            location.replace("/");
          }
        }}>{tr("settings.deleteSave")}</Btn>
      </div>
    </div>
  );
}

export function showSettings(): void {
  openModal(tr("menu.settings"), (close) => <Settings close={close} />, { size: "mid" });
}
