// Event playback (BoardDemoController.PlayEvents): events are queued and
// played one by one -- dice roll, token walk, banners, card reveals -- like
// the original. While the queue runs, tokens follow the animation instead of
// the state; when it drains everything reconciles with the state.

import { useEffect, useReducer, useRef, useState } from "react";
import { sfx } from "../../core/audio";
import { namesOf, turnNamesOf } from "../../core/names";
import { D, cardTitle } from "../../core/data";
import { fmtMsg, fmtMsgParts, partsText, type LogPart, type Names, type Msg } from "../../i18n/msg";
import type { MatchEvent, MatchView } from "../../core/types";
import { showEffect } from "./Popups";

/** How long the dice face rolls before it lands. */
const DICE_ROLL_MS = 10 * 55;
/** How long the effect popup stays up: just longer than the dice roll it
 *  follows, but never under 1.5s -- long enough to read which branch landed. */
const EFFECT_HOLD_MS = Math.max(DICE_ROLL_MS + 300, 1500);
/** How long a card activation's face stays up, fade-out included. */
const CARD_FLASH_MS = 1200;
/** A deep backlog of activations still shows each card, just briefly. */
const CARD_FLASH_FAST_MS = 350;
import type { GameSession } from "../../game/session";
import { t as tr } from "../../i18n/t";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** A deep queue fast-forwards decorative events; walking keeps its path.
 *  Explicit 4x replay fast-forwards both. */
const FAST_QUEUE = 8;

const EVENT_SFX: Record<string, string> = {
  gain: "coin_gain", pass: "bonus", lose: "coin_pay", draw: "draw", mulligan: "draw", discard: "card_play",
};

/** The caption key of a `card` activation's trigger kind. An already-in-play
 *  card's effect (`hook`) is captioned 「<卡名> 的效果」 instead. */
const KIND_KEY: Record<string, string> = {
  play: "board.cardKind.play",
  skill: "board.cardKind.skill",
  event: "board.cardKind.event",
  counter: "board.cardKind.counter",
};

export interface LogLine {
  id: number; text: string; turn: boolean; stage?: boolean;
  /** On a turn heading: the seat the group belongs to (for its colour bar). */
  who?: number;
  /** The line split around its card references, so the log can render them
   *  hoverable. Absent on plain lines. */
  parts?: LogPart[];
}

/** A turn-stage change, queued behind the events that led to it. `step` is the
 *  engine's stage number (1..4 = 开始 / 运营 / 移动 / 结束). */
interface StageMark { step: number; newTurn: boolean; label?: string }

type Item = { ev: MatchEvent } | { mark: StageMark };

/** How long the new-turn announcement stays on screen. */
const TURN_MS = 1500;

/** The log label key of an engine stage number. */
function stageKey(step: number): string {
  return step >= 4 ? "board.stepEnd" : step === 3 ? "board.stepMove" : step === 2 ? "board.stepOps" : "common.start";
}

export class Animator {
  private queue: Item[] = [];
  animating = false;
  /** The stage reached by the animation queue. The engine can roll, walk and
   *  reach 结束 in one update; logging the stage here keeps it after the events
   *  that led to it. Null means the queue has caught up with the state. */
  stage: number | null = null;
  /** Token positions while animating (null: use the state). */
  pos: number[] | null = null;
  dice = 0;
  rolling = false;
  banner: { title: string; body: string; id: number } | null = null;
  /** Only the new-turn announcement appears over the board. */
  turnAnnouncement: { label: string; id: number } | null = null;
  /** The card activation flash: the face shown prominently over the board,
   *  with the owner's colour and a short caption (who/what triggered it).
   *  `detail` holds the activation's effect lines (「<卡名> 的效果：…」) that
   *  ride the flash instead of popping their own modal. */
  flash: {
    card: string; owner: number; caption: string; negated: boolean;
    out: boolean; id: number;
    detail: string[];
    /** The activation's trigger kind (`card_trigger`); `hook` and kin caption
     *  generically, and their detail lines already name the card. */
    kind: string;
    /** The `"card"` event id this flash is, so body lines can ride it. */
    actId: number;
  } | null = null;
  hop: { playerId: number; id: number } | null = null;
  lastDiscard = "";
  log: LogLine[] = [];
  /** Activation event id -> indent depth, for nesting a child activation
   *  (and its children) under the parent that caused it. */
  private actDepth = new Map<number, number>();
  /** Animation speed scales every sleep and timer. A 4x replay additionally
   *  takes the instant path; live matches still show each movement step. */
  speed = 1;
  private seq = 0;
  private localLogId = 0;
  private bannerTimer = 0;
  private turnTimer = 0;
  private flashTimer = 0;
  private disposed = false;

