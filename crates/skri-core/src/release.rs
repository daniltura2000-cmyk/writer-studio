//! Release Planner — серійний командний центр (§5.6): Kanban глав + прогноз
//! вичерпання буфера.
//!
//! - Реєстр глав: auto-scan `manuscript/*.md` → таблиця `chapters_registry`
//!   (doc_path, order, pub_status, pub_date, platforms_json). **Статус живе у
//!   frontmatter глави (`pub_status:`)** — файли залишаються єдиним джерелом
//!   правди (залізне правило №1); таблиця — похідний індекс, перебудовується
//!   з файлів будь-коли, ідемпотентно.
//! - Kanban-звіт: групування по статусах draft → edit → buffer → scheduled →
//!   published + журнал переходів (`chapters_log`: зміна статусу між прогонами).
//! - Прогноз буфера: фактична швидкість письма = слів/день з mtime-дельт
//!   історії глав (ковзне середнє по останніх N главах); буфер = глави у
//!   статусі buffer/scheduled; «запас вичерпається через X днів» і дата
//!   «закінчиться Y-го числа» (YYYY-MM-DD, UTC).
//!
//! Std-only: дати — секунди Unix epoch, цивільна дата — арифметикою днів
//! від епохи (алгоритм Howard Hinnant), chrono не потрібен.

use crate::frontmatter;
use crate::store::ProjectStore;
use crate::{Error, Result};
use rusqlite::{params, OptionalExtension};

/// Канбан-колонки в порядку життєвого циклу глави.
pub const STATUSES: [&str; 5] = ["draft", "edit", "buffer", "scheduled", "published"];

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS chapters_registry (
  doc_path TEXT PRIMARY KEY,
  order_index INTEGER NOT NULL,
  pub_status TEXT NOT NULL DEFAULT 'draft',
  pub_date INTEGER,
  platforms_json TEXT NOT NULL DEFAULT '[]'
);
CREATE TABLE IF NOT EXISTS chapters_log (
  id INTEGER PRIMARY KEY,
  doc_path TEXT NOT NULL,
  old_status TEXT,
  new_status TEXT NOT NULL,
  changed_at INTEGER NOT NULL
);
"#;

/// Один рядок реєстру глав (об'єднаний з docs: title/words/mtime).
#[derive(Debug, Clone)]
pub struct ChapterEntry {
    pub doc_path: String,
    pub title: String,
    pub order: i64,
    pub status: String,
    /// epoch-секунди (UTC опівнічі) з frontmatter `pub_date: YYYY-MM-DD`
    pub pub_date: Option<i64>,
    pub platforms: Vec<String>,
    pub word_count: i64,
    pub mtime: i64,
}

/// Перехід статусу між прогонами перебудови.
#[derive(Debug, Clone)]
pub struct Transition {
    pub doc_path: String,
    pub from: Option<String>,
    pub to: String,
    pub at: i64,
}

/// Звіт перебудови реєстру.
#[derive(Debug, Default)]
pub struct RegistryReport {
    pub chapters: usize,
    pub transitions: usize,
}

/// Kanban-звіт: колонки по статусах + останні переходи.
#[derive(Debug, Default)]
pub struct KanbanReport {
    pub columns: Vec<(String, Vec<ChapterEntry>)>,
    pub transitions: Vec<Transition>,
}

/// Прогноз вичерпання буфера.
#[derive(Debug, Clone, Default)]
pub struct Forecast {
    pub window: usize,
    /// фактична швидкість письма, слів/день (ковзне середнє)
    pub words_per_day: f64,
    /// скільки глав увійшло у ковзне середнє
    pub samples: usize,
    pub buffer_chapters: usize,
    pub buffer_words: i64,
    pub days_per_chapter: f64,
    /// «запас вичерпається через X днів» (None = швидкість невідома)
    pub days_left: Option<f64>,
    /// «закінчиться Y-го числа» (YYYY-MM-DD, UTC)
    pub exhaustion_date: Option<String>,
}

