// Effect text as structured blocks: lead paragraphs, section labels, numbered
// effects. Skill bodies and card effect text share it.
//
// Cards and events bring their structure as data (`Block[]` from the display
// export, see ../core/body.ts) and skip parsing. Persona / group skill text is
// still a flat rulebook string, so it goes through `./bodyText.ts`.
//
// `[手]` / `[反击]` / `[持续]` (and `[特]` / `[限]`) are **subtitles**: a
// `label` block is the section header on its own line, and a tag that *opens*
// a paragraph / effect is the same voice inline at the start of that run.
// The same tags also show up as ordinary prose (「作为[反击]使用」) -- see
// `splitTypeTag`, which only ever looks at the very start of the string.

import { memo, type ReactNode } from "react";
import { cx } from "../core/cx";
import { splitTypeTag, type Block } from "../core/body";
import { parse } from "./bodyText";
import s from "./SkillBody.module.css";

/** `text` with its leading type tag (if any) as a subtitle prefix.
 *  The remainder is passed through verbatim -- no whitespace is invented. */
function TypeTagged({ text }: { text: string }) {
  const { tag, rest } = splitTypeTag(text);
  if (!tag) return text;
  return (
    <>
      <span className={s.tag}>{tag}</span>
      {rest}
    </>
  );
}

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
          // A paragraph that is nothing but the type tag (`[反击]`) is the
          // section header of the blocks under it, not a lead sentence.
          const { tag, rest } = splitTypeTag(b.text);
          if (tag && !rest.trim()) {
            const before = head ? <p className={s.lead}>{prefix}</p> : null;
            head = false;
            return (
              <div key={i}>
                {before}
                <p className={s.label}>{tag}</p>
              </div>
            );
          }
          const lead = head ? (
            <>
              {prefix} <TypeTagged text={b.text} />
            </>
          ) : (
            <TypeTagged text={b.text} />
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
                  <li key={k}>
                    <TypeTagged text={x} />
                  </li>
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
                  <TypeTagged text={it.text} />
                  {it.sub.length > 0 && (
                    <ol className={s.sub}>
                      {it.sub.map((x, k) => (
                        <li key={k}>
                          <TypeTagged text={x} />
                        </li>
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