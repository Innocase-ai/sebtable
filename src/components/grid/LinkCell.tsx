import { useState } from "react";
import type { Field, LinkValue } from "../../types/field";
import LinkPickerModal from "./LinkPickerModal";

interface Props {
  field: Field;
  recordId: string;
  value: unknown;
  readOnly?: boolean;
}

export default function LinkCell({ field, recordId, value, readOnly }: Props) {
  const [open, setOpen] = useState(false);
  const links = (Array.isArray(value) ? value : []) as LinkValue[];

  const start = () => {
    if (!readOnly) setOpen(true);
  };

  return (
    <div
      className="cell link-cell"
      onClick={start}
      role="button"
      tabIndex={readOnly ? -1 : 0}
      title="Modifier les liens"
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          start();
        }
      }}
    >
      {links.length === 0 ? (
        <span className="cell-empty">—</span>
      ) : (
        links.map((l) => (
          <span key={l.record_id} className="link-chip">
            {String(l.display ?? l.record_id)}
          </span>
        ))
      )}
      {open && (
        <LinkPickerModal
          field={field}
          recordId={recordId}
          value={links}
          onClose={() => setOpen(false)}
        />
      )}
    </div>
  );
}
