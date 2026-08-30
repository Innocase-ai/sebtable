import { useState } from "react";
import Modal from "../common/Modal";
import * as api from "../../lib/api";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import { useUiStore } from "../../stores/uiStore";
import { formatError } from "../../lib/formatError";

export default function CreateViewModal() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const tableId = useTableStore((s) => s.activeTableId);
  const viewConfig = useTableStore((s) => s.viewConfig);
  const setViews = useTableStore((s) => s.setViews);
  const closeModal = useUiStore((s) => s.closeModal);

  const [name, setName] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!dbId || !tableId || busy) return;
    setBusy(true);
    setError("");
    try {
      await api.createView(dbId, {
        table_id: tableId,
        name: name.trim(),
        type: "grid",
        config: viewConfig,
      });
      const views = await api.listViews(dbId, tableId);
      setViews(views);
      closeModal();
    } catch (e) {
      setError(formatError(e));
      setBusy(false);
    }
  };

  return (
    <Modal title="Enregistrer la vue">
      <div className="modal-field">
        <label>Nom de la vue</label>
        <input
          value={name}
          autoFocus
          placeholder="Vue filtrée"
          onChange={(e) => setName(e.target.value)}
        />
      </div>
      <div className="launcher-error" role="alert" aria-live="assertive">{error}</div>
      <div className="modal-actions">
        <button onClick={closeModal} disabled={busy}>Annuler</button>
        <button className="primary" disabled={!name.trim() || busy} onClick={() => void submit()}>
          {busy ? "Enregistrement…" : "Enregistrer"}
        </button>
      </div>
    </Modal>
  );
}