  constructor(private bump: () => void, private view: () => MatchView | null, private readonly replay = false) {}

  /** `sleep` scaled by [`speed`]. */
  private nap(ms: number): Promise<void> {
    return sleep(ms / Math.max(1, this.speed)) as Promise<void>;
  }

  /** Above 2x (live speed buttons or the replay transport) card flashes and
   *  effect popups are skipped: the log still records them, but the board
   *  does not stop to show them. */
  private noFlash(): boolean {
    return this.speed > 2;
  }

  /** The queue plays fast when it is deep, or when a replay is at 4x. */
  private fast(): boolean {
    return this.queue.length > FAST_QUEUE || (this.replay && this.speed >= 4);
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
    clearTimeout(this.turnTimer);
    clearTimeout(this.flashTimer);
  }

  addLog(e: MatchEvent): void {
    const names = e.type === "turn" ? turnNamesOf(this.view()?.state) : this.names();
    const parts = fmtMsgParts(e.msg, names);
    const line = partsText(parts) || fmtMsg(e.msg, names);
    // Group under the 「效果适用」 header: children indent one level past
    // their parent; a nested activation (a `"card"` event with a parent) and
    // its children nest one level deeper still. The child events stay in the
    // stream in code order for animations; the indent is presentational.
    const parent = e.parent ?? -1;
    let depth = 0;
    if (parent >= 0) depth = (this.actDepth.get(parent) ?? 0) + 1;
    if (e.type === "card") this.actDepth.set(e.id, depth);
    const indent = "  ".repeat(depth);
    const rows: LogLine[] = [];
    if (line) {
      rows.push({
        id: e.id, text: indent + line, turn: e.type === "turn",
        who: e.type === "turn" ? e.playerId : undefined,
        parts: parts.length ? parts : undefined,
      });
    }
    for (const r of e.results ?? []) {
      const rt = fmtMsg(r, names);
      if (rt) rows.push({ id: e.id, text: `${indent}  ${rt}`, turn: false });
    }
    // A body line that belongs to the activation currently on the flash rides
    // its face as the outcome summary (first two only).
    if (parent >= 0 && e.type !== "card" && this.flash && !this.flash.out
        && this.flash.actId === parent
        && this.flash.detail.length < 2 && line) {
      const f = this.flash;
      this.flash = { ...f, detail: [...f.detail, line], out: false };
      this.armFlash(f.id, this.fast());
    }
    if (!rows.length) return;
    this.log = [...this.log.slice(-199), ...rows];
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
   *  plays, the log holds the stage the animation started in. */
  stageTo(from: number, mark: StageMark): void {
    if (this.stage == null) this.stage = from;
    this.queue.push({ mark });
    void this.run();
  }

  private async enterStage(m: StageMark, fast: boolean): Promise<void> {
    if (!m.newTurn) return this.toStage(m.step);
    // A new turn starts from no roll -- don't leave the previous player's face
    // sitting on the dice.
    this.dice = 0;
    // Announce whose turn it is once. The engine's 开始 → 运营 transition
    // stays in the log and does not add another popup or animation delay.
    this.toStage(1, true);
    if (m.label) {
      this.showTurn(m.label);
      if (!fast) await this.nap(TURN_MS);
    }
    if (m.step > 1) this.toStage(m.step, true);
  }

  /** Record each stage once: the roll opens 移动 itself, so a state that stops
   *  mid-walk (a [反击] window) must not log 移动 a second time. */
  private toStage(step: number, force = false): void {
    const cur = this.stage ?? this.view()?.state.step;
    this.stage = step;
    if (force || cur !== step) {
      this.log = [...this.log.slice(-199), {
        id: --this.localLogId, text: tr("board.logStage", { stage: tr(stageKey(step)) }), turn: false, stage: true,
      }];
      this.bump();
    }
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

  /** The card activation flash: the face up for ~1.2s (briefly when the queue
   *  is deep), barred in the owner's colour with a short caption, and marked
   *  无效 when a counteraction negated it. The queue plays simultaneous
   *  activations one after another, so none is lost -- a new face waits for
   *  the previous one to fade. The hold is a timer, not a queue pause: the
   *  activation's own follow-ups (its roll, its effect lines) land on the face
   *  instead of waiting it out. */
  private async cardFlash(e: MatchEvent, fast: boolean): Promise<void> {
    while (this.flash && !this.disposed) await sleep(30 / Math.max(1, this.speed));
    if (this.disposed) return;
    // A genuine activation (play / [反击] / skill / event) names who did it.
    // An already-in-play card's 「效果适用」 line carries the why; the flash
    // shows that short why (plus a one-line outcome summary when there is
    // one) so the player sees why the card flashed.
    const kind = e.kind ?? "";
    let caption: string;
    if (KIND_KEY[kind]) {
      caption = tr(KIND_KEY[kind], { who: e.playerId >= 0 ? this.player(e.playerId) : tr("board.cardNeutral") });
    } else if (e.msg?.k === "log.effect_applied") {
      const why = e.msg.a?.why as { msg?: Msg } | undefined;
      const whyText = why && typeof why === "object" && "msg" in why ? fmtMsg(why.msg, this.names()) : "";
      caption = whyText || tr("board.cardEffect", { card: cardTitle(e.card) });
    } else {
      caption = tr("board.cardEffect", { card: cardTitle(e.card) });
    }
    if (!fast) sfx(e.negated ? "prompt" : "card_play");
    // A negated activation's `results` name what negated it; a live one's
    // outcome lines ride the face as they arrive (`addLog`).
    const detail = (e.results ?? []).map((r) => fmtMsg(r, this.names())).filter(Boolean);
    this.showFlash(e.card, e.playerId, caption, !!e.negated, kind, detail, fast, e.id);
  }

  /** Put a card face up; `armFlash` starts its hold. */
  private showFlash(card: string, owner: number, caption: string, negated: boolean, kind: string, detail: string[], fast: boolean, actId = -1): void {
    const id = ++this.seq;
    this.flash = { card, owner, caption, negated, out: false, id, detail, kind, actId };
    this.armFlash(id, fast);
    this.bump();
  }

  /** Start (or re-arm) the flash's hold, then its fade. One timer slot: an
   *  effect line that lands on the face re-arms it before the fade eats it. */
  private armFlash(id: number, fast: boolean): void {
    const hold = (fast ? CARD_FLASH_FAST_MS : CARD_FLASH_MS) / Math.max(1, this.speed);
    const fade = (fast ? 0 : 250) / Math.max(1, this.speed);
    clearTimeout(this.flashTimer);
    this.flashTimer = window.setTimeout(() => {
      if (this.flash?.id !== id) return;
      if (fade <= 0) {
        this.flash = null;
        this.bump();
        return;
      }
      this.flash = { ...this.flash, out: true };
      this.bump();
      this.flashTimer = window.setTimeout(() => {
        if (this.flash?.id !== id || !this.flash.out) return;
        this.flash = null;
        this.bump();
      }, fade);
    }, Math.max(0, hold - fade));
  }

  /** An `effect` line whose source is a card: it rides that card's flash (the
   *  same 「<卡名> 的效果：…」 wording the log keeps) instead of popping the
   *  effect modal -- 正论暴击 and every other `ctx::effect` land here. With no
   *  flash up the card's face comes back, line attached. */
  private async rideEffect(card: string, owner: number, text: string, fast: boolean): Promise<void> {
    const f = this.flash;
    if (f && f.card === card && !f.out) {
      // A hook's generic 「<卡名> 的效果」 caption repeats what the line says;
      // drop it and let the line speak. A named trigger keeps its caption.
      const caption = KIND_KEY[f.kind] ? f.caption : "";
      this.flash = { ...f, caption, detail: [...f.detail, text], out: false };
      this.armFlash(f.id, fast);
      this.bump();
      return;
    }
    while (this.flash && !this.disposed) await sleep(30 / Math.max(1, this.speed));
    if (this.disposed) return;
    this.showFlash(card, owner, "", false, "", [text], fast);
  }

  private showTurn(label: string): void {
    this.turnAnnouncement = { label, id: ++this.seq };
    clearTimeout(this.turnTimer);
    this.turnTimer = window.setTimeout(() => {
      this.turnAnnouncement = null;
      this.bump();
    }, TURN_MS / Math.max(1, this.speed));
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
    return playerId === v?.playerId ? tr("common.you") : this.player(playerId);
  }

  private player(playerId: number): string {
    return this.names().playerId(playerId);
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
        // The queued new-turn marker announces whose turn it is once.
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
        // A burst of reward/card events may fast-forward the log, but must
        // still show the path. Only explicit 4x replay skips movement frames.
        await this.walk(playerId, e.from, e.value, this.replay && this.speed >= 4 ? 0 : 130);
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
        // message already produced. An effect with a source card rides that
        // card's flash (正论暴击 and kin: no modal of their own); only an
        // unattributed one keeps the popup window.
        if (this.noFlash()) break;
        if (e.card) {
          await this.rideEffect(e.card, playerId, body, fast);
          await wait(fast ? 0 : 250);
          break;
        }
        if (!fast) sfx("place");
        showEffect(body, EFFECT_HOLD_MS);
        await wait(EFFECT_HOLD_MS);
        break;
      }
      case "move":
        if (!ok) break;
        this.showBanner(this.player(playerId), body);
        await this.walk(playerId, e.from, e.value, this.replay && this.speed >= 4 ? 0 : 90);
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
        // The card leaves the hand. Its face comes up with the `card`
        // activation event that follows (one flash per play, not two).
        if (!e.card) break;
        this.lastDiscard = e.card;
        this.showBanner(ok ? tr("anim.playedBy", { who: this.name(playerId) }) : tr("board.play"), body);
        if (!fast) sfx("card_play");
        await wait(450);
        break;
      case "card":
        // A card's effect activated -- or a counteraction negated it. This is
        // the one card flash: hand plays, skill presses, event draws, [反击]
        // bodies and field-card hooks all land here.
        if (!e.card) break;
        this.lastDiscard = e.card;
        if (this.noFlash()) break;
        await this.cardFlash(e, fast);
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
    const a = new Animator(bump, () => viewRef.current, sess.kind === "replay");
    for (const e of sess.view?.state.events ?? []) a.addLog(e);
    animRef.current = a;
  }
  useEffect(() => {
    const a = animRef.current!;
    // Both the hand's live controls and replay transport drive local speed.
    const syncSpeed = () => {
      a.speed = sess.animSpeed;
    };
    syncSpeed();
    const off = sess.subscribe(
      (v) => {
        const prev = viewRef.current?.state;
        viewRef.current = v;
        set({ view: v, at: performance.now() });
        // Queue stage changes for the match log after their events. Only a
        // new turn gets an announcement over the board.
        const now = v.state;
        if (prev && now.phase === "play" && (prev.turn !== now.turn || prev.step !== now.step)) {
          const newTurn = prev.turn !== now.turn;
          const who = namesOf(now).playerId(now.turn);
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
