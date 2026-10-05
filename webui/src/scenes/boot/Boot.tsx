// Boot (BootController): load manifest, fonts, game data and the wasm rules;
// create a profile on the first run. After a refresh (a route in the URL and a
// profile on disk) it goes straight back to that screen.

import { type FormEvent, useEffect, useState } from "react";
import { navigate } from "../../app/router";
import { useBackdrop } from "../../app/Stage";
import { installFonts, loadManifest, res } from "../../core/assets";
import { loadCardLocales } from "../../i18n";
import { playSceneBgm, unlockAudio } from "../../core/audio";
import { D, loadGameData } from "../../core/data";
import { createProfile, getProfile, hasProfile, loadProfile } from "../../core/store";
import { Btn } from "../../ui/Button";
import { toast } from "../../ui/Toast";
import s from "./Boot.module.css";
import { t as tr } from "../../i18n/t";

/** BootController.BandKeys -- boot art per home character's band. */
const BAND_KEYS: Record<string, string> = {
  "Poppin' Party": "001", Afterglow: "002", "Hello, Happy World!": "003", "Pastel✽Palettes": "004",
  Roselia: "005", "RAISE A SUILEN": "018", Morfonica: "021", "MyGO!!!!!": "045", CRYCHIC: "045",
};

const TIPS = [
  tr("boot.tip.circle"),
  tr("boot.tip.dealer"),
  tr("boot.tip.mortgage"),
  tr("boot.tip.timer"),
  tr("boot.tip.vote"),
  tr("boot.tip.disconnect"),
  tr("boot.tip.refresh"),
];

let loading: Promise<void> | null = null;
function loadAll(progress: (p: number) => void): Promise<void> {
  loading ??= (async () => {
    await loadManifest();
    installFonts();
    void loadCardLocales();
    progress(0.15);
    await loadGameData((p) => progress(0.15 + p * 0.75));
    loadProfile();
    progress(1);
  })();
  return loading;
}

type Stage = "loading" | "tap" | "create" | "error";

export function Boot({ resume, onReady }: { resume: boolean; onReady: () => void }) {
  useBackdrop(null);
  const [p, setP] = useState(0);
  const [stage, setStage] = useState<Stage>("loading");
  const [error, setError] = useState("");
  const [art, setArt] = useState("");
  const [tip] = useState(() => TIPS[Math.floor(Math.random() * TIPS.length)]);
  const [name, setName] = useState("");

  useEffect(() => {
    let alive = true;
    loadAll((v) => alive && setP(v))
      .then(() => {
        if (!alive) return;
        playSceneBgm("boot");
        const home = hasProfile() ? D.character(getProfile().homeCharacter) : undefined;
        setArt(res(`BandoriBoot/${BAND_KEYS[home?.band ?? ""] ?? "045"}`));
        if (!hasProfile()) setStage("create");
        else if (resume) onReady();
        else setStage("tap");
      })
      .catch((e: Error) => {
        if (!alive) return;
        setError(String(e.message ?? e));
        setStage("error");
      });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const start = () => {
    if (stage !== "tap") return;
    unlockAudio();
    navigate({ name: "menu" }, { replace: true });
    onReady();
  };

  const create = (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return toast(tr("boot.nameEmpty"));
    createProfile(name);
    unlockAudio();
    if (!resume) navigate({ name: "menu" }, { replace: true });
    onReady();
  };

  return (
    <div className={s.boot} onClick={start}>
      <div className={s.art} style={{ backgroundImage: art ? `url("${art}")` : undefined }} />
      <div className={s.panel}>
        {res("BandoriUI/transition_logo") && <img className={s.logo} src={res("BandoriUI/transition_logo")} alt="" />}
        <div className={s.tip}>{tip}</div>
        <div className={s.progress}><div style={{ width: `${Math.round(p * 100)}%` }} /></div>
        <div className={stage === "error" ? s.error : s.status}>
          {stage === "loading" ? tr("boot.loading") : stage === "tap" ? tr("boot.tapToStart") : stage === "error" ? error : tr("boot.needSave")}
        </div>
      </div>
      {stage === "create" && (
        <div className={s.createBack}>
          <form className={s.create} onSubmit={create} onClick={(e) => e.stopPropagation()}>
            <h2>{tr("boot.createTitle")}</h2>
            <p>{tr("boot.createText")}</p>
            <input className={s.input} maxLength={16} placeholder={tr("boot.namePlaceholder")} value={name} onChange={(e) => setName(e.target.value)} autoFocus />
            <Btn kind="pink" type="submit" wide>{tr("common.start")}</Btn>
          </form>
        </div>
      )}
    </div>
  );
}
