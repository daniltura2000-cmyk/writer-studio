//! Crosspost Orchestrator — журнал публікацій і пакувальник (§5.6).
//!
//! Один канонічний markdown глави → пакунки для платформ (RoyalRoad,
//! ScribbleHub, Wattpad, AO3, Patreon). Атомарний журнал у SQLite:
//! яка версія (version_hash) глави на якій платформі в якому статусі.
//! Правка канону після публікації → алерт `kind='crosspost-stale'`
//! («главу змінено після публікації на X — потрібна перевідправка»).
//!
//! ## Чесно про відправку
//! У платформ немає відкритих публічних API для вебновел:
//! - RoyalRoad/ScribbleHub/Wattpad — лише неофіційні скрейпери (ломаються,
//!   ризик бану акаунта);
//! - AO3 — офіційно закритий API для постингу (лімітовані запити на читання);
//! - Patreon — API v2 існує, але дає create post лише за OAuth-токеном
//!   кампанії.
//!
//! Тому **реальна відправка — ручна** (автор копіює `package.json`/HTML з
//! `.skri/outbound/` у редактор платформи), а статус у журналі оновлюється
//! командою `skri crosspost mark ... posted`. Майбутні адаптери з OAuth
//! (Patreon API v2 і т.д.) зможуть перевикористати той самий журнал і
//! пакунки — формат навмисно платформо-нейтральний.
//!
//! Залізне правило №1 дотримане: пакунки — файли на диску, журнал можна
//! стерти разом з `.skri/` і перезібрати командами pack/mark.

use crate::frontmatter;
use crate::inkbridge::sha256_hex;
use crate::store::ProjectStore;
use crate::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;

/// Платформи, для яких будується пакунок.
pub const PLATFORMS: [&str; 5] = ["royalroad", "scribblehub", "wattpad", "ao3", "patreon"];

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS crosspost_log (
  doc_path TEXT NOT NULL,
  platform TEXT NOT NULL,
  version_hash TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  packed_at INTEGER,
  published_at INTEGER,
  PRIMARY KEY (doc_path, platform)
);";

fn ensure_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

/// Пакунок для однієї платформи.
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    pub platform: String,
    pub title: String,
    pub html_body: String,
    pub notes: String,
    /// JSON-масив тегів (з frontmatter `tags`, через кому)
    pub tags_json: String,
    /// хеш канонічного тіла (version_hash)
    pub version_hash: String,
}

/// Рядок журналу публікацій.
#[derive(Debug, Clone, PartialEq)]
pub struct LogRow {
    pub doc_path: String,
    pub platform: String,
    pub version_hash: String,
    pub status: String,
    pub packed_at: Option<i64>,
    pub published_at: Option<i64>,
}

/// Звіт пакування.
#[derive(Debug)]
pub struct PackReport {
    pub doc_path: String,
    pub chapter: String,
    pub version_hash: String,
    /// платформи, для яких пакунок записано
    pub platforms: Vec<String>,
    /// платформи, пропущені бо status='posted' (версія вже опублікована)
    pub skipped_posted: Vec<String>,
    pub dir: PathBuf,
}

// --- markdown → html (простий, без залежностей) ---

/// Екранувати HTML-символи тексту.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

