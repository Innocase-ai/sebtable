export function formatError(e: unknown): string {
  if (e instanceof Error) return e.message;
  const s = String(e);
  // Tauri enveloppe souvent en "Error: message" — on nettoie le préfixe
  return s.replace(/^Error:\s*/i, "").trim() || "Une erreur est survenue";
}
