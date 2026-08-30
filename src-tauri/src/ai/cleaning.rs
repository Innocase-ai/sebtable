use serde::{Deserialize, Serialize};

use crate::ai::context_builder::AICrossDBContext;
use crate::ai::provider::CompletionRequest;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransformOpType {
    Trim,
    Upper,
    Lower,
    TitleCase,
    Deduplicate,
    FillNull,
    Replace,
    RegexReplace,
    NormalizeEmail,
    NormalizePhone,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformOp {
    #[serde(rename = "type")]
    pub op_type: TransformOpType,
    pub field_id: String,
    pub field_name: String,
    #[serde(default)]
    pub params: serde_json::Value,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewRow {
    pub record_id: String,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformPlan {
    pub ops: Vec<TransformOp>,
    pub preview: Vec<PreviewRow>,
    /// Estimation du nb de lignes affectées (sample complet, extrapolée).
    pub estimated_rows: usize,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformResult {
    pub applied_rows: usize,
    pub ops: Vec<TransformOp>,
}

fn find_field(ctx: &AICrossDBContext, table_id: &str, needle: &str) -> Option<(String, String, String)> {
    let nl = needle.to_lowercase();
    for schema in ctx.all_schemas() {
        for t in &schema.tables {
            if t.id == table_id {
                for f in &t.fields {
                    if f.name.to_lowercase().contains(&nl) || f.id == needle {
                        return Some((f.id.clone(), f.name.clone(), f.field_type.clone()));
                    }
                }
            }
        }
    }
    None
}

fn all_fields(ctx: &AICrossDBContext, table_id: &str) -> Vec<(String, String, String)> {
    // Champs stockés uniquement : les calculés (lookup/rollup/count/formula/backlink)
    // n'ont pas de colonne SQL et feraient échouer apply_transform.
    for schema in ctx.all_schemas() {
        for t in &schema.tables {
            if t.id == table_id {
                return t
                    .fields
                    .iter()
                    .filter(|f| f.is_stored())
                    .map(|f| (f.id.clone(), f.name.clone(), f.field_type.clone()))
                    .collect();
            }
        }
    }
    vec![]
}

fn sample_values(ctx: &AICrossDBContext, table_id: &str, field_id: &str) -> Vec<serde_json::Value> {
    for schema in ctx.all_schemas() {
        for t in &schema.tables {
            if t.id == table_id {
                return t.sample.iter().filter_map(|r| r.get(field_id).cloned()).collect();
            }
        }
    }
    vec![]
}

fn heuristic_plan(ctx: &AICrossDBContext, table_id: &str, instruction: &str) -> Vec<TransformOp> {
    let lower = instruction.to_lowercase();
    let mut ops = Vec::new();

    let fields = all_fields(ctx, table_id);

    // trim
    if lower.contains("trim") || lower.contains("espaces") || lower.contains("espace") || lower.contains("whitespace") {
        for (fid, fname, ftype) in &fields {
            if matches!(ftype.as_str(), "text" | "long_text" | "email" | "url") {
                // check if sample has padded values
                let vals = sample_values(ctx, table_id, fid);
                let has_pad = vals.iter().any(|v| v.as_str().map(|s| s != s.trim()).unwrap_or(false));
                if has_pad || lower.contains("tous") || lower.contains("all") {
                    ops.push(TransformOp {
                        op_type: TransformOpType::Trim,
                        field_id: fid.clone(),
                        field_name: fname.clone(),
                        params: serde_json::json!({}),
                        description: format!("Supprimer espaces superflus dans '{}'", fname),
                    });
                }
            }
        }
        if ops.is_empty() && !fields.is_empty() {
            // fallback trim first text field
            for (fid, fname, ftype) in &fields {
                if ftype == "text" {
                    ops.push(TransformOp {
                        op_type: TransformOpType::Trim,
                        field_id: fid.clone(),
                        field_name: fname.clone(),
                        params: serde_json::json!({}),
                        description: format!("Trim '{}'", fname),
                    });
                    break;
                }
            }
        }
    }
    if lower.contains("majuscule") || lower.contains("uppercase") || lower.contains("upper") {
        for (fid, fname, ftype) in &fields {
            if (ftype == "text" || ftype == "long_text")
                && (lower.contains(&fname.to_lowercase()) || lower.contains("tous") || ops.is_empty())
            {
                ops.push(TransformOp {
                    op_type: TransformOpType::Upper,
                    field_id: fid.clone(),
                    field_name: fname.clone(),
                    params: serde_json::json!({}),
                    description: format!("Passer '{}' en majuscules", fname),
                });
                if !lower.contains("tous") && !lower.contains("all") {
                    break;
                }
            }
        }
    }
    if lower.contains("minuscule") || lower.contains("lowercase") || lower.contains("lower") {
        for (fid, fname, ftype) in &fields {
            if (ftype == "text" || ftype == "email")
                && (lower.contains(&fname.to_lowercase()) || ops.is_empty())
            {
                ops.push(TransformOp {
                    op_type: TransformOpType::Lower,
                    field_id: fid.clone(),
                    field_name: fname.clone(),
                    params: serde_json::json!({}),
                    description: format!("Passer '{}' en minuscules", fname),
                });
                break;
            }
        }
    }
    if lower.contains("email") && (lower.contains("normal") || lower.contains("minuscule") || lower.contains("trim")) {
        for (fid, fname, ftype) in &fields {
            if ftype == "email" || fname.to_lowercase().contains("email") {
                ops.push(TransformOp {
                    op_type: TransformOpType::NormalizeEmail,
                    field_id: fid.clone(),
                    field_name: fname.clone(),
                    params: serde_json::json!({}),
                    description: format!("Normaliser emails dans '{}'", fname),
                });
            }
        }
    }
    if lower.contains("doublon") || lower.contains("duplicate") || lower.contains("dédupli") || lower.contains("dedupl") {
        // dedup needs at least one field to define uniqueness - use first text field
        if let Some((fid, fname, _)) = fields.iter().find(|(_, _, t)| t == "text" || t == "email").cloned() {
            ops.push(TransformOp {
                op_type: TransformOpType::Deduplicate,
                field_id: fid.clone(),
                field_name: fname.clone(),
                params: serde_json::json!({}),
                description: format!("Supprimer doublons sur '{}'", fname),
            });
        }
    }
    if lower.contains("vide") || lower.contains("null") || lower.contains("remplir") || lower.contains("fill") {
        // fill nulls with default
        let target = find_field(ctx, table_id, "nom").or_else(|| fields.first().cloned());
        if let Some((fid, fname, _)) = target {
            ops.push(TransformOp {
                op_type: TransformOpType::FillNull,
                field_id: fid.clone(),
                field_name: fname.clone(),
                params: serde_json::json!({"value": ""}),
                description: format!("Remplir valeurs vides dans '{}'", fname),
            });
        }
    }
    if lower.contains("téléphone") || lower.contains("telephone") || lower.contains("phone") {
        for (fid, fname, ftype) in &fields {
            if ftype == "phone" || fname.to_lowercase().contains("phone") || fname.to_lowercase().contains("tél") {
                ops.push(TransformOp {
                    op_type: TransformOpType::NormalizePhone,
                    field_id: fid.clone(),
                    field_name: fname.clone(),
                    params: serde_json::json!({}),
                    description: format!("Normaliser téléphones dans '{}'", fname),
                });
            }
        }
    }
    if ops.is_empty() {
        // fallback: if instruction mentions a field name, trim it
        for (fid, fname, _) in &fields {
            if lower.contains(&fname.to_lowercase()) {
                ops.push(TransformOp {
                    op_type: TransformOpType::Trim,
                    field_id: fid.clone(),
                    field_name: fname.clone(),
                    params: serde_json::json!({}),
                    description: format!("Trim '{}' (fallback)", fname),
                });
                break;
            }
        }
    }
    // if still empty, trim first text field
    if ops.is_empty() {
        if let Some((fid, fname, _)) = fields.iter().find(|(_,_,t)| t=="text").cloned() {
            ops.push(TransformOp {
                op_type: TransformOpType::Trim,
                field_id: fid,
                field_name: fname,
                params: serde_json::json!({}),
                description: "Trim par défaut".into(),
            });
        }
    }

    ops
}

/// Regex bornée (même politique que formula/evaluator S1) : le plan vient du
/// front/LLM donc on ne compile jamais un pattern arbitraire sans limite.
fn compile_regex_bounded(pat: &str) -> Option<regex::Regex> {
    if pat.is_empty() || pat.len() > 4096 {
        return None;
    }
    regex::RegexBuilder::new(pat)
        .size_limit(1 << 20)
        .dfa_size_limit(1 << 20)
        .build()
        .ok()
}

fn apply_op_to_value(op: &TransformOp, v: &serde_json::Value) -> serde_json::Value {
    match op.op_type {
        TransformOpType::Trim => match v {
            serde_json::Value::String(s) => serde_json::Value::String(s.trim().to_string()),
            _ => v.clone(),
        },
        TransformOpType::Upper => match v {
            serde_json::Value::String(s) => serde_json::Value::String(s.to_uppercase()),
            _ => v.clone(),
        },
        TransformOpType::Lower => match v {
            serde_json::Value::String(s) => serde_json::Value::String(s.to_lowercase()),
            _ => v.clone(),
        },
        TransformOpType::TitleCase => match v {
            serde_json::Value::String(s) => {
                let t = s.split_whitespace().map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                    }
                }).collect::<Vec<_>>().join(" ");
                serde_json::Value::String(t)
            },
            _ => v.clone(),
        },
        TransformOpType::NormalizeEmail => match v {
            serde_json::Value::String(s) => serde_json::Value::String(s.trim().to_lowercase()),
            _ => v.clone(),
        },
        TransformOpType::NormalizePhone => match v {
            serde_json::Value::String(s) => {
                let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
                serde_json::Value::String(digits)
            },
            _ => v.clone(),
        },
        TransformOpType::FillNull => {
            if matches!(v, serde_json::Value::Null) || v.as_str().map(|s| s.trim().is_empty()).unwrap_or(false) {
                op.params.get("value").cloned().unwrap_or(serde_json::Value::String(String::new()))
            } else {
                v.clone()
            }
        },
        TransformOpType::Replace => {
            let from = op.params.get("from").and_then(|x| x.as_str()).unwrap_or("");
            let to = op.params.get("to").and_then(|x| x.as_str()).unwrap_or("");
            match v {
                serde_json::Value::String(s) => serde_json::Value::String(s.replace(from, to)),
                _ => v.clone(),
            }
        },
        TransformOpType::RegexReplace => {
            let pat = op.params.get("pattern").and_then(|x| x.as_str()).unwrap_or("");
            let to = op.params.get("to").and_then(|x| x.as_str()).unwrap_or("");
            match v {
                serde_json::Value::String(s) => {
                    if let Some(re) = compile_regex_bounded(pat) {
                        serde_json::Value::String(re.replace_all(s, to).to_string())
                    } else {
                        v.clone()
                    }
                },
                _ => v.clone(),
            }
        },
        TransformOpType::Deduplicate => v.clone(), // handled at table level, not per value
    }
}

fn build_preview(ctx: &AICrossDBContext, table_id: &str, ops: &[TransformOp]) -> Vec<PreviewRow> {
    let mut rows = Vec::new();
    for schema in ctx.all_schemas() {
        for t in &schema.tables {
            if t.id == table_id {
                for rec in t.sample.iter().take(5) {
                    let rid = rec.get("_id").and_then(|x| x.as_str()).unwrap_or("?").to_string();
                    for op in ops {
                        if op.op_type == TransformOpType::Deduplicate {
                            continue;
                        }
                        if let Some(before) = rec.get(&op.field_id) {
                            let after = apply_op_to_value(op, before);
                            if &after != before {
                                rows.push(PreviewRow { record_id: rid.clone(), before: before.clone(), after });
                            }
                        }
                    }
                }
            }
        }
    }
    rows.truncate(10);
    rows
}

/// Estimation du nb de lignes affectées : sur TOUT le sample (pas seulement les
/// 5 lignes d'aperçu), extrapolée si le sample est partiel. C'est un ordre de
/// grandeur affiché à l'utilisateur, pas un compte exact.
fn estimate_affected(ctx: &AICrossDBContext, table_id: &str, ops: &[TransformOp]) -> usize {
    use std::collections::HashSet;
    let mut changed: HashSet<String> = HashSet::new();
    let mut dup_extra = 0usize;
    let mut sample_len = 0usize;
    let mut row_count = 0i64;
    for schema in ctx.all_schemas() {
        for t in &schema.tables {
            if t.id != table_id {
                continue;
            }
            sample_len = t.sample.len();
            row_count = t.row_count;
            let dedup_op = ops.iter().find(|o| o.op_type == TransformOpType::Deduplicate);
            let mut seen_keys: HashSet<String> = HashSet::new();
            for rec in &t.sample {
                for op in ops {
                    if op.op_type == TransformOpType::Deduplicate {
                        continue;
                    }
                    if let Some(before) = rec.get(&op.field_id) {
                        if apply_op_to_value(op, before) != *before {
                            if let Some(id) = rec.get("_id").and_then(|x| x.as_str()) {
                                changed.insert(id.to_string());
                            }
                            break; // une op suffit pour compter la ligne
                        }
                    }
                }
                if let Some(op) = dedup_op {
                    if let Some(v) = rec.get(&op.field_id).and_then(|x| x.as_str()) {
                        let k = v.trim().to_lowercase();
                        if !k.is_empty() && !seen_keys.insert(k) {
                            dup_extra += 1;
                        }
                    }
                }
            }
        }
    }
    let raw = changed.len() + dup_extra;
    if sample_len > 0 && row_count as usize > sample_len && !changed.is_empty() {
        ((raw as f64 * row_count as f64 / sample_len as f64).round() as usize).max(raw)
    } else {
        raw
    }
}

pub fn preview_heuristic(ctx: &AICrossDBContext, table_id: &str, instruction: &str) -> TransformPlan {
    let ops = heuristic_plan(ctx, table_id, instruction);
    let preview = build_preview(ctx, table_id, &ops);
    let estimated = estimate_affected(ctx, table_id, &ops);
    TransformPlan { ops, preview, estimated_rows: estimated, provider: "heuristic".into() }
}

pub async fn preview_with_llm(
    ctx: &AICrossDBContext,
    table_id: &str,
    instruction: &str,
    provider: Option<&dyn crate::ai::provider::LLMProvider>,
) -> TransformPlan {
    let heuristic = preview_heuristic(ctx, table_id, instruction);
    if let Some(p) = provider {
        let system = format!(
            "Tu es expert nettoyage données. Réponds en JSON {{\"ops\": [{{\"type\":\"trim|upper|lower|normalize_email|deduplicate|fill_null|replace\", \"field_id\":\"...\", \"params\":{{}}}}] }}. Contexte:\n{}",
            crate::ai::context_builder::context_prompt(ctx)
        );
        let user = format!("Instruction: {}\nTable: {}. Génère plan transformation minimal.", instruction, table_id);
        let req = CompletionRequest { system, user, max_tokens: 600, temperature: 0.2, json_mode: true };
        if let Ok(txt) = p.complete(req).await {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                if let Some(arr) = v.get("ops").and_then(|x| x.as_array()) {
                    let known = all_fields(ctx, table_id);
                    let mut ops = Vec::new();
                    for item in arr {
                        let t = item.get("type").and_then(|x| x.as_str()).unwrap_or("trim");
                        let fid = item.get("field_id").and_then(|x| x.as_str()).unwrap_or("");
                        // H4 : rejeter toute op sur un champ inexistant (sinon
                        // preview vide mais échec SQL à l'application)
                        let fname = match known.iter().find(|(id, _, _)| id == fid) {
                            Some((_, n, _)) => n.clone(),
                            None => continue,
                        };
                        let op_type = match t {
                            "trim" => TransformOpType::Trim,
                            "upper" => TransformOpType::Upper,
                            "lower" => TransformOpType::Lower,
                            "normalize_email" => TransformOpType::NormalizeEmail,
                            "normalize_phone" => TransformOpType::NormalizePhone,
                            "deduplicate" => TransformOpType::Deduplicate,
                            "fill_null" => TransformOpType::FillNull,
                            "replace" => TransformOpType::Replace,
                            "regex_replace" => TransformOpType::RegexReplace,
                            "titlecase" | "title_case" => TransformOpType::TitleCase,
                            _ => {
                                // Type inconnu : on conserve le type brut si possible,
                                // sinon on ignore l'op pour éviter un Trim silencieux.
                                continue;
                            }
                        };
                        ops.push(TransformOp { op_type, field_id: fid.to_string(), field_name: fname, params: item.get("params").cloned().unwrap_or(serde_json::json!({})), description: format!("LLM: {t}"), });
                    }
                    if !ops.is_empty() {
                        let preview = build_preview(ctx, table_id, &ops);
                        let estimated = estimate_affected(ctx, table_id, &ops);
                        return TransformPlan { ops, preview, estimated_rows: estimated, provider: p.name().to_string() };
                    }
                }
            }
        }
    }
    heuristic
}

