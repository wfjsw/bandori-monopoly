// Boot (BootController): load manifest, fonts, game data and the wasm rules;
// create a profile on the first run. After a refresh (a route in the URL and a
// profile on disk) it goes straight back to that screen.

import { type FormEvent, useEffect, useState } from "react";
import { navigate } from "../../app/router";
import { useBackdrop } from "../../app/Stage";
import { installFonts, loadManifest, res } from "../../core/assets";
import { loadCardLocales } from "../../i18n";
import { playSceneBgm, unlockAudio } from "../../core/audio";
import { loadGameData } from "../../core/data";
import { preloadImage } from "../../core/preload";
import { createProfile, hasProfile, loadProfile } from "../../core/store";
import { Btn } from "../../ui/Button";
import { toast } from "../../ui/Toast";
import s from "./Boot.module.css";
import { t as tr } from "../../i18n/t";

const TIPS = [
  tr("boot.tip.circle"),
  tr("boot.tip.dealer"),
  tr("boot.tip.mortgage"),
  tr("boot.tip.timer"),
  tr("boot.tip.vote"),
  tr("boot.tip.disconnect"),
  tr("boot.tip.refresh"),
];

/**
 * Boot-screen splash ids (`BandoriBoot/<id>`). Presentation only: which splash
 * shows is a UI decision, so the list lives here rather than in the ruleset
 * metadata. The keys match the asset set (`tools/asset-pipe/check.py`).
 */
const BOOT_ART = ["001", "002", "003", "004", "005", "018", "021", "045"];

/** Splash for this page load, picked at random -- not from the home persona.
 *  The art must not wait on the roster (or the progress bar), so the choice is
 *  made before any game data is in and every boot of the same page shows it. */
const bootArtId = BOOT_ART[Math.floor(Math.random() * BOOT_ART.length)];

const bootArtUrl = () => res(`BandoriBoot/${bootArtId}`);

/**
 * Full-bleed boot splash. The URL lands with the asset manifest (the first
 * fetch of `loadAll`); until then the layer is just the white plate. It is up
 * for the whole load, not gated on game data.
 */
function BootArt() {
  const art = bootArtUrl();
  return <div className={s.art} style={{ backgroundImage: art ? `url("${art}")` : undefined }} />;
}

let loading: Promise<void> | null = null;
let progressNow = 0;
let onProgress: (p: number) => void = () => {};

function loadAll(progress: (p: number) => void): Promise<void> {
  // The latest caller wins. StrictMode (and any remount) swaps the hook in
  // while the one-shot load is already in flight; without this the bar froze
  // at zero and nothing re-rendered once the manifest landed -- so the splash,
  // whose URL the manifest carries, never painted either.
  onProgress = progress;
  if (loading) {
    onProgress(progressNow);
    return loading;
  }
  // `loadGameData` is the single gate: it loads the ruleset first (the roster
  // source), then the tables, and assigns `D` only at the end. Deck builder /
  // gallery / home may read `D` as soon as this resolves -- never before.
  loading = (async () => {
    const tick = (p: number) => {
      progressNow = p;
      onProgress(p);
    };
    await loadManifest();
    installFonts();
    void loadCardLocales();
    // The splash needs only the manifest. Start its download now so it runs
    // alongside the wasm / ruleset instead of after the progress bar fills.
    void preloadImage(bootArtUrl());
    tick(0.15);
    await loadGameData((p) => tick(0.15 + p * 0.75));
    loadProfile();
    tick(1);
  })();
  return loading;
}

type Stage = "loading" | "tap" | "create" | "error";

/**
 * One-shot boot: manifest, fonts, then `loadGameData` (ruleset + tables + the
 * typed `D`), with a progress meter. On success it plays the boot BGM and
 * either hands back to the shell (`resume`) or offers the tap / create screens.
 * A throw leaves `D` unset and lands on the error stage -- `loadGameData`
 * assigns `D` only after every piece is in.
 */
function useBootAssets(opts: {
  resume: boolean;
  onReady: () => void;
  setP: (p: number) => void;
  setStage: (s: Stage) => void;
  setError: (e: string) => void;
}): void {
  const { resume, onReady, setP, setStage, setError } = opts;
  useEffect(() => {
    let alive = true;
    loadAll((v) => alive && setP(v))
      .then(() => {
        if (!alive) return;
        playSceneBgm("boot");
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
    // Mount-only: the boot sequence runs once (its loaders are memoized).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

export function Boot({ resume, onReady }: { resume: boolean; onReady: () => void }) {
  useBackdrop(null);
  const [p, setP] = useState(0);
  const [stage, setStage] = useState<Stage>("loading");
  const [error, setError] = useState("");
  const [tip] = useState(() => TIPS[Math.floor(Math.random() * TIPS.length)]);
  const [name, setName] = useState("");
  useBootAssets({ resume, onReady, setP, setStage, setError });

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
      <BootArt />
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
