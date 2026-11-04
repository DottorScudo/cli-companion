import { useState } from "react";
import { Plus } from "lucide-react";
import { deleteSecretary, saveLanguage } from "../../../app/actions/hubSettings";
import { useHubChannels } from "../../../hooks/useHubChannels";
import { errorMessage } from "../../../lib/errors";
import { LANGUAGES, SECRETARIES_MAX, secretariesSummary } from "../../../lib/hubSettings";
import type { HubLanguage, HubSettingsView } from "../../../lib/tauri";
import { Button } from "../../ui/button";
import { selectClass } from "../shared";
import HubAccordion from "./HubAccordion";
import SecretaryEditor from "./SecretaryEditor";
import SecretaryRow from "./SecretaryRow";

/** Secretario in modifica: il suo nome, `NEW` per uno nuovo, `null` nessuno (se ne apre uno solo alla volta). */
const NEW = Symbol("new-secretary");
type Editing = string | typeof NEW | null;

/** Accordion «Segretari»: lingua delle risposte, elenco ed editor. */
export default function SecretariesSection({ settings }: { settings: HubSettingsView }) {
  const { channels } = useHubChannels();
  const [editing, setEditing] = useState<Editing>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { secretaries } = settings;

  const run = async (op: () => Promise<void>) => {
    setError(null);
    setBusy(true);
    try {
      await op();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = (name: string) =>
    run(async () => {
      await deleteSecretary(name);
      setEditing((current) => (current === name ? null : current));
    });

  const editorFor = (name: string | null) => (
    <SecretaryEditor
      key={name ?? "new"}
      initial={name === null ? null : (secretaries.find((s) => s.name === name) ?? null)}
      taken={secretaries.filter((s) => s.name !== name).map((s) => s.name)}
      channels={channels}
      onSaved={(imageError) => {
        setEditing(null);
        setError(imageError === null ? null : `Segretario salvato, ma l’immagine no: ${imageError}`);
      }}
      onCancel={() => setEditing(null)}
    />
  );

  return (
    <HubAccordion
      title="Segretari"
      description="Nome, immagine, istruzioni, tool e canali. I loro nomi sono riservati e non vengono mai dati ai terminali. Si svegliano solo quando li menzioni con @nome."
      summary={secretariesSummary(settings)}
      defaultOpen
    >
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-md border bg-background/40 p-3">
        <label htmlFor="hub-language" className="flex flex-col">
          <span className="text-[13px] font-medium">Lingua delle risposte</span>
          <span className="text-[11px] text-muted-foreground">Tutti i segretari rispondono in questa lingua.</span>
        </label>
        <select
          id="hub-language"
          value={settings.language}
          disabled={busy}
          onChange={(e) => void run(() => saveLanguage(e.target.value as HubLanguage))}
          className={`${selectClass} h-8 w-auto min-w-[10rem]`}
        >
          {LANGUAGES.map((l) => (
            <option key={l.id} value={l.id}>
              {l.label}
            </option>
          ))}
        </select>
      </div>

      {secretaries.length === 0 && editing !== NEW && (
        <p className="text-xs text-muted-foreground">
          Nessun segretario. Creane uno e scrivigli con @nome in un thread.
        </p>
      )}
      <ul className="flex flex-col gap-2">
        {secretaries.map((s) =>
          editing === s.name ? (
            <li key={s.name}>{editorFor(s.name)}</li>
          ) : (
            <SecretaryRow
              key={s.name}
              secretary={s}
              channels={channels}
              disabled={busy}
              onEdit={() => setEditing(s.name)}
              onDelete={() => void remove(s.name)}
            />
          ),
        )}
      </ul>
      {editing === NEW && editorFor(null)}
      {error && <p className="text-[11px] text-destructive">{error}</p>}
      <div>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={busy || editing === NEW || secretaries.length >= SECRETARIES_MAX}
          onClick={() => setEditing(NEW)}
        >
          <Plus className="!size-3.5" aria-hidden="true" /> Nuovo segretario
        </Button>
      </div>
    </HubAccordion>
  );
}
