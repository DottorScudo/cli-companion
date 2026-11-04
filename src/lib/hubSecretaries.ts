// Segretari dell'Agents HUB (docs/HUB.MD): chi è menzionabile, menzioni nel
// testo, autocompletamento di `@` e stato (sveglio, al lavoro, dorme). Funzioni
// pure a zero dipendenze, importabili sia dall'app sia dai test `node --test`.

import type { HubSecretary } from "../bindings/HubSecretary.ts";
import type { HubSecretaryStatus } from "../bindings/HubSecretaryStatus.ts";

const MS_PER_MINUTE = 60_000;

// ---------------------------------------------------------------- elenco

/** Il segretario ha un system prompt: senza, esiste ma non risponde alle menzioni. */
export function isConfigured(secretary: HubSecretary): boolean {
  return secretary.system_prompt.trim() !== "";
}

/** Nomi dei segretari (gli handle), per escluderli dai nomi delle sessioni dei terminali. */
export function secretaryNames(secretaries: readonly HubSecretary[]): string[] {
  return secretaries.map((s) => s.name);
}

/** Il segretario con quel nome, senza badare alle maiuscole. */
export function findSecretary(secretaries: readonly HubSecretary[], name: string): HubSecretary | undefined {
  const key = name.toLowerCase();
  return secretaries.find((s) => s.name.toLowerCase() === key);
}

/** Il segretario può essere menzionato nel canale. */
export function allowsChannel(secretary: HubSecretary, channel: string): boolean {
  return secretary.channels.mode === "all" || secretary.channels.channels.includes(channel);
}

/** Segretari che rispondono a una menzione nel canale: ammessi e configurati. */
export function mentionable(secretaries: readonly HubSecretary[], channel: string): HubSecretary[] {
  return secretaries.filter((s) => isConfigured(s) && allowsChannel(s, channel));
}

/** Prima riga non vuota del system prompt, per l'elenco. */
export function promptFirstLine(prompt: string): string {
  return (
    prompt
      .split(/\r?\n/)
      .map((l) => l.trim())
      .find((l) => l !== "") ?? ""
  );
}

// ---------------------------------------------------------------- menzioni

/** Lunghezza massima del nome di un segretario (come il backend, `settings/secretary.rs`). */
export const SECRETARY_NAME_MAX = 24;

const isNameChar = (c: string): boolean => /^[A-Za-z0-9-]$/.test(c);
const isWordChar = (c: string): boolean => /^[\p{L}\p{N}_]$/u.test(c);

/** Un `@` dopo questi caratteri fa parte di un'altra parola (un indirizzo email, `@@mario`). */
const continuesToken = (prev: string): boolean => /^[\p{L}\p{N}\-_.@]$/u.test(prev);

export interface MentionQuery {
  /** Posizione del `@` nel testo. */
  start: number;
  /** Ciò che è stato scritto dopo il `@`, fino al cursore. */
  query: string;
}

/** La menzione che si sta scrivendo con il cursore a `caret`, se c'è. */
export function mentionQuery(text: string, caret: number): MentionQuery | null {
  let i = caret;
  while (i > 0 && isNameChar(text[i - 1])) i--;
  if (i === 0 || text[i - 1] !== "@") return null;
  const start = i - 1;
  if (start > 0 && continuesToken(text[start - 1])) return null;
  return { start, query: text.slice(i, caret) };
}

/** Segretari il cui nome inizia con `query`, in ordine alfabetico. */
export function filterMentions(candidates: readonly HubSecretary[], query: string): HubSecretary[] {
  const key = query.toLowerCase();
  return candidates.filter((s) => s.name.startsWith(key)).sort((a, b) => a.name.localeCompare(b.name));
}

export interface MentionCompletion {
  text: string;
  caret: number;
}

/** Sostituisce la menzione in corso con `@nome` e uno spazio (se non c'è già), portando il cursore dopo. */
export function completeMention(text: string, mention: MentionQuery, caret: number, name: string): MentionCompletion {
  const after = text.slice(caret);
  const gap = after.startsWith(" ") ? "" : " ";
  const head = `${text.slice(0, mention.start)}@${name}${gap}`;
  return { text: head + after, caret: head.length + (gap === "" ? 1 : 0) };
}

/** Voce attiva del menu dopo uno spostamento, con giro dalla fine all'inizio; `-1` se il menu è vuoto. */
export function moveActive(index: number, delta: number, count: number): number {
  if (count <= 0) return -1;
  return (((index + delta) % count) + count) % count;
}

export interface TextSegment {
  text: string;
  /** Menzione di un segretario noto: da evidenziare. */
  mention: boolean;
}

