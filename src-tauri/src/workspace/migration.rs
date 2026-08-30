use crate::error::AppError;
use sqlx::{Row, SqlitePool};

pub async fn run_meta(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| AppError::Msg(format!("migration méta: {e}")))?;

    // `source_db_id` (table liée cross-DB) a été ajouté en éditant 0001 après
    // coup. SQLite n'a pas de `ADD COLUMN IF NOT EXISTS` et modifier le fichier
    // 0001 casserait le checksum des workspaces existants : on ajoute donc la
    // colonne ici, de façon idempotente, aux bases héritées qui en sont privées.
    let rows = sqlx::query("PRAGMA table_info(\"_tables\")")
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Msg(format!("inspection schéma: {e}")))?;
    let has_col = rows.iter().any(|r| {
        r.try_get::<String, _>("name")
            .map(|n| n == "source_db_id")
            .unwrap_or(false)
    });
    if !has_col {
        sqlx::query("ALTER TABLE \"_tables\" ADD COLUMN source_db_id TEXT")
            .execute(pool)
            .await
            .map_err(|e| AppError::Msg(format!("ajout source_db_id: {e}")))?;
    }

    Ok(())
}

pub async fn run_index(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::migrate!("./migrations_index")
        .run(pool)
        .await
        .map_err(|e| AppError::Msg(format!("migration index: {e}")))?;
    Ok(())
}
