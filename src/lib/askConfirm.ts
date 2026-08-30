import { confirm } from "@tauri-apps/plugin-dialog";

/**
 * Confirmation compatible Tauri + navigateur.
 * En Tauri, window.confirm est patché pour appeler plugin:dialog|confirm
 * qui échoue si la permission/CSP n'est pas parfaite. On appelle directement
 * le plugin et on retombe sur window.confirm en cas d'échec.
 */
export async function askConfirm(message: string, title = "Confirmer"): Promise<boolean> {
  const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  if (isTauri) {
    try {
      return await confirm(message, { title, kind: "warning" });
    } catch (e) {
      console.warn("[askConfirm] Tauri confirm échoué, fallback window.confirm", e);
    }
  }
  return window.confirm(message);
}
