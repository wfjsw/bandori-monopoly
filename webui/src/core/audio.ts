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

// One AudioContext serves BGM, sfx and voice, and every clip is decoded once and
// replayed as a buffer source. Nothing is an <audio>/<video> element, so nothing
// can drag the page onto the "playback" audio session behind our back.
let ac: AudioContext | null = null;

function ctx(): AudioContext | null {
  if (!ac) {
    try {
      ac = new AudioContext();
    } catch {
      return null;
    }
    setSession(BASE_SESSION);
  }
  return ac;
}

// --- Audio session ---------------------------------------------------------
// The iOS ringer switch mutes the "ambient" session but not "playback". BGM
// stays ambient, so it follows the switch; sfx and voice are bumped to
// "playback" while they sound, so they are heard anyway. Safari-only -- Chrome
// and Android have no Audio Session API, and their Web Audio already plays on
// the media stream, which the ringer switch does not touch.
type SessionType = "auto" | "playback" | "transient" | "transient-solo" | "ambient" | "play-and-record";

const BASE_SESSION: SessionType = "ambient";
/** Keep the page on "playback" this long after the last sfx/voice ends. */
const RELEASE_MS = 400;

function setSession(type: SessionType): void {
  const session = (navigator as Navigator & { audioSession?: { type: SessionType } }).audioSession;
  if (!session || session.type === type) return;
  try {
    session.type = type;
  } catch {
    // No page context (`type` throws InvalidStateError).
  }
}

let audible = 0;
let releaseTimer = 0;

function holdAudible(): void {
  if (releaseTimer) {
    clearTimeout(releaseTimer);
    releaseTimer = 0;
  }
  if (audible++ === 0) setSession("playback");
}

function releaseAudible(): void {
  if (audible === 0) return;
  if (--audible > 0) return;
  releaseTimer = window.setTimeout(() => {
    releaseTimer = 0;
    if (audible === 0) setSession(BASE_SESSION);
  }, RELEASE_MS);
}

type Bgm = { track: string; gain: GainNode; source: AudioBufferSourceNode; level: number; started: boolean };

let current: Bgm | null = null;
let pending: string | null = null;
let seq = 0;
let unlocked = false;

/** Browsers block audio until the first user gesture. */
export function unlockAudio(): void {
  if (unlocked) return;
  unlocked = true;
  const c = ctx();
  if (c && c.state !== "running") void c.resume();
  if (current) startBgm(current);
}

function startBgm(b: Bgm): void {
  if (b.started) return;
  b.started = true;
  b.source.start();
  fadeTo(b.gain, b.level);
}

function fadeTo(g: GainNode, to: number, then?: () => void): void {
  const from = g.gain.value;
  const start = performance.now();
  const step = (t: number) => {
    const k = Math.min(1, (t - start) / (FADE * 1000));
    g.gain.value = from + (to - from) * k;
    if (k < 1) requestAnimationFrame(step);
    else then?.();
  };
  requestAnimationFrame(step);
}

function stopBgm(b: Bgm): void {
  fadeTo(b.gain, 0, () => {
    b.source.stop();
    b.gain.disconnect();
  });
}

function startNewBgm(track: string, buf: AudioBuffer): void {
  const c = ctx();
  if (!c) return;
  const old = current;
  const gain = c.createGain();
  gain.gain.value = 0;
  gain.connect(c.destination);
  const source = c.createBufferSource();
  source.buffer = buf;
  source.loop = true;
  source.connect(gain);
  const b: Bgm = { track, gain, source, level: settings().bgm / 10, started: false };
  current = b;
  if (old) stopBgm(old);
  if (unlocked) startBgm(b);
}

function stopBgmNow(): void {
  const b = current;
  if (!b) return;
  current = null;
  stopBgm(b);
}

export function playSceneBgm(scene: string): void {
  const track = SCENE_TRACKS[scene];
  if (!track) {
    seq++;
    pending = null;
    return stopBgmNow();
  }
  if (track === current?.track || track === pending) return;
  const src = res(`BandoriBGM/${track}`);
  if (!src || !ctx()) {
    seq++;
    pending = null;
    return stopBgmNow();
  }
  const mine = ++seq;
  pending = track;
  void bgmBuffer(src).then((buf) => {
    if (mine !== seq) return; // superseded by a newer scene
    pending = null;
    if (buf) startNewBgm(track, buf);
  });
}

export function applyVolumes(): void {
  if (!current) return;
  current.level = settings().bgm / 10;
  current.gain.gain.value = current.level;
}

// Decoded BGM is far bigger than an sfx clip (a whole track of PCM), so only
// the few most recent tracks are kept; a playing source holds its own buffer.
const BGM_CACHE = 3;
const bgmBuffers = new Map<string, AudioBuffer>();
const bgmLoads = new Map<string, Promise<AudioBuffer | null>>();

function bgmBuffer(src: string): Promise<AudioBuffer | null> {
  const hit = bgmBuffers.get(src);
  if (hit) {
    bgmBuffers.delete(src); // re-insert: recent tracks are evicted first
    bgmBuffers.set(src, hit);
    return Promise.resolve(hit);
  }
  const loading = bgmLoads.get(src);
  if (loading) return loading;
  const c = ctx();
  if (!c) return Promise.resolve(null);
  const p = fetch(src)
    .then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(String(r.status)))))
    .then((data) => c.decodeAudioData(data))
    .then((buf) => {
      bgmBuffers.set(src, buf);
      while (bgmBuffers.size > BGM_CACHE) bgmBuffers.delete(bgmBuffers.keys().next().value!);
      return buf;
    })
    .catch(() => null)
    .finally(() => bgmLoads.delete(src));
  bgmLoads.set(src, p);
  return p;
}

const lastPlayed = new Map<string, number>();

// Sfx and voice clips are small, so they stay decoded for the whole session.
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
  holdAudible();
  node.onended = () => {
    g.disconnect();
    releaseAudible();
  };
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
