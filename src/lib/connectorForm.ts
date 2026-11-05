// Form generico dei connettori: i valori si costruiscono dai campi del
// descrittore (configurazione dell'istanza o filtro di una regola), si
// controllano in locale al minimo (la validazione vera è del backend) e si
// riportano in un oggetto JSON. I campi `secret` non entrano mai nei valori: sono
// in sola scrittura. Funzioni pure, importabili dall'app e dai test.

import type { ConnectorConfigField } from "../bindings/ConnectorConfigField.ts";

/** Un riferimento a un thread dell'HUB, come lo vuole il backend per un campo `thread`. */
export interface ThreadRef {
  channel: string;
  thread_id: string;
}

/** Testi e scelte sono stringhe (un `number` resta testo finché non si salva), `bool` è booleano, `thread` un riferimento. */
export type FieldValue = string | boolean | ThreadRef | null;

/** I valori del form per chiave di campo. */
export type FormValues = Record<string, FieldValue>;

/** Errori per chiave di campo. */
export type FieldErrors = Record<string, string>;

const FIELD_ERROR = /^«([a-z0-9_]+)»:\s*(.+?)\.?$/;
const BACKEND_PREFIX = /^(?:Configurazione|Filtro) non valid[ao]:\s*/;

/** I campi che l'utente compila come valori (tutti tranne i segreti). */
export function valueFields(fields: readonly ConnectorConfigField[]): ConnectorConfigField[] {
  return fields.filter((f) => f.kind.kind !== "secret");
}

/** I campi `secret`, in sola scrittura. */
export function secretFields(fields: readonly ConnectorConfigField[]): ConnectorConfigField[] {
  return fields.filter((f) => f.kind.kind === "secret");
}

function isThreadRef(value: unknown): value is ThreadRef {
  if (typeof value !== "object" || value === null) return false;
  const ref = value as Record<string, unknown>;
  return typeof ref.channel === "string" && typeof ref.thread_id === "string";
}

function initialValue(field: ConnectorConfigField, saved: unknown): FieldValue {
  const kind = field.kind;
  switch (kind.kind) {
    case "bool":
      return typeof saved === "boolean" ? saved : kind.default;
    case "number":
      return typeof saved === "number" ? String(saved) : "";
    case "thread":
      return isThreadRef(saved) ? { channel: saved.channel, thread_id: saved.thread_id } : null;
    default:
      return typeof saved === "string" ? saved : "";
  }
}

/** I valori iniziali del form: quelli salvati in `config` (o nel filtro), altrimenti vuoti o il predefinito del campo. */
export function initialValues(fields: readonly ConnectorConfigField[], saved: Record<string, unknown>): FormValues {
  return Object.fromEntries(valueFields(fields).map((f) => [f.key, initialValue(f, saved[f.key])]));
}

function isBlank(value: FieldValue | undefined): boolean {
  if (value === undefined || value === null) return true;
  if (typeof value === "object") return value.thread_id === "";
  return typeof value === "string" && value.trim() === "";
}

/** Un campo non obbligatorio lasciato vuoto non entra nell'oggetto; un `bool` c'è sempre. */
function serialize(field: ConnectorConfigField, value: FieldValue | undefined): unknown {
  if (field.kind.kind === "bool") return value === true;
  if (isBlank(value)) return undefined;
  if (field.kind.kind === "number") return Number(String(value).trim());
  return typeof value === "string" ? value.trim() : value;
}

/** L'oggetto da mandare al backend (configurazione o filtro): solo i campi compilati. */
export function toPayload(fields: readonly ConnectorConfigField[], values: FormValues): Record<string, unknown> {
  const payload: Record<string, unknown> = {};
  for (const field of valueFields(fields)) {
    const serialized = serialize(field, values[field.key]);
    if (serialized !== undefined) payload[field.key] = serialized;
  }
  return payload;
}

function problem(field: ConnectorConfigField, value: FieldValue | undefined): string | null {
  const kind = field.kind;
  if (isBlank(value)) return field.required && kind.kind !== "bool" ? "Obbligatorio." : null;
  const text = typeof value === "string" ? value.trim() : "";
  if (kind.kind === "text" && Array.from(text).length > kind.max_len) return `Al massimo ${kind.max_len} caratteri.`;
  if (kind.kind === "number") {
    const n = Number(text);
    if (!Number.isInteger(n)) return "Deve essere un numero intero.";
    if (n < kind.min || n > kind.max) return `Da ${kind.min} a ${kind.max}.`;
  }
  if (kind.kind === "select" && !kind.options.some((o) => o.value === text)) return "Scegli una delle voci.";
  return null;
}

/** Il controllo locale minimo (obbligatori, lunghezza, intervallo): il resto lo dice il backend. */
export function validateValues(fields: readonly ConnectorConfigField[], values: FormValues): FieldErrors {
  const errors: FieldErrors = {};
  for (const field of valueFields(fields)) {
    const message = problem(field, values[field.key]);
    if (message !== null) errors[field.key] = message;
  }
  return errors;
}

/**
 * L'errore del backend («Configurazione non valida: «chiave»: motivo.») riportato al campo.
 * Un messaggio che non nomina un campo noto resta generale (`field` nullo).
 */
export function splitBackendError(
  message: string,
  fields: readonly ConnectorConfigField[],
): { field: string | null; text: string } {
  const body = message.trim().replace(BACKEND_PREFIX, "");
  const match = FIELD_ERROR.exec(body);
  if (match && fields.some((f) => f.key === match[1])) {
    const reason = match[2];
    return { field: match[1], text: `${reason.charAt(0).toUpperCase()}${reason.slice(1)}.` };
  }
  return { field: null, text: message.trim() };
}

/** Per l'interfaccia: il testo d'aiuto di un campo `number` («Da 1 a 10»). */
export function numberRangeHint(field: ConnectorConfigField): string | null {
  return field.kind.kind === "number" ? `Da ${field.kind.min} a ${field.kind.max}` : null;
}
