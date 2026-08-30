import { useState } from "react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import { useAIGenerateFormula } from "../../hooks/useAI";
import { formatError } from "../../lib/formatError";

export default function FormulaGenerator() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id ?? "");
  const activeTableId = useTableStore((s) => s.activeTableId);
  const [prompt, setPrompt] = useState("");
  const mut = useAIGenerateFormula();

  if (!activeTableId) return <p className="hint">Sélectionne une table.</p>;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <label className="modal-field">
        <span>Décris la formule en langage naturel</span>
        <textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder="Ex: si montant > 50 alors élevé sinon bas"
          rows={3}
        />
      </label>
      <button
        className="primary"
        disabled={!prompt.trim() || mut.isPending}
        onClick={() => mut.mutate({ dbId, tableId: activeTableId, prompt })}
      >
        {mut.isPending ? "Génération…" : "Générer"}
      </button>
      {mut.isError && <div role="alert" style={{ color: "var(--danger)" }}>{formatError(mut.error)}</div>}
      {mut.data && (
        <div style={{ border: "1px solid var(--border)", borderRadius: 8, padding: 8, display: "flex", flexDirection: "column", gap: 4 }}>
          <div><strong>Expression:</strong> <code>{mut.data.expression}</code></div>
          <div className="hint">{mut.data.explanation} — {mut.data.provider} {mut.data.valid ? "✅" : "❌"}</div>
          {mut.data.error && <div style={{ color: "var(--danger)" }}>{mut.data.error}</div>}
        </div>
      )}
    </div>
  );
}
