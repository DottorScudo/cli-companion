import { Loader2 } from "lucide-react";
import { Button } from "../ui/button";
import { Badge } from "../ui/badge";
import { agentEnvBadge } from "../../lib/agents";
import { ENV_ACTION_LABELS, envActions, envState, envStateText } from "../../lib/integrations";
import type { EnvAction, EnvState } from "../../lib/integrations";
import type { IntegrationEnv } from "../../lib/tauri";
import { cn } from "../../lib/utils";

const STATE_DOT: Record<EnvState, string> = {
  ready: "bg-status-finished",
  found: "bg-status-running",
  outdated: "bg-status-running",
  installed: "bg-muted-foreground/60",
  "not-found": "bg-muted-foreground/40",
};

/** Un ambiente (Windows o distro WSL) di un'integrazione: stato e azioni sul pacchetto. */
export default function IntegrationEnvRow({
  env,
  integrationEnabled,
  busy,
  disabled,
  onAction,
}: {
  env: IntegrationEnv;
  integrationEnabled: boolean;
  /** L'operazione in corso è su questo ambiente. */
  busy: boolean;
  /** Un'altra operazione è in corso: i bottoni aspettano. */
  disabled: boolean;
  onAction: (action: EnvAction) => void;
}) {
  return (
    <div className="flex items-center gap-2 rounded-md border bg-background/40 p-2">
      <Badge variant="secondary" className="font-mono">
        {agentEnvBadge(env.status)}
      </Badge>
      <span className={cn("h-2 w-2 flex-shrink-0 rounded-full", STATE_DOT[envState(env)])} />
      <span className="min-w-0 flex-1 truncate text-xs">{envStateText(env)}</span>
      {busy && <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />}
      {envActions(env, integrationEnabled).map((action) => (
        <Button
          key={action}
          size="sm"
          variant={action === "remove" ? "ghost" : "outline"}
          disabled={disabled || busy}
          onClick={() => onAction(action)}
        >
          {ENV_ACTION_LABELS[action]}
        </Button>
      ))}
    </div>
  );
}
