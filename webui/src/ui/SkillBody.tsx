// A skill body: the lead line plus `skill_simple`'s bullets, as a list.
//
// `skill_simple` writes one line per effect -- `· ` starts a bullet, an indented
// line continues the one above it, an indented `  - ` is a sub-bullet. The full
// rules text is prose with none of those markers, so it falls through to a plain
// paragraph and the call sites stay one component either way.
//
// Bullets become one `<ul>` of `<li>`s rather than a run of text in a `<p>`, so
// each effect reads as its own line and the list keeps its shape when the panel
// is narrow.

import type { ReactNode } from "react";
import s from "./SkillBody.module.css";

interface Item {
  text: string;
  sub: string[];
}

/** Split a skill body into its lead line and its `· ` bullets. */
function parse(text: string): { lead: string; items: Item[] } {
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
  return { lead, items };
}

export function SkillBody({ text, prefix }: { text: string; prefix?: ReactNode }) {
  const { lead, items } = parse(text);
  const head = prefix ? (
    <>
      {prefix}
      {lead ? " " : null}
      {lead}
    </>
  ) : (
    lead
  );
  if (!items.length) return <p className={s.body}>{head || text}</p>;
  return (
    <div className={s.body}>
      {head ? <p className={s.lead}>{head}</p> : null}
      <ul className={s.list}>
        {items.map((it, i) => (
          <li key={i}>
            {it.text}
            {it.sub.length > 0 && (
              <ul className={s.sub}>
                {it.sub.map((x, j) => (
                  <li key={j}>{x}</li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}