// "Rotate your phone" cover for a portrait phone (see RotateHint.module.css).

import { useSyncExternalStore } from "react";
import { t as tr } from "../i18n/t";
import s from "./RotateHint.module.css";

const query = "(orientation: portrait) and (pointer: coarse) and (max-width: 600px)";

function subscribe(cb: () => void): () => void {
  const mq = window.matchMedia(query);
  mq.addEventListener("change", cb);
  return () => mq.removeEventListener("change", cb);
}

export function RotateHint() {
  const portrait = useSyncExternalStore(subscribe, () => window.matchMedia(query).matches);
  if (!portrait) return null;
  return (
    <div className={s.hint} role="alert">
      <div className={s.phone} />
      <div>
        {tr("rotate.hint")}
        <small>{tr("rotate.why")}</small>
      </div>
    </div>
  );
}
