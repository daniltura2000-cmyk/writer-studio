//! Pitch/Source CRM + Fact Ledger (журналістика, специфікація §5.11).
//!
//! - **Pitch lifecycle**: idea → pitched → followed_up → assigned → drafting →
//!   submitted → published|rejected. Стан пітчів живе в БД — це CRM-дані,
//!   створені користувачем через CLI (так само, як glossary/trans_segments:
//!   таблиця — джерело правди для цих даних, файлів, з яких їх можна
//!   перебудувати, не існує). Журнал переходів `pitch_log` — для статистики
//!   часу в статусах; запис «створено як idea» пишеться автоматично.
//! - **Follow-up**: due = sent_date + followup_after_days днів. Кандидат на
//!   follow-up лист — пітч, у якого due вже настала, а last_touch давніша
//!   за due (жодного дотику після настання терміну); термінальні статуси
//!   (published/rejected) не нагадують.
//! - **Fact ledger**: твердження про реальний світ (claim) з посиланням на
//!   джерело і опційною прив'язкою до місця в рукописі (doc_path + char_start,
//!   байтовий offset тіла — так само, як у згадках Index Engine). Звіт
//!   `unverified` — твердження без verified=1 перед здачею матеріалу.
//! - **Content decay**: перевірені твердження, чия верифікація старша за
//!   N днів, повертаються у чергу пере-перевірки («цифра 2024, тобі 18 місяців»).
//!
//! Дати — epoch-секунди UTC опівночі; формат YYYY-MM-DD через публічні
//! `release::parse_ymd`/`release::fmt_date` (std-only, chrono не додається).
//! Функції, що залежать від «зараз», приймають `now: i64` параметром —
//! чиста логіка тестується без системного часу.

use crate::release;
use crate::store::ProjectStore;
use crate::{Error, Result};
use rusqlite::{params, OptionalExtension};
use std::collections::BTreeMap;

const DAY: i64 = 86_400;

/// Статуси pitch lifecycle у порядку конвеєра.
pub const STATUSES: [&str; 8] = [
    "idea",
    "pitched",
    "followed_up",
    "assigned",
    "drafting",
    "submitted",
    "published",
    "rejected",
];

/// Термінальні статуси: follow-up по них не нагадує.
const TERMINAL: [&str; 2] = ["published", "rejected"];

/// Норма фолоу-апу за замовчуванням: днів після sent.
pub const DEFAULT_FOLLOWUP_DAYS: i64 = 14;

