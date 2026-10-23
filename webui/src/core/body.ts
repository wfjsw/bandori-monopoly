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
  sub: string[];
}

export type Block =
  | { t: "p"; text: string }
  | { t: "label"; text: string }
  | { t: "ol"; items: Item[] }
  /** Standalone sub-steps `1. 2. 3.` -- the 「依次进行以下操作：」 lists. */
  | { t: "steps"; items: string[] };