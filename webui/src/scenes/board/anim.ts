// Event playback (BoardDemoController.PlayEvents): events are queued and
// played one by one -- dice roll, token walk, banners, card reveals -- like
// the original. While the queue runs, tokens follow the animation instead of
// the state; when it drains everything reconciles with the state.

import { useEffect, useReducer, useRef, useState } from "react";
import { sfx } from "../../core/audio";
import { namesOf } from "../../core/names";
import { D } from "../../core/data";
import { fmtMsg, type Names } from "../../i18n/msg";
import type { MatchEvent, MatchView } from "../../core/types";
import { showEffect } from "./Popups";

/** How long the dice face rolls before it lands. */
const DICE_ROLL_MS = 10 * 55;
/** How long the effect popup stays up: just longer than the dice roll it
 *  follows, but never under 1.5s -- long enough to read which branch landed. */
const EFFECT_HOLD_MS = Math.max(DICE_ROLL_MS + 300, 1500);
import type { GameSession } from "../../game/session";
import { t as tr } from "../../i18n/t";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** True while the queue is deep enough (or the replay is at 4x) that the
 *  events should just snap into place instead of playing out. */
const FAST_QUEUE = 8;

const EVENT_SFX: Record<string, string> = {
  gain: "coin_gain", pass: "bonus", lose: "coin_pay", draw: "draw", mulligan: "draw", discard: "card_play",
};

export interface LogLine { id: number; text: string; turn: boolean }

/** A turn-stage change, queued behind the events that led to it. `step` is the
 *  engine's stage number (1..4 = 开始 / 运营 / 移动 / 结束). */
interface StageMark { step: number; newTurn: boolean; label?: string }

type Item = { ev: MatchEvent } | { mark: StageMark };

/** How long a stage sweep holds (`showPhase`). A new turn's 回合 → 开始 → 运营
 *  run waits this long between sweeps, so 开始 is on screen as long as any
 *  other stage (结束 included) rather than cut short by the next sweep. */
const PHASE_MS = 1500;

/** The sweep / label key of an engine stage number. */
function stageKey(step: number): string {
  return step >= 4 ? "board.stepEnd" : step === 3 ? "board.stepMove" : step === 2 ? "board.stepOps" : "common.start";
}

export class Animator {
  private queue: Item[] = [];
  animating = false;
  /** The stage the marker shows while the queue plays (engine numbering), or
   *  null to follow the state. The engine rolls, walks and reaches 结束 in one
   *  call, so a single update carries the dice, the walk *and* `step = 结束` --
   *  read straight off the state, the marker (and the sweep) would say 结束
   *  before the dice land, flip to 移动 for the walk, then back. Instead the
   *  main roll opens 移动 as it plays, and each state's stage change is queued
   *  behind that update's events (`stageTo`), so stages show in the order the
   *  player sees them happen. */
  stage: number | null = null;
  /** Token positions while animating (null: use the state). */
  pos: number[] | null = null;
  dice = 0;
  rolling = false;
  banner: { title: string; body: string; id: number } | null = null;
  /** Stage transition sweeping in. `key` names the stage; `label` overrides the
   *  swept text -- a new turn's first sweep names whose turn it is. */
  phase: { key: string; label?: string; id: number } | null = null;
  reveal: { card: string; out: boolean; id: number } | null = null;
  hop: { playerId: number; id: number } | null = null;
  lastDiscard = "";
  log: LogLine[] = [];
  /** Replay speed (1 / 2 / 4): scales every sleep and timer. At 4 the queue
   *  takes the fast path and nothing waits at all. */
  speed = 1;
  private seq = 0;
  private bannerTimer = 0;
  private phaseTimer = 0;
  private disposed = false;

  constructor(private bump: () => void, private view: () => MatchView | null) {}

  /** `sleep` scaled by [`speed`]. */
  private nap(ms: number): Promise<void> {
    return sleep(ms / Math.max(1, this.speed)) as Promise<void>;
  }

  /** The queue plays fast when it is deep, or when a replay is at 4x. */
  private fast(): boolean {
    return this.queue.length > FAST_QUEUE || this.speed >= 4;
  }

  /** (Re)attach -- React StrictMode unmounts and remounts once in development. */
  revive(): void {
    this.disposed = false;
    if (this.queue.length) {
      this.animating = false;
      void this.run();
    }
  }

