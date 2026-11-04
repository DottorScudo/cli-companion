import { useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { api } from "../lib/tauri";
import { ignoreError, report } from "../lib/errors";
import { configStore } from "../stores/config";
import { workspacesStore } from "../stores/workspaces";

/**
 * Dati letti una volta all'avvio e scritti negli archivi: workspace, profili
 * degli agenti, avvii integrati, impostazioni degli avvii e versione dell'app.
 */
export function useInitialData(): void {
  useEffect(() => {
    api
      .agentProfiles()
      .then((agentProfiles) => configStore.patch({ agentProfiles }))
      .catch((e) => report("caricamento dei profili degli agenti", e, { notify: true }))
      // Profili caricati (o errore già segnalato): il ripristino della sessione li aspetta.
      .finally(() => configStore.patch({ profilesReady: true }));
  }, []);

  useEffect(() => {
    api
      .listWorkspaces()
      .then((list) => workspacesStore.loaded(list))
      .catch((e) => workspacesStore.failed(String(e)));
  }, []);

  // Avvii integrati e avvio predefinito globale: una volta. Senza di loro l'app
  // userebbe i valori di default: l'utente va avvisato.
  useEffect(() => {
    api
      .builtinLaunches()
      .then((builtinLaunches) => configStore.patch({ builtinLaunches }))
      .catch((e) => report("caricamento degli avvii integrati", e, { notify: true }));
    api
      .getLaunchSettings()
      .then((s) => configStore.patch({ globalLaunch: s.default_launch, showUsage: s.show_usage !== false }))
      .catch((e) => report("caricamento delle impostazioni degli avvii", e, { notify: true }));
  }, []);

  // Versione dell'app (footer della sidebar).
  useEffect(() => {
    getVersion()
      .then((appVersion) => configStore.patch({ appVersion }))
      .catch(ignoreError("la versione è solo un'etichetta nel footer; fuori da Tauri non esiste"));
  }, []);
}
