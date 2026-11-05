//! Роялті-дашборд з локальних CSV (§5.6 «роялті»).
//!
//! Імпорт CSV-звітів KDP / Draft2Digital / Patreon у таблицю
//! `royalty_rows(source_file, date, platform, book, units, revenue_cents)`
//! з гнучким парсером заголовків (різні варіанти назв колонок, суми з `$`
//! і тисячними комами). Невідомі колонки ігноруються; рядки без суми —
//! пропускаються. Агрегати: по платформах, по книгах, по місяцях.
//!
//! Реальної мережі нема: тільки локальні CSV, які автор завантажив з
//! дашбордів платформ вручну. Імпорт ідемпотентний: повторний імпорт того
//! ж файла перезаписує його рядки (source_file = ключ).
//!
//! Залізне правило №1: агрегати перераховуються з таблиці будь-коли;
//! вихідна правда — CSV-файли автора.

use crate::store::ProjectStore;
use crate::{Error, Result};
use rusqlite::{params, Connection};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS royalty_rows (
  id INTEGER PRIMARY KEY,
  source_file TEXT NOT NULL,
  row_idx INTEGER NOT NULL,
  date TEXT NOT NULL DEFAULT '',
  platform TEXT NOT NULL DEFAULT '',
  book TEXT NOT NULL DEFAULT '',
  units INTEGER NOT NULL DEFAULT 0,
  revenue_cents INTEGER NOT NULL DEFAULT 0
);";

fn ensure_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

/// Один рядок роялті.
#[derive(Debug, Clone, PartialEq)]
pub struct RoyaltyRow {
    pub source_file: String,
    pub date: String,
    pub platform: String,
    pub book: String,
    pub units: i64,
    pub revenue_cents: i64,
}

/// Звіт імпорту.
#[derive(Debug, Default)]
pub struct ImportReport {
    pub source_file: String,
    pub rows: usize,
    pub skipped: usize,
    /// які колонки чим розпізнано (header → поле)
    pub mapping: Vec<(String, String)>,
}

// --- CSV (std-only, з підтримкою лапок) ---

/// Розібрати CSV на рядки-комірки: лапки `"a,b"`, подвійні лапки `""`,
/// CRLF. Простий і достатній для звітів KDP/D2D/Patreon.
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' => in_quotes = true,
            ',' => {
                row.push(std::mem::take(&mut field));
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                out.push(std::mem::take(&mut row));
            }
            '\r' => {}
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        out.push(row);
    }
    out.retain(|r| !(r.len() == 1 && r[0].trim().is_empty()));
    out
}

/// Синоніми колонок (lowercase). Перший точний збіг виграє, потім
/// частковий (contains) у порядку списку.
const COL_BOOK: &[&str] = &["book", "title", "product", "product name", "work", "book title"];
const COL_DATE: &[&str] = &["date", "sale date", "sales date", "period", "month", "transaction date", "order date"];
const COL_PLATFORM: &[&str] = &["platform", "marketplace", "channel", "source", "store", "vendor", "distributor"];
const COL_REVENUE: &[&str] = &["revenue", "royalty", "royalties", "earnings", "amount", "total", "net", "payout", "net revenue", "total earnings"];
const COL_UNITS: &[&str] = &["units", "unit sales", "units sold", "quantity", "qty", "net units", "sales"];

/// Розпізнати колонки за заголовками: (індекс комірки) для кожного поля.
fn map_columns(header: &[String]) -> Vec<(&'static str, usize)> {
    let lower: Vec<String> = header.iter().map(|h| h.trim().to_lowercase()).collect();
    let mut out = Vec::new();
    for (field, names) in [
        ("book", COL_BOOK),
        ("date", COL_DATE),
        ("platform", COL_PLATFORM),
        ("revenue", COL_REVENUE),
        ("units", COL_UNITS),
    ] {
        let mut hit: Option<usize> = None;
        // точний збіг (у порядку синонімів — пріоритет)
        for name in names {
            if let Some(i) = lower.iter().position(|h| h == name) {
                hit = Some(i);
                break;
            }
        }
        // частковий (header на кшталт «Royalty (USD)»)
        if hit.is_none() {
            for name in names {
                if let Some(i) = lower.iter().position(|h| h.contains(name)) {
                    hit = Some(i);
                    break;
                }
            }
        }
        if let Some(i) = hit {
            out.push((field, i));
        }
    }
    out
}

