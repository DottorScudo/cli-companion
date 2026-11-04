import { useState } from "react";
import type { FormEvent } from "react";
import { saveProvider } from "../../../app/actions/hubSettings";
import { errorMessage } from "../../../lib/errors";
import { BASE_URL_MAX, MODEL_MAX, checkProvider, modelSummary } from "../../../lib/hubSettings";
import type { HubSettingsView } from "../../../lib/tauri";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { Field, fieldGridClass } from "../shared";
import ApiKeyField from "./ApiKeyField";
import HubAccordion from "./HubAccordion";

/** Accordion «Modello»: endpoint compatibile OpenAI, nome del modello e API key. */
export default function ModelSection({ settings }: { settings: HubSettingsView }) {
  const [baseUrl, setBaseUrl] = useState(settings.provider.base_url);
  const [model, setModel] = useState(settings.provider.model);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dirty = baseUrl !== settings.provider.base_url || model !== settings.provider.model;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const checked = checkProvider(baseUrl, model);
    if (!checked.ok) {
      setError(checked.reason);
      return;
    }
    setError(null);
    setSaving(true);
    try {
      await saveProvider(checked.value.baseUrl, checked.value.model);
      setBaseUrl(checked.value.baseUrl);
      setModel(checked.value.model);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <HubAccordion
      title="Modello"
      description="Un solo endpoint compatibile OpenAI per tutti i segretari: llama-server, Ollama o un servizio remoto."
      summary={modelSummary(settings)}
      defaultOpen
    >
      <form onSubmit={(e) => void submit(e)} className="flex flex-col gap-3">
        <div className={fieldGridClass}>
          <Field label="Endpoint" help="Indirizzo base dell’API, per esempio http://127.0.0.1:8080/v1.">
            <Input
              value={baseUrl}
              maxLength={BASE_URL_MAX}
              disabled={saving}
              onChange={(e) => setBaseUrl(e.target.value)}
              placeholder="http://127.0.0.1:8080/v1"
              aria-label="Endpoint"
              spellCheck={false}
              className="h-8 font-mono text-xs"
            />
          </Field>
          <Field label="Modello" help="Il nome che l’endpoint si aspetta.">
            <Input
              value={model}
              maxLength={MODEL_MAX}
              disabled={saving}
              onChange={(e) => setModel(e.target.value)}
              placeholder="nome del modello"
              aria-label="Modello"
              spellCheck={false}
              className="h-8 font-mono text-xs"
            />
          </Field>
        </div>
        <div className="flex items-center gap-3">
          <Button type="submit" size="sm" disabled={saving || !dirty}>
            Salva
          </Button>
          {error && <p className="min-w-0 text-[11px] text-destructive">{error}</p>}
        </div>
      </form>
      <ApiKeyField isSet={settings.api_key_set} />
    </HubAccordion>
  );
}
