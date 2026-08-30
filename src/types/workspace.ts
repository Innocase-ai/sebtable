export type DbRole = "reference" | "project";

export interface Database {
  id: string;
  path: string;
  role: DbRole;
  name: string;
}

export interface WorkspaceSettings {
  llm_provider: string;
  lmstudio_url: string;
  lmstudio_model: string;
  openai_api_key: string;
  openai_model: string;
}

export interface WorkspaceConfig {
  name: string;
  version: number;
  databases: Database[];
  active_database_id: string;
  settings: WorkspaceSettings;
}
