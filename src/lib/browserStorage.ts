/** `localStorage`, o `null` se il browser lo blocca (accesso negato, modalità privata). */
export function browserStorage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}
