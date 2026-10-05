// BGM (per scene, cross-faded), sound effects and voice lines (BgmPlayer / Sfx).

import { res } from "./assets";
import { settings } from "./store";

/** BgmPlayer.SceneTracks */
const SCENE_TRACKS: Record<string, string> = {
  boot: "21_Enjoy",
  menu: "21_Enjoy",
  lobby: "20_LiveLobby",
  select: "23_BeforeLive",
  board: "12_Odekake",
  gallery: "04_Nobiri",
  deck: "21_Enjoy",
};
const FADE = 0.8;

let current: { track: string; el: HTMLAudioElement } | null = null;
let unlocked = false;

/** Browsers block audio until the first user gesture. */
export function unlockAudio(): void {
  if (unlocked) return;
  unlocked = true;
  if (ac && ac.state === "suspended") void ac.resume();
  if (current) void current.el.play().catch(() => undefined);
}

function fade(el: HTMLAudioElement, to: number, then?: () => void): void {
  const from = el.volume;
  const start = performance.now();
  const step = (t: number) => {
    const k = Math.min(1, (t - start) / (FADE * 1000));
    el.volume = from + (to - from) * k;
    if (k < 1) requestAnimationFrame(step);
    else then?.();
  };
  requestAnimationFrame(step);
}

export function playSceneBgm(scene: string): void {
  const track = SCENE_TRACKS[scene];
  if (!track || current?.track === track) return;
  const src = res(`BandoriBGM/${track}`);
  const old = current;
  if (old) fade(old.el, 0, () => old.el.pause());
  if (!src) {
    current = null;
    return;
  }
  const el = new Audio(src);
  el.loop = true;
  el.volume = 0;
  current = { track, el };
  if (unlocked) void el.play().catch(() => undefined);
  fade(el, settings().bgm / 10);
}

export function applyVolumes(): void {
  if (current) current.el.volume = settings().bgm / 10;
}

const lastPlayed = new Map<string, number>();

/** Sfx.Play(name) -- rate-limited per name. */
export function sfx(name: string, minGapMs = 60): void {
  const level = settings().se;
  if (level <= 0 || !unlocked) return;
  const t = performance.now();
  if (t - (lastPlayed.get(name) ?? -1e9) < minGapMs) return;
  lastPlayed.set(name, t);
  const src = res(`BandoriSE/${name}`);
  if (!src) return;
  const a = new Audio(src);
  a.volume = level / 10;
  void a.play().catch(() => undefined);
}

let voice: HTMLAudioElement | null = null;

// Lip sync (Live2DPortrait.UpdateLipSync): the mouth opens from the playing
// voice line's RMS with asymmetric smoothing -- faster to open than to close.
// We tap the voice element through an AnalyserNode and keep the smoothing here
// so the model just reads one value per frame.

let ac: AudioContext | null = null;
let analyser: AnalyserNode | null = null;
let tapSrc: MediaElementAudioSourceNode | null = null;
let tapEl: HTMLAudioElement | null = null;
let tapBuf: Float32Array<ArrayBuffer> | null = null;
let mouth = 0;
let forcedMouth = -1;

function tap(el: HTMLAudioElement): void {
  try {
    if (!ac) ac = new AudioContext();
    if (ac.state === "suspended") void ac.resume();
    if (tapEl === el) return;
    tapSrc?.disconnect();
    analyser?.disconnect();
    tapSrc = ac.createMediaElementSource(el);
    analyser = ac.createAnalyser();
    analyser.fftSize = 512;
    tapSrc.connect(analyser);
    analyser.connect(ac.destination); // keep the voice audible through the graph
    tapBuf = new Float32Array(analyser.fftSize);
    tapEl = el;
  } catch {
    // tapping can fail (already routed, unsupported context) -- mouth stays shut
    tapEl = null;
    analyser = null;
    tapBuf = null;
  }
}

/** A character voice line (`BandoriVoice/<cnId>/<voice>`). */
export function playVoice(cnId: string, line: string): void {
  const src = res(`BandoriVoice/${cnId}/${line}`);
  if (!src || !unlocked) return;
  voice?.pause();
  voice = new Audio(src);
  voice.volume = settings().voice / 10;
  tap(voice);
  void voice.play().catch(() => undefined);
}

/** Raw RMS of the playing voice line, clamped to `[0,1]` (0 when silent). */
export function voiceLevel(): number {
  if (!analyser || !tapBuf || !voice || voice.paused || voice.ended) return 0;
  analyser.getFloatTimeDomainData(tapBuf);
  let sum = 0;
  for (let i = 0; i < tapBuf.length; i++) sum += tapBuf[i] * tapBuf[i];
  return Math.min(1, Math.sqrt(sum / tapBuf.length) * 7);
}

/** Hold the mouth at a fixed opening (`<0` releases, matching `forcedMouthOpen`). */
export function forceMouth(open: number): void {
  forcedMouth = open;
}

/**
 * `UpdateLipSync` per frame: returns the `PARAM_MOUTH_OPEN_Y` value, or
 * `undefined` when the mouth should not be written (the game only sets the
 * param once it opens past 0.02).
 */
export function mouthOpen(): number | undefined {
  const target = voiceLevel();
  mouth += (target - mouth) * (target > mouth ? 0.6 : 0.3);
  if (forcedMouth >= 0) mouth = forcedMouth;
  return mouth > 0.02 ? mouth : undefined;
}
