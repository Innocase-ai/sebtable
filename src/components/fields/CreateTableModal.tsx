import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import Modal from "../common/Modal";
import FieldTypeSelector from "./FieldTypeSelector";
import * as api from "../../lib/api";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import { useUiStore } from "../../stores/uiStore";
import { formatError } from "../../lib/formatError";
import type { FieldInput, FieldType } from "../../types/field";

interface FieldDraft {
  name: string;
  type: FieldType;
}

export default function CreateTableModal() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const setTables = useTableStore((s) => s.setTables);
  const setActiveTable = useTableStore((s) => s.setActiveTable);
  const closeModal = useUiStore((s) => s.closeModal);
  const queryClient = useQueryClient();

  const [name, setName] = useState("");
  const [fields, setFields] = useState<FieldDraft[]>([
    { name: "Nom", type: "text" },
  ]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const addField = () =>
    setFields((f) => [...f, { name: "", type: "text" }]);
  const removeField = (i: number) =>
    setFields((f) => f.filter((_, idx) => idx !== i));
  const updateField = (i: number, patch: Partial<FieldDraft>) =>
    setFields((f) => f.map((fld, idx) => (idx === i ? { ...fld, ...patch } : fld)));

  const submit = async () => {
    if (!dbId || busy) return;
    setBusy(true);
    setError("");
    const tempId = `tbl_tmp_${Date.now().toString(36)}`;
    const prevTables = [...useTableStore.getState().tables];
    const optimistic = { id: tempId, name: name.trim() } as import("../../types/database").Table;
    setTables([...prevTables, optimistic]);
    queryClient.setQueryData(["tables", dbId], [...prevTables, optimistic]);
    try {
      const fieldInputs: FieldInput[] = fields
        .filter((f) => f.name.trim())
        .map((f) => ({
          name: f.name.trim(),
          type: f.type,
          config: f.type === "select" ? { options: [] } : undefined,
        }));
      const table = await api.createTable(dbId, name.trim(), fieldInputs);
      const tables = await api.listTables(dbId);
      setTables(tables);
      setActiveTable(table.id);
      queryClient.setQueryData(["tables", dbId], tables);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["tables", dbId] }),
        queryClient.invalidateQueries({ queryKey: ["table-data", dbId] }),
      ]);
      closeModal();
    } catch (e) {
      setTables(prevTables);
      queryClient.setQueryData(["tables", dbId], prevTables);
      setError(formatError(e));
      setBusy(false);
    }
  };

  return (
    <Modal title="Nouvelle table">
      <div className="modal-field">
        <label>Nom de la table</label>
        <input
          value={name}
          autoFocus
          placeholder="Clients"
          onChange={(e) => setName(e.target.value)}
        />
      </div>

      <div className="modal-field">
        <label>Champs</label>
        {fields.map((f, i) => (
          <div className="field-row" key={i}>
            <input
              value={f.name}
              placeholder="Nom du champ"
              onChange={(e) => updateField(i, { name: e.target.value })}
            />
            <FieldTypeSelector
              value={f.type}
              storedOnly
              onChange={(t) => updateField(i, { type: t })}
            />
            <button
              onClick={() => removeField(i)}
              disabled={fields.length <= 1}
              aria-label={`Supprimer le champ ${i + 1}`}
            >
              ×
            </button>
          </div>
        ))}
        <button onClick={addField}>+ Ajouter un champ</button>
      </div>

      <div className="launcher-error" role="alert" aria-live="assertive">{error}</div>

      <div className="modal-actions">
        <button onClick={closeModal} disabled={busy}>Annuler</button>
        <button className="primary" disabled={!name.trim() || busy} onClick={() => void submit()}>
          {busy ? "Création…" : "Créer"}
        </button>
      </div>
    </Modal>
  );
}
