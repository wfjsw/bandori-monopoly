// A plain `t(key)` for UI strings (the `ui` namespace).
//
// Components do not use the React hook: on a language change the app re-renders
// wholesale (see `useLangVersion`), so a function-local `t` is always current and
// non-React modules (sessions, models) can translate too.

import i18n from "./index";

export type TParams = Record<string, unknown>;

export function t(key: string, params?: TParams): string {
  return i18n.t(key, { ...params, ns: "ui", defaultValue: key }) as string;
}

export { fmtMsg, type Msg, type Names } from "./msg";