import { useState } from "react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import { useAIAnalyze } from "../../hooks/useAI";
import { formatError } from "../../lib/formatError";

export default function AnalysisPanel() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id ?? "");
  const activeTableId = useTableStore((s) => s.activeTableId);
  const [question, setQuestion] = useState("");
  const mut = useAIAnalyze();

  if (!activeTableId) return <p className="hint">Sélectionne une table.</p>;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <label className="modal-field">
        <span>Question (optionnel)</span>
        <input value={question} onChange={(e) => setQuestion(e.target.value)} placeholder="Ex: quel est le total des montants ?" />
      </label>
      <button className="primary" disabled={mut.isPending} onClick={() => mut.mutate({ dbId, tableId: activeTableId, question: question || undefined })}>
        {mut.isPending ? "Analyse…" : "Analyser"}
      </button>
      {mut.isError && <div role="alert" style={{ color: "var(--danger)" }}>{formatError(mut.error)}</div>}
      {mut.data && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <div><strong>Résumé:</strong> {mut.data.summary} <span className="hint">({mut.data.provider})</span></div>
          <div><strong>Insights:</strong><ul>{mut.data.insights.map((x,i)=>(<li key={i}>{x}</li>))}</ul></div>
          <div><strong>Suggestions:</strong><ul>{mut.data.suggestions.map((x,i)=>(<li key={i}>{x}</li>))}</ul></div>
        </div>
      )}
    </div>
  );
}
