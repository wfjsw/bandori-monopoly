// The match's commit-reveal commitment (`docs/FAIRNESS.md`): a short,
// copyable chip. The full hash is what a skeptic compares against the
// recompute the replay viewer's "verify" runs.

import { toast } from "./Toast";
import { t as tr } from "../i18n/t";
import s from "./CommitChip.module.css";

export function CommitChip({ commit }: { commit: string }) {
  if (!commit) return null;
  const short = commit.slice(0, 12);
  return (
    <button
      type="button"
      className={s.chip}
      title={tr("room.commitHint")}
      onClick={() => {
        void navigator.clipboard?.writeText(commit);
        toast(tr("room.commitCopied"));
      }}
    >
      <span className={s.label}>{tr("room.commit")}</span>
      <code className={s.hash}>{short}…</code>
    </button>
  );
}