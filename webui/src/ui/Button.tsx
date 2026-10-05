import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cx } from "../core/cx";
import s from "./Button.module.css";
import { Icon } from "./Icon";

export interface BtnProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  kind?: "white" | "pink" | "blue";
  size?: "small" | "normal" | "big";
  icon?: string;
  wide?: boolean;
  children?: ReactNode;
}

/** The original's raised buttons: white, pink or blue, with an optional icon. */
export function Btn({ kind = "white", size = "normal", icon, wide, className, children, type = "button", ...rest }: BtnProps) {
  return (
    <button type={type} className={cx(s.btn, s[kind], size !== "normal" && s[size], wide && s.wide, className)} {...rest}>
      {icon && <Icon name={icon} className={s.icon} />}
      {children}
    </button>
  );
}

/** A small round pink button (steppers). */
export function RoundBtn({ icon, className, ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { icon: string }) {
  return (
    <button type="button" className={cx(s.round, className)} {...rest}>
      <Icon name={icon} />
    </button>
  );
}
