use serde::{Deserialize, Serialize};
use sqlx::Row;
use crate::commands::pool_for_db;
use crate::db::models::FieldInput;
use crate::error::AppError;
use crate::AppState;
use std::collections::HashMap;
use tauri::State;

/// Options d'import (MVP : table existante ou création simple).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOptions {
    /// "csv" | "json" | "xlsx" (case-insensitive, défaut = auto par extension)
    #[serde(default)]
    pub format: String,
    /// Table cible existante. Si None + table_name fourni -> création.
    #[serde(default)]
    pub table_id: Option<String>,
    #[serde(default)]
    pub table_name: Option<String>,
    /// CSV/XLSX : première ligne = en-tête (défaut true)
    #[serde(default = "default_true")]
    pub has_header: bool,
}

fn default_true() -> bool { true }

/// Id entrant réutilisable à l'import : uniquement les ids natifs de l'app
/// (préfixe `rec_`) — sinon on génère un nouvel id. Évite qu'un fichier
/// externe portant une colonne `id` homonyme écrase des records existants.
fn usable_import_id(id: Option<String>) -> String {
    match id {
        Some(s) if s.starts_with("rec_") => s,
        _ => crate::utils::new_id("rec"),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported_rows: usize,
    pub table_id: String,
    pub errors: Vec<String>,
}

// ---- Helpers ---------------------------------------------------------------

fn csv_escape_formula(s: &str) -> String {
    // Prévention d'injection CSV/Excel : neutralise = + - @ au début
    if s.starts_with('=') || s.starts_with('+') || s.starts_with('-') || s.starts_with('@') {
        format!("'{}", s)
    } else { s.to_string() }
}

#[allow(dead_code)]
fn guess_field_type(values: &[String]) -> &'static str {
    if values.iter().all(|s| s.trim().is_empty() || s.parse::<f64>().is_ok()) && values.iter().any(|s| s.parse::<f64>().is_ok()) {
        "number"
    } else if values.iter().all(|s| s.trim().is_empty() || matches!(s.to_lowercase().as_str(), "true"|"false"|"1"|"0"|"oui"|"non")) {
        "checkbox"
    } else {
        "text"
    }
}

/// Inférence des en-têtes depuis le fichier (CPU → spawn_blocking).
fn infer_headers(fmt: &str, file: &[u8], has_header: bool) -> Result<Vec<String>, AppError> {
    match fmt {
        "csv" => {
            let mut rdr = csv::ReaderBuilder::new().has_headers(has_header).from_reader(file);
            if has_header {
                rdr.headers().map(|h| h.iter().map(|s| s.to_string()).collect()).map_err(|e| AppError::Msg(e.to_string()))
            } else {
                let first = rdr.records().next().transpose().map_err(|e| AppError::Msg(e.to_string()))?;
                Ok(first.map(|rec| (0..rec.len()).map(|i| format!("Col{}", i+1)).collect()).unwrap_or_default())
            }
        }
        "json" => {
            let v: serde_json::Value = serde_json::from_slice(file).map_err(|e| AppError::Msg(e.to_string()))?;
            if let Some(arr) = v.as_array().and_then(|a| a.first()).and_then(|o| o.as_object()) {
                Ok(arr.keys().cloned().collect())
            } else { Ok(vec!["Col1".into()]) }
        }
        _ => {
            use calamine::{open_workbook_auto_from_rs, Reader};
            let mut wb = open_workbook_auto_from_rs(std::io::Cursor::new(file)).map_err(|e| AppError::Msg(e.to_string()))?;
            let range = wb.worksheet_range_at(0).ok_or_else(|| AppError::Msg("feuille xlsx vide".into()))?.map_err(|e| AppError::Msg(e.to_string()))?;
            Ok(range.rows().next().map(|r| r.iter().map(|c| c.to_string()).collect()).unwrap_or_default())
        }
    }
}

