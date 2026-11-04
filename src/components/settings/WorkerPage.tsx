import { useEffect, useState } from "react";
import { api } from "../../lib/tauri";
import type { Workspace } from "../../lib/tauri";
import { useStore } from "../../hooks/useStore";
import { useWorkerAgent } from "../../hooks/useWorkerAgent";
import { configStore } from "../../stores/config";
import { WORKER_PERMISSIONS, normalizeWorkerPermission, workerPermissionLabel } from "../../lib/workerPermission";
import type { WorkerPermission } from "../../lib/workerPermission";
import { WORKER_RULES_MAX, sanitizeWorkerRules } from "../../lib/board";
import UseScheduleSection from "./UseScheduleSection";
import WorkerAgentField from "./WorkerAgentField";
import WorkerAgentSettings from "./WorkerAgentSettings";
import { Field, Section, SettingsPage, selectClass, useWorkspaceOp } from "./shared";

interface Props {
  workspace: Workspace;
}

/** Workspace › Worker: agente, permessi, modello, regole e fasce orarie dei worker automatici. */
export default function WorkerPage({ workspace }: Props) {
  const { run, saving, error } = useWorkspaceOp();
  const integrations = useStore(configStore, (s) => s.integrations);
  const profiles = useStore(configStore, (s) => s.agentProfiles);
  // L'avviso sul bypass è dell'agente che esegue i worker; modello e regole si mostrano
  // per l'agente scelto in "Configurati anche per" (all'inizio, quello dei worker).
  const { agentId, profile: workerAgent } = useWorkerAgent(workspace);
  const [viewedAgentId, setViewedAgentId] = useState<string | null>(null);
  useEffect(() => setViewedAgentId(null), [workspace.id, agentId]);

  const [rulesDraft, setRulesDraft] = useState("");
  const [rulesSaved, setRulesSaved] = useState(false);

  useEffect(() => {
    setRulesDraft(workspace.worker_rules ?? "");
    setRulesSaved(false);
  }, [workspace.id, workspace.worker_rules]);

  // Salva all'uscita dal campo, solo se cambiato; non blocca la pagina.
  const saveRules = () => {
    const rules = sanitizeWorkerRules(rulesDraft);
    if (rules === (workspace.worker_rules ?? "")) return;
    void run(() => api.setWorkerRules(workspace.id, rules)).then((ok) => setRulesSaved(ok));
  };

  const permission = normalizeWorkerPermission(workspace.worker_permission);

  return (
    <SettingsPage
      title="Worker"
      description="Come lavorano i worker automatici che eseguono i task della coda di questo workspace."
    >
      <WorkerAgentField workspace={workspace} integrations={integrations} run={run} saving={saving} />

      <Section>
        <Field
          label="Permessi dei worker automatici"
          help="Livello massimo dei permessi dei worker automatici di questo workspace."
        >
          <select
            value={permission}
            disabled={saving}
            onChange={(e) => void run(() => api.setWorkerPermission(workspace.id, e.target.value as WorkerPermission))}
            className={selectClass}
          >
            {WORKER_PERMISSIONS.map((m) => (
              <option key={m} value={m}>
                {workerPermissionLabel(m)}
              </option>
            ))}
          </select>
          {permission === "bypass" && (
            <p className="mt-1 text-[11px] text-destructive">
              I worker eseguiranno qualsiasi comando senza conferma. {workerAgent?.worker_permissions?.bypass_note}
            </p>
          )}
        </Field>
      </Section>

      <WorkerAgentSettings
        workspace={workspace}
        agentId={viewedAgentId ?? agentId}
        profiles={profiles}
        integrations={integrations}
        permission={permission}
        onSelectAgent={setViewedAgentId}
        run={run}
        saving={saving}
      />

      <UseScheduleSection workspace={workspace} />

      <Section>
        <Field
          label="Regole per i worker"
          help="Test vietati, comandi da non lanciare… inviate a ogni worker automatico insieme alle sue istruzioni. Valgono per i nuovi worker; non serve ripeterle nei task."
        >
          <textarea
            value={rulesDraft}
            onChange={(e) => {
              setRulesDraft(e.target.value);
              setRulesSaved(false);
            }}
            onBlur={saveRules}
            rows={5}
            maxLength={WORKER_RULES_MAX}
            placeholder="Es. Non lanciare i test e2e. Non usare npm install: usa pnpm."
            aria-label="Regole per i worker"
            className="flex w-full resize-y rounded-md border border-input bg-transparent px-3 py-2 text-[12px] shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
          />
          <p className="mt-1 flex justify-between text-[11px] text-muted-foreground">
            <span>{rulesSaved && !saving ? "Salvato" : saving ? "Salvataggio…" : "Si salva all’uscita dal campo"}</span>
            <span className="font-mono">
              {rulesDraft.length}/{WORKER_RULES_MAX}
            </span>
          </p>
        </Field>
      </Section>

      {error && <p className="text-xs text-destructive">{error}</p>}
    </SettingsPage>
  );
}
