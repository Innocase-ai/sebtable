import { invoke } from "@tauri-apps/api/core";
import type { Database, DbRole, WorkspaceConfig } from "../types/workspace";
import type { Table } from "../types/database";
import type {
  Field,
  FieldChanges,
  FieldInput,
  LinkFieldConfig,
  LinkTarget,
  View,
  ViewConfig,
  ViewInput,
} from "../types/field";
import type { PaginatedRecords, Record as TableRecord } from "../types/record";

export function createWorkspace(dir: string, name: string): Promise<WorkspaceConfig> {
  return invoke("create_workspace", { dir, name });
}

export function openWorkspace(path: string): Promise<WorkspaceConfig> {
  return invoke("open_workspace", { path });
}

export function createDatabase(name: string, role: DbRole): Promise<Database> {
  return invoke("create_database", { name, role });
}

export function switchDatabase(dbId: string): Promise<WorkspaceConfig> {
  return invoke("switch_database", { dbId });
}

export function deleteDatabase(dbId: string): Promise<WorkspaceConfig> {
  return invoke("delete_database", { dbId });
}

export function listDatabases(): Promise<Database[]> {
  return invoke("list_databases");
}

export function listTables(dbId: string): Promise<Table[]> {
  return invoke("list_tables", { dbId });
}

export function createTable(
  dbId: string,
  name: string,
  fields: FieldInput[],
  sourceDbId?: string | null
): Promise<Table> {
  return invoke("create_table", { dbId, name, fields, sourceDbId: sourceDbId ?? null });
}

export function getRecordWithRelations(
  dbId: string,
  tableId: string,
  recordId: string,
  depth?: number
): Promise<{ record: TableRecord; relations: globalThis.Record<string, TableRecord[]> }> {
  return invoke("get_record_with_relations", { dbId, tableId, recordId, depth: depth ?? 1 });
}

export function deleteTable(dbId: string, tableId: string): Promise<void> {
  return invoke("delete_table", { dbId, tableId });
}

export function listFields(dbId: string, tableId: string): Promise<Field[]> {
  return invoke("list_fields", { dbId, tableId });
}

export function createField(
  dbId: string,
  tableId: string,
  field: FieldInput
): Promise<Field> {
  return invoke("create_field", { dbId, tableId, field });
}

export function updateField(
  dbId: string,
  fieldId: string,
  changes: FieldChanges
): Promise<void> {
  return invoke("update_field", { dbId, fieldId, changes });
}

export function deleteField(dbId: string, fieldId: string): Promise<void> {
  return invoke("delete_field", { dbId, fieldId });
}

export function createLinkField(
  dbId: string,
  sourceTableId: string,
  name: string,
  config: LinkFieldConfig
): Promise<Field> {
  return invoke("create_link_field", { dbId, sourceTableId, name, config });
}

export function linkRecords(
  dbId: string,
  linkFieldId: string,
  sourceRecordId: string,
  targets: LinkTarget[]
): Promise<void> {
  return invoke("link_records", { dbId, linkFieldId, sourceRecordId, targets });
}

export function unlinkRecords(
  dbId: string,
  linkFieldId: string,
  sourceRecordId: string,
  targetIds: string[]
): Promise<void> {
  return invoke("unlink_records", { dbId, linkFieldId, sourceRecordId, targetIds });
}

export function getTableData(
  dbId: string,
  tableId: string,
  viewConfig: ViewConfig
): Promise<PaginatedRecords> {
  return invoke("get_table_data", {
    dbId,
    tableId,
    viewConfig,
    includeLookups: false,
  });
}

export function upsertRecords(
  dbId: string,
  tableId: string,
  records: TableRecord[]
): Promise<TableRecord[]> {
  return invoke("upsert_records", { dbId, tableId, records });
}

export function deleteRecords(
  dbId: string,
  tableId: string,
  ids: string[]
): Promise<void> {
  return invoke("delete_records", { dbId, tableId, ids });
}

