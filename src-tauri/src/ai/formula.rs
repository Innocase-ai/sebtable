use serde::{Deserialize, Serialize};

use crate::ai::context_builder::AICrossDBContext;
use crate::ai::provider::CompletionRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaResult {
    pub expression: String,
    pub explanation: String,
    pub valid: bool,
    pub error: Option<String>,
    pub provider: String,
}

/// Champs référencés `{Nom}` dans l'expression mais absents de la table :
/// une formule qui y fait référence parserait OK mais évaluerait à Null.
fn missing_fields(expr: &str, ctx: &AICrossDBContext, table_id: &str) -> Vec<String> {
    let names: Vec<String> = ctx
        .all_schemas()
        .iter()
        .flat_map(|s| s.tables.iter())
        .filter(|t| t.id == table_id)
        .flat_map(|t| t.fields.iter().map(|f| f.name.clone()))
        .collect();
    let mut miss = Vec::new();
    let mut rest = expr;
    while let Some(start) = rest.find('{') {
        match rest[start..].find('}') {
            Some(endrel) => {
                let name = &rest[start + 1..start + endrel];
                if !name.is_empty() && !names.iter().any(|n| n == name) {
                    miss.push(name.to_string());
                }
                rest = &rest[start + endrel + 1..];
            }
            None => break,
        }
    }
    miss
}

fn heuristic_formula(ctx: &AICrossDBContext, table_id: &str, prompt: &str) -> Option<(String, String)> {
    let p = prompt.to_lowercase();
    // collect fields for table
    let mut fields: Vec<(String, String)> = Vec::new(); // (name, type)
    for schema in ctx.all_schemas() {
        for t in &schema.tables {
            if t.id == table_id {
                for f in &t.fields {
                    fields.push((f.name.clone(), f.field_type.clone()));
                }
            }
        }
    }
    // helpers to find field name matching keywords
    // Pas de fallback "premier champ" : une référence vers un champ sans rapport
    // produirait une suggestion trompeuse.
    let find_field = |keywords: &[&str]| -> Option<String> {
        for kw in keywords {
            for (name, _) in &fields {
                if name.to_lowercase().contains(kw) {
                    return Some(name.clone());
                }
            }
        }
        None
    };

    // 1. Si / Si statut = ...
    if p.contains("si ") || p.contains("if ") || p.contains("quand") {
        if p.contains("done") || p.contains("termin") || p.contains("fini") {
            let f = find_field(&["statut", "status", "état", "state"])?;
            return Some((format!("IF({{{}}}=\'Done\', 1, 0)", f), "Formule conditionnelle IF".into()));
        }
        if p.contains("élevé") || p.contains("eleve") || p.contains("sup") || p.contains(">") || p.contains("montant") {
            let montant = find_field(&["montant", "amount", "prix", "total"])?;
            return Some((format!("IF({{{}}} > 50, 'Elevé', 'Bas')", montant), "IF sur montant".into()));
        }
        let f = find_field(&["statut", "status", "état", "state"])?;
        return Some((format!("IF({{{}}}='Oui', 1, 0)", f), "IF générique".into()));
    }
    // somme / total
    if p.contains("somme") || p.contains("sum") || p.contains("total") || p.contains("addition") {
        let n = find_field(&["montant", "amount", "prix", "quantit", "total"])?;
        return Some((format!("SUM({{{}}})", n), "Somme".into()));
    }
    if p.contains("moyenne") || p.contains("average") || p.contains("avg") {
        let n = find_field(&["montant", "note", "score", "prix"])?;
        return Some((format!("AVERAGE({{{}}})", n), "Moyenne".into()));
    }
    if p.contains("concat") || p.contains("assembl") || p.contains("joindre") || p.contains("nom complet") {
        if fields.len() >= 2 {
            let a = fields[0].0.clone();
            let b = fields[1].0.clone();
            return Some((format!("CONCATENATE({{{}}}, ' - ', {{{}}})", a, b), "Concaténation".into()));
        }
        return None;
    }
    if p.contains("majuscule") || p.contains("uppercase") || p.contains("upper") {
        let n = find_field(&["nom", "name", "titre", "title"])?;
        return Some((format!("UPPER({{{}}})", n), "Mise en majuscules".into()));
    }
    if p.contains("minuscule") || p.contains("lowercase") {
        let n = find_field(&["nom", "name"])?;
        return Some((format!("LOWER({{{}}})", n), "Mise en minuscules".into()));
    }
    if p.contains("longueur") || p.contains("len") || p.contains("taille") {
        let n = find_field(&["nom", "description", "texte"])?;
        return Some((format!("LEN({{{}}})", n), "Longueur".into()));
    }
    if p.contains("date") || p.contains("diff") || p.contains("jours") || p.contains("jours entre") {
        let has = |needle: &str| fields.iter().any(|(n, _)| n.eq_ignore_ascii_case(needle));
        if has("Date fin") && has("Date début") {
            return Some(("DATETIME_DIFF({Date fin}, {Date début}, 'days')".into(), "Différence de dates".into()));
        }
        return None;
    }
    if p.contains("arrondi") || p.contains("round") {
        let n = find_field(&["montant", "prix"])?;
        return Some((format!("ROUND({{{}}}, 2)", n), "Arrondi".into()));
    }
    None
}

