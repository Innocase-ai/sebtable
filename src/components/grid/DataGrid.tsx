import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  flexRender,
  getCoreRowModel,
  useReactTable,
  type Row,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";

import { useTableStore } from "../../stores/tableStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useUiStore } from "../../stores/uiStore";
import { useQueryClient } from "@tanstack/react-query";
import {
  useTableData,
  useUpsertRecords,
  useDeleteRecords,
} from "../../hooks/useTableData";
import * as api from "../../lib/api";
import { buildColumns, type GridMeta } from "./ColumnDefFactory";
import FilterToolbar from "./FilterToolbar";
import ColumnVisibilityMenu from "./ColumnVisibilityMenu";
import InlineAddField from "./InlineAddField";
import RecordDetailModal from "./RecordDetailModal";
import type { Record } from "../../types/record";

type DisplayItem =
  | { type: "group"; key: string; label: string }
  | { type: "row"; key: string; row: Row<Record> };

function groupLabel(v: unknown, field?: import("../../types/field").Field): string {
  if (v === null || v === undefined || v === "") return "";
  if (field?.type === "select") {
    const opts = (field.config as { options?: { id: string; name: string }[] } | undefined)?.options ?? [];
    const found = opts.find((o) => o.id === String(v));
    if (found) return found.name;
  }
  if (field?.type === "checkbox") return v ? "✓" : "☐";
  return String(v);
}

