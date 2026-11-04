import { useEffect, useMemo, useState } from "react";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { api } from "../../lib/tauri";
import type { Workspace } from "../../lib/tauri";
import { useStore } from "../../hooks/useStore";
import { configStore } from "../../stores/config";
import {
  WORKER_PERMISSIONS,
  normalizeWorkerPermission,
  parseWorkerRules,
  rulesToText,
  workerPermissionLabel,
} from "../../lib/workerPermission";
import type { WorkerPermission } from "../../lib/workerPermission";
import { modelError, sanitizeModel } from "../../lib/model";
import { WORKER_RULES_MAX, sanitizeWorkerRules } from "../../lib/board";
import UseScheduleSection from "./UseScheduleSection";
import { Field, Section, SettingsPage, selectClass, useWorkspaceOp } from "./shared";

const rulesAreaClass =
  "w-full resize-y rounded-md border border-input bg-transparent px-2 py-1.5 font-mono text-[12px] outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-50";

const sameList = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

interface Props {
  workspace: Workspace;
}

/** Workspace › Worker: permessi, modello, regole e fasce orarie dei worker automatici. */
export default function WorkerPage({ workspace }: Props) {
  const { run, saving, error } = useWorkspaceOp();
  const agentProfiles = useStore(configStore, (s) => s.agentProfiles);

  // Bozze: seguono il workspace selezionato e il valore salvato.
  const [workerModelDraft, setWorkerModelDraft] = useState("");
  const [allowDraft, setAllowDraft] = useState("");
  const [denyDraft, setDenyDraft] = useState("");
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

  useEffect(() => {
    setWorkerModelDraft(workspace.worker_model ?? "");
  }, [workspace.id, workspace.worker_model]);

  useEffect(() => {
    setAllowDraft(rulesToText(workspace.worker_allow));
    setDenyDraft(rulesToText(workspace.worker_deny));
  }, [workspace.id, workspace.worker_allow, workspace.worker_deny]);

  // I worker usano l'agente predefinito: i testi sui permessi sono suoi.
  const agent = agentProfiles.find((p) => p.is_default);
  const guide = agent?.worker_permissions ?? null;
  const permission = normalizeWorkerPermission(workspace.worker_permission);
  const allowRules = useMemo(() => parseWorkerRules(allowDraft), [allowDraft]);
  const denyRules = useMemo(() => parseWorkerRules(denyDraft), [denyDraft]);
  const rulesChanged = !sameList(allowRules, workspace.worker_allow) || !sameList(denyRules, workspace.worker_deny);

  return (
    <SettingsPage
      title="Worker"
      description="Come lavorano i worker automatici che eseguono i task della coda di questo workspace."
    >
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
              I worker eseguiranno qualsiasi comando senza conferma. {guide?.bypass_note}
            </p>
          )}
        </Field>
      </Section>

      {guide ? (
        <Section>
          <Field label="Comandi pre-approvati" help={guide.rules_help}>
            {permission === "bypass" && (
              <p className="mb-2 text-[11px] text-muted-foreground">
                Con i permessi a bypass i worker non chiedono mai conferma: i Consentiti non servono, i Vietati restano
                in vigore.
              </p>
            )}
            <div className="grid gap-3 md:grid-cols-2">
              <div>
                <div className="mb-1 text-[11px] font-medium">Consentiti</div>
                <textarea
                  value={allowDraft}
                  onChange={(e) => setAllowDraft(e.target.value)}
                  disabled={permission === "bypass"}
                  rows={5}
                  spellCheck={false}
                  placeholder={guide.allow_placeholder}
                  className={rulesAreaClass}
                />
              </div>
              <div>
                <div className="mb-1 text-[11px] font-medium">Vietati</div>
                <textarea
                  value={denyDraft}
                  onChange={(e) => setDenyDraft(e.target.value)}
                  rows={5}
                  spellCheck={false}
                  placeholder={guide.deny_placeholder}
                  className={rulesAreaClass}
                />
              </div>
            </div>
            <div className="mt-2">
              <Button
                size="sm"
                variant="secondary"
                disabled={saving || !rulesChanged}
                onClick={() => void run(() => api.setWorkerPermissionRules(workspace.id, allowRules, denyRules))}
              >
                Salva
              </Button>
            </div>
          </Field>
        </Section>
      ) : (
        agent && (
          <Section>
            <Field label="Comandi pre-approvati">
              <p className="text-[11px] text-muted-foreground">
                {agent.agent_name} non supporta i comandi pre-approvati: le regole non si applicano ai worker.
              </p>
            </Field>
          </Section>
        )
      )}

      <Section>
        <Field
          label="Modello dei worker"
          help="Modello dei worker automatici di questo workspace. Vuoto = predefinito dell’agente. Un singolo task può indicare un modello diverso."
        >
          <div className="flex gap-2">
            <Input
              value={workerModelDraft}
              onChange={(e) => setWorkerModelDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
              }}
              placeholder="predefinito dell’agente"
              className="h-8 font-mono text-[12px]"
            />
            <Button
              size="sm"
              variant="secondary"
              disabled={
                saving ||
                modelError(workerModelDraft) !== null ||
                sanitizeModel(workerModelDraft) === workspace.worker_model
              }
              onClick={() => void run(() => api.setWorkerModel(workspace.id, sanitizeModel(workerModelDraft)))}
            >
              Salva
            </Button>
          </div>
          {modelError(workerModelDraft) && (
            <p className="mt-1 text-[11px] text-destructive">{modelError(workerModelDraft)}</p>
          )}
        </Field>
      </Section>

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
