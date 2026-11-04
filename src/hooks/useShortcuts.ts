import { useEffect } from "react";
import { useOfficeShortcut } from "../components/office/useOfficeShortcut";
import { closeOffice, toggleMaximizeActivePane, toggleManager, toggleOffice } from "../app/actions/view";
import { configStore } from "../stores/config";
import { layoutStore, viewWorkspaceId } from "../stores/layout";
import { modalOpen, uiStore } from "../stores/ui";
import { useStore } from "./useStore";

/** Ctrl+Maiusc+<key> senza altri modificatori. */
function isChord(e: KeyboardEvent, key: string): boolean {
  return e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === key;
}

/** Ascolta la tastiera in fase di cattura, così xterm non vede il tasto. */
function useCaptureShortcut(handler: (e: KeyboardEvent) => void): void {
  useEffect(() => {
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [handler]);
}

/** Ctrl+Maiusc+B: apre/chiude il Tasks Manager del workspace attivo (non nella vista Globale). */
function onToggleManagerKey(e: KeyboardEvent): void {
  if (!isChord(e, "b")) return;
  // Con un dialogo aperto la scorciatoia non fa nulla.
  if (modalOpen(uiStore.getState())) return;
  e.preventDefault();
  e.stopPropagation();
  const id = viewWorkspaceId(layoutStore.getState());
  if (id) toggleManager(id);
}

/** Ctrl+Maiusc+M: massimizza/ripristina il pannello attivo della vista mostrata. Niente Esc: serve alle CLI. */
function onToggleMaximizeKey(e: KeyboardEvent): void {
  if (!isChord(e, "m")) return;
  e.preventDefault();
  e.stopPropagation();
  toggleMaximizeActivePane();
}

/** Scorciatoie globali: Ctrl+Maiusc+B (Tasks Manager), +M (massimizza), +O (Ufficio). */
export function useShortcuts(): void {
  useCaptureShortcut(onToggleManagerKey);
  useCaptureShortcut(onToggleMaximizeKey);

  // Ufficio: Ctrl+Maiusc+O e pulsante della sidebar. Spegnere i bot lo chiude,
  // così riaccenderli non lo riapre da solo.
  const botsEnabled = useStore(configStore, (s) => s.botSettings.enabled);
  useOfficeShortcut(botsEnabled, toggleOffice);
  useEffect(() => {
    if (!botsEnabled) closeOffice();
  }, [botsEnabled]);
}
