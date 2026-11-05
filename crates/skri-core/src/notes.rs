//! Нотатник (специфікація §5.10, Obsidian-сегмент): вікі-лінки/беклінки,
//! daily notes, сироти й stale-нотатки.
//!
//! - **Backlinks**: скан `notes/*.md` на `[[вікі-лінки]]` (укр. + латиниця,
//!   форма `[[target]]` і `[[target|alias]]`; суфікси `#розділ`/`^блок`
//!   відрізаються) з байтовими offset у тілі. Таблиця `links(from_doc,
//!   to_title, target_doc, char_start, char_end)` перебудовується з файлів
//!   ідемпотентно (залізне правило №1). Резолв — по імені файла без
//!   розширення, без регістру, по ВСІХ документах проєкту (як vault).
//!   Broken = лінк без резолву; сирота = нотатка без вхідних лінків.
//! - **Daily note**: `notes/YYYY-MM-DD.md` — створюється з шаблоном
//!   (з пресету проєкту `templates.daily-note` або вбудованого) з
//!   auto-insert посилань на вчорашній/завтрашній день; існуючий файл
//!   НЕ перезаписується.
//! - **Stale**: нотатки без змін N днів по mtime (з docs — похідного індексу).
//!
//! Offsets — байтові, як у згадках Index Engine (indexer::tokenize):
//! брекети `[[`/`]]` ASCII, тому зріз `body[start..end]` завжди на
//! char boundary навіть на кирилиці.

use crate::indexer;
use crate::presets;
use crate::store::{DocKind, ProjectStore};
use crate::{Error, Result};
use rusqlite::params;
use std::collections::BTreeMap;

const DAY: i64 = 86_400;

/// Стандартний вікі-лінк Obsidian: `[[ціль]]` / `[[ціль|показ]]`.
#[derive(Debug, Clone, PartialEq)]
pub struct WikiLink {
    pub to_title: String,
    /// байтовий offset `[` у `[[` (в тілі документа)
    pub char_start: usize,
    /// байтовий offset одразу після `]]`
    pub char_end: usize,
}

