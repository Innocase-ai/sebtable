pub mod ai;
pub mod attachments;
pub mod import_export;
pub mod search;
pub mod tables;
pub mod views;
pub mod workspace;

use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::db::connection::open_pool;
use crate::error::AppError;
use crate::workspace::migration;
use crate::AppState;

/// Pool de la base demandée : base active → pool vivant, sinon ouverture du
/// fichier (Phase 3 cross-DB, permet de lire les tables d'une autre base).
/// Réutilise le cache `cross_pools` pour éviter de rouvrir un 2e pool sur le
/// même fichier (invariant WAL `max_connections(1)`).
pub(crate) async fn pool_for_db(state: &AppState, db_id: &str) -> Result<SqlitePool, AppError> {
    let (active_id, is_active, cached) = {
        let guard = state.workspace.read().await;
        let ws = guard.as_ref().ok_or(AppError::NoWorkspace)?;
        if db_id == ws.config.active_database_id {
            return Ok(ws.pool.clone());
        }
        let cached = {
            let c = state.cross_pools.read().await;
            c.get(db_id).cloned()
        };
        (ws.config.active_database_id.clone(), false, cached)
    };
    let _ = active_id;
    let _ = is_active;
    if let Some(pool) = cached {
        return Ok(pool);
    }
    // besoin du path hors du lock
    let (dir, path) = {
        let guard = state.workspace.read().await;
        let ws = guard.as_ref().ok_or(AppError::NoWorkspace)?;
        let db = ws
            .config
            .databases
            .iter()
            .find(|d| d.id == db_id)
            .ok_or_else(|| AppError::Msg("base introuvable".into()))?;
        (ws.dir.clone(), db.path.clone())
    };
    let pool = open_pool(&dir.join(&path)).await?;
    migration::run_meta(&pool).await?;
    {
        let mut cache = state.cross_pools.write().await;
        cache.insert(db_id.to_string(), pool.clone());
    }
    Ok(pool)
}

/// Pools des autres bases (hors base courante) pour la résolution cross-DB
/// côté repository. Les pools sont mis en cache dans `AppState::cross_pools`
/// (un `open_pool` par base au premier appel AI, réutilisé ensuite).
/// Ne jamais ouvrir un 2e pool sur le fichier de la base active : on
/// réutilise `workspace.pool` pour celui-ci (invariant `max_connections(1)`).
pub(crate) async fn other_db_pools(
    state: &AppState,
    current_db_id: &str,
) -> HashMap<String, SqlitePool> {
    // Snapshot de la config + pool actif hors lock pour ne pas la tenir pendant open_pool.
    let (databases, dir, active_id, active_pool) = {
        let guard = state.workspace.read().await;
        let Some(ws) = guard.as_ref() else {
            return HashMap::new();
        };
        (ws.config.databases.clone(), ws.dir.clone(), ws.config.active_database_id.clone(), ws.pool.clone())
    };
    let mut map = HashMap::new();
    for db in &databases {
        if db.id == current_db_id {
            continue;
        }
        if db.id == active_id {
            map.insert(db.id.clone(), active_pool.clone());
            continue;
        }
        // Cache hit ?
        {
            let cache = state.cross_pools.read().await;
            if let Some(pool) = cache.get(&db.id) {
                map.insert(db.id.clone(), pool.clone());
                continue;
            }
        }
        if let Ok(pool) = open_pool(&dir.join(&db.path)).await {
            {
                let mut cache = state.cross_pools.write().await;
                cache.insert(db.id.clone(), pool.clone());
            }
            map.insert(db.id.clone(), pool);
        }
    }
    map
}
