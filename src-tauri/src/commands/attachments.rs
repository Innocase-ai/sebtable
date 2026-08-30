use serde::{Deserialize, Serialize};
use crate::commands::pool_for_db;
use crate::error::AppError;
use crate::AppState;
use tauri::State;

/// Métadonnée d'un fichier attaché (stockée en JSON dans le champ `attachment`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentMeta {
    pub name: String,
    pub url: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub mime: String,
}

pub(crate) fn sanitize_filename(name: &str) -> String {
    let base = std::path::Path::new(name).file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let safe: String = base.chars().map(|c| if matches!(c, '/'|'\\'|':'|'*'|'?'|'"'|'<'|'>'|'|') { '_' } else { c }).collect();
    // Composant exact `..` → résoudrait vers le parent (dir_path.join).
    // Un nom comme `c..png` est un composant normal (safe).
    if safe == ".." {
        return "file".into();
    }
    // tronquer SANS couper un caractère UTF-8 (safe[..120] paniquerait sinon)
    if safe.len() > 120 {
        safe.char_indices().take_while(|(i, _)| *i < 120).map(|(_, c)| c).collect()
    } else {
        safe
    }
}

fn validate_id_component(label: &str, v: &str) -> Result<(), AppError> {
    if v.is_empty() || v.len() > 64 {
        return Err(AppError::Msg(format!("{label} invalide (vide ou trop long)")));
    }
    if v.contains('/') || v.contains('\\') || v.contains("..") {
        return Err(AppError::Msg(format!("{label} invalide (caractères interdits)")));
    }
    if !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(AppError::Msg(format!("{label} invalide (alphanum/_/- uniquement)")));
    }
    Ok(())
}

fn workspace_attachments_dir(state_dir: &std::path::Path, db_id: &str, table_id: &str, record_id: &str) -> std::path::PathBuf {
    state_dir.join("attachments").join(db_id).join(table_id).join(record_id)
}

/// Résout `p` (doit exister) et refuse si, après canonicalize, il sort du
/// workspace. Neutralise les symlinks de parents qui redirigeraient l'accès
/// hors de `attachments/...` malgré la validation des composants.
fn canonical_inside_workspace(dir: &std::path::Path, p: &std::path::Path) -> Result<std::path::PathBuf, AppError> {
    let base = dir
        .canonicalize()
        .map_err(|e| AppError::Msg(format!("workspace inaccessible : {e}")))?;
    let resolved = p
        .canonicalize()
        .map_err(|e| AppError::Msg(format!("chemin introuvable : {e}")))?;
    if !resolved.starts_with(&base) {
        return Err(AppError::Msg("chemin de pièce jointe hors workspace".into()));
    }
    Ok(resolved)
}

