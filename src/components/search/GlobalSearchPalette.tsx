import { useEffect, useState, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { searchWorkspace } from "../../lib/api";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import { useUiStore } from "../../stores/uiStore";
import * as api from "../../lib/api";
import { formatError } from "../../lib/formatError";

export default function GlobalSearchPalette() {
  const open = useUiStore((s) => s.searchOpen);
  const setOpen = useUiStore((s) => s.setSearchOpen);
  const [q, setQ] = useState("");
  const [debounced, setDebounced] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const config = useWorkspaceStore((s) => s.config);
  const setActiveTable = useTableStore((s) => s.setActiveTable);

  useEffect(() => {
    const t = setTimeout(() => setDebounced(q.trim()), 250);
    return () => clearTimeout(t);
  }, [q]);

  useEffect(() => {
    if (open) setTimeout(() => inputRef.current?.focus(), 50);
    else setQ("");
  }, [open]);

  const { data, isFetching, error } = useQuery({
    queryKey: ["search", debounced],
    queryFn: () => searchWorkspace(debounced),
    enabled: open && debounced.length >= 2,
  });

  const results = data ?? [];

  const handleSelect = async (r: (typeof results)[number]) => {
    try {
      if (r.db_id && r.db_id !== config?.active_database_id) {
        const cfg = await api.switchDatabase(r.db_id);
        useWorkspaceStore.getState().setConfig(cfg);
        useTableStore.getState().reset();
      }
      if (r.table_id) {
        setActiveTable(r.table_id);
      }
      setOpen(false);
    } catch (e) {
      console.error(formatError(e));
    }
  };

  if (!open) return null;

  return (
    <div className="modal-overlay" onClick={() => setOpen(false)} style={{ zIndex: 90 }}>
      <div className="modal" style={{ width: 560, maxHeight: "70vh" }} onClick={(e) => e.stopPropagation()} role="dialog" aria-label="Recherche globale">
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <input
            ref={inputRef}
            data-shortcut="global-search"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Rechercher tables, champs, enregistrements… (≥2 caractères)"
            style={{ flex: 1 }}
            aria-label="Recherche globale"
            onKeyDown={(e) => { if (e.key === "Escape") setOpen(false); if (e.key === "Enter" && results[0]) void handleSelect(results[0]); }}
          />
          <button onClick={() => setOpen(false)} aria-label="Fermer">×</button>
        </div>
        <div style={{ marginTop: 8, minHeight: 120, maxHeight: 360, overflowY: "auto", display: "flex", flexDirection: "column", gap: 4 }}>
          {isFetching && <span className="hint">Recherche…</span>}
          {error && <span role="alert" style={{ color: "var(--danger)", fontSize: 12 }}>{formatError(error)}</span>}
          {!isFetching && debounced.length >= 2 && results.length === 0 && !error && <span className="hint">Aucun résultat pour "{debounced}"</span>}
          {debounced.length < 2 && <span className="hint">Tape ≥2 caractères pour lancer la recherche (Ctrl+K pour ouvrir)</span>}
          {results.map((r) => (
            <button
              key={`${r.db_id}/${r.table_id}/${r.record_id}/${r.field_id}`}
              onClick={() => void handleSelect(r)}
              style={{ display: "flex", flexDirection: "column", alignItems: "flex-start", padding: "8px 10px", borderRadius: 8, border: "1px solid var(--border)", background: "var(--bg-header)", textAlign: "left" }}
            >
              <span style={{ fontWeight: 600, fontSize: 12 }}>
                {r.db_name} › {r.table_name} {r.field_name ? `· ${r.field_name}` : ""} {r.record_id ? `· ${r.record_id.slice(0, 8)}` : ""}
              </span>
              <span className="hint" style={{ fontSize: 11, wordBreak: "break-all" }}>{r.snippet || "—"}</span>
              <span style={{ fontSize: 10, color: "var(--text-muted)" }}>{r.record_id ? "Ouvrir la table (le record est dans la vue)" : "Ouvrir la table"}</span>
            </button>
          ))}
        </div>
        <span className="hint" style={{ fontSize: 11 }}>Entrée = ouvrir 1er résultat · Échap = fermer</span>
      </div>
    </div>
  );
}
