// Effect text as structured blocks: lead paragraphs, section labels, lists.
// Skill bodies and card effect text share it; the parsing (and the shapes it
// recognises) lives in ./bodyText.ts.

import type { ReactNode } from "react";
import { cx } from "../core/cx";
import { parse } from "./bodyText";
import s from "./SkillBody.module.css";

export function SkillBody({ text, prefix, className }: { text: string; prefix?: ReactNode; className?: string }) {
  const blocks = parse(text);
  // The skill name rides with the first paragraph; when the body opens straight
  // onto a list it gets a line of its own above it.
  let head = prefix != null;
  return (
    <div className={cx(s.body, className)}>
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
        if (b.t === "label") {
          return (
            <div key={i}>
              {before}
              <p className={s.label}>{b.text}</p>
            </div>
          );
        }
        const List = b.t === "ul" ? "ul" : "ol";
        return (
          <div key={i}>
            {before}
            <List className={s.list}>
              {b.items.map((it, j) => (
                <li key={j}>
                  {it.text}
                  {it.sub.length > 0 && (
                    <List className={s.sub}>
                      {it.sub.map((x, k) => (
                        <li key={k}>{x}</li>
                      ))}
                    </List>
                  )}
                </li>
              ))}
            </List>
          </div>
        );
      })}
    </div>
  );
}