/// Upload un fichier attaché : écrit sur disque + met à jour le champ `attachment` JSON du record.
#[tauri::command]
pub async fn upload_attachment(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    record_id: String,
    file_name: String,
    data: Vec<u8>,
) -> Result<AttachmentMeta, AppError> {
    if data.is_empty() { return Err(AppError::Msg("fichier vide".into())); }
    if data.len() > 10 * 1024 * 1024 { return Err(AppError::Msg("fichier trop volumineux (max 10 Mo)".into())); }
    validate_id_component("db_id", &db_id)?;
    validate_id_component("table_id", &table_id)?;
    validate_id_component("record_id", &record_id)?;
    let safe = sanitize_filename(&file_name);
    let ext = std::path::Path::new(&safe).extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let pool = pool_for_db(&state, &db_id).await?;
    let fields = crate::db::repository::list_fields(&pool, &table_id).await?;
    let att_field = fields.iter().find(|f| f.field_type == "attachment").cloned();
    if att_field.is_none() {
        return Err(AppError::Msg("aucun champ attachment dans cette table".into()));
    }
    if let Some(f) = att_field.as_ref() {
        // max_size_mb peut être u64 ou f64 selon l'UI
        if let Ok(cfg) = serde_json::from_value::<serde_json::Value>(f.config.clone()) {
            if let Some(v) = cfg.get("max_size_mb") {
                let max_bytes = v.as_u64().map(|n| n * 1024 * 1024)
                    .or_else(|| v.as_f64().map(|n| (n * 1024.0 * 1024.0) as u64));
                if let Some(limit) = max_bytes {
                    if (data.len() as u64) > limit {
                        return Err(AppError::Msg(format!("fichier > {} octets autorisés pour ce champ", limit)));
                    }
                }
            }
        }
    }
    // vérifier existence du record AVANT d'écrire le fichier (évite fichiers orphelins)
    let tbl_check = crate::db::repository::quote_ident_public(&table_id);
    let exists: Option<String> = sqlx::query_scalar(&format!("SELECT _id FROM {} WHERE _id = ?", tbl_check))
        .bind(&record_id).fetch_optional(&pool).await.map_err(|e| AppError::Msg(e.to_string()))?;
    if exists.is_none() {
        return Err(AppError::Msg("record introuvable".into()));
    }
    let dir = {
        let guard = state.workspace.read().await;
        let ws = guard.as_ref().ok_or(AppError::NoWorkspace)?;
        ws.dir.clone()
    };
    let dir_path = workspace_attachments_dir(&dir, &db_id, &table_id, &record_id);
    tokio::fs::create_dir_all(&dir_path).await.map_err(|e| AppError::Msg(e.to_string()))?;
    // Vérifier que la racine de pièces jointes reste bien dans le workspace
    // (un parent symlinké pourrait sinon rediriger l'écriture ailleurs).
    let base = dir.canonicalize().map_err(|e| AppError::Msg(format!("workspace inaccessible : {e}")))?;
    let canon_root = dir_path.canonicalize().map_err(|e| AppError::Msg(format!("dossier attachments inaccessible : {e}")))?;
    if !canon_root.starts_with(&base) {
        return Err(AppError::Msg("dossier de pièces jointes hors workspace".into()));
    }
    // éviter collision : suffixe si existe
    let mut dest = canon_root.join(&safe);
    if dest.exists() {
        let stem = std::path::Path::new(&safe).file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext_part = if ext.is_empty() { String::new() } else { format!(".{ext}") };
        dest = dir_path.join(format!("{}_{}{}", stem, crate::utils::new_id("f"), ext_part));
    }
    tokio::fs::write(&dest, &data).await.map_err(|e| AppError::Msg(e.to_string()))?;

    let final_name = dest.file_name().and_then(|s| s.to_str()).unwrap_or(&safe).to_string();
    let mime = mime_guess::from_path(&safe).first_or_octet_stream().to_string();
    // URL relative au workspace pour ne pas exposer le chemin absolu (C:\Users\...)
    let rel_url = format!("attachments/{}/{}/{}/{}", db_id, table_id, record_id, final_name);
    let meta = AttachmentMeta { name: final_name.clone(), url: rel_url, size: data.len() as u64, mime };

    // Mettre à jour la cellule attachment JSON (read-modify-write)
    if let Some(att) = att_field.as_ref() {
        let tbl = crate::db::repository::quote_ident_public(&table_id);
        let col = crate::db::repository::quote_ident_public(&att.id);
        let row: Option<(String,)> = sqlx::query_as(&format!("SELECT {} FROM {} WHERE _id = ?", col, tbl))
            .bind(&record_id).fetch_optional(&pool).await.map_err(|e| AppError::Msg(e.to_string()))?;
        let mut arr: Vec<serde_json::Value> = if let Some((raw,)) = row {
            if raw.is_empty() { vec![] } else { serde_json::from_str::<serde_json::Value>(&raw).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default() }
        } else { vec![] };
        arr.push(serde_json::to_value(&meta).unwrap());
        let json_str = serde_json::to_string(&arr).unwrap();
        let upd = sqlx::query(&format!("UPDATE {} SET {} = ? WHERE _id = ?", tbl, col))
            .bind(&json_str).bind(&record_id).execute(&pool).await;
        if let Err(e) = upd {
            // MAJ DB échouée → le fichier vient d'être écrit, on le retire pour
            // ne pas laisser d'orphelin sur disque.
            let _ = tokio::fs::remove_file(&dest).await;
            return Err(AppError::Msg(format!("MAJ attachment DB échouée (fichier retiré): {e}")));
        }
    }

    Ok(meta)
}

