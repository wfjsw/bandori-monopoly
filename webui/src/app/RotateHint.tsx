// "Rotate your phone" cover for a portrait phone (see RotateHint.module.css).

import { useSyncExternalStore } from "react";
import { t as tr } from "../i18n/t";
import { ROTATE_QUERY as query, forceLandscape, isForced, subscribeForced } from "./rotation";
import s from "./RotateHint.module.css";


function subscribe(cb: () => void): () => void {
  const mq = window.matchMedia(query);
  mq.addEventListener("change", cb);
  return () => mq.removeEventListener("change", cb);
}

export function RotateHint() {
  const portrait = useSyncExternalStore(subscribe, () => window.matchMedia(query).matches);
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
