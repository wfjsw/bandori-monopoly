// 托管 / 混沌 -- browser-side auto-play. The client computes every choice from
// what it can see (the match view, its own hand, its prompts) and sends it
// through the ordinary `act` path, exactly as a button press would. The
// server's engine is never asked to take the seat: no `players[i].ai` flip, no
// `member_left`.
//
// Two policies share one driver (`GameSession`'s `Autopilot`):
//
//   * `bot`   -- a port of `crates/game-core/src/engine/ai.rs` over public
//     data. The engine's own precomputed answer rides along in
//     `MatchView.aiAnswer` for the viewer's own prompt (never another seat's --
//     an auction ceiling is hidden information); when it is absent the
//     prompt's `fallback` / the table below is used.
//   * `chaos` -- legal but maximally disruptive: play every card, fire every
//     skill, counter on CHAOS_COUNTER_CHANCE of offers, buy/build whenever it keeps a
//     1 000 float (`CHAOS_RESERVE`). Prefers whatever makes more things happen.
//
// Both cover solo and online, never use hidden information (other hands, deck
// order), and return an ordered candidate list so a refused command is marked
// tried and the next one is attempted instead of looping.

import type { Command, MatchPrompt, MatchPlayer, MatchState, MatchView, TileData } from "../core/types";

// ---------------------------------------------------------------- policy knobs
// `bot` mirrors crates/game-core/src/engine/ai.rs. Keep the two in sync.

/** `AiWantsBuy` -- keep at least this much after buying. */
export const BUY_RESERVE = 2000;
/** `AiWantsBuild` -- keep at least this much after building. */
export const BUILD_RESERVE = 3500;
/** `AiRedeemChoice` -- keep at least this much after redeeming. */
export const REDEEM_RESERVE = 4000;
/** `OfferForceBuy` -- force-buy only while it leaves this much. */
export const FORCE_BUY_RESERVE = 4000;
/** Odds `bot` plays a card rather than rolling in 运营. */
export const PLAY_CARD_CHANCE = 0.7;
/** Hand cards `bot` plays in one turn before rolling. */
export const MAX_PLAYS_PER_TURN = 2;
/**
 * `chaos` still keeps a coin reserve so the seat does not die instantly.
 * Every voluntary spend (buy / build / redeem / auction bid / an optional paid
 * prompt choice / a card play whose **estimated execution cost** would dip
 * below the reserve) must leave at least this much. Counteracts stay
 * unrestricted -- answering is not a spend.
 */
export const CHAOS_RESERVE = 1000;

/** `chaos` declares a [反击] on this share of the counteract offers it gets
 *  (and passes on the rest), so a chain does not always escalate. Mirrors the
 *  engine's chaos bots (`ai.rs`). */
export const CHAOS_COUNTER_CHANCE = 0.3;

export type AutoMode = "off" | "bot" | "chaos";
export type PolicyName = Exclude<AutoMode, "off">;

export const wantsBuy = (money: number, price: number) => money - price >= BUY_RESERVE;
export const wantsBuild = (money: number, cost: number) => money - cost >= BUILD_RESERVE;
export const wantsRedeem = (money: number, cost: number) => money - cost >= REDEEM_RESERVE;

/** `BuyPrice` -- the quoted price. Prefers the engine's `S.buyPrice` preview
 * when it covers this tile; falls back to the formula (land + houses). */
export const buyPrice = (S: MatchState, tiles: TileData[], t: number) => {
  if (S.buyPrice >= 0 && S.landed === t && S.owners[t] < 0) return S.buyPrice;
  return tiles[t].price + (S.houses[t] ?? 0) * tiles[t].house;
};
/** `BuildCost` -- one more house. */
export const buildCost = (tiles: TileData[], t: number) => Math.max(0, tiles[t].house);
/** `MortgageValue` -- half the land price. */
export const mortgageValue = (tiles: TileData[], t: number) => Math.floor(tiles[t].price / 2);
/** `RedeemCost` -- 60% of the land price (matches `model.ts`). */
export const redeemCost = (tiles: TileData[], t: number) => Math.round(tiles[t].price * 0.6);

