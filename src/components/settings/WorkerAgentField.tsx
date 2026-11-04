import { api } from "../../lib/tauri";
import type { IntegrationInfo, Workspace } from "../../lib/tauri";
import { INHERIT_AGENT_VALUE, workerAgentOptions, workerAgentWarning } from "../../lib/workerAgentChoices";
import { Field, Section, selectClass } from "./shared";
import type { WorkspaceRun } from "./shared";

interface Props {
  workspace: Workspace;
  integrations: IntegrationInfo[];
  run: WorkspaceRun;
  saving: boolean;
}

/** Workspace › Worker: l'agente che esegue i worker (eredita il predefinito globale o ne sceglie uno). */
export default function WorkerAgentField({ workspace, integrations, run, saving }: Props) {
  const warning = workerAgentWarning(workspace.agent, integrations);
  return (
    <Section>
      <Field
        label="Agente dei worker"
        help="L’agente che esegue i task della coda di questo workspace. Un singolo task può indicarne uno diverso. Modello e regole di permesso si impostano per ogni agente."
      >
        <select
          value={workspace.agent ?? INHERIT_AGENT_VALUE}
          disabled={saving}
          onChange={(e) => void run(() => api.setWorkspaceAgent(workspace.id, e.target.value || null))}
          className={selectClass}
        >
          {workerAgentOptions(integrations, workspace.agent).map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
        {warning && (
          <p role="alert" className="mt-1 text-[11px] text-destructive">
            {warning}
          </p>
        )}
      </Field>
    </Section>
  );
}