/// Parse complet du fichier (CPU → spawn_blocking). Retourne les records et
/// les erreurs ligne par ligne. `field_types` : id -> type, `stored_names` :
/// noms des champs stockés dans l'ordre (pour CSV/XLSX sans en-tête).
fn parse_import(
    fmt: &str,
    file: &[u8],
    options: &ImportOptions,
    name_to_id: &HashMap<String, String>,
    field_types: &HashMap<String, String>,
    stored_names: &[String],
) -> Result<(Vec<serde_json::Value>, Vec<String>), AppError> {
    let mut records: Vec<serde_json::Value> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    match fmt {
        "csv" => {
            let mut rdr = csv::ReaderBuilder::new()
                .has_headers(options.has_header)
                .flexible(true)
                .trim(csv::Trim::All)
                .from_reader(file);
            let headers: Vec<String> = if options.has_header {
                rdr.headers().map(|h| h.iter().map(|s| s.to_string()).collect()).unwrap_or_default()
            } else {
                stored_names.to_vec()
            };
            let id_col_idx = headers.iter().position(|h| h.to_lowercase() == "_id" || h.to_lowercase() == "id");
            for (idx, rec) in rdr.records().enumerate() {
                match rec {
                    Ok(r) => {
                        let incoming_id = id_col_idx.and_then(|i| r.get(i)).map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string());
                        let rid = usable_import_id(incoming_id);
                        let mut m = serde_json::Map::new();
                        m.insert("_id".into(), serde_json::json!(rid));
                        for (col_idx, val) in r.iter().enumerate() {
                            if Some(col_idx) == id_col_idx { continue; }
                            let col_name = headers.get(col_idx).cloned().unwrap_or_else(|| format!("col{col_idx}"));
                            if let Some(fid) = name_to_id.get(&col_name.to_lowercase()) {
                                let ftype = field_types.get(fid).map(|s| s.as_str()).unwrap_or("text");
                                let v = match ftype {
                                    "number" => val.parse::<f64>().map(|n| serde_json::json!(n)).unwrap_or(serde_json::json!(val)),
                                    "checkbox" => {
                                        let b = matches!(val.to_lowercase().as_str(), "1"|"true"|"oui"|"yes"|"vrai");
                                        serde_json::json!(b)
                                    }
                                    _ => serde_json::json!(val),
                                };
                                m.insert(fid.clone(), v);
                            }
                        }
                        if m.len() == 1 {
                            errors.push(format!("ligne {}: aucune colonne reconnue, ignorée", idx + 1));
                        } else {
                            records.push(serde_json::Value::Object(m));
                        }
                    }
                    Err(e) => errors.push(format!("ligne {}: {}", idx+1, e)),
                }
                if records.len() >= 10000 {
                    errors.push("limite 10 000 lignes atteinte, tronqué".into());
                    break;
                }
            }
        }
        "json" => {
            let v: serde_json::Value = serde_json::from_slice(file).map_err(|e| AppError::Msg(format!("JSON invalide: {e}")))?;
            let arr = if let Some(a) = v.as_array() { a.clone() } else { vec![v] };
            for (idx, item) in arr.into_iter().enumerate() {
                if let Some(obj) = item.as_object() {
                    let incoming_id = obj.get("_id").or_else(|| obj.get("id")).and_then(|v| {
                        if let Some(s) = v.as_str() { Some(s.trim().to_string()) } else if v.is_number() { Some(v.to_string()) } else { None }
                    }).filter(|s| !s.is_empty());
                    let rid = usable_import_id(incoming_id);
                    let mut m = serde_json::Map::new();
                    m.insert("_id".into(), serde_json::json!(rid));
                    for (k, val) in obj {
                        if k.to_lowercase() == "_id" || k.to_lowercase() == "id" { continue; }
                        if let Some(fid) = name_to_id.get(&k.to_lowercase()) {
                            m.insert(fid.clone(), val.clone());
                        }
                    }
                    if m.len() == 1 {
                        errors.push(format!("entrée {idx}: aucune colonne reconnue, ignorée"));
                    } else {
                        records.push(serde_json::Value::Object(m));
                    }
                } else {
                    errors.push(format!("entrée {idx}: objet attendu"));
                }
                if records.len() >= 10000 { break; }
            }
        }
        "xlsx" | "excel" => {
            use calamine::{open_workbook_auto_from_rs, Reader, Data};
            let mut wb = open_workbook_auto_from_rs(std::io::Cursor::new(file)).map_err(|e| AppError::Msg(e.to_string()))?;
            let range = wb.worksheet_range_at(0).ok_or_else(|| AppError::Msg("xlsx: aucune feuille".into()))?.map_err(|e| AppError::Msg(e.to_string()))?;
            let mut rows = range.rows();
            let headers: Vec<String> = if options.has_header {
                rows.next().map(|r| r.iter().map(|c| match c { Data::String(s) => s.clone(), _ => c.to_string() }).collect()).unwrap_or_default()
            } else {
                stored_names.to_vec()
            };
            let id_col_idx = headers.iter().position(|h| h.to_lowercase() == "_id" || h.to_lowercase() == "id");
            for r in rows {
                let incoming_id = id_col_idx.and_then(|i| r.get(i)).map(|c| match c { Data::String(s) => s.trim().to_string(), _ => c.to_string().trim().to_string() }).filter(|s| !s.is_empty());
                let rid = usable_import_id(incoming_id);
                let mut m = serde_json::Map::new();
                m.insert("_id".into(), serde_json::json!(rid));
                for (col_idx, cell) in r.iter().enumerate() {
                    if Some(col_idx) == id_col_idx { continue; }
                    let col_name = headers.get(col_idx).cloned().unwrap_or_else(|| format!("col{col_idx}"));
                    if let Some(fid) = name_to_id.get(&col_name.to_lowercase()) {
                        let ftype = field_types.get(fid).map(|s| s.as_str()).unwrap_or("text");
                        let s = match cell { Data::String(v) => v.clone(), _ => cell.to_string() };
                        let v = match ftype {
                            "number" => s.parse::<f64>().map(|n| serde_json::json!(n)).unwrap_or(serde_json::json!(s)),
                            "checkbox" => { let b = matches!(s.to_lowercase().as_str(), "1"|"true"|"oui"|"yes"); serde_json::json!(b) },
                            _ => serde_json::json!(s),
                        };
                        m.insert(fid.clone(), v);
                    }
                }
                if m.len() == 1 {
                    errors.push("ligne xlsx: aucune colonne reconnue, ignorée".into());
                } else {
                    records.push(serde_json::Value::Object(m));
                }
                if records.len() >= 10000 { break; }
            }
        }
        _ => return Err(AppError::Msg(format!("format d'import inconnu: {fmt} (csv|json|xlsx)"))),
    }
    Ok((records, errors))
}