/** `HandLimitOf` / `OverHand` -- the player's own `handLimit` state. */
export function handLimit(me: MatchPlayer): number {
  return me.state?.handLimit?.value || 5;
}

/** `BuyableHere` -- after a main move onto unowned land, in 结束. */
export function buyableHere(S: MatchState, tiles: TileData[], me: number, i: number): boolean {
  const t = tiles[i];
  return (
    !!t &&
    (t.kind === "property" || t.kind === "ring") &&
    S.phase === "play" &&
    S.step === 4 &&
    S.turn === me &&
    !S.busy &&
    !S.bought &&
    S.landed === i &&
    S.players[me]?.pos === i &&
    S.owners[i] < 0
  );
}

/** `CanBuildHere` -- one more house where the turn stands. */
export function canBuildHere(S: MatchState, tiles: TileData[], me: number, i: number): boolean {
  const t = tiles[i];
  return (
    !!t &&
    S.phase === "play" &&
    S.step === 4 &&
    S.turn === me &&
    !S.busy &&
    !S.bought &&
    !S.built &&
    S.landed === i &&
    S.players[me]?.pos === i &&
    S.owners[i] === me &&
    t.kind === "property" &&
    t.rent.length > 1 &&
    !S.mortgaged[i] &&
    (S.houses[i] ?? 0) < t.rent.length - 1
  );
}

// ---------------------------------------------------------------- context

export interface AutopilotCtx {
  tiles: TileData[];
  /** Characters with their own art set (`cnId`), for ban/pick. */
  characters: { name: string; cnId: string }[];
  /** Hand cards the player has already played this turn (client-tracked). */
  playedThisTurn: number;
  /** The character's preset deck (`rules.deck_preset`). */
  deckPreset: (character: string) => string[];
  /** A random legal deck (`rules.deck_pool` + `rules.deck_clean`). */
  deckRandom: (character: string) => string[];
  /**
   * Deck book suggestion (`rules.deck_suggest`): the book's entry for the
   * public table when it has one, else the character's preset
   * (`docs/BOT.md` §3.7). `seat` is own seat, `opponents` the other seats'
   * characters in seat order. Pure -- no RNG.
   */
  deckSuggest: (character: string, seat: number, opponents: string[]) => string[];
  /** Client RNG. Not the engine's -- a takeover does not need to be deterministic. */
  random: () => number;
}

const pick = <T>(xs: T[], random: () => number): T | undefined =>
  xs.length ? xs[Math.floor(random() * xs.length) % xs.length] : undefined;

/** `RandomCharacter` -- prefers characters with their own art set. */
function randomCharacter(S: MatchState, ctx: AutopilotCtx, forPick: boolean): string {
  const taken = new Set(S.players.map((p) => p.character).filter(Boolean));
  const ok = (name: string) => !S.bans.includes(name) && (!forPick || !taken.has(name));
  const all = ctx.characters.map((c) => c.name).filter(ok);
  const withArt = ctx.characters.filter((c) => c.cnId && ok(c.name)).map((c) => c.name);
  return pick(withArt.length ? withArt : all, ctx.random) ?? "";
}

/** `AiRedeemChoice` -- most valuable mortgaged deed that still leaves the reserve. */
function redeemChoice(S: MatchState, tiles: TileData[], me: number, reserve: number): number | null {
  const money = S.players[me].money;
  const deeds: number[] = [];
  for (let t = 0; t < tiles.length; t++) {
    if (S.owners[t] === me && S.mortgaged[t]) deeds.push(t);
  }
  deeds.sort((a, b) => tiles[b].price - tiles[a].price);
  for (const t of deeds) if (money - redeemCost(tiles, t) >= reserve) return t;
  return null;
}

