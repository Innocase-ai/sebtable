use tauri::State;

use crate::ai::analysis;
use crate::ai::cleaning;
use crate::ai::context_builder;
use crate::ai::formula;
use crate::ai::provider;
use crate::ai::relation_suggest;
use crate::commands::{other_db_pools, pool_for_db};
use crate::db::models::WorkspaceConfig;
use crate::error::AppError;
use crate::AppState;

/// Clone la config du workspace SANS tenir le RwLock pendant les appels
/// réseau/DB ensuite (sinon switch_database serait bloqué jusqu'au timeout LLM).
async fn snapshot_config(state: &State<'_, AppState>) -> Result<WorkspaceConfig, AppError> {
    let guard = state.workspace.read().await;
    let ws = guard.as_ref().ok_or(AppError::NoWorkspace)?;
    Ok(ws.config.clone())
}

#[tauri::command]
pub async fn ai_suggest_relations(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
) -> Result<Vec<relation_suggest::RelationSuggestion>, AppError> {
    let config = snapshot_config(&state).await?;
    let pool = pool_for_db(&state, &db_id).await?;
    let other = other_db_pools(&state, &db_id).await;
    let ctx = context_builder::build_context(&config, &pool, &other, 50).await?;
    // Heuristique déterministe (le re-ranking LLM n'apporte rien au MVP).
    Ok(relation_suggest::suggest_relations(&ctx, &table_id))
}

#[tauri::command]
pub async fn ai_generate_formula(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    prompt: String,
) -> Result<formula::FormulaResult, AppError> {
    let config = snapshot_config(&state).await?;
    let pool = pool_for_db(&state, &db_id).await?;
    let other = other_db_pools(&state, &db_id).await;
    let ctx = context_builder::build_context(&config, &pool, &other, 50).await?;
    // Hors lock : get_provider peut pinger LM Studio (réseau).
    let provider = provider::get_provider(&config.settings).await;
    let prov_ref = provider.as_deref().map(|p| p as &dyn provider::LLMProvider);
    Ok(formula::generate_formula(&ctx, &table_id, &prompt, prov_ref).await)
}

#[tauri::command]
pub async fn ai_analyze(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    question: Option<String>,
) -> Result<analysis::AnalysisResult, AppError> {
    let config = snapshot_config(&state).await?;
    let pool = pool_for_db(&state, &db_id).await?;
    let other = other_db_pools(&state, &db_id).await;
    let ctx = context_builder::build_context(&config, &pool, &other, 50).await?;
    let provider = provider::get_provider(&config.settings).await;
    let prov_ref = provider.as_deref().map(|p| p as &dyn provider::LLMProvider);
    Ok(analysis::analyze(&ctx, Some(&table_id), question.as_deref(), prov_ref).await)
}

#[tauri::command]
pub async fn ai_clean_preview(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    instruction: String,
) -> Result<cleaning::TransformPlan, AppError> {
    let config = snapshot_config(&state).await?;
    let pool = pool_for_db(&state, &db_id).await?;
    let other = other_db_pools(&state, &db_id).await;
    let ctx = context_builder::build_context(&config, &pool, &other, 50).await?;
    let provider = provider::get_provider(&config.settings).await;
    let prov_ref = provider.as_deref().map(|p| p as &dyn provider::LLMProvider);
    Ok(cleaning::preview_with_llm(&ctx, &table_id, &instruction, prov_ref).await)
}

#[tauri::command]
pub async fn ai_apply_transform(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    plan: cleaning::TransformPlan,
) -> Result<cleaning::TransformResult, AppError> {
    let _ = snapshot_config(&state).await?; // exige un workspace ouvert, sans le garder
    let pool = pool_for_db(&state, &db_id).await?;
    cleaning::apply_transform(&pool, &table_id, &plan).await
}

#[tauri::command]
pub async fn ai_check_status(state: State<'_, AppState>) -> Result<provider::ProviderStatus, AppError> {
    let config = snapshot_config(&state).await?;
    // Hors lock : ping réseau LM Studio.
    Ok(provider::check_status(&config.settings).await)
}
