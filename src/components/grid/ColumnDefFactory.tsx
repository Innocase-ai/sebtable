import type { ColumnDef } from "@tanstack/react-table";
import { isBacklink, isStoredField, type Field } from "../../types/field";
import type { Record } from "../../types/record";
import Cell from "./Cell";
import LinkCell from "./LinkCell";
import AttachmentCell from "./AttachmentCell";
import ButtonCell from "./ButtonCell";
import { askConfirm } from "../../lib/askConfirm";

export interface GridMeta {
  onCommit: (recordId: string, fieldId: string, value: unknown) => void;
  onToggleSort: (fieldId: string) => void;
  sortDir: (fieldId: string) => "asc" | "desc" | null;
  onDeleteRow: (recordId: string) => void;
  onAddField: () => void;
}

export function colWidth(f: Field): number {
  switch (f.type) {
    case "checkbox":
      return 90;
    case "number":
      return 120;
    case "date":
      return 140;
    case "select":
      return 160;
    case "long_text":
      return 280;
    case "link":
      return 220;
    default:
      return 180;
  }
}

export function buildColumns(fields: Field[]): ColumnDef<Record>[] {
  const cols: ColumnDef<Record>[] = [
    {
      id: "__row",
      header: () => <div className="grid-header-cell row-num">#</div>,
      cell: ({ row, table }) => {
        const meta = table.options.meta as GridMeta | undefined;
        return (
          <div className="cell row-num">
            <span>{row.index + 1}</span>
            <button
              className="row-delete"
              title="Supprimer la ligne"
              aria-label={`Supprimer la ligne ${row.index + 1}`}
              onClick={async (e) => {
                e.stopPropagation();
                const ok = await askConfirm(`Supprimer la ligne ${row.index + 1} ?`, "Supprimer");
                if (ok) meta?.onDeleteRow(row.original._id);
              }}
            >
              ×
            </button>
          </div>
        );
      },
      size: 56,
      enableSorting: false,
    },
  ];

  for (const f of fields) {
    const stored = isStoredField(f);
    cols.push({
      id: f.id,
      accessorKey: f.id,
      header: ({ table }) => {
        const meta = table.options.meta as GridMeta | undefined;
        const dir = meta?.sortDir(f.id) ?? null;
        return (
          <div
            className={`grid-header-cell type-${f.type}${stored ? "" : " computed"}`}
            title={f.type}
            onClick={() => stored && meta?.onToggleSort(f.id)}
          >
            <span className="name">{f.name}</span>
            {!stored && (
              <span className="computed-ind" title="Champ calculé">
                ƒ
              </span>
            )}
            {stored && dir && (
              <span className="sort-ind">{dir === "asc" ? "↑" : "↓"}</span>
            )}
          </div>
        );
      },
      cell: ({ row, getValue, table }) => {
        if (f.type === "link") {
          return (
            <LinkCell
              field={f}
              recordId={row.original._id}
              value={getValue()}
              readOnly={isBacklink(f)}
            />
          );
        }
        if (f.type === "attachment") {
          return (
            <AttachmentCell
              fieldId={f.id}
              recordId={row.original._id}
              value={getValue()}
              readOnly={false}
            />
          );
        }
        if (f.type === "button") {
          return <ButtonCell field={f} recordId={row.original._id} />;
        }
        const meta = table.options.meta as GridMeta | undefined;
        return (
          <Cell
            field={f}
            value={getValue()}
            readOnly={!stored}
            onCommit={(v) => meta?.onCommit(row.original._id, f.id, v)}
          />
        );
      },
      size: colWidth(f),
      enableSorting: false,
    });
  }

  cols.push({
    id: "__add",
    header: ({ table }) => {
      const meta = table.options.meta as GridMeta | undefined;
      return (
        <div className="grid-header-cell add-col">
          <button onClick={() => meta?.onAddField()} title="Ajouter un champ">
            +
          </button>
        </div>
      );
    },
    cell: () => <div className="cell" />,
    size: 44,
    enableSorting: false,
  });

  return cols;
}