/// Розібрати `[[вікі-лінки]]` у тексті: укр. + латиниця, з байтовими offsets.
/// Підтримується `[[target|alias]]`; `#заголовок` і `^блок` відділяються.
pub fn parse_wiki_links(body: &str) -> Vec<WikiLink> {
    let bytes = body.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(rel) = body[i + 2..].find("]]") {
                let inner_end = i + 2 + rel;
                let inner = &body[i + 2..inner_end];
                let mut target = inner.split('|').next().unwrap_or("").trim();
                if let Some(cut) = target.find(['#', '^']) {
                    target = target[..cut].trim();
                }
                if !target.is_empty() {
                    out.push(WikiLink {
                        to_title: target.to_string(),
                        char_start: i,
                        char_end: inner_end + 2,
                    });
                }
                i = inner_end + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS links (
  id INTEGER PRIMARY KEY,
  from_doc TEXT NOT NULL,
  to_title TEXT NOT NULL,
  target_doc TEXT,
  char_start INTEGER NOT NULL,
  char_end INTEGER NOT NULL
);
"#;

/// Один рядок графа лінків.
#[derive(Debug, Clone)]
pub struct LinkRow {
    pub from_doc: String,
    pub to_title: String,
    /// резолвнутий документ (шлях відносно кореня) або None = broken
    pub target_doc: Option<String>,
    pub char_start: i64,
    pub char_end: i64,
}

/// Звіт графа лінків.
#[derive(Debug, Default)]
pub struct LinksReport {
    pub total: usize,
    pub resolved: usize,
    /// лінки без резолву (відсортовані за from_doc)
    pub broken: Vec<LinkRow>,
    /// вхідних лінків на кожен резолвнутий документ
    pub inbound: BTreeMap<String, usize>,
    /// нотатки без жодного вхідного лінку (сироти), сорт за шляхом
    pub orphans: Vec<String>,
}

fn stem(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".md")
        .to_string()
}

/// Перебудувати таблицю `links` з notes/*.md (ідемпотентно: DELETE + INSERT).
/// Повертає кількість знайдених лінків. Резолв — по імені файла без
/// розширення (без регістру) серед УСІХ документів проєкту.
pub fn rebuild_links(store: &ProjectStore) -> Result<usize> {
    store.conn().execute_batch(SCHEMA)?;
    store.conn().execute("DELETE FROM links", [])?;

    let all_paths: Vec<String> = {
        let mut stmt = store.conn().prepare("SELECT path FROM docs ORDER BY path")?;
        let rows = stmt.query_map([], |r| r.get(0))?.filter_map(|r| r.ok()).collect();
        rows
    };
    let stems: BTreeMap<String, String> = all_paths
        .iter()
        .map(|p| (stem(p).to_lowercase(), p.clone()))
        .collect();

    let notes_docs: Vec<String> = {
        let mut stmt = store
            .conn()
            .prepare("SELECT path FROM docs WHERE kind = ?1 ORDER BY path")?;
        let rows = stmt
            .query_map(params![DocKind::Notes.as_str()], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    let mut n = 0usize;
    for rel in notes_docs {
        let body = indexer::read_body(store, &rel)?;
        for l in parse_wiki_links(&body) {
            let target = stems.get(&l.to_title.to_lowercase()).cloned();
            store.conn().execute(
                "INSERT INTO links(from_doc, to_title, target_doc, char_start, char_end)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![rel, l.to_title, target, l.char_start as i64, l.char_end as i64],
            )?;
            n += 1;
        }
    }
    Ok(n)
}

/// Усі рядки графа лінків (після rebuild_links).
pub fn links(store: &ProjectStore) -> Result<Vec<LinkRow>> {
    ensure_schema(store)?;
    let mut stmt = store.conn().prepare(
        "SELECT from_doc, to_title, target_doc, char_start, char_end
         FROM links ORDER BY from_doc, char_start",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(LinkRow {
                from_doc: r.get(0)?,
                to_title: r.get(1)?,
                target_doc: r.get(2)?,
                char_start: r.get(3)?,
                char_end: r.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

fn ensure_schema(store: &ProjectStore) -> Result<()> {
    store.conn().execute_batch(SCHEMA)?;
    Ok(())
}

/// Звіт графа: resolved/broken, вхідні лінки, сироти.
pub fn report(store: &ProjectStore) -> Result<LinksReport> {
    ensure_schema(store)?;
    let rows = links(store)?;
    let mut rep = LinksReport {
        total: rows.len(),
        resolved: rows.iter().filter(|r| r.target_doc.is_some()).count(),
        ..Default::default()
    };
    for r in &rows {
        match &r.target_doc {
            Some(t) => *rep.inbound.entry(t.clone()).or_insert(0) += 1,
            None => rep.broken.push(r.clone()),
        }
    }
    // сироти: нотатки проєкту без жодного вхідного резолвнутого лінку
    let mut stmt = store
        .conn()
        .prepare("SELECT path FROM docs WHERE kind = ?1 ORDER BY path")?;
    let notes_docs: Vec<String> = stmt
        .query_map(params![DocKind::Notes.as_str()], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    rep.orphans = notes_docs
        .into_iter()
        .filter(|p| !rep.inbound.contains_key(p))
        .collect();
    Ok(rep)
}

/// Сироти: нотатки без вхідних лінків.
pub fn orphans(store: &ProjectStore) -> Result<Vec<String>> {
    Ok(report(store)?.orphans)
}

/// Чиста функція stale: (шлях, mtime) → (шлях, вік_у_днях) для тих, хто
/// не змінювався `days` днів і більше (найстаріші першими).
pub fn stale_with(mtimes: &[(String, i64)], now: i64, days: i64) -> Vec<(String, i64)> {
    let mut out: Vec<(String, i64)> = mtimes
        .iter()
        .map(|(p, m)| (p.clone(), (now - m).max(0) / DAY))
        .filter(|(_, age)| *age >= days)
        .collect();
    out.sort_by_key(|(_, age)| std::cmp::Reverse(*age));
    out
}

/// Нотатки без змін `days` днів і більше (по mtime з docs).
pub fn stale(store: &ProjectStore, now: i64, days: i64) -> Result<Vec<(String, i64)>> {
    ensure_schema(store)?;
    let mut stmt = store
        .conn()
        .prepare("SELECT path, mtime FROM docs WHERE kind = ?1")?;
    let rows: Vec<(String, i64)> = stmt
        .query_map(params![DocKind::Notes.as_str()], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(stale_with(&rows, now, days))
}

/// Вбудований шаблон daily note (коли пресет проєкту не має свого).
pub const DEFAULT_DAILY_TEMPLATE: &str = "---\ntitle: Щоденник {{date}}\ndate: {{date}}\n---\n\n# Щоденник — {{date}}\n\n← [[{{yesterday}}]] · [[{{tomorrow}}]] →\n\n## День\n\n- \n";

/// Вбудований шаблон тематичної нотатки.
pub const DEFAULT_TOPIC_TEMPLATE: &str = "---\ntitle: {{title}}\ndate: {{date}}\n---\n\n# {{title}}\n\n## Суть (одна думка)\n\n## Джерела\n\n";

fn template_from_preset(store: &ProjectStore, key: &str) -> Option<String> {
    let name = store
        .settings()
        .ok()?
        .get("preset")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        return None;
    }
    presets::builtin(&name)
        .and_then(|p| p.templates.get(key).cloned())
}

fn fill_template(tmpl: &str, title: &str, date: &str, yesterday: &str, tomorrow: &str) -> String {
    tmpl.replace("{{title}}", title)
        .replace("{{date}}", date)
        .replace("{{yesterday}}", yesterday)
        .replace("{{tomorrow}}", tomorrow)
}

/// Щоденник: `notes/YYYY-MM-DD.md` з шаблону пресету (або вбудованого),
/// auto-insert посилань на вчорашній і завтрашній day-notes.
/// Існуючий файл НЕ перезаписується (created = false).
pub fn daily(store: &ProjectStore, now: i64) -> Result<DailyNote> {
    let today = crate::release::fmt_date(now);
    let yesterday = crate::release::fmt_date(now - DAY);
    let tomorrow = crate::release::fmt_date(now + DAY);
    let rel = format!("notes/{today}.md");
    let abs = store.root.join(&rel);
    if abs.exists() {
        return Ok(DailyNote {
            path: rel,
            created: false,
            yesterday,
            tomorrow,
        });
    }
    std::fs::create_dir_all(store.root.join("notes"))?;
    let tmpl = template_from_preset(store, "daily-note").unwrap_or_else(|| DEFAULT_DAILY_TEMPLATE.to_string());
    let body = fill_template(&tmpl, &format!("Щоденник {today}"), &today, &yesterday, &tomorrow);
    std::fs::write(&abs, body)?;
    Ok(DailyNote {
        path: rel,
        created: true,
        yesterday,
        tomorrow,
    })
}

/// Нова тематична нотатка `notes/<назва>.md` з шаблону topic-note.
/// Існуючий файл НЕ перезаписується.
pub fn new_note(store: &ProjectStore, title: &str, now: i64) -> Result<String> {
    let title = title.trim();
    if title.is_empty() {
        return Err(Error::Invalid("порожня назва нотатки".into()));
    }
    // не даємо назві виїхати з notes/ або зламати шлях
    if title.contains('/') || title.contains('\\') || title.contains("..") {
        return Err(Error::Invalid(format!("некоректна назва нотатки «{title}»")));
    }
    let rel = format!("notes/{title}.md");
    let abs = store.root.join(&rel);
    if abs.exists() {
        return Ok(rel);
    }
    std::fs::create_dir_all(store.root.join("notes"))?;
    let tmpl = template_from_preset(store, "topic-note").unwrap_or_else(|| DEFAULT_TOPIC_TEMPLATE.to_string());
    let date = crate::release::fmt_date(now);
    let body = fill_template(&tmpl, title, &date, &date, &date);
    std::fs::write(&abs, body)?;
    Ok(rel)
}

/// Результат створення daily note.
#[derive(Debug, Clone)]
pub struct DailyNote {
    /// шлях відносно кореня проєкту
    pub path: String,
    /// false = файл уже існував (не перезаписано)
    pub created: bool,
    pub yesterday: String,
    pub tomorrow: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wiki_links_offsets_and_forms() {
        let body = "Див. [[Київ]] і [[Kyiv Post|джерело]], також [[Київ#Історія]] та [[зламана";
        let ls = parse_wiki_links(body);
        assert_eq!(ls.len(), 3, "незакритий лінк ігнорується");
        assert_eq!(ls[0].to_title, "Київ");
        assert_eq!(&body[ls[0].char_start..ls[0].char_end], "[[Київ]]");
        assert_eq!(ls[1].to_title, "Kyiv Post", "alias відділяється, ціль — до |");
        assert_eq!(&body[ls[1].char_start..ls[1].char_end], "[[Kyiv Post|джерело]]");
        assert_eq!(ls[2].to_title, "Київ", "#heading відрізається");
        assert!(ls[0].char_start < ls[1].char_start && ls[1].char_start < ls[2].char_start);
    }
}
