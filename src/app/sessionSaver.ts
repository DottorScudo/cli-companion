// Salvataggio della sessione: il salvatore (debounce) di `stores/session.ts`
// collegato agli archivi e al backend, più l'osservazione degli archivi che
// lo avvisa quando cambia qualcosa che si salva.

import { api } from "../lib/tauri";
import { report } from "../lib/errors";
import { buildSessionJson, createSessionSaver, sameSessionDeps, sessionDeps } from "../stores/session";
import { layoutStore } from "../stores/layout";
import { runtimeStore } from "../stores/tabRuntime";
import { tabsStore } from "../stores/tabs";

/** Il salvatore dell'app: spento finché la sessione non è stata letta con successo. */
export const sessionSaver = createSessionSaver({
  build: (withTitles) =>
    buildSessionJson(tabsStore.getState(), runtimeStore.getState(), layoutStore.getState().layout, withTitles),
  save: (json) => api.saveSession(json),
  onError: (e) => report("salvataggio della sessione", e, { notify: true }),
});

const currentDeps = () => sessionDeps(tabsStore.getState(), runtimeStore.getState(), layoutStore.getState().layout);

/**
 * Avvisa il salvatore a ogni modifica rilevante. Il confronto è sulle
 * dipendenze (`sameSessionDeps`), senza serializzare: una variazione di titolo
 * (cambiano di continuo con lo spinner degli agenti) non fa nemmeno partire il
 * confronto del JSON. Alla partenza avvisa una volta, come fa un primo render.
 */
export function watchSessionChanges(): () => void {
  let last = currentDeps();
  const check = () => {
    const next = currentDeps();
    if (sameSessionDeps(last, next)) return;
    last = next;
    sessionSaver.changed();
  };
  const offs = [tabsStore, runtimeStore, layoutStore].map((store) => store.subscribe(check));
  sessionSaver.changed();
  return () => offs.forEach((off) => off());
}