export default function DataGrid() {
  const fields = useTableStore((s) => s.fields);
  const setFields = useTableStore((s) => s.setFields);
  const tableId = useTableStore((s) => s.activeTableId);
  const viewConfig = useTableStore((s) => s.viewConfig);
  const setViewConfig = useTableStore((s) => s.setViewConfig);
  const openModal = useUiStore((s) => s.openModal);
  const queryClient = useQueryClient();

  const { data, isLoading, isFetching, error, isError } = useTableData();
  const upsert = useUpsertRecords();
  const deleteRows = useDeleteRecords();

  const records = useMemo<Record[]>(
    () => (data?.records ?? []) as Record[],
    [data]
  );

  const sortDir = useCallback(
    (fieldId: string): "asc" | "desc" | null => {
      const s = viewConfig.sorts.find((x) => x.field_id === fieldId);
      return s ? s.direction : null;
    },
    [viewConfig.sorts]
  );

  const onToggleSort = useCallback(
    (fieldId: string) => {
      const cur = sortDir(fieldId);
      const sorts = viewConfig.sorts.filter((s) => s.field_id !== fieldId);
      if (cur !== "desc") {
        sorts.push({
          field_id: fieldId,
          direction: (cur === "asc" ? "desc" : "asc") as "asc" | "desc",
        });
      }
      setViewConfig({ ...viewConfig, sorts });
    },
    [sortDir, viewConfig, setViewConfig]
  );

  const [commitError, setCommitError] = useState<string | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const onDeleteRow = useCallback(
    async (recordId: string) => {
      setDeleteError(null);
      try {
        await deleteRows([recordId]);
      } catch (e) {
        setDeleteError(String((e as Error)?.message ?? e));
      }
    },
    [deleteRows]
  );
  const [inlineAddOpen, setInlineAddOpen] = useState(false);
  const onAddField = useCallback(() => setInlineAddOpen((o) => !o), []);
  const onAddFieldAdvanced = useCallback(() => openModal("createField"), [openModal]);

  const visibleFields = useMemo(() => {
    const ids = viewConfig.visible_field_ids;
    if (!ids || ids.length === 0) return fields;
    const set = new Set(ids);
    return fields.filter((f) => set.has(f.id));
  }, [fields, viewConfig.visible_field_ids]);

  const columns = useMemo(() => buildColumns(visibleFields), [visibleFields]);

  const [columnSizing, setColumnSizing] = useState<globalThis.Record<string, number>>({});

  const [addError, setAddError] = useState<string | null>(null);
  const [isAdding, setIsAdding] = useState(false);
  const [highlightId, setHighlightId] = useState<string | null>(null);
  // 3a — sélection cellule + range (Shift)
  const [selected, setSelected] = useState<{ recordId: string; fieldId: string; rowIdx: number; colIdx: number } | null>(null);
  const [anchor, setAnchor] = useState<{ rowIdx: number; colIdx: number } | null>(null);
  const [draggedColId, setDraggedColId] = useState<string | null>(null);

  useEffect(() => {
    if (highlightId && !records.some((r) => (r as { _id: string })._id === highlightId)) {
      setHighlightId(null);
    }
  }, [records, highlightId]);

  const [quickSearch, setQuickSearch] = useState("");
  // 3a/3b/3d — sélection + clipboard + undo
  const [detailRecordId, setDetailRecordId] = useState<string | null>(null);
  type HistEntry = { recordId: string; fieldId: string; oldValue: unknown; newValue: unknown };
  const [undoStack, setUndoStack] = useState<HistEntry[]>([]);
  const [redoStack, setRedoStack] = useState<HistEntry[]>([]);

  const visibleFieldIds = useMemo(() => visibleFields.map((f) => f.id), [visibleFields]);
  const isSelected = (rid: string, fid: string) => selected?.recordId === rid && selected?.fieldId === fid;
  const isInRange = (rowIdx: number, colIdx: number) => {
    if (!selected || !anchor) return false;
    const r1 = Math.min(anchor.rowIdx, selected.rowIdx);
    const r2 = Math.max(anchor.rowIdx, selected.rowIdx);
    const c1 = Math.min(anchor.colIdx, selected.colIdx);
    const c2 = Math.max(anchor.colIdx, selected.colIdx);
    return rowIdx >= r1 && rowIdx <= r2 && colIdx >= c1 && colIdx <= c2;
  };
  const selectCell = (recordId: string, fieldId: string, rowIdx: number, colIdx: number, shift: boolean) => {
    if (shift && anchor) {
      setSelected({ recordId, fieldId, rowIdx, colIdx });
    } else {
      setAnchor({ rowIdx, colIdx });
      setSelected({ recordId, fieldId, rowIdx, colIdx });
    }
  };
  const moveSelection = (dr: number, dc: number, shift: boolean) => {
    if (!selected) return;
    const nr = Math.max(0, Math.min(records.length - 1, selected.rowIdx + dr));
    const nc = Math.max(0, Math.min(visibleFieldIds.length - 1, selected.colIdx + dc));
    const rid = (records[nr] as Record)?._id ?? "";
    const fid = visibleFieldIds[nc] ?? "";
    if (rid && fid) selectCell(rid, fid, nr, nc, shift);
  };
  const reorderColumn = async (dragId: string, dropId: string) => {
    if (dragId === dropId) return;
    const fieldsOrder = [...fields];
    const fromIdx = fieldsOrder.findIndex((f) => f.id === dragId);
    const toIdx = fieldsOrder.findIndex((f) => f.id === dropId);
    if (fromIdx === -1 || toIdx === -1) return;
    const [moved] = fieldsOrder.splice(fromIdx, 1);
    fieldsOrder.splice(toIdx, 0, moved);
    // persiste les positions côté backend (sinon retour à l'ordre DB au refetch)
    const dbId = useWorkspaceStore.getState().config?.active_database_id;
    if (dbId) {
      try {
        for (let i = 0; i < fieldsOrder.length; i++) {
          if (fieldsOrder[i].position !== i) {
            await api.updateField(dbId, fieldsOrder[i].id, { position: i });
          }
        }
        void queryClient.invalidateQueries({ queryKey: ["fields", dbId, tableId] });
      } catch {
        // best-effort : on garde l'ordre local même si la persistance échoue
      }
    }
    setFields(fieldsOrder);
    if (viewConfig.visible_field_ids && viewConfig.visible_field_ids.length > 0) {
      const v = [...viewConfig.visible_field_ids];
      const fi = v.indexOf(dragId);
      const ti = v.indexOf(dropId);
      if (fi !== -1 && ti !== -1) {
        v.splice(fi, 1);
        v.splice(ti, 0, dragId);
        setViewConfig({ ...viewConfig, visible_field_ids: v });
      }
    }
  };
  const handleUndo = async () => {
    const e = undoStack[undoStack.length - 1];
    if (!e) return;
    setUndoStack((s) => s.slice(0, -1));
    setRedoStack((s) => [...s, e]);
    try { await upsert([{ _id: e.recordId, [e.fieldId]: e.oldValue }]); } catch {}
  };
  const handleRedo = async () => {
    const e = redoStack[redoStack.length - 1];
    if (!e) return;
    setRedoStack((s) => s.slice(0, -1));
    setUndoStack((s) => [...s, e]);
    try { await upsert([{ _id: e.recordId, [e.fieldId]: e.newValue }]); } catch {}
  };
  const handleCopy = async () => {
    if (!selected || !anchor) {
      if (selected) {
        const rec = records.find((r) => (r as Record)._id === selected.recordId) as Record | undefined;
        const field = visibleFields.find((f) => f.id === selected.fieldId);
        const { formatValue: fmt } = await import("./Cell");
        const txt = field && rec ? fmt(field, rec[field.id]) : "";
        await navigator.clipboard.writeText(txt);
      }
      return;
    }
    const r1 = Math.min(anchor.rowIdx, selected.rowIdx);
    const r2 = Math.max(anchor.rowIdx, selected.rowIdx);
    const c1 = Math.min(anchor.colIdx, selected.colIdx);
    const c2 = Math.max(anchor.colIdx, selected.colIdx);
    const { formatValue: fmt } = await import("./Cell");
    const rows: string[] = [];
    for (let r = r1; r <= r2; r++) {
      const rec = records[r] as Record | undefined;
      if (!rec) continue;
      const cols: string[] = [];
      for (let c = c1; c <= c2; c++) {
        const fid = visibleFieldIds[c];
        const field = visibleFields.find((f) => f.id === fid);
        cols.push(field && rec ? fmt(field, rec[fid]) : "");
      }
      rows.push(cols.join("\t"));
    }
    await navigator.clipboard.writeText(rows.join("\n"));
  };
  const handlePaste = async () => {
    if (!selected) return;
    try {
      const txt = await navigator.clipboard.readText();
      if (!txt) return;
      const lines = txt.split(/\r?\n/).filter((l) => l.length > 0);
      const startR = selected.rowIdx;
      const startC = selected.colIdx;
      const payload: Record[] = [];
      for (let i = 0; i < lines.length; i++) {
        const cols = lines[i].split("\t");
        const rIdx = startR + i;
        let rec: Record | undefined;
        let isNew = false;
        if (rIdx < records.length) rec = records[rIdx] as Record;
        else {
          // dépassement : création de nouvelles lignes (paste qui déborde)
          rec = { _id: `rec_${Math.random().toString(36).slice(2, 10)}` } as unknown as Record;
          isNew = true;
        }
        for (let j = 0; j < cols.length; j++) {
          const cIdx = startC + j;
          if (cIdx >= visibleFieldIds.length) break;
          const fid = visibleFieldIds[cIdx];
          const field = visibleFields.find((f) => f.id === fid);
          if (!field || field.type === "link" || field.type === "attachment" || field.type === "lookup" || field.type === "rollup" || field.type === "count" || field.type === "formula" || field.type === "button") continue;
          let cellValue: unknown = cols[j];
          if (field.type === "select") {
            const opts = (field.config as { options?: { id: string; name: string }[] } | undefined)?.options ?? [];
            const byName = opts.find((o) => o.name.toLowerCase() === cols[j].trim().toLowerCase());
            const byId = opts.find((o) => o.id === cols[j].trim());
            if (byName) cellValue = byName.id;
            else if (byId) cellValue = byId.id;
            else continue;
          }
          if (isNew) {
            (rec as Record)[fid] = cellValue as never;
          } else {
            payload.push({ _id: (rec as Record)._id, [fid]: cellValue } as unknown as Record);
          }
        }
        if (isNew && Object.keys(rec as Record).length > 1) payload.push(rec);
      }
      if (payload.length) {
        const byId = new Map<string, Record>();
        for (const p of payload) {
          const id = (p as Record)._id;
          const cur = byId.get(id) ?? { _id: id } as Record;
          Object.assign(cur, p);
          byId.set(id, cur);
        }
        await upsert(Array.from(byId.values()) as Record[]);
      }
    } catch {}
  };

  const onCommit = useCallback(
    (recordId: string, fieldId: string, value: unknown) => {
      setCommitError(null);
      const rec = records.find((r) => (r as Record)._id === recordId) as Record | undefined;
      const oldValue = rec ? (rec as Record)[fieldId] : undefined;
      if (oldValue !== value) {
        setUndoStack((s) => [...s, { recordId, fieldId, oldValue, newValue: value }]);
        setRedoStack([]);
      }
      void upsert([{ _id: recordId, [fieldId]: value }]).catch((e) => {
        setCommitError(String((e as Error)?.message ?? e));
      });
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [upsert, records]
  );

  const meta: GridMeta = useMemo(
    () => ({ onCommit, onToggleSort, sortDir, onDeleteRow, onAddField }),
    [onCommit, onToggleSort, sortDir, onDeleteRow, onAddField]
  );

  const table = useReactTable({
    data: records,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getRowId: (row, index) => (row as Record)?._id ?? String(index),
    manualPagination: true,
    manualSorting: true,
    manualFiltering: true,
    meta,
    enableColumnResizing: true,
    columnResizeMode: "onChange",
    onColumnSizingChange: setColumnSizing,
    state: { columnSizing },
  });

  const groupFieldId = viewConfig.groups[0]?.field_id ?? null;

  const groupField = useMemo(
    () => fields.find((f) => f.id === groupFieldId) ?? null,
    [fields, groupFieldId]
  );

  const rowModelRows = table.getRowModel().rows;
  const displayRows: DisplayItem[] = useMemo(() => {
    if (!groupFieldId) {
      return rowModelRows.map((row) => ({ type: "row", key: (row.original as Record)?._id ?? row.id, row }));
    }
    const items: DisplayItem[] = [];
    let lastKey: string | null = null;
    for (const row of rowModelRows) {
      const raw = row.original[groupFieldId];
      const label = groupLabel(raw, groupField ?? undefined);
      const key = groupFieldId + "::" + String(raw ?? "");
      if (key !== lastKey) {
        items.push({ type: "group", key, label: label || "(vide)" });
        lastKey = key;
      }
      items.push({ type: "row", key: (row.original as Record)?._id ?? row.id, row });
    }
    return items;
  }, [rowModelRows, groupFieldId, groupField]);

  const scrollRef = useRef<HTMLDivElement>(null);
  const rowVirtualizer = useVirtualizer({
    count: displayRows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 32,
    overscan: 12,
  });
  const virtualItems = rowVirtualizer.getVirtualItems();

  const shouldVirtualize = displayRows.length > 50;
  const useVirtual = shouldVirtualize && virtualItems.length > 0;
  useEffect(() => {
    if (scrollRef.current) rowVirtualizer.measure();
  }, [displayRows.length, rowVirtualizer]);

  const page = viewConfig.page?.number ?? 1;
  const pageSize = viewConfig.page?.size ?? 100;
  const total = data?.total ?? 0;

  const setPage = (n: number) => {
    setViewConfig({ ...viewConfig, page: { number: n, size: pageSize } });
  };

  const addRow = async () => {
    const id = `rec_${Math.random().toString(36).slice(2, 12)}`;
    setAddError(null);
    setIsAdding(true);
    setHighlightId(id);
    setTimeout(() => setHighlightId((cur) => (cur === id ? null : cur)), 2000);
    try {
      await upsert([{ _id: id }]);
      requestAnimationFrame(() => {
        const el = document.getElementById(`row-${id}`);
        if (el) el.scrollIntoView({ behavior: "smooth", block: "center" });
        else scrollRef.current?.scrollTo({ top: 0, behavior: "smooth" });
      });
    } catch (e) {
      setAddError(String((e as Error)?.message ?? e));
      setHighlightId(null);
    } finally {
      setIsAdding(false);
    }
  };

  return (
    <>
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "6px 10px", borderBottom: "1px solid var(--border)", background: "var(--bg-panel)", flexWrap: "wrap" }}>
        <input value={quickSearch} onChange={(e) => setQuickSearch(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter" && quickSearch.trim()) { const target = fields.find((f) => f.type === "text" || f.type === "long_text"); if (target) { const base = viewConfig.filters.filter((f) => f.field_id !== target.id || f.operator !== "contains"); setViewConfig({ ...viewConfig, filters: [...base, { field_id: target.id, operator: "contains", value: quickSearch.trim() }] }); } } if (e.key === "Escape") setQuickSearch(""); }} placeholder="Filtre rapide — Entrée" aria-label="Filtre rapide" style={{ minWidth: 180, flex: "0 0 200px" }} />
        <FilterToolbar
          fields={fields}
          viewConfig={viewConfig}
          onChange={setViewConfig}
        />
        <span style={{ flex: 1 }} />
        <ColumnVisibilityMenu fields={fields} />
        <button onClick={onAddFieldAdvanced} title="Création avancée (lien, formule…)">+ Champ avancé</button>
      </div>
      {inlineAddOpen && (
        <div style={{ position: "relative", borderBottom: "1px solid var(--border)", background: "var(--bg-header)" }}>
          <InlineAddField onClose={() => setInlineAddOpen(false)} />
          <div style={{ padding: "6px 10px", fontSize: 11, color: "var(--text-muted)" }}>Ajout rapide — tape un nom, le type est suggéré. <button onClick={onAddFieldAdvanced} style={{ fontSize: 11 }}>Passer en avancé</button></div>
        </div>
      )}

      <div
        className="grid-scroll"
        ref={scrollRef}
        tabIndex={0}
        onKeyDown={(e) => {
          const target = e.target as HTMLElement;
          if (target instanceof HTMLInputElement || target instanceof HTMLSelectElement || target instanceof HTMLTextAreaElement) return;
          if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c") { e.preventDefault(); void handleCopy(); return; }
          if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "v") { e.preventDefault(); void handlePaste(); return; }
          if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z" && !e.shiftKey) { e.preventDefault(); void handleUndo(); return; }
          if ((e.ctrlKey || e.metaKey) && (e.key.toLowerCase() === "y" || (e.key.toLowerCase() === "z" && e.shiftKey))) { e.preventDefault(); void handleRedo(); return; }
          if (!selected) return;
          if (e.key === "ArrowUp") { e.preventDefault(); moveSelection(-1, 0, e.shiftKey); }
          if (e.key === "ArrowDown") { e.preventDefault(); moveSelection(1, 0, e.shiftKey); }
          if (e.key === "ArrowLeft") { e.preventDefault(); moveSelection(0, -1, e.shiftKey); }
          if (e.key === "ArrowRight") { e.preventDefault(); moveSelection(0, 1, e.shiftKey); }
          if (e.key === "Enter" && selected) { const el = document.querySelector(`[data-cell="${selected.recordId}:${selected.fieldId}"]`) as HTMLElement | null; el?.click(); }
          if (e.key === "Escape") { setSelected(null); setAnchor(null); }
        }}
      >
        <table
          className="grid-table"
          style={{ width: table.getTotalSize(), minWidth: "100%" }}
        >
          <thead>
            {table.getHeaderGroups().map((hg) => (
              <tr key={hg.id}>
                {hg.headers.map((h) => {
                  const isDataCol = visibleFieldIds.includes(h.id);
                  return (
                  <th
                    key={h.id}
                    draggable={isDataCol}
                    onDragStart={() => isDataCol && setDraggedColId(h.id)}
                    onDragOver={(e) => { if (isDataCol && draggedColId && draggedColId !== h.id) { e.preventDefault(); (e.currentTarget as HTMLElement).style.outline = "2px dashed var(--accent)"; } }}
                    onDragLeave={(e) => { (e.currentTarget as HTMLElement).style.outline = ""; }}
                    onDrop={(e) => {
                      e.preventDefault();
                      (e.currentTarget as HTMLElement).style.outline = "";
                      if (isDataCol && draggedColId && draggedColId !== h.id) void reorderColumn(draggedColId, h.id);
                      setDraggedColId(null);
                    }}
                    onDragEnd={() => setDraggedColId(null)}
                    style={{ width: h.getSize(), position: "relative", opacity: draggedColId === h.id ? 0.5 : 1, cursor: isDataCol ? "grab" : undefined }}
                    title={isDataCol ? "Glisser pour réordonner la colonne" : undefined}
                  >
                    {flexRender(h.column.columnDef.header, h.getContext())}
                    {h.column.getCanResize() && (
                      <div
                        onMouseDown={h.getResizeHandler()}
                        onTouchStart={h.getResizeHandler()}
                        style={{ position: "absolute", right: 0, top: 0, bottom: 0, width: 6, cursor: "col-resize", userSelect: "none", touchAction: "none" }}
                        onDoubleClick={() => h.column.resetSize()}
                        title="Glisser pour redimensionner — double-clic pour réinitialiser"
                      />
                    )}
                  </th>
                  );
                })}
              </tr>
            ))}
          </thead>
          <tbody
            style={
              useVirtual
                ? { height: rowVirtualizer.getTotalSize(), position: "relative" }
                : undefined
            }
          >
            {useVirtual
              ? virtualItems.map((vi) => {
                  const item = displayRows[vi.index];
                  if (item.type === "group") {
                    return (
                      <tr
                        key={item.key}
                        className="grid-row group-row"
                        style={{
                          transform: `translateY(${vi.start}px)`,
                          position: "absolute",
                          top: 0,
                          left: 0,
                          width: table.getTotalSize(),
                          height: 32,
                          display: "table",
                          tableLayout: "fixed",
                        }}
                      >
                        <td colSpan={table.getAllLeafColumns().length}>{item.label}</td>
                      </tr>
                    );
                  }
                  const row = item.row;
                  const rid = row.original._id;
                  return (
                    <tr
                      key={item.key}
                      id={`row-${rid}`}
                      className={"grid-row" + (highlightId === rid ? " highlighted" : "")}
                      style={{
                        transform: `translateY(${vi.start}px)`,
                        position: "absolute",
                        top: 0,
                        left: 0,
                        width: table.getTotalSize(),
                        height: 32,
                        display: "table",
                        tableLayout: "fixed",
                      }}
                    >
                      {row.getVisibleCells().map((cell) => {
                        const fid = cell.column.id;
                        const isData = visibleFieldIds.includes(fid);
                        const colIdx = isData ? visibleFieldIds.indexOf(fid) : -1;
                        const rowIdx = row.index;
                        const rid2 = (row.original as Record)._id;
                        const sel = isData && (isSelected(rid2, fid) || isInRange(rowIdx, colIdx));
                        return (
                          <td
                            key={cell.id}
                            data-cell={`${rid2}:${fid}`}
                            onClick={(e) => { if (isData) selectCell(rid2, fid, rowIdx, colIdx, e.shiftKey); }}
                            onDoubleClick={() => setDetailRecordId(rid2)}
                            style={{ width: cell.column.getSize(), background: sel ? "rgba(109,123,255,0.18)" : undefined, outline: sel ? "1px solid var(--accent)" : undefined, outlineOffset: "-1px", cursor: isData ? "cell" : undefined }}
                          >
                            {flexRender(cell.column.columnDef.cell, cell.getContext())}
                          </td>
                        );
                      })}
                    </tr>
                  );
                })
              : displayRows.map((item) =>
                  item.type === "group" ? (
                    <tr key={item.key} className="grid-row group-row" style={{ height: 32 }}>
                      <td colSpan={table.getAllLeafColumns().length}>{item.label}</td>
                    </tr>
                  ) : (
                    <tr
                      key={item.key}
                      id={`row-${item.row.original._id}`}
                      className={"grid-row" + (highlightId === item.row.original._id ? " highlighted" : "")}
                      style={{ height: 32 }}
                    >
                      {item.row.getVisibleCells().map((cell) => {
                        const fid = cell.column.id;
                        const isData = visibleFieldIds.includes(fid);
                        const colIdx = isData ? visibleFieldIds.indexOf(fid) : -1;
                        const rowIdx = item.row.index;
                        const rid2 = (item.row.original as Record)._id;
                        const sel = isData && (isSelected(rid2, fid) || isInRange(rowIdx, colIdx));
                        return (
                          <td
                            key={cell.id}
                            data-cell={`${rid2}:${fid}`}
                            onClick={(e) => { if (isData) selectCell(rid2, fid, rowIdx, colIdx, e.shiftKey); }}
                            onDoubleClick={() => setDetailRecordId(rid2)}
                            style={{ width: cell.column.getSize(), background: sel ? "rgba(109,123,255,0.18)" : undefined, outline: sel ? "1px solid var(--accent)" : undefined, outlineOffset: "-1px", cursor: isData ? "cell" : undefined }}
                          >
                            {flexRender(cell.column.columnDef.cell, cell.getContext())}
                          </td>
                        );
                      })}
                    </tr>
                  )
                )}
          </tbody>
        </table>

        {deleteError && (
          <div className="grid-overlay" role="alert" aria-live="assertive" style={{ color: "var(--danger)" }}>
            Suppression échouée : {deleteError} <button onClick={() => setDeleteError(null)}>×</button>
          </div>
        )}
        {addError && (
          <div className="grid-overlay" role="alert" aria-live="assertive" style={{ color: "var(--danger)" }}>
            Ajout échoué : {addError} <button onClick={() => setAddError(null)}>×</button>
          </div>
        )}
        {commitError && (
          <div className="grid-overlay" role="alert" aria-live="assertive" style={{ color: "var(--danger)" }}>
            Enregistrement échoué : {commitError} <button onClick={() => setCommitError(null)}>×</button>
          </div>
        )}
        {isError && (
          <div className="grid-overlay" role="alert" aria-live="assertive" style={{ color: "var(--danger)" }}>
            Erreur : {String((error as Error)?.message ?? error)}
          </div>
        )}
        {isLoading && !isError && (
          <div className="grid-overlay" aria-live="polite">Chargement…</div>
        )}
        {isFetching && !isLoading && (
          <div
            style={{
              position: "absolute",
              top: 6,
              right: 12,
              fontSize: 11,
              color: "var(--text-muted)",
              background: "var(--bg-panel)",
              border: "1px solid var(--border)",
              borderRadius: 999,
              padding: "2px 8px",
              display: "flex",
              alignItems: "center",
              gap: 6,
            }}
            aria-live="polite"
          >
            <span style={{ width: 8, height: 8, borderRadius: 999, background: "var(--accent)", display: "inline-block", animation: "pulse 1s infinite" }} />
            Actualisation…
          </div>
        )}
        {!isLoading && !isError && records.length === 0 && (
          <div className="grid-overlay">Aucun enregistrement</div>
        )}
      </div>

      <div className="grid-add-row">
        <button
          className="grid-add-btn"
          onClick={() => void addRow()}
          disabled={isAdding}
          aria-busy={isAdding}
          aria-label="Ajouter une ligne"
          title="Ajouter une ligne"
        >
          {isAdding ? "…" : "+"}
        </button>
      </div>

      <div className="grid-footer">
        <span>{total} enregistrement(s)</span>
        {selected && <span style={{ fontSize: 11, color: "var(--text-muted)" }}>· {visibleFields.find((f) => f.id === selected.fieldId)?.name ?? selected.fieldId} sélectionné {anchor && (anchor.rowIdx !== selected.rowIdx || anchor.colIdx !== selected.colIdx) ? `· plage ${Math.abs(anchor.rowIdx - selected.rowIdx) + 1}×${Math.abs(anchor.colIdx - selected.colIdx) + 1}` : ""} · Ctrl+C/V · double-clic fiche</span>}
        <div className="spacer" />
        <button disabled={undoStack.length === 0} onClick={() => void handleUndo()} title="Annuler (Ctrl+Z)">↩ Annuler</button>
        <button disabled={redoStack.length === 0} onClick={() => void handleRedo()} title="Rétablir (Ctrl+Y)">↪ Rétablir</button>
        <span style={{ opacity: 0.3 }}>|</span>
        <button disabled={page <= 1} onClick={() => setPage(page - 1)}>
          Précédent
        </button>
        <span>
          Page {page} / {Math.max(1, Math.ceil(total / pageSize))}
        </span>
        <button
          disabled={page * pageSize >= total}
          onClick={() => setPage(page + 1)}
        >
          Suivant
        </button>
      </div>
      {detailRecordId && <RecordDetailModal recordId={detailRecordId} onClose={() => setDetailRecordId(null)} />}
    </>
  );
}
