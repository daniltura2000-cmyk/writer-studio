//! Project Store: markdown-файли на диску + SQLite-індекс.
//!
//! Залізне правило №1: файлова структура — єдине джерело правди.
//! Базу `.skri/index.db` можна стерти будь-якої миті — вона перебудовується з файлів.

use crate::frontmatter;
use crate::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::time::SystemTime;


/// Тип документа за розташуванням.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Manuscript,
    Bible,
    Notes,
}

impl DocKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocKind::Manuscript => "manuscript",
            DocKind::Bible => "bible",
            DocKind::Notes => "notes",
        }
    }
    fn from_dir(dir: &Path) -> DocKind {
        match dir.to_str().unwrap_or("").replace('\\', "/").as_str() {
            "bible" => DocKind::Bible,
            "notes" => DocKind::Notes,
            _ => DocKind::Manuscript,
        }
    }
}

/// Статистика реіндексації.
#[derive(Debug, Default)]
pub struct IndexStats {
    pub docs: usize,
    pub words: usize,
    pub reindexed: usize,
}

/// Проєкт Writer Studio.
pub struct ProjectStore {
    pub root: PathBuf,
    conn: Connection,
}

/// Алерт консистентності.
#[derive(Debug, Clone)]
pub struct Alert {
    pub id: i64,
    pub kind: String,
    pub entity_id: Option<i64>,
    pub payload: serde_json::Value,
    pub resolved: bool,
}

