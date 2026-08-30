import { useState } from "react";
import { TEMPLATES, type Template } from "../../lib/templates";
import { useWorkspaceActions } from "../../hooks/useWorkspace";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import * as api from "../../lib/api";
import { formatError } from "../../lib/formatError";

export default function TemplatePicker({ onDone }: { onDone?: () => void }) {
  const { createDatabase } = useWorkspaceActions();
  const config = useWorkspaceStore((s) => s.config);
  const setActiveTable = useTableStore((s) => s.setActiveTable);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const createFromTemplate = async (tpl: Template) => {
    setBusy(tpl.id);
    setErr(null);
    try {
      const db = await createDatabase(tpl.name, "project");
      for (const t of tpl.tables) {
        await api.createTable(db.id, t.name, t.fields);
      }
      const cfg = await api.switchDatabase(db.id);
      useWorkspaceStore.getState().setConfig(cfg);
      useTableStore.getState().reset();
      const tables = await api.listTables(db.id);
      setActiveTable(tables[0]?.id ?? null);
      onDone?.();
    } catch (e) {
      setErr(formatError(e));
    } finally {
      setBusy(null);
    }
  };

  // si un workspace est déjà ouvert, on propose aussi création dans workspace courant
  // sinon on informe qu'il faut d'abord créer un workspace
  if (!config) return null;

  return (
    <div style={{ marginTop: 16 }}>
      <h3 style={{ margin: "0 0 8px", fontSize: 14 }}>Démarrer d'un modèle</h3>
      <p className="hint" style={{ margin: "0 0 10px" }}>1 clic → base pré-remplie (tables + champs). Modifiable ensuite.</p>
      <div className="db-home-grid">
        {TEMPLATES.map((tpl) => (
          <button
            key={tpl.id}
            className="db-card"
            onClick={() => void createFromTemplate(tpl)}
            disabled={!!busy}
            aria-label={`Créer ${tpl.name}`}
            style={{ textAlign: "left" }}
          >
            <span className="db-card-icon" aria-hidden>{tpl.icon}</span>
            <span className="db-card-body">
              <span className="db-card-name">{tpl.name}</span>
              <span className="db-card-meta">{tpl.description}</span>
              <span className="hint" style={{ marginTop: 4, fontSize: 11 }}>{tpl.tables.map((t) => t.name).join(" · ")}</span>
            </span>
            <span style={{ marginLeft: "auto", fontSize: 12, color: "var(--accent)" }}>{busy === tpl.id ? "…" : "+"}</span>
          </button>
        ))}
      </div>
      {err && <p role="alert" className="db-home-error">{err} <button onClick={() => setErr(null)} style={{ marginLeft: 8 }}>×</button></p>}
      <p className="hint" style={{ marginTop: 8 }}>Base vide aussi disponible via <strong>+ Nouvelle base</strong>.</p>
    </div>
  );
}
