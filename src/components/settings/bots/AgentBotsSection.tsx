import { useMemo, useState } from "react";
import { Plus } from "lucide-react";
import { reloadLaunchDefaults } from "../../../app/actions/settings";
import { useBotSkills } from "../../../hooks/useBotSkills";
import { useStore } from "../../../hooks/useStore";
import { secretaryNames } from "../../../lib/hubSecretaries";
import { api } from "../../../lib/tauri";
import type { AgentBotDraft } from "../../../lib/tauri";
import { configStore } from "../../../stores/config";
import { hubStore } from "../../../stores/hub";
import { Button } from "../../ui/button";
import { Section } from "../shared";
import AgentBotEditor from "./AgentBotEditor";
import AgentBotRow from "./AgentBotRow";
import { useBotOp } from "./useBotOp";

/** Bot in modifica: il suo id, `NEW` per uno nuovo, `null` nessuno (se ne apre uno solo alla volta). */
const NEW = Symbol("new-bot");
type Editing = string | typeof NEW | null;

/** Elenco dei Bot-agente con l'editor: crea, modifica, elimina, ripristina. */
export default function AgentBotsSection() {
  const bots = useStore(configStore, (s) => s.botSettings.agents);
  const integrations = useStore(configStore, (s) => s.integrations);
  const hubSettings = useStore(hubStore, (s) => s.settings);
  const { skills, error: skillsError } = useBotSkills();
  const { busy, error, setError, run } = useBotOp();
  const [editing, setEditing] = useState<Editing>(null);

  const secretaries = useMemo(
    () => (hubSettings === null ? [] : secretaryNames(hubSettings.secretaries)),
    [hubSettings],
  );
  const editedBot = typeof editing === "string" ? (bots.find((b) => b.id === editing) ?? null) : null;

  const open = (next: Editing) => {
    setError(null);
    setEditing(next);
  };

  const save = async (draft: AgentBotDraft): Promise<boolean> => {
    const saved = await run(() =>
      editedBot === null ? api.createAgentBot(draft) : api.updateAgentBot(editedBot.id, draft),
    );
    if (saved) setEditing(null);
    return saved;
  };

  const remove = async (id: string) => {
    if (!(await run(() => api.deleteAgentBot(id)))) return;
    setEditing(null);
    await reloadLaunchDefaults();
  };

  const editor = (
    <AgentBotEditor
      key={editedBot?.id ?? "new"}
      bot={editedBot}
      integrations={integrations}
      skills={skills}
      taken={{ bots: bots.filter((b) => b.id !== editedBot?.id).map((b) => b.name), secretaries }}
      busy={busy}
      error={error}
      onSave={save}
      onCancel={() => open(null)}
      onDelete={() => editedBot && void remove(editedBot.id)}
      onRestore={() => editedBot && void run(() => api.restoreAgentBotPreset(editedBot.id))}
      onEdited={() => setError(null)}
    />
  );

  return (
    <Section>
      <div>
        <h3 className="text-xs font-semibold">Bot</h3>
        <p className="mt-0.5 text-[11px] text-muted-foreground">
          Ogni Bot ha un compito, un agente e, se vuoi, un modello, un effort e una skill. Compaiono nel menu di avvio e
          nell’Ufficio.
        </p>
      </div>

      <ul className="flex flex-col gap-1.5">
        {bots.map((bot) => (
          <li key={bot.id} className="flex flex-col gap-1.5">
            <AgentBotRow
              bot={bot}
              integrations={integrations}
              skills={skills}
              editing={editing === bot.id}
              disabled={busy}
              onEdit={() => open(bot.id)}
            />
            {editing === bot.id && editor}
          </li>
        ))}
      </ul>

      {editing === NEW && editor}
      {skillsError && <p className="text-[11px] text-destructive">Skill non disponibili: {skillsError}</p>}
      {error !== null && editing === null && <p className="text-[11px] text-destructive">{error}</p>}

      <div className="flex flex-wrap items-center gap-2">
        <Button type="button" size="sm" disabled={busy || editing === NEW} onClick={() => open(NEW)}>
          <Plus className="!size-3.5" aria-hidden="true" /> Nuovo Bot
        </Button>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={busy}
          title="Ricrea i Bot predefiniti che hai eliminato"
          onClick={() => void run(() => api.restoreAgentBotPresets())}
        >
          Ripristina i predefiniti mancanti
        </Button>
      </div>
    </Section>
  );
}
