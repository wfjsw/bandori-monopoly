// Effect text as structured blocks: lead paragraphs, section labels, numbered
// effects. Skill bodies and card effect text share it.
//
// Cards and events bring their structure as data (`Block[]` from the display
// export, see ../core/body.ts) and skip parsing. Persona / group skill text is
// still a flat rulebook string, so it goes through `./bodyText.ts`.

import { memo, type ReactNode } from "react";
import { cx } from "../core/cx";
import type { Block } from "../core/body";
import { parse } from "./bodyText";
import s from "./SkillBody.module.css";

export const SkillBody = memo(function SkillBody({
  text,
  blocks,
  prefix,
  className,
}: {
  /** Flat rulebook text; parsed only when `blocks` is absent. */
  text?: string;
  /** Pre-structured sections / bulletpoints (cards, events). Preferred. */
  blocks?: Block[];
  prefix?: ReactNode;
  className?: string;
}) {
  const parsed = blocks ?? parse(text ?? "");
  // The skill name rides with the first paragraph; when the body opens straight
  // onto a list it gets a line of its own above it.
  let head = prefix != null;
  return (
    <div className={cx(s.body, className)}>
      {parsed.map((b, i) => {
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
        if (b.t === "steps") {
          return (
            <div key={i}>
              {before}
              <ol className={s.sub}>
                {b.items.map((x, k) => (
                  <li key={k}>{x}</li>
                ))}
              </ol>
            </div>
          );
        }
        return (
          <div key={i}>
            {before}
            <ol className={s.list}>
              {b.items.map((it, j) => (
                <li key={j}>
                  {it.text}
                  {it.sub.length > 0 && (
                    <ol className={s.sub}>
                      {it.sub.map((x, k) => (
                        <li key={k}>{x}</li>
                      ))}
                    </ol>
                  )}
                </li>
              ))}
            </ol>
          </div>
        );
      })}
    </div>
  );
});