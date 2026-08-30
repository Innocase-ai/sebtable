import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import Modal from "../common/Modal";
import FieldTypeSelector from "./FieldTypeSelector";
import * as api from "../../lib/api";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import { useUiStore } from "../../stores/uiStore";
import { formatError } from "../../lib/formatError";
import {
  isBacklink,
  isStoredField,
  type Field,
  type FieldType,
} from "../../types/field";

const ROLLUP_FUNCTIONS = [
  { value: "sum", label: "Somme" },
  { value: "count", label: "Compte" },
  { value: "avg", label: "Moyenne" },
  { value: "min", label: "Min" },
  { value: "max", label: "Max" },
  { value: "arrayjoin", label: "Joindre" },
];

export default function CreateFieldModal() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const tableId = useTableStore((s) => s.activeTableId);
  const tables = useTableStore((s) => s.tables);
  const fields = useTableStore((s) => s.fields);
  const setFields = useTableStore((s) => s.setFields);
  const closeModal = useUiStore((s) => s.closeModal);
  const queryClient = useQueryClient();

  const [name, setName] = useState("");
  const [type, setType] = useState<FieldType>("text");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  // select
  const [optionsText, setOptionsText] = useState("");
  // link — cross-DB (Phase 3)
  const [targetDbId, setTargetDbId] = useState("");
  const [targetTableId, setTargetTableId] = useState("");
  const [availableTables, setAvailableTables] = useState<{ id: string; name: string }[]>([]);
  const [cardinality, setCardinality] = useState<"one" | "many">("many");
  const [createBacklink, setCreateBacklink] = useState(true);
  // lookup / rollup / count
  const [sourceLinkFieldId, setSourceLinkFieldId] = useState("");
  const [targetFieldId, setTargetFieldId] = useState("");
  const [rollupFunction, setRollupFunction] = useState("sum");
  // formula
  const [expression, setExpression] = useState("");
  // validation
  const [required, setRequired] = useState(false);
  const [unique, setUnique] = useState(false);
  // button
  const [buttonLabel, setButtonLabel] = useState("");
  const [buttonUrl, setButtonUrl] = useState("");

  const workspaceConfig = useWorkspaceStore((s) => s.config);
  const linkFields = fields.filter((f) => f.type === "link" && !isBacklink(f));
  const sourceLink = linkFields.find((f) => f.id === sourceLinkFieldId);
  const targetTableOfLink = sourceLink?.config?.target_table_id as string | undefined;

  const [targetFields, setTargetFields] = useState<Field[]>([]);

  // Charger les tables de la DB cible sélectionnée (Phase 3 cross-DB)
  useEffect(() => {
    if (type !== "link") return;
    const effDbId = targetDbId || dbId;
    if (!effDbId) {
      setAvailableTables(tables.map((t) => ({ id: t.id, name: t.name })));
      return;
    }
    if (!targetDbId) {
      // intra-DB : utiliser tables déjà chargées
      setAvailableTables(tables.map((t) => ({ id: t.id, name: t.name })));
      return;
    }
    let cancelled = false;
    api
      .listTables(effDbId)
      .then((ts) => {
        if (!cancelled) setAvailableTables(ts.map((t) => ({ id: t.id, name: t.name })));
      })
      .catch(() => {
        if (!cancelled) setAvailableTables([]);
      });
    return () => {
      cancelled = true;
    };
  }, [type, dbId, targetDbId, tables]);

  useEffect(() => {
    let cancelled = false;
    if (!dbId || !sourceLinkFieldId || !targetTableOfLink) {
      setTargetFields([]);
      setTargetFieldId("");
      return;
    }
    api
      .listFields(dbId, targetTableOfLink)
      .then((f) => {
        // Un lookup cible un champ calculé n'a pas de colonne SQL → grille cassée.
        if (!cancelled) setTargetFields(f.filter(isStoredField));
      })
      .catch(console.error);
    return () => {
      cancelled = true;
    };
  }, [dbId, sourceLinkFieldId, targetTableOfLink]);

  const needsSourceLink = type === "lookup" || type === "rollup" || type === "count";
  const needsTargetField = type === "lookup" || type === "rollup";
  const needsTargetTable = type === "link";

  const canSubmit =
    name.trim() !== "" &&
    (!needsTargetTable || targetTableId !== "") &&
    (!needsSourceLink || sourceLinkFieldId !== "") &&
    (!needsTargetField || targetFieldId !== "") &&
    (type !== "formula" || expression.trim() !== "");

  const submit = async () => {
    if (!dbId || !tableId || busy) return;
    setBusy(true);
    setError("");
    // Optimiste : ajoute un champ temporaire pour feedback immédiat
    const tempId = `fld_tmp_${Date.now().toString(36)}`;
    const prevFields = [...fields];
    const optimistic: Field = {
      id: tempId,
      table_id: tableId,
      name: name.trim(),
      type,
      config: {} as Field["config"],
    } as Field;
    setFields([...fields, optimistic]);
    queryClient.setQueryData(["fields", dbId, tableId], [...fields, optimistic]);
    try {
      if (type === "link") {
        await api.createLinkField(dbId, tableId, name.trim(), {
          target_table_id: targetTableId,
          target_db_id: targetDbId || "",
          cardinality,
          allow_creating: true,
          is_backlink: createBacklink,
        });
      } else {
        let config: Record<string, unknown> = {};
        switch (type) {
          case "select": {
            const options = optionsText
              .split("\n")
              .map((s) => s.trim())
              .filter(Boolean)
              .map((s, i) => ({
                id: `opt_${Date.now().toString(36)}_${i}`,
                name: s,
                color: "#4f8cff",
              }));
            config = { options };
            break;
          }
          case "lookup":
            config = {
              source_link_field_id: sourceLinkFieldId,
              target_field_id: targetFieldId,
            };
            break;
          case "rollup":
            config = {
              source_link_field_id: sourceLinkFieldId,
              target_field_id: targetFieldId,
              function: rollupFunction,
            };
            break;
          case "count":
            config = { source_link_field_id: sourceLinkFieldId };
            break;
          case "formula":
            config = { expression };
            break;
          case "button":
            config = { label: buttonLabel.trim() || name.trim(), url: buttonUrl.trim() };
            break;
        }
        if (required) config.required = true;
        if (unique) config.unique = true;
        await api.createField(dbId, tableId, { name: name.trim(), type, config });
      }
      const f = await api.listFields(dbId, tableId);
      setFields(f);
      queryClient.setQueryData(["fields", dbId, tableId], f);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["fields", dbId, tableId] }),
        queryClient.invalidateQueries({ queryKey: ["table-data", dbId, tableId] }),
      ]);
      closeModal();
    } catch (e) {
      setFields(prevFields);
      queryClient.setQueryData(["fields", dbId, tableId], prevFields);
      setError(formatError(e));
      setBusy(false);
    }
  };

  return (
    <Modal title="Nouveau champ">
      <div className="modal-field">
        <label>Nom du champ</label>
        <input
          value={name}
          autoFocus
          placeholder="Montant"
          onChange={(e) => setName(e.target.value)}
        />
      </div>
      <div className="modal-field">
        <label>Type</label>
        <FieldTypeSelector value={type} onChange={setType} />
      </div>
      {type !== "button" && type !== "lookup" && type !== "rollup" && type !== "count" && type !== "formula" && (
        <div className="modal-field" style={{ flexDirection: "row", gap: 12 }}>
          <label className="modal-check"><input type="checkbox" checked={required} onChange={(e) => setRequired(e.target.checked)} /> Requis</label>
          <label className="modal-check"><input type="checkbox" checked={unique} onChange={(e) => setUnique(e.target.checked)} /> Unique</label>
        </div>
      )}
      {type === "button" && (
        <>
          <div className="modal-field">
            <label>Libellé du bouton</label>
            <input value={buttonLabel} onChange={(e) => setButtonLabel(e.target.value)} placeholder="Ouvrir" />
          </div>
          <div className="modal-field">
            <label>URL (optionnel, {`{recordId}`} sera remplacé)</label>
            <input value={buttonUrl} onChange={(e) => setButtonUrl(e.target.value)} placeholder="https://example.com/{recordId}" />
          </div>
        </>
      )}

      {type === "select" && (
        <div className="modal-field">
          <label>Options (une par ligne)</label>
          <textarea
            rows={5}
            value={optionsText}
            placeholder={"En cours\nTerminé\nBloqué"}
            onChange={(e) => setOptionsText(e.target.value)}
          />
        </div>
      )}

      {type === "link" && (
        <>
          {workspaceConfig && workspaceConfig.databases.length > 1 && (
            <div className="modal-field">
              <label>Base cible</label>
              <select value={targetDbId} onChange={(e) => { setTargetDbId(e.target.value); setTargetTableId(""); }}>
                <option value="">Base active ({workspaceConfig.databases.find((d) => d.id === dbId)?.name ?? dbId})</option>
                {workspaceConfig.databases.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.name} {d.role === "reference" ? "(référentiel)" : ""}
                  </option>
                ))}
              </select>
            </div>
          )}
          <div className="modal-field">
            <label>Table cible</label>
            <select value={targetTableId} onChange={(e) => setTargetTableId(e.target.value)}>
              <option value="">Choisir une table…</option>
              {availableTables.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
            </select>
          </div>
          <div className="modal-field">
            <label>Cardinalité</label>
            <select
              value={cardinality}
              onChange={(e) => setCardinality(e.target.value as "one" | "many")}
            >
              <option value="many">plusieurs</option>
              <option value="one">un seul</option>
            </select>
          </div>
          <label className="modal-check">
            <input
              type="checkbox"
              checked={createBacklink && !targetDbId}
              disabled={!!targetDbId}
              onChange={(e) => setCreateBacklink(e.target.checked)}
            />
            Créer le champ inverse (backlink)
          </label>
          {targetDbId && <span className="hint">Backlink non disponible en cross-DB pour l'instant — le champ inverse ne sera pas créé.</span>}
        </>
      )}

      {(type === "lookup" || type === "rollup" || type === "count") && (
        <div className="modal-field">
          <label>Champ de lien source</label>
          <select
            value={sourceLinkFieldId}
            onChange={(e) => setSourceLinkFieldId(e.target.value)}
          >
            <option value="">Choisir un champ de lien…</option>
            {linkFields.map((f) => (
              <option key={f.id} value={f.id}>
                {f.name}
              </option>
            ))}
          </select>
        </div>
      )}

      {needsTargetField && (
        <div className="modal-field">
          <label>Champ cible (dans la table liée)</label>
          <select
            value={targetFieldId}
            onChange={(e) => setTargetFieldId(e.target.value)}
            disabled={!sourceLinkFieldId || !targetFields.length}
          >
            <option value="">
              {sourceLinkFieldId ? "Choisir un champ…" : "Choisir d'abord le lien source"}
            </option>
            {targetFields.map((f) => (
              <option key={f.id} value={f.id}>
                {f.name}
              </option>
            ))}
          </select>
        </div>
      )}

      {type === "rollup" && (
        <div className="modal-field">
          <label>Fonction</label>
          <select value={rollupFunction} onChange={(e) => setRollupFunction(e.target.value)}>
            {ROLLUP_FUNCTIONS.map((f) => (
              <option key={f.value} value={f.value}>
                {f.label}
              </option>
            ))}
          </select>
        </div>
      )}

      {type === "formula" && (
        <div className="modal-field">
          <label>Expression</label>
          <textarea
            rows={4}
            value={expression}
            placeholder={"IF({Montant} > 50, 'Elevé', 'Bas')"}
            onChange={(e) => setExpression(e.target.value)}
          />
          <span className="hint">
            Références : {"{Champ}"} · IF, SWITCH, AND, OR, CONCATENATE, LEFT/RIGHT/MID,
            SUM, AVERAGE, ROUND, DATETIME_DIFF, ARRAYJOIN…
          </span>
        </div>
      )}

      <div className="launcher-error" role="alert" aria-live="assertive">{error}</div>

      <div className="modal-actions">
        <button onClick={closeModal} disabled={busy}>Annuler</button>
        <button className="primary" disabled={!canSubmit || busy} onClick={() => void submit()}>
          {busy ? "Ajout…" : "Ajouter"}
        </button>
      </div>
    </Modal>
  );
}
