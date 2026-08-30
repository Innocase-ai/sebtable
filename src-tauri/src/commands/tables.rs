use serde_json::Value;
use tauri::State;

use crate::commands::{other_db_pools, pool_for_db};
use crate::error::AppError;
use crate::db::models::{
    Field, FieldChanges, FieldInput, LinkFieldConfig, LinkTarget, PaginatedRecords, Table,
    TableChanges, ViewConfig,
};
use crate::db::repository;
use crate::AppState;

#[tauri::command]
pub async fn list_tables(state: State<'_, AppState>, db_id: String) -> Result<Vec<Table>, AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::list_tables(&pool).await
}

#[tauri::command]
pub async fn create_table(
    state: State<'_, AppState>,
    db_id: String,
    name: String,
    fields: Vec<FieldInput>,
    source_db_id: Option<String>,
) -> Result<Table, AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::create_table(&pool, name, fields, source_db_id).await
}

#[tauri::command]
pub async fn get_record_with_relations(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    record_id: String,
    depth: Option<u8>,
) -> Result<crate::db::models::RecordWithRelations, AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    let db_pools = other_db_pools(&state, &db_id).await;
    repository::get_record_with_relations(&pool, &table_id, &record_id, depth.unwrap_or(1), &db_pools, &db_id).await
}

#[tauri::command]
pub async fn update_table(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    changes: TableChanges,
) -> Result<(), AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::update_table(&pool, &table_id, &changes).await
}

#[tauri::command]
pub async fn delete_table(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
) -> Result<(), AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::delete_table(&pool, &table_id).await
}

#[tauri::command]
pub async fn list_fields(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
) -> Result<Vec<Field>, AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::list_fields(&pool, &table_id).await
}

#[tauri::command]
pub async fn create_field(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    field: FieldInput,
) -> Result<Field, AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::create_field(&pool, &table_id, field).await
}

#[tauri::command]
pub async fn update_field(
    state: State<'_, AppState>,
    db_id: String,
    field_id: String,
    changes: FieldChanges,
) -> Result<(), AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::update_field(&pool, &field_id, &changes).await
}

#[tauri::command]
pub async fn delete_field(
    state: State<'_, AppState>,
    db_id: String,
    field_id: String,
) -> Result<(), AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::delete_field(&pool, &field_id).await
}

#[tauri::command]
pub async fn get_table_data(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    view_config: ViewConfig,
    // Paramètre legacy (était: include lookups) — lookups/rollups sont désormais
    // toujours résolus via `compute_computed_fields`; gardé pour compat API.
    _include_lookups: Option<bool>,
) -> Result<PaginatedRecords, AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    let db_pools = other_db_pools(&state, &db_id).await;
    repository::get_table_data(&pool, &table_id, &view_config, &db_pools, &db_id).await
}

#[tauri::command]
pub async fn upsert_records(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    records: Vec<Value>,
) -> Result<Vec<Value>, AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::upsert_records(&pool, &table_id, records).await
}

#[tauri::command]
pub async fn delete_records(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    ids: Vec<String>,
) -> Result<(), AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::delete_records(&pool, &table_id, &ids).await
}

#[tauri::command]
pub async fn create_link_field(
    state: State<'_, AppState>,
    db_id: String,
    source_table_id: String,
    name: String,
    config: LinkFieldConfig,
) -> Result<Field, AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::create_link_field(&pool, &source_table_id, name, config).await
}

#[tauri::command]
pub async fn link_records(
    state: State<'_, AppState>,
    db_id: String,
    link_field_id: String,
    source_record_id: String,
    targets: Vec<LinkTarget>,
) -> Result<(), AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::link_records(&pool, &link_field_id, &source_record_id, targets).await
}

#[tauri::command]
pub async fn unlink_records(
    state: State<'_, AppState>,
    db_id: String,
    link_field_id: String,
    source_record_id: String,
    target_ids: Vec<String>,
) -> Result<(), AppError> {
    let pool = pool_for_db(&state, &db_id).await?;
    repository::unlink_records(&pool, &link_field_id, &source_record_id, &target_ids).await
}
