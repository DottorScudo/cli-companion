import { useState } from "react";
import { Button } from "../ui/button";
import {
  disableImpactOf,
  installPackage,
  removeFromEnvironments,
  removePackage,
  setDefaultIntegration,
  setIntegrationEnabled,
} from "../../app/actions/integrations";
import { agentRowKey } from "../../lib/agents";
import { errorMessage } from "../../lib/errors";
import { capabilityLabels, disableImpactLines, removableEnvs } from "../../lib/integrations";
import type { EnvAction } from "../../lib/integrations";
import type { IntegrationInfo } from "../../lib/tauri";
import IntegrationEnvRow from "./IntegrationEnvRow";
import { Section } from "./shared";

/** Nome del gruppo di radio: una sola integrazione predefinita. */
const DEFAULT_RADIO_GROUP = "default-integration";
/** Chiave di "operazione in corso" per ciò che non riguarda un solo ambiente. */
const CARD_BUSY_KEY = "card";

/** Una integrazione: abilitazione, predefinita, contratti dichiarati e pacchetto per ambiente. */
export default function IntegrationCard({ info }: { info: IntegrationInfo }) {
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [log, setLog] = useState<string | null>(null);
  // Righe della conferma di disabilitazione (null = nessuna conferma aperta).
  const [confirming, setConfirming] = useState<string[] | null>(null);

  const run = async (key: string, operation: () => Promise<string | void>) => {
    setBusyKey(key);
    setError(null);
    setLog(null);
    try {
      const out = await operation();
      if (out) setLog(out);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusyKey(null);
    }
  };

  const changeEnabled = (enabled: boolean) => {
    if (!enabled) {
      const lines = disableImpactLines(info.name, disableImpactOf(info.id));
      if (lines.length > 0) {
        setError(null);
        setConfirming(lines);
        return;
      }
    }
    void run(CARD_BUSY_KEY, () => setIntegrationEnabled(info.id, enabled));
  };

  const confirmDisable = () => {
    setConfirming(null);
    void run(CARD_BUSY_KEY, () => setIntegrationEnabled(info.id, false));
  };

  const runEnvAction = (key: string, envKey: string, action: EnvAction) =>
    run(key, () => (action === "remove" ? removePackage(info.id, envKey) : installPackage(info.id, envKey)));

  const removable = removableEnvs(info);
  const idle = busyKey === null;

  return (
    <Section>
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
        <h3 className="min-w-0 flex-1 truncate text-[13px] font-semibold">{info.name}</h3>
        <label className="flex items-center gap-1.5 text-xs">
          <input
            type="checkbox"
            checked={info.enabled}
            disabled={!idle}
            onChange={(e) => changeEnabled(e.target.checked)}
            className="h-3.5 w-3.5 accent-primary"
          />
          Abilitata
        </label>
        <label className="flex items-center gap-1.5 text-xs">
          <input
            type="radio"
            name={DEFAULT_RADIO_GROUP}
            checked={info.is_default}
            disabled={!info.enabled || !idle}
            onChange={() => void run(CARD_BUSY_KEY, () => setDefaultIntegration(info.id))}
            className="h-3.5 w-3.5 accent-primary"
          />
          Predefinita
        </label>
      </div>

      <ul className="flex flex-wrap gap-1" aria-label={`Funzioni di ${info.name}`}>
        {capabilityLabels(info.capabilities).map((label) => (
          <li key={label} className="rounded border px-1.5 py-px text-[10px] text-muted-foreground">
            {label}
          </li>
        ))}
      </ul>

      <div className="flex flex-col gap-1.5">
        {info.envs.map((env) => {
          const key = agentRowKey(env.status);
          return (
            <IntegrationEnvRow
              key={key}
              env={env}
              integrationEnabled={info.enabled}
              busy={busyKey === key}
              disabled={!idle}
              onAction={(action) => void runEnvAction(key, env.status.env_key, action)}
            />
          );
        })}
      </div>

      {!info.enabled && (
        <div className="flex items-center gap-2">
          <p className="flex-1 text-[11px] text-muted-foreground">
            Disabilitata: non compare negli avvii, nei task e nei worker e non viene aggiornata. Il pacchetto già
            installato resta negli ambienti.
          </p>
          {removable.length > 0 && (
            <Button
              size="sm"
              variant="outline"
              disabled={!idle}
              onClick={() => void run(CARD_BUSY_KEY, () => removeFromEnvironments(info))}
            >
              Rimuovi dagli ambienti
            </Button>
          )}
        </div>
      )}

      {confirming && (
        <div className="flex flex-col gap-1.5 rounded-md border border-destructive/40 p-2" role="alert">
          <p className="text-xs font-semibold">Disabilitare {info.name}?</p>
          {confirming.map((line) => (
            <p key={line} className="text-xs">
              {line}
            </p>
          ))}
          <div className="flex items-center gap-1.5">
            <Button size="sm" variant="destructive" onClick={confirmDisable}>
              Disabilita
            </Button>
            <Button size="sm" variant="secondary" onClick={() => setConfirming(null)}>
              Annulla
            </Button>
          </div>
        </div>
      )}

      {error && <p className="whitespace-pre-wrap text-xs text-destructive">{error}</p>}
      {log && (
        <pre className="max-h-32 overflow-auto whitespace-pre-wrap rounded-lg border bg-muted/50 p-2.5 font-mono text-[11px]">
          {log}
        </pre>
      )}
    </Section>
  );
}
