import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useTableStore } from "../../stores/tableStore";
import * as api from "../../lib/api";
import { formatError } from "../../lib/formatError";

interface Props {
  fieldId: string;
  recordId: string;
  value: unknown;
  readOnly?: boolean;
}

// Cache module : évite de re-télécharger les octets à chaque scroll/remontage.
// Borné (LRU) pour ne pas accumuler des object URLs à vie : à l'éviction on
// révoque l'URL. On NE révoque JAMAIS à l'unmount (l'URL reste réutilisée par
// d'autres montages via le cache).
const thumbCache = new Map<string, string>();
const THUMB_CACHE_MAX = 100;
function cacheThumb(key: string, url: string) {
  if (thumbCache.size >= THUMB_CACHE_MAX) {
    const oldest = thumbCache.keys().next().value as string | undefined;
    if (oldest !== undefined) {
      const u = thumbCache.get(oldest);
      if (u) URL.revokeObjectURL(u);
      thumbCache.delete(oldest);
    }
  }
  thumbCache.set(key, url);
}

export function AttachmentThumb({
  dbId,
  tableId,
  recordId,
  fileName,
  mime,
  name,
}: {
  dbId: string;
  tableId: string;
  recordId: string;
  fileName: string;
  mime: string;
  name: string;
}) {
  const key = `${dbId}/${tableId}/${recordId}/${fileName}`;
  const [blobUrl, setBlobUrl] = useState<string | null>(() => thumbCache.get(key) ?? null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (blobUrl) return;
    let cancelled = false;
    let objectUrl: string | null = null;
    api
      .getAttachmentData(dbId, tableId, recordId, fileName)
      .then((b64) => {
        if (cancelled) return;
        const bin = atob(b64);
        const bytes = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
        objectUrl = URL.createObjectURL(new Blob([bytes], { type: mime }));
        cacheThumb(key, objectUrl);
        setBlobUrl(objectUrl);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    // Pas de revoke à l'unmount : l'URL vit dans le cache pour réutilisation.
    // `blobUrl` n'est PAS dans les deps (sinon cleanup → revoke juste après load).
    return () => { cancelled = true; };
  }, [key, dbId, tableId, recordId, fileName, mime]);

  if (failed) {
    return (
      <span
        role="img"
        aria-label={`${name} (aperçu indisponible)`}
        style={{ width: 44, height: 44, display: "inline-flex", alignItems: "center", justifyContent: "center", background: "var(--border)", borderRadius: 4, fontSize: 18 }}
      >
        🖼
      </span>
    );
  }
  if (!blobUrl) {
    return (
      <span
        style={{ width: 44, height: 44, display: "inline-block", background: "var(--border)", borderRadius: 4 }}
        aria-label="Chargement miniature"
      />
    );
  }
  return (
    <a href={blobUrl} target="_blank" rel="noopener noreferrer" onClick={(e) => e.stopPropagation()} style={{ display: "inline-block", lineHeight: 0 }} title={name}>
      <img
        src={blobUrl}
        alt={name}
        loading="lazy"
        onError={() => setFailed(true)}
        style={{
          width: 44,
          height: 44,
          objectFit: "cover",
          borderRadius: 4,
          border: "1px solid var(--border)",
          display: "block",
        }}
      />
    </a>
  );
}

export default function AttachmentCell({ recordId, value, readOnly }: Props) {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id ?? "");
  const tableId = useTableStore((s) => s.activeTableId ?? "");
  const queryClient = useQueryClient();
  const parsed: unknown[] = (() => {
    if (Array.isArray(value)) return value as unknown[];
    if (typeof value === "string" && value.trim().startsWith("[")) {
      try {
        const p = JSON.parse(value);
        return Array.isArray(p) ? p : [];
      } catch {
        return [];
      }
    }
    return [];
  })();
  const arr = parsed as any[];
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const refetch = () => queryClient.invalidateQueries({ queryKey: ["table-data", dbId, tableId] });

  const onDrop = async (files: File[]) => {
    if (!files.length || readOnly) return;
    setBusy(true);
    setErr(null);
    try {
      for (const f of files) {
        const buf = await f.arrayBuffer();
        await api.uploadAttachment(dbId, tableId, recordId, f.name, Array.from(new Uint8Array(buf)));
      }
      await refetch();
    } catch (e) {
      setErr(formatError(e));
    }
    setBusy(false);
  };

  if (readOnly && arr.length === 0) return <div className="cell">—</div>;

  return (
    <div className="cell attachment-cell" style={{ display: "flex", flexWrap: "wrap", gap: 4, alignItems: "center" }}>
      {arr.map((a: any, i: number) => {
        const mime: string = a.type || a.mime || "";
        const isImage = mime.startsWith("image/");
        return (
          <span
            key={a.name ?? i}
            className="chip"
            title={`${a.name} (${Math.round((a.size || 0) / 1024)} Ko)`}
            style={{ display: "inline-flex", alignItems: "center", gap: 4, padding: isImage ? 2 : undefined }}
          >
            {isImage ? (
              <AttachmentThumb dbId={dbId} tableId={tableId} recordId={recordId} fileName={a.name} mime={mime} name={a.name} />
            ) : (
              <span style={{ color: "inherit" }}>{a.name}</span>
            )}
            {!readOnly && (
              <button
                onClick={async (e) => {
                  e.stopPropagation();
                  try {
                    await api.deleteAttachment(dbId, tableId, recordId, a.name);
                    const k = `${dbId}/${tableId}/${recordId}/${a.name}`;
                    const url = thumbCache.get(k);
                    if (url) { URL.revokeObjectURL(url); thumbCache.delete(k); }
                    await refetch();
                  } catch (ex) {
                    setErr(formatError(ex));
                  }
                }}
                aria-label={`Supprimer ${a.name}`}
                style={{ marginLeft: 2 }}
              >
                ×
              </button>
            )}
          </span>
        );
      })}
      {!readOnly && (
        <label className="chip add-chip" style={{ cursor: busy ? "wait" : "pointer" }}>
          {busy ? "…" : "+"}
          <input type="file" multiple style={{ display: "none" }} onChange={(e) => { const fl = e.target.files ? Array.from(e.target.files) : []; void onDrop(fl as any); }} />
        </label>
      )}
      {err && (
        <span role="alert" style={{ color: "var(--danger)", fontSize: 11 }}>
          {err}
        </span>
      )}
    </div>
  );
}