/** Il testo diviso in tratti normali e menzioni di segretari noti (stessa grammatica del backend, `engine/mentions.rs`). */
export function splitMentions(text: string, names: readonly string[]): TextSegment[] {
  const known = new Set(names.map((n) => n.toLowerCase()));
  const segments: TextSegment[] = [];
  let plainFrom = 0;
  const chars = [...text];
  let offset = 0; // indice UTF-16 di chars[i]
  for (let i = 0; i < chars.length; offset += chars[i].length, i++) {
    if (chars[i] !== "@" || (i > 0 && continuesToken(chars[i - 1]))) continue;
    let end = i + 1;
    while (end < chars.length && isNameChar(chars[end])) end++;
    if (end < chars.length && isWordChar(chars[end])) continue;
    let tokenEnd = end;
    while (tokenEnd > i + 1 && chars[tokenEnd - 1] === "-") tokenEnd--;
    const token = chars.slice(i + 1, tokenEnd).join("");
    if (!known.has(token.toLowerCase())) continue;
    const mentionText = chars.slice(i, tokenEnd).join("");
    if (offset > plainFrom) segments.push({ text: text.slice(plainFrom, offset), mention: false });
    segments.push({ text: mentionText, mention: true });
    plainFrom = offset + mentionText.length;
  }
  if (plainFrom < text.length) segments.push({ text: text.slice(plainFrom), mention: false });
  return segments;
}

// ---------------------------------------------------------------- stato

/** Stato di un segretario per la colonna di sinistra. */
export type SecretaryActivity =
  { kind: "unconfigured" } | { kind: "asleep" } | { kind: "awake"; expiresAt: number; working: boolean };

/** Dorme, è sveglio in almeno un thread (scade quando scade l'ultima sessione) o non è configurato. */
export function secretaryActivity(secretary: HubSecretary, statuses: readonly HubSecretaryStatus[]): SecretaryActivity {
  if (!isConfigured(secretary)) return { kind: "unconfigured" };
  const mine = statuses.filter((s) => s.secretary === secretary.name);
  if (mine.length === 0) return { kind: "asleep" };
  return {
    kind: "awake",
    expiresAt: Math.max(...mine.map((s) => s.expires_at)),
    working: mine.some((s) => s.working),
  };
}

/** Minuti che restano, arrotondati per eccesso (almeno 1 finché la sessione non è scaduta); 0 = scaduta. */
export function minutesLeft(expiresAt: number, now: number): number {
  const ms = expiresAt - now;
  return ms <= 0 ? 0 : Math.max(1, Math.ceil(ms / MS_PER_MINUTE));
}

/** «scade tra 12 min», «scade a momenti». */
export function expiryText(expiresAt: number, now: number): string {
  const min = minutesLeft(expiresAt, now);
  return min === 0 ? "scade a momenti" : `scade tra ${min} min`;
}

/** Testo di stato della colonna di sinistra. */
export function activityLabel(activity: SecretaryActivity, now: number): string {
  switch (activity.kind) {
    case "unconfigured":
      return "non configurato";
    case "asleep":
      return "dorme";
    case "awake":
      return activity.working ? "al lavoro" : `sveglio · ${expiryText(activity.expiresAt, now)}`;
  }
}

/** Sessioni dei segretari sul thread. */
export function threadStatuses(
  statuses: readonly HubSecretaryStatus[],
  channel: string,
  threadId: string,
): HubSecretaryStatus[] {
  return statuses.filter((s) => s.channel === channel && s.thread_id === threadId);
}

/** Il titolare del thread: la sessione che scade per ultima, cioè quella con l'attività più recente. */
export function threadOwner(
  statuses: readonly HubSecretaryStatus[],
  channel: string,
  threadId: string,
): HubSecretaryStatus | null {
  let owner: HubSecretaryStatus | null = null;
  for (const s of threadStatuses(statuses, channel, threadId)) {
    if (owner === null || s.expires_at > owner.expires_at) owner = s;
  }
  return owner;
}

/** Nomi dei segretari che stanno lavorando sul thread. */
export function typingNames(statuses: readonly HubSecretaryStatus[], channel: string, threadId: string): string[] {
  return threadStatuses(statuses, channel, threadId)
    .filter((s) => s.working)
    .map((s) => s.secretary);
}

/** «mario sta scrivendo…», «mario e luisa stanno scrivendo…»; vuoto se nessuno lavora. */
export function typingLabel(names: readonly string[]): string {
  if (names.length === 0) return "";
  if (names.length === 1) return `${names[0]} sta scrivendo…`;
  return `${names.slice(0, -1).join(", ")} e ${names[names.length - 1]} stanno scrivendo…`;
}
