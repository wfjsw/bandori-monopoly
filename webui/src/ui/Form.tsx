// Form bits: rounded text input and labelled rows.

import type { InputHTMLAttributes, ReactNode } from "react";
import { cx } from "../core/cx";
import s from "./Form.module.css";

export function TextInput({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={cx(s.input, className)} {...rest} />;
}

export function FormRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={s.row}>
      <span className={s.label}>{label}</span>
      {children}
    </div>
  );
}

export function Form({ onSubmit, children }: { onSubmit: () => void; children: ReactNode }) {
  return (
    <form className={s.form} onSubmit={(e) => { e.preventDefault(); onSubmit(); }}>
      {children}
    </form>
  );
}

export function Center({ children }: { children: ReactNode }) {
  return <div className={s.center}>{children}</div>;
}
