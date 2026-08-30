use tauri::State;

use crate::commands::pool_for_db;
use crate::error::AppError;
use crate::db::models::{View, ViewConfig, ViewInput};
use crate::db::repository;
use crate::AppState;

#[tauri::command]
pub async fn list_views(state: State<'_, AppState>, db_id: String, table_id: String) -> Result<Vec<View>, AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::list_views(&pool, &table_id).await
}

#[tauri::command]
pub async fn create_view(state: State<'_, AppState>, db_id: String, view: ViewInput) -> Result<View, AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::create_view(&pool, view).await
}

#[tauri::command]
pub async fn update_view(
    state: State<'_, AppState>,
    db_id: String,
    view_id: String,
    config: ViewConfig,
) -> Result<(), AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::update_view(&pool, &view_id, &config).await
}

#[tauri::command]
pub async fn delete_view(
    state: State<'_, AppState>,
    db_id: String,
    view_id: String,
) -> Result<(), AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::delete_view(&pool, &view_id).await
}
