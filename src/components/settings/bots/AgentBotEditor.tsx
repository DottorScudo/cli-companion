import { useEffect, useMemo, useState } from "react";
import type { FormEvent } from "react";
import {
  BOT_TASK_MAX,
  draftFromForm,
  emptyBotForm,
  fieldOfBackendError,
  formFromBot,
  isFormDirty,
  isGeneralBot,
  validateBotForm,
} from "../../../lib/botEditor";
import type { BotForm, BotFormErrors, TakenNames } from "../../../lib/botEditor";
import { BOT_NAME_MAX } from "../../../lib/bots";
import type { AgentBot, AgentBotDraft, BotSkillOption, IntegrationInfo } from "../../../lib/tauri";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import SecretaryImageField from "../hub/SecretaryImageField";
import { Field } from "../shared";
import BotAgentFields from "./BotAgentFields";
import BotKindsField from "./BotKindsField";
import ConfirmButton from "./ConfirmButton";

const NEW_BOT_FALLBACK_NAME = "Nuovo Bot";

interface Props {
  /** Il Bot in modifica, `null` per uno nuovo. */
  bot: AgentBot | null;
  integrations: readonly IntegrationInfo[];
  skills: readonly BotSkillOption[];
  taken: TakenNames;
  busy: boolean;
  /** Errore dell'ultima operazione del backend: va accanto al campo che riguarda. */
  error: string | null;
  /** Salva; `true` se è andata a buon fine (l'editor si chiude). */
  onSave: (draft: AgentBotDraft) => Promise<boolean>;
  onCancel: () => void;
  onDelete: () => void;
  onRestore: () => void;
  /** Il modulo è cambiato: l'errore del backend non vale più. */
  onEdited: () => void;
}

/** Campi di cui il modulo è stato toccato: gli errori si mostrano solo per questi, finché non si prova a salvare. */
function touchedFields(form: BotForm, saved: BotForm): Set<keyof BotFormErrors> {
  const touched = new Set<keyof BotFormErrors>();
  if (form.name !== saved.name) touched.add("name");
  if (form.task !== saved.task) touched.add("task");
  if (form.agent !== saved.agent || form.modelKind !== saved.modelKind || form.modelId !== saved.modelId) {
    touched.add("model");
  }
  return touched;
}

/** Editor di un Bot: compito, agente, modello, effort, skill, immagine e abilitazione. */
export default function AgentBotEditor({
  bot,
  integrations,
  skills,
  taken,
  busy,
  error,
  onSave,
  onCancel,
  onDelete,
  onRestore,
  onEdited,
}: Props) {
  const saved = useMemo(() => (bot === null ? emptyBotForm() : formFromBot(bot)), [bot]);
  const savedKey = JSON.stringify(saved);
  const [form, setForm] = useState(saved);
  const [attempted, setAttempted] = useState(false);
  const [imageError, setImageError] = useState<string | null>(null);

  // Il Bot salvato è cambiato davvero (ripristino, modifica altrove): il modulo riparte da lì.
  useEffect(() => setForm(JSON.parse(savedKey) as BotForm), [savedKey]);

  const general = bot !== null && isGeneralBot(bot);
  const errors = validateBotForm(form, taken);
  const touched = touchedFields(form, saved);
  const shown: BotFormErrors = Object.fromEntries(
    Object.entries(errors).filter(([field]) => attempted || touched.has(field as keyof BotFormErrors)),
  );
  const backendField = error === null ? null : fieldOfBackendError(error);
  if (error !== null && backendField !== null) shown[backendField] = error;
  const imageMessage = imageError ?? (backendField === "image" ? error : null);

  const change = (next: BotForm) => {
    setForm(next);
    onEdited();
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setAttempted(true);
    if (Object.keys(errors).length > 0) return;
    await onSave(draftFromForm(form));
  };

  return (
    <form
      onSubmit={(e) => void submit(e)}
      className="flex flex-col gap-3 rounded-md border bg-background/40 p-3"
      aria-label={bot === null ? "Nuovo Bot" : `Modifica ${bot.name}`}
    >
      <Field label="Nome">
        <Input
          aria-label="Nome del Bot"
          className="h-8"
          value={form.name}
          maxLength={BOT_NAME_MAX}
          disabled={busy}
          onChange={(e) => change({ ...form, name: e.target.value })}
        />
        {shown.name && <p className="mt-1 text-[11px] text-destructive">{shown.name}</p>}
      </Field>

      <Field
        label="Compito"
        help="In inglese, una riga: lo legge il modello che sceglie quale Bot usare per un lavoro."
      >
        <Input
          aria-label="Compito del Bot"
          className="h-8"
          value={form.task}
          maxLength={BOT_TASK_MAX}
          disabled={busy}
          onChange={(e) => change({ ...form, task: e.target.value })}
        />
        {shown.task && <p className="mt-1 text-[11px] text-destructive">{shown.task}</p>}
      </Field>

      <BotAgentFields
        form={form}
        integrations={integrations}
        skills={skills}
        errors={shown}
        disabled={busy}
        onChange={change}
      />

      <BotKindsField kinds={form.kinds} disabled={busy} onChange={(kinds) => change({ ...form, kinds })} />

      <SecretaryImageField
        name={form.name}
        fallbackName={NEW_BOT_FALLBACK_NAME}
        image={form.image}
        disabled={busy}
        onPick={(image) => change({ ...form, image })}
        onRemove={() => change({ ...form, image: null })}
        onError={setImageError}
      />
      {imageMessage && <p className="text-[11px] text-destructive">{imageMessage}</p>}

      {!general && (
        <label className="flex items-center gap-2 text-[13px]">
          <input
            type="checkbox"
            checked={form.enabled}
            disabled={busy}
            onChange={(e) => change({ ...form, enabled: e.target.checked })}
            className="h-3.5 w-3.5 accent-primary"
          />
          Attivo
        </label>
      )}

      {error !== null && backendField === null && <p className="text-[11px] text-destructive">{error}</p>}

      <div className="flex flex-wrap items-center gap-2">
        <Button type="submit" size="sm" disabled={busy || (bot !== null && !isFormDirty(form, saved))}>
          {bot === null ? "Crea Bot" : "Salva"}
        </Button>
        <Button type="button" size="sm" variant="outline" disabled={busy} onClick={onCancel}>
          Annulla
        </Button>
        {bot !== null && bot.preset !== null && (
          <ConfirmButton
            label="Ripristina predefinito"
            question="Compito, modello, effort, skill e tipi di task tornano quelli predefiniti; nome, agente e immagine restano."
            confirmLabel="Ripristina"
            disabled={busy}
            onConfirm={onRestore}
          />
        )}
        {bot !== null && !general && (
          <ConfirmButton
            label="Elimina"
            question={`Eliminare “${bot.name}”?`}
            confirmLabel="Elimina"
            destructive
            disabled={busy}
            onConfirm={onDelete}
          />
        )}
      </div>
    </form>
  );
}