  dispose(): void {
    this.disposed = true;
    clearTimeout(this.bannerTimer);
    clearTimeout(this.phaseTimer);
  }

  addLog(e: MatchEvent): void {
    const line = fmtMsg(e.msg, this.names());
    if (!line) return;
    this.log = [...this.log.slice(-199), { id: e.id, text: line, turn: e.type === "turn" }];
  }

  /** Names for message arguments (players/tiles/cards of the running match). */
  private names(): Names {
    return namesOf(this.view()?.state);
  }

  push(e: MatchEvent): void {
    this.queue.push({ ev: e });
    void this.run();
  }

  /** The state moved from stage `from` to `mark.step`. The view arrives after
   *  the events that produced it (solo `pump` and the SSE frames both send
   *  events first), so queueing the change here plays it after them; until it
   *  plays, the marker holds the stage the animation started in. */
  stageTo(from: number, mark: StageMark): void {
    if (this.stage == null) this.stage = from;
    this.queue.push({ mark });
    void this.run();
  }

  private async enterStage(m: StageMark, fast: boolean): Promise<void> {
    if (!m.newTurn) return this.toStage(m.step, m.label);
    // A new turn starts from no roll -- don't leave the previous player's face
    // sitting on the dice.
    this.dice = 0;
    // A new turn sweeps 「<player> 的回合」, then 开始, then the stage the turn
    // has reached (usually 运营 -- the engine runs through 开始 in the same
    // call), each like any other stage change.
    const gap = () => (fast ? Promise.resolve() : this.nap(PHASE_MS));
    this.stage = 1;
    if (m.label) {
      this.showPhase(stageKey(1), m.label);
      await gap();
    }
    this.showPhase(stageKey(1));
    if (m.step > 1) {
      await gap();
      this.toStage(m.step, undefined, true);
    }
  }

  /** Put the marker on `step`, sweeping its name only when that is a change:
   *  the roll opens 移动 itself, so a state that stopped mid-walk (a [反击]
   *  window) arriving at 移动 behind it must not sweep 移动 a second time. */
  private toStage(step: number, label?: string, force = false): void {
    const cur = this.stage ?? this.view()?.state.step;
    this.stage = step;
    if (force || cur !== step) this.showPhase(stageKey(step), label);
  }

  private showBanner(title: string, body = ""): void {
    this.banner = { title, body, id: ++this.seq };
    clearTimeout(this.bannerTimer);
    this.bannerTimer = window.setTimeout(() => {
      this.banner = null;
      this.bump();
    }, 2200 / Math.max(1, this.speed));
    this.bump();
  }

  /** Chain/resolve the stage name across the board (Master Duel's phase change):
   *  chain links snap in across the banner, then it resolves. The effect runs
   *  for 1.5s; the marker does not wait for it (see `stage`). */
  showPhase(key: string, label?: string): void {
    this.phase = { key, label, id: ++this.seq };
    clearTimeout(this.phaseTimer);
    this.phaseTimer = window.setTimeout(() => {
      this.phase = null;
      this.bump();
    }, PHASE_MS / Math.max(1, this.speed));
    this.bump();
  }

  private async run(): Promise<void> {
    if (this.animating) return;
    this.animating = true;
    this.pos = this.view()?.state.players.map((x) => x.pos) ?? null;
    this.bump();
    while (this.queue.length && !this.disposed) {
      const it = this.queue.shift()!;
      try {
        if ("mark" in it) await this.enterStage(it.mark, this.fast());
        else await this.play(it.ev, this.fast());
      } catch (err) {
        console.error(err);
      }
    }
    if (this.disposed) return;
    this.animating = false;
    this.pos = null;
    // Caught up: the last stage played is the state's own.
    this.stage = null;
    this.bump();
  }

  private name(playerId: number): string {
    const v = this.view();
    return playerId === v?.playerId ? tr("common.you") : v?.state.players[playerId]?.player ?? "";
  }

  private player(playerId: number): string {
    return this.view()?.state.players[playerId]?.player ?? "";
  }

  private async diceAnim(value: number): Promise<void> {
    this.rolling = true;
    sfx("dice_roll");
    for (let k = 0; k < DICE_ROLL_MS / 55; k++) {
      this.dice = 1 + Math.floor(Math.random() * 20);
      this.bump();
      await this.nap(55);
    }
    this.dice = value;
    this.rolling = false;
    this.bump();
    sfx("dice_result");
  }

