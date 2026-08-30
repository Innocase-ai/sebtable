import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { formatValue } from "./Cell";
import type { Field } from "../../types/field";

export default function SortableKanbanCard({ id, title, record, fields }: { id: string; title: string; record: Record<string, unknown>; fields: Field[] }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.6 : 1,
    background: "var(--bg-header)",
    border: "1px solid var(--border)",
    borderRadius: 8,
    padding: 10,
    cursor: "grab",
    display: "flex",
    flexDirection: "column",
    gap: 4,
  };
  const snippetFields = fields.filter((f) => f.type !== "attachment").slice(0, 3);
  return (
    <div ref={setNodeRef} style={style} {...attributes} {...listeners}>
      <strong style={{ fontSize: 13, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{title}</strong>
      {snippetFields.map((f) => (
        <span key={f.id} style={{ fontSize: 11, color: "var(--text-muted)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          <em>{f.name}:</em> {formatValue(f, record[f.id]) || "—"}
        </span>
      ))}
    </div>
  );
}
