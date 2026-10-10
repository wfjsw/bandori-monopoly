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

/** Standard's default [反击] declare propensity, in milli (600 = 60 % per
 *  offered card per offer). Mirrors
 *  `game_core::strategy::DEFAULT_COUNTERACT_PROPENSITY_MILLI` -- user ruling
 *  2026-10-08, "bots must be able to counteract" (the old default was 0 =
 *  never, which left every [反击] card dead in a bot's hand). */
export const DEFAULT_COUNTERACT_PROPENSITY_MILLI = 600;

// ---------------------------------------------------------------- strategy book
// `bot` reads its thresholds from the seat's resolved **strategy parameters**
// (`docs/BOT.md` §3.8) -- the same struct `rules.strategy_for` returns, one
// source of truth with `engine/ai.rs`. The constants above are their defaults
// (`DEFAULT_STRATEGY`), so an empty strategy book changes nothing. Chaos is
// not parameterised: it keeps `CHAOS_RESERVE` / `CHAOS_COUNTER_CHANCE`.
//
// Field names are snake_case, like `data/strategy_book.json` and
// `game_core::strategy::StrategyParams`.

/** `game_core::strategy::StrategyParams` as the glue serialises it (snake_case). */
export interface StrategyParams {
  buy_reserve: number;
  buy_reserve_mid: number;
  buy_reserve_late: number;
  phase_mid_round: number;
  phase_late_round: number;
  buy_group_weight: number[];
  set_complete_bonus_milli: number;
  max_price_ratio_milli: number;
  build_reserve: number;
  build_group_weight: number[];
  target_houses: number[];
  force_buy_reserve: number;
  auction_worth_lo_milli: number;
  auction_worth_span_milli: number;
  auction_cash_margin: number;
  bid_step: number;
  bid_nudge_steps: number;
  bid_frac_milli: number;
  play_card_chance_milli: number;
  max_plays_per_turn: number;
  play_card_reserve: number;
  cards: Record<string, CardPlayParams>;
  skills: Record<string, SkillParams>;
  counteract_propensity_milli: number;
  counteract: Record<string, CounterParams>;
  mortgage_house_key_milli: number;
  mortgage_price_key_milli: number;
  redeem_reserve: number;
  prior_buy_yes_milli: number;
  prior_buy_no_milli: number;
  prior_build_yes_milli: number;
  prior_build_no_milli: number;
  prior_alt_milli: number;
  prior_play_base_milli: number;
  prior_play_bonus_milli: number;
  prior_bid_neutral_milli: number;
  prior_bid_floor_milli: number;
  prior_bid_ceil_milli: number;
  prior_counter_skip_milli: number;
  prior_counter_declare_milli: number;
}

/** `game_core::strategy::CardPlayParams`. */
export interface CardPlayParams {
  play_weight_milli: number;
  hold_for_counteract: boolean;
  min_round: number;
  max_round: number;
  min_cash_after_est: number | null;
}

/** `game_core::strategy::SkillParams` (default: never press). */
export interface SkillParams {
  play_weight_milli: number;
  min_fires: number;
  min_crystals: number;
  keep_markers: number;
  min_round: number;
  max_round: number;
}

/** `game_core::strategy::CounterParams` -- a listed card's own spec
 *  (`0` = hold it back); an unlisted card takes `counteract_propensity_milli`. */
export interface CounterParams {
  propensity_milli: number;
  by_kind: Record<string, number>;
}

/**
 * Every field's default = today's constant. Mirrors
 * `game_core::strategy::StrategyParams::default()`; the glue is the authority
 * in a running match (`rules.strategy_for`), this table is the offline /
 * test fallback.
 */
export const DEFAULT_STRATEGY: StrategyParams = {
  buy_reserve: BUY_RESERVE,
  buy_reserve_mid: BUY_RESERVE,
  buy_reserve_late: BUY_RESERVE,
  phase_mid_round: 20,
  phase_late_round: 40,
  buy_group_weight: [],
  set_complete_bonus_milli: 0,
  max_price_ratio_milli: 0,
  build_reserve: BUILD_RESERVE,
  build_group_weight: [],
  target_houses: [],
  force_buy_reserve: FORCE_BUY_RESERVE,
  auction_worth_lo_milli: 600,
  auction_worth_span_milli: 700,
  auction_cash_margin: 1000,
  bid_step: 100,
  bid_nudge_steps: 3,
  bid_frac_milli: 750,
  play_card_chance_milli: 700,
  max_plays_per_turn: MAX_PLAYS_PER_TURN,
  play_card_reserve: BUY_RESERVE,
  cards: {},
  skills: {},
  counteract_propensity_milli: DEFAULT_COUNTERACT_PROPENSITY_MILLI,
  counteract: {},
  mortgage_house_key_milli: 1000,
  mortgage_price_key_milli: 1000,
  redeem_reserve: REDEEM_RESERVE,
  prior_buy_yes_milli: 800,
  prior_buy_no_milli: 200,
  prior_build_yes_milli: 800,
  prior_build_no_milli: 200,
  prior_alt_milli: 300,
  prior_play_base_milli: 400,
  prior_play_bonus_milli: 300,
  prior_bid_neutral_milli: 400,
  prior_bid_floor_milli: 150,
  prior_bid_ceil_milli: 900,
  prior_counter_skip_milli: 700,
  prior_counter_declare_milli: 500,
};