  private async walk(playerId: number, from: number, steps: number, hop: number): Promise<void> {
    if (!this.pos) return;
    const n = D.tiles.length;
    const dir = steps >= 0 ? 1 : -1;
    let p = from;
    this.pos[playerId] = from;
    this.bump();
    try {
      for (let k = 0; k < Math.abs(steps); k++) {
        p = (((p + dir) % n) + n) % n;
        this.pos[playerId] = p;
        if (hop > 0) {
          this.hop = { playerId, id: ++this.seq };
          sfx("step");
          this.bump();
          await this.nap(hop);
        }
      }
    } finally {
      this.hop = null;
      this.bump();
    }
  }

  private async play(e: MatchEvent, fast: boolean): Promise<void> {
    const body = fmtMsg(e.msg, this.names());
    this.addLog(e);
    this.bump();
    const v = this.view();
    const playerId = e.playerId;
    const ok = playerId >= 0 && !!v && playerId < v.state.players.length;
    const wait = (ms: number) => (fast ? Promise.resolve() : this.nap(ms));
    switch (e.type) {
      case "turn":
        // The turn announcement rides the 开始 stage sweep (see `showPhase`) --
        // there is no separate 「<player> 的回合」 card over the board.
        if (!ok) break;
        if (!fast) sfx(playerId === v!.playerId ? "my_turn" : "turn");
        await wait(300);
        break;
      case "roll": {
        if (!ok) break;
        // The main move roll is where 移动 begins. The engine usually passes
        // through MOVE inside one call, so no state shows it -- the roll does.
        this.toStage(3);
        const d = Math.abs(e.dice || e.value);
        if (!fast) await this.diceAnim(d);
        else this.dice = d;
        this.showBanner(tr("board.roll", { n: d }), body);
        await wait(450);
        await this.walk(playerId, e.from, e.value, fast ? 0 : 130);
        break;
      }
      case "dice": {
        // A card's own roll (`ctx::roll`): same dice visual and pacing as the
        // main move roll, so a card's 「roll 1d10」 reads as a roll and settles
        // before the effect it decides is applied.
        if (!ok) break;
        const d = Math.abs(e.dice || e.value);
        if (!fast) await this.diceAnim(d);
        else this.dice = d;
        this.showBanner(tr("board.roll", { n: d }), body);
        await wait(450);
        break;
      }
      case "effect": {
        // Which effect a card just applied. Additive to the log line the same
        // message already produced -- this is the popup window on top of it.
        if (!fast) sfx("place");
        showEffect(body, EFFECT_HOLD_MS);
        await wait(EFFECT_HOLD_MS);
        break;
      }
      case "move":
        if (!ok) break;
        this.showBanner(this.player(playerId), body);
        await this.walk(playerId, e.from, e.value, fast ? 0 : 90);
        break;
      case "teleport":
        if (!ok || !this.pos) break;
        if (!fast) sfx("teleport");
        this.pos[playerId] = e.to;
        this.bump();
        await wait(350);
        break;
      case "rent": case "pay":
        this.showBanner(e.type === "rent" ? tr("anim.payRent") : tr("anim.pay"), body);
        if (!fast) sfx("coin_pay");
        await wait(e.type === "rent" ? 800 : 350);
        break;
      case "buy": this.showBanner(tr("anim.buyOk"), body); if (!fast) sfx("buy"); await wait(600); break;
      case "build": this.showBanner(tr("anim.build"), body); if (!fast) sfx("build"); await wait(600); break;
      case "mortgage": case "redeem":
        this.showBanner(e.type === "mortgage" ? tr("board.mortgageDeeds") : tr("board.redeemDeeds"), body);
        if (!fast) sfx("mortgage");
        await wait(500);
        break;
      case "forcebuy": this.showBanner(tr("anim.forceBuy"), body); if (!fast) sfx("buy"); await wait(800); break;
      case "bankrupt": case "left":
        if (!ok) break;
        this.showBanner(e.type === "bankrupt" ? (playerId === v!.playerId ? tr("anim.youBankrupt") : tr("anim.bankruptOf", { who: this.player(playerId) })) : tr("anim.leftOf", { who: this.player(playerId) }), body);
        if (!fast) sfx(e.type === "bankrupt" ? "bankrupt" : "place");
        await wait(1400);
        break;
      case "vote": this.showBanner(tr("board.voteTitle"), body); if (!fast) sfx("prompt"); break;
      case "draw": case "mulligan": case "discard": case "ai":
        this.showBanner(({ draw: tr("anim.draw"), discard: tr("anim.discard"), mulligan: tr("anim.redraw"), ai: tr("anim.aiTakeover") } as Record<string, string>)[e.type], body);
        if (e.type === "discard" && e.card) this.lastDiscard = e.card;
        if (!fast && EVENT_SFX[e.type]) sfx(EVENT_SFX[e.type]);
        await wait(450);
        break;
      case "play":
        if (!e.card) break;
        this.lastDiscard = e.card;
        this.showBanner(ok ? tr("anim.playedBy", { who: this.name(playerId) }) : tr("board.play"), body);
        if (fast) break;
        sfx("card_play");
        this.reveal = { card: e.card, out: false, id: ++this.seq };
        this.bump();
        await this.nap(1100);
        this.reveal = { ...this.reveal, out: true };
        this.bump();
        await this.nap(250);
        this.reveal = null;
        this.bump();
        break;
      case "event": if (!fast) sfx("event_card"); this.showBanner(tr("anim.event"), body); await wait(900); break;
      case "eventend": this.showBanner(tr("anim.eventEnd"), body); await wait(500); break;
      case "status": this.showBanner(ok ? this.player(playerId) : tr("anim.status"), body); await wait(500); break;
      case "skill": this.showBanner(ok ? tr("anim.skillOf", { who: this.player(playerId) }) : tr("anim.skill"), body); if (!fast) sfx("place"); await wait(500); break;
      case "place": case "unplace": this.showBanner(e.type === "place" ? tr("anim.place") : tr("anim.unplace"), body); if (!fast) sfx("place"); await wait(450); break;
      case "pass": case "gain": case "lose": if (!fast) sfx(EVENT_SFX[e.type]); await wait(250); break;
      default: await wait(120);
    }
  }
}

