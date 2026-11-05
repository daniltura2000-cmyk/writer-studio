//! Retcon Manager — draft/published гілки (специфікація §5.4).
//!
//! - Публікація глави = фіксація версії: тіло (без frontmatter) зберігається
//!   в `published_snapshots(doc_path, version, body_sha256, published_at, body)`.
//!   Хеш — той самий std-only SHA-256, що й для crosspost/ink-імпортів.
//! - Diff чернетки проти останнього снапшоту = **retcon-зобов'язання**:
//!   якщо дельта (змінені рядки, обидва боки) містить назву сутності з entities
//!   або її alias — усі згадки цієї сутності в ПІЗНІШИХ главах (order > поточної)
//!   потрапляють у чергу ревізій `retcon_queue` з переходами open → reviewed.
//! - Перебудова черги ідемпотентна: open-рядки щоразу перезаписуються,
//!   reviewed не плодяться (той самий ключ не повертається назад у open).
//!
//! Правило перше дотримане і тут: `retcon::rebuild` сам робить reindex +
//! scan_project, тож черга — похідна від файлів, базу можна стерти будь-коли.
//!
//! Обмеження (свідомо): по-рядковий diff без внутрішньорядкових змін —
//! «змінений» рядок виглядає як пара removed+added; для дуже великих глав
//! (добуток рядків × рядків > ~4 млн клітинок) LCS не будується і ВСІ
//! рядки вважаються зміненими (консервативно: більше retcon-зобов'язань,
//! ніж менше).

use crate::indexer;
use crate::inkbridge::sha256_hex;
use crate::release;
use crate::store::ProjectStore;
use crate::{Error, Result};
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS published_snapshots (
  doc_path TEXT NOT NULL,
  version INTEGER NOT NULL,
  body_sha256 TEXT NOT NULL,
  published_at INTEGER NOT NULL,
  body TEXT NOT NULL,
  PRIMARY KEY (doc_path, version)
);
CREATE TABLE IF NOT EXISTS retcon_queue (
  id INTEGER PRIMARY KEY,
  entity_name TEXT NOT NULL,
  source_doc TEXT NOT NULL,
  affected_doc TEXT NOT NULL,
  affected_pos INTEGER NOT NULL,
  sentence TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'open'
);
CREATE INDEX IF NOT EXISTS idx_retcon_key
  ON retcon_queue(entity_name, source_doc, affected_doc, affected_pos);
CREATE INDEX IF NOT EXISTS idx_retcon_status ON retcon_queue(status);
"#;

fn ensure_tables(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

/// Звіт публікації глави.
#[derive(Debug, Clone)]
pub struct PublishReport {
    pub doc_path: String,
    pub version: i64,
    pub body_sha256: String,
    pub published_at: i64,
}

/// Рядок по-рядкового diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineDiff {
    /// "added" (рядок є в чернетці, немає в published) або "removed" (навпаки)
    pub kind: &'static str,
    /// 1-based номер рядка в чернетці (для added)
    pub draft_line: Option<usize>,
    /// 1-based номер рядка в published-снапшоті (для removed)
    pub published_line: Option<usize>,
    pub text: String,
}

/// Звіт diff чернетки проти останнього published-снапшоту.
#[derive(Debug, Clone)]
pub struct DiffReport {
    pub doc_path: String,
    /// версія снапшоту, з якою порівнювали
    pub version: i64,
    /// хеш снапшоту
    pub body_sha256: String,
    /// хеш ПОТОЧНОЇ чернетки (для зіставлення з crosspost-версією)
    pub draft_sha256: String,
    pub changes: Vec<LineDiff>,
}

/// Рядок черги ревізій.
#[derive(Debug, Clone)]
pub struct QueueRow {
    pub id: i64,
    pub entity_name: String,
    pub source_doc: String,
    pub affected_doc: String,
    /// char-offset згадки в тілі affected-документа
    pub affected_pos: i64,
    pub sentence: String,
    pub status: String,
}

/// Звіт перебудови черги.
#[derive(Debug, Default, Clone)]
pub struct QueueReport {
    pub open: usize,
    pub reviewed: usize,
}

