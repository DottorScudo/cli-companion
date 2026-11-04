import { useState } from "react";
import type { FormEvent } from "react";
import { KeyRound } from "lucide-react";
import { clearApiKey, saveApiKey } from "../../../app/actions/hubSettings";
import { errorMessage } from "../../../lib/errors";
import { API_KEY_MAX, checkApiKey } from "../../../lib/hubSettings";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";

const FIELD_NAME = "hub-api-key";

interface Props {
  /** Il backend ha una API key nel portachiavi (la chiave stessa non arriva mai qui). */
  isSet: boolean;
}

/**
 * Stato della API key con Imposta/Cambia/Rimuovi. Il testo scritto vive solo
 * nel campo (non controllato) finché non si invia: poi il campo si svuota,
 * anche se il backend rifiuta. Nessuno stato React lo contiene.
 */
export default function ApiKeyField({ isSet }: Props) {
  const [editing, setEditing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async (op: () => Promise<void>): Promise<boolean> => {
    setError(null);
    setBusy(true);
    try {
      await op();
      return true;
    } catch (e) {
      setError(errorMessage(e));
      return false;
    } finally {
      setBusy(false);
    }
  };

  const submit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const field = e.currentTarget.elements.namedItem(FIELD_NAME) as HTMLInputElement;
    const checked = checkApiKey(field.value);
    field.value = "";
    if (!checked.ok) {
      setError(checked.reason);
      return;
    }
    if (await run(() => saveApiKey(checked.value))) setEditing(false);
  };

  const cancel = () => {
    setError(null);
    setEditing(false);
  };

  return (
    <div className="flex flex-col gap-2 rounded-md border bg-background/40 p-3">
      <div className="flex flex-wrap items-center gap-3">
        <KeyRound
          aria-hidden="true"
          className={isSet ? "!size-4 text-status-finished" : "!size-4 text-muted-foreground"}
        />
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="text-[13px] font-medium">{isSet ? "API key impostata" : "API key non impostata"}</span>
          <span className="text-[11px] text-muted-foreground">
            Salvata nel portachiavi del sistema, mai nei file dell’app e mai mostrata.
          </span>
        </div>
        {!editing && (
          <div className="flex gap-2">
            <Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => setEditing(true)}>
              {isSet ? "Cambia" : "Imposta"}
            </Button>
            {isSet && (
              <Button
                type="button"
                size="sm"
                variant="outline"
                className="text-destructive"
                disabled={busy}
                onClick={() => void run(clearApiKey)}
              >
                Rimuovi
              </Button>
            )}
          </div>
        )}
      </div>
      {editing && (
        <form onSubmit={(e) => void submit(e)} className="flex gap-2">
          <Input
            name={FIELD_NAME}
            type="password"
            autoComplete="off"
            spellCheck={false}
            autoFocus
            maxLength={API_KEY_MAX}
            disabled={busy}
            placeholder="Incolla la API key"
            aria-label="API key"
            className="h-8"
          />
          <Button type="submit" size="sm" disabled={busy}>
            Salva
          </Button>
          <Button type="button" size="sm" variant="outline" disabled={busy} onClick={cancel}>
            Annulla
          </Button>
        </form>
      )}
      {error && <p className="text-[11px] text-destructive">{error}</p>}
    </div>
  );
}
