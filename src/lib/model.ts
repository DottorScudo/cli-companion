// Nome del modello di un worker o di un task: normalizzazione e validazione.
// Funzioni pure a zero dipendenze, importabili sia dall'app sia dai test.

/** Lunghezza massima del nome di un modello (come `MODEL_SHAPE_MAX` del backend). */
export const MODEL_MAX = 128;

/**
 * Come `valid_model_shape` (`MODEL_SHAPE_PATTERN`) del backend: 1-128 caratteri tra
 * lettere, cifre, '.', '_', '-', '/', ':'; niente '-' o '/' iniziale, '/' finale o '//'.
 * Solo la barriera generica: la regola del singolo agente la applica il backend.
 */
const MODEL_RE = /^(?![-/])(?!.*\/\/)[A-Za-z0-9._:/-]{0,127}[A-Za-z0-9._:-]$/;

/** Modello normalizzato: trim, `null` se vuoto. */
export function sanitizeModel(raw: string): string | null {
  const m = raw.trim().slice(0, MODEL_MAX);
  return m === "" ? null : m;
}

/** Errore sul nome del modello o null (vuoto = valido: predefinito). */
export function modelError(raw: string): string | null {
  const m = raw.trim();
  if (m === "") return null;
  return MODEL_RE.test(m)
    ? null
    : `Il modello deve avere 1-${MODEL_MAX} caratteri tra lettere, cifre, '.', '_', '-', ':' e '/' (senza '-' o '/' iniziale, '/' finale o '//')`;
}
