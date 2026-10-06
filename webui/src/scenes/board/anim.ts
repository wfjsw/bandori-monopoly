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

const EVENT_SFX: Record<string, string> = {
  gain: "coin_gain", pass: "bonus", lose: "coin_pay", draw: "draw", mulligan: "draw", discard: "card_play",
};

export interface LogLine { id: number; text: string; turn: boolean }

/** How long the 「<player> 的回合」 banner holds before the next 开始 stage sweeps in. */
const TURN_BANNER_HOLD = 1500;

export class Animator {
  queue: MatchEvent[] = [];
  animating = false;
  /** Token positions while animating (null: use the state). */
  pos: number[] | null = null;
  dice = 0;
  rolling = false;
  banner: { title: string; body: string; id: number } | null = null;
  /** Stage transition: the i18n key of the stage name sweeping in. */
  phase: { key: string; id: number } | null = null;
  reveal: { card: string; out: boolean; id: number } | null = null;
  hop: { playerId: number; id: number } | null = null;
  lastDiscard = "";
  log: LogLine[] = [];
  private seq = 0;
  private bannerTimer = 0;
  private phaseTimer = 0;
  private disposed = false;

  constructor(private bump: () => void, private view: () => MatchView | null) {}

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
    this.queue.push(e);
    void this.run();
  }

  private showBanner(title: string, body = ""): void {
    this.banner = { title, body, id: ++this.seq };
    clearTimeout(this.bannerTimer);
    this.bannerTimer = window.setTimeout(() => {
      this.banner = null;
      this.bump();
    }, 2200);
    this.bump();
  }

  /** Chain/resolve the stage name across the board (Master Duel's phase change):
   *  chain links snap in across the banner, then it resolves. The effect runs
   *  for STAGE_HOLD ms -- the same length the marker holds each stage. */
  showPhase(key: string): void {
    this.phase = { key, id: ++this.seq };
    clearTimeout(this.phaseTimer);
    this.phaseTimer = window.setTimeout(() => {
      this.phase = null;
      this.bump();
    }, 1500);
    this.bump();
  }

  private async run(): Promise<void> {
    if (this.animating) return;
    this.animating = true;
    this.pos = this.view()?.state.players.map((x) => x.pos) ?? null;
    this.bump();
    while (this.queue.length && !this.disposed) {
      const e = this.queue.shift()!;
      try {
        await this.play(e, this.queue.length > 8);
      } catch (err) {
        console.error(err);
      }
    }
    if (this.disposed) return;
    this.animating = false;
    this.pos = null;
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
      await sleep(55);
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
    for (let k = 0; k < Math.abs(steps); k++) {
      p = (((p + dir) % n) + n) % n;
      this.pos[playerId] = p;
      if (hop > 0) {
        this.hop = { playerId, id: ++this.seq };
        sfx("step");
        this.bump();
        await sleep(hop);
      }
    }
    this.hop = null;
    this.bump();
  }

  private async play(e: MatchEvent, fast: boolean): Promise<void> {
    const body = fmtMsg(e.msg, this.names());
    this.addLog(e);
    this.bump();
    const v = this.view();
    const playerId = e.playerId;
    const ok = playerId >= 0 && !!v && playerId < v.state.players.length;
    const wait = (ms: number) => (fast ? Promise.resolve() : sleep(ms));
    switch (e.type) {
      case "turn":
        if (!ok) break;
        this.showBanner(playerId === v!.playerId ? tr("anim.yourTurn") : tr("anim.turnOf", { who: this.player(playerId) }), tr("anim.turnBanner"));
        if (!fast) sfx(playerId === v!.playerId ? "my_turn" : "turn");
        await wait(300);
        break;
      case "roll": {
        if (!ok) break;
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
        await sleep(1100);
        this.reveal = { ...this.reveal, out: true };
        this.bump();
        await sleep(250);
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
    let turnHold = 0;
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
          const label = now.step >= 4 ? "board.stepEnd" : now.step === 3 ? "board.stepMove" : now.step === 2 ? "board.stepOps" : "common.start";
          // A turn change opens with 「<player> 的回合」 (2.2s, from the `turn`
          // event below). The new player's 开始 stage must not sweep in over it:
          // hold the stage name until the banner has read, so the order is the
          // previous player's 结束 -> the turn banner -> this player's 开始.
          clearTimeout(turnHold);
          if (prev.turn !== now.turn && label === "common.start") {
            turnHold = window.setTimeout(() => a.showPhase(label), TURN_BANNER_HOLD);
          } else {
            a.showPhase(label);
          }
        }
      },
      (e) => a.push(e),
    );
    return () => {
      clearTimeout(turnHold);
      off();
    };
  }, [sess]);
  useEffect(() => {
    animRef.current!.revive();
    return () => animRef.current?.dispose();
  }, []);
  return { ...state, anim: animRef.current };
}