// ---- Export ----------------------------------------------------------------

#[tauri::command]
pub async fn export_table(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    format: String,
) -> Result<Vec<u8>, AppError> {
    let fmt = format.to_lowercase();
    let pool = pool_for_db(&state, &db_id).await?;
    let fields = crate::db::repository::list_fields(&pool, &table_id).await?;
    let stored: Vec<_> = fields.iter().filter(|f| f.is_stored()).collect();
    if stored.is_empty() {
        return Err(AppError::Msg("table sans colonne exportable".into()));
    }
    // Fetch all rows (pas de pagination, streaming via query)
    let cols: Vec<String> = {
        let mut c = vec![crate::db::repository::quote_ident_public("_id")];
        for f in &stored { c.push(crate::db::repository::quote_ident_public(&f.id)); }
        c
    };
    let tbl = crate::db::repository::quote_ident_public(&table_id);
    let sql = format!("SELECT {} FROM {} ORDER BY _id", cols.join(", "), tbl);
    let rows = sqlx::query(&sql).fetch_all(&pool).await?;

    // Sérialisation CPU (csv/xlsx) hors du runtime async : les données sont
    // toutes en mémoire (`Vec<SqliteRow>` = owned, Send), spawn_blocking suffit.
    let fmt2 = fmt.clone();
    let stored_owned: Vec<(String, String, String)> = stored
        .iter()
        .map(|f| (f.id.clone(), f.name.clone(), f.field_type.clone()))
        .collect();
    let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, AppError> {
        match fmt2.as_str() {
            "csv" => {
                let mut wtr = csv::WriterBuilder::new().has_headers(true).from_writer(vec![]);
                // header
                let header: Vec<String> = stored_owned.iter().map(|(_, n, _)| n.clone()).collect();
                wtr.write_record(&header).map_err(|e| AppError::Msg(e.to_string()))?;
                for row in rows {
                    let mut rec: Vec<String> = Vec::with_capacity(stored_owned.len());
                    for (fid, _, ftype) in &stored_owned {
                        let s = match ftype.as_str() {
                            "number" => row.try_get::<Option<f64>, _>(fid.as_str()).ok().flatten().map(|v| v.to_string()).unwrap_or_default(),
                            "checkbox" => row.try_get::<Option<i64>, _>(fid.as_str()).ok().flatten().map(|v| if v!=0 {"1".into()} else {"0".into()}).unwrap_or_default(),
                            // select : colonne stocke le JSON `"opt_1"` → exporter l'id propre (ré-import ré-encode)
                            "select" => row.try_get::<Option<String>, _>(fid.as_str()).ok().flatten().map(|s| s.trim_matches('"').to_string()).unwrap_or_default(),
                            _ => row.try_get::<Option<String>, _>(fid.as_str()).ok().flatten().unwrap_or_default(),
                        };
                        // Échappement CSV injection seulement sur le texte : préfixer `'`
                        // sur un nombre négatif casserait le round-trip (parse → NULL).
                        let is_text = matches!(ftype.as_str(), "text" | "long_text" | "email" | "url" | "phone" | "select" | "link" | "attachment");
                        rec.push(if is_text { csv_escape_formula(&s) } else { s });
                    }
                    wtr.write_record(&rec).map_err(|e| AppError::Msg(e.to_string()))?;
                }
                Ok(wtr.into_inner().map_err(|e| AppError::Msg(e.to_string()))?)
            }
            "json" => {
                let mut out = Vec::new();
                for row in rows {
                    let mut m = serde_json::Map::new();
                    let id: String = row.try_get("_id").unwrap_or_default();
                    m.insert("_id".into(), serde_json::json!(id));
                    for (fid, name, ftype) in &stored_owned {
                        let v = match ftype.as_str() {
                            "number" => row.try_get::<Option<f64>, _>(fid.as_str()).ok().flatten().map(|n| serde_json::json!(n)).unwrap_or(serde_json::Value::Null),
                            "checkbox" => row.try_get::<Option<i64>, _>(fid.as_str()).ok().flatten().map(|n| serde_json::json!(n!=0)).unwrap_or(serde_json::Value::Null),
                            // select : id propre (ré-import ré-encode via push_value)
                            "select" => row.try_get::<Option<String>, _>(fid.as_str()).ok().flatten().map(|s| serde_json::json!(s.trim_matches('"'))).unwrap_or(serde_json::Value::Null),
                            _ => row.try_get::<Option<String>, _>(fid.as_str()).ok().flatten().map(|s| serde_json::json!(s)).unwrap_or(serde_json::Value::Null),
                        };
                        m.insert(name.clone(), v);
                    }
                    out.push(serde_json::Value::Object(m));
                }
                serde_json::to_vec_pretty(&out).map_err(|e| AppError::Msg(e.to_string()))
            }
            "xlsx" | "excel" => {
                use rust_xlsxwriter::{Workbook, Format};
                let mut workbook = Workbook::new();
                let ws = workbook.add_worksheet();
                ws.set_name("Export").ok();
                let header_fmt = Format::new().set_bold();
                for (col, (_, name, _)) in stored_owned.iter().enumerate() {
                    ws.write_string_with_format(0, col as u16, name, &header_fmt).map_err(|e| AppError::Msg(e.to_string()))?;
                    ws.set_column_width(col as u16, 18).ok();
                }
                for (r, row) in rows.iter().enumerate() {
                    for (c, (fid, _, ftype)) in stored_owned.iter().enumerate() {
                        let s = match ftype.as_str() {
                            "number" => row.try_get::<Option<f64>, _>(fid.as_str()).ok().flatten().map(|v| v.to_string()).unwrap_or_default(),
                            "checkbox" => row.try_get::<Option<i64>, _>(fid.as_str()).ok().flatten().map(|v| if v!=0 {"1".into()} else {"0".into()}).unwrap_or_default(),
                            "select" => row.try_get::<Option<String>, _>(fid.as_str()).ok().flatten().map(|s| s.trim_matches('"').to_string()).unwrap_or_default(),
                            _ => row.try_get::<Option<String>, _>(fid.as_str()).ok().flatten().unwrap_or_default(),
                        };
                        // number try write as number
                        if ftype == "number" {
                            if let Ok(n) = s.parse::<f64>() {
                                let _ = ws.write_number((r+1) as u32, c as u16, n);
                                continue;
                            }
                        }
                        let safe = csv_escape_formula(&s);
                        ws.write_string((r+1) as u32, c as u16, &safe).map_err(|e| AppError::Msg(e.to_string()))?;
                    }
                }
                workbook.save_to_buffer().map_err(|e| AppError::Msg(e.to_string()))
            }
            _ => Err(AppError::Msg(format!("format d'export inconnu: {fmt2} (csv|json|xlsx)"))),
        }
    })
    .await
    .map_err(|e| AppError::Msg(format!("export interrompu : {e}")))??;
    Ok(bytes)
}

