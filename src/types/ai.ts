export interface RelationSuggestion {
  source_db_id: string;
  source_table_id: string;
  source_table_name: string;
  source_field_id: string;
  source_field_name: string;
  target_db_id: string;
  target_table_id: string;
  target_table_name: string;
  target_field_id: string;
  target_field_name: string;
  cardinality: string;
  confidence: number;
  reason: string;
}

export interface FormulaResult {
  expression: string;
  explanation: string;
  valid: boolean;
  error?: string | null;
  provider: string;
}

export interface TableStats {
  table_id: string;
  table_name: string;
  row_count: number;
  fields: FieldStats[];
}

export interface FieldStats {
  field_id: string;
  field_name: string;
  field_type: string;
  non_null: number;
  nulls: number;
  distinct: number;
  top_values: [string, number][];
  numeric?: { min: number; max: number; avg: number; sum: number } | null;
}

export interface AnalysisResult {
  summary: string;
  insights: string[];
  suggestions: string[];
  stats: TableStats[];
  provider: string;
}

export type TransformOpType =
  | "trim"
  | "upper"
  | "lower"
  | "title_case"
  | "deduplicate"
  | "fill_null"
  | "replace"
  | "regex_replace"
  | "normalize_email"
  | "normalize_phone";

export interface TransformOp {
  type: TransformOpType;
  field_id: string;
  field_name: string;
  params: Record<string, unknown>;
  description: string;
}

export interface PreviewRow {
  record_id: string;
  before: unknown;
  after: unknown;
}

export interface TransformPlan {
  ops: TransformOp[];
  preview: PreviewRow[];
  /** Estimation du nb de lignes affectées (sample complet, extrapolée) */
  estimated_rows: number;
  provider: string;
}

export interface TransformResult {
  applied_rows: number;
  ops: TransformOp[];
}

export interface ProviderStatus {
  lmstudio_available: boolean;
  openai_configured: boolean;
  active_provider: string;
}
