import { useEffect, useId, useState } from "react";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { api } from "../../lib/tauri";
import type { IntegrationInfo, Workspace } from "../../lib/tauri";
import { modelError, sanitizeModel } from "../../lib/model";
import { modelPlaceholder, modelSuggestions } from "../../lib/workerAgentChoices";
import { Field, Section } from "./shared";
import type { WorkspaceRun } from "./shared";

interface Props {
  workspace: Workspace;
  /** Integrazione di cui si imposta il modello. */
  integration: IntegrationInfo | undefined;
  agentId: string;
  /** Modello salvato per l'agente (`null` = quello dell'agente). */
  saved: string | null;
  run: WorkspaceRun;
  saving: boolean;
}

/** Modello dei worker di un agente: campo libero con i suggerimenti dichiarati dall'integrazione. */
export default function WorkerModelField({ workspace, integration, agentId, saved, run, saving }: Props) {
  const listId = useId();
  const [draft, setDraft] = useState(saved ?? "");
  useEffect(() => setDraft(saved ?? ""), [workspace.id, agentId, saved]);

  const error = modelError(draft);
  const suggestions = modelSuggestions(integration);

  return (
    <Section>
      <Field
        label="Modello dei worker"
        help="Modello dei worker automatici di questo workspace con questo agente. Vuoto = predefinito dell’agente. Un singolo task può indicare un modello diverso."
      >
        <div className="flex gap-2">
          <Input
            value={draft}
            list={suggestions.length > 0 ? listId : undefined}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur();
            }}
            placeholder={modelPlaceholder(integration)}
            aria-label="Modello dei worker"
            className="h-8 font-mono text-[12px]"
          />
          <Button
            size="sm"
            variant="secondary"
            disabled={saving || error !== null || sanitizeModel(draft) === saved}
            onClick={() => void run(() => api.setWorkerModel(workspace.id, agentId, sanitizeModel(draft)))}
          >
            Salva
          </Button>
        </div>
        {suggestions.length > 0 && (
          <datalist id={listId}>
            {suggestions.map((s) => (
              <option key={s.id} value={s.id}>
                {s.label}
              </option>
            ))}
          </datalist>
        )}
        {error && <p className="mt-1 text-[11px] text-destructive">{error}</p>}
      </Field>
    </Section>
  );
}
