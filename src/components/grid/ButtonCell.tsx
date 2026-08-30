import type { Field } from "../../types/field";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export default function ButtonCell({ field, recordId, onAction }: { field: Field; recordId: string; onAction?: (fieldId: string, recordId: string) => void }) {
  const cfg = field.config as { label?: string; action?: string; url?: string } | undefined;
  const label = cfg?.label || field.name || "Action";
  const handleClick = async () => {
    if (onAction) { onAction(field.id, recordId); return; }
    const url = cfg?.url;
    if (url) {
      const filled = url.replace("{recordId}", recordId).replace("{fieldId}", field.id);
      try {
        if (isTauri()) {
          // plugin shell : ouvre dans le navigateur par défaut (window.open est bloqué en prod)
          const { open } = await import("@tauri-apps/plugin-shell");
          void open(filled);
        } else {
          window.open(filled, "_blank", "noopener,noreferrer");
        }
      } catch {
        window.open(filled, "_blank", "noopener,noreferrer");
      }
    } else {
      alert(`${label} — record ${recordId.slice(0, 8)}`);
    }
  };
  return (
    <button
      onClick={handleClick}
      style={{ padding: "2px 8px", fontSize: 12, background: "var(--accent)", color: "#fff", border: "1px solid var(--accent)", borderRadius: 6 }}
      title={cfg?.action || label}
    >
      {label}
    </button>
  );
}
