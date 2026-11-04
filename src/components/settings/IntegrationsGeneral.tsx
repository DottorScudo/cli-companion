import { useState } from "react";
import { Loader2 } from "lucide-react";
import { Button } from "../ui/button";
import { setShowUsage as publishShowUsage } from "../../app/actions/settings";
import { api } from "../../lib/tauri";
import type { IntegrationInfo } from "../../lib/tauri";
import { errorMessage, report } from "../../lib/errors";
import { declaresAny, enabledOf, onlyEnabled } from "../../lib/integrations";
import { usageLimitsNotices } from "../../lib/usage";
import { useStore } from "../../hooks/useStore";
import { configStore } from "../../stores/config";
import { Section } from "./shared";

/** Contratti per cui l'interruttore "Mostra utilizzo" ha senso. */
const USAGE_CAPABILITIES = ["usage-limits", "session-usage"] as const;

/** Impostazioni che valgono per tutte le integrazioni: prova della notifica e utilizzo. */
export default function IntegrationsGeneral({ integrations }: { integrations: IntegrationInfo[] }) {
  const showUsage = useStore(configStore, (s) => s.showUsage);
  const profiles = useStore(configStore, (s) => s.agentProfiles);
  const [testing, setTesting] = useState(false);
  const [testLog, setTestLog] = useState<string | null>(null);
  const [testError, setTestError] = useState<string | null>(null);

  const usageApplies = declaresAny(enabledOf(integrations), USAGE_CAPABILITIES);

  const toggleUsage = async (value: boolean) => {
    try {
      publishShowUsage((await api.setShowUsage(value)).show_usage);
    } catch (e) {
      report("salvataggio dell'impostazione di utilizzo", e, { notify: true });
    }
  };

  const test = async () => {
    setTesting(true);
    setTestError(null);
    setTestLog(null);
    try {
      setTestLog(await api.testAgentNotify());
    } catch (e) {
      setTestError(errorMessage(e));
    } finally {
      setTesting(false);
    }
  };

  return (
    <Section>
      <p className="text-xs font-semibold">Per tutte le integrazioni</p>

      <div>
        <Button size="sm" variant="outline" disabled={testing} onClick={() => void test()}>
          {testing && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
          Prova notifica
        </Button>
        <p className="mt-1 text-[11px] text-muted-foreground">
          Invia un segnale di prova attraverso lo stesso canale degli hook.
        </p>
        {testLog && (
          <pre className="mt-2 max-h-32 overflow-auto whitespace-pre-wrap rounded-lg border bg-muted/50 p-2.5 font-mono text-[11px]">
            {testLog}
          </pre>
        )}
        {testError && <p className="mt-2 whitespace-pre-wrap text-xs text-destructive">{testError}</p>}
      </div>

      {usageApplies && (
        <div>
          <label className="flex items-center gap-2 text-[13px]">
            <input
              type="checkbox"
              checked={showUsage}
              onChange={(e) => void toggleUsage(e.target.checked)}
              className="h-3.5 w-3.5 accent-primary"
            />
            Mostra utilizzo (limiti 5h/settimanali e token per sessione)
          </label>
          {usageLimitsNotices(onlyEnabled(profiles, integrations)).map((notice) => (
            <p key={notice} className="mt-1 text-[11px] text-muted-foreground">
              {notice}
            </p>
          ))}
        </div>
      )}
    </Section>
  );
}