pub async fn apply_transform(
    pool: &sqlx::SqlitePool,
    table_id: &str,
    plan: &TransformPlan,
) -> Result<TransformResult, crate::error::AppError> {
    let fields = crate::db::repository::list_fields(pool, table_id).await?;
    // dedup : on supprime les doublons puis on CONTINUE avec les autres ops
    let mut dedup_deleted = 0usize;
    if let Some(op) = plan.ops.iter().find(|o| o.op_type == TransformOpType::Deduplicate) {
        let col = crate::db::repository::quote_ident_public(&op.field_id);
        let tbl = crate::db::repository::quote_ident_public(table_id);
        let sql = format!("SELECT _id, {} FROM {} ORDER BY _id", col, tbl);
        let rows = sqlx::query(&sql)
            .fetch_all(pool)
            .await
            .map_err(|e| crate::error::AppError::Msg(e.to_string()))?;
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut to_delete: Vec<String> = Vec::new();
        for row in rows {
            use sqlx::Row;
            let id: String = row.try_get("_id").unwrap_or_default();
            let key = if let Ok(Some(s)) = row.try_get::<Option<String>, _>(op.field_id.as_str()) {
                s.trim().to_lowercase()
            } else if let Ok(Some(n)) = row.try_get::<Option<f64>, _>(op.field_id.as_str()) {
                format!("{n}")
            } else if let Ok(Some(i)) = row.try_get::<Option<i64>, _>(op.field_id.as_str()) {
                format!("{i}")
            } else {
                continue;
            };
            if key.is_empty() { continue; }
            if !seen.insert(key) { to_delete.push(id); }
        }
        if !to_delete.is_empty() {
            crate::db::repository::delete_records(pool, table_id, &to_delete).await?;
            dedup_deleted = to_delete.len();
        }
        if plan.ops.iter().all(|o| o.op_type == TransformOpType::Deduplicate) {
            return Ok(TransformResult { applied_rows: dedup_deleted, ops: plan.ops.clone() });
        }
    }

    // per-row value transforms (hors dedup)
    let mut non_dedup: Vec<&TransformOp> = plan.ops.iter().filter(|o| o.op_type != TransformOpType::Deduplicate).collect();
    // Colonnes JSON-encodées (select/link/attachment) : les transformations de
    // chaîne les corrompraient (ex. upper sur `"opt_1"` → `"OPT_1"`). On les ignore.
    non_dedup.retain(|op| {
        fields.iter().find(|f| f.id == op.field_id).map(|f| !matches!(f.field_type.as_str(), "select" | "link" | "attachment")).unwrap_or(true)
    });
    if non_dedup.is_empty() {
        return Ok(TransformResult { applied_rows: dedup_deleted, ops: plan.ops.clone() });
    }

    let tbl = crate::db::repository::quote_ident_public(table_id);
    // fetch all records _id + affected cols
    let mut cols = vec![crate::db::repository::quote_ident_public("_id")];
    for op in &non_dedup {
        cols.push(crate::db::repository::quote_ident_public(&op.field_id));
    }
    cols.sort();
    cols.dedup();
    let sql = format!("SELECT {} FROM {}", cols.join(", "), tbl);
    let rows = sqlx::query(&sql).fetch_all(pool).await.map_err(|e| crate::error::AppError::Msg(e.to_string()))?;

    let mut updates: Vec<(String, String, serde_json::Value)> = Vec::new(); // (record_id, field_id, new_value)
    for row in rows {
        use sqlx::Row;
        let rid: String = row.try_get("_id").unwrap_or_default();
        for op in &non_dedup {
            // read current value as string/number
            let cur: serde_json::Value = if let Ok(Some(s)) = row.try_get::<Option<String>, _>(op.field_id.as_str()) {
                // try to keep type: try parse as json
                serde_json::Value::String(s)
            } else if let Ok(Some(n)) = row.try_get::<Option<f64>, _>(op.field_id.as_str()) {
                serde_json::json!(n)
            } else if let Ok(Some(i)) = row.try_get::<Option<i64>, _>(op.field_id.as_str()) {
                serde_json::json!(i)
            } else {
                serde_json::Value::Null
            };
            // need correct typed read: try to get raw string for text/email, f64 for number
            // above handles most; refine: for checkbox etc not in ops normally
            let new_val = apply_op_to_value(op, &cur);
            if new_val != cur {
                updates.push((rid.clone(), op.field_id.clone(), new_val));
            }
        }
    }

    // group by record_id
    let mut by_record: std::collections::HashMap<String, Vec<(String, serde_json::Value)>> = std::collections::HashMap::new();
    for (rid, fid, nv) in updates {
        by_record.entry(rid).or_default().push((fid, nv));
    }
    let mut records: Vec<serde_json::Value> = Vec::with_capacity(by_record.len());
    for (rid, changes) in by_record {
        // build Value obj for upsert
        let mut obj = serde_json::Map::new();
        obj.insert("_id".to_string(), serde_json::json!(rid));
        for (fid, nv) in changes {
            let ftype = fields.iter().find(|f| f.id==fid).map(|f| f.field_type.clone()).unwrap_or("text".into());
            let v = match ftype.as_str() {
                "number" => nv.as_str().and_then(|s| s.parse::<f64>().ok()).map(|n| serde_json::json!(n)).unwrap_or(nv),
                _ => nv,
            };
            obj.insert(fid, v);
        }
        records.push(serde_json::Value::Object(obj));
    }
    let applied = records.len();
    if applied > 0 {
        crate::db::repository::upsert_records(pool, table_id, records).await?;
    }
    Ok(TransformResult { applied_rows: applied + dedup_deleted, ops: plan.ops.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::context_builder::{AICrossDBContext, DatabaseSchema, TableSchema};
    use crate::db::models::Field;
    use serde_json::json;

    fn field(id: &str, name: &str, ftype: &str) -> Field {
        Field { id: id.into(), table_id: "tbl1".into(), name: name.into(), field_type: ftype.into(), config: json!({}), position: 0 }
    }

    #[test]
    fn heuristic_trim() {
        let ctx = AICrossDBContext {
            active_db_id: "db1".into(),
            active_schema: Some(DatabaseSchema {
                db_id: "db1".into(), db_name: "Main".into(),
                tables: vec![TableSchema {
                    id: "tbl1".into(), name: "T".into(),
                    fields: vec![field("fld1", "Nom", "text")],
                    sample: vec![json!({"_id":"r1","fld1":"  Alice  "})],
                    row_count: 1,
                }],
            }),
            reference_schemas: vec![], relations: vec![], sample_limit: 50,
        };
        let plan = preview_heuristic(&ctx, "tbl1", "supprimer les espaces");
        assert!(!plan.ops.is_empty());
        assert_eq!(plan.ops[0].op_type, TransformOpType::Trim);
    }
}
