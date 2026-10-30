// Boot-time hydration of the persistent stores. Run once, before React mounts
// (`index.tsx`): every module that reads a setting synchronously gets its
// in-memory copy from here, and the two settings that were already applied at
// module scope (language, game server) are re-applied now that they are known.

import { applyStoredLang } from "../i18n";
import { initToken } from "../net/api";
import { applyStoredServer } from "../net/server";
import { hydrateSoloSave } from "../game/soloSave";
import { hydrateCustomRules } from "./customRules";
import { hydrateKv, KV_STORES } from "./kv";

export async function hydrateAppStorage(): Promise<void> {
  await Promise.all([...KV_STORES.map(hydrateKv), hydrateCustomRules(), hydrateSoloSave()]);
  applyStoredServer();
  initToken();
  await applyStoredLang();
}