/// Скільки днів вважається «застарілим» перевіреним твердженням (content decay).
pub const DEFAULT_DECAY_DAYS: i64 = 365;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS pitches (
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  outlet TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'idea',
  due_date INTEGER,
  sent_date INTEGER,
  followup_after_days INTEGER NOT NULL DEFAULT 14,
  last_touch INTEGER NOT NULL DEFAULT 0,
  note_doc TEXT
);
CREATE TABLE IF NOT EXISTS pitch_log (
  id INTEGER PRIMARY KEY,
  pitch_id INTEGER NOT NULL REFERENCES pitches(id) ON DELETE CASCADE,
  old_status TEXT,
  new_status TEXT NOT NULL,
  at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS facts (
  id INTEGER PRIMARY KEY,
  claim TEXT NOT NULL,
  source_ref TEXT NOT NULL DEFAULT '',
  verified INTEGER NOT NULL DEFAULT 0,
  doc_path TEXT,
  char_start INTEGER,
  verified_at INTEGER
);
"#;

/// Пітч у CRM.
#[derive(Debug, Clone)]
pub struct Pitch {
    pub id: i64,
    pub title: String,
    pub outlet: String,
    pub status: String,
    /// дедлайн здачі (epoch, UTC опівнічі) — з CLI "YYYY-MM-DD"
    pub due_date: Option<i64>,
    /// коли пітч відправлено (epoch)
    pub sent_date: Option<i64>,
    /// норма фолоу-апу цього видання, днів після sent
    pub followup_after_days: i64,
    /// останній дотик (set статусу / followup) — epoch
    pub last_touch: i64,
    /// опційний документ-заметка про пітч (відносний шлях)
    pub note_doc: Option<String>,
}

/// Твердження факт-леджера.
#[derive(Debug, Clone)]
pub struct Fact {
    pub id: i64,
    pub claim: String,
    pub source_ref: String,
    pub verified: bool,
    /// рукопис, де твердження використано (відносний шлях), опційно
    pub doc_path: Option<String>,
    /// байтовий offset у тілі документа
    pub char_start: Option<i64>,
    /// коли верифіковано (epoch) — для content decay
    pub verified_at: Option<i64>,
}

pub fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn ensure_schema(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

// --- Pitch CRM ---

/// Створити пітч у статусі idea. `due_date` — "YYYY-MM-DD" або None;
/// `at` — момент створення (epoch), пишеться в журнал і last_touch.
pub fn add(
    store: &ProjectStore,
    title: &str,
    outlet: &str,
    due_date: Option<&str>,
    followup_days: i64,
    note_doc: Option<&str>,
    at: i64,
) -> Result<i64> {
    if title.trim().is_empty() {
        return Err(Error::Invalid("порожня назва пітча".into()));
    }
    let due = due_date
        .map(|d| {
            release::parse_ymd(d)
                .ok_or_else(|| Error::Invalid(format!("due_date: очікується YYYY-MM-DD, є «{d}»")))
        })
        .transpose()?;
    ensure_schema(store.conn())?;
    store.conn().execute(
        "INSERT INTO pitches(title, outlet, status, due_date, followup_after_days, note_doc, last_touch)
         VALUES (?1, ?2, 'idea', ?3, ?4, ?5, ?6)",
        params![title.trim(), outlet.trim(), due, followup_days, note_doc, at],
    )?;
    let id = store.conn().last_insert_rowid();
    // запис створення в журнал — для статистики часу в статусах
    store.conn().execute(
        "INSERT INTO pitch_log(pitch_id, old_status, new_status, at) VALUES (?1, NULL, 'idea', ?2)",
        params![id, at],
    )?;
    Ok(id)
}

/// Отримати пітч за id.
pub fn get(store: &ProjectStore, id: i64) -> Result<Pitch> {
    let t = store
        .conn()
        .query_row("SELECT * FROM pitches WHERE id = ?1", params![id], row_pitch)
        .optional()?
        .ok_or_else(|| Error::Invalid(format!("пітч #{id} не знайдено")))?;
    Ok(row_to_pitch(t))
}

/// Перехід статусу (та опційна дата sent "YYYY-MM-DD"). Перехід у `pitched`
/// без явної sent_date фіксує sent = `at`. Останній дотик до видання:
/// якщо передано backdated sent_date — це дата події (запис історії не має
/// гасити follow-up нагадування), інакше `at`. Кожна зміна пишеться в
/// `pitch_log` (це журнал подій, як chapters_log у release).
pub fn set_status(
    store: &ProjectStore,
    id: i64,
    status: &str,
    sent_date: Option<&str>,
    at: i64,
) -> Result<Pitch> {
    let status = status.trim().to_lowercase();
    if !STATUSES.contains(&status.as_str()) {
        return Err(Error::Invalid(format!(
            "невідомий статус «{status}» (очікується: {})",
            STATUSES.join(" | ")
        )));
    }
    let sent = sent_date
        .map(|d| {
            release::parse_ymd(d)
                .ok_or_else(|| Error::Invalid(format!("sent_date: очікується YYYY-MM-DD, є «{d}»")))
        })
        .transpose()?;
    let p = get(store, id)?;
    let touch = sent.unwrap_or(at);
    store.conn().execute(
        "UPDATE pitches SET status = ?2, last_touch = ?3,
         sent_date = COALESCE(?4, CASE WHEN ?2 = 'pitched' AND sent_date IS NULL THEN ?3 ELSE sent_date END)
         WHERE id = ?1",
        params![id, status, touch, sent],
    )?;
    store.conn().execute(
        "INSERT INTO pitch_log(pitch_id, old_status, new_status, at) VALUES (?1, ?2, ?3, ?4)",
        params![id, p.status, status, at],
    )?;
    get(store, id)
}

/// Змінити норму фолоу-апу пітча (днів після sent).
pub fn set_followup(store: &ProjectStore, id: i64, days: i64) -> Result<()> {
    if days < 0 {
        return Err(Error::Invalid("followup_after_days не може бути від'ємним".into()));
    }
    get(store, id)?;
    store
        .conn()
        .execute("UPDATE pitches SET followup_after_days = ?2 WHERE id = ?1", params![id, days])?;
    Ok(())
}

fn row_pitch(r: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, String, String, String, Option<i64>, Option<i64>, i64, i64, Option<String>)> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
    ))
}

