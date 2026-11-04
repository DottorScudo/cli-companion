import { useEffect, useMemo, useState } from "react";
import { Button } from "../ui/button";
import { api } from "../../lib/tauri";
import type { AgentProfile, Workspace } from "../../lib/tauri";
import { parseWorkerRules, rulesToText } from "../../lib/workerPermission";
import type { WorkerPermission } from "../../lib/workerPermission";
import { Field, Section } from "./shared";
import type { WorkspaceRun } from "./shared";

const rulesAreaClass =
  "w-full resize-y rounded-md border border-input bg-transparent px-2 py-1.5 font-mono text-[12px] outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-50";

const sameList = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

interface Props {
  workspace: Workspace;
  profile: AgentProfile;
  permission: WorkerPermission;
  /** Regole salvate per l'agente. */
  saved: { allow: string[]; deny: string[] };
  run: WorkspaceRun;
  saving: boolean;
}

/** Regole allow/deny di un agente, nella sintassi che l'integrazione dichiara; senza il contratto dei permessi le regole non si applicano. */
export default function WorkerPermissionRules({ workspace, profile, permission, saved, run, saving }: Props) {
  const [allowDraft, setAllowDraft] = useState("");
  const [denyDraft, setDenyDraft] = useState("");
  useEffect(() => {
    setAllowDraft(rulesToText(saved.allow));
    setDenyDraft(rulesToText(saved.deny));
  }, [workspace.id, profile.agent_id, saved.allow, saved.deny]);

  const allowRules = useMemo(() => parseWorkerRules(allowDraft), [allowDraft]);
  const denyRules = useMemo(() => parseWorkerRules(denyDraft), [denyDraft]);
  const changed = !sameList(allowRules, saved.allow) || !sameList(denyRules, saved.deny);
  const guide = profile.worker_permissions;

  if (!guide) {
    return (
      <Section>
        <Field label="Comandi pre-approvati">
          <p className="text-[11px] text-muted-foreground">
            {profile.agent_name} non supporta i comandi pre-approvati: le regole non si applicano ai worker.
          </p>
        </Field>
      </Section>
    );
  }

  return (
    <Section>
      <Field label="Comandi pre-approvati" help={guide.rules_help}>
        {permission === "bypass" && (
          <p className="mb-2 text-[11px] text-muted-foreground">
            Con i permessi a bypass i worker non chiedono mai conferma: i Consentiti non servono, i Vietati restano in
            vigore.
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
              aria-label="Comandi consentiti"
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
              aria-label="Comandi vietati"
              className={rulesAreaClass}
            />
          </div>
        </div>
        <div className="mt-2">
          <Button
            size="sm"
            variant="secondary"
            disabled={saving || !changed}
            onClick={() =>
              void run(() => api.setWorkerPermissionRules(workspace.id, profile.agent_id, allowRules, denyRules))
            }
          >
            Salva
          </Button>
        </div>
      </Field>
    </Section>
  );
}
