use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::models::{Field, WorkspaceConfig};
use crate::db::repository;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSchema {
    pub db_id: String,
    pub db_name: String,
    pub tables: Vec<TableSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub id: String,
    pub name: String,
    pub fields: Vec<Field>,
    pub sample: Vec<serde_json::Value>,
    pub row_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationInfo {
    pub source_table_id: String,
    pub source_field_id: String,
    pub target_db_id: String,
    pub target_table_id: String,
    pub cardinality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AICrossDBContext {
    pub active_db_id: String,
    pub active_schema: Option<DatabaseSchema>,
    pub reference_schemas: Vec<DatabaseSchema>,
    pub relations: Vec<RelationInfo>,
    pub sample_limit: usize,
}

impl AICrossDBContext {
    pub fn all_schemas(&self) -> Vec<&DatabaseSchema> {
        let mut v = Vec::new();
        if let Some(a) = &self.active_schema {
            v.push(a);
        }
        for r in &self.reference_schemas {
            v.push(r);
        }
        v
    }
}

pub async fn build_context(
    config: &WorkspaceConfig,
    active_pool: &SqlitePool,
    other_pools: &HashMap<String, SqlitePool>,
    sample_limit: usize,
) -> Result<AICrossDBContext, crate::error::AppError> {
    let limit = sample_limit.clamp(5, 50);
    let active_db_id = config.active_database_id.clone();

    let mut active_schema: Option<DatabaseSchema> = None;
    let mut reference_schemas: Vec<DatabaseSchema> = Vec::new();
    let mut relations: Vec<RelationInfo> = Vec::new();

    for db in &config.databases {
        let pool = if db.id == active_db_id {
            active_pool
        } else if let Some(p) = other_pools.get(&db.id) {
            p
        } else {
            // try open ephemeral
            continue;
        };

        let tables = repository::list_tables(pool).await.unwrap_or_default();
        let mut t_schemas = Vec::new();
        for t in tables {
            let fields = repository::list_fields(pool, &t.id).await.unwrap_or_default();
            // row_count
            let row_count: i64 = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) FROM {}",
                crate::db::repository::quote_ident_public(&t.id)
            ))
            .fetch_one(pool)
            .await
            .unwrap_or(0);
            // sample: fetch first `limit` rows as PaginatedRecords via raw query lightweight
            let sample = fetch_sample(pool, &t.id, &fields, limit).await;
            // collect relations for this db
            let rels: Vec<(String, String, String, String, String)> = sqlx::query_as(
                "SELECT source_table_id, source_field_id, target_db_id, target_table_id, cardinality FROM _relations WHERE source_table_id = ?",
            )
            .bind(&t.id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            for (stid, sfid, tdb, ttid, card) in rels {
                relations.push(RelationInfo {
                    source_table_id: stid,
                    source_field_id: sfid,
                    target_db_id: tdb,
                    target_table_id: ttid,
                    cardinality: card,
                });
            }
            t_schemas.push(TableSchema {
                id: t.id.clone(),
                name: t.name.clone(),
                fields,
                sample,
                row_count,
            });
        }
        let schema = DatabaseSchema {
            db_id: db.id.clone(),
            db_name: db.name.clone(),
            tables: t_schemas,
        };
        if db.id == active_db_id {
            active_schema = Some(schema);
        } else {
            reference_schemas.push(schema);
        }
    }

    // also ensure relations cross-db where target outside active: already collected

    Ok(AICrossDBContext {
        active_db_id,
        active_schema,
        reference_schemas,
        relations,
        sample_limit: limit,
    })
}

async fn fetch_sample(
    pool: &SqlitePool,
    table_id: &str,
    fields: &[Field],
    limit: usize,
) -> Vec<serde_json::Value> {
    let stored: Vec<&Field> = fields.iter().filter(|f| f.is_stored()).collect();
    let mut cols = vec![crate::db::repository::quote_ident_public("_id")];
    for f in &stored {
        cols.push(crate::db::repository::quote_ident_public(&f.id));
    }
    let sql = format!(
        "SELECT {} FROM {} LIMIT {}",
        cols.join(", "),
        crate::db::repository::quote_ident_public(table_id),
        limit
    );
    let rows = match sqlx::query(&sql).fetch_all(pool).await {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let mut out = Vec::new();
    for row in rows {
        use sqlx::Row;
        let mut m = serde_json::Map::new();
        if let Ok(id) = row.try_get::<String, _>("_id") {
            m.insert("_id".into(), serde_json::json!(id));
        }
        for f in &stored {
            let v = match f.field_type.as_str() {
                "number" => row
                    .try_get::<Option<f64>, _>(f.id.as_str())
                    .ok()
                    .flatten()
                    .map(|x| serde_json::json!(x))
                    .unwrap_or(serde_json::Value::Null),
                "checkbox" => row
                    .try_get::<Option<i64>, _>(f.id.as_str())
                    .ok()
                    .flatten()
                    .map(|x| serde_json::json!(x != 0))
                    .unwrap_or(serde_json::Value::Null),
                _ => row
                    .try_get::<Option<String>, _>(f.id.as_str())
                    .ok()
                    .flatten()
                    .map(|x| serde_json::json!(x))
                    .unwrap_or(serde_json::Value::Null),
            };
            m.insert(f.id.clone(), v);
        }
        out.push(serde_json::Value::Object(m));
    }
    out
}

pub fn context_prompt(ctx: &AICrossDBContext) -> String {
    let mut s = String::new();
    s.push_str("Workspace schema:\n");
    for schema in ctx.all_schemas() {
        s.push_str(&format!("DB {} ({})\n", schema.db_name, schema.db_id));
        for t in &schema.tables {
            s.push_str(&format!("  Table {} ({}) rows={}\n", t.name, t.id, t.row_count));
            for f in &t.fields {
                s.push_str(&format!("    - {} ({}:{}) config={}\n", f.name, f.id, f.field_type, f.config));
            }
            if !t.sample.is_empty() {
                s.push_str("    sample:\n");
                for rec in t.sample.iter().take(3) {
                    s.push_str(&format!("      {}\n", rec));
                }
            }
        }
    }
    if !ctx.relations.is_empty() {
        s.push_str("Relations:\n");
        for r in &ctx.relations {
            s.push_str(&format!(
                "  {}:{} -> {}:{} ({})\n",
                r.source_table_id, r.source_field_id, r.target_db_id, r.target_table_id, r.cardinality
            ));
        }
    }
    s
}
