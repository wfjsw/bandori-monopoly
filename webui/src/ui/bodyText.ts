// Effect text as structured blocks: lead paragraphs, section labels, lists.
// Rendered by `SkillBody`; shared by skill bodies and card effect text.
//
// Two shapes live behind `skillText`, and each has its own markers:
//
//   `skill_simple`  `火罐：开局 4，最多 4。\n· effect one\n· effect two`
//                   one line per effect, `· ` starts a bullet, an indented
//                   line continues the one above, `  - ` is a sub-bullet.
//                   -> <ul>
//
//   the full text   `（1）effect one（2）effect two（3）effect three`
//                   numbered effects run inline, sometimes restarting at （1）
//                   for a second section. -> <ol>
//
// Card text is the full shape plus two things of its own:
//
//   * section labels on a line of their own (`[特]：`, `[手]：`, `[持续]`), each
//     restarting the numbering under it -- so the text is cut into sections at
//     those lines first, and each section is parsed on its own;
//   * sub-steps inside one effect, a line each (`\n1. …\n2. …`) -> a nested <ol>.
//
// The text also *names* numbered effects: 「使用（2）效果」, 「角色的（2）技能」,
// 「[特]效果（1）」, 「选择（2）或（3）效果之一」. Those are cross-references, not
// items, so they are left in the prose (see `isXref`). Prose with no marker at
// all falls through to a single paragraph, so the call sites stay one component.

export interface Item {
  text: string;
  sub: string[];
}

export type Block =
  | { t: "p"; text: string }
  | { t: "label"; text: string }
  | { t: "ul"; items: Item[] }
  | { t: "ol"; items: Item[] };

/** `（1）` or halfwidth `(1)`; the digits are the list's own numbering. */
const MARK = /[（(](\d+)[）)]/g;

/** A line that is nothing but a section label: `[特]：`, `[持续]`, `[共鸣][反击]：`. */
const LABEL = /^\s*(?:\[[^\]\n]+\])+\s*[：:]?\s*$/;

/** A sub-step line inside one numbered effect: `1. …` (not `1.5倍`). */
const STEP = /^\s*\d+[.．](?!\d)\s*/;

/** Whether the `（N）` ending at `end` refers to an effect instead of opening
 *  one. An item opens onto its content; a reference is followed by what it
 *  names (「（2）技能」「（2）效果」), by a conjunction into the next reference
 *  (「（2）或（3）效果」), or by the punctuation closing the sentence that cites
 *  it (「[特]效果（1）。」). What *precedes* says nothing: 「…失去该技能（2）（抵押
 *  或赎回以外）…」 opens item 2 right after the word 技能. */
function isXref(text: string, end: number): boolean {
  const after = text.slice(end, end + 2);
  return after.startsWith("技能") || after.startsWith("效果") || /^(?:[或和与、。，；,;]|$)/.test(after);
}

/** `skill_simple`'s `· ` bullets. */
function parseBullets(text: string): Block[] {
  let lead = "";
  const items: Item[] = [];
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    const indented = /^\s/.test(raw);
    const body = line.replace(/^[·-]\s*/, "");
    const marked = /^[·-]/.test(line);
    const last = items[items.length - 1];
    if (marked && !indented) items.push({ text: body, sub: [] });
    else if (indented && line.startsWith("-") && last) last.sub.push(body);
    else if (last) {
      // A continuation: join it onto whatever it belongs to, bullet or sub.
      if (last.sub.length) last.sub[last.sub.length - 1] += ` ${body}`;
      else last.text += ` ${body}`;
    } else lead = lead ? `${lead} ${body}` : body;
  }
  const out: Block[] = [];
  if (lead) out.push({ t: "p", text: lead });
  if (items.length) out.push({ t: "ul", items });
  return out;
}

/** One numbered effect, with its `1. / 2.` sub-step lines split out. */
function toItem(raw: string): Item {
  const head: string[] = [];
  const sub: string[] = [];
  for (const line of raw.trim().split("\n")) {
    const step = STEP.exec(line);
    if (step) sub.push(line.slice(step[0].length).trim());
    else if (sub.length) sub[sub.length - 1] += `\n${line.trim()}`;
    else head.push(line);
  }
  return { text: head.join("\n").trim(), sub };
}

/** The full text's `（1）（2）（3）` runs. */
function parseNumbered(text: string): Block[] {
  const marks: { pos: number; end: number; n: number }[] = [];
  let last = 0;
  for (const m of text.matchAll(MARK)) {
    const at = m.index ?? 0;
    const end = at + m[0].length;
    const n = Number(m[1]);
    if (isXref(text, end)) continue;
    // Items count up from （1）, or restart at （1） for a new section; any
    // other number is a citation that happens to be followed by content
    // (「使用自己原有的技能（2）时」 inside item 2).
    if (n !== 1 && n !== last + 1) continue;
    last = n;
    marks.push({ pos: at, end, n });
  }
  // Without a （1） there is no list to open -- the text is prose that happens
  // to name a numbered effect somewhere.
  if (!marks.some((m) => m.n === 1)) return [{ t: "p", text }];

  const out: Block[] = [];
  let items: Item[] = [];
  let item = "";
  let inItem = false;
  let lead = "";
  let pos = 0;
  const closeItem = () => {
    if (inItem) {
      items.push(toItem(item));
      item = "";
      inItem = false;
    }
  };
  const closeList = () => {
    if (items.length) out.push({ t: "ol", items });
    items = [];
  };
  for (const mk of marks) {
    const gap = text.slice(pos, mk.pos);
    if (inItem && mk.n === 1) {
      // A section restart (「状态1：…」 / 「状态2：…」): the label sits on its own
      // line just before the new （1）, so the gap's last line is the next list's
      // lead rather than the tail of the previous item.
      const cut = gap.trimEnd().lastIndexOf("\n");
      if (cut >= 0) {
        item += gap.slice(0, cut);
        closeItem();
        closeList();
        lead = gap.slice(cut + 1).trim();
      } else {
        item += gap;
        closeItem();
        closeList();
      }
    } else if (inItem) {
      item += gap;
      closeItem();
    } else lead += gap;
    // Opening an item: any lead paragraph goes above its list, not after.
    if (!items.length && lead.trim()) {
      out.push({ t: "p", text: lead.trim() });
      lead = "";
    }
    inItem = true;
    pos = mk.end;
  }
  const tail = text.slice(pos);
  if (inItem) {
    item += tail;
    closeItem();
  } else lead += tail;
  closeList();
  if (lead.trim()) out.push({ t: "p", text: lead.trim() });
  return out;
}

/** Cut the text at its label-only lines. The part before the first label has
 *  no label of its own. */
function sections(text: string): { label?: string; body: string }[] {
  const out: { label?: string; body: string }[] = [{ body: "" }];
  for (const line of text.split("\n")) {
    if (LABEL.test(line)) out.push({ label: line.trim(), body: "" });
    else {
      const cur = out[out.length - 1];
      cur.body = cur.body ? `${cur.body}\n${line}` : line;
    }
  }
  return out;
}

export function parse(text: string): Block[] {
  if (/^\s*·/m.test(text)) return parseBullets(text);
  const out: Block[] = [];
  for (const sec of sections(text)) {
    if (sec.label) out.push({ t: "label", text: sec.label });
    const body = sec.body.trim();
    if (!body) continue;
    const numbered = parseNumbered(body);
    out.push(...(numbered.some((b) => b.t === "ol") ? numbered : [{ t: "p" as const, text: body }]));
  }
  return out;
}
