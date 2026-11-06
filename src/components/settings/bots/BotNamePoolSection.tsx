import { useState } from "react";
import type { FormEvent } from "react";
import { api } from "../../../lib/tauri";
import { useStore } from "../../../hooks/useStore";
import { configStore } from "../../../stores/config";
import { BOT_NAME_MAX } from "../../../lib/bots";
import { cn } from "../../../lib/utils";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import BotImagePreview from "../BotImagePreview";
import BotNameRow from "../BotNameRow";
import { Field, Section, fieldGridClass } from "../shared";
import { BOT_IMAGE_ACCEPT, useBotImagePicker } from "../useBotImagePicker";
import { useBotOp } from "./useBotOp";

/** Nomi dei terminali generici: la pool da cui si pescano, con le immagini associate. */
export default function BotNamePoolSection() {
  const settings = useStore(configStore, (s) => s.botSettings);
  const [newName, setNewName] = useState("");
  const [confirmingReset, setConfirmingReset] = useState(false);
  const { busy, error, setError, run } = useBotOp();
  const picker = useBotImagePicker(setError);

  const removeName = (name: string) =>
    run(() =>
      api.setBotSettings(
        settings.enabled,
        settings.names.filter((n) => n !== name),
      ),
    );

  const removeImage = (name: string) => run(() => api.removeBotImage(name));

  const confirmImage = async () => {
    const { candidate } = picker;
    if (!candidate) return;
    const { name, image } = candidate;
    if (await run(() => api.setBotImage(name, image.png, image.color))) picker.cancel();
  };

  const addName = async (e: FormEvent) => {
    e.preventDefault();
    if (newName.trim() === "") return;
    const added = await run(() => api.setBotSettings(settings.enabled, [...settings.names, newName]));
    if (added) setNewName("");
  };

  const resetNames = async () => {
    setConfirmingReset(false);
    await run(() => api.resetBotNames());
  };

  return (
    <Section>
      <Field
        label="Nomi dei terminali generici"
        help="Ogni nuova sessione generica prende a caso un nome non usato dalle sessioni aperte. Finiti i nomi, si aggiunge un numero (“Bolt 2”, che usa l’immagine di “Bolt”). A ogni nome puoi associare un’immagine: diventa il corpo del bot (funzionano meglio i PNG con sfondo trasparente)."
      >
        <form onSubmit={(e) => void addName(e)} className="flex gap-2">
          <Input
            value={newName}
            maxLength={BOT_NAME_MAX}
            disabled={busy}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="Nuovo nome"
            aria-label="Nuovo nome"
            className="h-8"
          />
          <Button type="submit" size="sm" disabled={busy || newName.trim() === ""}>
            Aggiungi
          </Button>
        </form>
      </Field>
      {error && <p className="text-[11px] text-destructive">{error}</p>}
      <input
        ref={picker.inputRef}
        type="file"
        accept={BOT_IMAGE_ACCEPT}
        className="hidden"
        tabIndex={-1}
        aria-hidden="true"
        onChange={(e) => void picker.onFileChange(e)}
      />
      {picker.candidate && (
        <BotImagePreview
          name={picker.candidate.name}
          image={picker.candidate.image}
          busy={busy || picker.pixelating}
          onConfirm={() => void confirmImage()}
          onCancel={picker.cancel}
        />
      )}

      {settings.names.length === 0 ? (
        <p className="text-xs text-muted-foreground">Nessun nome: le sessioni si chiameranno “Bot 1”, “Bot 2”...</p>
      ) : (
        <ul className={cn(fieldGridClass, "gap-1.5")}>
          {settings.names.map((name) => (
            <BotNameRow
              key={name}
              name={name}
              image={settings.images[name]}
              disabled={busy || picker.pixelating}
              onPickImage={picker.pick}
              onRemoveImage={(n) => void removeImage(n)}
              onRemove={(n) => void removeName(n)}
            />
          ))}
        </ul>
      )}

      <div className="flex items-center gap-2">
        {confirmingReset ? (
          <>
            <span className="text-[11px] text-muted-foreground">Sostituire l’elenco con quello predefinito?</span>
            <Button size="sm" variant="destructive" disabled={busy} onClick={() => void resetNames()}>
              Ripristina
            </Button>
            <Button size="sm" variant="outline" onClick={() => setConfirmingReset(false)}>
              Annulla
            </Button>
          </>
        ) : (
          <Button size="sm" variant="outline" disabled={busy} onClick={() => setConfirmingReset(true)}>
            Ripristina nomi predefiniti
          </Button>
        )}
      </div>
    </Section>
  );
}
