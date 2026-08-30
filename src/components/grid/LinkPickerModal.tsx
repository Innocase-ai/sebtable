import { useEffect, useId, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import * as api from "../../lib/api";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useDebouncedValue } from "../../hooks/useDebouncedValue";
import type {
  Field,
  LinkFieldConfig,
  LinkTarget,
  LinkValue,
} from "../../types/field";

function getFocusable(root: HTMLElement): HTMLElement[] {
  return Array.from(
    root.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
    )
  ).filter((el) => !el.hasAttribute("hidden") && el.getAttribute("aria-hidden") !== "true");
}

interface Props {
  field: Field;
  recordId: string;
  value: LinkValue[];
  onClose: () => void;
}

interface Item {
  id: string;
  label: string;
}

export default function LinkPickerModal({ field, recordId, value, onClose }: Props) {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const queryClient = useQueryClient();
  const cfg = field.config as LinkFieldConfig;
  const targetTableId = cfg.target_table_id;
  // Phase 3 cross-DB : la table cible peut vivre dans une autre base.
  const targetDbId = cfg.target_db_id || "";
  const fetchDbId = targetDbId || dbId;
  const one = cfg.cardinality === "one";

  const [items, setItems] = useState<Item[]>([]);
  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(value.map((l) => l.record_id))
  );
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const debouncedQuery = useDebouncedValue(query, 300);
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);
  const pageSize = 50;
  const titleId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);

  useEffect(() => {
    previousFocus.current = document.activeElement as HTMLElement | null;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }
      if (e.key !== "Tab" || !dialogRef.current) return;
      const list = getFocusable(dialogRef.current);
      if (list.length === 0) {
        e.preventDefault();
        return;
      }
      const first = list[0];
      const last = list[list.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (e.shiftKey) {
        if (active === first) {
          e.preventDefault();
          last.focus();
        }
      } else {
        if (active === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    document.addEventListener("keydown", onKeyDown);
    // focus first element after items load is handled by second effect
    requestAnimationFrame(() => {
      const focusables = dialogRef.current ? getFocusable(dialogRef.current) : [];
      if (focusables.length) focusables[0].focus();
      else dialogRef.current?.focus();
    });
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      previousFocus.current?.focus();
    };
  }, [onClose]);

  useEffect(() => {
    let cancelled = false;
    if (!fetchDbId) return;
    setLoading(true);
    (async () => {
      try {
        const fields = await api.listFields(fetchDbId, targetTableId);
        const primary = fields[0]?.id ?? "_id";
        const filters =
          debouncedQuery.trim() !== "" && primary !== "_id"
            ? [{ field_id: primary, operator: "contains", value: debouncedQuery.trim() }]
            : [];
        const data = await api.getTableData(fetchDbId, targetTableId, {
          filters,
          sorts: [],
          groups: [],
          page: { number: page, size: pageSize },
        });
        if (cancelled) return;
        const list: Item[] = data.records.map((r) => ({
          id: r._id,
          label: String((r as Record<string, unknown>)[primary] ?? r._id),
        }));
        setItems(list);
        setTotal(data.total);
      } catch (e) {
        if (!cancelled) setError(String((e as Error)?.message ?? e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [fetchDbId, targetTableId, debouncedQuery, page]);

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (one) {
        next.clear();
        next.add(id);
      } else if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const save = async () => {
    if (!dbId) return;
    setBusy(true);
    setError("");
    try {
      const current = new Set(value.map((l) => l.record_id));
      const added: LinkTarget[] = [...selected]
        .filter((id) => !current.has(id))
        .map((id) => ({ record_id: id }));
      const removed = [...current].filter((id) => !selected.has(id));
      if (added.length) await api.linkRecords(dbId, field.id, recordId, added);
      if (removed.length)
        await api.unlinkRecords(dbId, field.id, recordId, removed);
      queryClient.invalidateQueries({ queryKey: ["table-data", dbId, field.table_id] });
      onClose();
    } catch (e) {
      setError(String((e as Error)?.message ?? e).replace(/^Error:\s*/i, ""));
      setBusy(false);
    }
  };

  // Reset page on search
  useEffect(() => {
    setPage(1);
  }, [debouncedQuery]);

  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        ref={dialogRef}
        className="modal link-picker"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 id={titleId}>Lier — {field.name}</h2>
        <div className="modal-field">
          <input
            placeholder="Rechercher…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label="Rechercher"
          />
        </div>
        <div className="link-picker-list" aria-live="polite">
          {loading && <div className="cell-empty">Chargement…</div>}
          {!loading && items.length === 0 && !error && (
            <div className="cell-empty">Aucun résultat</div>
          )}
          {items.map((it) => {
            const checked = selected.has(it.id);
            return (
              <label key={it.id} className="link-picker-item">
                <input
                  type={one ? "radio" : "checkbox"}
                  name={one ? "pick" : undefined}
                  checked={checked}
                  onChange={() => toggle(it.id)}
                />
                <span>{it.label}</span>
              </label>
            );
          })}
        </div>
        <div className="grid-footer" style={{ borderTop: "none", padding: "8px 0 0" }}>
          <span>{total} résultat(s)</span>
          <div className="spacer" />
          <button disabled={page <= 1} onClick={() => setPage((p) => p - 1)}>
            Précédent
          </button>
          <span>
            Page {page} / {totalPages}
          </span>
          <button disabled={page >= totalPages} onClick={() => setPage((p) => p + 1)}>
            Suivant
          </button>
        </div>
        <div className="launcher-error" role="alert" aria-live="assertive">{error}</div>
        <div className="modal-actions">
          <button onClick={onClose}>Annuler</button>
          <button className="primary" disabled={busy} onClick={() => void save()}>
            Enregistrer
          </button>
        </div>
      </div>
    </div>
  );
}
