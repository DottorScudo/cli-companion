import { useState } from "react";
import type { FormEvent } from "react";
import { saveSecretary } from "../../../app/actions/hubSettings";
import type { ImageChange } from "../../../app/actions/hubSettings";
import { errorMessage } from "../../../lib/errors";
import type { HubChannel } from "../../../lib/hub";
import { SECRETARY_NAME_MAX } from "../../../lib/hubSecretaries";
import { EMPTY_DRAFT, SYSTEM_PROMPT_MAX, checkDraft, draftOf } from "../../../lib/hubSettings";
import type { SecretaryDraft } from "../../../lib/hubSettings";
import { draftOfPreset, presetNameError, presetOf, updatedPresetOf } from "../../../lib/hubPresets";
import type { HubSecretary, HubSecretaryPreset } from "../../../lib/tauri";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { Field } from "../shared";
import PresetRestore from "./PresetRestore";
import { ChannelsField, ToolsField } from "./SecretaryFields";
import SecretaryImageField from "./SecretaryImageField";

interface Props {
  /** Il segretario da modificare; `null` = nuovo. */
  initial: HubSecretary | null;
  /** Il modello da cui parte un segretario nuovo; `null` = da zero. */
  fromPreset: HubSecretaryPreset | null;
  /** I modelli noti, per il ripristino di un segretario che ne viene da uno. */
  presets: readonly HubSecretaryPreset[];
  onRestored: () => void;
  /** Nomi degli altri segretari, che questo non può prendere. */
  taken: readonly string[];
  channels: readonly HubChannel[];
  onSaved: (imageError: string | null) => void;
  onCancel: () => void;
}

/** Editor di un segretario: nome, immagine, system prompt, tool e canali. Salva tutto insieme. */
export default function SecretaryEditor({
  initial,
  fromPreset,
  presets,
  onRestored,
  taken,
  channels,
  onSaved,
  onCancel,
}: Props) {
  const [draft, setDraft] = useState<SecretaryDraft>(() => {
    if (initial !== null) return draftOf(initial);
    return fromPreset === null ? EMPTY_DRAFT : draftOfPreset(fromPreset);
  });
  const [imageChange, setImageChange] = useState<ImageChange>({ kind: "keep" });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(() =>
    initial === null && fromPreset !== null ? presetNameError(fromPreset, taken) : null,
  );
  const restorable = initial === null ? null : presetOf(initial, presets);
  const updated = initial !== null && updatedPresetOf(initial, presets) !== null;
  const patch = (changes: Partial<SecretaryDraft>) => setDraft((d) => ({ ...d, ...changes }));

  const shownImage =
    imageChange.kind === "set" ? imageChange.image : imageChange.kind === "remove" ? null : (initial?.image ?? null);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const checked = checkDraft(draft, taken);
    if (!checked.ok) {
      setError(checked.reason);
      return;
    }
    setError(null);
    setSaving(true);
    try {
      const { imageError } = await saveSecretary(
        initial?.name ?? null,
        checked.value,
        imageChange,
        fromPreset?.id ?? null,
      );
      onSaved(imageError);
    } catch (err) {
      setError(errorMessage(err));
      setSaving(false);
    }
  };

  return (
    <form
      onSubmit={(e) => void submit(e)}
      aria-label={
        initial === null
          ? fromPreset === null
            ? "Nuovo segretario"
            : `Nuovo segretario da modello ${fromPreset.label}`
          : `Modifica @${initial.name}`
      }
      className="flex flex-col gap-4 rounded-lg border border-primary/50 bg-background/40 p-3"
    >
      <Field label="Nome" help={`Si chiama con @nome. Minuscole, cifre e -, fino a ${SECRETARY_NAME_MAX} caratteri.`}>
        <div className="flex items-center gap-1.5">
          <span aria-hidden="true" className="text-muted-foreground">
            @
          </span>
          <Input
            value={draft.name}
            maxLength={SECRETARY_NAME_MAX}
            disabled={saving}
            onChange={(e) => patch({ name: e.target.value })}
            aria-label="Nome del segretario"
            autoFocus
            spellCheck={false}
            className="h-8 font-mono text-xs"
          />
        </div>
      </Field>
      <SecretaryImageField
        name={draft.name}
        image={shownImage}
        disabled={saving}
        onPick={(image) => setImageChange({ kind: "set", image })}
        onRemove={() => setImageChange({ kind: "remove" })}
        onError={setError}
      />
      <Field label="System prompt" help="In inglese: lo legge il modello. Vuoto = non configurato, non risponde.">
        <textarea
          value={draft.systemPrompt}
          rows={5}
          maxLength={SYSTEM_PROMPT_MAX}
          disabled={saving}
          onChange={(e) => patch({ systemPrompt: e.target.value })}
          aria-label="System prompt"
          spellCheck={false}
          className="w-full resize-y rounded-md border border-input bg-transparent px-3 py-2 text-xs leading-relaxed shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
        />
      </Field>
      {initial !== null && restorable !== null && (
        <PresetRestore
          name={initial.name}
          preset={restorable}
          updated={updated}
          disabled={saving}
          onRestored={onRestored}
          onError={setError}
        />
      )}
      <ToolsField draft={draft} disabled={saving} onChange={(tools) => patch({ tools })} />
      <ChannelsField draft={draft} channels={channels} disabled={saving} onChange={(c) => patch({ channels: c })} />
      <div className="flex items-center justify-end gap-2">
        {error && <p className="mr-auto min-w-0 text-[11px] text-destructive">{error}</p>}
        <Button type="button" size="sm" variant="outline" disabled={saving} onClick={onCancel}>
          Annulla
        </Button>
        <Button type="submit" size="sm" disabled={saving}>
          Salva
        </Button>
      </div>
    </form>
  );
}
