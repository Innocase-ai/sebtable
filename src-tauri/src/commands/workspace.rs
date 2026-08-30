use std::path::Path;

use tauri::State;

use crate::db::models::{Database, DbRole, DELETE_API_KEY, MASKED_API_KEY, WorkspaceConfig};
use crate::error::AppError;
use crate::security;
use crate::workspace::manager::Workspace;
use crate::AppState;

#[tauri::command]
pub async fn create_workspace(
    state: State<'_, AppState>,
    dir: String,
    name: String,
) -> Result<WorkspaceConfig, AppError> {
    let ws = Workspace::create(Path::new(&dir), name).await?;
    let config = ws.config.masked();
    *state.workspace.write().await = Some(ws);
    state.cross_pools.write().await.clear();
    Ok(config)
}

#[tauri::command]
pub async fn open_workspace(
    state: State<'_, AppState>,
    path: String,
) -> Result<WorkspaceConfig, AppError> {
    let ws = Workspace::open(Path::new(&path)).await?;
    let config = ws.config.masked();
    *state.workspace.write().await = Some(ws);
    state.cross_pools.write().await.clear();
    Ok(config)
}

#[tauri::command]
pub async fn create_database(
    state: State<'_, AppState>,
    name: String,
    role: DbRole,
) -> Result<Database, AppError> {
    let mut guard = state.workspace.write().await;
    let ws = guard.as_mut().ok_or(AppError::NoWorkspace)?;
    ws.create_database(name, role).await
}

#[tauri::command]
pub async fn switch_database(
    state: State<'_, AppState>,
    db_id: String,
) -> Result<WorkspaceConfig, AppError> {
    let config = {
        let mut guard = state.workspace.write().await;
        let ws = guard.as_mut().ok_or(AppError::NoWorkspace)?;
        ws.switch_database(&db_id).await?;
        ws.config.masked()
    };
    // Le cache peut contenir un pool pour la base qui devient active : le garder
    // ferait vivre 2 pools sur le même fichier (invariant WAL documenté dans
    // manager.rs/populate_index). Re-rempli paresseusement au prochain appel.
    state.cross_pools.write().await.clear();
    Ok(config)
}

#[tauri::command]
pub async fn delete_database(
    state: State<'_, AppState>,
    db_id: String,
) -> Result<WorkspaceConfig, AppError> {
    // Retirer le pool de la base à supprimer AVANT la suppression du fichier :
    // sur Windows, un fichier ouvert ne peut pas être supprimé (PermissionDenied
    // avalé par le best-effort de manager.rs → .db orphelin). Le reste du cache
    // sera vidé après pour l'invariant WAL.
    state.cross_pools.write().await.remove(&db_id);
    let config = {
        let mut guard = state.workspace.write().await;
        let ws = guard.as_mut().ok_or(AppError::NoWorkspace)?;
        ws.delete_database(&db_id).await?;
        ws.config.masked()
    };
    // Même logique que switch_database : vider tout le cache. Si la base qui
    // devient active y était (mise en cache pendant qu'elle était non-active),
    // garder son pool ferait vivre 2 pools sur le même fichier (invariant WAL).
    state.cross_pools.write().await.clear();
    Ok(config)
}

#[tauri::command]
pub async fn list_databases(state: State<'_, AppState>) -> Result<Vec<Database>, AppError> {
    let guard = state.workspace.read().await;
    let ws = guard.as_ref().ok_or(AppError::NoWorkspace)?;
    Ok(ws.config.databases.clone())
}

#[tauri::command]
pub async fn get_workspace_settings(state: State<'_, AppState>) -> Result<crate::db::models::WorkspaceSettings, AppError> {
    let guard = state.workspace.read().await;
    let ws = guard.as_ref().ok_or(AppError::NoWorkspace)?;
    Ok(ws.config.settings.masked())
}

#[tauri::command]
pub async fn update_workspace_settings(
    state: State<'_, AppState>,
    settings: crate::db::models::WorkspaceSettings,
) -> Result<crate::db::models::WorkspaceSettings, AppError> {
    let mut guard = state.workspace.write().await;
    let ws = guard.as_mut().ok_or(AppError::NoWorkspace)?;
    // validation légère
    if !["hybrid","lmstudio","openai","off"].contains(&settings.llm_provider.as_str()) {
        return Err(AppError::Msg("llm_provider invalide (hybrid|lmstudio|openai|off)".into()));
    }
    let dir = ws.dir.clone();
    let mut new_settings = settings;
    match new_settings.openai_api_key.as_str() {
        // Front n'a jamais la clé réelle : vide ou "***" = conserver l'existante.
        MASKED_API_KEY | "" => {
            new_settings.openai_api_key = ws.config.settings.openai_api_key.clone();
        }
        DELETE_API_KEY => {
            security::delete_api_key(&dir)?;
            new_settings.openai_api_key = String::new();
        }
        key => {
            security::store_api_key(&dir, key)?;
            new_settings.openai_api_key = key.to_string();
        }
    }
    ws.config.settings = new_settings;
    ws.save()?;
    Ok(ws.config.settings.masked())
}
