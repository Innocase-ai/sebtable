use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

pub async fn open_pool(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(10));

    SqlitePoolOptions::new()
        // Une seule connexion : évite les races de schéma SQLite quand un DDL
        // (ALTER/CREATE/DROP) sur une connexion n'est pas vu par une autre
        // connexion du pool dont le schéma est figé ("no such column" /
        // ColumnNotFound). Un éditeur de tables local n'a pas besoin de
        // parallélisme de connexions.
        .max_connections(1)
        .connect_with(opts)
        .await
}