export function listViews(dbId: string, tableId: string): Promise<View[]> {
  return invoke("list_views", { dbId, tableId });
}

export function createView(dbId: string, view: ViewInput): Promise<View> {
  return invoke("create_view", { dbId, view });
}

export function updateView(
  dbId: string,
  viewId: string,
  config: ViewConfig
): Promise<void> {
  return invoke("update_view", { dbId, viewId, config });
}

export function deleteView(dbId: string, viewId: string): Promise<void> {
  return invoke("delete_view", { dbId, viewId });
}

// ---- IA hybride (Phase 4)
import type {
  AnalysisResult,
  FormulaResult,
  ProviderStatus,
  RelationSuggestion,
  TransformPlan,
  TransformResult,
} from "../types/ai";

export function aiSuggestRelations(dbId: string, tableId: string): Promise<RelationSuggestion[]> {
  return invoke("ai_suggest_relations", { dbId, tableId });
}

export function aiGenerateFormula(
  dbId: string,
  tableId: string,
  prompt: string
): Promise<FormulaResult> {
  return invoke("ai_generate_formula", { dbId, tableId, prompt });
}

export function aiAnalyze(
  dbId: string,
  tableId: string,
  question?: string | null
): Promise<AnalysisResult> {
  return invoke("ai_analyze", { dbId, tableId, question: question ?? null });
}

export function aiCleanPreview(
  dbId: string,
  tableId: string,
  instruction: string
): Promise<TransformPlan> {
  return invoke("ai_clean_preview", { dbId, tableId, instruction });
}

export function aiApplyTransform(
  dbId: string,
  tableId: string,
  plan: TransformPlan
): Promise<TransformResult> {
  return invoke("ai_apply_transform", { dbId, tableId, plan });
}

export function aiCheckStatus(): Promise<ProviderStatus> {
  return invoke("ai_check_status");
}

// ---- Phase 5 : Import/Export / Attachments / Settings
export function exportTable(dbId: string, tableId: string, format: "csv" | "json" | "xlsx"): Promise<number[]> {
  return invoke("export_table", { dbId, tableId, format });
}
export function importTable(dbId: string, file: number[], options: { format: string; tableId?: string | null; tableName?: string | null; hasHeader?: boolean }): Promise<{ imported_rows: number; table_id: string; errors: string[] }> {
  return invoke("import_table", { dbId, file, options });
}
export function uploadAttachment(dbId: string, tableId: string, recordId: string, fileName: string, data: number[]): Promise<{ name: string; url: string; size: number; type: string }> {
  return invoke("upload_attachment", { dbId, tableId, recordId, fileName, data });
}
export function listAttachments(dbId: string, tableId: string, recordId: string): Promise<{ name: string; url: string; size: number; type: string }[]> {
  return invoke("list_attachments", { dbId, tableId, recordId });
}
export function deleteAttachment(dbId: string, tableId: string, recordId: string, fileName: string): Promise<void> {
  return invoke("delete_attachment", { dbId, tableId, recordId, fileName });
}
export function getAttachmentData(dbId: string, tableId: string, recordId: string, fileName: string): Promise<string> {
  return invoke("get_attachment_data", { dbId, tableId, recordId, fileName });
}
export function getWorkspaceSettings(): Promise<{ llm_provider: string; lmstudio_url: string; lmstudio_model: string; openai_api_key: string; openai_model: string }> {
  return invoke("get_workspace_settings");
}
export function updateWorkspaceSettings(settings: { llm_provider: string; lmstudio_url: string; lmstudio_model: string; openai_api_key: string; openai_model: string }): Promise<{ llm_provider: string; lmstudio_url: string; lmstudio_model: string; openai_api_key: string; openai_model: string }> {
  return invoke("update_workspace_settings", { settings });
}
export function searchWorkspace(query: string): Promise<{ db_id: string; db_name: string; table_id: string; table_name: string; record_id: string; field_id: string; field_name: string; snippet: string }[]> {
  return invoke("search_workspace", { query });
}