/** Every mortgaged deed `chaos` will redeem (keeps [`CHAOS_RESERVE`]). */
function redeemable(S: MatchState, tiles: TileData[], me: number, reserve: number): number[] {
  const money = S.players[me].money;
  const deeds: number[] = [];
  for (let t = 0; t < tiles.length; t++) {
    if (S.owners[t] === me && S.mortgaged[t] && money - redeemCost(tiles, t) >= reserve) deeds.push(t);
  }
  deeds.sort((a, b) => tiles[a].price - tiles[b].price);
  return deeds;
}

/** `AiAgentChoice` -- first affordable purchase, else first build, else none.
 * `prices` (parallel to `options`) carries the quoted prices from the prompt. */
function agentChoice(S: MatchState, tiles: TileData[], me: number, options: number[], prices?: number[]): number {
  const money = S.players[me].money;
  for (let k = 0; k < options.length; k++) {
    const t = options[k];
    const quoted = prices?.[k];
    const price = quoted != null && quoted >= 0 ? quoted : buyPrice(S, tiles, t);
    if (S.owners[t] < 0 && wantsBuy(money, price)) return k;
  }
  for (let k = 0; k < options.length; k++) {
    const t = options[k];
    if (S.owners[t] === me && wantsBuild(money, buildCost(tiles, t))) return k;
  }
  return options.length; // "none"
}

/** `AutoMortgage` -- bare land first, cheapest first, until `bid` is covered. */
function autoMortgage(S: MatchState, tiles: TileData[], items: string[], need: number): string[] {
  const order = [...items].map(Number).sort((a, b) => {
    const ha = (S.houses[a] ?? 0) > 0 ? 1 : 0;
    const hb = (S.houses[b] ?? 0) > 0 ? 1 : 0;
    return ha - hb || tiles[a].price - tiles[b].price || a - b;
  });
  let got = 0;
  const out: string[] = [];
  for (const t of order) {
    if (got >= need) break;
    out.push(String(t));
    got += mortgageValue(tiles, t);
  }
  return out;
}

/** A random subset of `items` whose mortgage value covers `need`. */
function randomMortgage(tiles: TileData[], items: string[], need: number, random: () => number): string[] {
  const rest = [...items];
  const out: string[] = [];
  let got = 0;
  while (rest.length && (got < need || out.length === 0)) {
    const k = Math.floor(random() * rest.length) % rest.length;
    const id = rest.splice(k, 1)[0];
    out.push(id);
    got += mortgageValue(tiles, Number(id));
  }
  return out;
}

function clampAnswer(p: MatchPrompt, v: number): number {
  const max = p.kind === "tile" ? p.items.length : Math.max(0, p.options.length - 1);
  return Math.min(Math.max(0, v), Math.max(0, max));
}

/** Is this prompt one of the engine's yes/no offers with a known policy? */
function offerKind(p: MatchPrompt): "buy" | "build" | "force_buy" | "" {
  const title = p.title.k ?? "";
  if (title === "ask.buy.title") return "buy";
  if (title === "ask.build.title") return "build";
  if (title === "ask.force_buy.title") return "force_buy";
  return "";
}

// ---------------------------------------------------------------- prompts

