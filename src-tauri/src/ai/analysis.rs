use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::ai::context_builder::{AICrossDBContext, TableSchema};
use crate::ai::provider::CompletionRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStats {
    pub table_id: String,
    pub table_name: String,
    pub row_count: i64,
    pub fields: Vec<FieldStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldStats {
    pub field_id: String,
    pub field_name: String,
    pub field_type: String,
    pub non_null: usize,
    pub nulls: usize,
    pub distinct: usize,
    pub top_values: Vec<(String, usize)>,
    pub numeric: Option<NumericStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumericStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub sum: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub summary: String,
    pub insights: Vec<String>,
    pub suggestions: Vec<String>,
    pub stats: Vec<TableStats>,
    pub provider: String,
}

fn compute_stats(schema: &TableSchema) -> TableStats {
    let mut fields_stats = Vec::new();
    for f in &schema.fields {
        // Les champs sans colonne SQL (calculés ou backlink) sont absents du
        // sample : les compter produirait de faux "100% vides".
        match f.field_type.as_str() {
            "lookup" | "rollup" | "count" | "formula" => continue,
            "link" if f.is_backlink() => continue,
            _ => {}
        }
        let mut non_null = 0usize;
        let mut nulls = 0usize;
        let mut distinct_set: HashSet<String> = HashSet::new();
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut nums: Vec<f64> = Vec::new();

        for rec in &schema.sample {
            if let Some(v) = rec.get(&f.id) {
                if matches!(v, serde_json::Value::Null) {
                    nulls += 1;
                } else {
                    let s = match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => v.to_string(),
                    };
                    if s.trim().is_empty() {
                        nulls += 1;
                    } else {
                        non_null += 1;
                        distinct_set.insert(s.clone());
                        *counts.entry(s.clone()).or_default() += 1;
                        if f.field_type == "number" {
                            if let Ok(n) = s.parse::<f64>() {
                                nums.push(n);
                            } else if let Some(n) = v.as_f64() {
                                nums.push(n);
                            }
                        }
                    }
                }
            } else {
                nulls += 1;
            }
        }
        // if sample smaller than row_count, extrapolate nulls roughly?
        // keep as is for determinism
        let mut top: Vec<(String, usize)> = counts.into_iter().collect();
        top.sort_by_key(|x| std::cmp::Reverse(x.1));
        top.truncate(5);

        let numeric = if !nums.is_empty() {
            let min = nums.iter().cloned().fold(f64::INFINITY, |a, b| a.min(b));
            let max = nums.iter().cloned().fold(f64::NEG_INFINITY, |a, b| a.max(b));
            let sum: f64 = nums.iter().sum();
            let avg = sum / nums.len() as f64;
            Some(NumericStats { min, max, avg, sum })
        } else {
            None
        };

        fields_stats.push(FieldStats {
            field_id: f.id.clone(),
            field_name: f.name.clone(),
            field_type: f.field_type.clone(),
            non_null,
            nulls,
            distinct: distinct_set.len(),
            top_values: top,
            numeric,
        });
    }

    TableStats {
        table_id: schema.id.clone(),
        table_name: schema.name.clone(),
        row_count: schema.row_count,
        fields: fields_stats,
    }
}