/** The board's view of the session plus the event animator. */
export function useBoardSession(sess: GameSession): { view: MatchView | null; at: number; anim: Animator } {
  const [, bump] = useReducer((x: number) => x + 1, 0);
  const [state, set] = useState<{ view: MatchView | null; at: number }>(() => ({ view: sess.view, at: performance.now() }));
  const viewRef = useRef(state.view);
  viewRef.current = state.view;
  const animRef = useRef<Animator | null>(null);
  if (!animRef.current) {
    const a = new Animator(bump, () => viewRef.current);
    for (const e of sess.view?.state.events ?? []) a.addLog(e);
    animRef.current = a;
  }
  useEffect(() => {
    const a = animRef.current!;
    // A replay drives `Animator.speed` (1 / 2 / 4) through the session.
    const syncSpeed = () => {
      a.speed = sess.animSpeed;
    };
    syncSpeed();
    const off = sess.subscribe(
      (v) => {
        const prev = viewRef.current?.state;
        viewRef.current = v;
        set({ view: v, at: performance.now() });
        // Stage transition: every turn-stage change sweeps the
        // stage name across the board. The rulebook's four stages are
        // 开始 / 运营 / 移动 / 结束 (`rulebook.txt:2957`) and the engine's `step`
        // is 0 before a turn and 1..4 for those, so `step - 1` indexes the
        // label list (`Side.tsx`'s `phases()`).
        const now = v.state;
        if (prev && now.phase === "play" && (prev.turn !== now.turn || prev.step !== now.step)) {
          const newTurn = prev.turn !== now.turn;
          // A new turn first sweeps whose turn it is (this replaces the separate
          // 「<player> 的回合」 card that used to sit over the board), then 开始
          // and the stage reached -- see `enterStage`.
          const who = now.players[now.turn]?.player ?? "";
          const label = newTurn ? (now.turn === v.playerId ? tr("anim.yourTurn") : tr("anim.turnOf", { who })) : undefined;
          a.stageTo(prev.step, { step: now.step, newTurn, label });
        }
      },
      (e) => a.push(e),
      syncSpeed,
    );
    return () => {
      off();
    };
  }, [sess]);
  useEffect(() => {
    animRef.current!.revive();
    return () => animRef.current?.dispose();
  }, []);
  return { ...state, anim: animRef.current };
}