/// Суму «$1,234.56» / «1 234,56 €» / «1234» → центи.
///
/// Якщо є і `.` і `,` — останній роздільник десятковий, інший тисячний.
/// Якщо тільки `,`: `1,234` (групи по 3) — тисячний, інакше десятковий.
pub fn parse_money(s: &str) -> Option<i64> {
    let t: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && !"$€£₴¥".contains(*c) && !matches!(c, 'u' | 's' | 'd' | 'e' | 'h' | 'r' | 'U' | 'S' | 'D' | 'E' | 'H' | 'R'))
        .collect();
    // обережно: фільтр літер з'їдає валютні коди (USD/EUR/UAH), але й
    // випадкові літери; далі парсимо лише цифри/розділювачі
    let digits: String = t
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',' || *c == '-')
        .collect();
    if digits.is_empty() || digits == "-" {
        return None;
    }
    let neg = digits.starts_with('-');
    let body = digits.trim_start_matches('-');
    let norm = if body.contains('.') && body.contains(',') {
        // останній роздільник — десятковий
        let (dot, comma) = (body.rfind('.'), body.rfind(','));
        let (dec, thou) = if dot > comma { ('.', ',') } else { (',', '.') };
        body.replace(thou, "").replace(dec, ".")
    } else if body.contains(',') {
        if is_thousands(body, ',') {
            body.replace(',', "")
        } else {
            body.replace(',', ".")
        }
    } else {
        body.to_string()
    };
    let v: f64 = norm.parse().ok()?;
    let cents = (v * 100.0).round() as i64;
    Some(if neg { -cents } else { cents })
}

/// `1,234` / `12,345,678` — групи по 3 цифри після кожної коми?
fn is_thousands(s: &str, sep: char) -> bool {
    let mut parts = s.split(sep);
    parts.next(); // перша група будь-яка (1..3 цифри)
    for p in parts {
        if p.len() != 3 || !p.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }
    true
}

/// Ціле з комірки одиниць («5», «1,205»).
fn parse_units(s: &str) -> Option<i64> {
    let t: String = s.chars().filter(|c| c.is_ascii_digit() || *c == '-').collect();
    t.parse().ok()
}

const MONTHS_EN: &[&str] = &[
    "january", "february", "march", "april", "may", "june",
    "july", "august", "september", "october", "november", "december",
];

/// Нормалізувати дату: → (дата YYYY-MM-DD | як є, місяць YYYY-MM | «»).
pub fn normalize_date(raw: &str) -> (String, String) {
    let t = raw.trim();
    // ISO
    if let Some(m) = iso_month(t) {
        return (t.to_string(), m);
    }
    // 01/15/2026 або 1/5/2026 (M/D/Y — звіти KDP/D2D US)
    let slash: Vec<&str> = t.split('/').collect();
    if slash.len() == 3 {
        let (m, d, y) = (slash[0].trim(), slash[1].trim(), slash[2].trim());
        if y.len() == 4 && m.parse::<u32>().map(|v| (1..=12).contains(&v)).unwrap_or(false) {
            let date = format!("{y}-{:02}-{:02}", m.parse::<u32>().unwrap_or(0), d.parse::<u32>().unwrap_or(0));
            return (date.clone(), date[..7].to_string());
        }
    }
    // 15.01.2026 (D.M.Y — українські/європейські звіти)
    let dot: Vec<&str> = t.split('.').collect();
    if dot.len() == 3 {
        let (d, m, y) = (dot[0].trim(), dot[1].trim(), dot[2].trim());
        if y.len() == 4 && m.parse::<u32>().map(|v| (1..=12).contains(&v)).unwrap_or(false) {
            let date = format!("{y}-{:02}-{:02}", m.parse::<u32>().unwrap_or(0), d.parse::<u32>().unwrap_or(0));
            return (date.clone(), date[..7].to_string());
        }
    }
    // «January 2026» / «Jan 2026»
    let low = t.to_lowercase();
    for (i, name) in MONTHS_EN.iter().enumerate() {
        if low.contains(name)
            || low.starts_with(&name[..3])
        {
            let year: String = low
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            if year.len() == 4 {
                let m = format!("{year}-{:02}", i + 1);
                return (m.clone(), m);
            }
        }
    }
    (t.to_string(), String::new())
}