/** `bot` prompt answer: the engine's `aiAnswer`, else the policy table. */
function botPrompt(view: MatchView, ctx: AutopilotCtx, p: MatchPrompt): Command | null {
  const me = view.playerId;
  const k = p.players.indexOf(me);
  if (!(p.id > 0) || k < 0 || (p.answers[k] ?? -1) >= 0) return null;
  const tiles = ctx.tiles;
  const money = view.state.players[me]?.money ?? 0;
  const ai = view.aiAnswer ?? null;

  if (ai) {
    if (p.kind === "mortgage" || p.kind === "pick") return { act: "answer", prompt: p.id, cards: ai.picked };
    if (p.kind === "auction") return auctionCommand(view, p, ai.worth, ctx, "bot");
    return { act: "answer", prompt: p.id, value: clampAnswer(p, ai.answer) };
  }

  switch (p.kind) {
    case "auction":
      return auctionCommand(view, p, auctionWorth(view, ctx), ctx, "bot");
    case "mortgage":
      return { act: "answer", prompt: p.id, cards: autoMortgage(view.state, tiles, p.items, p.bid) };
    case "pick":
      return { act: "answer", prompt: p.id, cards: p.items.slice(0, Math.max(0, p.count)) };
    case "tile": {
      const t = p.items.map(Number);
      const pickIdx = (p.title.k ?? "") === "ask.agent.title" ? agentChoice(view.state, tiles, me, t, p.prices) : p.fallback;
      return { act: "answer", prompt: p.id, value: clampAnswer(p, pickIdx) };
    }
    default: {
      switch (offerKind(p)) {
        case "buy": {
          const price = p.price >= 0 ? p.price : buyPrice(view.state, tiles, p.tile);
          return { act: "answer", prompt: p.id, value: wantsBuy(money, price) ? 0 : 1 };
        }
        case "build":
          return { act: "answer", prompt: p.id, value: wantsBuild(money, buildCost(tiles, p.tile)) ? 0 : 1 };
        case "force_buy": {
          const price = p.price >= 0 ? p.price : 2 * buyPrice(view.state, tiles, p.tile);
          return { act: "answer", prompt: p.id, value: money - price < FORCE_BUY_RESERVE ? 1 : 0 };
        }
        default:
          // mulligan (keep), circle (money), counteract (skip), card-rule
          // prompts: the engine's AI answer *is* the fallback.
          return { act: "answer", prompt: p.id, value: clampAnswer(p, p.fallback) };
      }
    }
  }
}

/**
 * `chaos` prompt answer: accept every offer that keeps [`CHAOS_RESERVE`], take
 * a random target, pick uniformly among the non-default options. A counteract
 * offer declares (a random card) only on [`CHAOS_COUNTER_CHANCE`], else skips.
 * The fallback only when it is the sole option.
 */
function chaosPrompt(view: MatchView, ctx: AutopilotCtx, p: MatchPrompt): Command | null {
  const me = view.playerId;
  const k = p.players.indexOf(me);
  if (!(p.id > 0) || k < 0 || (p.answers[k] ?? -1) >= 0) return null;
  const tiles = ctx.tiles;
  const money = view.state.players[me]?.money ?? 0;
  const random = ctx.random;

  switch (p.kind) {
    case "auction":
      // Cap the ceiling at money - CHAOS_RESERVE so the seat keeps a float.
      return auctionCommand(view, p, money - CHAOS_RESERVE, ctx, "chaos");
    case "mortgage":
      return { act: "answer", prompt: p.id, cards: randomMortgage(tiles, p.items, p.bid, random) };
    case "pick": {
      const n = Math.max(0, p.count);
      const rest = [...p.items];
      const cards: string[] = [];
      while (rest.length && cards.length < n) cards.push(rest.splice(Math.floor(random() * rest.length) % rest.length, 1)[0]);
      return { act: "answer", prompt: p.id, cards };
    }
    case "tile": {
      // Never "none" while a target exists.
      const n = p.items.length;
      if (!n) return { act: "answer", prompt: p.id, value: 0 };
      return { act: "answer", prompt: p.id, value: Math.floor(random() * n) % n };
    }
    default: {
      switch (offerKind(p)) {
        case "buy": {
          const price = p.price >= 0 ? p.price : buyPrice(view.state, tiles, p.tile);
          return { act: "answer", prompt: p.id, value: money - price >= CHAOS_RESERVE ? 0 : 1 };
        }
        case "build":
          return { act: "answer", prompt: p.id, value: money - buildCost(tiles, p.tile) >= CHAOS_RESERVE ? 0 : 1 };
        case "force_buy": {
          const price = p.price >= 0 ? p.price : 2 * buyPrice(view.state, tiles, p.tile);
          return { act: "answer", prompt: p.id, value: money - price >= CHAOS_RESERVE ? 0 : 1 };
        }
        default: {
          const n = p.options.length;
          // [反击] offer: counter only on CHAOS_COUNTER_CHANCE, else skip
          // (the fallback is the skip option).
          if (p.title?.k === "ask.counteract.title" && n > 1 && random() >= CHAOS_COUNTER_CHANCE) {
            return { act: "answer", prompt: p.id, value: clampAnswer(p, p.fallback) };
          }
          // Uniform among the non-default options; the fallback (the engine's
          // "do nothing") only when it is alone.
          if (n <= 1) return { act: "answer", prompt: p.id, value: clampAnswer(p, p.fallback) };
          const others = Array.from({ length: n }, (_, i) => i).filter((i) => i !== p.fallback);
          const v = pick(others.length ? others : [p.fallback], random) ?? p.fallback;
          return { act: "answer", prompt: p.id, value: clampAnswer(p, v) };
        }
      }
    }
  }
}