fn row_to_pitch(t: (i64, String, String, String, Option<i64>, Option<i64>, i64, i64, Option<String>)) -> Pitch {
    Pitch {
        id: t.0,
        title: t.1,
        outlet: t.2,
        status: t.3,
        due_date: t.4,
        sent_date: t.5,
        followup_after_days: t.6,
        last_touch: t.7,
        note_doc: t.8,
    }
}

/// Усі пітчі, за id.
pub fn list(store: &ProjectStore) -> Result<Vec<Pitch>> {
    ensure_schema(store.conn())?;
    let mut stmt = store.conn().prepare("SELECT * FROM pitches ORDER BY id")?;
    let rows = stmt
        .query_map([], row_pitch)?
        .filter_map(|r| r.ok())
        .map(row_to_pitch)
        .collect();
    Ok(rows)
}

/// Чиста функція: id пітчів — кандидатів на follow-up лист.
/// Кандидат = не-термінальний статус, sent_date є, due = sent + N днів
/// вже настала (due ≤ now), а last_touch давніша за due.
pub fn due_followups(pitches: &[Pitch], now: i64) -> Vec<i64> {
    pitches
        .iter()
        .filter(|p| {
            !TERMINAL.contains(&p.status.as_str())
                && p.sent_date
                    .map(|sent| {
                        let due = sent + p.followup_after_days * DAY;
                        due <= now && p.last_touch < due
                    })
                    .unwrap_or(false)
        })
        .map(|p| p.id)
        .collect()
}

/// Кандидати на follow-up (пітчі, відсортовані за давністю: найстаріший touch перший).
pub fn followup_candidates(store: &ProjectStore, now: i64) -> Result<Vec<Pitch>> {
    let all = list(store)?;
    let due_ids = due_followups(&all, now);
    let mut out: Vec<Pitch> = all.into_iter().filter(|p| due_ids.contains(&p.id)).collect();
    out.sort_by_key(|p| p.last_touch);
    Ok(out)
}

/// Статистика pitch-воронки.
#[derive(Debug, Default, Clone)]
pub struct PitchStats {
    pub total: usize,
    /// кількість пітчів у кожному статусі (порядок STATUSES, лише ненульові)
    pub by_status: Vec<(String, usize)>,
    pub published: usize,
    /// конверсія idea→published, % (published / total * 100)
    pub conversion: f64,
    /// середній час у статусі, днів (з pitch_log, вкл. поточний інтервал до now)
    pub avg_days_in_status: Vec<(String, f64)>,
}

