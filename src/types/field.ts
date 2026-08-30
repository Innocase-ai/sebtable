export type FieldType =
  | "text"
  | "long_text"
  | "number"
  | "checkbox"
  | "select"
  | "date"
  | "email"
  | "url"
  | "phone"
  | "link"
  | "lookup"
  | "rollup"
  | "count"
  | "formula"
  | "created_time"
  | "last_modified_time"
  | "created_by"
  | "last_modified_by"
  | "attachment"
  | "button";

export interface SelectOption {
  id: string;
  name: string;
  color: string;
}

export interface Field {
  id: string;
  table_id: string;
  name: string;
  type: FieldType;
  config: any;
  position: number;
}

export interface FieldInput {
  name: string;
  type: FieldType;
  config?: Record<string, unknown>;
}

export interface FieldChanges {
  name?: string | null;
  config?: Record<string, unknown> | null;
  position?: number | null;
}

export interface Filter {
  field_id: string;
  operator: string;
  value: unknown;
}

export interface Sort {
  field_id: string;
  direction: "asc" | "desc";
}

export interface Group {
  field_id: string;
  order: "asc" | "desc";
}

export interface Page {
  number: number;
  size: number;
}

export interface ViewConfig {
  filters: Filter[];
  /** Conjonction entre filtres : "and" (défaut, tous) ou "or" (au moins un) */
  filter_conjunction?: "and" | "or" | null;
  sorts: Sort[];
  groups: Group[];
  visible_field_ids?: string[] | null;
  page?: Page | null;
}

export interface View {
  id: string;
  table_id: string;
  name: string;
  type: string;
  config: ViewConfig;
  is_default: boolean;
}

export interface ViewInput {
  table_id: string;
  name: string;
  type?: string;
  config?: ViewConfig;
}

// ---- Relations & champs calculés ----

export interface LinkValue {
  db_id?: string | null;
  table_id: string;
  record_id: string;
  display?: unknown;
}

export interface LinkTarget {
  record_id: string;
}

export interface LinkFieldConfig {
  target_table_id: string;
  target_db_id?: string;
  cardinality: "one" | "many";
  allow_creating?: boolean;
  is_backlink?: boolean;
  source_link_field_id?: string;
}

export interface LookupFieldConfig {
  source_link_field_id: string;
  target_field_id: string;
}

export interface RollupFieldConfig {
  source_link_field_id: string;
  target_field_id: string;
  function: string;
}

export interface CountFieldConfig {
  source_link_field_id: string;
}

export interface FormulaFieldConfig {
  expression: string;
}

export function isStoredField(field: Field): boolean {
  switch (field.type) {
    case "lookup":
    case "rollup":
    case "count":
    case "formula":
    case "button":
      return false;
    case "link": {
      const cfg = field.config as LinkFieldConfig | undefined;
      return !cfg?.is_backlink;
    }
    default:
      return true;
  }
}

export function isLinkField(field: Field): boolean {
  return field.type === "link";
}

export function isBacklink(field: Field): boolean {
  const cfg = field.config as LinkFieldConfig | undefined;
  return field.type === "link" && !!cfg?.is_backlink;
}
