use thiserror::Error;

#[derive(Error, Debug)]
pub enum LauncherError {
    #[error("G/Ç Hatası: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serileştirme hatası: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Güvenlik ihlali: {0}")]
    Security(String),

    #[error("Veritabanı hatası: {0}")]
    Database(String),

    #[error("Doğrulama hatası: {0}")]
    Validation(String),

    #[error("Bulunamadı: {0}")]
    NotFound(String),

    #[error("Ağ hatası: {0}")]
    Network(String),

    #[error("Bilinmeyen hata: {0}")]
    Unknown(String),
}

pub type LauncherResult<T> = Result<T, LauncherError>;

