use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Valeur renvoyée au front quand une clé OpenAI est stockée (jamais la clé réelle).
pub const MASKED_API_KEY: &str = "***";
/// Sentinelle envoyée par le front pour demander la suppression de la clé stockée.
pub const DELETE_API_KEY: &str = "__delete__";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DbRole {
    Reference,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    pub id: String,
    pub path: String,
    pub role: DbRole,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSettings {
    pub llm_provider: String,
    pub lmstudio_url: String,
    pub lmstudio_model: String,
    pub openai_api_key: String,
    pub openai_model: String,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            llm_provider: "hybrid".into(),
            lmstudio_url: "http://localhost:1234/v1".into(),
            lmstudio_model: "auto".into(),
            openai_api_key: String::new(),
            openai_model: "gpt-4o-mini".into(),
        }
    }
}

impl WorkspaceSettings {
    pub fn has_api_key(&self) -> bool {
        !self.openai_api_key.trim().is_empty()
    }

    /// Copie sans jamais exposer la clé réelle : `"***"` si une clé est
    /// configurée, chaîne vide sinon. Utilisé pour le fichier et le front.
    pub fn masked(&self) -> Self {
        let mut s = self.clone();
        s.openai_api_key = if self.has_api_key() {
            MASKED_API_KEY.to_string()
        } else {
            String::new()
        };
        s
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub name: String,
    pub version: u32,
    pub databases: Vec<Database>,
    pub active_database_id: String,
    #[serde(default)]
    pub settings: WorkspaceSettings,
}

impl WorkspaceConfig {
    /// Copie destinée au fichier et au front, avec la clé OpenAI masquée.
    pub fn masked(&self) -> Self {
        let mut c = self.clone();
        c.settings = c.settings.masked();
        c
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_db_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub id: String,
    pub table_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub config: Value,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInput {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TableChanges {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FieldChanges {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub field_id: String,
    pub operator: String,
    #[serde(default)]
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sort {
    pub field_id: String,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub field_id: String,
    pub order: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Page {
    pub number: u32,
    pub size: u32,
}

fn default_and() -> String { "and".into() }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ViewConfig {
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default = "default_and")]
    pub filter_conjunction: String,
    #[serde(default)]
    pub sorts: Vec<Sort>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_field_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<Page>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct View {
    pub id: String,
    pub table_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub view_type: String,
    #[serde(default)]
    pub config: ViewConfig,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewInput {
    pub table_id: String,
    pub name: String,
    #[serde(rename = "type", default = "default_view_type")]
    pub view_type: String,
    #[serde(default)]
    pub config: ViewConfig,
}

fn default_view_type() -> String {
    "grid".into()
}

// ---- Relations & champs calculés -------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkValue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_id: Option<String>,
    pub table_id: String,
    pub record_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkTarget {
    pub record_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinkFieldConfig {
    #[serde(default)]
    pub target_table_id: String,
    #[serde(default)]
    pub target_db_id: String,
    #[serde(default = "default_many")]
    pub cardinality: String,
    #[serde(default = "default_true")]
    pub allow_creating: bool,
    #[serde(default)]
    pub is_backlink: bool,
    #[serde(default)]
    pub source_link_field_id: String,
}

fn default_many() -> String {
    "many".into()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupFieldConfig {
    pub source_link_field_id: String,
    pub target_field_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollupFieldConfig {
    pub source_link_field_id: String,
    pub target_field_id: String,
    pub function: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountFieldConfig {
    pub source_link_field_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FormulaFieldConfig {
    pub expression: String,
}

impl Field {
    pub fn is_stored(&self) -> bool {
        match self.field_type.as_str() {
            "lookup" | "rollup" | "count" | "formula" | "button" => false,
            "link" => !self.is_backlink(),
            _ => true,
        }
    }

    pub fn link_config(&self) -> Option<LinkFieldConfig> {
        if self.field_type == "link" {
            serde_json::from_value(self.config.clone()).ok()
        } else {
            None
        }
    }

    pub fn is_backlink(&self) -> bool {
        self.link_config().map(|c| c.is_backlink).unwrap_or(false)
    }

    pub fn formula_config(&self) -> Option<FormulaFieldConfig> {
        if self.field_type == "formula" {
            serde_json::from_value(self.config.clone()).ok()
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedRecords {
    pub records: Vec<Value>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordWithRelations {
    pub record: Value,
    pub relations: std::collections::HashMap<String, Vec<Value>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masked_never_leaks_key() {
        let s = WorkspaceSettings {
            openai_api_key: "sk-super-secret".into(),
            ..Default::default()
        };
        assert_eq!(s.masked().openai_api_key, MASKED_API_KEY);
        assert!(s.masked().has_api_key());

        let empty = WorkspaceSettings::default();
        assert_eq!(empty.masked().openai_api_key, "");
        assert!(!empty.masked().has_api_key());
    }
}
