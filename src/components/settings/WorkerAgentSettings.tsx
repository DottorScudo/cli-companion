import { Button } from "../ui/button";
import type { AgentProfile, IntegrationInfo, Workspace } from "../../lib/tauri";
import { otherConfiguredAgents } from "../../lib/workerAgentChoices";
import { workerAgentSettings } from "../../lib/workerAgent";
import type { WorkerPermission } from "../../lib/workerPermission";
import { Section } from "./shared";
import type { WorkspaceRun } from "./shared";
import WorkerModelField from "./WorkerModelField";
import WorkerPermissionRules from "./WorkerPermissionRules";

interface Props {
  workspace: Workspace;
  /** Agente di cui si mostrano le impostazioni (`undefined` = profili non ancora caricati). */
  agentId: string | undefined;
  profiles: AgentProfile[];
  integrations: IntegrationInfo[];
  permission: WorkerPermission;
  onSelectAgent: (agentId: string) => void;
  run: WorkspaceRun;
  saving: boolean;
}

/** Modello e regole di permesso di un agente, con il passaggio agli altri agenti già configurati. */
export default function WorkerAgentSettings({
  workspace,
  agentId,
  profiles,
  integrations,
  permission,
  onSelectAgent,
  run,
  saving,
}: Props) {
  const profile = profiles.find((p) => p.agent_id === agentId);
  if (!profile) {
    return (
      <Section>
        <p className="text-[11px] text-muted-foreground">
          Modello e regole di permesso si impostano per un agente abilitato: scegline uno qui sopra.
        </p>
      </Section>
    );
  }

  const settings = workerAgentSettings(workspace, profile.agent_id);
  const others = otherConfiguredAgents(workspace.worker_agents, profile.agent_id, profiles);

  return (
    <>
      <p className="text-[11px] text-muted-foreground">
        Impostazioni di <span className="font-medium text-foreground">{profile.agent_name}</span>
      </p>
      <WorkerPermissionRules
        workspace={workspace}
        profile={profile}
        permission={permission}
        saved={settings}
        run={run}
        saving={saving}
      />
      <WorkerModelField
        workspace={workspace}
        integration={integrations.find((i) => i.id === profile.agent_id)}
        agentId={profile.agent_id}
        saved={settings.model}
        run={run}
        saving={saving}
      />
      {others.length > 0 && (
        <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
          <span>Configurati anche per:</span>
          {others.map((a) => (
            <Button key={a.id} size="sm" variant="outline" onClick={() => onSelectAgent(a.id)}>
              {a.name}
            </Button>
          ))}
        </div>
      )}
    </>
  );
}
