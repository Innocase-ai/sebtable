import { useMemo } from "react";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import { useTableData } from "../../hooks/useTableData";
import { formatValue } from "./Cell";
import { AttachmentThumb } from "./AttachmentCell";
import type { Field } from "../../types/field";

function getAttachmentThumb(record: Record<string, unknown>, attachmentField?: Field): { url: string; name: string } | null {
  if (!attachmentField) return null;
  const v = record[attachmentField.id];
  if (Array.isArray(v) && v.length > 0) {
    const first = v[0] as { name?: string; url?: string };
    if (first?.name) return { url: first.url ?? "", name: first.name };
  }
  return null;
}

export default function GalleryView() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id ?? "");
  const tableId = useTableStore((s) => s.activeTableId ?? "");
  const fields = useTableStore((s) => s.fields);
  const viewConfig = useTableStore((s) => s.viewConfig);
  const setViewConfig = useTableStore((s) => s.setViewConfig);
  const { data, isLoading, isError, error } = useTableData();

  const records = useMemo(() => (data?.records ?? []) as Record<string, unknown>[], [data]);
  const visibleFields = useMemo(() => {
    const ids = viewConfig.visible_field_ids;
    if (!ids || ids.length === 0) return fields;
    const set = new Set(ids);
    return fields.filter((f) => set.has(f.id));
  }, [fields, viewConfig.visible_field_ids]);

  const coverField = useMemo(() => visibleFields.find((f) => f.type === "attachment") ?? null, [visibleFields]);
  const titleField = useMemo(() => visibleFields.find((f) => f.type === "text" || f.type === "long_text") ?? visibleFields[0] ?? null, [visibleFields]);

  const page = viewConfig.page?.number ?? 1;
  const pageSize = viewConfig.page?.size ?? 100;
  const total = data?.total ?? 0;
  const setPage = (n: number) => setViewConfig({ ...viewConfig, page: { number: n, size: pageSize } });

  if (isLoading) return <div className="grid-overlay">Chargement…</div>;
  if (isError) return <div className="grid-overlay" role="alert">Erreur : {String((error as Error)?.message ?? error)}</div>;
  if (records.length === 0) return <div className="grid-overlay">Aucun enregistrement</div>;

  return (
    <>
      <div className="gallery-grid">
        {records.map((rec) => {
          const rid = String((rec as Record<string, unknown>)._id ?? "");
          const thumb = getAttachmentThumb(rec as Record<string, unknown>, coverField ?? undefined);
          const title = titleField ? formatValue(titleField, rec[titleField.id]) || rid.slice(0, 8) : rid.slice(0, 8);
          return (
            <div key={rid} className="gallery-card" role="article" aria-label={title}>
              {thumb ? (
                <div className="gallery-cover" title={thumb.name} style={{ background: "var(--border)", height: 140, borderRadius: "8px 8px 0 0", display: "flex", alignItems: "center", justifyContent: "center", overflow: "hidden" }}>
                  <AttachmentThumb dbId={dbId} tableId={tableId} recordId={rid} fileName={thumb.name} mime={(coverField && (Array.isArray(rec[coverField.id]) ? (rec[coverField.id] as { type?: string }[])[0]?.type : undefined)) ?? "image/jpeg"} name={thumb.name} />
                </div>
              ) : (
                <div className="gallery-cover gallery-cover--empty" style={{ height: 80, background: "linear-gradient(135deg, rgba(109,123,255,0.15), rgba(255,107,157,0.12))", borderRadius: "8px 8px 0 0" }} />
              )}
              <div style={{ padding: 10, display: "flex", flexDirection: "column", gap: 6 }}>
                <strong style={{ fontSize: 13, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{title}</strong>
                {visibleFields.filter((f) => f.id !== titleField?.id && f.id !== coverField?.id).slice(0, 3).map((f) => (
                  <span key={f.id} style={{ fontSize: 11, color: "var(--text-muted)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                    <em>{f.name}:</em> {formatValue(f, rec[f.id]) || "—"}
                  </span>
                ))}
              </div>
            </div>
          );
        })}
      </div>
      <div className="grid-footer">
        <span>{total} enregistrement(s)</span>
        <div className="spacer" />
        <button disabled={page <= 1} onClick={() => setPage(page - 1)}>Précédent</button>
        <span>Page {page} / {Math.max(1, Math.ceil(total / pageSize))}</span>
        <button disabled={page * pageSize >= total} onClick={() => setPage(page + 1)}>Suivant</button>
      </div>
    </>
  );
}