/** `worth = ((base × U(0.6, 1.3)) / 100 | 0) × 100`, capped at money - 1000. */
function auctionWorth(view: MatchView, ctx: AutopilotCtx): number {
  const p = view.state.prompt;
  const me = view.playerId;
  const base = buyPrice(view.state, ctx.tiles, p.tile);
  const v = Math.trunc((base * (0.6 + ctx.random() * 0.7)) / 100) * 100;
  return Math.min(v, (view.state.players[me]?.money ?? 0) - 1000);
}

/** Bid inside the ceiling (bot: `min + 0..200`; chaos: anywhere up to money-1), else pass. */
function auctionCommand(view: MatchView, p: MatchPrompt, worth: number, ctx: AutopilotCtx, mode: PolicyName): Command {
  const me = view.playerId;
  const money = view.state.players[me]?.money ?? 0;
  const st = view.state.players[me];
  const canPay = !!st && !st.bankrupt && !st.left && !(st.state?.stun?.value > 0) && !(st.state?.exile?.value > 0);
  const min = p.bid <= 0 ? 100 : p.bid + 100;
  const cap = Math.min(worth, money);
  if (min > cap || !canPay) return { act: "answer", prompt: p.id, value: -1 };
  const bid =
    mode === "chaos"
      ? min + 100 * (Math.floor(ctx.random() * (Math.floor((cap - min) / 100) + 1)) % (Math.floor((cap - min) / 100) + 1))
      : Math.min(cap, min + 100 * (Math.floor(ctx.random() * 3) % 3));
  return { act: "answer", prompt: p.id, value: bid };
}

// ---------------------------------------------------------------- policies

/** What one policy wants to do, in preference order, for one state. */
export interface Policy {
  readonly name: PolicyName;
  prompt(view: MatchView, ctx: AutopilotCtx, p: MatchPrompt): Command | null;
  vote(view: MatchView, ctx: AutopilotCtx): Command | null;
  setup(view: MatchView, ctx: AutopilotCtx): Command | null;
  /** Over the hand limit: what to discard (empty when not over). */
  discard(view: MatchView, ctx: AutopilotCtx): Command[];
  /** 运营 / 结束 actions, best first. */
  turn(view: MatchView, ctx: AutopilotCtx): Command[];
}

/**
 * Cards legal to play **and** affordable against the seat's reserve, using the
 * bot-only estimated execution cost (`view.estCost`, user ruling 2026-10-07).
 * Never legality -- a human may still play the card and take the Q1 shortfall
 * path. Mirrors `ai.rs` `bot_wants_play_card`.
 */
function affordableCards(view: MatchView, reserve: number): string[] {
  const money = view.state.players[view.playerId]?.money ?? 0;
  const ok: string[] = [];
  for (let i = 0; i < view.hand.length; i++) {
    if (view.playable && !view.playable[i]) continue;
    const est = view.estCost?.[i] ?? 0;
    if (est > 0 && money - est < reserve) continue;
    ok.push(view.hand[i]);
  }
  return ok;
}

/** Enabled character / band skills on `me`'s field. */
function usableSkills(view: MatchView): string[] {
  return (view.state.players[view.playerId]?.actions ?? []).filter((a) => a.enabled).map((a) => a.id);
}