/// Картка сутності Story Bible.
#[derive(Debug, Clone)]
pub struct EntityCard {
    pub entity_id: i64,
    pub entity_type: String,
    pub canonical: String,
    pub aliases: Vec<String>,
    pub mention_total: usize,
    pub docs: Vec<(String, i64)>,
    pub assertions: Vec<(String, String, String)>,
}

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS docs (
  id INTEGER PRIMARY KEY,
  path TEXT UNIQUE,
  kind TEXT,
  title TEXT,
  lang TEXT,
  mtime INTEGER,
  word_count INTEGER
);
CREATE VIRTUAL TABLE IF NOT EXISTS docs_fts USING fts5(path UNINDEXED, title, body);
CREATE TABLE IF NOT EXISTS entities (
  id INTEGER PRIMARY KEY,
  type TEXT NOT NULL DEFAULT 'персонаж',
  canonical_form TEXT UNIQUE,
  aliases_json TEXT NOT NULL DEFAULT '[]',
  source TEXT NOT NULL DEFAULT 'auto',
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS mentions (
  id INTEGER PRIMARY KEY,
  entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
  doc_id INTEGER NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
  char_start INTEGER,
  char_end INTEGER,
  surface_form TEXT,
  sentence TEXT
);
CREATE INDEX IF NOT EXISTS idx_mentions_doc ON mentions(doc_id);
CREATE INDEX IF NOT EXISTS idx_mentions_entity ON mentions(entity_id);
CREATE TABLE IF NOT EXISTS assertions (
  id INTEGER PRIMARY KEY,
  entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
  attribute TEXT NOT NULL,
  value TEXT NOT NULL,
  doc_id INTEGER REFERENCES docs(id) ON DELETE SET NULL,
  status TEXT NOT NULL DEFAULT 'canonical'
);
CREATE TABLE IF NOT EXISTS alerts (
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL,
  entity_id INTEGER,
  payload_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  resolved INTEGER NOT NULL DEFAULT 0
);
"#;

impl ProjectStore {
    /// Ініціалізувати нову папку проєкту з пресетом.
    pub fn init(root: &Path, preset: &str, language: &str) -> Result<Self> {
        std::fs::create_dir_all(root.join(".skri"))?;
        std::fs::create_dir_all(root.join("manuscript"))?;
        std::fs::create_dir_all(root.join("bible"))?;
        std::fs::create_dir_all(root.join("notes"))?;
        let settings = serde_json::json!({ "preset": preset, "language": language });
        std::fs::write(
            root.join(".skri").join("settings.json"),
            serde_json::to_string_pretty(&settings)?,
        )?;
        Self::open(root)
    }

    /// Відкрити існуючий проєкт (перевіряє `.skri/settings.json`).
    pub fn open(root: &Path) -> Result<Self> {
        if !root.join(".skri").join("settings.json").exists() {
            return Err(Error::NotAProject(root.to_path_buf()));
        }
        let conn = Connection::open(root.join(".skri").join("index.db"))?;
        conn.execute_batch(SCHEMA)?;
        Ok(ProjectStore {
            root: root.to_path_buf(),
            conn,
        })
    }

    pub fn settings(&self) -> Result<serde_json::Value> {
        let raw = std::fs::read_to_string(self.root.join(".skri").join("settings.json"))?;
        Ok(serde_json::from_str(&raw)?)
    }

    /// Повна реіндексація документів з диска (плюс FTS).
    /// Ідемпотентна: перечитує лише файли з новим mtime.
    pub fn reindex(&self) -> Result<IndexStats> {
        let mut stats = IndexStats::default();
        for entry in walkdir::WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| e.file_name() != ".skri" && e.file_name() != ".git")
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let rel = path
                .strip_prefix(&self.root)
                .map_err(|_| Error::Invalid("шлях поза проєктом".into()))?
                .to_string_lossy()
                .replace('\\', "/");
            let rel_dir = Path::new(&rel)
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let kind = DocKind::from_dir(Path::new(&rel_dir));
            let meta = entry
                .metadata()
                .map_err(|e| Error::Io(e.into_io_error().unwrap_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::Other, "walkdir")
                })))?;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|m: SystemTime| m.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let existing: Option<(i64, i64)> = self
                .conn
                .query_row(
                    "SELECT id, mtime FROM docs WHERE path = ?1",
                    params![rel],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            stats.docs += 1;

            if let Some((id, old_mtime)) = existing {
                if old_mtime == mtime as i64 {
                    let wc: i64 = self
                        .conn
                        .query_row("SELECT word_count FROM docs WHERE id=?1", params![id], |r| {
                            r.get(0)
                        })?;
                    stats.words += wc.max(0) as usize;
                    continue; // без змін
                }
                self.index_file(id, path, &rel, kind, mtime, &mut stats)?;
            } else {
                self.conn
                    .execute("INSERT INTO docs(path, kind) VALUES (?1, ?2)", params![rel, kind.as_str()])?;
                let id = self.conn.last_insert_rowid();
                self.index_file(id, path, &rel, kind, mtime, &mut stats)?;
            }
        }
        Ok(stats)
    }

    fn index_file(
        &self,
        doc_id: i64,
        path: &Path,
        rel: &str,
        kind: DocKind,
        mtime: u64,
        stats: &mut IndexStats,
    ) -> Result<()> {
        let raw = std::fs::read_to_string(path)?;
        let parsed = frontmatter::parse(&raw);
        let title = parsed
            .meta
            .get("title")
            .cloned()
            .unwrap_or_else(|| {
                rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".md").to_string()
            });
        let lang = parsed.meta.get("lang").cloned().unwrap_or_default();
        let word_count = crate::indexer::count_words(&parsed.body);
        stats.words += word_count;
        stats.reindexed += 1;

        self.conn.execute(
            "UPDATE docs SET kind=?2, title=?3, lang=?4, mtime=?5, word_count=?6 WHERE id=?1",
            params![doc_id, kind.as_str(), title, lang, mtime as i64, word_count as i64],
        )?;
        // FTS: рядок перебудовується повністю
        self.conn
            .execute("DELETE FROM docs_fts WHERE path = ?1", params![rel])?;
        self.conn.execute(
            "INSERT INTO docs_fts(path, title, body) VALUES (?1, ?2, ?3)",
            params![rel, title, parsed.body],
        )?;
        Ok(())
    }

    /// id документа за відносним шляхом.
    pub fn doc_id(&self, rel: &str) -> Option<i64> {
        self.conn
            .query_row("SELECT id FROM docs WHERE path=?1", params![rel], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    /// Пошук FTS: (path, title, сніпет).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, title, snippet(docs_fts, 2, '[', ']', '…', 12)
             FROM docs_fts WHERE docs_fts MATCH ?1 LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![query, limit as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Зберегти вибір моделі для задачі (делегат до `models::set_model`).
    pub fn set_model(&self, task: &str, model: &str, auto: bool) -> Result<()> {
        crate::models::set_model(&self.root, task, model, auto)
    }

    /// Поточне призначення моделі: Some((model, auto)).
    pub fn model_for(&self, task: &str) -> Result<Option<(String, bool)>> {
        crate::models::model_for(&self.root, task)
    }
}
