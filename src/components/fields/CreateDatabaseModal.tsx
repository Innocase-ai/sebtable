import { useState } from "react";
import Modal from "../common/Modal";
import * as api from "../../lib/api";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useUiStore } from "../../stores/uiStore";
import { formatError } from "../../lib/formatError";

export default function CreateDatabaseModal() {
  const createDatabase = useUiStore((s) => s.modal === "createDatabase");
  const closeModal = useUiStore((s) => s.closeModal);
  const addDatabase = useWorkspaceStore((s) => s.addDatabase);

  const [name, setName] = useState("");
  const [role, setRole] = useState<"reference" | "project">("project");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  if (!createDatabase) return null;

  const submit = async () => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const db = await api.createDatabase(name.trim(), role);
      addDatabase(db);
      closeModal();
    } catch (e) {
      setError(formatError(e));
      setBusy(false);
    }
  };

  return (
    <Modal title="Nouvelle base">
      <div className="modal-field">
        <label>Nom de la base</label>
        <input
          value={name}
          autoFocus
          placeholder="Référentiel Produits"
          onChange={(e) => setName(e.target.value)}
        />
      </div>
      <div className="modal-field">
        <label>Rôle</label>
        <select
          value={role}
          onChange={(e) => setRole(e.target.value as "reference" | "project")}
        >
          <option value="project">Projet</option>
          <option value="reference">Référentiel (partagé)</option>
        </select>
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