/// `**bold**` → `<strong>` (попарні `**`; непарний хвіст закривається).
fn bold_pass(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut open = true;
    let mut rest = s;
    while let Some(i) = rest.find("**") {
        out.push_str(&rest[..i]);
        out.push_str(if open { "<strong>" } else { "</strong>" });
        open = !open;
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    if !open {
        out.push_str("</strong>");
    }
    out
}

/// `*italic*` → `<em>` (одинарні `*`; викликається ПІСЛЯ bold, тож `**`
/// уже з'їдений і не ламає підрахунок).
fn italic_pass(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut open = true;
    let mut rest = s;
    while let Some(i) = rest.find('*') {
        out.push_str(&rest[..i]);
        out.push_str(if open { "<em>" } else { "</em>" });
        open = !open;
        rest = &rest[i + 1..];
    }
    out.push_str(rest);
    if !open {
        out.push_str("</em>");
    }
    out
}

/// Інлайн-розмітка одного рядка: escape → **bold** → *italic*.
pub fn inline_html(line: &str) -> String {
    italic_pass(&bold_pass(&escape_html(line)))
}

/// Прості markdown → html: параграфи (розділені порожнім рядком),
/// `---` → `<hr>`, `**bold**`, `*italic*`. Заголовки/таблиці/посилання
/// свідомо не підтримуються (недостатньо для пакунка — документуємо).
pub fn markdown_to_html(md: &str) -> String {
    let mut out = String::with_capacity(md.len() + 64);
    let mut para: Vec<String> = Vec::new();
    for line in md.lines() {
        let t = line.trim();
        if t.is_empty() {
            if !para.is_empty() {
                out.push_str("<p>");
                out.push_str(&para.join("\n"));
                out.push_str("</p>\n");
                para.clear();
            }
            continue;
        }
        if t == "---" {
            if !para.is_empty() {
                out.push_str("<p>");
                out.push_str(&para.join("\n"));
                out.push_str("</p>\n");
                para.clear();
            }
            out.push_str("<hr>\n");
            continue;
        }
        para.push(inline_html(t));
    }
    if !para.is_empty() {
        out.push_str("<p>");
        out.push_str(&para.join("\n"));
        out.push_str("</p>\n");
    }
    out
}

// --- пакування ---

/// Теги з frontmatter `tags` (через кому) → JSON-масив.
fn tags_json_of(meta: &std::collections::BTreeMap<String, String>) -> String {
    let raw = meta.get("tags").cloned().unwrap_or_default();
    let tags: Vec<String> = raw
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into())
}

/// Нотатки для читачів: frontmatter `notes` або `author_note`.
fn notes_of(meta: &std::collections::BTreeMap<String, String>) -> String {
    meta.get("notes")
        .or_else(|| meta.get("author_note"))
        .cloned()
        .unwrap_or_default()
}

/// Хеш канонічної версії глави (тіло без frontmatter).
pub fn version_hash_of(raw: &str) -> String {
    sha256_hex(frontmatter::parse(raw).body.as_bytes())
}

/// Ім'я папки глави (стем документа).
pub fn chapter_name(doc_rel: &str) -> String {
    let stem = doc_rel.replace('\\', "/");
    let stem = stem.rsplit('/').next().unwrap_or(&stem);
    stem.trim_end_matches(".md").to_string()
}

