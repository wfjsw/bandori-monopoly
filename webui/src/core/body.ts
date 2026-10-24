// Structured effect text: sections, lead paragraphs, bulletpoints.
//
// This is the display shape `CardText` authors in Rust and `dump_display`
// exports as `body` on a card / event row (`docs/DISPLAY.md`). `SkillBody`
// renders it directly; the flat `text` string stays alongside for prose
// consumers and for rule-text substring checks.
//
// Persona / group skill text is still a flat string -- `ui/bodyText.parse`
// derives the same blocks from it at render time.

/** One numbered effect, with its `1. / 2.` sub-step lines split out. */
export interface Item {
  text: string;
  /**
   * Nested `1. / 2.` sub-steps. **Optional on the wire**: the display export
   * (`dump_display.rs`) omits the key entirely when an effect has none, so
   * every reader must treat a missing `sub` as `[]` -- see [`itemSubs`].
   * `ui/bodyText.parse` always fills it in.
   */
  sub?: string[];
}

/** The sub-steps of a numbered effect, tolerating an omitted `sub`.
 *
 *  The display export drops empty arrays, so `{ text }` is a complete item.
 *  Reading `it.sub.length` unguarded throws `Cannot read properties of
 *  undefined` and blanks the whole board -- use this instead. */
export function itemSubs(it: Item): string[] {
  return it.sub ?? [];
}

export type Block =
  | { t: "p"; text: string }
  | { t: "label"; text: string }
  | { t: "ol"; items: Item[] }
  /** Standalone sub-steps `1. 2. 3.` -- the 「依次进行以下操作：」 lists. */
  | { t: "steps"; items: string[] };
/** Card-type tags that act as a subtitle when they *open* a text run.
 *
 *  `[手]` / `[反击]` / `[持续]` / `[特]` / `[限]` print as a section header on
 *  their own line (`[手]：`, `[持续]：`) and as a type marker at the start of an
 *  effect (「[反击] 当你被…」). Both are subtitles. The same tags also appear as
 *  ordinary prose -- 「作为[反击]使用」, 「打出[反击]卡」, 「[手]效果」 -- and
 *  those are **not** headers: only a tag at the very start of the run is split,
 *  and it is split out as display text, never rewritten. */
const TYPE_TAG = /^(?:\[(?:手|反击|持续|特|限)\])+/;

/** Split a leading card-type tag off `text`.
 *
 *  `{ tag: "", rest: text }` when the run does not open with one, so a
 *  mid-sentence 「[反击]」 is returned untouched. `tag` is the bracketed run
 *  (`"[反击]"`, `"[反击][持续]"`); `rest` is the remainder **verbatim**
 *  (its leading space / `：` stays with it). */
export function splitTypeTag(text: string): { tag: string; rest: string } {
  const m = TYPE_TAG.exec(text);
  if (!m) return { tag: "", rest: text };
  return { tag: m[0], rest: text.slice(m[0].length) };
}
