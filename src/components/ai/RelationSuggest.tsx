import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import { useQuery } from "@tanstack/react-query";
import * as api from "../../lib/api";
import { formatError } from "../../lib/formatError";

export default function RelationSuggest() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id ?? "");
  const activeTableId = useTableStore((s) => s.activeTableId);
  const q = useQuery({
    queryKey: ["ai-suggest-relations", dbId, activeTableId],
    queryFn: () => api.aiSuggestRelations(dbId, activeTableId!),
    enabled: !!dbId && !!activeTableId,
  });

  if (!activeTableId) return <p className="hint">Sélectionne une table.</p>;
  if (q.isPending) return <div>Chargement suggestions…</div>;
  if (q.isError) return <div role="alert" style={{ color: "var(--danger)" }}>{formatError(q.error)}</div>;
  const list = q.data ?? [];
  if (list.length === 0) return <p className="hint">Aucune suggestion de relation (chevauchement/noms insuffisants).</p>;
  return (
    <ul style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      {list.map((s, i) => (
        <li key={i} style={{ border: "1px solid var(--border)", borderRadius: 8, padding: 8 }}>
          <div><strong>{s.source_field_name}</strong> → <strong>{s.target_table_name}.{s.target_field_name}</strong> <span className="hint">({s.confidence.toFixed(2)})</span></div>
          <div className="hint">{s.reason} — {s.cardinality}</div>
          <div className="hint">{s.target_db_id !== s.source_db_id ? "cross-DB" : "intra-DB"} · {s.target_table_name} ({s.target_table_id.slice(0, 8)}…)</div>
        </li>
      ))}
    </ul>
  );
}
