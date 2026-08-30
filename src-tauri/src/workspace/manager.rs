use std::path::{Path, PathBuf};

use sqlx::{Row, SqlitePool};

use crate::db::connection::open_pool;
use crate::db::models::{Database, DbRole, WorkspaceConfig, WorkspaceSettings};
use crate::error::AppError;
use crate::security;
use crate::utils::new_id;

use super::migration;

pub struct Workspace {
    pub config: WorkspaceConfig,
    pub dir: PathBuf,
    pub pool: SqlitePool,
    pub index_pool: SqlitePool,
}

impl Workspace {
    pub async fn create(dir: &Path, name: String) -> Result<Self, AppError> {
        std::fs::create_dir_all(dir)?;
        std::fs::create_dir_all(dir.join("databases"))?;

        let db_id = new_id("db");
        let rel_path = format!("databases/{db_id}.db");

        let database = Database {
            id: db_id.clone(),
            path: rel_path,
            role: DbRole::Project,
            name: "Base principale".into(),
        };
        let config = WorkspaceConfig {
            name,
            version: 1,
            databases: vec![database],
            active_database_id: db_id,
            settings: WorkspaceSettings::default(),
        };
        std::fs::write(
            dir.join("workspace.json"),
            serde_json::to_string_pretty(&config.masked())?,
        )?;

        Self::open(dir).await
    }

    pub async fn open(dir: &Path) -> Result<Self, AppError> {
        let config_path = dir.join("workspace.json");
        if !config_path.exists() {
            return Err(AppError::Msg(
                "workspace.json introuvable dans ce dossier".into(),
            ));
        }
        let mut config: WorkspaceConfig =
            serde_json::from_str(&std::fs::read_to_string(&config_path)?)?;

        // Clé OpenAI : jamais stockée en clair dans workspace.json.
        // 1) Migration d'un ancien workspace.json contenant la clé en clair.
        let legacy_key = config.settings.openai_api_key.clone();
        if !legacy_key.trim().is_empty() {
            security::store_api_key(dir, &legacy_key)?;
            // Purger la clé du fichier immédiatement ; l'état mémoire garde la vraie clé.
            std::fs::write(&config_path, serde_json::to_string_pretty(&config.masked())?)?;
        }
        // 2) Chargement depuis le keychain OS (workspace.json ne contient que "").
        if !config.settings.has_api_key() {
            match security::load_api_key(dir) {
                Ok(Some(k)) => config.settings.openai_api_key = k,
                Ok(None) => {}
                Err(e) => eprintln!("[security] keychain illisible, clé OpenAI ignorée : {e}"),
            }
        }

        let pool = open_active_pool(dir, &config).await?;
        let index_pool = open_index_pool(dir).await?;
        populate_index(&index_pool, &pool, &config.active_database_id, dir, &config).await?;

        Ok(Self {
            config,
            dir: dir.to_path_buf(),
            pool,
            index_pool,
        })
    }

    pub async fn switch_database(&mut self, db_id: &str) -> Result<(), AppError> {
        let db = self
            .config
            .databases
            .iter()
            .find(|d| d.id == db_id)
            .ok_or_else(|| AppError::Msg("base introuvable".into()))?
            .clone();

        let pool = open_pool(&self.dir.join(&db.path)).await?;
        migration::run_meta(&pool).await?;

        self.pool.close().await;
        self.pool = pool;
        self.config.active_database_id = db_id.to_string();
        self.save()?;
        Ok(())
    }

    pub async fn create_database(&mut self, name: String, role: DbRole) -> Result<Database, AppError> {
        let id = new_id("db");
        let rel_path = format!("databases/{id}.db");
        let pool = open_pool(&self.dir.join(&rel_path)).await?;
        migration::run_meta(&pool).await?;
        pool.close().await;

        let db = Database {
            id,
            path: rel_path,
            role,
            name,
        };
        self.config.databases.push(db.clone());
        self.save()?;
        populate_index(&self.index_pool, &self.pool, &self.config.active_database_id, &self.dir, &self.config).await?;
        Ok(db)
    }

