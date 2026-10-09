// Rendering engine messages: the wire `Msg` (see crates/game-core/src/msg.rs) is a
// key plus typed arguments; this turns one into display text in the active language.
// Arguments that name game objects (players, tiles, cards, characters, events, bands)
// are resolved against the match data here, so the Rust side never carries names.

import i18n from "./index";

export type MsgArg =
  | { playerId: number }
  | { tile: number }
  | { card: string }
  | { char: string }
  | { event: string }
  | { band: string }
  | { n: number }
  | { i: number }
  | { text: string }
  | { msg: Msg }
  | { list: MsgArg[] };

export interface Msg {
  k: string;
  a?: Record<string, MsgArg>;
}

/** How a message resolves game-object arguments to display names. */
export interface Names {
  playerId(playerId: number): string;
  tile(tile: number): string;
  card(id: string): string;
  chara(name: string): string;
  event(id: string): string;
  band(name: string): string;
}

/** Names straight from the data files; use `namesOf` for a match state. */
export const dataNames: Names = {
  playerId: (n) => `#${n + 1}`,
  tile: (n) => `#${n}`,
  card: (id) => id,
  chara: (name) => name,
  event: (id) => id,
  band: (name) => name,
};

const isMsg = (v: unknown): v is Msg => !!v && typeof v === "object" && typeof (v as Msg).k === "string";

/** A message, or a plain string (older payloads), as display text. */
export function fmtMsg(m: Msg | string | undefined | null, names: Names = dataNames): string {
  if (m === undefined || m === null) return "";
  if (typeof m === "string") return m;
  if (!isMsg(m)) return "";
  const args: Record<string, unknown> = {};
  for (const [name, arg] of Object.entries(m.a ?? {})) args[name] = fmtArg(arg, names);
  // Engine keys have no prefix and live in `game`; card keys name their own
  // namespace ("cards:crate.key"). Unknown keys fall back to the raw key.
  const ns = m.k.includes(":") ? undefined : "game";
  return i18n.t(m.k, { ...args, ns, defaultValue: m.k }) as string;
}

/** One piece of a log line: plain text, or a card reference to hover/click. */
export type LogPart = string | { card: string; text: string };

const CARD_OPEN = String.fromCharCode(0xe000);
const CARD_CLOSE = String.fromCharCode(0xe001);

/** [`fmtMsg`], but `{{card}}` arguments come back as card references instead of
 *  being folded into the string -- the log and the prompt body can then render
 *  them hoverable. Cards named inside a nested `msg` / `list` argument (the
 *  [反击] prompt's `detail`) come back the same way. */
export function fmtMsgParts(m: Msg | string | undefined | null, names: Names = dataNames): LogPart[] {
  if (m === undefined || m === null) return [];
  if (typeof m === "string") return m ? [m] : [];
  if (!isMsg(m)) return [];
  const cards: { card: string; text: string }[] = [];
  const s = fmtMsgMarkers(m, names, cards);
  const parts: LogPart[] = [];
  const re = new RegExp(`${CARD_OPEN}(\\d+)${CARD_CLOSE}`, "g");
  let last = 0;
  for (const match of s.matchAll(re)) {
    const at = match.index ?? 0;
    if (at > last) parts.push(s.slice(last, at));
    const c = cards[Number(match[1])];
    if (c) parts.push(c);
    last = at + match[0].length;
  }
  if (last < s.length) parts.push(s.slice(last));
  return parts;
}

/** Like [`fmtMsg`], but each `card` argument becomes a marker into `cards`. */
function fmtMsgMarkers(m: Msg, names: Names, cards: { card: string; text: string }[]): string {
  const args: Record<string, unknown> = {};
  for (const [name, arg] of Object.entries(m.a ?? {})) args[name] = fmtArgMarkers(arg, names, cards);
  const ns = m.k.includes(":") ? undefined : "game";
  return i18n.t(m.k, { ...args, ns, defaultValue: m.k }) as string;
}

function fmtArgMarkers(arg: MsgArg, names: Names, cards: { card: string; text: string }[]): string {
  if (arg === null || arg === undefined) return "";
  if (typeof arg === "number") return String(arg);
  if (typeof arg === "string") return arg;
  if ("card" in arg) {
    const card = arg.card;
    cards.push({ card, text: names.card(card) });
    return `${CARD_OPEN}${cards.length - 1}${CARD_CLOSE}`;
  }
  if ("msg" in arg) return fmtMsgMarkers(arg.msg, names, cards);
  if ("list" in arg) return arg.list.map((x) => fmtArgMarkers(x, names, cards)).join(listSep());
  return fmtArg(arg, names);
}

/** The plain text of [`fmtMsgParts`] -- what the line says without its markup. */
export function partsText(parts: LogPart[]): string {
  return parts.map((p) => (typeof p === "string" ? p : p.text)).join("");
}

function fmtArg(arg: MsgArg, names: Names): string {
  if (arg === null || arg === undefined) return "";
  if (typeof arg === "number") return String(arg);
  if (typeof arg === "string") return arg;
  if ("playerId" in arg) return names.playerId(arg.playerId);
  if ("tile" in arg) return names.tile(arg.tile);
  if ("card" in arg) return names.card(arg.card);
  if ("char" in arg) return names.chara(arg.char);
  if ("event" in arg) return names.event(arg.event);
  if ("band" in arg) return names.band(arg.band);
  if ("text" in arg) return arg.text;
  if ("n" in arg) return n0(arg.n);
  if ("i" in arg) return String(arg.i);
  if ("msg" in arg) return fmtMsg(arg.msg, names);
  if ("list" in arg) return arg.list.map((x) => fmtArg(x, names)).join(listSep());
  return "";
}

/** The locale's list separator ("、" in zh-CN, ", " in English). */
function listSep(): string {
  return i18n.t("list", { ns: "game", defaultValue: ", " }) as string;
}

/** C# `ToString("N0")` -- thousands separators. */
export function n0(v: number): string {
  return Math.round(v).toLocaleString("en-US");
}

/** Is this a message (rather than a plain string)? */
export const asMsg = (m: unknown): Msg | undefined => (isMsg(m) ? m : undefined);