#[tauri::command]
pub async fn list_attachments(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    record_id: String,
) -> Result<Vec<AttachmentMeta>, AppError> {
    validate_id_component("db_id", &db_id)?;
    validate_id_component("table_id", &table_id)?;
    validate_id_component("record_id", &record_id)?;
    let pool = pool_for_db(&state, &db_id).await?;
    let fields = crate::db::repository::list_fields(&pool, &table_id).await?;
    let att = fields.iter().find(|f| f.field_type == "attachment").ok_or_else(|| AppError::Msg("aucun champ attachment dans cette table".into()))?;
    let tbl = crate::db::repository::quote_ident_public(&table_id);
    let col = crate::db::repository::quote_ident_public(&att.id);
    let row: Option<String> = sqlx::query_scalar(&format!("SELECT {} FROM {} WHERE _id = ?", col, tbl))
        .bind(&record_id).fetch_optional(&pool).await.map_err(|e| AppError::Msg(e.to_string()))?.flatten();
    if let Some(s) = row {
        if s.trim().is_empty() { return Ok(vec![]); }
        let v: serde_json::Value = serde_json::from_str(&s).map_err(|e| AppError::Msg(e.to_string()))?;
        if let Some(arr) = v.as_array() {
            let metas: Vec<AttachmentMeta> = arr.iter().filter_map(|x| serde_json::from_value(x.clone()).ok()).collect();
            return Ok(metas);
        }
    }
    Ok(vec![])
}

/// Lit les octets d'une pièce jointe (helper testable, sans State).
pub(crate) async fn read_attachment_bytes(
    dir: &std::path::Path,
    db_id: &str,
    table_id: &str,
    record_id: &str,
    file_name: &str,
) -> Result<Vec<u8>, AppError> {
    let safe = sanitize_filename(file_name);
    let p = workspace_attachments_dir(dir, db_id, table_id, record_id).join(&safe);
    let resolved = canonical_inside_workspace(dir, &p)?;
    let data = tokio::fs::read(&resolved).await.map_err(|e| AppError::Msg(format!("fichier introuvable: {e}")))?;
    if data.len() > 10 * 1024 * 1024 {
        return Err(AppError::Msg("fichier trop volumineux (max 10 Mo)".into()));
    }
    Ok(data)
}

#[tauri::command]
pub async fn get_attachment_data(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    record_id: String,
    file_name: String,
) -> Result<String, AppError> {
    validate_id_component("db_id", &db_id)?;
    validate_id_component("table_id", &table_id)?;
    validate_id_component("record_id", &record_id)?;
    let dir = {
        let guard = state.workspace.read().await;
        let ws = guard.as_ref().ok_or(AppError::NoWorkspace)?;
        ws.dir.clone()
    };
    let data = read_attachment_bytes(&dir, &db_id, &table_id, &record_id, &file_name).await?;
    // Retourne en base64 pour affichage data: URL (évite asset:// et problèmes d'espace)
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    Ok(B64.encode(&data))
}

#[tauri::command]
pub async fn delete_attachment(
    state: State<'_, AppState>,
    db_id: String,
    table_id: String,
    record_id: String,
    file_name: String,
) -> Result<(), AppError> {
    validate_id_component("db_id", &db_id)?;
    validate_id_component("table_id", &table_id)?;
    validate_id_component("record_id", &record_id)?;
    let safe = sanitize_filename(&file_name);
    let dir = {
        let guard = state.workspace.read().await;
        let ws = guard.as_ref().ok_or(AppError::NoWorkspace)?;
        ws.dir.clone()
    };
    let pool = pool_for_db(&state, &db_id).await?;
    let fields = crate::db::repository::list_fields(&pool, &table_id).await?;
    let att = fields.iter().find(|f| f.field_type == "attachment").ok_or_else(|| AppError::Msg("aucun champ attachment".into()))?;

    // retirer du JSON
    let tbl = crate::db::repository::quote_ident_public(&table_id);
    let col = crate::db::repository::quote_ident_public(&att.id);
    let cur: Option<String> = sqlx::query_scalar(&format!("SELECT {} FROM {} WHERE _id = ?", col, tbl))
        .bind(&record_id).fetch_optional(&pool).await.map_err(|e| AppError::Msg(e.to_string()))?.flatten();
    if let Some(s) = cur {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(arr) = v.as_array() {
                let filtered: Vec<serde_json::Value> = arr.iter().filter(|x| x.get("name").and_then(|n| n.as_str()) != Some(&safe)).cloned().collect();
                let json_str = serde_json::to_string(&filtered).unwrap();
                sqlx::query(&format!("UPDATE {} SET {} = ? WHERE _id = ?", tbl, col))
                    .bind(&json_str).bind(&record_id).execute(&pool).await.map_err(|e| AppError::Msg(e.to_string()))?;
            }
        }
    }
    // supprimer fichier disque (best-effort, en restant dans le workspace)
    let p = workspace_attachments_dir(&dir, &db_id, &table_id, &record_id).join(&safe);
    if let Ok(resolved) = canonical_inside_workspace(&dir, &p) {
        let _ = tokio::fs::remove_file(&resolved).await;
    }
    Ok(())
}