/// Перебудувати реєстр глав з `manuscript/*.md` (ідемпотентно).
///
/// Статус читається з frontmatter (`pub_status:`, за потреби `pub_date:` та
/// `pub_platforms:`); зміна статусу між прогонами пишеться в `chapters_log`.
pub fn rebuild(store: &ProjectStore) -> Result<RegistryReport> {
    let conn = store.conn();
    conn.execute_batch(SCHEMA)?;
    let mut report = RegistryReport::default();

    let paths: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT path FROM docs WHERE kind='manuscript' AND path LIKE 'manuscript/%' ORDER BY path",
        )?;
        let rows = stmt.query_map([], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    let mut seen: Vec<String> = Vec::new();
    for (i, rel) in paths.iter().enumerate() {
        let path = store.root.join(rel);
        // файл міг зникнути після останнього reindex (reindex не чистить docs) —
        // такий рядок піде з реєстру разом зі stale-чисткою нижче
        if !path.exists() {
            continue;
        }
        let raw = std::fs::read_to_string(&path)?;
        let parsed = frontmatter::parse(&raw);
        let status = parsed
            .meta
            .get("pub_status")
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "draft".to_string());
        let pub_date = parsed.meta.get("pub_date").and_then(|s| parse_ymd(s));
        let platforms: Vec<String> = parsed
            .meta
            .get("pub_platforms")
            .map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let order = chapter_number(rel).unwrap_or((i + 1) as i64);
        let platforms_json = serde_json::to_string(&platforms)?;

        let prev: Option<String> = conn
            .query_row(
                "SELECT pub_status FROM chapters_registry WHERE doc_path = ?1",
                params![rel],
                |r| r.get(0),
            )
            .optional()?;
        match prev {
            Some(old) => {
                conn.execute(
                    "UPDATE chapters_registry SET order_index=?2, pub_status=?3, pub_date=?4,
                     platforms_json=?5 WHERE doc_path=?1",
                    params![rel, order, status, pub_date, platforms_json],
                )?;
                if old != status {
                    conn.execute(
                        "INSERT INTO chapters_log(doc_path, old_status, new_status, changed_at)
                         VALUES (?1, ?2, ?3, strftime('%s','now'))",
                        params![rel, old, status],
                    )?;
                    report.transitions += 1;
                }
            }
            None => {
                conn.execute(
                    "INSERT INTO chapters_registry(doc_path, order_index, pub_status, pub_date, platforms_json)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![rel, order, status, pub_date, platforms_json],
                )?;
            }
        }
        seen.push(rel.clone());
        report.chapters += 1;
    }

    // файли, яких більше не існує (або виїхали з manuscript/) — геть з реєстру
    let stale: Vec<String> = {
        let mut stmt = conn.prepare("SELECT doc_path FROM chapters_registry")?;
        let rows = stmt.query_map([], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .filter(|p: &String| !seen.contains(p))
            .collect();
        rows
    };
    for p in stale {
        conn.execute("DELETE FROM chapters_registry WHERE doc_path = ?1", params![p])?;
    }

    Ok(report)
}

