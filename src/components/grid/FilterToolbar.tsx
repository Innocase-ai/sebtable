import { useState } from "react";
import { isStoredField, type Field, type Filter, type ViewConfig } from "../../types/field";
import { useTableStore } from "../../stores/tableStore";

const OPERATORS: { value: string; label: string }[] = [
  { value: "contains", label: "contient" },
  { value: "does_not_contain", label: "ne contient pas" },
  { value: "is", label: "est égal à" },
  { value: "is_not", label: "n'est pas" },
  { value: "gt", label: ">" },
  { value: "gte", label: "≥" },
  { value: "lt", label: "<" },
  { value: "lte", label: "≤" },
  { value: "is_empty", label: "est vide" },
  { value: "is_not_empty", label: "n'est pas vide" },
];

interface Props {
  fields: Field[];
  viewConfig: ViewConfig;
  onChange: (c: ViewConfig) => void;
}

export default function FilterToolbar({ fields, viewConfig, onChange }: Props) {
  const [adding, setAdding] = useState(false);
  const [fieldId, setFieldId] = useState("");
  const [operator, setOperator] = useState("contains");
  const [value, setValue] = useState("");

  const groupFieldId = viewConfig.groups[0]?.field_id ?? "";
  const storedFields = fields.filter(isStoredField);
  // Vues : sélecteur léger (R5) — les vues sont inertes sans ce branchement
  const views = useTableStore((s) => s.views);
  const activeViewId = useTableStore((s) => s.activeViewId);
  const setActiveView = useTableStore((s) => s.setActiveView);

  const setGroup = (gid: string) => {
    onChange({
      ...viewConfig,
      groups: gid ? [{ field_id: gid, order: "asc" }] : [],
    });
  };

  const setConjunction = (v: "and" | "or") => {
    onChange({ ...viewConfig, filter_conjunction: v });
  };

  const addFilter = () => {
    if (!fieldId) return;
    const filter: Filter = { field_id: fieldId, operator, value };
    onChange({ ...viewConfig, filters: [...viewConfig.filters, filter] });
    setAdding(false);
    setFieldId("");
    setOperator("contains");
    setValue("");
  };

  const removeFilter = (i: number) => {
    onChange({
      ...viewConfig,
      filters: viewConfig.filters.filter((_, idx) => idx !== i),
    });
  };

  const needsValue = operator !== "is_empty" && operator !== "is_not_empty";
  const selectedField = fields.find((x) => x.id === fieldId);
  const selectOptions = (selectedField?.config as { options?: { id: string; name: string }[] } | undefined)?.options ?? null;
  const isSelectField = selectedField?.type === "select" && selectOptions && selectOptions.length > 0;

  return (
    <div className="grid-toolbar">
      {views.length > 0 && (
        <>
          <label htmlFor="view-select">Vue</label>
          <select
            id="view-select"
            value={activeViewId ?? ""}
            onChange={(e) => setActiveView(e.target.value || null)}
          >
            <option value="">— (sans vue)</option>
            {views.map((v) => (
              <option key={v.id} value={v.id}>
                {v.name}
              </option>
            ))}
          </select>
        </>
      )}
      <label>Grouper par</label>
      <select value={groupFieldId} onChange={(e) => setGroup(e.target.value)}>
        <option value="">—</option>
        {storedFields.map((f) => (
          <option key={f.id} value={f.id}>
            {f.name}
          </option>
        ))}
      </select>

      {viewConfig.filters.length >= 2 && (
        <span className="field-type-chip" style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
          Logique:
          <select value={viewConfig.filter_conjunction ?? "and"} onChange={(e) => setConjunction(e.target.value as "and" | "or")} aria-label="Logique des filtres">
            <option value="and">ET (tous)</option>
            <option value="or">OU (au moins un)</option>
          </select>
        </span>
      )}
      {viewConfig.filters.map((f, i) => {
        const name = fields.find((x) => x.id === f.field_id)?.name ?? f.field_id;
        return (
          <span key={`${f.field_id}:${f.operator}:${String(f.value ?? "")}:${i}`} className="field-type-chip">
            {name} {OPERATORS.find((o) => o.value === f.operator)?.label} {String(f.value ?? "")}
            <button className="delete-btn" aria-label={`Supprimer le filtre ${name}`} onClick={() => removeFilter(i)}>
              ×
            </button>
          </span>
        );
      })}

      {adding ? (
        <>
          <select
            value={fieldId}
            onChange={(e) => setFieldId(e.target.value)}
          >
            <option value="">Champ…</option>
            {storedFields.map((f) => (
              <option key={f.id} value={f.id}>
                {f.name}
              </option>
            ))}
          </select>
          <select
            value={operator}
            onChange={(e) => setOperator(e.target.value)}
          >
            {OPERATORS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
          {needsValue && (
            isSelectField ? (
              <select value={value} onChange={(e) => setValue(e.target.value)}>
                <option value="">— choisir</option>
                {selectOptions!.map((o) => (
                  <option key={o.id} value={o.id}>{o.name}</option>
                ))}
              </select>
            ) : (
              <input value={value} onChange={(e) => setValue(e.target.value)} placeholder="valeur…" data-shortcut="search" />
            )
          )}
          <button onClick={addFilter} disabled={!fieldId || (needsValue && !value)}>
            Ajouter
          </button>
          <button onClick={() => setAdding(false)}>Annuler</button>
        </>
      ) : (
        <button onClick={() => setAdding(true)}>+ Filtre</button>
      )}
    </div>
  );
}
