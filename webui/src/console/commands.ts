import type { Command, MatchView } from "../core/types.ts";

export const COMMANDS = ["help", "clear", "status", "players", "hand", "inspect", "cards", "tiles", "auto", "act", "money", "tp", "give", "draw", "state"] as const;
export interface ConsoleSession {
  kind: "solo" | "online" | "replay";
  id: string;
  view: MatchView | null;
  readOnly: boolean;
  autoMode: string;
  act(cmd: Command): Promise<unknown | null>;
  setAutoMode(mode: "off" | "bot" | "chaos" | "advanced"): void;
}
export interface ConsoleContext {
  session(): ConsoleSession | null;
  cards(): { id: string; name: string; band: string }[];
  tiles(): { index: number; name: string; kind: string }[];
  profile(): unknown;
  engine(): unknown;
  clear(): void;
  t(key: string, params?: Record<string, unknown>): string;
  errorMessage(error: unknown): string;
}

/** Quoted card names and backslash escapes; raw JSON after `act` stays intact. */
export function parseCommand(line: string): { name: string; args: string[] } {
  const input = line.trim();
  const match = /^(\S+)(?:\s+([\s\S]*))?$/.exec(input);
  if (!match) return { name: "", args: [] };
  const name = match[1].toLowerCase();
  if (name === "act") return { name, args: [match[2] ?? ""] };
  const tokens: string[] = [];
  let token = "", quote = "", escaped = false, started = false;
  for (const c of match[2] ?? "") {
    if (escaped) { token += c; escaped = false; }
    else if (c === "\\") { escaped = true; started = true; }
    else if (quote) { if (c === quote) quote = ""; else token += c; }
    else if (c === '"' || c === "'") { quote = c; started = true; }
    else if (/\s/.test(c)) {
      if (started) { tokens.push(token); token = ""; started = false; }
    } else { token += c; started = true; }
  }
  if (quote || escaped) throw new Error("console.unclosedQuote");
  if (started) tokens.push(token);
  return { name, args: tokens };
}

function integer(raw: string | undefined, min: number, max: number): number {
  if (raw === undefined || !/^-?\d+$/.test(raw)) throw new Error("console.integer");
  const n = Number(raw);
  if (!Number.isSafeInteger(n) || n < min || n > max) throw new Error("console.range");
  return n;
}