/** `StrategyParams::buy_reserve_at` -- the phase split (all equal by default). */
export function buyReserveAt(p: StrategyParams, round: number): number {
  if (round >= p.phase_late_round) return p.buy_reserve_late;
  if (round >= p.phase_mid_round) return p.buy_reserve_mid;
  return p.buy_reserve;
}

/** `StrategyParams::wants_buy_tile` (no set-completion / ratio extras here). */
export function wantsBuyP(p: StrategyParams, money: number, price: number, round = 0): boolean {
  // Bots do not mortgage to buy land (user ruling 2026-10-08): require **cash
  // alone** to cover the price. `can_buy_here` is funded by cash +
  // mortgageable deeds -- legality for humans is unchanged; the autopilot
  // declines instead of auto-mortgaging.
  return money >= price && money - price >= buyReserveAt(p, round);
}

/** `StrategyParams::wants_build`. */
export function wantsBuildP(p: StrategyParams, money: number, cost: number): boolean {
  return money - cost >= p.build_reserve;
}

/** `StrategyParams::wants_redeem`. */
export function wantsRedeemP(p: StrategyParams, money: number, cost: number): boolean {
  return money - cost >= p.redeem_reserve;
}

/** `StrategyParams::play_card_reserve_for`. */
export function playCardReserveFor(p: StrategyParams, card: string): number {
  return p.cards?.[card]?.min_cash_after_est ?? p.play_card_reserve;
}

/**
 * `advanced` (进阶, `docs/BOT.md` B6): the same search the server's
 * `bot-service` runs, in a Web Worker pool -- ISMCTS over determinizations of
 * the public view. Lazy-loaded; falls back to `bot` on any failure. Like
 * `bot` / `chaos`, it never uses hidden information and answers through the
 * ordinary `act` path.
 */
export type AutoMode = "off" | "bot" | "chaos" | "advanced";
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
  /**
   * Strategy book lookup (`rules.strategy_for`, `docs/BOT.md` §3.8): the
   * seat's resolved parameters for this public table, else
   * [`DEFAULT_STRATEGY`] (today's constants). `seat` is own seat,
   * `opponents` the other seats' characters in seat order. One source of
   * truth with `engine/ai.rs` -- the policy below never hard-codes a
   * threshold. Pure -- no RNG.
   *
   * Optional: a context that has no glue in hand (tests, offline) falls back
   * to [`DEFAULT_STRATEGY`], which is exactly today's constants.
   */
  strategyFor?: (character: string, seat: number, opponents: string[]) => StrategyParams;
  /** Client RNG. Not the engine's -- a takeover does not need to be deterministic. */
  random: () => number;
}