/// Résultat quand la forme de la formule est reconnue mais qu'aucun champ de la
/// table ne permet de la construire honnêtement.
fn no_compatible_field(prompt_kind: &str) -> FormulaResult {
    FormulaResult {
        expression: String::new(),
        explanation: format!("Prompt « {prompt_kind} » reconnu mais aucun champ compatible dans cette table"),
        valid: false,
        error: Some("aucun champ compatible pour cette suggestion".into()),
        provider: "heuristic".into(),
    }
}

pub fn generate_heuristic(ctx: &AICrossDBContext, table_id: &str, prompt: &str) -> FormulaResult {
    // Validation complète : parse + tous les {Champs} référencés doivent exister.
    let validate = |expr: &str, expl: String| -> FormulaResult {
        let missing = missing_fields(expr, ctx, table_id);
        let parse_ok = crate::formula::parse(expr).is_ok();
        let valid = parse_ok && missing.is_empty();
        let error = if valid {
            None
        } else if !missing.is_empty() {
            Some(format!("champs inexistants dans la table : {}", missing.join(", ")))
        } else {
            Some("formule invalide (parse)".into())
        };
        FormulaResult { expression: expr.to_string(), explanation: expl, valid, error, provider: "heuristic".into() }
    };

    // 0) Prompt déjà formulé en langage de formule -> valider tel quel AVANT
    //    les heuristiques (sinon "SUM({X})" serait réécrit en SUM({Montant})).
    let t = prompt.trim();
    let func_prefixed = ["IF", "SWITCH", "SUM", "AVERAGE", "CONCATENATE", "UPPER", "LOWER", "LEN", "ROUND", "DATETIME_DIFF"]
        .iter()
        .any(|f| t.starts_with(f));
    if func_prefixed || (t.contains('{') && crate::formula::parse(t).is_ok()) {
        return validate(t, "Formule fournie telle quelle".into());
    }
    if let Some((expr, expl)) = heuristic_formula(ctx, table_id, prompt) {
        return validate(&expr, expl);
    }
    // Exemple par défaut construit sur les champs RÉELS de la table (jamais un
    // nom de champ inventé) ; sinon résultat explicite invalide.
    let field_of = |ty: &str| {
        ctx.all_schemas()
            .iter()
            .flat_map(|s| s.tables.iter())
            .filter(|t| t.id == table_id)
            .flat_map(|t| t.fields.iter())
            .find(|f| f.field_type == ty)
            .map(|f| f.name.clone())
    };
    match (field_of("select").or_else(|| field_of("text")), field_of("number")) {
        (Some(s), Some(m)) => validate(
            &format!("IF({{{s}}}='Done', {{{m}}}, 0)"),
            "Exemple par défaut IF sur les champs existants".into(),
        ),
        (Some(s), None) => validate(
            &format!("IF({{{s}}}='Done', 1, 0)"),
            "Exemple par défaut IF sur champ texte/select existant".into(),
        ),
        _ => no_compatible_field("non reconnu"),
    }
}

