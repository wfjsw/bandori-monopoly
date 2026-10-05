// Rendering engine messages: the wire `Msg` (see crates/game-core/src/msg.rs) is a
// key plus typed arguments; this turns one into display text in the active language.
// Arguments that name game objects (seats, tiles, cards, characters, events, bands)
// are resolved against the match data here, so the Rust side never carries names.

import i18n from "./index";

export type MsgArg =
  | { seat: number }
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
  seat(seat: number): string;
  tile(tile: number): string;
  card(id: string): string;
  chara(name: string): string;
  event(id: string): string;
  band(name: string): string;
}

/** Names straight from the data files; use `namesOf` for a match state. */
export const dataNames: Names = {
  seat: (n) => `#${n + 1}`,
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

function fmtArg(arg: MsgArg, names: Names): string {
  if (arg === null || arg === undefined) return "";
  if (typeof arg === "number") return String(arg);
  if (typeof arg === "string") return arg;
  if ("seat" in arg) return names.seat(arg.seat);
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