fn heuristic_insights(stats: &[TableStats], question: Option<&str>) -> (String, Vec<String>, Vec<String>) {
    let mut summary_parts = Vec::new();
    let mut insights = Vec::new();
    let mut suggestions = Vec::new();

    for ts in stats {
        summary_parts.push(format!("Table '{}' : {} lignes", ts.table_name, ts.row_count));
        for fs in &ts.fields {
            if fs.nulls > 0 && fs.non_null > 0 {
                let pct = fs.nulls as f64 / (fs.nulls + fs.non_null) as f64 * 100.0;
                if pct > 30.0 {
                    insights.push(format!(
                        "Champ '{}' a {:.0}% de valeurs manquantes ({} vides / {})",
                        fs.field_name, pct, fs.nulls, fs.nulls + fs.non_null
                    ));
                    suggestions.push(format!("Nettoyer/compléter le champ '{}'", fs.field_name));
                }
            }
            if let Some(n) = &fs.numeric {
                insights.push(format!(
                    "Champ '{}' : min {:.2}, max {:.2}, moy {:.2}",
                    fs.field_name, n.min, n.max, n.avg
                ));
                if n.min < 0.0 {
                    suggestions.push(format!("Vérifier les valeurs négatives dans '{}'", fs.field_name));
                }
            }
            if fs.distinct == 1 && fs.non_null > 1 {
                insights.push(format!("Champ '{}' a une seule valeur distincte", fs.field_name));
            }
        }
        if ts.row_count == 0 {
            insights.push(format!("Table '{}' est vide", ts.table_name));
            suggestions.push(format!("Importer des données dans '{}'", ts.table_name));
        }
    }

    if let Some(q) = question {
        let ql = q.to_lowercase();
        if ql.contains("montant") || ql.contains("total") || ql.contains("somme") {
            for ts in stats {
                for fs in &ts.fields {
                    if let Some(n) = &fs.numeric {
                        if fs.field_name.to_lowercase().contains("montant") {
                            insights.push(format!("Total {} = {:.2}", fs.field_name, n.sum));
                        }
                    }
                }
            }
        }
        if ql.contains("vide") || ql.contains("manquant") {
            // already covered
        }
    }

    if insights.is_empty() {
        insights.push("Aucune anomalie majeure détectée dans l'échantillon".into());
    }
    if suggestions.is_empty() {
        suggestions.push("Aucune action recommandée".into());
    }

    let summary = if summary_parts.is_empty() {
        "Aucune table à analyser".to_string()
    } else {
        summary_parts.join(" | ")
    };

    (summary, insights, suggestions)
}

pub fn analyze_heuristic(ctx: &AICrossDBContext, table_id: Option<&str>, question: Option<&str>) -> AnalysisResult {
    let mut stats = Vec::new();
    for schema in ctx.all_schemas() {
        for t in &schema.tables {
            if let Some(tid) = table_id {
                if t.id != tid {
                    continue;
                }
            }
            stats.push(compute_stats(t));
        }
    }
    let (summary, insights, suggestions) = heuristic_insights(&stats, question);
    AnalysisResult {
        summary,
        insights,
        suggestions,
        stats,
        provider: "heuristic".into(),
    }
}

pub async fn analyze(
    ctx: &AICrossDBContext,
    table_id: Option<&str>,
    question: Option<&str>,
    provider: Option<&dyn crate::ai::provider::LLMProvider>,
) -> AnalysisResult {
    let heuristic = analyze_heuristic(ctx, table_id, question);
    if let Some(p) = provider {
        let system = format!(
            "Tu es analyste data. À partir des stats et échantillon, fournis insights concis en français. Réponds en JSON {{\"summary\":\"...\",\"insights\":[\"...\"],\"suggestions\":[\"...\"]}}. Contexte:\n{}\nStats:\n{}",
            crate::ai::context_builder::context_prompt(ctx),
            serde_json::to_string_pretty(&heuristic.stats).unwrap_or_default()
        );
        let user = format!(
            "Question: {}. Fournis analyse et suggestions actionnables.",
            question.unwrap_or("Analyse générale de la base")
        );
        let req = CompletionRequest {
            system,
            user,
            max_tokens: 800,
            temperature: 0.3,
            json_mode: true,
        };
        if let Ok(txt) = p.complete(req).await {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                let summary = v.get("summary").and_then(|x| x.as_str()).map(|s| s.to_string()).unwrap_or(heuristic.summary.clone());
                let insights = v.get("insights").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or(heuristic.insights.clone());
                let suggestions = v.get("suggestions").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or(heuristic.suggestions.clone());
                return AnalysisResult {
                    summary,
                    insights,
                    suggestions,
                    stats: heuristic.stats,
                    provider: p.name().to_string(),
                };
            }
        }
    }
    heuristic
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
    fn numeric_stats() {
        let schema = TableSchema {
            id: "tbl1".into(),
            name: "T".into(),
            fields: vec![field("fld1", "Montant", "number")],
            sample: vec![
                json!({"_id":"r1","fld1":10}),
                json!({"_id":"r2","fld1":20}),
                json!({"_id":"r3","fld1":null}),
            ],
            row_count: 3,
        };
        let ctx = AICrossDBContext { active_db_id: "db1".into(), active_schema: Some(DatabaseSchema { db_id: "db1".into(), db_name: "Main".into(), tables: vec![schema] }), reference_schemas: vec![], relations: vec![], sample_limit: 50 };
        let res = analyze_heuristic(&ctx, Some("tbl1"), None);
        assert!(res.stats[0].fields[0].numeric.is_some());
        assert_eq!(res.stats[0].fields[0].nulls, 1);
    }
}
