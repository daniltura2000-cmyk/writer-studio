//! Writer Studio core — «додаток, який читає рукопис».
//!
//! M0+M1 вертикальний слайс:
//! - [`store`]   — Project Store: markdown-файли + SQLite (WAL), FTS5-пошук
//! - [`indexer`] — витяг сутностей/згадок з тексту правилами (кирилиця+латиниця)
//! - [`drift`]   — детекція дрейфу імен (Kaldrin vs Kaldryn)
//! - [`bible`]   — живий Story Bible: картки сутностей, assertions, конфлікти
//! - [`presets`] — композабельні пресети жанрів (JSON-маніфести)

pub mod animation;
pub mod bible;
pub mod crosspost;
pub mod drift;
pub mod frontmatter;
pub mod glossary;
pub mod indexer;
pub mod inkbridge;
pub mod loc;
pub mod models;
pub mod narrative;
pub mod presets;
pub mod recap;
pub mod release;
pub mod royalty;
pub mod stats;
pub mod store;
pub mod translate;

pub use store::{Alert, EntityCard, IndexStats, ProjectStore};

/// Спільні хелпери для acceptance-тестів (інтеграційні тести імпортують
/// `skri_core::tests_common`; це публічний API для тестів, не для продакшну).
pub mod tests_common {
    use crate::ProjectStore;
    use std::path::PathBuf;

    /// Тимчасовий проєкт із валідним пресетом.
    pub fn tmp_project(name: &str) -> (PathBuf, ProjectStore) {
        let root = std::env::temp_dir().join(format!("skri-cmn-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let store = ProjectStore::init(&root, "novelist-uk", "uk").expect("init");
        (root, store)
    }
}

/// Тип помилки ядра.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Db(rusqlite::Error),
    Json(serde_json::Error),
    /// Проєкт не ініціалізований (немає `.skri/settings.json`)
    NotAProject(std::path::PathBuf),
    /// Інвалідний пресет/дані
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O: {e}"),
            Error::Db(e) => write!(f, "SQLite: {e}"),
            Error::Json(e) => write!(f, "JSON: {e}"),
            Error::NotAProject(p) => {
                write!(f, "це не проєкт Writer Studio (немає .skri у {})", p.display())
            }
            Error::Invalid(m) => write!(f, "некоректні дані: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Db(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