// ---- Import ----------------------------------------------------------------

#[tauri::command]
pub async fn import_table(
    state: State<'_, AppState>,
    db_id: String,
    file: Vec<u8>,
    options: ImportOptions,
) -> Result<ImportResult, AppError> {
    if file.is_empty() {
        return Err(AppError::Msg("fichier vide".into()));
    }
    if file.len() > 20 * 1024 * 1024 {
        return Err(AppError::Msg("fichier trop volumineux (max 20 Mo)".into()));
    }
    let fmt = options.format.to_lowercase();
    let pool = pool_for_db(&state, &db_id).await?;

    // Résoudre table cible (existante ou création)
    let target_table_id: String = if let Some(tid) = options.table_id.clone() {
        // vérifier existence
        let tables = crate::db::repository::list_tables(&pool).await?;
        if !tables.iter().any(|t| t.id == tid) {
            return Err(AppError::Msg(format!("table cible introuvable: {tid}")));
        }
        tid
    } else {
        // création : inférer colonnes depuis le fichier (header) hors runtime async
        let name = options.table_name.clone().unwrap_or_else(|| "Import".into());
        let headers: Vec<String> = {
            let file = file.clone();
            let fmt = fmt.clone();
            let has_header = options.has_header;
            tokio::task::spawn_blocking(move || infer_headers(&fmt, &file, has_header))
                .await
                .map_err(|e| AppError::Msg(format!("import interrompu : {e}")))?
        }?;
        if headers.is_empty() {
            return Err(AppError::Msg("en-tête vide, import impossible".into()));
        }
        let inputs: Vec<FieldInput> = headers.into_iter().map(|h| FieldInput { name: if h.trim().is_empty() { "Col".into() } else { h }, field_type: "text".into(), config: serde_json::json!({}) }).collect();
        let tbl = crate::db::repository::create_table(&pool, name, inputs, None).await?;
        tbl.id
    };

    let fields = crate::db::repository::list_fields(&pool, &target_table_id).await?;
    let stored: Vec<_> = fields.iter().filter(|f| f.is_stored()).collect();
    // mapping nom de colonne (lower) -> field id + types pour le parse offline
    let mut name_to_id: HashMap<String, String> = HashMap::new();
    let mut field_types: HashMap<String, String> = HashMap::new();
    let mut stored_names: Vec<String> = Vec::new();
    for f in &stored {
        name_to_id.insert(f.name.to_lowercase(), f.id.clone());
        name_to_id.insert(f.id.to_lowercase(), f.id.clone());
        field_types.insert(f.id.clone(), f.field_type.clone());
        stored_names.push(f.name.clone());
    }

    // Parser selon format : CPU-heavy, exécuté hors du runtime async.
    let fmt2 = fmt.clone();
    let file2 = file;
    let opts2 = options.clone();
    let (records, errors) = tokio::task::spawn_blocking(move || {
        parse_import(&fmt2, &file2, &opts2, &name_to_id, &field_types, &stored_names)
    })
    .await
    .map_err(|e| AppError::Msg(format!("import interrompu : {e}")))??;

    if records.is_empty() && errors.is_empty() {
        return Err(AppError::Msg("aucune ligne à importer".into()));
    }
    let imported = records.len();
    if !records.is_empty() {
        crate::db::repository::upsert_records(&pool, &target_table_id, records).await?;
    }
    Ok(ImportResult { imported_rows: imported, table_id: target_table_id, errors })
}
