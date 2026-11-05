import { useState } from "react";
import { createRoute, updateRoute } from "../../../app/actions/connectors";
import { useHubChannels } from "../../../hooks/useHubChannels";
import { useStore } from "../../../hooks/useStore";
import { splitBackendError } from "../../../lib/connectorForm";
import type { FieldErrors, FieldValue } from "../../../lib/connectorForm";
import { LABEL_MAX_CHARS } from "../../../lib/connectors";
import {
  ROUTE_MODES,
  buildRouteInput,
  hasProblems,
  newRouteForm,
  routeFormOf,
  suggestRouteLabel,
  validateRouteForm,
  wakeForChannel,
  wakeOptions,
} from "../../../lib/connectorRoutes";
import type { RouteForm, RouteMode, RouteProblems } from "../../../lib/connectorRoutes";
import type { ConnectorDescriptor, ConnectorRoute, HubSecretary } from "../../../lib/tauri";
import { hubStore } from "../../../stores/hub";
import ChannelSelect from "../../hub/ChannelSelect";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { selectClass } from "../shared";
import FieldsForm from "./FieldsForm";
import ThreadPicker from "./ThreadPicker";
import { useOperation } from "./useOperation";

const NO_SECRETARIES: readonly HubSecretary[] = [];

/** Dove va a finire una regola quando è già deciso (si parte da un thread). */
export interface LockedTarget {
  channel: string;
  threadId: string;
}

interface Props {
  instanceId: string;
  descriptor: ConnectorDescriptor;
  /** La regola da modificare; `null` per crearne una. */
  route: ConnectorRoute | null;
  /** Se c'è, canale e thread non si scelgono: la regola porta lì. */
  locked?: LockedTarget;
  onDone: () => void;
  onCancel: () => void;
}

const WAKE_CAPABILITY = "wake_secretaries";

function withoutKey(errors: FieldErrors, key: string): FieldErrors {
  const { [key]: _cleared, ...rest } = errors;
  return rest;
}

/** Crea o modifica una regola di una fonte: filtro, destinazione, risveglio. Tutto dal descrittore. */
export default function RouteEditor({ instanceId, descriptor, route, locked, onDone, onCancel }: Props) {
  const { channels } = useHubChannels();
  const secretaries = useStore(hubStore, (s) => s.settings?.secretaries) ?? NO_SECRETARIES;
  const filterFields = descriptor.filter_fields;
  const canWake = descriptor.permissions.hub.includes(WAKE_CAPABILITY);

  const [form, setForm] = useState<RouteForm>(() => {
    if (route !== null) return routeFormOf(route, filterFields);
    const fresh = newRouteForm(filterFields, locked?.channel);
    return locked ? { ...fresh, threadId: locked.threadId } : fresh;
  });
  const [problems, setProblems] = useState<RouteProblems>({ filter: {}, thread: null });
  const { run, busy, error, setError } = useOperation();

  const patch = (changes: Partial<RouteForm>) => {
    setForm((current) => ({ ...current, ...changes }));
    setError(null);
  };
  const changeFilter = (key: string, value: FieldValue) => {
    setForm((current) => ({ ...current, filter: { ...current.filter, [key]: value } }));
    setProblems((current) => ({ ...current, filter: withoutKey(current.filter, key) }));
    setError(null);
  };
  const changeChannel = (channel: string) =>
    patch({ channel, threadId: "", wake: wakeForChannel(form.wake, secretaries, channel, canWake) });

  const submit = async () => {
    const found = validateRouteForm(form, filterFields);
    setProblems(found);
    if (hasProblems(found)) return;
    const input = buildRouteInput(form, filterFields, descriptor.label);
    const ok = await run(() => (route === null ? createRoute(instanceId, input) : updateRoute(route.id, input)));
    if (ok) onDone();
  };

  const remote = error === null ? null : splitBackendError(error, filterFields);
  const filterErrors: FieldErrors = remote?.field
    ? { ...problems.filter, [remote.field]: remote.text }
    : problems.filter;
  const generalError = remote !== null && remote.field === null ? remote.text : null;
  const options = wakeOptions(secretaries, form.channel, canWake, form.wake);
  const placeholder = suggestRouteLabel(filterFields, form.filter, descriptor.label);

  return (
    <form
      aria-label={route === null ? "Nuova regola" : "Modifica regola"}
      className="flex flex-col gap-3 rounded-md border bg-background/50 p-3"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div>
        <label htmlFor="route-label" className="mb-1 block text-xs font-semibold">
          Etichetta
        </label>
        <Input
          id="route-label"
          value={form.label}
          maxLength={LABEL_MAX_CHARS}
          placeholder={placeholder}
          disabled={busy}
          onChange={(e) => patch({ label: e.target.value })}
        />
      </div>

      <FieldsForm
        fields={filterFields}
        values={form.filter}
        errors={filterErrors}
        onChange={changeFilter}
        disabled={busy}
      />

      {locked === undefined && (
        <div className="flex flex-col gap-2">
          <p className="text-xs font-semibold">Destinazione</p>
          <ChannelSelect
            label="Canale di destinazione"
            channels={channels}
            value={form.channel}
            disabled={busy}
            onChange={changeChannel}
            className="h-9 text-sm"
          />
          <select
            aria-label="Thread di destinazione"
            value={form.mode}
            disabled={busy}
            onChange={(e) => patch({ mode: e.target.value as RouteMode })}
            className={selectClass}
          >
            {ROUTE_MODES.map((m) => (
              <option key={m.mode} value={m.mode}>
                {m.label}
              </option>
            ))}
          </select>
          <p className="text-[11px] text-muted-foreground">{ROUTE_MODES.find((m) => m.mode === form.mode)?.help}</p>
          {form.mode === "existing" && (
            <ThreadPicker
              label="Thread di destinazione"
              channel={form.channel}
              threadId={form.threadId}
              channelLocked
              disabled={busy}
              onChange={(_channel, threadId) => patch({ threadId })}
            />
          )}
          {problems.thread && (
            <p className="text-[11px] text-destructive" role="alert">
              {problems.thread}
            </p>
          )}
        </div>
      )}

      <div>
        <label htmlFor="route-wake" className="mb-1 block text-xs font-semibold">
          Risveglio
        </label>
        <select
          id="route-wake"
          value={form.wake}
          disabled={busy}
          onChange={(e) => patch({ wake: e.target.value })}
          className={selectClass}
        >
          {options.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
        {!canWake && (
          <p className="mt-1 text-[11px] text-muted-foreground">Questo connettore non può svegliare i segretari.</p>
        )}
      </div>

      <label className="flex items-center gap-2 text-[13px]">
        <input
          type="checkbox"
          checked={form.enabled}
          disabled={busy}
          onChange={(e) => patch({ enabled: e.target.checked })}
          className="h-3.5 w-3.5 accent-primary"
        />
        Regola abilitata
      </label>

      {generalError && (
        <p className="whitespace-pre-wrap text-xs text-destructive" role="alert">
          {generalError}
        </p>
      )}
      <div className="flex items-center gap-2">
        <Button type="submit" size="sm" disabled={busy}>
          {route === null ? "Aggiungi regola" : "Salva regola"}
        </Button>
        <Button type="button" size="sm" variant="secondary" disabled={busy} onClick={onCancel}>
          Annulla
        </Button>
      </div>
    </form>
  );
}
