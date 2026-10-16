// "Rotate your phone" cover for a portrait phone (see RotateHint.module.css).

import { useSyncExternalStore } from "react";
import { t as tr } from "../i18n/t";
import { forceLandscape, isForced, subscribeForced, usePortraitPhone } from "./rotation";
import s from "./RotateHint.module.css";

export function RotateHint() {
  const portrait = usePortraitPhone();
  const forced = useSyncExternalStore(subscribeForced, isForced);
  if (!portrait || forced) return null;
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
