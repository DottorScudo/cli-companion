import { useId } from "react";
import {
  EFFORT_LABELS,
  FOLLOW_WORKSPACE,
  MODEL_KIND_LABELS,
  agentChoices,
  effortChoices,
  formAfterAgentChange,
  modelSuggestionsOf,
  skillValue,
} from "../../../lib/botEditor";
import type { BotFormErrors, BotForm, ModelKind } from "../../../lib/botEditor";
import { MODEL_MAX } from "../../../lib/model";
import type { Effort } from "../../../bindings/Effort";
import type { BotSkillOption, IntegrationInfo } from "../../../lib/tauri";
import { Input } from "../../ui/input";
import { Field, fieldGridClass, selectClass } from "../shared";

interface Props {
  form: BotForm;
  integrations: readonly IntegrationInfo[];
  skills: readonly BotSkillOption[];
  /** Errori, già filtrati su quelli da mostrare. */
  errors: BotFormErrors;
  disabled: boolean;
  onChange: (form: BotForm) => void;
}

const NO_EFFORT_LABEL = "Predefinito";
const NO_SKILL_LABEL = "Nessuna";
const MISSING_SKILL_SUFFIX = "(non trovata)";
const SKILL_GROUPS = [
  { kind: "plugin", label: "Skill del plugin" },
  { kind: "user", label: "Le tue skill" },
] as const;

const FieldError = ({ message }: { message?: string }) =>
  message ? <p className="mt-1 text-[11px] text-destructive">{message}</p> : null;

/** Agente, modello, effort e skill di un Bot: i controlli che dipendono dalle integrazioni. */
export default function BotAgentFields({ form, integrations, skills, errors, disabled, onChange }: Props) {
  const modelListId = useId();
  const efforts = effortChoices(integrations, form.agent);
  const suggestions = modelSuggestionsOf(integrations, form.agent);
  const hasUnknownSkill = form.skill !== "" && !skills.some((s) => skillValue(s.skill) === form.skill);
  const skillsOfKind = (kind: "plugin" | "user") => skills.filter((s) => s.skill.kind === kind);

  return (
    <div className={fieldGridClass}>
      <Field label="Agente" help="Chi esegue il Bot. «Segue il workspace»: l’agente del workspace in cui parte.">
        <select
          aria-label="Agente"
          className={selectClass}
          value={form.agent}
          disabled={disabled}
          onChange={(e) => onChange(formAfterAgentChange(integrations, form, e.target.value))}
        >
          {agentChoices(integrations, form.agent).map((c) => (
            <option key={c.id} value={c.id}>
              {c.label}
            </option>
          ))}
        </select>
        <FieldError message={errors.agent} />
      </Field>

      <Field
        label="Modello"
        help="«Standard» e «Forte» scelgono la fascia; un modello preciso richiede l’agente fissato."
      >
        <select
          aria-label="Modello"
          className={selectClass}
          value={form.modelKind}
          disabled={disabled}
          onChange={(e) => onChange({ ...form, modelKind: e.target.value as ModelKind, modelId: "" })}
        >
          {(Object.keys(MODEL_KIND_LABELS) as ModelKind[]).map((kind) => (
            <option key={kind} value={kind} disabled={kind === "id" && form.agent === FOLLOW_WORKSPACE}>
              {MODEL_KIND_LABELS[kind]}
            </option>
          ))}
        </select>
        {form.modelKind === "id" && (
          <>
            <Input
              aria-label="Nome del modello"
              className="mt-1.5 h-8"
              value={form.modelId}
              maxLength={MODEL_MAX}
              list={suggestions.length > 0 ? modelListId : undefined}
              disabled={disabled}
              placeholder="Nome del modello"
              onChange={(e) => onChange({ ...form, modelId: e.target.value })}
            />
            <datalist id={modelListId}>
              {suggestions.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.label}
                </option>
              ))}
            </datalist>
          </>
        )}
        <FieldError message={errors.model} />
      </Field>

      {efforts.show && (
        <Field label="Effort" help={efforts.note ?? "Quanto ragiona il modello prima di rispondere."}>
          <select
            aria-label="Effort"
            className={selectClass}
            value={form.effort}
            disabled={disabled}
            onChange={(e) => onChange({ ...form, effort: e.target.value as Effort | "" })}
          >
            <option value="">{NO_EFFORT_LABEL}</option>
            {efforts.levels.map((level) => (
              <option key={level} value={level}>
                {EFFORT_LABELS[level]}
              </option>
            ))}
          </select>
          <FieldError message={errors.effort} />
        </Field>
      )}

      <Field label="Skill" help="Una skill caricata all’avvio del Bot: del plugin o tua.">
        <select
          aria-label="Skill"
          className={selectClass}
          value={form.skill}
          disabled={disabled}
          onChange={(e) => onChange({ ...form, skill: e.target.value })}
        >
          <option value="">{NO_SKILL_LABEL}</option>
          {SKILL_GROUPS.filter(({ kind }) => skillsOfKind(kind).length > 0).map(({ kind, label }) => (
            <optgroup key={kind} label={label}>
              {skillsOfKind(kind).map((s) => (
                <option key={skillValue(s.skill)} value={skillValue(s.skill)} title={s.description}>
                  {s.name}
                </option>
              ))}
            </optgroup>
          ))}
          {hasUnknownSkill && (
            <option value={form.skill}>
              {form.skill.slice(form.skill.indexOf(":") + 1)} {MISSING_SKILL_SUFFIX}
            </option>
          )}
        </select>
        <FieldError message={errors.skill} />
      </Field>
    </div>
  );
}
