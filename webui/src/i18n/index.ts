// i18next setup: three namespaces.
//
//   ui     -- web client copy (src/i18n/locales/<lang>/ui.json)
//   game   -- engine / server messages (keys come from Msg.k in the wire protocol)
//   cards  -- card-module strings (built from the card crates' locales/*.json)
//
// `game` keys are flat (`log.roll`, `err.poor`) and match `Msg::new("..")` in the
// Rust code 1:1; tools/i18n/check.py keeps the two sides in sync. Card keys are
// `cards:<crate>.<key>`. Keys with no translation fall back to zh-CN (the source
// language of the game data), never to the raw key.

import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import enCards from "./locales/en/cards.json";
import enGame from "./locales/en/game.json";
import enUi from "./locales/en/ui.json";
import zhCards from "./locales/zh-CN/cards.json";
import zhGame from "./locales/zh-CN/game.json";
import zhUi from "./locales/zh-CN/ui.json";

export const LANGS = ["zh-CN", "en"] as const;
export type Lang = (typeof LANGS)[number];
export const LANG_LABEL: Record<Lang, string> = { "zh-CN": "简体中文", en: "English" };
const KEY = "bm.lang";

export function savedLang(): Lang {
  const v = localStorage.getItem(KEY);
  return v === "en" || v === "zh-CN" ? v : "zh-CN";
}

/** Every language change bumps this; `useLangVersion` re-renders the app on it. */
let version = 0;
const listeners = new Set<() => void>();

export function langVersion(): number {
  return version;
}

export function onLangChange(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/** Switch language at runtime (设定), and remember the choice. */
export async function setLang(lang: Lang): Promise<void> {
  localStorage.setItem(KEY, lang);
  await i18n.changeLanguage(lang);
  document.documentElement.lang = lang;
  version += 1;
  listeners.forEach((cb) => cb());
}

const bundled = {
  "zh-CN": { ui: zhUi, game: zhGame, cards: zhCards },
  en: { ui: enUi, game: enGame, cards: enCards },
};

void i18n.use(initReactI18next).init({
  lng: savedLang(),
  fallbackLng: "zh-CN",
  supportedLngs: [...LANGS],
  defaultNS: "ui",
  ns: ["ui", "game", "cards"],
  // Flat keys everywhere (`log.roll` is one key, not nested), matching the Rust
  // message keys and the card crates' locale files exactly.
  keySeparator: false,
  nsSeparator: ":",
  // An argument the sender left out renders as nothing rather than as a raw
  // `{{what}}` -- the engine's convention is that optional parts (`what`,
  // `detail`, ...) are blank when absent, and an older server or a card that
  // forgets one should not leak template syntax into the log.
  interpolation: { escapeValue: false },
  missingInterpolationHandler: (_text: string, value: unknown) => {
    if (import.meta.env?.DEV) console.warn("[i18n] missing interpolation", value);
    return "";
  },
  returnNull: false,
  resources: bundled as unknown as Record<string, Record<string, object>>,
});

document.documentElement.lang = i18n.language;

/**
 * Card modules are built separately (`tools/build-ruleset.sh`); merge their locale
 * bundles on top of the stub so `cards:<crate>.<key>` resolves. A missing bundle is
 * fine -- card messages just fall back.
 */
export async function loadCardLocales(): Promise<void> {
  for (const lang of i18n.languages ?? [i18n.language]) {
    try {
      const r = await fetch(`/assets/i18n/cards-${lang}.json`);
      if (!r.ok) continue;
      const data = (await r.json()) as Record<string, Record<string, string>>;
      for (const [crate, strings] of Object.entries(data)) {
        for (const [k, v] of Object.entries(strings)) {
          i18n.addResource(lang, "cards", `${crate}.${k}`, v);
        }
      }
    } catch {
      // offline or not built yet
    }
  }
}

export default i18n;