mod ai;
mod commands;
pub mod db;
mod error;
mod formula;
mod security;
mod utils;
pub mod workspace;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use sqlx::SqlitePool;
use tokio::sync::RwLock;
use workspace::manager::Workspace;

pub struct AppState {
    pub workspace: RwLock<Option<Workspace>>,
    /// Pools des bases non-actives mis en cache (cross-DB). `SqlitePool` clone
    /// est peu coûteux (Arc interne) ; évite de rouvrir un fichier à chaque
    /// appel AI (était: un `open_pool` par base à chaque `build_context`).
    pub cross_pools: RwLock<HashMap<String, SqlitePool>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            workspace: RwLock::new(None),
            cross_pools: RwLock::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::workspace::create_workspace,
            commands::workspace::open_workspace,
            commands::workspace::create_database,
            commands::workspace::delete_database,
            commands::workspace::switch_database,
            commands::workspace::list_databases,
            commands::tables::list_tables,
            commands::tables::list_fields,
            commands::tables::create_table,
            commands::tables::update_table,
            commands::tables::delete_table,
            commands::tables::create_field,
            commands::tables::update_field,
            commands::tables::delete_field,
            commands::tables::get_table_data,
            commands::tables::upsert_records,
            commands::tables::delete_records,
            commands::tables::create_link_field,
            commands::tables::link_records,
            commands::tables::unlink_records,
            commands::tables::get_record_with_relations,
            commands::views::create_view,
            commands::views::update_view,
            commands::views::delete_view,
            commands::views::list_views,
            commands::ai::ai_suggest_relations,
            commands::ai::ai_generate_formula,
            commands::ai::ai_analyze,
            commands::ai::ai_clean_preview,
            commands::ai::ai_apply_transform,
            commands::ai::ai_check_status,
            commands::import_export::export_table,
            commands::import_export::import_table,
            commands::attachments::upload_attachment,
            commands::attachments::list_attachments,
            commands::attachments::get_attachment_data,
            commands::attachments::delete_attachment,
            commands::workspace::get_workspace_settings,
            commands::workspace::update_workspace_settings,
            commands::search::search_workspace,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
