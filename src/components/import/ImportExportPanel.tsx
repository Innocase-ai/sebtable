import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import * as api from "../../lib/api";
import { formatError } from "../../lib/formatError";

function download(bytes: number[], filename: string, mime: string) {
  const blob = new Blob([new Uint8Array(bytes)], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url; a.download = filename; a.click();
  URL.revokeObjectURL(url);
}

export default function ImportExportPanel() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id ?? "");
  const tableId = useTableStore((s) => s.activeTableId);
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const doExport = async (fmt: "csv"|"json"|"xlsx") => {
    if (!tableId) return;
    setBusy(true); setErr(null); setMsg(null);
    try {
      const bytes = await api.exportTable(dbId, tableId, fmt);
      const mime = fmt === "csv" ? "text/csv" : fmt === "json" ? "application/json" : "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
      download(bytes, `export_${tableId}.${fmt}`, mime);
      setMsg(`Export ${fmt.toUpperCase()} OK (${bytes.length} o)`);
    } catch (e) { setErr(formatError(e)); }
    setBusy(false);
  };

  const doImport = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file || !tableId) return;
    setBusy(true); setErr(null); setMsg(null);
    try {
      const buf = await file.arrayBuffer();
      const ext = file.name.split(".").pop()?.toLowerCase() ?? "csv";
      const fmt = ext === "xlsx" || ext === "xls" ? "xlsx" : ext === "json" ? "json" : "csv";
      const res = await api.importTable(dbId, Array.from(new Uint8Array(buf)), { format: fmt, tableId, hasHeader: true });
      setMsg(`Import OK : ${res.imported_rows} lignes → table ${res.table_id}${res.errors.length ? ` (${res.errors.length} avertissements)` : ""}`);
      await queryClient.invalidateQueries({ queryKey: ["table-data", dbId, tableId] });
      if (res.table_id !== tableId) {
        await queryClient.invalidateQueries({ queryKey: ["tables", dbId] });
      }
    } catch (ex) { setErr(formatError(ex)); }
    setBusy(false);
    e.target.value = "";
  };

  if (!tableId) return <p className="hint">Sélectionne une table.</p>;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
        <button onClick={() => doExport("csv")} disabled={busy}>Exporter CSV</button>
        <button onClick={() => doExport("json")} disabled={busy}>Exporter JSON</button>
        <button onClick={() => doExport("xlsx")} disabled={busy}>Exporter XLSX</button>
      </div>
      <label className="hint" style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        Importer (CSV/JSON/XLSX) vers cette table
        <input type="file" accept=".csv,.json,.xlsx,.xls" onChange={doImport} disabled={busy} />
      </label>
      {msg && <div style={{ color: "var(--accent)" }}>{msg}</div>}
      {err && <div role="alert" style={{ color: "var(--danger)" }}>{err}</div>}
    </div>
  );
}