/// Опублікувати главу: зафіксувати снапшот тіла як нову версію.
///
/// Статус у frontmatter не чіпається (цього робить `release::set_status`) —
/// снапшот лише фіксує, ЯКА версія стала published.
pub fn publish(store: &ProjectStore, doc_rel: &str) -> Result<PublishReport> {
    ensure_tables(store.conn())?;
    let rel = doc_rel.replace('\\', "/");
    let raw = std::fs::read_to_string(store.root.join(&rel))?;
    let body = crate::frontmatter::parse(&raw).body;
    let sha = sha256_hex(body.as_bytes());
    let conn = store.conn();
    let next: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM published_snapshots WHERE doc_path = ?1",
            params![&rel],
            |r| r.get(0),
        )?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO published_snapshots(doc_path, version, body_sha256, published_at, body)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![&rel, next, &sha, now, &body],
    )?;
    Ok(PublishReport {
        doc_path: rel,
        version: next,
        body_sha256: sha,
        published_at: now,
    })
}

/// Останній published-снапшот документа: (version, sha256, body).
pub fn latest_snapshot(
    store: &ProjectStore,
    doc_rel: &str,
) -> Result<Option<(i64, String, String)>> {
    ensure_tables(store.conn())?;
    let rel = doc_rel.replace('\\', "/");
    let conn = store.conn();
    conn.query_row(
        "SELECT version, body_sha256, body FROM published_snapshots
         WHERE doc_path = ?1 ORDER BY version DESC LIMIT 1",
        params![&rel],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
    .map_err(Error::from)
}

/// Diff чернетки проти останнього published-снапшоту.
pub fn diff(store: &ProjectStore, doc_rel: &str) -> Result<DiffReport> {
    ensure_tables(store.conn())?;
    let rel = doc_rel.replace('\\', "/");
    let (version, sha, snap_body) = latest_snapshot(store, &rel)?.ok_or_else(|| {
        Error::Invalid(format!(
            "у «{rel}» немає published-снапшоту — спершу `skri retcon publish`"
        ))
    })?;
    let raw = std::fs::read_to_string(store.root.join(&rel))?;
    let draft = crate::frontmatter::parse(&raw).body;
    let draft_sha = sha256_hex(draft.as_bytes());
    Ok(DiffReport {
        doc_path: rel,
        version,
        body_sha256: sha,
        draft_sha256: draft_sha,
        changes: diff_lines(&snap_body, &draft),
    })
}

/// По-рядковий diff (LCS, std-only): published → draft.
///
/// «Змінений» рядок = пара removed (номер у published) + added (номер у
/// чернетці). Порожні хвостові рядки після останнього '\n' не розрізняються.
pub fn diff_lines(published: &str, draft: &str) -> Vec<LineDiff> {
    let a: Vec<&str> = published.lines().collect();
    let b: Vec<&str> = draft.lines().collect();
    let (n, m) = (a.len(), b.len());
    if n.saturating_mul(m) > 4_000_000 {
        // консервативний fallback: усе вважається зміненим
        let mut out: Vec<LineDiff> = a
            .iter()
            .enumerate()
            .map(|(i, t)| LineDiff {
                kind: "removed",
                published_line: Some(i + 1),
                draft_line: None,
                text: (*t).to_string(),
            })
            .collect();
        out.extend(b.iter().enumerate().map(|(j, t)| LineDiff {
            kind: "added",
            published_line: None,
            draft_line: Some(j + 1),
            text: (*t).to_string(),
        }));
        return out;
    }
    // lcs[i][j] = довжина LCS a[i..] × b[j..]
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    let mut lcs = vec![0usize; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[idx(i, j)] = if a[i] == b[j] {
                lcs[idx(i + 1, j + 1)] + 1
            } else {
                lcs[idx(i + 1, j)].max(lcs[idx(i, j + 1)])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            i += 1;
            j += 1;
        } else if lcs[idx(i + 1, j)] >= lcs[idx(i, j + 1)] {
            out.push(LineDiff {
                kind: "removed",
                published_line: Some(i + 1),
                draft_line: None,
                text: a[i].to_string(),
            });
            i += 1;
        } else {
            out.push(LineDiff {
                kind: "added",
                published_line: None,
                draft_line: Some(j + 1),
                text: b[j].to_string(),
            });
            j += 1;
        }
    }
    while i < n {
        out.push(LineDiff {
            kind: "removed",
            published_line: Some(i + 1),
            draft_line: None,
            text: a[i].to_string(),
        });
        i += 1;
    }
    while j < m {
        out.push(LineDiff {
            kind: "added",
            published_line: None,
            draft_line: Some(j + 1),
            text: b[j].to_string(),
        });
        j += 1;
    }
    out
}

/// Перебудувати чергу retcon-ревізій (ідемпотентно).
///
/// Кроки: reindex → scan_project (свіжі згадки) → release::rebuild (порядок
/// глав) → для кожного документа з published-снапшотом: diff чернетки →
/// сутності, торкнуті дельтою → згадки цих сутностей у ПІЗНІШИХ главах →
/// черга. Open-рядки перезаписуються; позиція, вже позначена reviewed
/// (той самий ключ сутність+джерело+документ+позиція), назад у open не
/// повертається.
pub fn rebuild(store: &ProjectStore) -> Result<QueueReport> {
    ensure_tables(store.conn())?;
    store.reindex()?;
    indexer::scan_project(store)?;
    release::rebuild(store)?;
    let conn = store.conn();

    // порядок глав (як у release: ведучі цифри імені файла, інакше позиція)
    let order_of: HashMap<String, i64> = {
        let mut stmt =
            conn.prepare("SELECT doc_path, order_index FROM chapters_registry")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    // останні снапшоти
    let snaps: Vec<(String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT s.doc_path, s.body FROM published_snapshots s
             JOIN (SELECT doc_path, MAX(version) AS v FROM published_snapshots GROUP BY doc_path) t
               ON t.doc_path = s.doc_path AND t.v = s.version",
        )?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };

    // open-рядки перезаписуються повністю
    conn.execute("DELETE FROM retcon_queue WHERE status = 'open'", [])?;

    for (path, snap_body) in &snaps {
        let current = indexer::read_body(store, path)?;
        let changes = diff_lines(snap_body, &current);
        if changes.is_empty() {
            continue;
        }
        let delta: Vec<String> = changes.iter().map(|c| c.text.to_lowercase()).collect();
        let src_order = order_of
            .get(path)
            .copied()
            .unwrap_or_else(|| fallback_order(path));
        for (entity_id, name) in touched_entities(conn, &delta)? {
            let mentions: Vec<(String, i64, String)> = {
                let mut stmt = conn.prepare(
                    "SELECT d.path, m.char_start, m.sentence
                     FROM mentions m JOIN docs d ON d.id = m.doc_id
                     WHERE m.entity_id = ?1",
                )?;
                let rows = stmt
                    .query_map(params![entity_id], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    })?
                    .filter_map(|r| r.ok())
                    .filter(|(doc, _, _)| {
                        doc != path
                            && order_of.get(doc).copied().unwrap_or(i64::MIN) > src_order
                    })
                    .collect();
                rows
            };
            for (affected_doc, pos, sentence) in mentions {
                // reviewed не плодяться: та сама позиція вже розглянута автором
                let seen: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM retcon_queue
                         WHERE entity_name=?1 AND source_doc=?2 AND affected_doc=?3
                           AND affected_pos=?4 AND status='reviewed'",
                        params![&name, path, &affected_doc, pos],
                        |r| r.get(0),
                    )
                    .optional()?;
                if seen.is_some() {
                    continue;
                }
                conn.execute(
                    "INSERT INTO retcon_queue(entity_name, source_doc, affected_doc, affected_pos, sentence, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'open')",
                    params![&name, path, &affected_doc, pos, &sentence],
                )?;
            }
        }
    }

    let (open, reviewed) = conn.query_row(
        "SELECT
           COALESCE(SUM(status = 'open'), 0),
           COALESCE(SUM(status = 'reviewed'), 0)
         FROM retcon_queue",
        [],
        |r| Ok((r.get::<_, i64>(0)?.max(0) as usize, r.get::<_, i64>(1)?.max(0) as usize)),
    )?;
    Ok(QueueReport { open, reviewed })
}