/** The seat's resolved `StrategyParams` for this table (`ctx.strategyFor`). */
export function botParams(view: MatchView, ctx: AutopilotCtx): StrategyParams {
  const S = view.state;
  const me = view.playerId;
  const f = ctx.strategyFor;
  if (!f) return DEFAULT_STRATEGY;
  const opponents = S.players.filter((_, j) => j !== me).map((p) => p.character);
  try {
    const p = f(S.players[me]?.character ?? "", me, opponents);
    return p && typeof p === "object" ? { ...DEFAULT_STRATEGY, ...p } : DEFAULT_STRATEGY;
  } catch {
    return DEFAULT_STRATEGY;
  }
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
function agentChoice(
  S: MatchState,
  tiles: TileData[],
  me: number,
  options: number[],
  prices: number[] | undefined,
  p: StrategyParams,
): number {
  const money = S.players[me].money;
  for (let k = 0; k < options.length; k++) {
    const t = options[k];
    const quoted = prices?.[k];
    const price = quoted != null && quoted >= 0 ? quoted : buyPrice(S, tiles, t);
    if (S.owners[t] < 0 && wantsBuyP(p, money, price, S.round)) return k;
  }
  for (let k = 0; k < options.length; k++) {
    const t = options[k];
    if (S.owners[t] === me && wantsBuildP(p, money, buildCost(tiles, t))) return k;
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

/** The card id a prompt option declares (`Arg::Card` on the option label). */
function optionCard(m: { k?: string; a?: Record<string, unknown> } | undefined): string {
  const v = m?.a?.["card"];
  return typeof v === "string" ? v : "";
}

/** `bot` prompt answer: the engine's `aiAnswer`, else the policy table. */
function botPrompt(view: MatchView, ctx: AutopilotCtx, p: MatchPrompt): Command | null {
  const me = view.playerId;
  const k = p.players.indexOf(me);
  if (!(p.id > 0) || k < 0 || (p.answers[k] ?? -1) >= 0) return null;
  const tiles = ctx.tiles;
  const money = view.state.players[me]?.money ?? 0;
  const ai = view.aiAnswer ?? null;
  const params = botParams(view, ctx);

  if (ai) {
    if (p.kind === "mortgage" || p.kind === "pick") return { act: "answer", prompt: p.id, cards: ai.picked };
    if (p.kind === "auction") return auctionCommand(view, p, ai.worth, ctx, "bot", params);
    return { act: "answer", prompt: p.id, value: clampAnswer(p, ai.answer) };
  }

  switch (p.kind) {
    case "auction":
      return auctionCommand(view, p, auctionWorth(view, ctx, params), ctx, "bot", params);
    case "mortgage":
      return { act: "answer", prompt: p.id, cards: autoMortgage(view.state, tiles, p.items, p.bid) };
    case "pick":
      return { act: "answer", prompt: p.id, cards: p.items.slice(0, Math.max(0, p.count)) };
    case "tile": {
      const t = p.items.map(Number);
      const pickIdx =
        (p.title.k ?? "") === "ask.agent.title" ? agentChoice(view.state, tiles, me, t, p.prices, params) : p.fallback;
      return { act: "answer", prompt: p.id, value: clampAnswer(p, pickIdx) };
    }
    default: {
      switch (offerKind(p)) {
        case "buy": {
          const price = p.price >= 0 ? p.price : buyPrice(view.state, tiles, p.tile);
          return { act: "answer", prompt: p.id, value: wantsBuyP(params, money, price, view.state.round) ? 0 : 1 };
        }
        case "build":
          return {
            act: "answer",
            prompt: p.id,
            value: wantsBuildP(params, money, buildCost(tiles, p.tile)) ? 0 : 1,
          };
        case "force_buy": {
          const price = p.price >= 0 ? p.price : 2 * buyPrice(view.state, tiles, p.tile);
          return { act: "answer", prompt: p.id, value: money - price < params.force_buy_reserve ? 1 : 0 };
        }
        default: {
          // [反击] offer: declare on the seat's per-card propensity
          // (`docs/BOT.md` §3.8 "counteraction"). An unlisted card takes the
          // base rate `counteract_propensity_milli` (default 600‰ -- user
          // ruling 2026-10-08, "bots must be able to counteract"); a listed
          // entry is the card's own spec (`propensity_milli` 0 = hold it back).
          if (p.title?.k === "ask.counteract.title") {
            for (let i = 0; i < p.options.length; i++) {
              if (i === p.fallback) continue;
              const id = optionCard(p.options[i] as { a?: Record<string, unknown> });
              if (!id) continue;
              const entry = params.counteract?.[id];
              const propensity = entry
                ? (entry.propensity_milli ?? 0)
                : (params.counteract_propensity_milli ?? DEFAULT_COUNTERACT_PROPENSITY_MILLI);
              if (propensity > 0 && ctx.random() * 1000 < propensity) {
                return { act: "answer", prompt: p.id, value: clampAnswer(p, i) };
              }
            }
            return { act: "answer", prompt: p.id, value: clampAnswer(p, p.fallback) };
          }
          // mulligan (keep), circle (money), card-rule prompts: the engine's
          // AI answer *is* the fallback.
          return { act: "answer", prompt: p.id, value: clampAnswer(p, p.fallback) };
        }
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
      return auctionCommand(view, p, money - CHAOS_RESERVE, ctx, "chaos", DEFAULT_STRATEGY);
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

/** `worth = ((base × U(lo, lo+span)) / 100 | 0) × 100`, capped at money - margin.
 *  The lo / span / margin are the seat's `StrategyParams` (defaults 0.6 / 0.7 /
 *  1000, the old literals). */
function auctionWorth(view: MatchView, ctx: AutopilotCtx, params: StrategyParams): number {
  const p = view.state.prompt;
  const me = view.playerId;
  const base = buyPrice(view.state, ctx.tiles, p.tile);
  const lo = params.auction_worth_lo_milli / 1000;
  const span = params.auction_worth_span_milli / 1000;
  const v = Math.trunc((base * (lo + ctx.random() * span)) / 100) * 100;
  return Math.min(v, (view.state.players[me]?.money ?? 0) - params.auction_cash_margin);
}

/** Bid inside the ceiling (bot: `min + 0..(nudge-1)·step`; chaos: anywhere up
 *  to the cap), else pass. The raise step / nudge are the seat's
 *  `StrategyParams` for `bot` (defaults `100` / `3`, the old literals). */
function auctionCommand(
  view: MatchView,
  p: MatchPrompt,
  worth: number,
  ctx: AutopilotCtx,
  mode: PolicyName,
  params: StrategyParams,
): Command {
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
      : Math.min(
          cap,
          min +
            params.bid_step * (Math.floor(ctx.random() * params.bid_nudge_steps) % Math.max(1, params.bid_nudge_steps)),
        );
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
 * path. Mirrors `ai.rs` `bot_wants_play_card`, under the seat's
 * `StrategyParams` (the per-card `min_cash_after_est` floor, else
 * `play_card_reserve`).
 */
function affordableCards(view: MatchView, params: StrategyParams | number): string[] {
  const money = view.state.players[view.playerId]?.money ?? 0;
  const ok: string[] = [];
  for (let i = 0; i < view.hand.length; i++) {
    if (view.playable && !view.playable[i]) continue;
    const card = view.hand[i];
    const est = view.estCost?.[i] ?? 0;
    // A bare number is chaos's fixed `CHAOS_RESERVE`; the bot passes its
    // per-card `StrategyParams`.
    const reserve = typeof params === "number" ? params : playCardReserveFor(params, card);
    if (est > 0 && money - est < reserve) continue;
    ok.push(card);
  }
  return ok;
}

/**
 * The skill the standard policy would press, or null: a placed, enabled
 * character / band skill whose per-skill `StrategyParams` say press
 * (`docs/BOT.md` §3.8 "skills"). The default entry is "never" -- the old
 * standard policy, which left skills to the player. Mirrors `ai.rs`
 * `ai_skill_choice`; no RNG draw when nothing qualifies.
 */
function skillChoice(view: MatchView, ctx: AutopilotCtx, params: StrategyParams): string | null {
  const S = view.state;
  const round = S.round;
  for (const id of usableSkills(view)) {
    const sk = params.skills?.[id];
    const weight = sk?.play_weight_milli ?? 0;
    if (weight <= 0) continue;
    if (round < (sk?.min_round ?? 0)) continue;
    if (round > (sk?.max_round ?? i32max)) continue;
    if (weight < 1000 && ctx.random() * 1000 >= weight) continue;
    return id;
  }
  return null;
}

const i32max = 2147483647;

/** Enabled character / band skills from the engine's viewer list. */
function usableSkills(view: MatchView): string[] {
  return (view.skills ?? []).filter((a) => a.enabled).map((a) => a.id);
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
      const params = botParams(view, ctx);
      if (S.step === 2) {
        if (myTurn) {
          const r = redeemChoice(S, ctx.tiles, me, params.redeem_reserve);
          if (r !== null) return [{ act: "redeem", value: r }];
          if (S.skipMove) return [{ act: "end" }];
          if (ctx.playedThisTurn < params.max_plays_per_turn && ctx.random() < params.play_card_chance_milli / 1000) {
            const card = pick(affordableCards(view, params), ctx.random);
            if (card) out.push({ act: "play", card });
          }
          // A skill press only when the seat's per-skill entry asks for one
          // (`docs/BOT.md` §3.8 "skills"); the default entry is "never", the
          // old policy. No RNG draw otherwise.
          const sk = skillChoice(view, ctx, params);
          if (sk) out.push({ act: "skill", card: sk });
        }
        // A refused card must still let the turn move: roll (or end) is next.
        if (S.roller === me) out.push({ act: "roll" });
        return out;
      }
      if (S.step !== 4 || !myTurn) return out; // 开始 lifts itself; 移动 is running
      const t = S.landed;
      if (t >= 0 && buyableHere(S, ctx.tiles, me, t) && wantsBuyP(params, mine.money, buyPrice(S, ctx.tiles, t), S.round)) {
        out.push({ act: "buy", value: t });
      }
      if (t >= 0 && canBuildHere(S, ctx.tiles, me, t) && wantsBuildP(params, mine.money, buildCost(ctx.tiles, t))) {
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

  /**
   * `advanced` is the worker-pool search (`docs/BOT.md` B6) for play-phase
   * decisions; setup (ban / pick / deck) and the fallback after a failed
   * search run the standard policy, exactly like the server holds an Advanced
   * seat's setup engine-side (`MatchPlayer::auto_setup`). The search itself
   * lives in `botDrive.ts` / `botPool.ts`; this alias exists so [`plan`] can
   * take `mode` uniformly.
   */
  advanced: {} as Policy,
};

policies.advanced = policies.bot;

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

/** The 托管 policy a non-search fallback uses when `advanced` fails. */
export const FALLBACK_POLICY: PolicyName = "bot";