export const policies: Record<PolicyName, Policy> = {
  bot: {
    name: "bot",
    prompt: botPrompt,
    vote: () => ({ act: "vote", value: 1 }),
    setup(view, ctx) {
      const S = view.state;
      const me = view.playerId;
      const mine = S.players[me];
      if (S.phase === "ban" || S.phase === "pick") {
        if (S.turn !== me) return null;
        if (S.phase === "ban") {
          if (mine.banDone) return null;
          // `AiStep`'s ban: 50% a random character, else no ban.
          return { act: "ban", character: ctx.random() < 0.5 ? randomCharacter(S, ctx, false) : "" };
        }
        if (mine.character) return null;
        return { act: "pick", character: randomCharacter(S, ctx, true) };
      }
      if (S.phase === "deck") {
        if (mine.deckReady) return null;
        // The deck book's entry for this public table, else the preset
        // (`docs/BOT.md` §3.7). Key is public: own character, own seat, the
        // other seats' characters in seat order.
        const opponents = S.players.filter((_, j) => j !== me).map((p) => p.character);
        const cards = ctx.deckSuggest(mine.character, me, opponents);
        return { act: "deck", cards: cards.length ? cards : ctx.deckPreset(mine.character) };
      }
      return null;
    },
    discard(view, ctx) {
      const k = Math.floor(ctx.random() * view.hand.length) % Math.max(1, view.hand.length);
      const card = view.hand[k];
      return card ? [{ act: "discard", card }] : [];
    },
    turn(view, ctx) {
      const S = view.state;
      const me = view.playerId;
      const mine = S.players[me];
      const myTurn = S.turn === me;
      const out: Command[] = [];
      if (S.step === 2) {
        if (myTurn) {
          const r = redeemChoice(S, ctx.tiles, me, REDEEM_RESERVE);
          if (r !== null) return [{ act: "redeem", value: r }];
          if (S.skipMove) return [{ act: "end" }];
          if (ctx.playedThisTurn < MAX_PLAYS_PER_TURN && ctx.random() < PLAY_CARD_CHANCE) {
            const card = pick(affordableCards(view, BUY_RESERVE), ctx.random);
            if (card) out.push({ act: "play", card });
          }
        }
        // A refused card must still let the turn move: roll (or end) is next.
        if (S.roller === me) out.push({ act: "roll" });
        return out;
      }
      if (S.step !== 4 || !myTurn) return out; // 开始 lifts itself; 移动 is running
      const t = S.landed;
      if (t >= 0 && buyableHere(S, ctx.tiles, me, t) && wantsBuy(mine.money, buyPrice(S, ctx.tiles, t))) {
        out.push({ act: "buy", value: t });
      }
      if (t >= 0 && canBuildHere(S, ctx.tiles, me, t) && wantsBuild(mine.money, buildCost(ctx.tiles, t))) {
        out.push({ act: "build", value: t });
      }
      out.push({ act: "end" });
      return out;
    },
  },

  chaos: {
    name: "chaos",
    prompt: chaosPrompt,
    vote: (_view, ctx) => ({ act: "vote", value: ctx.random() < 0.5 ? 1 : 0 }),
    setup(view, ctx) {
      const S = view.state;
      const me = view.playerId;
      const mine = S.players[me];
      if (S.phase === "ban" || S.phase === "pick") {
        if (S.turn !== me) return null;
        if (S.phase === "ban") {
          if (mine.banDone) return null;
          // Always ban something, if anything is left.
          return { act: "ban", character: randomCharacter(S, ctx, false) };
        }
        if (mine.character) return null;
        return { act: "pick", character: randomCharacter(S, ctx, true) };
      }
      if (S.phase === "deck") {
        if (mine.deckReady) return null;
        // A random legal deck, not the designer's preset.
        const cards = ctx.deckRandom(mine.character);
        return { act: "deck", cards: cards.length ? cards : ctx.deckPreset(mine.character) };
      }
      return null;
    },
    discard(view, ctx) {
      // Any card will do; list them all so a refusal falls through.
      return [...view.hand].sort(() => ctx.random() - 0.5).map((card) => ({ act: "discard", card }) as Command);
    },
    turn(view, ctx) {
      const S = view.state;
      const me = view.playerId;
      const mine = S.players[me];
      const myTurn = S.turn === me;
      const out: Command[] = [];
      if (S.step === 2) {
        if (myTurn) {
          if (S.skipMove && !affordableCards(view, CHAOS_RESERVE).length && !usableSkills(view).length) return [{ act: "end" }];
          // Every playable card first (100%, no cap) -- keep making things happen.
          // The only filter is chaos's own coin reserve against the card's
          // estimated execution cost (`view.estCost`).
          for (const card of affordableCards(view, CHAOS_RESERVE).sort(() => ctx.random() - 0.5)) {
            out.push({ act: "play", card });
          }
          // Then every enabled skill (character + band).
          for (const id of usableSkills(view)) out.push({ act: "skill", card: id });
          // Then redeem whatever keeps the reserve.
          for (const t of redeemable(S, ctx.tiles, me, CHAOS_RESERVE)) out.push({ act: "redeem", value: t });
          if (S.skipMove) out.push({ act: "end" });
        }
        if (S.roller === me) out.push({ act: "roll" });
        return out;
      }
      if (S.step !== 4 || !myTurn) return out;
      // Buy / build whenever the reserve survives, then end only when nothing
      // is left.
      const t = S.landed;
      if (t >= 0 && buyableHere(S, ctx.tiles, me, t) && mine.money - buyPrice(S, ctx.tiles, t) >= CHAOS_RESERVE) {
        out.push({ act: "buy", value: t });
      }
      if (t >= 0 && canBuildHere(S, ctx.tiles, me, t) && mine.money - buildCost(ctx.tiles, t) >= CHAOS_RESERVE) {
        out.push({ act: "build", value: t });
      }
      out.push({ act: "end" });
      return out;
    },
  },
};

