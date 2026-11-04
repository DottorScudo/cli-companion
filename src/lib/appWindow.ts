import { getCurrentWindow } from "@tauri-apps/api/window";

export type AppWindow = ReturnType<typeof getCurrentWindow>;

/** La finestra dell'app, o `null` fuori da Tauri (anteprima nel browser). */
export function appWindow(): AppWindow | null {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}
