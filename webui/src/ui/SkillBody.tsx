// A skill body as structured blocks: a lead paragraph, then a list.
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
// The full text also writes `（2）技能` / `（2）效果` when an effect *names*
// another skill's numbered effect (「使用（2）效果」). Those are cross-references,
// not items, so they are left in the prose -- only a `（N）` that is not
// immediately followed by 技能 or 效果 opens a list item. Prose with neither
// marker falls through to a single <p>, so the call sites stay one component.

import type { ReactNode } from "react";
import s from "./SkillBody.module.css";

interface Bullet {
  text: string;
  sub: string[];
}

type Block =
  | { t: "p"; text: string }
  | { t: "ul"; items: Bullet[] }
  | { t: "ol"; items: string[] };

/** `（1）` or halfwidth `(1)`; the digits are the list's own numbering. */
const MARK = /[（(](\d+)[）)]/g;

function isXref(text: string, at: number): boolean {
  const tail = text.slice(at, at + 2);
  return tail.startsWith("技能") || tail.startsWith("效果");
}

/** `skill_simple`'s `· ` bullets. */
function parseBullets(text: string): Block[] {
  let lead = "";
  const items: Bullet[] = [];
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

/** The full text's `（1）（2）（3）` runs. */
function parseNumbered(text: string): Block[] {
  const marks: { pos: number; end: number; n: number }[] = [];
  for (const m of text.matchAll(MARK)) {
    const at = m.index ?? 0;
    if (isXref(text, at + m[0].length)) continue;
    marks.push({ pos: at, end: at + m[0].length, n: Number(m[1]) });
  }
  // Without a （1） there is no list to open -- the text is prose that happens
  // to name a numbered effect somewhere.
  if (!marks.some((m) => m.n === 1)) return [{ t: "p", text }];

  const out: Block[] = [];
  let items: string[] = [];
  let item = "";
  let inItem = false;
  let lead = "";
  let pos = 0;
  const closeItem = () => {
    if (inItem) {
      items.push(item.trim());
      item = "";
      inItem = false;
    }
  };
  const closeList = () => {
    if (items.length) out.push({ t: "ol", items });
    items = [];
  };
  for (const mk of marks) {
    let gap = text.slice(pos, mk.pos);
    if (inItem && mk.n === 1) {
      // A section restart (「状态1：…」 / 「状态2：…」): the label sits on its own
      // line just before the new （1）, so the gap's last line is the next list's
      // lead rather than the tail of the previous item.
      const cut = gap.lastIndexOf("\n");
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

function parse(text: string): Block[] {
  if (/^\s*·/m.test(text)) return parseBullets(text);
  const numbered = parseNumbered(text);
  return numbered.some((b) => b.t === "ol") ? numbered : [{ t: "p", text }];
}

export function SkillBody({ text, prefix }: { text: string; prefix?: ReactNode }) {
  const blocks = parse(text);
  // The skill name rides with the first paragraph; when the body opens straight
  // onto a list it gets a line of its own above it.
  let head = prefix != null;
  return (
    <div className={s.body}>
      {blocks.map((b, i) => {
        if (b.t === "p") {
          const lead = head ? (
            <>
              {prefix} {b.text}
            </>
          ) : (
            b.text
          );
          head = false;
          return (
            <p key={i} className={s.lead}>
              {lead}
            </p>
          );
        }
        const before = head ? <p className={s.lead}>{prefix}</p> : null;
        head = false;
        return (
          <div key={i}>
            {before}
            {b.t === "ul" ? (
              <ul className={s.list}>
                {b.items.map((it, j) => (
                  <li key={j}>
                    {it.text}
                    {it.sub.length > 0 && (
                      <ul className={s.sub}>
                        {it.sub.map((x, k) => (
                          <li key={k}>{x}</li>
                        ))}
                      </ul>
                    )}
                  </li>
                ))}
              </ul>
            ) : (
              <ol className={s.list}>
                {b.items.map((x, j) => (
                  <li key={j}>{x}</li>
                ))}
              </ol>
            )}
          </div>
        );
      })}
    </div>
  );
}