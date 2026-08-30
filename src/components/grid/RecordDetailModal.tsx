import { useEffect, useState } from "react";
import Modal from "../common/Modal";
import * as api from "../../lib/api";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import type { Field } from "../../types/field";
import type { Record as TableRecord } from "../../types/record";
import { formatValue } from "./Cell";
import { formatError } from "../../lib/formatError";

export default function RecordDetailModal({ recordId, onClose }: { recordId: string; onClose: () => void }) {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const tableId = useTableStore((s) => s.activeTableId);
  const fields = useTableStore((s) => s.fields);
  const [data, setData] = useState<{ record: TableRecord; relations: globalThis.Record<string, TableRecord[]> } | null>(null);
  const [err, setErr] = useState("");
  const [editingField, setEditingField] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    if (!dbId || !tableId || !recordId) return;
    api.getRecordWithRelations(dbId, tableId, recordId, 1).then(setData).catch((e) => setErr(formatError(e)));
  }, [dbId, tableId, recordId]);

  if (!recordId) return null;
  if (err) return <Modal title="Détail"><div role="alert" style={{ color: "var(--danger)" }}>{err}</div><button onClick={onClose}>Fermer</button></Modal>;
  if (!data) return <Modal title="Détail"><div>Chargement…</div></Modal>;

  const rec = data.record as TableRecord;
  const upsert = async (fieldId: string, value: unknown) => {
    if (!dbId || !tableId) return;
    await api.upsertRecords(dbId, tableId, [{ _id: recordId, [fieldId]: value } as unknown as TableRecord]);
    const fresh = await api.getRecordWithRelations(dbId, tableId, recordId, 1);
    setData(fresh as { record: TableRecord; relations: globalThis.Record<string, TableRecord[]> });
  };

  return (
    <Modal title={`Enregistrement ${String(rec._id).slice(0, 8)}`}>
      <div style={{ display: "flex", flexDirection: "column", gap: 10, maxHeight: "60vh", overflowY: "auto" }}>
        {fields.map((f: Field) => {
          const isBacklinkField = f.type === "link" && (f.config as { is_backlink?: boolean })?.is_backlink;
          const isComputed = f.type === "lookup" || f.type === "rollup" || f.type === "count" || f.type === "formula" || isBacklinkField;
          // Éditable texte : uniquement scalaires stockés. link/attachment ont
          // leurs propres éditeurs (LinkPicker/AttachmentCell) — une chaîne brute
          // écraserait la colonne JSON.
          const editable = !isComputed && !["link", "attachment", "created_time", "last_modified_time", "created_by", "last_modified_by"].includes(f.type);
          const val = rec[f.id];
          let txt = formatValue(f, val);
          if (f.type === "link" && Array.isArray(val)) {
            txt = (val as unknown[]).map((lv) => {
              const o = lv as { display?: unknown; record_id?: string };
              return typeof o.display === "string" && o.display ? o.display : String(o.record_id ?? "").slice(0, 8);
            }).join(", ");
          } else if (f.type === "attachment" && Array.isArray(val)) {
            txt = (val as unknown[]).map((a) => (a as { name?: string }).name ?? "").filter(Boolean).join(", ");
          }
          const isEditing = editingField === f.id;
          return (
            <div key={f.id} style={{ display: "flex", flexDirection: "column", gap: 4, padding: 8, border: "1px solid var(--border)", borderRadius: 8, background: "var(--bg-header)" }}>
              <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{f.name} <em>({f.type})</em> {isComputed && "ƒ"}</span>
              {isEditing && editable ? (
                <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                  {f.type === "select" ? (
                    <select value={draft} onChange={(e) => setDraft(e.target.value)} autoFocus style={{ flex: 1 }}>
                      <option value="">—</option>
                      {((f.config as { options?: { id: string; name: string }[] })?.options ?? []).map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
                    </select>
                  ) : f.type === "checkbox" ? (
                    <input type="checkbox" checked={draft === "1" || draft === "true" || draft === "true"} onChange={(e) => setDraft(String(e.target.checked))} autoFocus />
                  ) : (
                    <input value={draft} onChange={(e) => setDraft(e.target.value)} autoFocus style={{ flex: 1 }} />
                  )}
                  <button className="primary" onClick={async () => { await upsert(f.id, f.type === "number" ? (draft.trim() === "" ? null : Number(draft)) : f.type === "checkbox" ? draft === "true" : draft); setEditingField(null); }}>OK</button>
                  <button onClick={() => setEditingField(null)}>Annuler</button>
                </div>
              ) : (
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span style={{ flex: 1, whiteSpace: "pre-wrap", wordBreak: "break-word" }}>{txt || "—"}</span>
                  {editable && <button onClick={() => { setDraft(String(val ?? "")); setEditingField(f.id); }}>Éditer</button>}
                </div>
              )}
              {data.relations[f.id] && (data.relations[f.id] as unknown[]).length > 0 && (
                <span className="hint" style={{ fontSize: 11 }}>{(data.relations[f.id] as unknown[]).length} relation(s) liée(s)</span>
              )}
            </div>
          );
        })}
      </div>
      <div className="modal-actions">
        <button onClick={onClose}>Fermer</button>
      </div>
    </Modal>
  );
}
