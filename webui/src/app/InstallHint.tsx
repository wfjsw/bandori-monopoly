// "Add to home screen" cover, shown on a phone browser tab ahead of the
// rotate cover (see InstallHint.module.css). Either orientation. One prompt
// at a time: this one takes the screen until it is installed or explicitly
// overridden, and only then does RotateHint ask for a turn.

import { useSyncExternalStore, useState } from "react";
import { t as tr } from "../i18n/t";
import {
  dismissInstallHint,
  hasInstallPrompt,
  installHintVersion,
  runInstallPrompt,
  subscribeInstallHint,
  useInstallHint,
} from "./installHint";
import s from "./InstallHint.module.css";

export function InstallHint() {
  const show = useInstallHint();
  // Re-read whenever the store moves: `beforeinstallprompt` can land after
  // mount, and the Android sheet button must light up then.
  useSyncExternalStore(subscribeInstallHint, installHintVersion);
  // iOS Safari never fires `beforeinstallprompt`, so there the cover only
  // prints the share-menu steps. Once the sheet has been raised and the user
  // turned it down, fall back to those steps too rather than a dead button.
  const [sheetDone, setSheetDone] = useState(false);
  const canPrompt = hasInstallPrompt() && !sheetDone;
  if (!show) return null;
  return (
    <div className={s.hint} role="dialog" aria-label={tr("install.hint")}>
      <div className={s.phone}>
        <div className={s.arrow} />
      </div>
      <div>
        {tr("install.hint")}
        <small>{tr("install.why")}</small>
        {!canPrompt && <small className={s.steps}>{tr("install.steps")}</small>}
      </div>
      {canPrompt && (
        <button
          type="button"
          className={s.install}
          onClick={() => {
            setSheetDone(true);
            void runInstallPrompt();
          }}
        >
          {tr("install.action")}
        </button>
      )}
      <button type="button" className={s.override} onClick={dismissInstallHint}>
        {tr("install.override")}
      </button>
    </div>
  );
}