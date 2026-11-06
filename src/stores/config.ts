// Dati letti dal backend o dalle impostazioni, non legati a un tab: profili
// degli agenti, integrazioni (con lo stato dei loro ambienti), avvii integrati, bot.

import { createStore } from "../lib/store.ts";
import { botImageOf } from "../lib/bots.ts";
import type { BotImage } from "../lib/bots.ts";
import { defaultOf, enabledEnvStatuses, enabledOf } from "../lib/integrations.ts";
import type { AgentEnvStatus, AgentProfile, BotSettings, BuiltinLaunch, IntegrationInfo } from "../lib/tauri.ts";

export interface ConfigState {
  /** Profili di input e ripresa degli agenti (statici): letti una volta all'avvio. */
  agentProfiles: AgentProfile[];
  /** Profili caricati (o errore già segnalato): il ripristino della sessione li aspetta. */
  profilesReady: boolean;
  /** Avvii integrati con i preset sull'agente predefinito (workspace senza un agente proprio). */
  builtinLaunches: BuiltinLaunch[];
  /** Avvii integrati per i workspace con un agente proprio, per id dell'agente. */
  agentLaunches: Record<string, BuiltinLaunch[]>;
  globalLaunch: string | null;
  /** Interruttore globale dell'utilizzo (limiti e token). */
  showUsage: boolean;
  /** Nome e avatar di ogni sessione; la pool serve ai nomi dei tab ripristinati. */
  botSettings: BotSettings;
  /** Tutte le integrazioni, abilitate o no, con lo stato per ambiente: i componenti usano i selettori. */
  integrations: IntegrationInfo[];
  appVersion: string | null;
}

export const DEFAULT_BOT_SETTINGS: BotSettings = { enabled: true, names: [], images: {}, agents: [] };

export const INITIAL_CONFIG: ConfigState = {
  agentProfiles: [],
  profilesReady: false,
  builtinLaunches: [],
  agentLaunches: {},
  globalLaunch: null,
  showUsage: true,
  botSettings: DEFAULT_BOT_SETTINGS,
  integrations: [],
  appVersion: null,
};

/** Aggiorna alcuni campi. Se ognuno è `Object.is`-uguale a quello attuale: stesso stato. */
export function patchConfig(s: ConfigState, patch: Partial<ConfigState>): ConfigState {
  const changes = Object.entries(patch).filter(
    ([key, value]) => value !== undefined && !Object.is(s[key as keyof ConfigState], value),
  );
  return changes.length === 0 ? s : { ...s, ...Object.fromEntries(changes) };
}

type BotImageResolver = (name: string) => BotImage | undefined;

// Un risolutore per coppia (names, images): finché le impostazioni non cambiano
// la funzione è la stessa e le immagini sono gli stessi oggetti, così i figli
// memoizzati non si ridisegnano.
const resolverCache = new WeakMap<readonly string[], WeakMap<object, BotImageResolver>>();

function makeResolver(source: { names: readonly string[]; images: Record<string, BotImage> }): BotImageResolver {
  const byName = new Map<string, BotImage | undefined>();
  return (name) => {
    if (!byName.has(name)) byName.set(name, botImageOf(source, name));
    return byName.get(name);
  };
}

/** Immagine di un bot per nome, con cache per `(names, images)`. */
export function botImageResolver(bots: BotSettings): BotImageResolver {
  let byImages = resolverCache.get(bots.names);
  if (!byImages) {
    byImages = new WeakMap();
    resolverCache.set(bots.names, byImages);
  }
  let resolver = byImages.get(bots.images);
  if (!resolver) {
    resolver = makeResolver({ names: bots.names, images: bots.images });
    byImages.set(bots.images, resolver);
  }
  return resolver;
}

/** Il bot si mostra solo con la funzione attiva. */
export function shownBot(bots: BotSettings, name: string): string | undefined {
  return bots.enabled ? name : undefined;
}

/** Avvii integrati di un workspace: i preset partono con il suo agente, se ne ha uno. */
export function launchesFor(
  s: Pick<ConfigState, "builtinLaunches" | "agentLaunches">,
  ws: { agent?: string | null },
): BuiltinLaunch[] {
  return (ws.agent ? s.agentLaunches[ws.agent] : undefined) ?? s.builtinLaunches;
}

type WithIntegrations = Pick<ConfigState, "integrations">;

/** Integrazioni abilitate: l'unico elenco che il resto dell'interfaccia deve considerare. */
export const enabledIntegrations = (s: WithIntegrations): IntegrationInfo[] => enabledOf(s.integrations);

/** Integrazione predefinita (una delle abilitate); `null` finché l'elenco non è caricato. */
export const defaultIntegration = (s: WithIntegrations): IntegrationInfo | null => defaultOf(s.integrations);

/** Stato degli ambienti delle integrazioni abilitate. */
export const enabledEnvs = (s: WithIntegrations): AgentEnvStatus[] => enabledEnvStatuses(s.integrations);

export function createConfigStore() {
  const store = createStore<ConfigState>(INITIAL_CONFIG);
  return {
    ...store,
    patch: (patch: Partial<ConfigState>) => store.update((s) => patchConfig(s, patch)),
  };
}

export const configStore = createConfigStore();
