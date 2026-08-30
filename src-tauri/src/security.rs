use std::path::Path;

use keyring::Entry;

use crate::error::AppError;

/// Service keyring utilisé pour la clé OpenAI (stockée dans le keychain OS,
/// jamais en clair dans workspace.json).
const SERVICE: &str = "sebtable";

fn entry_for(dir: &Path) -> Result<Entry, AppError> {
    let account = std::fs::canonicalize(dir)
        .unwrap_or_else(|_| dir.to_path_buf())
        .to_string_lossy()
        .to_string();
    Entry::new(SERVICE, &account)
        .map_err(|e| AppError::Msg(format!("Keychain inaccessible : {e}")))
}

pub fn store_api_key(dir: &Path, key: &str) -> Result<(), AppError> {
    let entry = entry_for(dir)?;
    entry
        .set_password(key)
        .map_err(|e| AppError::Msg(format!("Impossible de stocker la clé dans le keychain : {e}")))
}

pub fn load_api_key(dir: &Path) -> Result<Option<String>, AppError> {
    let entry = entry_for(dir)?;
    match entry.get_password() {
        Ok(k) if !k.is_empty() => Ok(Some(k)),
        Ok(_) => Ok(None),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::Msg(format!("Keychain illisible : {e}"))),
    }
}

pub fn delete_api_key(dir: &Path) -> Result<(), AppError> {
    let entry = entry_for(dir)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Msg(format!(
            "Impossible de supprimer la clé du keychain : {e}"
        ))),
    }
}
