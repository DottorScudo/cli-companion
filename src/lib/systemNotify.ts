// Notifica di sistema Windows (plugin ufficiale tauri-plugin-notification).
// Fuori da agents.ts perche' importa Tauri: i test node non lo caricano.

import { api } from "./tauri";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";

/** `true` se la finestra dell'app ha il focus (altrimenti: notifica di sistema). */
export async function appHasFocus(): Promise<boolean> {
  try {
    return await getCurrentWindow().isFocused();
  } catch {
    return document.hasFocus();
  }
}

/** Notifica del plugin ufficiale (senza click), chiedendo il permesso la prima volta. */
async function sendWithPlugin(title: string, body: string): Promise<void> {
  let granted = await isPermissionGranted();
  if (!granted) granted = (await requestPermission()) === "granted";
  if (granted) sendNotification({ title, body });
}

/**
 * Notifica di sistema senza tab associato (nessun click: il toast cliccabile
 * porta a un tab). Mostrata solo se la finestra non e' in primo piano.
 */
export async function notifyPlainIfUnfocused(title: string, body: string): Promise<void> {
  try {
    if (await appHasFocus()) return;
    await sendWithPlugin(title, body);
  } catch {
    // Notifiche non disponibili.
  }
}

/**
 * Mostra una notifica di sistema se la finestra non e' in primo piano.
 * Sicura da chiamare ovunque: permessi negati o plugin assente = nessun errore.
 */
export async function notifyIfUnfocused(tabId: string, title: string, body: string): Promise<void> {
  try {
    if (await appHasFocus()) return;
    try {
      // Toast cliccabile (Windows): il click porta al tab. Altrove errore.
      await api.showAgentNotification(tabId, title, body);
      return;
    } catch {
      // Non disponibile: plugin ufficiale, senza click.
    }
    await sendWithPlugin(title, body);
  } catch {
    // Notifiche non disponibili: il suono resta come segnale.
  }
}

/**
 * Notifica di sistema di un thread dell'Agents HUB, mostrata sempre: se
 * mostrarla lo decide il chiamante (`lib/hubNotify.ts`). Il toast cliccabile
 * (Windows) apre il thread; altrove il plugin, senza click.
 */
export async function notifyHubThread(channel: string, threadId: string, title: string, body: string): Promise<void> {
  try {
    try {
      await api.showHubNotification(channel, threadId, title, body);
      return;
    } catch {
      // Non disponibile: plugin ufficiale, senza click.
    }
    await sendWithPlugin(title, body);
  } catch {
    // Notifiche non disponibili: il suono resta come segnale.
  }
}
