// The user-facing crash modal. Self-contained on purpose: `ModalHost` may live
// inside the crashed subtree, so this panel renders its own overlay chrome and
// never goes through `openModal`.

import { useEffect, useRef, useState } from "react";
import { crashDebugText, CRASH_LOG_TAIL, type CrashInfo, type CrashLogLine } from "../core/crash";
import { getLogs } from "../console/log";
import { t } from "../i18n/t";
import { Btn } from "../ui/Button";
import s from "./CrashPanel.module.css";

function formatLogLine(line: CrashLogLine): string {
  return `${new Date(line.time).toISOString()} [${line.level}] ${line.text}`;
}

export function CrashPanel({ info }: { info: CrashInfo }) {
  const [copied, setCopied] = useState(false);
  const [fallback, setFallback] = useState<string | null>(null);
  const fallbackRef = useRef<HTMLTextAreaElement>(null);
  // Snapshot once per panel: the tree is gone, a live tail would only churn.
  const logs = getLogs().slice(-CRASH_LOG_TAIL);

  // Built at copy time so the blob is exactly what the user saw listed below.
  const debug = () => crashDebugText(info, logs);

  // After dropping to the textarea, select everything so a Ctrl+C still works.
  useEffect(() => {
    if (fallback) fallbackRef.current?.select();
  }, [fallback]);

  async function copy() {
    const text = debug();
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      // Clipboard denied or missing (some webviews): show the text to copy by hand.
      setFallback(text);
    }
  }

  return (
    <div className={s.back}>
      <div
        className={s.panel}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="crash-title"
        aria-describedby="crash-body"
        tabIndex={-1}
      >
        <div className={s.head}>
          <h2 id="crash-title">{t("crash.title")}</h2>
        </div>
        <div className={s.body}>
          <p id="crash-body" className={s.lead}>
            {t("crash.body")}
          </p>
          <div className={s.row}>
            <Btn kind="pink" size="big" autoFocus onClick={() => location.reload()}>
              {t("crash.refresh")}
            </Btn>
          </div>
          <p className={s.hint}>{t("crash.copyHint")}</p>
          <details className={s.details}>
            <summary className={s.summary}>
              <span>{t("crash.detail")}</span>
              <span className={s.summaryMsg}>{t("crash.message", { message: info.message })}</span>
            </summary>
            <div className={s.debugBox}>
              {info.stack && (
                <>
                  <h3 className={s.debugLabel}>{t("crash.stack")}</h3>
                  <pre className={s.debug}>{info.stack}</pre>
                </>
              )}
              {info.componentStack && (
                <>
                  <h3 className={s.debugLabel}>{t("crash.componentStack")}</h3>
                  <pre className={s.debug}>{info.componentStack}</pre>
                </>
              )}
              {logs.length > 0 && (
                <>
                  <h3 className={s.debugLabel}>{t("crash.recent")}</h3>
                  <pre className={s.debug}>{logs.map(formatLogLine).join("\n")}</pre>
                </>
              )}
            </div>
            <div className={s.row}>
              <Btn kind="white" size="small" onClick={() => void copy()} disabled={copied}>
                {copied ? t("crash.copied") : t("crash.copy")}
              </Btn>
            </div>
            {fallback !== null && (
              <textarea
                ref={fallbackRef}
                className={s.fallback}
                readOnly
                value={fallback}
                aria-label={t("crash.copy")}
              />
            )}
          </details>
        </div>
      </div>
    </div>
  );
}