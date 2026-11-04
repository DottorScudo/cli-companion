import { useState } from "react";
import type { FormEvent } from "react";
import { saveSession } from "../../../app/actions/hubSettings";
import { errorMessage } from "../../../lib/errors";
import { IDLE_MINUTES_RANGE, MAX_STEPS_RANGE, checkSession, sessionSummary } from "../../../lib/hubSettings";
import type { HubSettingsView } from "../../../lib/tauri";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { Field } from "../shared";
import HubAccordion from "./HubAccordion";

/** Accordion «Sessioni dei segretari»: quando tornano a dormire e quanti passi fanno a ogni risveglio. */
export default function SessionSection({ settings }: { settings: HubSettingsView }) {
  const { idle_minutes, max_steps } = settings.session;
  const [idle, setIdle] = useState(String(idle_minutes));
  const [steps, setSteps] = useState(String(max_steps));
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dirty = idle !== String(idle_minutes) || steps !== String(max_steps);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const checked = checkSession(idle, steps);
    if (!checked.ok) {
      setError(checked.reason);
      return;
    }
    setError(null);
    setSaving(true);
    try {
      await saveSession(checked.value.idleMinutes, checked.value.maxSteps);
      setIdle(String(checked.value.idleMinutes));
      setSteps(String(checked.value.maxSteps));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <HubAccordion
      title="Sessioni dei segretari"
      description="Un segretario menzionato resta sveglio nel thread finché gli scrivi; dopo l’inattività riassume il thread e torna a dormire."
      summary={sessionSummary(settings.session)}
    >
      <form onSubmit={(e) => void submit(e)} className="flex flex-col gap-3">
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <Field
            label="Torna a dormire dopo (minuti)"
            help={`Da ${IDLE_MINUTES_RANGE.min} a ${IDLE_MINUTES_RANGE.max} minuti di inattività.`}
          >
            <Input
              type="number"
              inputMode="numeric"
              min={IDLE_MINUTES_RANGE.min}
              max={IDLE_MINUTES_RANGE.max}
              value={idle}
              disabled={saving}
              onChange={(e) => setIdle(e.target.value)}
              aria-label="Minuti di inattività"
              className="h-8"
            />
          </Field>
          <Field
            label="Passi massimi per risveglio"
            help={`Da ${MAX_STEPS_RANGE.min} a ${MAX_STEPS_RANGE.max}: i giri tra modello e tool per una risposta.`}
          >
            <Input
              type="number"
              inputMode="numeric"
              min={MAX_STEPS_RANGE.min}
              max={MAX_STEPS_RANGE.max}
              value={steps}
              disabled={saving}
              onChange={(e) => setSteps(e.target.value)}
              aria-label="Passi massimi"
              className="h-8"
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
    </HubAccordion>
  );
}
