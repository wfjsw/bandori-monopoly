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

// One AudioContext serves BGM, sfx and voice. Creating an <audio> element per
// clip makes iOS re-negotiate its audio session each time (the page flickers),
// and iOS ignores HTMLMediaElement.volume anyway, so everything goes through
// gain nodes on this shared context.
let ac: AudioContext | null = null;

function ctx(): AudioContext | null {
  if (!ac) {
    try {
      ac = new AudioContext();
    } catch {
      return null;
    }
  }
  return ac;
}

type Bgm = { track: string; el: HTMLAudioElement; gain: GainNode | null };

let current: Bgm | null = null;
let unlocked = false;

/** Browsers block audio until the first user gesture. */
export function unlockAudio(): void {
  if (unlocked) return;
  unlocked = true;
  const c = ctx();
  if (c && c.state !== "running") void c.resume();
  if (current) void current.el.play().catch(() => undefined);
}

function bgmVolume(b: Bgm): number {
  return b.gain ? b.gain.gain.value : b.el.volume;
}

function setBgmVolume(b: Bgm, v: number): void {
  v = Math.min(1, Math.max(0, v));
  if (b.gain) b.gain.gain.value = v;
  else b.el.volume = v;
}

function fade(b: Bgm, to: number, then?: () => void): void {
  const from = bgmVolume(b);
  const start = performance.now();
  const step = (t: number) => {
    const k = Math.min(1, (t - start) / (FADE * 1000));
    setBgmVolume(b, from + (to - from) * k);
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
  if (old) fade(old, 0, () => old.el.pause());
  if (!src) {
    current = null;
    return;
  }
  const el = new Audio(src);
  el.loop = true;
  let gain: GainNode | null = null;
  const c = ctx();
  if (c) {
    try {
      gain = c.createGain();
      c.createMediaElementSource(el).connect(gain).connect(c.destination);
    } catch {
      gain = null;
    }
  }
  const b: Bgm = (current = { track, el, gain });
  setBgmVolume(b, 0);
  if (unlocked) void el.play().catch(() => undefined);
  fade(b, settings().bgm / 10);
}

export function applyVolumes(): void {
  if (current) setBgmVolume(current, settings().bgm / 10);
}

const lastPlayed = new Map<string, number>();

// Clips are fetched and decoded once, then replayed as buffer sources.
const clips = new Map<string, Promise<AudioBuffer | null>>();

function clipBuffer(src: string): Promise<AudioBuffer | null> {
  let p = clips.get(src);
  if (!p) {
    const c = ctx();
    if (!c) return Promise.resolve(null);
    p = fetch(src)
      .then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(String(r.status)))))
      .then((data) => c.decodeAudioData(data))
      .catch(() => {
        clips.delete(src); // allow a retry on the next play
        return null;
      });
    clips.set(src, p);
  }
  return p;
}

function playBuffer(buf: AudioBuffer, volume: number, dest?: AudioNode): AudioBufferSourceNode | null {
  const c = ctx();
  if (!c) return null;
  const node = c.createBufferSource();
  node.buffer = buf;
  const g = c.createGain();
  g.gain.value = volume;
  node.connect(g).connect(dest ?? c.destination);
  node.onended = () => g.disconnect();
  node.start();
  return node;
}

/** Sfx.Play(name) -- rate-limited per name. */
export function sfx(name: string, minGapMs = 60): void {
  const level = settings().se;
  if (level <= 0 || !unlocked) return;
  const t = performance.now();
  if (t - (lastPlayed.get(name) ?? -1e9) < minGapMs) return;
  lastPlayed.set(name, t);
  const src = res(`BandoriSE/${name}`);
  if (!src) return;
  void clipBuffer(src).then((buf) => {
    if (buf) playBuffer(buf, level / 10);
  });
}

// Lip sync (Live2DPortrait.UpdateLipSync): the mouth opens from the playing
// voice line's RMS with asymmetric smoothing -- faster to open than to close.
// Voice lines pass through an AnalyserNode and the smoothing lives here so the
// model just reads one value per frame.

let voice: AudioBufferSourceNode | null = null;
let latestVoice: symbol | null = null;
let analyser: AnalyserNode | null = null;
let tapBuf: Float32Array<ArrayBuffer> | null = null;
let mouth = 0;
let forcedMouth = -1;

function voiceAnalyser(c: AudioContext): AnalyserNode {
  if (!analyser) {
    analyser = c.createAnalyser();
    analyser.fftSize = 512;
    analyser.connect(c.destination);
    tapBuf = new Float32Array(analyser.fftSize);
  }
  return analyser;
}

/** A character voice line (`BandoriVoice/<cnId>/<voice>`). */
export function playVoice(cnId: string, line: string): void {
  const src = res(`BandoriVoice/${cnId}/${line}`);
  const c = ctx();
  if (!src || !unlocked || !c) return;
  try {
    voice?.stop();
  } catch {
    // already stopped
  }
  voice = null;
  const token = (latestVoice = Symbol());
  void clipBuffer(src).then((buf) => {
    if (!buf || latestVoice !== token) return; // superseded by a newer line
    const node = playBuffer(buf, settings().voice / 10, voiceAnalyser(c));
    if (!node) return;
    voice = node;
    node.addEventListener("ended", () => {
      if (voice === node) voice = null;
    });
  });
}

/** Raw RMS of the playing voice line, clamped to `[0,1]` (0 when silent). */
export function voiceLevel(): number {
  if (!analyser || !tapBuf || !voice) return 0;
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