    pub async fn delete_database(&mut self, db_id: &str) -> Result<(), AppError> {
        if self.config.databases.len() <= 1 {
            return Err(AppError::Msg(
                "Impossible de supprimer la dernière base (il en faut au moins une)".into(),
            ));
        }
        let pos = self
            .config
            .databases
            .iter()
            .position(|d| d.id == db_id)
            .ok_or_else(|| AppError::Msg("base introuvable".into()))?;
        let db = self.config.databases.remove(pos);
        let was_active = self.config.active_database_id == db_id;

        if was_active {
            // Basculer sur la première base restante avant de supprimer le fichier
            let next = self.config.databases[0].clone();
            let pool = open_pool(&self.dir.join(&next.path)).await?;
            migration::run_meta(&pool).await?;
            self.pool.close().await;
            self.pool = pool;
            self.config.active_database_id = next.id;
        }

        // Persister l'intention AVANT de supprimer des fichiers : si le save
        // échoue, workspace.json ne doit pas référencer un fichier disparu.
        self.save()?;

        // Supprimer le fichier SQLite + wal/shm
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let p = self.dir.join(format!("{}{}", db.path, suffix));
            let _ = std::fs::remove_file(p);
        }

        populate_index(
            &self.index_pool,
            &self.pool,
            &self.config.active_database_id,
            &self.dir,
            &self.config,
        )
        .await?;
        Ok(())
    }

    pub(crate) fn save(&self) -> Result<(), AppError> {
        std::fs::write(
            self.dir.join("workspace.json"),
            serde_json::to_string_pretty(&self.config.masked())?,
        )?;
        Ok(())
    }
}

async fn open_active_pool(dir: &Path, config: &WorkspaceConfig) -> Result<SqlitePool, AppError> {
    let active = config
        .databases
        .iter()
        .find(|d| d.id == config.active_database_id)
        .ok_or_else(|| AppError::Msg("base active introuvable".into()))?;
    let pool = open_pool(&dir.join(&active.path)).await?;
    migration::run_meta(&pool).await?;
    Ok(pool)
}

async fn open_index_pool(dir: &Path) -> Result<SqlitePool, AppError> {
    let cache = dir.join(".cache");
    std::fs::create_dir_all(&cache)?;
    let pool = open_pool(&cache.join("workspace_index.db")).await?;
    migration::run_index(&pool).await?;
    Ok(pool)
}

async fn populate_index(
    index_pool: &SqlitePool,
    active_pool: &SqlitePool,
    active_db_id: &str,
    dir: &Path,
    config: &WorkspaceConfig,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM workspace_tables")
        .execute(index_pool)
        .await?;

    for db in &config.databases {
        // La base active partage déjà un pool vivant : l'utiliser directement
        // pour éviter un 2e pool sur le même fichier (race de schéma WAL).
        // Les autres bases (fichiers distincts) ouvrent un pool temporaire.
        let rows = if db.id == active_db_id {
            sqlx::query("SELECT id, name FROM _tables")
                .fetch_all(active_pool)
                .await?
        } else {
            let tmp = open_pool(&dir.join(&db.path)).await?;
            let r = sqlx::query("SELECT id, name FROM _tables")
                .fetch_all(&tmp)
                .await?;
            tmp.close().await;
            r
        };
        for row in rows {
            let table_id: String = row.try_get("id")?;
            let name: String = row.try_get("name")?;
            sqlx::query("INSERT OR REPLACE INTO workspace_tables (db_id, table_id, name) VALUES (?, ?, ?)")
                .bind(&db.id)
                .bind(&table_id)
                .bind(&name)
                .execute(index_pool)
                .await?;
        }
    }
    Ok(())
}