/// Запакувати канонічний markdown глави в пакунки всіх платформ:
/// `.skri/outbound/<глава>/<платформа>/package.json`.
/// Журнал: для кожної платформи — рядок pending (posted-рядки не чіпаються).
pub fn pack(store: &ProjectStore, doc_rel: &str) -> Result<PackReport> {
    let doc_rel = doc_rel.replace('\\', "/");
    let raw = std::fs::read_to_string(store.root.join(&doc_rel))?;
    let parsed = frontmatter::parse(&raw);
    let title = parsed
        .meta
        .get("title")
        .cloned()
        .unwrap_or_else(|| chapter_name(&doc_rel));
    let version_hash = sha256_hex(parsed.body.as_bytes());
    let chapter = chapter_name(&doc_rel);
    let tags_json = tags_json_of(&parsed.meta);
    let notes = notes_of(&parsed.meta);
    let html_body = markdown_to_html(&parsed.body);

    let dir = store.root.join(".skri").join("outbound").join(&chapter);
    let mut platforms = Vec::new();
    let mut skipped_posted = Vec::new();

    let conn = store.conn();
    ensure_tables(conn)?;
    for platform in PLATFORMS {
        let pkg = Package {
            platform: platform.to_string(),
            title: title.clone(),
            html_body: html_body.clone(),
            notes: notes.clone(),
            tags_json: tags_json.clone(),
            version_hash: version_hash.clone(),
        };
        let pdir = dir.join(platform);
        std::fs::create_dir_all(&pdir)?;
        let json = serde_json::json!({
            "platform": pkg.platform,
            "title": pkg.title,
            "html_body": pkg.html_body,
            "notes": pkg.notes,
            "tags_json": pkg.tags_json,
            "version_hash": pkg.version_hash,
        });
        std::fs::write(pdir.join("package.json"), serde_json::to_string_pretty(&json)?)?;
        // пакет також у готовому до копіювання HTML (для ручної відправки)
        std::fs::write(pdir.join("chapter.html"), &pkg.html_body)?;

        let posted: bool = conn
            .query_row(
                "SELECT status FROM crosspost_log WHERE doc_path=?1 AND platform=?2",
                params![&doc_rel, platform],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(|s| s == "posted")
            .unwrap_or(false);
        if posted {
            skipped_posted.push(platform.to_string());
            continue;
        }
        conn.execute(
            "INSERT INTO crosspost_log(doc_path, platform, version_hash, status, packed_at)
             VALUES (?1, ?2, ?3, 'pending', strftime('%s','now'))
             ON CONFLICT(doc_path, platform) DO UPDATE SET
               version_hash=excluded.version_hash, packed_at=excluded.packed_at",
            params![&doc_rel, platform, &version_hash],
        )?;
        platforms.push(platform.to_string());
    }

    Ok(PackReport {
        doc_path: doc_rel,
        chapter,
        version_hash,
        platforms,
        skipped_posted,
        dir,
    })
}

// --- журнал ---

/// Валідні статуси журналу.
pub fn valid_status(s: &str) -> bool {
    matches!(s, "pending" | "posted" | "failed")
}

/// Позначити статус публікації вручну (`skri crosspost mark ... posted`).
///
/// posted: published_at = зараз; зафіксованим стає хеш ПОТОЧНОГО канону
/// (перевідправлена версія), відкриті crosspost-stale алерти для
/// (док, платформа) розв'язуються.
pub fn mark(store: &ProjectStore, doc_rel: &str, platform: &str, status: &str) -> Result<LogRow> {
    if !valid_status(status) {
        return Err(Error::Invalid(format!(
            "невідомий статус «{status}» (pending|posted|failed)"
        )));
    }
    if !PLATFORMS.contains(&platform) {
        return Err(Error::Invalid(format!(
            "невідома платформа «{platform}» ({})",
            PLATFORMS.join(", ")
        )));
    }
    let doc_rel = doc_rel.replace('\\', "/");
    let conn = store.conn();
    ensure_tables(conn)?;

    let raw = std::fs::read_to_string(store.root.join(&doc_rel))?;
    let current_hash = version_hash_of(&raw);
    let version_hash = if status == "posted" {
        // публікується те, що в каноні зараз (перевідправлена версія)
        current_hash
    } else {
        // pending/failed: тримаємо запакований хеш, якщо був
        let existing: Option<String> = conn
            .query_row(
                "SELECT version_hash FROM crosspost_log WHERE doc_path=?1 AND platform=?2",
                params![&doc_rel, platform],
                |r| r.get(0),
            )
            .optional()?;
        existing.unwrap_or(current_hash)
    };

    if status == "posted" {
        conn.execute(
            "INSERT INTO crosspost_log(doc_path, platform, version_hash, status, published_at)
             VALUES (?1, ?2, ?3, 'posted', strftime('%s','now'))
             ON CONFLICT(doc_path, platform) DO UPDATE SET
               status='posted', version_hash=excluded.version_hash,
               published_at=strftime('%s','now')",
            params![&doc_rel, platform, &version_hash],
        )?;
        // розв'язати відкриті stale-алерти цієї пари (перевідправка зроблена)
        resolve_stale_alerts(conn, &doc_rel, platform)?;
    } else {
        conn.execute(
            "INSERT INTO crosspost_log(doc_path, platform, version_hash, status)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(doc_path, platform) DO UPDATE SET status=excluded.status",
            params![&doc_rel, platform, &version_hash, status],
        )?;
    }

    log_row(conn, &doc_rel, platform)?.ok_or_else(|| Error::Invalid("журнал втрачено".into()))
}

fn log_row(conn: &Connection, doc_rel: &str, platform: &str) -> Result<Option<LogRow>> {
    let row = conn
        .query_row(
            "SELECT doc_path, platform, version_hash, status, packed_at, published_at
             FROM crosspost_log WHERE doc_path=?1 AND platform=?2",
            params![doc_rel, platform],
            |r| {
                Ok(LogRow {
                    doc_path: r.get(0)?,
                    platform: r.get(1)?,
                    version_hash: r.get(2)?,
                    status: r.get(3)?,
                    packed_at: r.get(4)?,
                    published_at: r.get(5)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Весь журнал (або одного документа), упорядковано.
pub fn log_rows(store: &ProjectStore, doc: Option<&str>) -> Result<Vec<LogRow>> {
    let conn = store.conn();
    ensure_tables(conn)?;
    let mut stmt = conn.prepare(
        "SELECT doc_path, platform, version_hash, status, packed_at, published_at
         FROM crosspost_log
         WHERE (?1 IS NULL OR doc_path = ?1)
         ORDER BY doc_path, platform",
    )?;
    let rows = stmt
        .query_map(params![doc], |r| {
            Ok(LogRow {
                doc_path: r.get(0)?,
                platform: r.get(1)?,
                version_hash: r.get(2)?,
                status: r.get(3)?,
                packed_at: r.get(4)?,
                published_at: r.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

// --- stale-алерти ---

/// Звіт перевірки застарілих публікацій.
#[derive(Debug, Default)]
pub struct StaleReport {
    /// (док, платформа, хеш_опублікованої, хеш_поточної)
    pub stale: Vec<(String, String, String, String)>,
}

/// Правка канону після публікації: хеш поточного тіла ≠ version_hash
/// рядка зі статусом posted → алерт `kind='crosspost-stale'`.
/// Ідемпотентно: по одному відкритому алерту на (док, платформа).
pub fn check_stale(store: &ProjectStore) -> Result<StaleReport> {
    let conn = store.conn();
    ensure_tables(conn)?;
    let mut report = StaleReport::default();
    let posted: Vec<(String, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT doc_path, platform, version_hash FROM crosspost_log
             WHERE status='posted' ORDER BY doc_path, platform",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };
    for (doc, platform, old_hash) in posted {
        let path = store.root.join(&doc);
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue; // файл зник — не stale, окремої історії нема
        };
        let new_hash = version_hash_of(&raw);
        if new_hash == old_hash {
            continue;
        }
        if stale_alert_exists(conn, &doc, &platform)? {
            report.stale.push((doc, platform, old_hash, new_hash));
            continue;
        }
        let payload = serde_json::json!({
            "doc": doc,
            "platform": platform,
            "old_hash": old_hash,
            "new_hash": new_hash,
            "message": format!(
                "главу змінено після публікації на {platform} — потрібна перевідправка"
            ),
        });
        conn.execute(
            "INSERT INTO alerts(kind, entity_id, payload_json, created_at)
             VALUES ('crosspost-stale', NULL, ?1, strftime('%s','now'))",
            params![serde_json::to_string(&payload)?],
        )?;
        report.stale.push((doc, platform, old_hash, new_hash));
    }
    Ok(report)
}

fn stale_alert_exists(conn: &Connection, doc: &str, platform: &str) -> Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT payload_json FROM alerts
         WHERE kind='crosspost-stale' AND resolved=0",
    )?;
    let payloads: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    for p in payloads {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&p) else {
            continue;
        };
        if v.get("doc").and_then(|d| d.as_str()) == Some(doc)
            && v.get("platform").and_then(|p| p.as_str()) == Some(platform)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn resolve_stale_alerts(conn: &Connection, doc: &str, platform: &str) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, payload_json FROM alerts
         WHERE kind='crosspost-stale' AND resolved=0",
    )?;
    let found: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    for (id, p) in found {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&p) else {
            continue;
        };
        if v.get("doc").and_then(|d| d.as_str()) == Some(doc)
            && v.get("platform").and_then(|p| p.as_str()) == Some(platform)
        {
            conn.execute("UPDATE alerts SET resolved=1 WHERE id=?1", params![id])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_bold_italic_paragraphs_hr() {
        let html = markdown_to_html("Вступ із **жирним** і *курсивом*.\n\nДругий абзац.\n---\nХвіст.");
        assert!(html.contains("<strong>жирним</strong>"), "bold: {html}");
        assert!(html.contains("<em>курсивом</em>"), "italic: {html}");
        assert!(html.contains("<p>Вступ із"), "paragraph: {html}");
        assert!(html.contains("<hr>"), "hr: {html}");
        assert_eq!(html.matches("<p>").count(), 3, "3 параграфи: {html}");
    }

    #[test]
    fn html_escapes_and_mismatched_markers() {
        assert_eq!(inline_html("a<b & c"), "a&lt;b &amp; c");
        assert_eq!(inline_html("**незакритий"), "<strong>незакритий</strong>");
        assert_eq!(inline_html("п*рів"), "п<em>рів</em>");
    }
}
