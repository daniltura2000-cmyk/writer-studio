//! Серійний глосарій (M2): імена, вигадані терміни, правила транслітерації —
//! зберігаються між томами серії (series_wide).
//!
//! Лікує досліджені болі:
//! - *"Alice becoming Elise by chapter four"* — примусова консистентність
//! - *"the constant need to reinsert glossaries"* — глосарій живе в проєкті,
//!   а не вставляється в кожен промпт вручну

use crate::store::ProjectStore;
use crate::{Error, Result};
use rusqlite::{params, OptionalExtension};

/// Запис глосарію.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GlossaryEntry {
    pub id: i64,
    /// термін у джерельній мові (канонічна форма)
    pub term: String,
    /// мова джерела (напр. uk)
    pub source_lang: String,
    /// мова перекладу (напр. en)
    pub target_lang: String,
    /// канонічний переклад (порожній = не вирішено)
    pub canonical_translation: String,
    /// правило: transliterate | translate | keep
    pub rule: String,
    pub notes: String,
    /// діє для всієї серії (переноситься між томами)
    pub series_wide: bool,
}

/// Додати/оновити запис глосарію (upsert за term+langs).
pub fn upsert(store: &ProjectStore, e: &GlossaryEntry) -> Result<i64> {
    if e.term.trim().is_empty() {
        return Err(Error::Invalid("порожній термін".into()));
    }
    if e.source_lang.is_empty() || e.target_lang.is_empty() {
        return Err(Error::Invalid("вкажіть source_lang і target_lang".into()));
    }
    let conn = store.conn();
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM glossary WHERE lower(term)=lower(?1)
             AND source_lang=?2 AND target_lang=?3",
            params![e.term, e.source_lang, e.target_lang],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        conn.execute(
            "UPDATE glossary SET canonical_translation=?2, rule=?3, notes=?4, series_wide=?5
             WHERE id=?1",
            params![id, e.canonical_translation, e.rule, e.notes, e.series_wide],
        )?;
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO glossary(term, source_lang, target_lang, canonical_translation, rule, notes, series_wide)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            e.term, e.source_lang, e.target_lang,
            e.canonical_translation, e.rule, e.notes, e.series_wide
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Усі записи пари мов.
pub fn list(store: &ProjectStore, source_lang: &str, target_lang: &str) -> Result<Vec<GlossaryEntry>> {
    let conn = store.conn();
    let mut stmt = conn.prepare(
        "SELECT id, term, source_lang, target_lang, canonical_translation, rule, notes, series_wide
         FROM glossary WHERE source_lang=?1 AND target_lang=?2 ORDER BY term",
    )?;
    let rows = stmt
        .query_map(params![source_lang, target_lang], |r| {
            Ok(GlossaryEntry {
                id: r.get(0)?,
                term: r.get(1)?,
                source_lang: r.get(2)?,
                target_lang: r.get(3)?,
                canonical_translation: r.get(4)?,
                rule: r.get(5)?,
                notes: r.get(6)?,
                series_wide: r.get::<_, i64>(7)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Запис за терміном.
pub fn find(store: &ProjectStore, term: &str, source_lang: &str, target_lang: &str) -> Result<Option<GlossaryEntry>> {
    let all = list(store, source_lang, target_lang)?;
    Ok(all.into_iter().find(|e| e.term.eq_ignore_ascii_case(term)))
}

/// Серіальний експорт: усі series_wide записи (для наступного тому).
pub fn series_export(store: &ProjectStore) -> Result<Vec<GlossaryEntry>> {
    let conn = store.conn();
    let mut stmt = conn.prepare(
        "SELECT id, term, source_lang, target_lang, canonical_translation, rule, notes, series_wide
         FROM glossary WHERE series_wide=1 ORDER BY source_lang, target_lang, term",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(GlossaryEntry {
                id: r.get(0)?,
                term: r.get(1)?,
                source_lang: r.get(2)?,
                target_lang: r.get(3)?,
                canonical_translation: r.get(4)?,
                rule: r.get(5)?,
                notes: r.get(6)?,
                series_wide: r.get::<_, i64>(7)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Пост-валідація перекладеного тексту проти глосарію.
///
/// Перевіряє, що кожен термін, який ЗУСТРІЧАЄТЬСЯ в оригіналі, у перекладі
/// представлений канонічним перекладом (або, для rule=keep, оригіналом).
/// Повертає список порушень: (term, очікувано, проблема).
pub fn validate_translation(
    store: &ProjectStore,
    source_text: &str,
    translated_text: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<Vec<GlossaryViolation>> {
    let entries = list(store, source_lang, target_lang)?;
    let mut out = Vec::new();
    for e in entries {
        if !source_text.to_lowercase().contains(&e.term.to_lowercase()) {
            continue; // терміна немає в цьому фрагменті
        }
        let present = match e.rule.as_str() {
            "keep" => translated_text.to_lowercase().contains(&e.term.to_lowercase()),
            _ => {
                if e.canonical_translation.is_empty() {
                    // переклад не вирішено — попередження окремим типом
                    out.push(GlossaryViolation {
                        term: e.term.clone(),
                        expected: String::new(),
                        problem: "переклад терміна не визначено в глосарії".into(),
                        unresolved: true,
                    });
                    continue;
                }
                translated_text
                    .to_lowercase()
                    .contains(&e.canonical_translation.to_lowercase())
            }
        };
        if !present {
            out.push(GlossaryViolation {
                term: e.term.clone(),
                expected: if e.rule == "keep" {
                    e.term.clone()
                } else {
                    e.canonical_translation.clone()
                },
                problem: format!(
                    "термін «{}» є в оригіналі, але канонічний варіант «{}» не знайдений у перекладі",
                    e.term,
                    if e.rule == "keep" { &e.term } else { &e.canonical_translation }
                ),
                unresolved: false,
            });
        }
    }
    Ok(out)
}

/// Порушення глосарію.
#[derive(Debug, Clone)]
pub struct GlossaryViolation {
    pub term: String,
    pub expected: String,
    pub problem: String,
    /// true = переклад терміна не визначено (потрібне рішення автора)
    pub unresolved: bool,
}

/// Глосарій як текст для AI-промпту (рекомендації дослідження: вставляти
/// глосарій у кожен запит — тепер це генерується автоматично).
pub fn prompt_block(store: &ProjectStore, source_lang: &str, target_lang: &str) -> Result<String> {
    let entries = list(store, source_lang, target_lang)?;
    if entries.is_empty() {
        return Ok(String::new());
    }
    let mut lines = vec!["ГЛОСАРІЙ СЕРІЇ (дотримуватися строго):".to_string()];
    for e in entries {
        let rule_note = match e.rule.as_str() {
            "keep" => "не перекладати",
            "transliterate" => "транслітерувати",
            _ => "перекладати",
        };
        let target = if e.canonical_translation.is_empty() {
            "<НЕВИЗНАЧЕНО>"
        } else {
            &e.canonical_translation
        };
        lines.push(format!(
            "- {} → {} ({rule_note}){}",
            e.term,
            target,
            if e.notes.is_empty() { String::new() } else { format!(" [{}]", e.notes) }
        ));
    }
    Ok(lines.join("\n"))
}
