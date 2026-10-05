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
import type { GameSession } from "../../game/session";
import { t as tr } from "../../i18n/t";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const EVENT_SFX: Record<string, string> = {
  gain: "coin_gain", pass: "bonus", lose: "coin_pay", draw: "draw", mulligan: "draw", discard: "card_play",
};

export interface LogLine { id: number; text: string; turn: boolean }

export class Animator {
  queue: MatchEvent[] = [];
  animating = false;
  /** Token positions while animating (null: use the state). */
  pos: number[] | null = null;
  dice = 0;
  rolling = false;
  banner: { title: string; body: string; id: number } | null = null;
  /** Master Duel-style stage transition: the i18n key of the phase sweeping in. */
  phase: { key: string; id: number } | null = null;
  reveal: { card: string; out: boolean; id: number } | null = null;
  hop: { seat: number; id: number } | null = null;
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

  /** Names for message arguments (seats/tiles/cards of the running match). */
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

  /** Sweep the phase name across the board (Master Duel's stage transition). */
  showPhase(key: string): void {
    this.phase = { key, id: ++this.seq };
    clearTimeout(this.phaseTimer);
    this.phaseTimer = window.setTimeout(() => {
      this.phase = null;
      this.bump();
    }, 1900);
    this.bump();
  }

  private async run(): Promise<void> {
    if (this.animating) return;
    this.animating = true;
    this.pos = this.view()?.state.seats.map((x) => x.pos) ?? null;
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

  private name(seat: number): string {
    const v = this.view();
    return seat === v?.seat ? tr("common.you") : v?.state.seats[seat]?.player ?? "";
  }

  private player(seat: number): string {
    return this.view()?.state.seats[seat]?.player ?? "";
  }

  private async diceAnim(value: number): Promise<void> {
    this.rolling = true;
    sfx("dice_roll");
    for (let k = 0; k < 10; k++) {
      this.dice = 1 + Math.floor(Math.random() * 20);
      this.bump();
      await sleep(55);
    }
    this.dice = value;
    this.rolling = false;
    this.bump();
    sfx("dice_result");
  }

  private async walk(seat: number, from: number, steps: number, hop: number): Promise<void> {
    if (!this.pos) return;
    const n = D.tiles.length;
    const dir = steps >= 0 ? 1 : -1;
    let p = from;
    this.pos[seat] = from;
    for (let k = 0; k < Math.abs(steps); k++) {
      p = (((p + dir) % n) + n) % n;
      this.pos[seat] = p;
      if (hop > 0) {
        this.hop = { seat, id: ++this.seq };
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
    const seat = e.seat;
    const ok = seat >= 0 && !!v && seat < v.state.seats.length;
    const wait = (ms: number) => (fast ? Promise.resolve() : sleep(ms));
    switch (e.type) {
      case "turn":
        if (!ok) break;
        this.showBanner(seat === v!.seat ? tr("anim.yourTurn") : tr("anim.turnOf", { who: this.player(seat) }), tr("anim.turnBanner"));
        if (!fast) sfx(seat === v!.seat ? "my_turn" : "turn");
        await wait(300);
        break;
      case "roll": {
        if (!ok) break;
        const d = Math.abs(e.dice || e.value);
        if (!fast) await this.diceAnim(d);
        else this.dice = d;
        this.showBanner(tr("board.roll", { n: d }), body);
        await wait(450);
        await this.walk(seat, e.from, e.value, fast ? 0 : 130);
        break;
      }
      case "move":
        if (!ok) break;
        this.showBanner(this.player(seat), body);
        await this.walk(seat, e.from, e.value, fast ? 0 : 90);
        break;
      case "teleport":
        if (!ok || !this.pos) break;
        if (!fast) sfx("teleport");
        this.pos[seat] = e.to;
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
        this.showBanner(e.type === "bankrupt" ? (seat === v!.seat ? tr("anim.youBankrupt") : tr("anim.bankruptOf", { who: this.player(seat) })) : tr("anim.leftOf", { who: this.player(seat) }), body);
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
        this.showBanner(ok ? tr("anim.playedBy", { who: this.name(seat) }) : tr("board.play"), body);
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
      case "status": this.showBanner(ok ? this.player(seat) : tr("anim.status"), body); await wait(500); break;
      case "skill": this.showBanner(ok ? tr("anim.skillOf", { who: this.player(seat) }) : tr("anim.skill"), body); if (!fast) sfx("place"); await wait(500); break;
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
    const off = sess.subscribe(
      (v) => {
        viewRef.current = v;
        set({ view: v, at: performance.now() });
      },
      (e) => a.push(e),
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
