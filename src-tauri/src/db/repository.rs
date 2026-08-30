use std::collections::{HashMap, HashSet};

use crate::db::models::{
    CountFieldConfig, Field, FieldChanges, FieldInput, LinkFieldConfig, LinkTarget, LinkValue,
    LookupFieldConfig, PaginatedRecords, RecordWithRelations, RollupFieldConfig, Table,
    TableChanges, View, ViewConfig, ViewInput,
};
use crate::error::AppError;
use crate::formula;
use crate::utils::new_id;
use serde_json::{json, Value};
use sqlx::sqlite::{Sqlite, SqliteRow, SqliteTransaction};
use sqlx::{Executor, QueryBuilder, Row, SqlitePool};

fn quote_ident(id: &str) -> String {
    format!("\"{}\"", id.replace('"', "\"\""))
}

pub fn quote_ident_public(id: &str) -> String {
    quote_ident(id)
}

fn sqlite_col_type(field_type: &str) -> &'static str {
    match field_type {
        "number" => "REAL",
        "checkbox" => "INTEGER",
        "created_time" | "last_modified_time" => "INTEGER",
        _ => "TEXT",
    }
}

// ---- Tables ---------------------------------------------------------------

pub async fn list_tables(pool: &SqlitePool) -> Result<Vec<Table>, AppError> {
    let rows = sqlx::query("SELECT id, name, source_db_id FROM _tables ORDER BY created_at ASC, name ASC")
        .fetch_all(pool)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(Table {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            source_db_id: row.try_get("source_db_id")?,
        });
    }
    Ok(out)
}