fn iso_month(t: &str) -> Option<String> {
    if t.len() >= 7
        && t.as_bytes()[4] == b'-'
        && t[..4].chars().all(|c| c.is_ascii_digit())
        && t[5..7].chars().all(|c| c.is_ascii_digit())
    {
        let m: u32 = t[5..7].parse().ok()?;
        if (1..=12).contains(&m) {
            return Some(t[..7].to_string());
        }
    }
    None
}

// --- імпорт ---

/// Імпортувати CSV-звіт у royalty_rows (ідемпотентно: перезапис рядків
/// того ж source_file). Рядки без розпізнаної суми пропускаються.
pub fn import_csv(store: &ProjectStore, csv_path: &std::path::Path) -> Result<ImportReport> {
    let text = std::fs::read_to_string(csv_path)?;
    let source_file = csv_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::Invalid("некоректне ім'я CSV-файла".into()))?
        .to_string();
    let table = parse_csv(&text);
    if table.is_empty() {
        return Err(Error::Invalid("порожній CSV".into()));
    }
    let header = &table[0];
    let cols = map_columns(header);
    let mapping: Vec<(String, String)> = cols
        .iter()
        .map(|(f, i)| (header[*i].clone(), f.to_string()))
        .collect();

    let conn = store.conn();
    ensure_tables(conn)?;
    conn.execute("DELETE FROM royalty_rows WHERE source_file=?1", params![&source_file])?;

    let mut report = ImportReport {
        source_file: source_file.clone(),
        mapping,
        ..Default::default()
    };
    let mut row_idx = 0i64;
    for cells in &table[1..] {
        let get = |field: &str| -> Option<String> {
            cols.iter()
                .find(|(f, _)| *f == field)
                .and_then(|(_, i)| cells.get(*i))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };
        let revenue = get("revenue").and_then(|s| parse_money(&s));
        if revenue.is_none() {
            report.skipped += 1;
            continue;
        }
        let (date, _) = get("date")
            .map(|s| normalize_date(&s))
            .unwrap_or_else(|| (String::new(), String::new()));
        let row = RoyaltyRow {
            source_file: source_file.clone(),
            date,
            platform: get("platform").unwrap_or_default(),
            book: get("book").unwrap_or_default(),
            units: get("units").and_then(|s| parse_units(&s)).unwrap_or(0),
            revenue_cents: revenue.unwrap_or(0),
        };
        conn.execute(
            "INSERT INTO royalty_rows(source_file, row_idx, date, platform, book, units, revenue_cents)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                row.source_file,
                row_idx,
                row.date,
                row.platform,
                row.book,
                row.units,
                row.revenue_cents
            ],
        )?;
        row_idx += 1;
        report.rows += 1;
    }
    Ok(report)
}

// --- агрегати ---

/// Розріз агрегування.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    Platform,
    Book,
    Month,
}

impl By {
    pub fn parse(s: &str) -> Result<By> {
        match s {
            "platform" => Ok(By::Platform),
            "book" => Ok(By::Book),
            "month" => Ok(By::Month),
            other => Err(Error::Invalid(format!(
                "невідомий розріз «{other}» (platform|book|month)"
            ))),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            By::Platform => "платформа",
            By::Book => "книга",
            By::Month => "місяць",
        }
    }
}

/// Агрегат одного розрізу.
#[derive(Debug, Clone, PartialEq)]
pub struct Agg {
    pub key: String,
    pub units: i64,
    pub revenue_cents: i64,
}

