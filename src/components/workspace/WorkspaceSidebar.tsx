import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useWorkspaceActions } from "../../hooks/useWorkspace";
import { useTableStore } from "../../stores/tableStore";
import { useUiStore } from "../../stores/uiStore";
import * as api from "../../lib/api";
import { formatError } from "../../lib/formatError";
import { askConfirm } from "../../lib/askConfirm";

export default function WorkspaceSidebar() {
  const config = useWorkspaceStore((s) => s.config);
  const tables = useTableStore((s) => s.tables);
  const activeTableId = useTableStore((s) => s.activeTableId);
  const setActiveTable = useTableStore((s) => s.setActiveTable);
  const openModal = useUiStore((s) => s.openModal);
  const { switchDatabase, deleteDatabase } = useWorkspaceActions();
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);

  const dbId = config?.active_database_id;

  const handleDeleteTable = async (tableId: string, name: string) => {
    if (!dbId) return;
    const ok = await askConfirm(
      `Supprimer la table « ${name} » ?\n\nToutes ses données, champs et vues seront définitivement perdus.`,
      "Supprimer la table"
    );
    if (!ok) return;
    setError(null);
    try {
      await api.deleteTable(dbId, tableId);
      if (activeTableId === tableId) setActiveTable(null);
      await queryClient.invalidateQueries({ queryKey: ["tables", dbId] });
      queryClient.removeQueries({ queryKey: ["table-data", dbId, tableId] });
      queryClient.removeQueries({ queryKey: ["fields", dbId, tableId] });
      queryClient.removeQueries({ queryKey: ["views", dbId, tableId] });
    } catch (e) {
      setError(formatError(e));
    }
  };

  const handleDeleteDatabase = async (targetDbId: string, name: string) => {
    if (config && config.databases.length <= 1) {
      setError("Impossible de supprimer la dernière base.");
      return;
    }
    const countHint = tables.length ? ` (${tables.length} table(s) dans la base active)` : "";
    const ok = await askConfirm(
      `Supprimer la base « ${name} » ?\n\nLe fichier SQLite sera supprimé définitivement${targetDbId === dbId ? countHint : ""}. Cette action est irréversible.`,
      "Supprimer la base"
    );
    if (!ok) return;
    setError(null);
    try {
      await deleteDatabase(targetDbId);
      // les tables/fields/views seront rechargés automatiquement via les hooks
      if (dbId) {
        queryClient.removeQueries({ queryKey: ["tables", targetDbId] });
        queryClient.removeQueries({ queryKey: ["table-data", targetDbId] });
      }
    } catch (e) {
      setError(formatError(e));
    }
  };

  return (
    <aside className="sidebar" aria-label="Navigation workspace">
      <button className="sidebar-home-btn" onClick={() => setActiveTable(null)} title="Retour à l'accueil des bases" aria-label="Accueil bases">⌂ Accueil</button>
      <div className="sidebar-section" id="sidebar-bases-heading">Bases</div>
      <div role="list" aria-labelledby="sidebar-bases-heading">
        {config?.databases.map((db) => (
          <div
            key={db.id}
            role="button"
            tabIndex={0}
            aria-current={db.id === config.active_database_id ? "true" : undefined}
            aria-label={`Base ${db.name}${db.id === config.active_database_id ? " (active)" : ""}`}
            className={`sidebar-item ${db.id === config.active_database_id ? "active" : ""}`}
            onClick={() => void switchDatabase(db.id)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                void switchDatabase(db.id);
              }
            }}
          >
            <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis" }}>{db.name}</span>
            {db.role === "reference" && (
              <span className="field-type-chip">référentiel</span>
            )}
            <button
              className="row-delete sidebar-delete"
              title={`Supprimer la base « ${db.name} »`}
              aria-label={`Supprimer la base ${db.name}`}
              onClick={(e) => {
                e.stopPropagation();
                void handleDeleteDatabase(db.id, db.name);
              }}
            >
              ×
            </button>
          </div>
        ))}
      </div>
      <button onClick={() => openModal("createDatabase")}>+ Nouvelle base</button>

      <div className="sidebar-section" id="sidebar-tables-heading">Tables</div>
      <div role="list" aria-labelledby="sidebar-tables-heading">
        {tables.map((t) => (
          <div
            key={t.id}
            role="button"
            tabIndex={0}
            aria-current={t.id === activeTableId ? "true" : undefined}
            aria-label={`Table ${t.name}${t.id === activeTableId ? " (active)" : ""}`}
            className={`sidebar-item ${t.id === activeTableId ? "active" : ""}`}
            onClick={() => setActiveTable(t.id)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                setActiveTable(t.id);
              }
            }}
          >
            <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis" }}>{t.name}</span>
            <button
              className="row-delete sidebar-delete"
              title={`Supprimer la table « ${t.name} »`}
              aria-label={`Supprimer la table ${t.name}`}
              onClick={(e) => {
                e.stopPropagation();
                void handleDeleteTable(t.id, t.name);
              }}
            >
              ×
            </button>
          </div>
        ))}
      </div>
      {error && (
        <div role="alert" style={{ color: "var(--danger)", fontSize: 12, padding: "6px 8px", border: "1px solid var(--danger)", borderRadius: 6, background: "rgba(60,20,20,0.5)" }}>
          {error} <button onClick={() => setError(null)} style={{ marginLeft: 8 }}>×</button>
        </div>
      )}
      <div className="sidebar-actions">
        <button onClick={() => openModal("createTable")}>+ Table</button>
        <button
          disabled={!activeTableId}
          onClick={() => openModal("createView")}
        >
          + Vue
        </button>
      </div>
    </aside>
  );
}
