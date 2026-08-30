import { useState } from "react";
import { useTableStore } from "../../stores/tableStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import * as api from "../../lib/api";
import { useQueryClient } from "@tanstack/react-query";
import { formatError } from "../../lib/formatError";
import type { Field } from "../../types/field";

export default function FormView() {
  const fields = useTableStore((s) => s.fields);
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const tableId = useTableStore((s) => s.activeTableId);
  const queryClient = useQueryClient();
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [err, setErr] = useState("");
  const [ok, setOk] = useState("");
  const [busy, setBusy] = useState(false);

  // Exclut les champs sans saisie texte : calculés + link/attachment/button
  // (lien et pièce jointe ont leurs propres éditeurs — une chaîne brute
  // écraserait la colonne JSON).
  const editable = fields.filter((f) => !["lookup","rollup","count","formula","link","attachment","button","created_time","last_modified_time","created_by","last_modified_by"].includes(f.type));

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!dbId || !tableId) return;
    setErr(""); setOk(""); setBusy(true);
    try {
      const rec: Record<string, unknown> = { _id: `rec_${Math.random().toString(36).slice(2,10)}` };
      for (const f of editable) {
        const v = values[f.id];
        if (v !== undefined && v !== "") rec[f.id] = f.type === "number" ? Number(v) : f.type === "checkbox" ? Boolean(v) : v;
      }
      await api.upsertRecords(dbId, tableId, [rec as unknown as import("../../types/record").Record]);
      await queryClient.invalidateQueries({ queryKey: ["table-data", dbId, tableId] });
      setOk("Enregistrement créé");
      setValues({});
    } catch (ex) { setErr(formatError(ex)); }
    setBusy(false);
  };

  return (
    <form onSubmit={submit} style={{ maxWidth: 560, margin: "16px auto", background: "var(--bg-panel)", border: "1px solid var(--border)", borderRadius: 12, padding: 16, display: "flex", flexDirection: "column", gap: 12 }}>
      <h3 style={{ margin: 0 }}>Nouvel enregistrement</h3>
      <p className="hint" style={{ margin: 0 }}>Saisie simple — les champs calculés ne sont pas affichés.</p>
      {editable.map((f: Field) => (
        <label key={f.id} style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <span style={{ fontSize: 12, color: "var(--text-muted)" }}>{f.name} <em>({f.type})</em></span>
          {f.type === "checkbox" ? (
            <input type="checkbox" checked={Boolean(values[f.id])} onChange={(e) => setValues((s) => ({ ...s, [f.id]: e.target.checked }))} />
          ) : f.type === "select" ? (
            <select value={String(values[f.id] ?? "")} onChange={(e) => setValues((s) => ({ ...s, [f.id]: e.target.value }))}>
              <option value="">—</option>
              {((f.config as { options?: { id: string; name: string }[] })?.options ?? []).map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
            </select>
          ) : f.type === "long_text" ? (
            <textarea value={String(values[f.id] ?? "")} onChange={(e) => setValues((s) => ({ ...s, [f.id]: e.target.value }))} rows={3} />
          ) : f.type === "date" ? (
            <input type="date" value={String(values[f.id] ?? "")} onChange={(e) => setValues((s) => ({ ...s, [f.id]: e.target.value }))} />
          ) : (
            <input type={f.type === "number" ? "number" : f.type === "email" ? "email" : "text"} value={String(values[f.id] ?? "")} onChange={(e) => setValues((s) => ({ ...s, [f.id]: e.target.value }))} placeholder={f.name} />
          )}
        </label>
      ))}
      {err && <span role="alert" style={{ color: "var(--danger)", fontSize: 12 }}>{err}</span>}
      {ok && <span role="status" style={{ color: "var(--accent)", fontSize: 12 }}>{ok}</span>}
      <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
        <button type="submit" className="primary" disabled={busy}>{busy ? "Envoi…" : "Créer"}</button>
      </div>
    </form>
  );
}