/// Агрегувати рядки: сума одиниць і центів по ключу, відсортовано.
pub fn aggregate(rows: &[RoyaltyRow], by: By) -> Vec<Agg> {
    let mut map: std::collections::BTreeMap<String, (i64, i64)> = std::collections::BTreeMap::new();
    for r in rows {
        let key = match by {
            By::Platform => {
                if r.platform.is_empty() {
                    "(невідома платформа)".to_string()
                } else {
                    r.platform.clone()
                }
            }
            By::Book => {
                if r.book.is_empty() {
                    "(без назви)".to_string()
                } else {
                    r.book.clone()
                }
            }
            By::Month => {
                let m = normalize_date(&r.date).1;
                if m.is_empty() {
                    "(без дати)".to_string()
                } else {
                    m
                }
            }
        };
        let e = map.entry(key).or_insert((0, 0));
        e.0 += r.units;
        e.1 += r.revenue_cents;
    }
    map.into_iter()
        .map(|(key, (units, revenue_cents))| Agg { key, units, revenue_cents })
        .collect()
}

/// Усі рядки з таблиці (для summary і майбутнього UI).
pub fn rows(store: &ProjectStore) -> Result<Vec<RoyaltyRow>> {
    let conn = store.conn();
    ensure_tables(conn)?;
    let mut stmt = conn.prepare(
        "SELECT source_file, date, platform, book, units, revenue_cents
         FROM royalty_rows ORDER BY source_file, row_idx",
    )?;
    let out = stmt
        .query_map([], |r| {
            Ok(RoyaltyRow {
                source_file: r.get(0)?,
                date: r.get(1)?,
                platform: r.get(2)?,
                book: r.get(3)?,
                units: r.get(4)?,
                revenue_cents: r.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(out)
}

/// Дашборд: агрегат по обраному розрізу + загальний підсумок.
pub fn summary(store: &ProjectStore, by: By) -> Result<(Vec<Agg>, i64, i64)> {
    let all = rows(store)?;
    let total_units = all.iter().map(|r| r.units).sum();
    let total_cents = all.iter().map(|r| r.revenue_cents).sum();
    Ok((aggregate(&all, by), total_units, total_cents))
}

/// Форматування центів у «$1,234.56» (для CLI).
pub fn fmt_money(cents: i64) -> String {
    let neg = cents < 0;
    let v = cents.unsigned_abs();
    format!(
        "{}${}.{:02}",
        if neg { "-" } else { "" },
        thousands(v / 100),
        v % 100
    )
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_parsing_us_and_eu() {
        assert_eq!(parse_money("$1,234.56"), Some(123_456));
        assert_eq!(parse_money("$0.99"), Some(99));
        assert_eq!(parse_money("1,234.56 USD"), Some(123_456));
        assert_eq!(parse_money("-12.30"), Some(-1_230));
        assert_eq!(parse_money("1.234,56"), Some(123_456)); // європейський формат
        assert_eq!(parse_money("2,350"), Some(235_000)); // тисячні без дробів
        assert_eq!(parse_money("2,35"), Some(235)); // десяткова кома
        assert_eq!(parse_money(""), None);
        assert_eq!(parse_money("n/a"), None);
    }

    #[test]
    fn csv_quotes_and_dates() {
        let rows = parse_csv("a,\"b,c\"\"d\",e\n1,2,3\n");
        assert_eq!(rows[0], vec!["a", "b,c\"d", "e"]);
        assert_eq!(rows[1], vec!["1", "2", "3"]);
        assert_eq!(normalize_date("2026-01-15"), ("2026-01-15".into(), "2026-01".into()));
        assert_eq!(normalize_date("1/15/2026").1, "2026-01");
        assert_eq!(normalize_date("15.01.2026").1, "2026-01");
        assert_eq!(normalize_date("January 2026").1, "2026-01");
        assert_eq!(normalize_date("невідомо").1, "");
    }

    #[test]
    fn money_formatting() {
        assert_eq!(fmt_money(123_456), "$1,234.56");
        assert_eq!(fmt_money(99), "$0.99");
        assert_eq!(fmt_money(-1_230), "-$12.30");
        assert_eq!(fmt_money(0), "$0.00");
    }
}
