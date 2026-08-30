import { useState, useEffect, useRef } from "react";
import type { FieldType } from "../../types/field";
import FieldTypeSelector from "../fields/FieldTypeSelector";
import * as api from "../../lib/api";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import { useQueryClient } from "@tanstack/react-query";
import { formatError } from "../../lib/formatError";

function inferType(name: string): FieldType {
  const n = name.toLowerCase();
  if (n.includes("mail") || n.includes("email")) return "email";
  if (n.includes("tel") || n.includes("phone")) return "phone";
  if (n.includes("date") || n.includes("échéance") || n.includes("deadline")) return "date";
  if (n.includes("montant") || n.includes("prix") || n.includes("quant") || n.includes("nombre") || n.includes("amount")) return "number";
  if (n.includes("fait") || n.includes("pay") || n.includes("done") || n.includes("check")) return "checkbox";
  if (n.includes("url") || n.includes("lien")) return "url";
  return "text";
}

export default function InlineAddField({ onClose }: { onClose: () => void }) {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const tableId = useTableStore((s) => s.activeTableId);
  const setFields = useTableStore((s) => s.setFields);
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [type, setType] = useState<FieldType>("text");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => { inputRef.current?.focus(); }, []);
  useEffect(() => {
    if (name.trim().length >= 2) {
      const inferred = inferType(name);
      if (inferred !== type) setType(inferred);
    }
  }, [name]);

  const can = name.trim().length > 0 && !busy && !!dbId && !!tableId;

  const create = async () => {
    if (!can || !dbId || !tableId) return;
    setBusy(true);
    setErr("");
    try {
      await api.createField(dbId, tableId, { name: name.trim(), type, config: type === "select" ? { options: [] } : {} });
      const f = await api.listFields(dbId, tableId);
      setFields(f);
      queryClient.setQueryData(["fields", dbId, tableId], f);
      await queryClient.invalidateQueries({ queryKey: ["table-data", dbId, tableId] });
      onClose();
    } catch (e) {
      setErr(formatError(e));
      setBusy(false);
    }
  };

  return (
    <div role="dialog" aria-label="Ajout rapide de champ" style={{ display: "flex", alignItems: "center", gap: 6, padding: "6px 8px", background: "var(--bg-panel)", border: "1px solid var(--accent)", borderRadius: 8, boxShadow: "0 8px 24px rgba(0,0,0,0.4)", position: "absolute", top: 4, right: 4, zIndex: 10, minWidth: 360 }}>
      <input ref={inputRef} value={name} onChange={(e) => setName(e.target.value)} placeholder="Nom du champ (ex: Montant)" style={{ flex: 1 }} onKeyDown={(e) => { if (e.key === "Enter" && can) void create(); if (e.key === "Escape") onClose(); }} aria-label="Nom du champ" />
      <FieldTypeSelector value={type} onChange={setType} storedOnly />
      <button className="primary" disabled={!can} onClick={() => void create()} aria-label="Créer champ">{busy ? "…" : "Ajouter"}</button>
      <button onClick={onClose} aria-label="Fermer">×</button>
      {err && <span role="alert" style={{ color: "var(--danger)", fontSize: 11, maxWidth: 120, overflow: "hidden", textOverflow: "ellipsis" }}>{err}</span>}
    </div>
  );
}