/** Parse the whole command before dispatch; rejected input never mutates state. */
export async function executeCommand(line: string, ctx: ConsoleContext): Promise<unknown> {
  let parsed: ReturnType<typeof parseCommand>;
  try { parsed = parseCommand(line); } catch (e) { throw new Error(ctx.t((e as Error).message)); }
  const { name, args } = parsed;
  const fail = (key: string, params?: Record<string, unknown>): never => { throw new Error(ctx.t(key, params)); };
  const arity = (min: number, max = min) => {
    if (args.length < min || args.length > max) fail("console.usage", { command: name });
  };
  const num = (raw: string | undefined, min: number, max: number) => {
    try { return integer(raw, min, max); } catch (e) { return fail((e as Error).message, { min, max }); }
  };
  const sess = () => ctx.session() ?? fail("console.noMatch");
  const view = () => sess().view ?? fail("console.noView");
  const seat = (raw?: string) => num(raw ?? String(view().playerId), 0, view().state.players.length - 1);
  const send = async (cmd: Command) => {
    const s = sess();
    if (s.readOnly || s.kind === "replay") fail("console.readOnly");
    if (s.autoMode !== "off") fail("console.autoActive");
    if (cmd.act === "debug" && s.kind !== "solo") fail("console.soloOnly");
    const error = await s.act(cmd);
    if (error) throw new Error(ctx.errorMessage(error));
    return ctx.t("console.ok");
  };
  switch (name) {
    case "": return undefined;
    case "help": arity(0); return ctx.t("console.help");
    case "clear": arity(0); ctx.clear(); return undefined;
    case "status": {
      arity(0);
      const s = ctx.session(), st = s?.view?.state;
      return { session: s?.kind ?? null, id: s?.id ?? null, auto: s?.autoMode ?? null,
        phase: st?.phase, round: st?.round, turn: st?.turn, step: st?.step,
        seq: st?.seq, busy: st?.busy, prompt: st?.prompt.id, cheated: st?.debugOpen };
    }
    case "players": arity(0); return view().state.players.map((p, seat) => ({ seat, name: p.player, character: p.character, money: p.money, pos: p.pos, hand: p.hand, out: p.bankrupt || p.left }));
    case "hand": arity(0); return view().hand.map((id, index) => ({ index, id, name: ctx.cards().find((c) => c.id === id)?.name }));
    case "inspect": {
      arity(0, 2);
      const what = args[0] ?? "state";
      if (what === "player") return view().state.players[seat(args[1])];
      if (args.length > 1) fail("console.usage", { command: name });
      if (what === "state") return view().state;
      if (what === "prompt") return view().state.prompt;
      if (what === "profile") return ctx.profile();
      if (what === "engine") return ctx.engine();
      return fail("console.usage", { command: name });
    }
    case "cards": {
      arity(0, 1);
      const query = (args[0] ?? "").toLowerCase();
      const all = ctx.cards().filter((c) => `${c.id} ${c.name} ${c.band}`.toLowerCase().includes(query));
      return { total: all.length, cards: all.slice(0, 100).map(({ id, name }) => ({ id, name })) };
    }
    case "tiles": arity(0); return ctx.tiles().map(({ index, name, kind }) => ({ index, name, kind }));
    case "auto": {
      arity(1);
      const mode = args[0];
      if (mode !== "off" && mode !== "bot" && mode !== "chaos" && mode !== "advanced") fail("console.usage", { command: name });
      const s = sess();
      if (s.readOnly || s.kind === "replay") fail("console.readOnly");
      s.setAutoMode(mode as "off" | "bot" | "chaos" | "advanced");
      return ctx.t("console.ok");
    }
    case "act": {
      let cmd: Command;
      try { cmd = JSON.parse(args[0]); } catch { return fail("console.invalidJson"); }
      if (!cmd || typeof cmd !== "object" || Array.isArray(cmd) || typeof cmd.act !== "string" || !cmd.act) return fail("console.invalidJson");
      for (const field of ["value", "target", "prompt"] as const) {
        if (cmd[field] !== undefined && (!Number.isInteger(cmd[field]) || cmd[field]! < -2_147_483_648 || cmd[field]! > 2_147_483_647)) fail("console.invalidJson");
      }
      for (const field of ["character", "card", "debug"] as const) {
        if (cmd[field] !== undefined && typeof cmd[field] !== "string") fail("console.invalidJson");
      }
      if (cmd.cards !== undefined && (!Array.isArray(cmd.cards) || cmd.cards.some((c) => typeof c !== "string"))) fail("console.invalidJson");
      return send(cmd);
    }
    case "money": case "tp": case "draw": {
      arity(1, 2);
      const max = name === "money" ? 100_000_000 : name === "tp" ? ctx.tiles().length - 1 : 100;
      return send({ act: "debug", debug: name, value: num(args[0], name === "draw" ? 1 : 0, max), target: seat(args[1]) });
    }
    case "give": {
      arity(1, 3);
      const matches = ctx.cards().filter((c) => c.id === args[0]);
      const cards = matches.length ? matches : ctx.cards().filter((c) => c.name === args[0]);
      if (cards.length !== 1) return fail("console.cardNotFound", { card: args[0] });
      return send({ act: "debug", debug: "give", card: cards[0].id, value: args[1] === undefined ? 1 : num(args[1], 1, 100), target: seat(args[2]) });
    }
    case "state": arity(2, 3); return send({ act: "debug", debug: "state", character: args[0], value: num(args[1], -1_000_000, 1_000_000), target: seat(args[2]) });
    default: return fail("console.unknown", { command: name });
  }
}
