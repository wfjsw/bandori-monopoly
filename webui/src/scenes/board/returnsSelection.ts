// Present Returns' eight setup choices together, then submit the selected
// card ids through the existing prompt protocol (also works with saved picks).
import type { MatchPrompt } from "../../core/types";
import type { Msg } from "../../i18n/msg";

export function optionCard(o: Msg): string | null {
  const args = Object.values(o.a ?? {});
  const v = args.length === 1 ? args[0] : undefined;
  return v && "card" in v ? v.card : null;
}

export function returnsPickNumber(p: MatchPrompt | undefined): number | null {
  if (!p?.id || !p.text.k.endsWith(".returns_add_eight_pick")) return null;
  const arg = p.text.a?.n;
  const n = arg && ("i" in arg ? arg.i : "n" in arg ? arg.n : 0);
  return n && n >= 1 && n <= 8 ? n : null;
}

export function returnsPickCount(p: MatchPrompt): number {
  const n = returnsPickNumber(p);
  return n === null ? 0 : Math.min(9 - n, p.options.filter((o) => optionCard(o) !== null).length);
}

export interface ReturnsSelectionSource {
  current(): MatchPrompt | undefined;
  answer(prompt: number, value: number): Promise<boolean>;
  subscribe(changed: () => void): () => void;
}

function nextPrompt(source: ReturnsSelectionSource, id: number, timeoutMs: number) {
  let off = () => {};
  let timer: ReturnType<typeof setTimeout>;
  let finish: (p: MatchPrompt | undefined) => void;
  let done = false;
  const promise = new Promise<MatchPrompt | undefined>((resolve) => {
    finish = (p) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      off();
      resolve(p);
    };
    timer = setTimeout(() => finish(undefined), timeoutMs);
    off = source.subscribe(() => {
      const p = source.current();
      if (p?.id !== id) finish(p);
    });
    if (done) off(); // a subscription may publish its current view immediately
  });
  return { promise, cancel: () => finish(undefined) };
}

/** Resolve each option from its card id again: every accepted pick shrinks
 *  the pool and shifts its indices. Wait for SSE before sending the next one. */
export async function submitReturnsSelection(
  source: ReturnsSelectionSource,
  cards: string[],
  progress: (n: number) => void = () => {},
  timeoutMs = 10_000,
): Promise<boolean> {
  const first = source.current();
  const start = returnsPickNumber(first);
  if (!first || start === null || cards.length !== returnsPickCount(first)
    || new Set(cards).size !== cards.length
    || cards.some((id) => !first.options.some((o) => optionCard(o) === id))) return false;
  for (let k = 0; k < cards.length; k++) {
    const p = source.current();
    if (!p || returnsPickNumber(p) !== start + k) return false;
    const value = p.options.findIndex((o) => optionCard(o) === cards[k]);
    if (value < 0) return false;
    const next = nextPrompt(source, p.id, timeoutMs);
    try {
      if (!await source.answer(p.id, value)) return false;
      if (!await next.promise) return false;
      progress(k + 1);
    } finally {
      next.cancel();
    }
  }
  return true;
}
