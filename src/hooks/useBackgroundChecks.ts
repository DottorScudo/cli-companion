import { useEffect } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { api } from "../lib/tauri";
import { ignoreError, report } from "../lib/errors";
import { uiStore } from "../stores/ui";
import { useStore } from "./useStore";

/** Attesa prima del controllo in background (pacchetti non aggiornati) dopo il ripristino della sessione. */
const BACKGROUND_CHECK_DELAY_MS = 1500;

type Check = (isCancelled: () => boolean) => void;

/**
 * Esegue `run` una sola volta, dopo `BACKGROUND_CHECK_DELAY_MS`, a sessione
 * già ripristinata. Il cleanup annulla il timer e fa scartare la risposta
 * (anche nel doppio effect di StrictMode).
 */
function useDelayedCheck(enabled: boolean, run: Check): void {
  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    const id = setTimeout(() => run(() => cancelled), BACKGROUND_CHECK_DELAY_MS);
    return () => {
      cancelled = true;
      clearTimeout(id);
    };
  }, [enabled, run]);
}

/** Plugin installati con versione diversa dall'app: controllo una sola volta per avvio. */
const detectOutdatedPlugins: Check = (isCancelled) => {
  api
    .agentPluginsOutdated()
    .then((list) => {
      if (!isCancelled() && list.length > 0) uiStore.setOutdatedPlugins(list);
    })
    .catch((e) => report("controllo dei plugin installati", e));
};

/**
 * Controlli in background. Aggiornamento dell'app (plugin updater): silenzioso
 * all'avvio, se c'è un update compare il pallino su "Impostazioni"; gli errori
 * (rete, release assente) non fanno rumore. Pacchetti degli agenti non
 * aggiornati: una volta sola, con un po' di ritardo, quando la sessione è già
 * ripristinata. Non blocca la UI.
 */
export function useBackgroundChecks(): void {
  const sessionReady = useStore(uiStore, (s) => s.sessionReady);

  useEffect(() => {
    check()
      .then((update) => uiStore.setUpdateAvailable(update !== null))
      .catch(ignoreError("controllo silenzioso degli aggiornamenti: senza rete o release non c'è nulla da dire"));
  }, []);

  useDelayedCheck(sessionReady, detectOutdatedPlugins);
}