pub fn stats(store: &ProjectStore, now: i64) -> Result<PitchStats> {
    ensure_schema(store.conn())?;
    let all = list(store)?;
    let total = all.len();
    let published = all.iter().filter(|p| p.status == "published").count();
    let mut by_status: Vec<(String, usize)> = STATUSES
        .iter()
        .map(|s| {
            (
                s.to_string(),
                all.iter().filter(|p| p.status == *s).count(),
            )
        })
        .filter(|(_, n)| *n > 0)
        .collect();
    // нестандартні статуси (пряма правка БД) — теж показати
    for p in &all {
        if !STATUSES.contains(&p.status.as_str())
            && !by_status.iter().any(|(s, _)| *s == p.status)
        {
            by_status.push((p.status.clone(), 1));
        }
    }
    let entries: Vec<(i64, String, i64)> = {
        let mut stmt = store
            .conn()
            .prepare("SELECT pitch_id, new_status, at FROM pitch_log ORDER BY pitch_id, at, id")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };
    Ok(PitchStats {
        total,
        by_status,
        published,
        conversion: if total > 0 {
            published as f64 / total as f64 * 100.0
        } else {
            0.0
        },
        avg_days_in_status: avg_time_in_status(&entries, now),
    })
}

/// Чиста функція: середній час у статусах (днів) із журналу переходів.
///
/// `entries` — рядки pitch_log (pitch_id, new_status, at); перший запис
/// пітча — створення (new_status='idea'). Інтервал [at_i, at_{i+1}) належить
/// статусу запису i; останній інтервал пітча тягнеться до `now` (поточний
/// статус «триває»). Середнє = сумарний час у статусі / кількість інтервалів.
pub fn avg_time_in_status(entries: &[(i64, String, i64)], now: i64) -> Vec<(String, f64)> {
    let mut by_pitch: BTreeMap<i64, Vec<(String, i64)>> = BTreeMap::new();
    for (pid, st, at) in entries {
        by_pitch.entry(*pid).or_default().push((st.clone(), *at));
    }
    let mut sum: BTreeMap<String, f64> = BTreeMap::new();
    let mut cnt: BTreeMap<String, usize> = BTreeMap::new();
    for (_, mut log) in by_pitch {
        log.sort_by_key(|(_, at)| *at);
        for i in 0..log.len() {
            let end = log.get(i + 1).map(|e| e.1).unwrap_or(now);
            let dur = (end - log[i].1).max(0) as f64 / DAY as f64;
            *sum.entry(log[i].0.clone()).or_insert(0.0) += dur;
            *cnt.entry(log[i].0.clone()).or_insert(0) += 1;
        }
    }
    let mut out: Vec<(String, f64)> = sum
        .into_iter()
        .map(|(s, total)| {
            let c = cnt[&s].max(1) as f64;
            (s, total / c)
        })
        .collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out
}

// --- Fact Ledger ---

/// Додати неперевірене твердження. `doc_path`/`char_start` — опційна
/// прив'язка до місця в рукописі (offset у тілі, байти).
pub fn fact_add(
    store: &ProjectStore,
    claim: &str,
    source_ref: &str,
    doc_path: Option<&str>,
    char_start: Option<i64>,
) -> Result<i64> {
    if claim.trim().is_empty() {
        return Err(Error::Invalid("порожнє твердження".into()));
    }
    ensure_schema(store.conn())?;
    store.conn().execute(
        "INSERT INTO facts(claim, source_ref, verified, doc_path, char_start)
         VALUES (?1, ?2, 0, ?3, ?4)",
        params![claim.trim(), source_ref.trim(), doc_path, char_start],
    )?;
    Ok(store.conn().last_insert_rowid())
}

/// Позначити твердження верифікованим (verified=1, verified_at=`at`).
pub fn fact_verify(store: &ProjectStore, id: i64, at: i64) -> Result<()> {
    let n = store
        .conn()
        .execute("UPDATE facts SET verified = 1, verified_at = ?2 WHERE id = ?1", params![id, at])?;
    if n == 0 {
        return Err(Error::Invalid(format!("твердження #{id} не знайдено")));
    }
    Ok(())
}

fn row_fact(r: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, String, String, i64, Option<String>, Option<i64>, Option<i64>)> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
    ))
}

fn row_to_fact(t: (i64, String, String, i64, Option<String>, Option<i64>, Option<i64>)) -> Fact {
    Fact {
        id: t.0,
        claim: t.1,
        source_ref: t.2,
        verified: t.3 != 0,
        doc_path: t.4,
        char_start: t.5,
        verified_at: t.6,
    }
}