pub async fn generate_formula(
    ctx: &AICrossDBContext,
    table_id: &str,
    prompt: &str,
    provider: Option<&dyn crate::ai::provider::LLMProvider>,
) -> FormulaResult {
    // try LLM first if available
    if let Some(p) = provider {
        let system = format!(
            "Tu es un expert formules Airtable. Réponds en JSON {{\"expression\": \"...\", \"explanation\": \"...\"}}. Formules supportées: IF, SWITCH, AND, OR, NOT, CONCATENATE, LEFT/RIGHT/MID, LEN, LOWER/UPPER/TRIM, REGEX_MATCH/EXTRACT, SUM/AVERAGE/MIN/MAX/ROUND/ABS/MOD, DATETIME_DIFF/FORMAT/DATEADD/TODAY/NOW, ARRAYJOIN/UNIQUE/COMPACT. Contexte:\n{}",
            crate::ai::context_builder::context_prompt(ctx)
        );
        let user = format!("Prompt utilisateur: {}\nTable: {}\nGénère la formule Airtable correspondante.", prompt, table_id);
        let req = CompletionRequest {
            system,
            user,
            max_tokens: 500,
            temperature: 0.2,
            json_mode: true,
        };
        if let Ok(txt) = p.complete(req).await {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                if let Some(expr) = v.get("expression").and_then(|x| x.as_str()) {
                    let expl = v.get("explanation").and_then(|x| x.as_str()).unwrap_or("LLM").to_string();
                    // parse + champs référencés doivent exister, sinon heuristique
                    let missing = missing_fields(expr, ctx, table_id);
                    if crate::formula::parse(expr).is_ok() && missing.is_empty() {
                        return FormulaResult {
                            expression: expr.to_string(),
                            explanation: expl,
                            valid: true,
                            error: None,
                            provider: p.name().to_string(),
                        };
                    }
                    // if invalid or references unknown fields, fallback to heuristic
                }
            }
        }
    }
    generate_heuristic(ctx, table_id, prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::context_builder::{AICrossDBContext, DatabaseSchema, TableSchema};
    use crate::db::models::Field;
    use serde_json::json;

    fn ctx_with_fields() -> AICrossDBContext {
        AICrossDBContext {
            active_db_id: "db1".into(),
            active_schema: Some(DatabaseSchema {
                db_id: "db1".into(),
                db_name: "Main".into(),
                tables: vec![TableSchema {
                    id: "tbl1".into(),
                    name: "Commandes".into(),
                    fields: vec![
                        Field { id: "fld1".into(), table_id: "tbl1".into(), name: "Montant".into(), field_type: "number".into(), config: json!({}), position: 0 },
                        Field { id: "fld2".into(), table_id: "tbl1".into(), name: "Statut".into(), field_type: "select".into(), config: json!({}), position: 1 },
                        Field { id: "fld3".into(), table_id: "tbl1".into(), name: "Nom".into(), field_type: "text".into(), config: json!({}), position: 2 },
                    ],
                    sample: vec![],
                    row_count: 0,
                }],
            }),
            reference_schemas: vec![],
            relations: vec![],
            sample_limit: 50,
        }
    }

    #[test]
    fn heuristic_si_montant() {
        let ctx = ctx_with_fields();
        let r = generate_heuristic(&ctx, "tbl1", "si montant > 50 alors élevé sinon bas");
        assert!(r.valid);
        assert!(r.expression.contains("IF"));
    }

    #[test]
    fn heuristic_somme() {
        let ctx = ctx_with_fields();
        let r = generate_heuristic(&ctx, "tbl1", "somme du montant");
        assert!(r.valid);
        assert!(r.expression.contains("SUM"));
    }
}