// ---------------------------------------------------------------- plan

/**
 * Ordered candidates for this state under `mode` -- best first. The driver
 * sends them in order, skipping anything already refused for this state, so a
 * speculative command that bounces moves on instead of looping.
 */
export function plan(view: MatchView, ctx: AutopilotCtx, mode: PolicyName): Command[] {
  const S = view.state;
  const me = view.playerId;
  const mine = S.players[me];
  if (!mine || mine.bankrupt || mine.left) return [];
  const policy = policies[mode];

  // 1. A prompt waiting on me outranks everything.
  const prompt = policy.prompt(view, ctx, S.prompt);
  if (prompt) return [prompt];

  // 2. A live vote must be answered (a browser seat is not `ai`, so it counts).
  const v = S.vote;
  if (v.id > 0 && v.players.includes(me) && (v.answers[v.players.indexOf(me)] ?? -1) < 0) {
    const vote = policy.vote(view, ctx);
    if (vote) return [vote];
  }

  // 3. Pre-game phases (order just waits for the clock).
  const setup = policy.setup(view, ctx);
  if (setup) return [setup];
  if (S.phase !== "play") return [];

  // 4. The match is busy resolving something -- wait.
  if (S.busy) return [];

  // 5. Over the hand limit: discard first (legal on any turn).
  if (view.hand.length > handLimit(mine)) return policy.discard(view, ctx);

  // 6. Turn input: only on my turn (rolling is the one exception -- `S.roller`
  //    may be someone else rolling for the turn player).
  if (S.turn !== me && S.roller !== me) return [];
  return policy.turn(view, ctx);
}

/** The single best command (what the driver sends first). */
export function suggest(view: MatchView, ctx: AutopilotCtx, mode: PolicyName = "bot"): Command | null {
  return plan(view, ctx, mode)[0] ?? null;
}

/** Guard shared by the UI: is this seat under either auto mode? */
export const isAuto = (mode: AutoMode): mode is PolicyName => mode !== "off";