/// Усі твердження, за id.
pub fn facts(store: &ProjectStore) -> Result<Vec<Fact>> {
    ensure_schema(store.conn())?;
    let mut stmt = store.conn().prepare("SELECT * FROM facts ORDER BY id")?;
    let rows = stmt
        .query_map([], row_fact)?
        .filter_map(|r| r.ok())
        .map(row_to_fact)
        .collect();
    Ok(rows)
}

/// Твердження без verified=1 — звіт перед здачею матеріалу.
/// Ті, що прив'язані до рукопису (doc_path), йдуть першими: їх найбільше
/// сенс перевіряти перед публікацією.
pub fn unverified_facts(store: &ProjectStore) -> Result<Vec<Fact>> {
    let mut all = facts(store)?;
    all.retain(|f| !f.verified);
    all.sort_by_key(|f| match f.doc_path {
        Some(_) => 0,
        None => 1,
    });
    Ok(all)
}

/// Content decay: перевірені твердження, чия верифікація старша за
/// `max_age_days` днів — повертаються у чергу пере-перевірки.
pub fn decayed_facts(store: &ProjectStore, now: i64, max_age_days: i64) -> Result<Vec<Fact>> {
    let mut all = facts(store)?;
    all.retain(|f| f.verified && f.verified_at.map(|t| now - t >= max_age_days * DAY).unwrap_or(false));
    all.sort_by_key(|f| f.verified_at.unwrap_or(0));
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn followup_candidates_pure_logic() {
        let mk = |id: i64, status: &str, sent: Option<i64>, touch: i64, days: i64| Pitch {
            id,
            title: format!("p{id}"),
            outlet: String::new(),
            status: status.into(),
            due_date: None,
            sent_date: sent,
            followup_after_days: days,
            last_touch: touch,
            note_doc: None,
        };
        let now = 100 * DAY;
        let ps = vec![
            // due = 70*DAY+14d = 84*DAY < now, touch 70d < due → кандидат
            mk(1, "pitched", Some(70 * DAY), 70 * DAY, 14),
            // доторкнулися після due → не кандидат
            mk(2, "followed_up", Some(70 * DAY), 90 * DAY, 14),
            // термінальний → не кандидат
            mk(3, "published", Some(0), 0, 14),
            // не відправлений (без sent_date) → не кандидат
            mk(4, "idea", None, 0, 14),
        ];
        assert_eq!(due_followups(&ps, now), vec![1]);
    }

    #[test]
    fn avg_time_in_status_pure_math() {
        let d = DAY;
        // p1: idea [0,2d], pitched [2d,12d], published [12d,20d]
        // p2: idea [0,4d], pitched [4d,14d], published [14d,20d]
        // p3: idea [0,1d], rejected [1d,20d]
        let e = vec![
            (1, "idea".into(), 0),
            (1, "pitched".into(), 2 * d),
            (1, "published".into(), 12 * d),
            (2, "idea".into(), 0),
            (2, "pitched".into(), 4 * d),
            (2, "published".into(), 14 * d),
            (3, "idea".into(), 0),
            (3, "rejected".into(), d),
            (4, "idea".into(), 0), // залишається в idea до now
        ];
        let now = 20 * d;
        let avg = avg_time_in_status(&e, now);
        let get = |s: &str| avg.iter().find(|(k, _)| k == s).map(|(_, v)| *v).unwrap();
        assert!((get("idea") - (2.0 + 4.0 + 1.0 + 20.0) / 4.0).abs() < 1e-9);
        assert!((get("pitched") - 10.0).abs() < 1e-9);
        assert!((get("published") - (8.0 + 6.0) / 2.0).abs() < 1e-9);
        // rejected триває до now: [1d, 20d] = 19 днів
        assert!((get("rejected") - 19.0).abs() < 1e-9);
    }
}
