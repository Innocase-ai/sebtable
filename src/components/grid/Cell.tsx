import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { Field, SelectOption } from "../../types/field";

function selectOptions(field: Field): SelectOption[] {
  const cfg = field.config as { options?: SelectOption[] } | undefined;
  return cfg?.options ?? [];
}

export function formatValue(field: Field, value: unknown): string {
  if (value === null || value === undefined) return "";
  if (Array.isArray(value)) {
    return value.map((v) => formatValue(field, v)).filter((s) => s !== "").join(", ");
  }
  switch (field.type) {
    case "checkbox":
      return value ? "✓" : "";
    case "select": {
      const opts = selectOptions(field);
      const ids: unknown[] = Array.isArray(value) ? value : [value];
      return ids
        .map((id) => opts.find((o) => o.id === id)?.name ?? String(id))
        .join(", ");
    }
    case "number":
      return String(value);
    default:
      return String(value);
  }
}

function valueToString(field: Field, value: unknown): string {
  if (value === null || value === undefined) return "";
  switch (field.type) {
    case "number":
      return String(value);
    case "date":
      return typeof value === "string" ? value : "";
    case "select":
      return typeof value === "string" ? value : "";
    default:
      return String(value);
  }
}

function isValidNumberString(s: string): boolean {
  // N'accepte que décimaux simples, rejette Infinity/hex/scientifique
  return /^-?\d+(\.\d+)?$/.test(s.trim());
}
function stringToValue(field: Field, s: string): unknown {
  switch (field.type) {
    case "number": {
      if (s.trim() === "") return null;
      if (!isValidNumberString(s)) return null;
      const n = Number(s);
      return Number.isFinite(n) ? n : null;
    }
    case "date":
    case "select":
      return s === "" ? null : s;
    default:
      return s;
  }
}

interface CellProps {
  field: Field;
  value: unknown;
  onCommit: (v: unknown) => void;
  readOnly?: boolean;
}

export default function Cell({ field, value, onCommit, readOnly }: CellProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");

  if (field.type === "checkbox") {
    const checked = !!value;
    const toggle = () => !readOnly && onCommit(!checked);
    return (
      <div
        className="cell cell-checkbox"
        role="button"
        tabIndex={readOnly ? -1 : 0}
        aria-checked={checked}
        onClick={toggle}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            toggle();
          }
        }}
      >
        <input type="checkbox" checked={checked} readOnly tabIndex={-1} />
      </div>
    );
  }

  if (editing && !readOnly) {
    return (
      <div className="cell editing">
        <Editor
          field={field}
          initial={draft}
          onCommit={(v) => {
            onCommit(v);
            setEditing(false);
          }}
          onCancel={() => setEditing(false)}
        />
      </div>
    );
  }

  const text = formatValue(field, value);
  const startEdit = () => {
    if (readOnly) return;
    setDraft(valueToString(field, value));
    setEditing(true);
  };
  return (
    <div
      className="cell"
      title={text}
      tabIndex={readOnly ? -1 : 0}
      role="button"
      onClick={startEdit}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          startEdit();
        }
      }}
    >
      {text !== "" ? text : <span className="cell-empty">—</span>}
    </div>
  );
}

function isEmailField(field: Field): boolean {
  if (field.type === "email") return true;
  const n = field.name.toLowerCase();
  return n === "mail" || n === "email" || n.includes("mail") || n.includes("e-mail");
}

function Editor({
  field,
  initial,
  onCommit,
  onCancel,
}: {
  field: Field;
  initial: string;
  onCommit: (v: unknown) => void;
  onCancel: () => void;
}) {
  const [val, setVal] = useState(initial);
  const [error, setError] = useState("");
  const ref = useRef<HTMLInputElement>(null);
  const committed = useRef(false);

  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);

  const validate = (): boolean => {
    if (field.type === "number" && val.trim() !== "" && (!isValidNumberString(val) || !Number.isFinite(Number(val)))) {
      setError("Nombre invalide");
      return false;
    }
    if (isEmailField(field) && val.trim() !== "") {
      // doit contenir @ et .xx (au moins 2 caractères après le dernier point)
      if (!/^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/.test(val.trim())) {
        setError("Email invalide : doit contenir @ et .xx");
        return false;
      }
    }
    setError("");
    return true;
  };

  const commit = () => {
    if (committed.current) return;
    if (!validate()) return;
    committed.current = true;
    onCommit(stringToValue(field, val));
  };

  const commitForce = () => {
    // used by blur outside? still validate
    if (!validate()) {
      // keep editing open: focus back
      ref.current?.focus();
      return;
    }
    commit();
  };

  const keyHandler = (e: KeyboardEvent) => {
    if (e.key === "Enter") commit();
    if (e.key === "Escape") onCancel();
  };

  if (field.type === "select") {
    const opts = selectOptions(field);
    return (
      <select
        value={val}
        autoFocus
        onChange={(e) => setVal(e.target.value)}
        onBlur={commit}
        onKeyDown={keyHandler}
      >
        <option value="">—</option>
        {opts.map((o) => (
          <option key={o.id} value={o.id}>
            {o.name}
          </option>
        ))}
      </select>
    );
  }

  const inputType =
    field.type === "number"
      ? "number"
      : field.type === "date"
        ? "date"
        : isEmailField(field)
          ? "email"
          : field.type === "url"
            ? "url"
            : field.type === "phone"
              ? "tel"
              : "text";

  return (
    <div style={{ display: "flex", flexDirection: "column", width: "100%" }}>
      <input
        ref={ref}
        type={inputType}
        value={val}
        aria-invalid={!!error}
        aria-describedby={error ? "cell-error" : undefined}
        onChange={(e) => {
          setVal(e.target.value);
          if (error) setError("");
        }}
        onBlur={commitForce}
        onKeyDown={keyHandler}
        style={error ? { borderColor: "var(--danger)" } : undefined}
      />
      {error && (
        <span id="cell-error" role="alert" style={{ color: "var(--danger)", fontSize: 11 }}>
          {error}
        </span>
      )}
    </div>
  );
}
