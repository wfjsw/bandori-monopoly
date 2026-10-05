import { sceneImg } from "../core/assets";
import { cx } from "../core/cx";
import s from "./Icon.module.css";

/** A white icon sprite (`ic_*`) used as a mask and tinted with `currentColor`. */
export function Icon({ name, size, className }: { name: string; size?: number; className?: string }) {
  const src = sceneImg(name.startsWith("ic_") ? name : `ic_${name}`);
  return <i className={cx(s.icon, className)} style={{ ["--ic" as string]: src ? `url("${src}")` : undefined, width: size, height: size }} />;
}
