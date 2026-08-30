#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("aucun workspace ouvert")]
    NoWorkspace,
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(String),
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let msg = match self {
            AppError::NoWorkspace | AppError::Msg(_) => self.to_string(),
            // Ne jamais exposer path SQLite / schéma / chemins de fichiers au
            // front : on logge le détail côté serveur, message générique côté UI.
            AppError::Db(e) => {
                eprintln!("[sebtable] erreur DB : {e}");
                "opération échouée (base de données)".into()
            }
            AppError::Io(e) => {
                eprintln!("[sebtable] erreur I/O : {e}");
                "opération échouée (système de fichiers)".into()
            }
            AppError::Serde(e) => {
                eprintln!("[sebtable] erreur de sérialisation : {e}");
                "opération échouée (données invalides)".into()
            }
        };
        serializer.serialize_str(&msg)
    }
}
