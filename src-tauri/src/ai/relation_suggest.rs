use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::ai::context_builder::AICrossDBContext;
use crate::db::models::Field;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationSuggestion {
    pub source_db_id: String,
    pub source_table_id: String,
    pub source_table_name: String,
    pub source_field_id: String,
    pub source_field_name: String,
    pub target_db_id: String,
    pub target_table_id: String,
    pub target_table_name: String,
    pub target_field_id: String,
    pub target_field_name: String,
    pub cardinality: String,
    pub confidence: f64,
    pub reason: String,
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn name_similarity(a: &str, b: &str) -> f64 {
    let na = normalize(a);
    let nb = normalize(b);
    if na == nb {
        return 1.0;
    }
    if na.contains(&nb) || nb.contains(&na) {
        return 0.8;
    }
    // token overlap
    let ta: HashSet<String> = a
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let tb: HashSet<String> = b
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let inter = ta.intersection(&tb).count() as f64;
    let union = ta.union(&tb).count() as f64;
    inter / union
}

fn is_id_like(name: &str, ftype: &str) -> bool {
    let n = name.to_lowercase();
    (n.contains("email") && matches!(ftype, "email" | "text"))
        || (n.contains("id") || n.contains("ref") || n.contains("code"))
}

fn value_overlap(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let sa: HashSet<&String> = a.iter().collect();
    let sb: HashSet<&String> = b.iter().collect();
    let inter = sa.intersection(&sb).count() as f64;
    let denom = (a.len().min(b.len())) as f64;
    if denom == 0.0 {
        0.0
    } else {
        inter / denom
    }
}

fn collect_values(samples: &[serde_json::Value], field_id: &str) -> Vec<String> {
    samples
        .iter()
        .filter_map(|r| r.get(field_id))
        .filter_map(|v| match v {
            serde_json::Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .collect()
}

fn existing_relation(ctx: &AICrossDBContext, s_field: &str, t_db: &str, t_table: &str) -> bool {
    ctx.relations.iter().any(|r| {
        r.source_field_id == s_field && r.target_db_id == t_db && r.target_table_id == t_table
    })
}

pub fn suggest_relations(ctx: &AICrossDBContext, table_id: &str) -> Vec<RelationSuggestion> {
    let active_db = ctx.active_db_id.clone();
    // find source table
    let mut source_db_id = active_db.clone();
    let mut source_table = None;
    for schema in ctx.all_schemas() {
        if let Some(t) = schema.tables.iter().find(|t| t.id == table_id) {
            source_db_id = schema.db_id.clone();
            source_table = Some(t.clone());
            break;
        }
    }
    let Some(src) = source_table else {
        return vec![];
    };

    let src_fields: Vec<&Field> = src.fields.iter().filter(|f| f.is_stored()).collect();

    let mut out = Vec::new();

    for tgt_schema in ctx.all_schemas() {
        for tgt in &tgt_schema.tables {
            if tgt.id == src.id && tgt_schema.db_id == source_db_id {
                continue;
            }
            // skip if already linked via existing relation from this source table
            // (but allow suggestion for other fields)
            for sf in &src_fields {
                // don't suggest link field itself
                if sf.field_type == "link" {
                    continue;
                }
                for tf in &tgt.fields {
                    if tf.field_type == "link" && tf.is_backlink() {
                        continue;
                    }
                    // type compatibility: text/email/phone <-> text/email/phone, number <-> number
                    let type_ok = matches!(
                        (sf.field_type.as_str(), tf.field_type.as_str()),
                        ("text", "text")
                            | ("text", "email")
                            | ("email", "text")
                            | ("email", "email")
                            | ("number", "number")
                            | ("select", "text")
                            | ("text", "select")
                            | ("url", "text")
                            | ("text", "url")
                    );
                    if !type_ok {
                        continue;
                    }
                    let sim = name_similarity(&sf.name, &tf.name);
                    let id_hint = if is_id_like(&sf.name, &sf.field_type) || is_id_like(&tf.name, &tf.field_type) {
                        0.15
                    } else {
                        0.0
                    };
                    let sv = collect_values(&src.sample, &sf.id);
                    let tv = collect_values(&tgt.sample, &tf.id);
                    let overlap = value_overlap(&sv, &tv);
                    // also check primary overlap if tf is not primary but tgt has primary text field
                    let mut confidence = sim * 0.5 + overlap * 0.5 + id_hint;
                    // bonus if field names contain shared token like "client", "product"
                    // and overlap >0
                    if overlap > 0.2 {
                        confidence += 0.15;
                    }
                    if overlap > 0.5 {
                        confidence += 0.15;
                    }
                    confidence = confidence.clamp(0.0, 1.0);

                    // thresholds
                    let threshold = if tgt_schema.db_id != source_db_id { 0.45 } else { 0.5 };
                    if confidence < threshold {
                        continue;
                    }
                    if overlap == 0.0 && sim < 0.6 {
                        continue;
                    }
                    // avoid duplicate suggestion for existing relation
                    if existing_relation(ctx, &sf.id, &tgt_schema.db_id, &tgt.id) {
                        continue;
                    }
                    // Cardinalité : valeurs quasi uniques par ligne -> chaque
                    // enregistrement référence une cible distincte ("one") ;
                    // valeurs répétées (ex. email partagé) -> "many".
                    let src_unique = {
                        let set: HashSet<&String> = sv.iter().collect();
                        set.len() as f64 / sv.len().max(1) as f64
                    };
                    let cardinality = if src_unique > 0.9 { "one" } else { "many" };
                    let reason = if overlap > 0.3 {
                        format!(
                            "Chevauchement de valeurs {:.0}% et similarité de noms {:.0}%",
                            overlap * 100.0,
                            sim * 100.0
                        )
                    } else {
                        format!("Similarité de noms {:.0}%", sim * 100.0)
                    };
                    out.push(RelationSuggestion {
                        source_db_id: source_db_id.clone(),
                        source_table_id: src.id.clone(),
                        source_table_name: src.name.clone(),
                        source_field_id: sf.id.clone(),
                        source_field_name: sf.name.clone(),
                        target_db_id: tgt_schema.db_id.clone(),
                        target_table_id: tgt.id.clone(),
                        target_table_name: tgt.name.clone(),
                        target_field_id: tf.id.clone(),
                        target_field_name: tf.name.clone(),
                        cardinality: cardinality.to_string(),
                        confidence,
                        reason,
                    });
                }
            }
        }
    }

    out.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
    out.truncate(10);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::context_builder::{AICrossDBContext, DatabaseSchema, TableSchema};
    use crate::db::models::Field;
    use serde_json::json;

    fn field(id: &str, name: &str, ftype: &str) -> Field {
        Field {
            id: id.into(),
            table_id: "tbl_src".into(),
            name: name.into(),
            field_type: ftype.into(),
            config: json!({}),
            position: 0,
        }
    }

    #[test]
    fn suggests_email_overlap() {
        let ctx = AICrossDBContext {
            active_db_id: "db1".into(),
            active_schema: Some(DatabaseSchema {
                db_id: "db1".into(),
                db_name: "Main".into(),
                tables: vec![
                    TableSchema {
                        id: "tbl_src".into(),
                        name: "Commandes".into(),
                        fields: vec![field("fld_a", "email_client", "email"), field("fld_b", "Montant", "number")],
                        sample: vec![
                            json!({"_id":"r1","fld_a":"alice@example.com"}),
                            json!({"_id":"r2","fld_a":"bob@example.com"}),
                        ],
                        row_count: 2,
                    },
                    TableSchema {
                        id: "tbl_tgt".into(),
                        name: "Clients".into(),
                        fields: vec![field("fld_c", "Email", "email"), field("fld_d", "Nom", "text")],
                        sample: vec![
                            json!({"_id":"c1","fld_c":"alice@example.com"}),
                            json!({"_id":"c2","fld_c":"bob@example.com"}),
                        ],
                        row_count: 2,
                    },
                ],
            }),
            reference_schemas: vec![],
            relations: vec![],
            sample_limit: 50,
        };
        let s = suggest_relations(&ctx, "tbl_src");
        assert!(!s.is_empty());
        assert_eq!(s[0].source_field_id, "fld_a");
        assert_eq!(s[0].target_field_id, "fld_c");
        assert!(s[0].confidence > 0.5);
    }

    #[test]
    fn no_suggest_when_no_overlap_and_low_sim() {
        let ctx = AICrossDBContext {
            active_db_id: "db1".into(),
            active_schema: Some(DatabaseSchema {
                db_id: "db1".into(),
                db_name: "Main".into(),
                tables: vec![
                    TableSchema {
                        id: "tbl_src".into(),
                        name: "A".into(),
                        fields: vec![field("fld_a", "Foo", "text")],
                        sample: vec![json!({"_id":"r1","fld_a":"xyz"})],
                        row_count: 1,
                    },
                    TableSchema {
                        id: "tbl_tgt".into(),
                        name: "B".into(),
                        fields: vec![field("fld_b", "Bar", "text")],
                        sample: vec![json!({"_id":"c1","fld_b":"abc"})],
                        row_count: 1,
                    },
                ],
            }),
            reference_schemas: vec![],
            relations: vec![],
            sample_limit: 50,
        };
        let s = suggest_relations(&ctx, "tbl_src");
        assert!(s.is_empty());
    }
}