/// Сутності, чиї назви/alias-и є в дельті (substr без регістру):
/// (entity_id, canonical_form).
fn touched_entities(
    conn: &rusqlite::Connection,
    delta: &[String],
) -> Result<Vec<(i64, String)>> {
    let mut out = Vec::new();
    let mut stmt = conn.prepare("SELECT id, canonical_form, aliases_json FROM entities")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();
    for (id, canonical, aliases_json) in rows {
        let mut forms = vec![canonical.to_lowercase()];
        if let Ok(aliases) = serde_json::from_str::<Vec<String>>(&aliases_json) {
            forms.extend(aliases.iter().map(|a| a.to_lowercase()));
        }
        forms.retain(|f| !f.is_empty());
        if forms
            .iter()
            .any(|f| delta.iter().any(|line| line.contains(f.as_str())))
        {
            out.push((id, canonical));
        }
    }
    Ok(out)
}

/// Черга ревізій (спершу перебудова — read-команда завжди свіжа).
pub fn queue(store: &ProjectStore) -> Result<Vec<QueueRow>> {
    rebuild(store)?;
    queue_rows(store)
}

/// Рядки черги без перебудови (для UI/тестів).
pub fn queue_rows(store: &ProjectStore) -> Result<Vec<QueueRow>> {
    ensure_tables(store.conn())?;
    let conn = store.conn();
    let mut stmt = conn.prepare(
        "SELECT id, entity_name, source_doc, affected_doc, affected_pos, sentence, status
         FROM retcon_queue
         ORDER BY CASE status WHEN 'open' THEN 0 ELSE 1 END, id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(QueueRow {
                id: r.get(0)?,
                entity_name: r.get(1)?,
                source_doc: r.get(2)?,
                affected_doc: r.get(3)?,
                affected_pos: r.get(4)?,
                sentence: r.get(5)?,
                status: r.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Позначити ревізію розглянутою (`skri retcon review <id>`): open → reviewed.
/// Ідемпотентно: повторний review уже-reviewed рядка — no-op.
pub fn review(store: &ProjectStore, id: i64) -> Result<QueueRow> {
    ensure_tables(store.conn())?;
    let conn = store.conn();
    let exists: Option<i64> = conn
        .query_row("SELECT id FROM retcon_queue WHERE id = ?1", params![id], |r| {
            r.get(0)
        })
        .optional()?;
    if exists.is_none() {
        return Err(Error::Invalid(format!("реткон-рядок #{id} не існує")));
    }
    conn.execute(
        "UPDATE retcon_queue SET status = 'reviewed' WHERE id = ?1",
        params![id],
    )?;
    Ok(queue_rows(store)?
        .into_iter()
        .find(|r| r.id == id)
        .expect("рядок щойно перевірений"))
}

/// Порядок глави з імені файла (дубль правила release: ведучі цифри).
fn fallback_order(rel: &str) -> i64 {
    let stem = rel.rsplit('/').next().unwrap_or(rel);
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        0
    } else {
        digits.parse().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_lines_added_removed_and_unchanged() {
        let published = "рядок один\nрядок два\nрядок три";
        let draft = "рядок один\nрядок ДВА новий\nрядок три\nрядок чотири";
        let d = diff_lines(published, draft);
        assert_eq!(
            d,
            vec![
                LineDiff {
                    kind: "removed",
                    published_line: Some(2),
                    draft_line: None,
                    text: "рядок два".into()
                },
                LineDiff {
                    kind: "added",
                    published_line: None,
                    draft_line: Some(2),
                    text: "рядок ДВА новий".into()
                },
                LineDiff {
                    kind: "added",
                    published_line: None,
                    draft_line: Some(4),
                    text: "рядок чотири".into()
                },
            ]
        );
        assert!(diff_lines(published, published).is_empty());
        // вставка в середині: лише один added
        assert_eq!(
            diff_lines("a\nb", "a\nc\nb").len(),
            1,
            "вставка = один added"
        );
    }
}
