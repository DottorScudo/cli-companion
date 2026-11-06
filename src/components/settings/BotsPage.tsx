import { api } from "../../lib/tauri";
import { useStore } from "../../hooks/useStore";
import { configStore } from "../../stores/config";
import AgentBotsSection from "./bots/AgentBotsSection";
import BotNamePoolSection from "./bots/BotNamePoolSection";
import { useBotOp } from "./bots/useBotOp";
import { Section, SettingsPage } from "./shared";

/** Generale › Bot: i Bot-agente (compito, agente, modello, effort, skill) e la pool dei nomi dei terminali generici. */
export default function BotsPage() {
  const settings = useStore(configStore, (s) => s.botSettings);
  const { busy, error, run } = useBotOp();

  return (
    <SettingsPage
      title="Bot"
      description="Ogni sessione è un bot con un nome e un piccolo avatar, per riconoscerla a colpo d’occhio."
    >
      <Section>
        <label className="flex items-center gap-2 text-[13px]">
          <input
            type="checkbox"
            checked={settings.enabled}
            disabled={busy}
            onChange={(e) => void run(() => api.setBotSettings(e.target.checked, settings.names))}
            className="h-3.5 w-3.5 accent-primary"
          />
          Mostra le sessioni come bot
        </label>
        <p className="-mt-1 text-[11px] text-muted-foreground">
          I nomi vengono assegnati comunque: riaccendendo la funzione compaiono subito.
        </p>
        {error && <p className="text-[11px] text-destructive">{error}</p>}
      </Section>

      <AgentBotsSection />
      <BotNamePoolSection />
    </SettingsPage>
  );
}
