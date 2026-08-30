import { useMemo, useState } from "react";
import { DndContext, closestCenter, PointerSensor, useSensor, useSensors, useDroppable, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { useTableStore } from "../../stores/tableStore";
import { useTableData, useUpsertRecords } from "../../hooks/useTableData";
import { formatValue } from "./Cell";
import type { Field } from "../../types/field";
import type { Record as TableRecord } from "../../types/record";
import SortableKanbanCard from "./SortableKanbanCard";

// Colonne droppable : permet de déposer une carte dans une colonne vide
// (sans carte, il n'y a pas de SortableContext pour capter le drop).
function KanbanColumn({ id, label, count, children }: { id: string; label: string; count: number; children: React.ReactNode }) {
  const { setNodeRef, isOver } = useDroppable({ id });
  return (
    <div
      ref={setNodeRef}
      data-column-id={id}
      style={{ minWidth: 240, maxWidth: 300, flex: "0 0 260px", background: "var(--bg-panel)", border: `1px solid ${isOver ? "var(--accent)" : "var(--border)"}`, borderRadius: 10, display: "flex", flexDirection: "column", maxHeight: "100%" }}
    >
      <div style={{ padding: "8px 10px", fontWeight: 600, borderBottom: "1px solid var(--border)", display: "flex", justifyContent: "space-between" }}>
        <span>{label}</span><span className="hint">{count}</span>
      </div>
      {children}
    </div>
  );
}

function groupOptions(field?: Field): { id: string; label: string }[] {
  if (!field) return [];
  if (field.type === "select") {
    const opts = (field.config as { options?: { id: string; name: string }[] })?.options ?? [];
    return opts.map((o) => ({ id: o.id, label: o.name }));
  }
  return [];
}

export default function KanbanView() {
  const fields = useTableStore((s) => s.fields);
  const viewConfig = useTableStore((s) => s.viewConfig);
  const setViewConfig = useTableStore((s) => s.setViewConfig);
  const { data } = useTableData();
  const upsert = useUpsertRecords();

  const records = useMemo(() => (data?.records ?? []) as TableRecord[], [data]);

  // choix du champ de groupe : par défaut le même que le DataGrid, sinon 1er select
  const groupFieldId = viewConfig.groups[0]?.field_id ?? fields.find((f) => f.type === "select")?.id ?? null;
  const groupField = fields.find((f) => f.id === groupFieldId) ?? null;
  const [dragError, setDragError] = useState<string | null>(null);

  const columns = useMemo(() => {
    if (!groupField) return [{ id: "__none", label: "(tous)", records }];
    const opts = groupOptions(groupField);
    if (opts.length === 0) {
      const vals = Array.from(new Set(records.map((r) => String(r[groupField.id] ?? ""))));
      return vals.map((v) => ({ id: v || "__empty", label: v || "(vide)", records: records.filter((r) => String(r[groupField.id] ?? "") === v) }));
    }
    const cols = opts.map((o) => ({ id: o.id, label: o.label, records: records.filter((r) => String(r[groupField.id] ?? "") === o.id) }));
    // records dont le select est vide / null / id d'option obsolète : colonne (vide)
    const known = new Set(opts.map((o) => o.id));
    const rest = records.filter((r) => !known.has(String(r[groupField.id] ?? "")));
    if (rest.length > 0) cols.push({ id: "__empty", label: "(vide)", records: rest });
    return cols;
  }, [records, groupField]);

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));

  const handleDragEnd = async (e: DragEndEvent) => {
    const { active, over } = e;
    if (!over || !groupField) return;
    const activeId = String(active.id);
    const overId = String(over.id);
    // active.id = recordId, over.id = colId ou recordId
    // on récupère la colonne cible via over.data ou en cherchant le record over
    const activeRec = records.find((r) => String((r as TableRecord)._id) === activeId);
    if (!activeRec) return;
    let targetGroupValue: string | null = null;
    const overIsColumn = columns.some((c) => c.id === overId);
    if (overIsColumn) targetGroupValue = overId === "__none" || overId === "__empty" ? "" : overId;
    else {
      const overRec = records.find((r) => String((r as TableRecord)._id) === overId);
      if (overRec) targetGroupValue = String((overRec as TableRecord)[groupField.id] ?? "");
      else targetGroupValue = overId;
    }
    if (targetGroupValue === null) return;
    const current = String((activeRec as TableRecord)[groupField.id] ?? "");
    if (current === targetGroupValue) return;
    setDragError(null);
    try {
      await upsert([{ _id: String((activeRec as TableRecord)._id), [groupField.id]: targetGroupValue } as unknown as TableRecord]);
    } catch (err) {
      setDragError(String((err as Error)?.message ?? err));
    }
  };

  if (!groupField) {
    return <div className="grid-overlay">Kanban : aucun champ "Sélection" pour grouper. Crée un champ de type Sélection puis choisis-le comme "Grouper par" dans la barre d'outils.</div>;
  }

  return (
    <div style={{ flex: 1, display: "flex", flexDirection: "column", minHeight: 0 }}>
      <div style={{ padding: "6px 10px", display: "flex", gap: 8, alignItems: "center", borderBottom: "1px solid var(--border)", background: "var(--bg-panel)" }}>
        <span className="hint">Grouper par</span>
        <select value={groupFieldId ?? ""} onChange={(e) => setViewConfig({ ...viewConfig, groups: e.target.value ? [{ field_id: e.target.value, order: "asc" }] : [] })}>
          {fields.filter((f) => f.type === "select").map((f) => <option key={f.id} value={f.id}>{f.name}</option>)}
          {fields.filter((f) => f.type !== "select").slice(0, 3).map((f) => <option key={f.id} value={f.id}>{f.name} (texte)</option>)}
        </select>
        <span className="hint" style={{ marginLeft: 8 }}>Glisser une carte pour changer de colonne</span>
        {dragError && <span role="alert" style={{ color: "var(--danger)", fontSize: 11 }}>{dragError}</span>}
      </div>
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <div style={{ flex: 1, display: "flex", gap: 12, padding: 12, overflowX: "auto", overflowY: "auto" }}>
          {columns.map((col) => (
            <KanbanColumn key={col.id} id={col.id} label={col.label} count={col.records.length}>
              <SortableContext items={col.records.map((r) => String((r as TableRecord)._id))} strategy={verticalListSortingStrategy}>
                <div style={{ flex: 1, overflowY: "auto", padding: 8, display: "flex", flexDirection: "column", gap: 8 }}>
                  {col.records.map((rec) => {
                    const rid = String((rec as TableRecord)._id);
                    const titleField = fields.find((f) => f.type === "text") ?? fields[0];
                    const title = titleField ? formatValue(titleField, (rec as TableRecord)[titleField.id]) : rid.slice(0, 8);
                    return <SortableKanbanCard key={rid} id={rid} title={title || rid.slice(0, 8)} record={rec as TableRecord} fields={fields} />;
                  })}
                  {col.records.length === 0 && <span className="hint" style={{ textAlign: "center", padding: 12 }}>Aucun</span>}
                </div>
              </SortableContext>
            </KanbanColumn>
          ))}
        </div>
      </DndContext>
    </div>
  );
}
