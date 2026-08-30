import { useState } from "react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import { useAICleanPreview, useAIApply } from "../../hooks/useAI";
import { formatError } from "../../lib/formatError";

export default function CleaningWizard() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id ?? "");
  const activeTableId = useTableStore((s) => s.activeTableId);
  const [instruction, setInstruction] = useState("");
  const previewMut = useAICleanPreview();
  const applyMut = useAIApply();

  if (!activeTableId) return <p className="hint">Sélectionne une table.</p>;

  const plan = previewMut.data;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <label className="modal-field">
        <span>Instruction de nettoyage</span>
        <input value={instruction} onChange={(e) => setInstruction(e.target.value)} placeholder="Ex: supprimer les espaces, normaliser les emails, dédupliquer" />
      </label>
      <button className="primary" disabled={!instruction.trim() || previewMut.isPending} onClick={() => previewMut.mutate({ dbId, tableId: activeTableId, instruction })}>
        {previewMut.isPending ? "Prévisualisation…" : "Prévisualiser"}
      </button>
      {previewMut.isError && <div role="alert" style={{ color: "var(--danger)" }}>{formatError(previewMut.error)}</div>}
      {plan && (
        <div style={{ border: "1px solid var(--border)", borderRadius: 8, padding: 8, display: "flex", flexDirection: "column", gap: 6 }}>
          <div><strong>Ops ({plan.provider}):</strong></div>
          <ul>{plan.ops.map((op, i) => <li key={i}>{op.description} — {op.type} sur {op.field_name}</li>)}</ul>
          {plan.preview.length > 0 ? (
            <div>
              <strong>Aperçu ({plan.preview.length}):</strong>
              <ul>{plan.preview.map((r, i) => <li key={i}><code>{String(r.before).slice(0, 40)}</code> → <code>{String(r.after).slice(0, 40)}</code> <span className="hint">({r.record_id})</span></li>)}</ul>
            </div>
          ) : <span className="hint">Aucun changement détecté sur l'échantillon</span>}
          <button disabled={applyMut.isPending || plan.ops.length === 0} onClick={() => applyMut.mutate({ dbId, tableId: activeTableId, plan })} className="primary">
            {applyMut.isPending ? "Application…" : `Appliquer (≈ ${plan.estimated_rows} ligne(s) estimée(s))`}
          </button>
          {applyMut.isError && <div role="alert" style={{ color: "var(--danger)" }}>{formatError(applyMut.error)}</div>}
          {applyMut.data && <div style={{ color: "var(--accent)" }}>{applyMut.data.applied_rows} ligne(s) modifiée(s) ✅</div>}
        </div>
      )}
    </div>
  );
}