pub async fn create_table(
    pool: &SqlitePool,
    name: String,
    fields: Vec<FieldInput>,
    source_db_id: Option<String>,
) -> Result<Table, AppError> {
    let table_id = new_id("tbl");

    // Les types relationnels/exports nécessitent une cible et une config :
    // seul create_link_field / la fiche champ les gère correctement.
    for f in &fields {
        match f.field_type.as_str() {
            "link" | "lookup" | "rollup" | "count" | "formula" | "button" => {
                return Err(AppError::Msg(format!(
                    "Type de champ '{}' non supporté à la création d'une table : créez la table puis ce champ via la fiche champ",
                    f.field_type
                )));
            }
            _ => {}
        }
    }

    let mut ddl = format!(
        "CREATE TABLE {} ({} TEXT PRIMARY KEY",
        quote_ident(&table_id),
        quote_ident("_id")
    );

    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO _tables (id, name, source_db_id) VALUES (?, ?, ?)")
        .bind(&table_id)
        .bind(&name)
        .bind(&source_db_id)
        .execute(&mut *tx)
        .await?;

    for (i, f) in fields.into_iter().enumerate() {
        let field_id = new_id("fld");
        let tmp = Field {
            id: field_id.clone(),
            table_id: table_id.clone(),
            name: f.name.clone(),
            field_type: f.field_type.clone(),
            config: f.config.clone(),
            position: i as i64,
        };
        if tmp.is_stored() {
            ddl.push_str(&format!(
                ", {} {}",
                quote_ident(&field_id),
                sqlite_col_type(&f.field_type)
            ));
        }
        sqlx::query("INSERT INTO _fields (id, table_id, name, \"type\", config, position) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&field_id)
            .bind(&table_id)
            .bind(&f.name)
            .bind(&f.field_type)
            .bind(f.config.to_string())
            .bind(i as i64)
            .execute(&mut *tx)
            .await?;
    }
    ddl.push(')');
    sqlx::query(&ddl).execute(&mut *tx).await?;
    tx.commit().await?;

    Ok(Table {
        id: table_id,
        name,
        source_db_id,
    })
}

pub async fn update_table(
    pool: &SqlitePool,
    table_id: &str,
    changes: &TableChanges,
) -> Result<(), AppError> {
    if let Some(name) = &changes.name {
        sqlx::query("UPDATE _tables SET name = ?, updated_at = strftime('%s','now') WHERE id = ?")
            .bind(name)
            .bind(table_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn delete_table(pool: &SqlitePool, table_id: &str) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    // Champs de lien d'autres tables ciblant cette table : suppression en cascade
    let pattern = format!("%\"target_table_id\":\"{table_id}\"%");
    let rows = sqlx::query("SELECT id FROM _fields WHERE \"type\" = 'link' AND table_id != ? AND config LIKE ?")
        .bind(table_id)
        .bind(&pattern)
        .fetch_all(&mut *tx)
        .await?;
    for row in rows {
        let fid: String = row.try_get("id")?;
        delete_link_field_meta(&mut tx, &fid).await?;
    }

    sqlx::query(&format!("DROP TABLE IF EXISTS {}", quote_ident(table_id)))
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM _tables WHERE id = ?")
        .bind(table_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM _fields WHERE table_id = ?")
        .bind(table_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM _views WHERE table_id = ?")
        .bind(table_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM _relations WHERE source_table_id = ? OR target_table_id = ?")
        .bind(table_id)
        .bind(table_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

// ---- Fields ---------------------------------------------------------------

pub async fn list_fields(pool: &SqlitePool, table_id: &str) -> Result<Vec<Field>, AppError> {
    let rows = sqlx::query(
        "SELECT id, table_id, name, \"type\" AS field_type, config, position FROM _fields WHERE table_id = ? ORDER BY position ASC",
    )
    .bind(table_id)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(field_from_row(&row)?);
    }
    Ok(out)
}

pub async fn get_field(pool: &SqlitePool, field_id: &str) -> Result<Field, AppError> {
    let row = sqlx::query(
        "SELECT id, table_id, name, \"type\" AS field_type, config, position FROM _fields WHERE id = ?",
    )
    .bind(field_id)
    .fetch_optional(pool)
    .await?;
    match row {
        Some(r) => field_from_row(&r),
        None => Err(AppError::Msg("champ introuvable".into())),
    }
}

fn field_from_row(row: &SqliteRow) -> Result<Field, AppError> {
    let config_str: String = row.try_get("config").unwrap_or_else(|_| "{}".to_string());
    let config: Value = serde_json::from_str(&config_str).unwrap_or_else(|_| json!({}));
    Ok(Field {
        id: row.try_get("id")?,
        table_id: row.try_get("table_id")?,
        name: row.try_get("name")?,
        field_type: row.try_get("field_type")?,
        config,
        position: row.try_get("position")?,
    })
}

pub async fn create_field(
    pool: &SqlitePool,
    table_id: &str,
    input: FieldInput,
) -> Result<Field, AppError> {
    // Un champ `link` exige une relation (`_relations`, backlink) : seul
    // create_link_field les crée. Sans garde, `create_field` produirait une
    // colonne TEXT sans relation ni cascade de nettoyage.
    // (lookup/rollup/count/formula sont calculés → autorisés ici, ils
    // référencent un champ de lien existant.)
    if input.field_type == "link" {
        return Err(AppError::Msg(
            "Type de champ 'link' non supporté via create_field : utilisez create_link_field".into(),
        ));
    }

    let field_id = new_id("fld");
    let tmp = Field {
        id: field_id.clone(),
        table_id: table_id.to_string(),
        name: input.name.clone(),
        field_type: input.field_type.clone(),
        config: input.config.clone(),
        position: 0,
    };

    let mut tx = pool.begin().await?;
    if tmp.is_stored() {
        sqlx::query(&format!(
            "ALTER TABLE {} ADD COLUMN {} {}",
            quote_ident(table_id),
            quote_ident(&field_id),
            sqlite_col_type(&input.field_type)
        ))
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "INSERT INTO _fields (id, table_id, name, \"type\", config, position) \
         VALUES (?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM _fields WHERE table_id = ?))",
    )
    .bind(&field_id)
    .bind(table_id)
    .bind(&input.name)
    .bind(&input.field_type)
    .bind(input.config.to_string())
    .bind(table_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(tmp)
}

pub async fn update_field(
    pool: &SqlitePool,
    field_id: &str,
    changes: &FieldChanges,
) -> Result<(), AppError> {
    if let Some(name) = &changes.name {
        sqlx::query("UPDATE _fields SET name = ? WHERE id = ?")
            .bind(name)
            .bind(field_id)
            .execute(pool)
            .await?;
    }
    if let Some(config) = &changes.config {
        sqlx::query("UPDATE _fields SET config = ? WHERE id = ?")
            .bind(config.to_string())
            .bind(field_id)
            .execute(pool)
            .await?;
    }
    if let Some(position) = changes.position {
        sqlx::query("UPDATE _fields SET position = ? WHERE id = ?")
            .bind(position)
            .bind(field_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn delete_field(pool: &SqlitePool, field_id: &str) -> Result<(), AppError> {
    let field = get_field(pool, field_id).await?;
    let table_id = field.table_id.clone();

    let mut tx = pool.begin().await?;
    if field.is_stored() {
        sqlx::query(&format!(
            "ALTER TABLE {} DROP COLUMN {}",
            quote_ident(&table_id),
            quote_ident(field_id)
        ))
        .execute(&mut *tx)
        .await?;
    }
    delete_link_field_meta(&mut tx, field_id).await?;
    // Cascade : champs calculés (lookup/rollup) dont ce champ est la CIBLE
    // (`target_field_id`) — sans cela la grille échouerait sur colonne absente.
    let target_pattern = format!("%\"target_field_id\":\"{field_id}\"%");
    let deps: Vec<String> = sqlx::query("SELECT id FROM _fields WHERE config LIKE ? AND id != ?")
        .bind(&target_pattern)
        .bind(field_id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .filter_map(|r| r.try_get::<String, _>("id").ok())
        .collect();
    for did in deps {
        sqlx::query("DELETE FROM _fields WHERE id = ?")
            .bind(&did)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

// Supprime les méta d'un champ de lien (forward + backlink) dans une transaction.
async fn delete_link_field_meta(
    tx: &mut SqliteTransaction<'_>,
    field_id: &str,
) -> Result<(), AppError> {
    let row = sqlx::query(
        "SELECT id, table_id, name, \"type\" AS field_type, config, position FROM _fields WHERE id = ?",
    )
    .bind(field_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(());
    };
    let field = field_from_row(&row)?;
    let forward_is_link = field.field_type == "link" && !field.is_backlink();
    if forward_is_link {
        // Cascade : backlink + champs calculés (lookup/rollup/count) qui
        // référencent ce champ de lien via `source_link_field_id`.
        let pattern = format!("%\"source_link_field_id\":\"{field_id}\"%");
        let deps = sqlx::query("SELECT id FROM _fields WHERE config LIKE ? AND id != ?")
            .bind(&pattern)
            .bind(field_id)
            .fetch_all(&mut **tx)
            .await?;
        for r in deps {
            let did: String = r.try_get("id")?;
            sqlx::query("DELETE FROM _fields WHERE id = ?")
                .bind(&did)
                .execute(&mut **tx)
                .await?;
        }
        sqlx::query("DELETE FROM _relations WHERE source_field_id = ?")
            .bind(field_id)
            .execute(&mut **tx)
            .await?;
    }
    sqlx::query("DELETE FROM _fields WHERE id = ?")
        .bind(field_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

// ---- Relations / liens ----------------------------------------------------

pub async fn create_link_field(
    pool: &SqlitePool,
    source_table_id: &str,
    name: String,
    config: LinkFieldConfig,
) -> Result<Field, AppError> {
    // target_db_id = '' → intra-DB, sinon cross-DB (Phase 3)
    if config.target_db_id.is_empty() {
        let target_exists = sqlx::query("SELECT 1 FROM _tables WHERE id = ?")
            .bind(&config.target_table_id)
            .fetch_optional(pool)
            .await?
            .is_some();
        if !target_exists {
            return Err(AppError::Msg("table cible introuvable".into()));
        }
    } else {
        // Cross-DB : la validation de l'existence est différée au resolve
        // (nécessite pool de la DB cible). On accepte la création.
    }

    let field_id = new_id("fld");
    let mut tx = pool.begin().await?;

    sqlx::query(&format!(
        "ALTER TABLE {} ADD COLUMN {} TEXT",
        quote_ident(source_table_id),
        quote_ident(&field_id)
    ))
    .execute(&mut *tx)
    .await?;

    let forward_config = json!({
        "target_table_id": config.target_table_id,
        "target_db_id": config.target_db_id,
        "cardinality": config.cardinality,
        "allow_creating": config.allow_creating,
        "is_backlink": false
    });

    sqlx::query(
        "INSERT INTO _fields (id, table_id, name, \"type\", config, position) \
         VALUES (?, ?, ?, 'link', ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM _fields WHERE table_id = ?))",
    )
    .bind(&field_id)
    .bind(source_table_id)
    .bind(&name)
    .bind(forward_config.to_string())
    .bind(source_table_id)
    .execute(&mut *tx)
    .await?;

    if config.is_backlink {
        if config.target_db_id.is_empty() {
            let bl_id = new_id("fld");
            let bl_config = json!({
                "is_backlink": true,
                "source_link_field_id": field_id,
                "target_table_id": source_table_id
            });
            let bl_name = format!("{name} (inverse)");
            sqlx::query(
                "INSERT INTO _fields (id, table_id, name, \"type\", config, position) \
                 VALUES (?, ?, ?, 'link', ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM _fields WHERE table_id = ?))",
            )
            .bind(&bl_id)
            .bind(&config.target_table_id)
            .bind(&bl_name)
            .bind(bl_config.to_string())
            .bind(&config.target_table_id)
            .execute(&mut *tx)
            .await?;
        } else {
            // Cross-DB backlink : à créer dans la DB cible (pool différent) —
            // skip pour Phase 3 intra, sera géré via workspace manager.
        }
    }

    sqlx::query(
        "INSERT INTO _relations (id, source_table_id, source_field_id, target_db_id, target_table_id, cardinality) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(new_id("rel"))
    .bind(source_table_id)
    .bind(&field_id)
    .bind(&config.target_db_id)
    .bind(&config.target_table_id)
    .bind(&config.cardinality)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Field {
        id: field_id,
        table_id: source_table_id.to_string(),
        name,
        field_type: "link".into(),
        config: forward_config,
        position: 0,
    })
}

pub async fn link_records(
    pool: &SqlitePool,
    link_field_id: &str,
    source_record_id: &str,
    targets: Vec<LinkTarget>,
) -> Result<(), AppError> {
    let field = get_field(pool, link_field_id).await?;
    if field.field_type != "link" || field.is_backlink() {
        return Err(AppError::Msg("champ de lien invalide".into()));
    }
    let cfg = field
        .link_config()
        .ok_or_else(|| AppError::Msg("config de lien invalide".into()))?;

    let mut tx = pool.begin().await?;
    let mut links = read_link_value(&mut *tx, &field.table_id, source_record_id, &field.id).await?;
    for t in targets {
        let lv = LinkValue {
            db_id: if cfg.target_db_id.is_empty() { None } else { Some(cfg.target_db_id.clone()) },
            table_id: cfg.target_table_id.clone(),
            record_id: t.record_id.clone(),
            display: None,
        };
        if cfg.cardinality == "one" {
            links = vec![lv];
        } else if !links.iter().any(|l| l.record_id == t.record_id) {
            links.push(lv);
        }
    }
    write_link_value(&mut *tx, &field.table_id, source_record_id, &field.id, &links).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn unlink_records(
    pool: &SqlitePool,
    link_field_id: &str,
    source_record_id: &str,
    target_ids: &[String],
) -> Result<(), AppError> {
    let field = get_field(pool, link_field_id).await?;
    if field.field_type != "link" || field.is_backlink() {
        return Err(AppError::Msg("champ de lien invalide".into()));
    }
    let mut tx = pool.begin().await?;
    let mut links = read_link_value(&mut *tx, &field.table_id, source_record_id, &field.id).await?;
    links.retain(|l| !target_ids.contains(&l.record_id));
    write_link_value(&mut *tx, &field.table_id, source_record_id, &field.id, &links).await?;
    tx.commit().await?;
    Ok(())
}

async fn read_link_value<'e, E>(
    exec: E,
    table_id: &str,
    record_id: &str,
    field_id: &str,
) -> Result<Vec<LinkValue>, AppError>
where
    E: Executor<'e, Database = Sqlite>,
{
    let sql = format!(
        "SELECT {} FROM {} WHERE _id = ?",
        quote_ident(field_id),
        quote_ident(table_id)
    );
    let row = sqlx::query(&sql).bind(record_id).fetch_optional(exec).await?;
    let Some(row) = row else {
        return Ok(vec![]);
    };
    let raw: Option<String> = row.try_get(field_id)?;
    match raw {
        Some(s) => Ok(serde_json::from_str(&s).unwrap_or_default()),
        None => Ok(vec![]),
    }
}

async fn write_link_value<'e, E>(
    exec: E,
    table_id: &str,
    record_id: &str,
    field_id: &str,
    links: &[LinkValue],
) -> Result<(), AppError>
where
    E: Executor<'e, Database = Sqlite>,
{
    let sql = format!(
        "UPDATE {} SET {} = ? WHERE _id = ?",
        quote_ident(table_id),
        quote_ident(field_id)
    );
    let raw = serde_json::to_string(links)?;
    sqlx::query(&sql)
        .bind(raw)
        .bind(record_id)
        .execute(exec)
        .await?;
    Ok(())
}

// ---- Data -----------------------------------------------------------------

pub async fn get_table_data(
    pool: &SqlitePool,
    table_id: &str,
    view_config: &ViewConfig,
    db_pools: &HashMap<String, SqlitePool>,
    current_db_id: &str,
) -> Result<PaginatedRecords, AppError> {
    let fields = list_fields(pool, table_id).await?;
    // On SELECTe TOUS les champs stockés, même masqués par visible_field_ids :
    // les champs calculés (lookup/rollup/count/formula) lisent les valeurs des
    // champs de lien dans les records. Masquer la colonne liée ne doit pas
    // vider les calculés. Le masquage d'affichage se fait au prune final.
    let stored_fields: Vec<Field> = fields.iter().filter(|f| f.is_stored()).cloned().collect();

    let table = quote_ident(table_id);

    let mut count_qb = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM ");
    count_qb.push(&table);
    push_filters(&mut count_qb, view_config, &fields);
    let total: i64 = count_qb.build_query_scalar().fetch_one(pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new("SELECT ");
    qb.push(quote_ident("_id"));
    for f in &stored_fields {
        qb.push(", ").push(quote_ident(&f.id));
    }
    qb.push(" FROM ").push(&table);
    push_filters(&mut qb, view_config, &fields);
    push_order(&mut qb, view_config, &fields);

    let page = view_config
        .page
        .as_ref()
        .map(|p| p.number)
        .unwrap_or(1)
        .max(1);
    let page_size = view_config
        .page
        .as_ref()
        .map(|p| p.size)
        .unwrap_or(100)
        .clamp(1, 1000);
    let offset = (page as i64 - 1) * (page_size as i64);
    qb.push(" LIMIT ")
        .push_bind(page_size as i64)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows = qb.build().fetch_all(pool).await?;

    let mut records = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut rec = serde_json::Map::new();
        let id: String = row.try_get("_id")?;
        rec.insert("_id".to_string(), json!(id));
        for f in &stored_fields {
            let v = read_cell(row, &f.id, &f.field_type);
            rec.insert(f.id.clone(), v);
        }
        records.push(Value::Object(rec));
    }

    // Cache des champs par table pour éviter N+1 requêtes _fields
    let mut fields_cache: HashMap<String, Vec<Field>> = HashMap::new();
    fields_cache.insert(table_id.to_string(), fields.clone());
    resolve_link_displays(pool, db_pools, current_db_id, &fields, &mut records, &mut fields_cache).await?;
    compute_computed_fields(pool, db_pools, current_db_id, &fields, &mut records, &mut fields_cache).await?;

    if let Some(visible) = view_config.visible_field_ids.as_ref() {
        if !visible.is_empty() {
            let visible_set: std::collections::HashSet<&String> = visible.iter().collect();
            for rec in records.iter_mut() {
                if let Some(obj) = rec.as_object_mut() {
                    obj.retain(|k, _| k == "_id" || visible_set.contains(k));
                }
            }
        }
    }

    Ok(PaginatedRecords {
        records,
        total,
        page,
        page_size,
    })
}

pub async fn upsert_records(
    pool: &SqlitePool,
    table_id: &str,
    records: Vec<Value>,
) -> Result<Vec<Value>, AppError> {
    if records.is_empty() {
        return Ok(vec![]);
    }
    let fields = list_fields(pool, table_id).await?;
    let stored_fields: Vec<Field> = fields.iter().filter(|f| f.is_stored()).cloned().collect();
    let table = quote_ident(table_id);

    // Préparer les objets et détecter si un batch multi-rows est possible
    let mut objs: Vec<(serde_json::Map<String, Value>, String)> = Vec::with_capacity(records.len());
    for record in &records {
        let obj = record
            .as_object()
            .ok_or_else(|| AppError::Msg("un enregistrement doit être un objet".into()))?
            .clone();
        let id = obj
            .get("_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| new_id("rec"));
        objs.push((obj, id));
    }

    // Validation email : uniquement champ type email (pas heuristique nom)
    {
        use regex::Regex;
        let re = Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]{2,}$").unwrap();
        for (obj, _) in &objs {
            for f in &stored_fields {
                let is_email = f.field_type == "email";
                if !is_email { continue; }
                if let Some(v) = obj.get(&f.id) {
                    if let Some(s) = v.as_str() {
                        let t = s.trim();
                        if t.is_empty() { continue; }
                        if !re.is_match(t) {
                            return Err(AppError::Msg(format!(
                                "Email invalide dans « {} » : doit contenir @ et .xx (ex: nom@domaine.fr)",
                                f.name
                            )));
                        }
                    } else if !v.is_null() {
                        return Err(AppError::Msg(format!("Email invalide dans « {} »", f.name)));
                    }
                }
            }
        }
    }
    // Validation required / unique (config {required:true, unique:true})
    {
        for f in &stored_fields {
            let required = f.config.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
            let unique = f.config.get("unique").and_then(|v| v.as_bool()).unwrap_or(false);
            if !required && !unique { continue; }
            let col = quote_ident(&f.id);
            // required : champ présent et non vide/null
            if required {
                for (obj, rid) in &objs {
                    let v = obj.get(&f.id).unwrap_or(&Value::Null);
                    let empty = match v {
                        Value::Null => true,
                        Value::String(s) => s.trim().is_empty(),
                        Value::Array(a) => a.is_empty(),
                        _ => false,
                    };
                    if empty {
                        return Err(AppError::Msg(format!("Champ requis « {} » vide (record {})", f.name, rid)));
                    }
                }
            }
            if unique {
                // doublon intra-batch
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
                for (obj, _) in &objs {
                    if let Some(v) = obj.get(&f.id) {
                        if matches!(v, Value::Null) { continue; }
                        let key = match v {
                            Value::String(s) => s.trim().to_lowercase(),
                            _ => v.to_string().trim().to_lowercase(),
                        };
                        if key.is_empty() { continue; }
                        if !seen.insert(key.clone()) {
                            return Err(AppError::Msg(format!("Valeur dupliquée dans le lot pour « {} » : {}", f.name, key)));
                        }
                    }
                }
                // doublon vs DB existante (hors batch)
                let vals: Vec<(String, String)> = objs.iter().filter_map(|(obj, rid)| {
                    obj.get(&f.id).and_then(|v| {
                        if matches!(v, Value::Null) { return None; }
                        let s = match v {
                            Value::String(st) => st.trim().to_string(),
                            _ => v.to_string(),
                        };
                        if s.is_empty() { return None; }
                        Some((rid.clone(), s))
                    })
                }).collect();
                for (rid, val) in vals {
                    // selon le type, on compare via = (TEXT) ou = (REAL) — on passe par str_val
                    let sql = format!("SELECT _id FROM {} WHERE {} = ? AND _id != ? LIMIT 1", table, col);
                    // pour number on bind en f64 si parseable, sinon texte
                    let exists: Option<String> = if f.field_type == "number" {
                        if let Ok(n) = val.parse::<f64>() {
                            sqlx::query_scalar::<_, String>(&sql).bind(n).bind(&rid).fetch_optional(pool).await.map_err(|e| AppError::Msg(e.to_string()))?
                        } else {
                            sqlx::query_scalar::<_, String>(&sql).bind(&val).bind(&rid).fetch_optional(pool).await.map_err(|e| AppError::Msg(e.to_string()))?
                        }
                    } else {
                        sqlx::query_scalar::<_, String>(&sql).bind(&val).bind(&rid).fetch_optional(pool).await.map_err(|e| AppError::Msg(e.to_string()))?
                    };
                    if exists.is_some() {
                        return Err(AppError::Msg(format!("Valeur déjà existante pour « {} » : {}", f.name, val)));
                    }
                }
            }
        }
    }
    // Signature des champs présents (triée) par enregistrement
    let sigs: Vec<Vec<String>> = objs
        .iter()
        .map(|(obj, _)| {
            let mut v: Vec<String> = stored_fields
                .iter()
                .filter(|f| obj.contains_key(&f.id))
                .map(|f| f.id.clone())
                .collect();
            v.sort();
            v
        })
        .collect();
    let uniform = sigs.windows(2).all(|w| w[0] == w[1]);

    let mut tx = pool.begin().await?;

    if uniform && objs.len() > 1 {
        // Batch multi-rows
        let present_ids = &sigs[0];
        let present_fields: Vec<&Field> = stored_fields
            .iter()
            .filter(|f| present_ids.contains(&f.id))
            .collect();

        let mut qb = QueryBuilder::<Sqlite>::new("INSERT INTO ");
        qb.push(&table)
            .push(" (")
            .push(quote_ident("_id"));
        for f in &stored_fields {
            qb.push(", ").push(quote_ident(&f.id));
        }
        qb.push(") VALUES ");
        let mut first_row = true;
        for (obj, id) in &objs {
            if !first_row {
                qb.push(", ");
            }
            first_row = false;
            qb.push("(");
            qb.push_bind(id);
            for f in &stored_fields {
                qb.push(", ");
                let v = obj.get(&f.id).unwrap_or(&Value::Null);
                push_value(&mut qb, v, &f.field_type);
            }
            qb.push(")");
        }
        if !present_fields.is_empty() {
            qb.push(" ON CONFLICT(")
                .push(quote_ident("_id"))
                .push(") DO UPDATE SET ");
            for (i, f) in present_fields.iter().enumerate() {
                if i > 0 {
                    qb.push(", ");
                }
                qb.push(quote_ident(&f.id))
                    .push(" = excluded.")
                    .push(quote_ident(&f.id));
            }
        } else {
            qb.push(" ON CONFLICT(")
                .push(quote_ident("_id"))
                .push(") DO NOTHING");
        }
        qb.build().execute(&mut *tx).await?;
    } else {
        for (obj, id) in &objs {
            let mut qb = QueryBuilder::<Sqlite>::new("INSERT INTO ");
            qb.push(&table)
                .push(" (")
                .push(quote_ident("_id"));
            for f in &stored_fields {
                qb.push(", ").push(quote_ident(&f.id));
            }
            qb.push(") VALUES (");
            qb.push_bind(id);
            for f in &stored_fields {
                qb.push(", ");
                let v = obj.get(&f.id).unwrap_or(&Value::Null);
                push_value(&mut qb, v, &f.field_type);
            }
            qb.push(")");
            let present: Vec<&Field> = stored_fields.iter().filter(|f| obj.contains_key(&f.id)).collect();
            if !present.is_empty() {
                qb.push(" ON CONFLICT(")
                    .push(quote_ident("_id"))
                    .push(") DO UPDATE SET ");
                for (i, f) in present.iter().enumerate() {
                    if i > 0 {
                        qb.push(", ");
                    }
                    qb.push(quote_ident(&f.id))
                        .push(" = excluded.")
                        .push(quote_ident(&f.id));
                }
            } else {
                qb.push(" ON CONFLICT(")
                    .push(quote_ident("_id"))
                    .push(") DO NOTHING");
            }
            qb.build().execute(&mut *tx).await?;
        }
    }

    let mut result = Vec::with_capacity(objs.len());
    for (obj, id) in objs {
        let mut out = serde_json::Map::new();
        out.insert("_id".to_string(), json!(id));
        for f in &stored_fields {
            out.insert(f.id.clone(), obj.get(&f.id).cloned().unwrap_or(Value::Null));
        }
        result.push(Value::Object(out));
    }

    tx.commit().await?;
    Ok(result)
}

pub async fn delete_records(
    pool: &SqlitePool,
    table_id: &str,
    ids: &[String],
) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    let table = quote_ident(table_id);
    let mut tx = pool.begin().await?;

    let mut qb = QueryBuilder::<Sqlite>::new("DELETE FROM ");
    qb.push(&table)
        .push(" WHERE ")
        .push(quote_ident("_id"))
        .push(" IN (");
    let mut separated = qb.separated(", ");
    for id in ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    qb.build().execute(&mut *tx).await?;

    cleanup_dangling_links(&mut tx, table_id, ids).await?;

    tx.commit().await?;
    Ok(())
}

// Supprime les références vers les enregistrements supprimés dans toutes les
// colonnes de lien JSON (d'autres tables pointant vers `table_id`, y compris
// la table elle-même pour les auto-liens) afin d'éviter les orphelins.
// Travail délégué à SQLite (json_each/json_group_array) : pas de scan ni de
// désérialisation côté Rust, et un seul UPDATE par table source, limité aux
// lignes contenant réellement un _id supprimé (clause EXISTS).
async fn cleanup_dangling_links(
    tx: &mut SqliteTransaction<'_>,
    table_id: &str,
    deleted_ids: &[String],
) -> Result<(), AppError> {
    let relations: Vec<(String, String)> = sqlx::query_as(
        "SELECT source_table_id, source_field_id FROM _relations \
         WHERE target_db_id = '' AND target_table_id = ?",
    )
    .bind(table_id)
    .fetch_all(&mut **tx)
    .await?;

    for (source_table_id, source_field_id) in relations {
        let src = quote_ident(&source_table_id);
        let col = quote_ident(&source_field_id);
        let mut qb = QueryBuilder::<Sqlite>::new("UPDATE ");
        qb.push(&src)
            .push(" SET ")
            .push(&col)
            .push(" = (SELECT json_group_array(value) FROM json_each(")
            .push(&src)
            .push(".")
            .push(&col)
            .push(") WHERE json_extract(value, '$.record_id') NOT IN (");
        let mut keep = qb.separated(", ");
        for id in deleted_ids {
            keep.push_bind(id);
        }
        keep.push_unseparated(")) WHERE ");
        qb.push(&col)
            .push(" IS NOT NULL AND EXISTS (SELECT 1 FROM json_each(")
            .push(&src)
            .push(".")
            .push(&col)
            .push(") WHERE json_extract(value, '$.record_id') IN (");
        let mut removed = qb.separated(", ");
        for id in deleted_ids {
            removed.push_bind(id);
        }
        removed.push_unseparated("))");
        qb.build().execute(&mut **tx).await?;
    }
    Ok(())
}

// ---- Views ----------------------------------------------------------------

pub async fn list_views(pool: &SqlitePool, table_id: &str) -> Result<Vec<View>, AppError> {
    let rows = sqlx::query(
        "SELECT id, table_id, name, \"type\" AS view_type, config, is_default FROM _views \
         WHERE table_id = ? ORDER BY is_default DESC, name ASC",
    )
    .bind(table_id)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let config_str: String = row.try_get("config").unwrap_or_else(|_| "{}".to_string());
        let config: ViewConfig = serde_json::from_str(&config_str).unwrap_or_default();
        out.push(View {
            id: row.try_get("id")?,
            table_id: row.try_get("table_id")?,
            name: row.try_get("name")?,
            view_type: row.try_get("view_type")?,
            config,
            is_default: row.try_get::<i64, _>("is_default")? != 0,
        });
    }
    Ok(out)
}

pub async fn create_view(pool: &SqlitePool, input: ViewInput) -> Result<View, AppError> {
    let id = new_id("view");
    sqlx::query("INSERT INTO _views (id, table_id, name, \"type\", config, is_default) VALUES (?, ?, ?, ?, ?, 0)")
        .bind(&id)
        .bind(&input.table_id)
        .bind(&input.name)
        .bind(&input.view_type)
        .bind(serde_json::to_string(&input.config)?)
        .execute(pool)
        .await?;

    Ok(View {
        id,
        table_id: input.table_id,
        name: input.name,
        view_type: input.view_type,
        config: input.config,
        is_default: false,
    })
}

pub async fn update_view(
    pool: &SqlitePool,
    view_id: &str,
    config: &ViewConfig,
) -> Result<(), AppError> {
    sqlx::query("UPDATE _views SET config = ? WHERE id = ?")
        .bind(serde_json::to_string(config)?)
        .bind(view_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_view(pool: &SqlitePool, view_id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM _views WHERE id = ?")
        .bind(view_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ---- Query building helpers ------------------------------------------------

fn push_filters<'q>(qb: &mut QueryBuilder<'q, Sqlite>, view_config: &ViewConfig, fields: &[Field]) {
    let conj = if view_config.filter_conjunction.to_lowercase() == "or" { " OR " } else { " AND " };
    let mut first = true;
    for filter in &view_config.filters {
        let Some(field) = fields.iter().find(|f| f.id == filter.field_id) else {
            continue;
        };
        if !field.is_stored() {
            continue;
        }
        if first {
            qb.push(" WHERE ");
            first = false;
        } else {
            qb.push(conj);
        }
        push_filter(qb, filter, field);
    }
}

fn push_filter<'q>(qb: &mut QueryBuilder<'q, Sqlite>, filter: &crate::db::models::Filter, field: &Field) {
    let col = quote_ident(&field.id);
    match filter.operator.as_str() {
        "is_empty" => match field.field_type.as_str() {
            // Checkbox : non coché (0) ou jamais renseigné (NULL) = "vide".
            // Parenthèses : AND lie plus fort que OR dans un filtre multiple.
            "checkbox" => {
                qb.push("(").push(&col).push(" IS NULL OR ").push(&col).push(" = 0)");
            }
            // Nombre : 0 est une valeur valide, seul NULL est "vide".
            "number" => {
                qb.push(&col).push(" IS NULL");
            }
            _ => {
                qb.push("(").push(&col).push(" IS NULL OR ").push(&col).push(" = '')");
            }
        },
        "is_not_empty" => match field.field_type.as_str() {
            // Checkbox : coché (1).
            "checkbox" => {
                qb.push(&col).push(" = 1");
            }
            // Nombre : tout sauf NULL (0 inclus).
            "number" => {
                qb.push(&col).push(" IS NOT NULL");
            }
            _ => {
                qb.push("(").push(&col).push(" IS NOT NULL AND ").push(&col).push(" != '')");
            }
        },
        "contains" => {
            qb.push(&col)
                .push(" LIKE ")
                .push_bind(format!("%{}%", escape_like(&str_val(&filter.value))))
                .push(" ESCAPE '\\'");
        }
        "does_not_contain" => {
            qb.push(&col)
                .push(" NOT LIKE ")
                .push_bind(format!("%{}%", escape_like(&str_val(&filter.value))))
                .push(" ESCAPE '\\'");
        }
        "is" => {
            qb.push(&col).push(" = ");
            push_value(qb, &filter.value, &field.field_type);
        }
        "is_not" => {
            qb.push(&col).push(" != ");
            push_value(qb, &filter.value, &field.field_type);
        }
        "gt" => {
            qb.push(&col).push(" > ");
            push_value(qb, &filter.value, &field.field_type);
        }
        "gte" => {
            qb.push(&col).push(" >= ");
            push_value(qb, &filter.value, &field.field_type);
        }
        "lt" => {
            qb.push(&col).push(" < ");
            push_value(qb, &filter.value, &field.field_type);
        }
        "lte" => {
            qb.push(&col).push(" <= ");
            push_value(qb, &filter.value, &field.field_type);
        }
        _ => {
            qb.push(&col).push(" IS NULL");
        }
    }
}

fn push_order<'q>(qb: &mut QueryBuilder<'q, Sqlite>, view_config: &ViewConfig, fields: &[Field]) {
    let mut order: Vec<(&str, &str)> = Vec::new();
    let is_stored = |fid: &str| fields.iter().any(|f| f.id == fid && f.is_stored());
    for g in &view_config.groups {
        if is_stored(&g.field_id) {
            order.push((g.field_id.as_str(), g.order.as_str()));
        }
    }
    for s in &view_config.sorts {
        if is_stored(&s.field_id) {
            order.push((s.field_id.as_str(), s.direction.as_str()));
        }
    }
    // Tie-breaker _id : sans ORDER BY, SQLite ne garantit pas l'ordre des lignes
    // → pagination instable (doublons/omissions entre pages). Toujours ajouter.
    order.push(("_id", "asc"));
    qb.push(" ORDER BY ");
    for (i, (fid, dir)) in order.iter().enumerate() {
        if i > 0 {
            qb.push(", ");
        }
        qb.push(quote_ident(fid))
            .push(" ")
            .push(if *dir == "desc" { "DESC" } else { "ASC" });
    }
}

fn str_val(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => v.to_string(),
    }
}

// Échappe les jokers SQL LIKE (% _ \) pour que la recherche soit littérale.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn push_value<'q>(qb: &mut QueryBuilder<'q, Sqlite>, v: &Value, field_type: &str) {
    match field_type {
        "number" => {
            // Valeur non numérique : NULL (aucune correspondance) plutôt qu'une
            // coercition silencieuse en 0.0 qui fausserait les résultats.
            let n: Option<f64> = match v {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => s.trim().parse::<f64>().ok(),
                Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                _ => None,
            };
            match n {
                Some(x) => qb.push_bind(x),
                None => qb.push_bind(None::<f64>),
            };
        }
        "checkbox" => {
            let b = match v {
                Value::Bool(b) => *b,
                Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
                Value::String(s) => s == "true" || s == "1",
                _ => false,
            };
            qb.push_bind(b as i64);
        }
        // select : valeur = id d'option (chaîne). Stockée en JSON pour être
        // relue par read_cell (sinon round-trip cassé).
        "select" => match v {
            Value::Null => {
                qb.push_bind(None::<String>);
            }
            _ => {
                qb.push_bind(serde_json::to_string(v).unwrap_or_else(|_| str_val(v)));
            }
        },
        _ => {
            qb.push_bind(str_val(v));
        }
    }
}

fn read_cell(row: &SqliteRow, col: &str, field_type: &str) -> Value {
    match field_type {
        "number" => row
            .try_get::<Option<f64>, _>(col)
            .ok()
            .flatten()
            .map(|v| json!(v))
            .unwrap_or(Value::Null),
        "checkbox" => row
            .try_get::<Option<i64>, _>(col)
            .ok()
            .flatten()
            .map(|v| json!(v != 0))
            .unwrap_or(Value::Null),
        "created_time" | "last_modified_time" => row
            .try_get::<Option<i64>, _>(col)
            .ok()
            .flatten()
            .map(|v| json!(v))
            .unwrap_or(Value::Null),
        "select" => row
            .try_get::<Option<String>, _>(col)
            .ok()
            .flatten()
            .map(|s| serde_json::from_str::<Value>(&s).unwrap_or_else(|_| json!(s)))
            .unwrap_or(Value::Null),
        "link" | "attachment" => row
            .try_get::<Option<String>, _>(col)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or(Value::Null),
        _ => row
            .try_get::<Option<String>, _>(col)
            .ok()
            .flatten()
            .map(|v| json!(v))
            .unwrap_or(Value::Null),
    }
}

// ---- Cache champs par table (P1/P4) ---------------------------------------

// Résout le pool cible d'un lien : '' = base courante, sinon pool de la base
// cible (Phase 3 cross-DB). None si la base cible n'est pas ouverte/connue →
// l'appelant saute la résolution plutôt que d'échouer la grille entière.
fn pool_for<'a>(
    current: &'a SqlitePool,
    db_pools: &'a HashMap<String, SqlitePool>,
    db_id: &str,
) -> Option<&'a SqlitePool> {
    if db_id.is_empty() {
        Some(current)
    } else {
        db_pools.get(db_id)
    }
}

// db_id cible d'un champ de lien : config d'abord, sinon db_id porté par les
// LinkValue (écrit par link_records).
fn link_target_db_id(field: &Field, records: &[Value]) -> String {
    let cfg_db = field.link_config().map(|c| c.target_db_id).unwrap_or_default();
    if !cfg_db.is_empty() {
        return cfg_db;
    }
    records
        .iter()
        .find_map(|r| {
            r.get(&field.id)
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|lv| lv.get("db_id").and_then(|x| x.as_str()))
                .map(|s| s.to_string())
        })
        .unwrap_or_default()
}

async fn cached_fields(
    cache: &mut HashMap<String, Vec<Field>>,
    pool: &SqlitePool,
    table_id: &str,
) -> Result<Vec<Field>, AppError> {
    if let Some(v) = cache.get(table_id) {
        return Ok(v.clone());
    }
    let v = list_fields(pool, table_id).await?;
    cache.insert(table_id.to_string(), v.clone());
    Ok(v)
}

// ---- Résolution des champs calculés ----------------------------------------

async fn resolve_link_displays(
    pool: &SqlitePool,
    db_pools: &HashMap<String, SqlitePool>,
    current_db_id: &str,
    fields: &[Field],
    records: &mut [Value],
    cache: &mut HashMap<String, Vec<Field>>,
) -> Result<(), AppError> {
    let _ = current_db_id;
    for field in fields {
        if field.field_type != "link" || field.is_backlink() {
            continue;
        }
        let target_db_id = link_target_db_id(field, records);
        // Base cible non ouverte/connue → affichage sans display, pas d'erreur.
        let Some(target_pool) = pool_for(pool, db_pools, &target_db_id) else {
            continue;
        };
        let mut targets: HashMap<String, HashSet<String>> = HashMap::new();
        for rec in records.iter() {
            if let Some(arr) = rec.get(&field.id).and_then(|v| v.as_array()) {
                for lv in arr {
                    if let (Some(t), Some(rid)) = (
                        lv.get("table_id").and_then(|x| x.as_str()),
                        lv.get("record_id").and_then(|x| x.as_str()),
                    ) {
                        targets.entry(t.to_string()).or_default().insert(rid.to_string());
                    }
                }
            }
        }
        let mut display_map: HashMap<(String, String), Value> = HashMap::new();
        for (t, ids) in targets {
            let displays = fetch_primary_displays(target_pool, &t, &ids, cache).await?;
            for (rid, disp) in displays {
                display_map.insert((t.clone(), rid), disp);
            }
        }
        for rec in records.iter_mut() {
            if let Some(arr) = rec.get_mut(&field.id).and_then(|v| v.as_array_mut()) {
                for lv in arr.iter_mut() {
                    let t = lv.get("table_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let rid = lv.get("record_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    if let Some(d) = display_map.get(&(t, rid)) {
                        if let Some(o) = lv.as_object_mut() {
                            o.insert("display".to_string(), d.clone());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

async fn compute_computed_fields(
    pool: &SqlitePool,
    db_pools: &HashMap<String, SqlitePool>,
    current_db_id: &str,
    fields: &[Field],
    records: &mut [Value],
    cache: &mut HashMap<String, Vec<Field>>,
) -> Result<(), AppError> {
    let _ = current_db_id;
    for field in fields {
        if field.is_stored() {
            continue;
        }
        match field.field_type.as_str() {
            "link" => {
                if field.is_backlink() {
                    compute_backlink_field(pool, field, records, cache).await?;
                }
            }
            "count" => compute_count_field(field, records),
            "lookup" => compute_lookup_field(pool, db_pools, field, records, cache).await?,
            "rollup" => compute_rollup_field(pool, db_pools, field, records, cache).await?,
            "formula" => compute_formula_field(field, fields, records),
            _ => {}
        }
    }
    Ok(())
}

fn compute_count_field(field: &Field, records: &mut [Value]) {
    let cfg: CountFieldConfig = serde_json::from_value(field.config.clone())
        .unwrap_or(CountFieldConfig { source_link_field_id: String::new() });
    for rec in records.iter_mut() {
        let n = rec
            .get(&cfg.source_link_field_id)
            .and_then(|v| v.as_array())
            .map(|a| a.len() as i64)
            .unwrap_or(0);
        rec_insert(rec, &field.id, json!(n));
    }
}

async fn compute_lookup_field(
    pool: &SqlitePool,
    db_pools: &HashMap<String, SqlitePool>,
    field: &Field,
    records: &mut [Value],
    cache: &mut HashMap<String, Vec<Field>>,
) -> Result<(), AppError> {
    let cfg: LookupFieldConfig = serde_json::from_value(field.config.clone())
        .map_err(|e| AppError::Msg(format!("config lookup invalide: {e}")))?;
    let (target_table, value_map) =
        collect_target_values(pool, db_pools, &cfg.source_link_field_id, &cfg.target_field_id, records, cache).await?;

    for rec in records.iter_mut() {
        let vals: Vec<Value> = linked_target_values(rec, &cfg.source_link_field_id, &value_map);
        let v = if vals.is_empty() { Value::Null } else { Value::Array(vals) };
        rec_insert(rec, &field.id, v);
    }
    let _ = target_table;
    Ok(())
}

async fn compute_rollup_field(
    pool: &SqlitePool,
    db_pools: &HashMap<String, SqlitePool>,
    field: &Field,
    records: &mut [Value],
    cache: &mut HashMap<String, Vec<Field>>,
) -> Result<(), AppError> {
    let cfg: RollupFieldConfig = serde_json::from_value(field.config.clone())
        .map_err(|e| AppError::Msg(format!("config rollup invalide: {e}")))?;
    let (_, value_map) =
        collect_target_values(pool, db_pools, &cfg.source_link_field_id, &cfg.target_field_id, records, cache).await?;

    for rec in records.iter_mut() {
        let vals = linked_target_values(rec, &cfg.source_link_field_id, &value_map);
        let v = rollup_apply(&cfg.function, &vals);
        rec_insert(rec, &field.id, v);
    }
    Ok(())
}

async fn collect_target_values(
    pool: &SqlitePool,
    db_pools: &HashMap<String, SqlitePool>,
    source_link_field_id: &str,
    target_field_id: &str,
    records: &[Value],
    cache: &mut HashMap<String, Vec<Field>>,
) -> Result<(Option<String>, HashMap<String, Value>), AppError> {
    let mut targets: HashSet<String> = HashSet::new();
    let mut target_table: Option<String> = None;
    let mut target_db_id = String::new();
    for rec in records.iter() {
        if let Some(arr) = rec.get(source_link_field_id).and_then(|v| v.as_array()) {
            for lv in arr {
                if let Some(t) = lv.get("table_id").and_then(|x| x.as_str()) {
                    target_table = Some(t.to_string());
                }
                if let Some(db) = lv.get("db_id").and_then(|x| x.as_str()) {
                    target_db_id = db.to_string();
                }
                if let Some(rid) = lv.get("record_id").and_then(|x| x.as_str()) {
                    targets.insert(rid.to_string());
                }
            }
        }
    }
    // Base cible non ouverte → lookup/rollup vide (pas d'erreur de grille).
    let value_map = match &target_table {
        Some(tt) => match pool_for(pool, db_pools, &target_db_id) {
            Some(tp) => fetch_target_values(tp, tt, target_field_id, &targets, cache).await?,
            None => HashMap::new(),
        },
        None => HashMap::new(),
    };
    Ok((target_table, value_map))
}

fn linked_target_values(
    rec: &Value,
    source_link_field_id: &str,
    value_map: &HashMap<String, Value>,
) -> Vec<Value> {
    rec.get(source_link_field_id)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|lv| lv.get("record_id").and_then(|x| x.as_str()))
                .filter_map(|rid| value_map.get(rid).cloned())
                .collect()
        })
        .unwrap_or_default()
}

fn rollup_apply(function: &str, values: &[Value]) -> Value {
    match function {
        "count" => json!(values.len() as i64),
        "sum" => json!(values.iter().map(|v| v.as_f64().unwrap_or(0.0)).sum::<f64>()),
        "avg" => {
            if values.is_empty() {
                Value::Null
            } else {
                json!(values.iter().map(|v| v.as_f64().unwrap_or(0.0)).sum::<f64>() / values.len() as f64)
            }
        }
        "min" => values
            .iter()
            .filter_map(|v| v.as_f64())
            .fold(None, |acc: Option<f64>, x| Some(acc.map_or(x, |a| a.min(x))))
            .map(|x| json!(x))
            .unwrap_or(Value::Null),
        "max" => values
            .iter()
            .filter_map(|v| v.as_f64())
            .fold(None, |acc: Option<f64>, x| Some(acc.map_or(x, |a| a.max(x))))
            .map(|x| json!(x))
            .unwrap_or(Value::Null),
        "arrayjoin" => json!(values
            .iter()
            .map(display_string)
            .collect::<Vec<_>>()
            .join(", ")),
        _ => Value::Null,
    }
}

fn display_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => v.to_string(),
    }
}

fn compute_formula_field(field: &Field, fields: &[Field], records: &mut [Value]) {
    let cfg = field.formula_config().unwrap_or_default();
    let parsed = formula::parse(&cfg.expression).ok();
    let name_map: HashMap<String, String> = fields.iter().map(|f| (f.name.clone(), f.id.clone())).collect();

    for rec in records.iter_mut() {
        let value = match &parsed {
            Some(expr) => {
                let mut ctx: formula::Context = rec
                    .as_object()
                    .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();
                for (name, id) in &name_map {
                    if let Some(v) = rec.get(id) {
                        ctx.insert(name.clone(), v.clone());
                    }
                }
                formula::eval(expr, &ctx)
            }
            None => json!(format!("#ERREUR: {}", cfg.expression)),
        };
        rec_insert(rec, &field.id, value);
    }
}

async fn compute_backlink_field(
    pool: &SqlitePool,
    field: &Field,
    records: &mut [Value],
    cache: &mut HashMap<String, Vec<Field>>,
) -> Result<(), AppError> {
    let cfg: LinkFieldConfig = field.link_config().unwrap_or_default();
    let forward_id = cfg.source_link_field_id;
    if forward_id.is_empty() {
        return Ok(());
    }
    let forward = get_field(pool, &forward_id).await?;
    let source_table = forward.table_id.clone();

    // P2 : ne charger que les sources qui lient effectivement les records visibles,
    // au lieu de scanner toute la table source.
    let target_ids: HashSet<String> = records
        .iter()
        .filter_map(|r| r.get("_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();
    if target_ids.is_empty() {
        for rec in records.iter_mut() {
            rec_insert(rec, &field.id, Value::Null);
        }
        return Ok(());
    }
    // WHERE instr(col, '"record_id":"<id>"') > 0 pour chaque cible
    let col = quote_ident(&forward_id);
    let tbl = quote_ident(&source_table);
    let mut qb = QueryBuilder::<Sqlite>::new(format!("SELECT _id, {col} FROM {tbl} WHERE "));
    let mut first = true;
    for tid in &target_ids {
        if !first {
            qb.push(" OR ");
        }
        first = false;
        let pat = format!("\"record_id\":\"{tid}\"");
        qb.push(format!("instr({col}, "));
        qb.push_bind(pat);
        qb.push(") > 0");
    }
    let rows = qb.build().fetch_all(pool).await?;

    let mut reverse: HashMap<String, Vec<LinkValue>> = HashMap::new();
    let mut source_ids: HashSet<String> = HashSet::new();
    for row in rows {
        let sid: String = row.try_get("_id")?;
        let raw: Option<String> = row.try_get(forward_id.as_str())?;
        if let Some(raw) = raw {
            if let Ok(arr) = serde_json::from_str::<Vec<LinkValue>>(&raw) {
                for lv in arr {
                    if !target_ids.contains(&lv.record_id) {
                        continue;
                    }
                    reverse.entry(lv.record_id).or_default().push(LinkValue {
                        db_id: None,
                        table_id: source_table.clone(),
                        record_id: sid.clone(),
                        display: None,
                    });
                    source_ids.insert(sid.clone());
                }
            }
        }
    }
    let displays = fetch_primary_displays(pool, &source_table, &source_ids, cache).await?;

    for rec in records.iter_mut() {
        let rid = rec.get("_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let mut links = reverse.get(&rid).cloned().unwrap_or_default();
        for lv in links.iter_mut() {
            if let Some(d) = displays.get(&lv.record_id) {
                lv.display = Some(d.clone());
            }
        }
        let v = if links.is_empty() {
            Value::Null
        } else {
            serde_json::to_value(&links)?
        };
        rec_insert(rec, &field.id, v);
    }
    Ok(())
}

fn rec_insert(rec: &mut Value, key: &str, value: Value) {
    if let Some(map) = rec.as_object_mut() {
        map.insert(key.to_string(), value);
    }
}

// ---- Helpers de lecture cross-record ---------------------------------------

async fn fetch_primary_displays(
    pool: &SqlitePool,
    table_id: &str,
    ids: &HashSet<String>,
    cache: &mut HashMap<String, Vec<Field>>,
) -> Result<HashMap<String, Value>, AppError> {
    let mut map = HashMap::new();
    if ids.is_empty() {
        return Ok(map);
    }
    let fields = cached_fields(cache, pool, table_id).await?;
    let primary = fields.iter().min_by_key(|f| f.position).cloned();
    let (col, ftype) = match &primary {
        Some(f) => (f.id.clone(), f.field_type.clone()),
        None => ("_id".to_string(), "text".to_string()),
    };

    let mut qb = QueryBuilder::<Sqlite>::new("SELECT ");
    qb.push(quote_ident("_id"))
        .push(", ")
        .push(quote_ident(&col))
        .push(" FROM ")
        .push(quote_ident(table_id))
        .push(" WHERE ")
        .push(quote_ident("_id"))
        .push(" IN (");
    let mut sep = qb.separated(", ");
    for id in ids {
        sep.push_bind(id);
    }
    sep.push_unseparated(")");
    let rows = qb.build().fetch_all(pool).await?;
    for row in rows {
        let id: String = row.try_get("_id")?;
        let v = read_cell(&row, &col, &ftype);
        map.insert(id, v);
    }
    Ok(map)
}

async fn fetch_target_values(
    pool: &SqlitePool,
    table_id: &str,
    field_id: &str,
    ids: &HashSet<String>,
    cache: &mut HashMap<String, Vec<Field>>,
) -> Result<HashMap<String, Value>, AppError> {
    let mut map = HashMap::new();
    if ids.is_empty() {
        return Ok(map);
    }
    let fields = cached_fields(cache, pool, table_id).await?;
    // Champ cible non stocké (lookup/rollup/count/formula/backlink, ou colonne
    // supprimée) → pas de colonne SQL : on renvoie vide plutôt qu'une erreur
    // SQL qui ferait échouer toute la grille.
    let target_field = fields.iter().find(|f| f.id == field_id);
    let Some(target_field) = target_field else { return Ok(map); };
    if !target_field.is_stored() {
        return Ok(map);
    }
    let ftype = target_field.field_type.clone();

    let mut qb = QueryBuilder::<Sqlite>::new("SELECT ");
    qb.push(quote_ident("_id"))
        .push(", ")
        .push(quote_ident(field_id))
        .push(" FROM ")
        .push(quote_ident(table_id))
        .push(" WHERE ")
        .push(quote_ident("_id"))
        .push(" IN (");
    let mut sep = qb.separated(", ");
    for id in ids {
        sep.push_bind(id);
    }
    sep.push_unseparated(")");
    let rows = qb.build().fetch_all(pool).await?;
    for row in rows {
        let id: String = row.try_get("_id")?;
        let v = read_cell(&row, field_id, &ftype);
        map.insert(id, v);
    }
    Ok(map)
}

// ---- get_record_with_relations (Phase 3) -----------------------------------

pub async fn get_record_with_relations(
    pool: &SqlitePool,
    table_id: &str,
    record_id: &str,
    depth: u8,
    db_pools: &HashMap<String, SqlitePool>,
    current_db_id: &str,
) -> Result<RecordWithRelations, AppError> {
    let depth = depth.min(3);
    let fields = list_fields(pool, table_id).await?;
    let stored_fields: Vec<Field> = fields.iter().filter(|f| f.is_stored()).cloned().collect();

    // Fetch base record
    let table = quote_ident(table_id);
    let mut qb = QueryBuilder::<Sqlite>::new("SELECT ");
    qb.push(quote_ident("_id"));
    for f in &stored_fields {
        qb.push(", ").push(quote_ident(&f.id));
    }
    qb.push(" FROM ").push(&table).push(" WHERE ").push(quote_ident("_id")).push(" = ").push_bind(record_id);
    let row = qb
        .build()
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::Msg("enregistrement introuvable".into()))?;

    let mut rec = serde_json::Map::new();
    let id: String = row.try_get("_id")?;
    rec.insert("_id".to_string(), json!(id));
    for f in &stored_fields {
        let v = read_cell(&row, &f.id, &f.field_type);
        rec.insert(f.id.clone(), v);
    }
    let mut base = Value::Object(rec);

    // Resolve displays for base (utile pour LinkValue display)
    {
        let mut cache: HashMap<String, Vec<Field>> = HashMap::new();
        cache.insert(table_id.to_string(), fields.clone());
        let mut tmp = vec![base.clone()];
        resolve_link_displays(pool, db_pools, current_db_id, &fields, &mut tmp, &mut cache).await?;
        base = tmp.into_iter().next().unwrap();
    }

    if depth == 0 {
        return Ok(RecordWithRelations { record: base, relations: HashMap::new() });
    }

    let relations = collect_relations(pool, db_pools, current_db_id, &fields, &base, depth).await?;

    Ok(RecordWithRelations { record: base, relations })
}

async fn collect_relations(
    pool: &SqlitePool,
    db_pools: &HashMap<String, SqlitePool>,
    current_db_id: &str,
    fields: &[Field],
    record: &Value,
    depth: u8,
) -> Result<HashMap<String, Vec<Value>>, AppError> {
    let _ = current_db_id;
    let mut out: HashMap<String, Vec<Value>> = HashMap::new();
    for field in fields.iter().filter(|f| f.field_type == "link" && !f.is_backlink()) {
        let arr = match record.get(&field.id).and_then(|v| v.as_array()) {
            Some(a) => a.clone(),
            None => continue,
        };
        if arr.is_empty() {
            out.insert(field.id.clone(), vec![]);
            continue;
        }
        let mut target_ids: HashSet<String> = HashSet::new();
        for lv in &arr {
            if let Some(rid) = lv.get("record_id").and_then(|x| x.as_str()) {
                target_ids.insert(rid.to_string());
            }
        }
        if target_ids.is_empty() {
            out.insert(field.id.clone(), vec![]);
            continue;
        }
        let cfg = field.link_config().unwrap_or_default();
        let target_table = if !cfg.target_table_id.is_empty() {
            cfg.target_table_id.clone()
        } else {
            arr.first()
                .and_then(|lv| lv.get("table_id").and_then(|x| x.as_str()))
                .unwrap_or("")
                .to_string()
        };
        if target_table.is_empty() {
            out.insert(field.id.clone(), vec![]);
            continue;
        }
        // Cross-DB : résout le pool de la table cible ; base non ouverte → vide.
        let target_db_id = if cfg.target_db_id.is_empty() {
            arr.first()
                .and_then(|lv| lv.get("db_id").and_then(|x| x.as_str()))
                .unwrap_or("")
                .to_string()
        } else {
            cfg.target_db_id.clone()
        };
        let Some(target_pool) = pool_for(pool, db_pools, &target_db_id) else {
            out.insert(field.id.clone(), vec![]);
            continue;
        };
        let targets = fetch_full_records(target_pool, &target_table, &target_ids).await?;
        // Depth>1 : récursion future via Box::pin si besoin. Pour Phase 3, depth 1 suffit
        // (on évite la récursion async mutuelle get_record_with_relations <-> collect_relations).
        let _ = depth;
        out.insert(field.id.clone(), targets);
    }
    Ok(out)
}

async fn fetch_full_records(
    pool: &SqlitePool,
    table_id: &str,
    ids: &HashSet<String>,
) -> Result<Vec<Value>, AppError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let fields = list_fields(pool, table_id).await?;
    let stored: Vec<Field> = fields.iter().filter(|f| f.is_stored()).cloned().collect();
    let mut qb = QueryBuilder::<Sqlite>::new("SELECT ");
    qb.push(quote_ident("_id"));
    for f in &stored {
        qb.push(", ").push(quote_ident(&f.id));
    }
    qb.push(" FROM ").push(quote_ident(table_id)).push(" WHERE ").push(quote_ident("_id")).push(" IN (");
    let mut sep = qb.separated(", ");
    for id in ids {
        sep.push_bind(id);
    }
    sep.push_unseparated(")");
    let rows = qb.build().fetch_all(pool).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut rec = serde_json::Map::new();
        let id: String = row.try_get("_id")?;
        rec.insert("_id".to_string(), json!(id));
        for f in &stored {
            let v = read_cell(&row, &f.id, &f.field_type);
            rec.insert(f.id.clone(), v);
        }
        out.push(Value::Object(rec));
    }
    Ok(out)
}
