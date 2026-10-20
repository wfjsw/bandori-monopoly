// "Rotate your phone" cover for a portrait phone (see RotateHint.module.css).
// Steps aside while the install cover (InstallHint) is up: one prompt at a
// time, install first, rotate after that one is resolved.

import { useSyncExternalStore } from "react";
import { t as tr } from "../i18n/t";
import { useInstallHint } from "./installHint";
import { forceLandscape, isForced, subscribeForced, usePortraitPhone } from "./rotation";
import s from "./RotateHint.module.css";

export function RotateHint() {
  const portrait = usePortraitPhone();
  const forced = useSyncExternalStore(subscribeForced, isForced);
  const installCover = useInstallHint();
  if (!portrait || forced || installCover) return null;
  return (
    <div className={s.hint} role="alert">
      <div className={s.phone} />
      <div>
        {tr("rotate.hint")}
        <small>{tr("rotate.why")}</small>
      </div>
      <button type="button" className={s.force} onClick={forceLandscape}>
        {tr("rotate.force")}
      </button>
    </div>
  );
}