/// Kanban-звіт: групування глав по статусах + останні переходи.
pub fn kanban(store: &ProjectStore) -> Result<KanbanReport> {
    rebuild(store)?;
    let conn = store.conn();
    let mut entries: Vec<ChapterEntry> = {
        let mut stmt = conn.prepare(
            "SELECT r.doc_path, COALESCE(d.title,''), r.order_index, r.pub_status, r.pub_date,
                    r.platforms_json, COALESCE(d.word_count,0), COALESCE(d.mtime,0)
             FROM chapters_registry r LEFT JOIN docs d ON d.path = r.doc_path
             ORDER BY r.order_index, r.doc_path",
        )?;
        let rows = stmt.query_map([], |r| {
            let platforms_json: String = r.get(5)?;
            Ok(ChapterEntry {
                doc_path: r.get(0)?,
                title: {
                    let t: String = r.get(1)?;
                    if t.is_empty() {
                        fallback_title(r.get::<_, String>(0)?.as_str())
                    } else {
                        t
                    }
                },
                order: r.get(2)?,
                status: r.get(3)?,
                pub_date: r.get(4)?,
                platforms: serde_json::from_str(&platforms_json).unwrap_or_default(),
                word_count: r.get(6)?,
                mtime: r.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
        rows
    };

    let mut columns: Vec<(String, Vec<ChapterEntry>)> = STATUSES
        .iter()
        .map(|s| (s.to_string(), Vec::new()))
        .collect();
    for e in entries.drain(..) {
        match columns.iter_mut().find(|(s, _)| *s == e.status) {
            Some((_, col)) => col.push(e),
            None => columns.push((e.status.clone(), vec![e])), // нестандартний статус — окрема колонка
        }
    }

    let transitions: Vec<Transition> = {
        let mut stmt = conn.prepare(
            "SELECT doc_path, old_status, new_status, changed_at FROM chapters_log
             ORDER BY changed_at DESC, id DESC LIMIT 50",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Transition {
                doc_path: r.get(0)?,
                from: r.get(1)?,
                to: r.get(2)?,
                at: r.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
        rows
    };

    Ok(KanbanReport { columns, transitions })
}

/// Встановити статус глави ПРЯМО У frontmatter файлу (єдине джерело правди).
/// `pub_date` — "YYYY-MM-DD"; платформи — список; обидва опційні (None = не чіпати).
pub fn set_status(
    store: &ProjectStore,
    doc: &str,
    status: &str,
    pub_date: Option<&str>,
    platforms: &[String],
) -> Result<()> {
    let status = status.trim().to_lowercase();
    if !STATUSES.contains(&status.as_str()) {
        return Err(Error::Invalid(format!(
            "невідомий статус «{status}» (очікується: {})",
            STATUSES.join(" | ")
        )));
    }
    if let Some(d) = pub_date {
        if parse_ymd(d).is_none() {
            return Err(Error::Invalid(format!("pub_date: очікується YYYY-MM-DD, є «{d}»")));
        }
    }
    let rel = doc.replace('\\', "/");
    let path = store.root.join(&rel);
    let raw = std::fs::read_to_string(&path)?;
    let mut updates: Vec<(&str, String)> = vec![("pub_status", status)];
    if let Some(d) = pub_date {
        updates.push(("pub_date", d.to_string()));
    }
    if !platforms.is_empty() {
        updates.push(("pub_platforms", platforms.join(", ")));
    }
    std::fs::write(&path, upsert_header_keys(&raw, &updates))?;
    Ok(())
}

/// Точка історії письма: (mtime epoch, слів у главі).
pub fn forecast(store: &ProjectStore, window: usize) -> Result<Forecast> {
    rebuild(store)?;
    let conn = store.conn();
    let history: Vec<(i64, i64)> = {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(d.mtime,0), COALESCE(d.word_count,0)
             FROM chapters_registry r LEFT JOIN docs d ON d.path = r.doc_path
             WHERE COALESCE(d.mtime,0) > 0 AND COALESCE(d.word_count,0) > 0
             ORDER BY d.mtime",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };
    let (buf_chapters, buf_words): (usize, i64) = {
        let mut stmt = conn.prepare(
            "SELECT COUNT(*), COALESCE(SUM(COALESCE(d.word_count,0)),0)
             FROM chapters_registry r LEFT JOIN docs d ON d.path = r.doc_path
             WHERE r.pub_status IN ('buffer','scheduled')",
        )?;
        stmt.query_row([], |r| {
            Ok((r.get::<_, i64>(0)?.max(0) as usize, r.get::<_, i64>(1)?.max(0)))
        })?
    };
    Ok(forecast_with(&history, buf_chapters, buf_words, now_epoch(), window))
}

/// Чиста функція прогнозу (для тестів і майбутнього UI): історія письма +
/// буфер + «зараз» → слова/день, дні запасу, кінцева дата.
pub fn forecast_with(
    history: &[(i64, i64)],
    buffer_chapters: usize,
    buffer_words: i64,
    now: i64,
    window: usize,
) -> Forecast {
    let (words_per_day, samples) = speed_from_history(history, window);
    let avg = if buffer_chapters > 0 { buffer_words as f64 / buffer_chapters as f64 } else { 0.0 };
    let mut f = Forecast {
        window,
        words_per_day,
        samples,
        buffer_chapters,
        buffer_words,
        days_per_chapter: if words_per_day > 0.0 { avg / words_per_day } else { 0.0 },
        days_left: None,
        exhaustion_date: None,
    };
    if words_per_day > 0.0 && buffer_chapters > 0 {
        let days = buffer_words as f64 / words_per_day;
        f.days_left = Some(days);
        // стеля — консервативно: запас гарантовано вичерпається на цей день
        f.exhaustion_date = Some(fmt_date(now + (days.ceil() as i64) * 86_400));
    }
    f
}

/// Фактична швидкість: для кожної глави (крім першої) rate = слів глави /
/// днів від попередньої за mtime; ковзне середнє по останніх `window` главах.
/// Повертає (слів/день, скільки точок увійшло).
pub fn speed_from_history(history: &[(i64, i64)], window: usize) -> (f64, usize) {
    let mut rates: Vec<f64> = Vec::new();
    for pair in history.windows(2) {
        let dt = (pair[1].0 - pair[0].0) as f64 / 86_400.0;
        if dt > 0.0 {
            rates.push(pair[1].1 as f64 / dt);
        }
    }
    let take = rates.len().min(window.max(1));
    if take == 0 {
        return (0.0, 0);
    }
    let used = &rates[rates.len() - take..];
    (used.iter().sum::<f64>() / used.len() as f64, used.len())
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Номер глави з імені файла: ведучі цифри («03-розділ.md» → 3).
fn chapter_number(rel: &str) -> Option<i64> {
    let stem = rel.rsplit('/').next()?;
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() { None } else { digits.parse().ok() }
}

fn fallback_title(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".md").to_string()
}

/// "YYYY-MM-DD" → epoch-секунди (UTC опівнічі).
pub fn parse_ymd(s: &str) -> Option<i64> {
    let s = s.trim().trim_matches('"');
    let mut it = s.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    if it.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d) * 86_400)
}

/// epoch-секунди → "YYYY-MM-DD" (UTC; арифметика днів від епохи, std-only).
pub fn fmt_date(epoch: i64) -> String {
    let (y, m, d) = civil_from_days(epoch.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Днів від 1970-01-01 для цивільної дати (Howard Hinnant, days_from_civil).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Цивільна дата з днів від епохи (Howard Hinnant, civil_from_days).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Вставити/замінити ключі у frontmatter-блоці, решту файлу не чіпати.
/// Файл без frontmatter отримує новий блок на початку.
fn upsert_header_keys(raw: &str, updates: &[(&str, String)]) -> String {
    let has_fm = raw.starts_with("---\n") || raw.starts_with("---\r\n");
    if has_fm {
        let open_len = if raw.starts_with("---\r\n") { 5 } else { 4 };
        let rest = &raw[open_len..];
        let mut close_off = None;
        let mut pos = 0usize;
        for line in rest.split_inclusive('\n') {
            if line.trim_end_matches(|c| c == '\n' || c == '\r').trim() == "---" {
                close_off = Some(pos);
                break;
            }
            pos += line.len();
        }
        if let Some(close_off) = close_off {
            let header = &rest[..close_off];
            let tail = &rest[close_off..]; // рядок "---" + тіло, як є
            let mut lines: Vec<String> = header.lines().map(str::to_string).collect();
            for (k, _) in updates {
                let prefix = format!("{k}:");
                lines.retain(|l| !l.trim_start().starts_with(&prefix));
            }
            for (k, v) in updates {
                lines.push(format!("{k}: {v}"));
            }
            let mut out = String::from("---\n");
            for l in lines {
                out.push_str(&l);
                out.push('\n');
            }
            out.push_str(tail);
            return out;
        }
    }
    // без frontmatter (або незакритий блок) — новий блок на початку
    let mut out = String::from("---\n");
    for (k, v) in updates {
        out.push_str(&format!("{k}: {v}\n"));
    }
    out.push_str("---\n\n");
    out.push_str(raw);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_roundtrip_epoch_arithmetic() {
        assert_eq!(fmt_date(0), "1970-01-01");
        // 2026-11-20 ↔ epoch: туди й назад
        let e = parse_ymd("2026-11-20").unwrap();
        assert_eq!(fmt_date(e), "2026-11-20");
        assert_eq!(parse_ymd("2026-13-01"), None);
        assert_eq!(parse_ymd("не-дата"), None);
        // високосний 2024-02-29
        assert_eq!(fmt_date(parse_ymd("2024-02-29").unwrap()), "2024-02-29");
    }

    #[test]
    fn speed_sliding_window() {
        let day = 86_400i64;
        let h = [(0, 1000), (day, 1000), (2 * day, 500)];
        // по главах: 1000/1 + 500/1 → середнє 750
        assert_eq!(speed_from_history(&h, 7), (750.0, 2));
        // вікно 1 → лише остання глава
        assert_eq!(speed_from_history(&h, 1), (500.0, 1));
        // однакові mtime — пара пропускається
        assert_eq!(speed_from_history(&[(0, 1000), (0, 1000)], 7), (0.0, 0));
    }
}
