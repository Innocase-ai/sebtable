import { useState, useRef, useEffect } from "react";
import type { Field } from "../../types/field";
import { useTableStore } from "../../stores/tableStore";

export default function ColumnVisibilityMenu({ fields }: { fields: Field[] }) {
  const viewConfig = useTableStore((s) => s.viewConfig);
  const setViewConfig = useTableStore((s) => s.setViewConfig);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const h = (e: MouseEvent) => { if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false); };
    document.addEventListener("mousedown", h);
    return () => document.removeEventListener("mousedown", h);
  }, []);

  const visible = viewConfig.visible_field_ids;
  const isVisible = (id: string) => !visible || visible.length === 0 || visible.includes(id);

  const toggle = (id: string) => {
    const cur = visible && visible.length > 0 ? [...visible] : fields.map((f) => f.id);
    const idx = cur.indexOf(id);
    if (idx >= 0) cur.splice(idx, 1);
    else cur.push(id);
    // si tout coché → null (affiche tout, évite liste vide)
    const next = cur.length === fields.length || cur.length === 0 ? null : cur;
    setViewConfig({ ...viewConfig, visible_field_ids: next });
  };

  const showAll = () => setViewConfig({ ...viewConfig, visible_field_ids: null });
  const hideCount = fields.length - (visible ? visible.length : fields.length);

  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button onClick={() => setOpen((o) => !o)} aria-label="Colonnes" title="Afficher/masquer colonnes" aria-expanded={open} aria-haspopup="menu">
        👁 Colonnes {hideCount > 0 ? `(${hideCount} masquées)` : ""}
      </button>
      {open && (
        <div role="menu" style={{ position: "absolute", top: "calc(100% + 6px)", right: 0, background: "var(--bg-panel)", border: "1px solid var(--border)", borderRadius: 8, padding: 8, minWidth: 220, zIndex: 20, boxShadow: "0 12px 32px rgba(0,0,0,0.4)", display: "flex", flexDirection: "column", gap: 4 }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 4 }}>
            <strong style={{ fontSize: 12 }}>Colonnes</strong>
            <button onClick={showAll} style={{ fontSize: 11 }}>Tout afficher</button>
          </div>
          {fields.map((f) => (
            <label key={f.id} role="menuitemcheckbox" aria-checked={isVisible(f.id)} style={{ display: "flex", alignItems: "center", gap: 8, padding: "4px 6px", borderRadius: 6, cursor: "pointer" }}>
              <input type="checkbox" checked={isVisible(f.id)} onChange={() => toggle(f.id)} />
              <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{f.name}</span>
              <span className="hint" style={{ fontSize: 10 }}>{f.type}</span>
            </label>
          ))}
        </div>
      )}
    </div>
  );
}
