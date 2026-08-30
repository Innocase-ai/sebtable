use serde::{Deserialize, Serialize};
use sqlx::Row;
use crate::AppState;
use crate::error::AppError;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub db_id: String,
    pub db_name: String,
    pub table_id: String,
    pub table_name: String,
    pub record_id: String,
    pub field_id: String,
    pub field_name: String,
    pub snippet: String,
}

#[tauri::command]
pub async fn search_workspace(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<SearchResult>, AppError> {
    let q = query.trim();
    if q.is_empty() || q.len() < 2 {
        return Ok(vec![]);
    }
    let q_lower = q.to_lowercase();
    let pattern = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let (databases, dir, active_id, active_pool) = {
        let guard = state.workspace.read().await;
        let Some(ws) = guard.as_ref() else { return Err(AppError::NoWorkspace); };
        (ws.config.databases.clone(), ws.dir.clone(), ws.config.active_database_id.clone(), ws.pool.clone())
    };
    let mut out: Vec<SearchResult> = Vec::new();
    for db in &databases {
        let pool = if db.id == active_id {
            active_pool.clone()
        } else {
            // réutilise cache si dispo
            let cached = {
                let c = state.cross_pools.read().await;
                c.get(&db.id).cloned()
            };
            if let Some(p) = cached { p } else {
                // Fichier absent/corrompu → on saute cette base plutôt que
                // d'échouer toute la recherche (cohérent avec le reste).
                let Ok(p) = crate::db::connection::open_pool(&dir.join(&db.path)).await else { continue };
                let _ = crate::workspace::migration::run_meta(&p).await;
                let mut cache = state.cross_pools.write().await;
                cache.insert(db.id.clone(), p.clone());
                p
            }
        };
        let tables = crate::db::repository::list_tables(&pool).await.unwrap_or_default();
        for tbl in tables {
            // recherche nom de table
            if tbl.name.to_lowercase().contains(&q_lower) {
                out.push(SearchResult {
                    db_id: db.id.clone(),
                    db_name: db.name.clone(),
                    table_id: tbl.id.clone(),
                    table_name: tbl.name.clone(),
                    record_id: String::new(),
                    field_id: String::new(),
                    field_name: String::new(),
                    snippet: format!("Table: {}", tbl.name),
                });
                if out.len() >= 30 { break; }
            }
            let fields = crate::db::repository::list_fields(&pool, &tbl.id).await.unwrap_or_default();
            let text_fields: Vec<_> = fields.iter().filter(|f| matches!(f.field_type.as_str(), "text" | "long_text" | "email" | "url" | "phone" | "select")).collect();
            if text_fields.is_empty() { continue; }
            for field in text_fields {
                // LIKE insensible à la casse via lower()
                let col = crate::db::repository::quote_ident_public(&field.id);
                let tbl_q = crate::db::repository::quote_ident_public(&tbl.id);
                let sql = format!("SELECT _id, {} FROM {} WHERE lower({}) LIKE lower(?) ESCAPE '\\' LIMIT 5", col, tbl_q, col);
                let rows = sqlx::query(&sql).bind(&pattern).fetch_all(&pool).await.unwrap_or_default();
                // select : map option id → libellé (config.options)
                let select_label = |id: &str| -> Option<String> {
                    if field.field_type != "select" { return None; }
                    field.config.get("options")?.as_array()?.iter().find_map(|o| {
                        if o.get("id").and_then(|v| v.as_str()) == Some(id) { o.get("name").and_then(|n| n.as_str()).map(String::from) } else { None }
                    })
                };
                for row in rows {
                    let rid: String = row.try_get("_id").unwrap_or_default();
                    let val: Option<String> = row.try_get::<Option<String>, _>(field.id.as_str()).ok().flatten();
                    let mut snippet = val.unwrap_or_default();
                    // select : la colonne stocke le JSON `"opt_1"` → afficher le libellé
                    if field.field_type == "select" {
                        let raw = snippet.trim_matches('"').to_string();
                        snippet = select_label(&raw).unwrap_or(raw);
                    }
                    // découpe en graphemes/chars, pas en octets (panic UTF-8 sinon)
                    let short: String = snippet.chars().take(80).collect();
                    let short = if snippet.chars().count() > 80 { format!("{short}…") } else { short };
                    out.push(SearchResult {
                        db_id: db.id.clone(),
                        db_name: db.name.clone(),
                        table_id: tbl.id.clone(),
                        table_name: tbl.name.clone(),
                        record_id: rid,
                        field_id: field.id.clone(),
                        field_name: field.name.clone(),
                        snippet: short,
                    });
                    if out.len() >= 30 { break; }
                }
                if out.len() >= 30 { break; }
            }
            if out.len() >= 30 { break; }
        }
        if out.len() >= 30 { break; }
    }
    Ok(out)
}
