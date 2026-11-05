//! Легкий синк для одного автора на кількох машинах (специфікація §Sync).
//!
//! Це НЕ повноцінний CRDT (Automerge/Yjs — велика зовнішня залежність, а
//! проєкт тримається на правилі «без нових crates»). Типова ситуація
//! письменника: ОДИН автор, 2–3 машини (настільний ПК + ноутбук), обмін
//! через спільну папку (Syncthing/USB/хмара) або флешку. Тому власний
//! легкий механізм однокористувацького multi-device merge:
//!
//! - **Векторні годинники**: кожен пристрій має `device_id` (uuid у
//!   `.skri/device.json`) і лічильник. На кожному доку в frontmatter
//!   (`vclock: dev=3;dev2=1`) зберігається вектор device → counter —
//!   файл є джерелом правди, годинник подорожує разом із документом.
//!   Лічильник інкрементується лише при ЛОКАЛЬНІЙ правці (`save_doc`);
//!   імпорт чужого пакета годинник НЕ збільшує (береться max по компонентах).
//! - **merge**: якщо один вектор строго домінує інший — перемагає
//!   домінуючий (fast-forward; звичайний випадок «новіша правка з одного
//!   пристрою»). Якщо вектори непорівнювані (справжня одночасна правка з
//!   двох пристроїв) — по-рядковий three-way merge відносно бази
//!   (`sync_base` = стан на момент останньої синхронізації; перший
//!   `save_doc` фіксує базу = стан ДО правки): рядки, змінені лише з
//!   одного боку, застосовуються обидва; рядки, змінені з обох боків
//!   по-різному, → маркери `<<<<<<< device-a` / `>>>>>>> device-b` прямо
//!   у файлі + рядки в `sync_conflicts` + алерт `kind='sync-conflict'`.
//! - **Пакет обміну**: `skri sync export <папка> <вихід.json>` пише JSON
//!   з векторами і тілами змінених доків (на флешку/в спільну папку),
//!   `skri sync import <папка> <пакет.json>` зливає його в локальний проєкт.
//! - **Журнал**: `sync_log(ts, device, doc_path, action
//!   merged|fast-forward|conflict)` (device = пристрій-джерело правки).
//!
//! Обмеження (свідомо): база для diff3 зберігається локально в БД — якщо
//! `.skri/index.db` стерти, base для доків без запису деградує до
//! «локальне тіло» (merge тоді забирає лише чужі зміни); маркери
//! конфліктів розв'язує автор вручну у файлі, наступний цикл
//! export/import зшиває стани остаточно.

use crate::frontmatter;
use crate::inkbridge::sha256_hex;
use crate::store::ProjectStore;
use crate::{Error, Result};
use rusqlite::{params, OptionalExtension};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Векторний годинник: device_id → лічильник.
pub type VClock = BTreeMap<String, u64>;

pub const PACKET_FORMAT: &str = "skri-sync-packet";
const PACKET_VERSION: i64 = 1;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sync_conflicts (
  id INTEGER PRIMARY KEY,
  doc_path TEXT NOT NULL,
  local_device TEXT NOT NULL,
  remote_device TEXT NOT NULL,
  local_line INTEGER,
  remote_line INTEGER,
  local_text TEXT NOT NULL,
  remote_text TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  resolved INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_sync_conflicts_doc ON sync_conflicts(doc_path);
CREATE TABLE IF NOT EXISTS sync_log (
  id INTEGER PRIMARY KEY,
  ts INTEGER NOT NULL,
  device TEXT NOT NULL,
  doc_path TEXT NOT NULL,
  action TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sync_log_doc ON sync_log(doc_path);
CREATE TABLE IF NOT EXISTS sync_base (
  doc_path TEXT PRIMARY KEY,
  base_body TEXT NOT NULL,
  base_clock TEXT NOT NULL
);
"#;

fn ensure_tables(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------- годинники

/// Розібрати `a=1;b=2` у вектор (зіпсовані компоненти пропускаються).
pub fn parse_clock(s: &str) -> VClock {
    let mut out = VClock::new();
    for part in s.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((dev, cnt)) = part.rsplit_once('=') {
            if let Ok(n) = cnt.trim().parse::<u64>() {
                let dev = dev.trim();
                if !dev.is_empty() {
                    out.insert(dev.to_string(), n);
                }
            }
        }
    }
    out
}

/// Вектор → `a=1;b=2` (порожній вектор → порожній рядок).
pub fn clock_to_string(clock: &VClock) -> String {
    clock
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(";")
}

/// `a >= b` по компонентах (відсутній у a компонент b = 0).
fn ge(a: &VClock, b: &VClock) -> bool {
    b.iter().all(|(k, bv)| a.get(k).copied().unwrap_or(0) >= *bv)
}

/// Порівняння векторних годинників.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockOrder {
    /// ідентичні
    Equal,
    /// `a` строго домінує `b` (a новіший)
    After,
    /// `b` строго домінує `a`
    Before,
    /// непорівнювані (справжня одночасна правка)
    Concurrent,
}

pub fn compare(a: &VClock, b: &VClock) -> ClockOrder {
    // a >= b по компонентах (відсутній компонент = 0)
    let a_ge = ge(a, b);
    let b_ge = ge(b, a);
    match (a_ge, b_ge) {
        (true, true) => ClockOrder::Equal,
        (true, false) => ClockOrder::After,
        (false, true) => ClockOrder::Before,
        (false, false) => ClockOrder::Concurrent,
    }
}

/// max по компонентах (стан після злиття двох гілок).
pub fn merge_clocks(a: &VClock, b: &VClock) -> VClock {
    let mut out = a.clone();
    for (k, v) in b {
        let e = out.entry(k.clone()).or_insert(0);
        if *v > *e {
            *e = *v;
        }
    }
    out
}

/// Ідентифікатор пристрою проєкту: `.skri/device.json` (створюється за
/// потреби — детермінований uuid-подібний рядок з SHA-256, std-only).
pub fn device_id(root: &Path) -> Result<String> {
    let path = root.join(".skri").join("device.json");
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(id) = v.get("device_id").and_then(|x| x.as_str()) {
                if !id.is_empty() {
                    return Ok(id.to_string());
                }
            }
        }
    }
    let seed = format!(
        "skri-device|{}|{}|{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        root.display()
    );
    let hex = sha256_hex(seed.as_bytes());
    // uuid-подібний формат 8-4-4-4-12
    let h: Vec<char> = hex.chars().collect();
    let id = format!(
        "{}-{}-{}-{}-{}",
        h[0..8].iter().collect::<String>(),
        h[8..12].iter().collect::<String>(),
        h[12..16].iter().collect::<String>(),
        h[16..20].iter().collect::<String>(),
        h[20..32].iter().collect::<String>(),
    );
    std::fs::create_dir_all(root.join(".skri"))?;
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::json!({ "device_id": id }))?,
    )?;
    Ok(id)
}

/// Прочитати векторний годинник документа з його frontmatter.
pub fn doc_clock(store: &ProjectStore, rel: &str) -> Result<VClock> {
    let rel = rel.replace('\\', "/");
    let raw = std::fs::read_to_string(store.root.join(&rel))
        .map_err(|_| Error::Invalid(format!("документ «{rel}» не існує")))?;
    Ok(read_file_clock(&raw))
}

fn read_file_clock(raw: &str) -> VClock {
    frontmatter::parse(raw)
        .meta
        .get("vclock")
        .map(|s| parse_clock(s))
        .unwrap_or_default()
}

/// Перебудувати файл: зберегти наявний frontmatter (без `vclock`),
/// підставити/додати рядок `vclock:` і покласти нове тіло.
fn compose_raw(existing: Option<&str>, body: &str, clock: &VClock) -> String {
    let clock_line = format!("vclock: {}", clock_to_string(clock));
    // canonical-тіло (як його повертає frontmatter::parse) починається з "\n"
    // — перенос закриваючого "---" входить у тіло. У ФАЙЛ пишемо тіло БЕЗ
    // ведучого "\n" (його дає сам перенос "---"), тоді parse(compose(c)) == c
    // і export/import не нарощує порожні рядки.
    let body_canon = if body.starts_with('\n') { body.to_string() } else { format!("\n{body}") };
    let body_on_disk = &body_canon[1..];
    if let Some(raw) = existing {
        let parsed = frontmatter::parse(raw);
        if parsed.body_start > 0 {
            // closing "---" займає 3 байти безпосередньо перед тілом
            let head_end = parsed.body_start.saturating_sub(3);
            let head = &raw[..head_end];
            let mut out = String::from("---\n");
            for line in head.lines().skip(1) {
                if line.trim_start().starts_with("vclock:") {
                    continue;
                }
                out.push_str(line);
                out.push('\n');
            }
            out.push_str(&clock_line);
            out.push('\n');
            out.push_str("---\n");
            out.push_str(body_on_disk);
            return out;
        }
    }
    format!("---\n{clock_line}\n---\n{body_on_disk}")
}

// ---------------------------------------------------------------- base/журнал

fn set_base(
    conn: &rusqlite::Connection,
    rel: &str,
    body: &str,
    clock: &VClock,
) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_base(doc_path, base_body, base_clock) VALUES (?1, ?2, ?3)
         ON CONFLICT(doc_path) DO UPDATE SET base_body=?2, base_clock=?3",
        params![rel, body, clock_to_string(clock)],
    )?;
    Ok(())
}

fn get_base(conn: &rusqlite::Connection, rel: &str) -> Result<Option<(String, VClock)>> {
    conn.query_row(
        "SELECT base_body, base_clock FROM sync_base WHERE doc_path = ?1",
        params![rel],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                parse_clock(&r.get::<_, String>(1)?),
            ))
        },
    )
    .optional()
    .map_err(Error::from)
}

fn log(conn: &rusqlite::Connection, device: &str, doc_path: &str, action: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_log(ts, device, doc_path, action) VALUES (?1, ?2, ?3, ?4)",
        params![now_epoch(), device, doc_path, action],
    )?;
    Ok(())
}

/// Рядок журналу синхронізації.
#[derive(Debug, Clone)]
pub struct LogRow {
    pub id: i64,
    pub ts: i64,
    pub device: String,
    pub doc_path: String,
    pub action: String,
}

/// Останні події журналу (свіжіші першими).
pub fn log_rows(store: &ProjectStore, limit: usize) -> Result<Vec<LogRow>> {
    ensure_tables(store.conn())?;
    let conn = store.conn();
    let mut stmt = conn.prepare(
        "SELECT id, ts, device, doc_path, action FROM sync_log
         ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit as i64], |r| {
            Ok(LogRow {
                id: r.get(0)?,
                ts: r.get(1)?,
                device: r.get(2)?,
                doc_path: r.get(3)?,
                action: r.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Нерозв'язані конфлікти синку.
#[derive(Debug, Clone)]
pub struct ConflictRow {
    pub id: i64,
    pub doc_path: String,
    pub local_device: String,
    pub remote_device: String,
    pub local_line: Option<i64>,
    pub remote_line: Option<i64>,
    pub local_text: String,
    pub remote_text: String,
}

pub fn conflicts(store: &ProjectStore) -> Result<Vec<ConflictRow>> {
    ensure_tables(store.conn())?;
    let conn = store.conn();
    let mut stmt = conn.prepare(
        "SELECT id, doc_path, local_device, remote_device, local_line, remote_line,
                local_text, remote_text
         FROM sync_conflicts WHERE resolved = 0 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ConflictRow {
                id: r.get(0)?,
                doc_path: r.get(1)?,
                local_device: r.get(2)?,
                remote_device: r.get(3)?,
                local_line: r.get(4)?,
                remote_line: r.get(5)?,
                local_text: r.get(6)?,
                remote_text: r.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

// ---------------------------------------------------------------- save_doc

/// ЛОКАЛЬНЕ збереження тіла документа (точка інтеграції для CLI/Tauri):
/// пише тіло, зберігаючи наявний frontmatter байт-в-байт, інкрементує
/// свій лічильник у векторному годиннику дока, за першим збереженням
/// фіксує базу для diff3 (стан до правки) і реіндексує проєкт.
/// Повертає новий векторний годинник.
pub fn save_doc(store: &ProjectStore, rel: &str, body: &str) -> Result<VClock> {
    ensure_tables(store.conn())?;
    let rel = rel.replace('\\', "/");
    let dev = device_id(&store.root)?;
    let path = store.root.join(&rel);
    let old_raw = std::fs::read_to_string(&path).ok();

    let old_clock = old_raw.as_deref().map(read_file_clock).unwrap_or_default();
    let mut clock = old_clock.clone();
    *clock.entry(dev).or_insert(0) += 1;

    // база для майбутнього three-way merge: стан ДО першої несинхронізованої
    // правки; під час наступних локальних збережень НЕ рухається (рухається
    // лише імпортом пакета — «останній спільний стан»)
    let has_base = get_base(store.conn(), &rel)?;
    if has_base.is_none() {
        let old_body = old_raw
            .as_deref()
            .map(|r| frontmatter::parse(r).body)
            .unwrap_or_default();
        set_base(store.conn(), &rel, &old_body, &old_clock)?;
    }

    let new_raw = compose_raw(old_raw.as_deref(), body, &clock);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, new_raw)?;
    store.reindex()?;
    Ok(clock)
}

// ---------------------------------------------------------------- three-way merge

/// Один блок правок сторони відносно бази: діапазон базових рядків
/// `[base_start, base_end)` замінюється на `lines` (base_start == base_end
/// → чиста вставка ПЕРЕД рядком base_start; порожні lines → видалення).
#[derive(Debug, Clone)]
struct SideOp {
    base_start: usize,
    base_end: usize,
    /// 0-based номер першого рядка сторони в блоці
    side_start: usize,
    lines: Vec<String>,
}

impl SideOp {
    fn is_insert_at(&self, i: usize) -> bool {
        self.base_start == self.base_end && self.base_start == i
    }
    fn covers(&self, i: usize) -> bool {
        self.base_start < self.base_end && self.base_start <= i && i < self.base_end
    }
}

/// LCS-пари (індекси збігів) між двома послідовностями рядків.
/// Стеля розміру як у retcon::diff_lines: добуток > 4 млн → без пар
/// (усе вважається зміненим — консервативно).
fn lcs_pairs(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 || n.saturating_mul(m) > 4_000_000 {
        return Vec::new();
    }
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
            out.push((i, j));
            i += 1;
            j += 1;
        } else if lcs[idx(i + 1, j)] >= lcs[idx(i, j + 1)] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// Блоки правок сторони відносно бази (між збігами LCS).
fn side_ops(base: &[&str], side: &[&str]) -> Vec<SideOp> {
    let pairs = lcs_pairs(base, side);
    let mut ops = Vec::new();
    let (mut pb, mut ps) = (0usize, 0usize);
    for &(bi, si) in &pairs {
        if bi > pb || si > ps {
            ops.push(SideOp {
                base_start: pb,
                base_end: bi,
                side_start: ps,
                lines: side[ps..si].iter().map(|s| s.to_string()).collect(),
            });
        }
        pb = bi + 1;
        ps = si + 1;
    }
    if base.len() > pb || side.len() > ps {
        ops.push(SideOp {
            base_start: pb,
            base_end: base.len(),
            side_start: ps,
            lines: side[ps..].iter().map(|s| s.to_string()).collect(),
        });
    }
    ops
}

/// Конфліктна область: пара фрагментів (локальний vs віддалений).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictRegion {
    /// 1-based номер першого рядка фрагмента у ЛОКАЛЬНОМУ тілі
    pub local_line: usize,
    /// 1-based номер першого рядка фрагмента у ВІДДАЛЕНОМУ тілі
    pub remote_line: usize,
    pub local_text: String,
    pub remote_text: String,
}

/// Результат three-way merge.
#[derive(Debug, Clone)]
pub struct MergeOutcome {
    pub body: String,
    pub conflicts: Vec<ConflictRegion>,
}

/// По-рядковий three-way merge (std-only): база + локальна версія +
/// віддалена версія. Рядки, змінені лише з одного боку, застосовуються;
/// однакові зміни обох боків — один раз; різні зміни того самого місця →
/// маркери `<<<<<<< local-dev / ======= / >>>>>>> remote-dev`.
///
/// Правило «обидва змінили один і той самий рядок» = обидва боки мають
/// блок правок, що покриває той самий діапазон базових рядків, з різним
/// вмістом (включно з «один видалив, інший лишив»).
pub fn merge3(
    base: &str,
    local: &str,
    remote: &str,
    local_dev: &str,
    remote_dev: &str,
) -> MergeOutcome {
    let bl: Vec<&str> = base.lines().collect();
    let ll: Vec<&str> = local.lines().collect();
    let rl: Vec<&str> = remote.lines().collect();
    let lops = side_ops(&bl, &ll);
    let rops = side_ops(&bl, &rl);

    let mut out = String::new();
    let mut conflicts = Vec::new();
    let (mut li, mut ri) = (0usize, 0usize);
    let mut i = 0usize;
    while i <= bl.len() {
        // вставки в точці i: беруться обидва (однакові — один раз)
        let mut lins: Vec<String> = Vec::new();
        let mut rins: Vec<String> = Vec::new();
        while li < lops.len() && lops[li].is_insert_at(i) {
            lins.extend(lops[li].lines.iter().cloned());
            li += 1;
        }
        while ri < rops.len() && rops[ri].is_insert_at(i) {
            rins.extend(rops[ri].lines.iter().cloned());
            ri += 1;
        }
        if lins != rins {
            for l in &lins {
                out.push_str(l);
                out.push('\n');
            }
            for l in &rins {
                out.push_str(l);
                out.push('\n');
            }
        } else {
            for l in &lins {
                out.push_str(l);
                out.push('\n');
            }
        }
        if i == bl.len() {
            break;
        }
        let lb = lops.get(li).filter(|op| op.covers(i));
        let rb = rops.get(ri).filter(|op| op.covers(i));
        match (lb, rb) {
            (None, None) => {
                out.push_str(bl[i]);
                out.push('\n');
                i += 1;
            }
            (Some(op), None) | (None, Some(op)) => {
                for l in &op.lines {
                    out.push_str(l);
                    out.push('\n');
                }
                i = op.base_end;
                if lb.is_some() {
                    li += 1;
                } else {
                    ri += 1;
                }
            }
            (Some(lo), Some(ro)) => {
                if lo.lines == ro.lines {
                    for l in &lo.lines {
                        out.push_str(l);
                        out.push('\n');
                    }
                } else {
                    out.push_str("<<<<<<< ");
                    out.push_str(local_dev);
                    out.push('\n');
                    for l in &lo.lines {
                        out.push_str(l);
                        out.push('\n');
                    }
                    out.push_str("=======\n");
                    for l in &ro.lines {
                        out.push_str(l);
                        out.push('\n');
                    }
                    out.push_str(">>>>>>> ");
                    out.push_str(remote_dev);
                    out.push('\n');
                    conflicts.push(ConflictRegion {
                        local_line: lo.side_start + 1,
                        remote_line: ro.side_start + 1,
                        local_text: lo.lines.join("\n"),
                        remote_text: ro.lines.join("\n"),
                    });
                }
                i = lo.base_end.max(ro.base_end);
                li += 1;
                ri += 1;
            }
        }
    }
    MergeOutcome { body: out, conflicts }
}

// ---------------------------------------------------------------- пакет обміну

/// Звіт експорту пакета.
#[derive(Debug, Clone)]
pub struct ExportReport {
    pub device: String,
    pub docs: usize,
    pub paths: Vec<String>,
    pub out_path: PathBuf,
}

/// `skri sync export`: JSON-пакет з векторами і тілами змінених доків
/// (тіло без frontmatter; змінені = хеш тіла ≠ хеш бази або бази немає).
pub fn export_packet(store: &ProjectStore, out_path: &Path) -> Result<ExportReport> {
    ensure_tables(store.conn())?;
    let dev = device_id(&store.root)?;
    let conn = store.conn();
    let mut docs_json: Vec<serde_json::Value> = Vec::new();
    let mut paths: Vec<String> = Vec::new();

    for entry in walkdir::WalkDir::new(&store.root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".skri" && e.file_name() != ".git")
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&store.root)
            .map_err(|_| Error::Invalid("шлях поза проєктом".into()))?
            .to_string_lossy()
            .replace('\\', "/");
        let raw = match std::fs::read_to_string(entry.path()) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let parsed = frontmatter::parse(&raw);
        let clock = read_file_clock(&raw);
        let changed = match get_base(conn, &rel)? {
            None => true,
            Some((base_body, _)) => {
                sha256_hex(parsed.body.as_bytes()) != sha256_hex(base_body.as_bytes())
            }
        };
        if !changed {
            continue;
        }
        let clock_json: serde_json::Map<String, serde_json::Value> = clock
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::json!(v)))
            .collect();
        docs_json.push(serde_json::json!({
            "path": rel,
            "clock": serde_json::Value::Object(clock_json),
            "body": parsed.body,
        }));
        paths.push(rel);
    }

    let packet = serde_json::json!({
        "format": PACKET_FORMAT,
        "version": PACKET_VERSION,
        "device": dev,
        "created_at": now_epoch(),
        "docs": docs_json,
    });
    std::fs::create_dir_all(
        out_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    std::fs::write(out_path, serde_json::to_string_pretty(&packet)?)?;
    Ok(ExportReport {
        device: dev,
        docs: docs_json.len(),
        paths,
        out_path: out_path.to_path_buf(),
    })
}

/// Звіт імпорту пакета.
#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub device: String,
    pub fast_forward: usize,
    pub merged: usize,
    pub conflicts: usize,
    pub skipped: usize,
    pub conflict_docs: Vec<String>,
}

/// `skri sync import`: злити пакет у локальний проєкт.
/// Логіка на док: fast-forward (домінування) → three-way merge
/// (непорівнювані) → skip (локальний новіший/ідентичний).
pub fn import_packet(store: &ProjectStore, packet_path: &Path) -> Result<ImportReport> {
    ensure_tables(store.conn())?;
    let raw = std::fs::read_to_string(packet_path)?;
    let packet: serde_json::Value = serde_json::from_str(&raw)?;
    if packet.get("format").and_then(|v| v.as_str()) != Some(PACKET_FORMAT) {
        return Err(Error::Invalid(
            "це не пакет skri-sync (немає format = skri-sync-packet)".into(),
        ));
    }
    let remote_dev = packet
        .get("device")
        .and_then(|v| v.as_str())
        .unwrap_or("невідомий-пристрій")
        .to_string();
    let local_dev = device_id(&store.root)?;
    let mut rep = ImportReport {
        device: remote_dev.clone(),
        ..Default::default()
    };
    let docs = packet
        .get("docs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for d in docs {
        let rel = d
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .replace('\\', "/");
        if rel.is_empty() || rel.contains("..") {
            continue; // підозрілий/порожній шлях — пропускаємо
        }
        let mut incoming_clock = VClock::new();
        if let Some(obj) = d.get("clock").and_then(|v| v.as_object()) {
            for (k, v) in obj {
                if let Some(n) = v.as_u64() {
                    incoming_clock.insert(k.clone(), n);
                }
            }
        }
        let incoming_body = d
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let conn = store.conn();

        let local_raw = std::fs::read_to_string(store.root.join(&rel)).ok();
        let Some(local) = local_raw else {
            // дока локально немає → створюємо (fast-forward add)
            let new_raw = compose_raw(None, &incoming_body, &incoming_clock);
            let path = store.root.join(&rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, new_raw)?;
            set_base(conn, &rel, &incoming_body, &incoming_clock)?;
            log(conn, &remote_dev, &rel, "fast-forward")?;
            rep.fast_forward += 1;
            continue;
        };

        let local_clock = read_file_clock(&local);
        let local_body = frontmatter::parse(&local).body;
        match compare(&local_clock, &incoming_clock) {
            ClockOrder::After => {
                rep.skipped += 1; // локальний новіший
            }
            ClockOrder::Equal if local_body == incoming_body => {
                rep.skipped += 1; // ідентичні
            }
            ClockOrder::Before => {
                let new_raw = compose_raw(Some(&local), &incoming_body, &incoming_clock);
                std::fs::write(store.root.join(&rel), new_raw)?;
                set_base(conn, &rel, &incoming_body, &incoming_clock)?;
                log(conn, &remote_dev, &rel, "fast-forward")?;
                rep.fast_forward += 1;
            }
            // непорівнювані (або однакові годинники, але різні тіла)
            _ => {
                // база: останній спільний стан; без запису — деградація до
                // «base = локальне тіло» (забираємо лише чужі зміни)
                let (base_body, _) =
                    get_base(conn, &rel)?.unwrap_or_else(|| (local_body.clone(), local_clock.clone()));
                let outcome = merge3(&base_body, &local_body, &incoming_body, &local_dev, &remote_dev);
                let merged_clock = merge_clocks(&local_clock, &incoming_clock);
                let new_raw = compose_raw(Some(&local), &outcome.body, &merged_clock);
                std::fs::write(store.root.join(&rel), new_raw)?;
                if outcome.conflicts.is_empty() {
                    set_base(conn, &rel, &outcome.body, &merged_clock)?;
                    log(conn, &remote_dev, &rel, "merged")?;
                    rep.merged += 1;
                } else {
                    let now = now_epoch();
                    for c in &outcome.conflicts {
                        conn.execute(
                            "INSERT INTO sync_conflicts(doc_path, local_device, remote_device,
                               local_line, remote_line, local_text, remote_text, created_at)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                            params![
                                &rel,
                                &local_dev,
                                &remote_dev,
                                c.local_line as i64,
                                c.remote_line as i64,
                                &c.local_text,
                                &c.remote_text,
                                now
                            ],
                        )?;
                    }
                    raise_conflict_alert(conn, &rel, &local_dev, &remote_dev, outcome.conflicts.len(), now)?;
                    // база = віддалена версія: після розв'язання маркерів і
                    // наступного збереження локальна правка коректно
                    // перезливається з непорушеним віддаленим станом
                    set_base(conn, &rel, &incoming_body, &incoming_clock)?;
                    log(conn, &remote_dev, &rel, "conflict")?;
                    rep.conflicts += 1;
                    rep.conflict_docs.push(rel.clone());
                }
            }
        }
    }

    store.reindex()?;
    Ok(rep)
}

/// Алерт `kind='sync-conflict'` — ідемпотентно (один відкритий алерт на док).
fn raise_conflict_alert(
    conn: &rusqlite::Connection,
    rel: &str,
    local_dev: &str,
    remote_dev: &str,
    n: usize,
    now: i64,
) -> Result<()> {
    // погасити попередні відкриті sync-conflict алерти для цього дока
    let mut stmt = conn.prepare(
        "SELECT id, payload_json FROM alerts WHERE kind='sync-conflict' AND resolved=0",
    )?;
    let stale: Vec<i64> = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .filter_map(|r| r.ok())
        .filter(|(_, payload)| {
            serde_json::from_str::<serde_json::Value>(payload)
                .ok()
                .and_then(|v| v.get("doc").and_then(|d| d.as_str()).map(|s| s == rel))
                .unwrap_or(false)
        })
        .map(|(id, _)| id)
        .collect();
    for id in stale {
        conn.execute("DELETE FROM alerts WHERE id = ?1", params![id])?;
    }
    let payload = serde_json::json!({
        "doc": rel,
        "local_device": local_dev,
        "remote_device": remote_dev,
        "conflicts": n,
        "message": format!(
            "конфлікт синку в «{rel}»: {n} конфліктних фрагментів, маркери у файлі"
        ),
    });
    conn.execute(
        "INSERT INTO alerts(kind, entity_id, payload_json, created_at)
         VALUES ('sync-conflict', NULL, ?1, ?2)",
        params![serde_json::to_string(&payload)?, now],
    )?;
    Ok(())
}

// ---------------------------------------------------------------- status

/// Годинник одного дока для звіту.
#[derive(Debug, Clone)]
pub struct DocClock {
    pub path: String,
    pub clock: String,
}

/// Зведення для `skri sync status`.
#[derive(Debug, Clone)]
pub struct SyncStatus {
    pub device_id: String,
    pub docs: Vec<DocClock>,
    pub log: Vec<LogRow>,
}

pub fn status(store: &ProjectStore) -> Result<SyncStatus> {
    ensure_tables(store.conn())?;
    let dev = device_id(&store.root)?;
    let mut docs: Vec<DocClock> = Vec::new();
    for entry in walkdir::WalkDir::new(&store.root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".skri" && e.file_name() != ".git")
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|e| e.to_str()) != Some("md")
        {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&store.root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if let Ok(raw) = std::fs::read_to_string(entry.path()) {
            let clock = clock_to_string(&read_file_clock(&raw));
            docs.push(DocClock { path: rel, clock });
        }
    }
    docs.sort_by(|a, b| a.path.cmp(&b.path));
    let mut log = log_rows(store, 20)?;
    log.reverse(); // хронологічний порядок у звіті
    Ok(SyncStatus { device_id: dev, docs, log })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_parse_format_compare() {
        let c = parse_clock("a=3; b=1; сміття; =5; c=x");
        assert_eq!(c.get("a"), Some(&3));
        assert_eq!(c.get("b"), Some(&1));
        assert_eq!(c.len(), 2);
        assert_eq!(clock_to_string(&c), "a=3;b=1");

        let a = parse_clock("a=2;b=1");
        let b = parse_clock("a=1;b=1");
        let c1 = parse_clock("a=1;c=1");
        assert_eq!(compare(&a, &b), ClockOrder::After);
        assert_eq!(compare(&b, &a), ClockOrder::Before);
        assert_eq!(compare(&a, &a), ClockOrder::Equal);
        assert_eq!(compare(&a, &c1), ClockOrder::Concurrent);
        let m = merge_clocks(&a, &c1);
        assert_eq!(clock_to_string(&m), "a=2;b=1;c=1");
        // порожній вектор (док без save_doc) — найстаріший: {a:1} домінує {}
        assert_eq!(compare(&VClock::new(), &parse_clock("a=1")), ClockOrder::Before);
        // компонент, відсутний в одному з векторів, теж враховується
        assert_eq!(compare(&parse_clock("a=1"), &parse_clock("a=1;b=1")), ClockOrder::Before);
        assert_eq!(compare(&parse_clock("a=1;b=1"), &parse_clock("a=1")), ClockOrder::After);
    }

    #[test]
    fn merge3_nonconflicting_and_conflicting() {
        // різні рядки → автозлиття обох
        let base = "один\nдва\nтри";
        let local = "один\nДВА\nтри";
        let remote = "один\nдва\nТРИ";
        let m = merge3(base, local, remote, "dev-a", "dev-b");
        assert!(m.conflicts.is_empty());
        assert_eq!(m.body, "один\nДВА\nТРИ\n");

        // той самий рядок змінено обома по-різному → маркери
        let remote2 = "один\nДВА ІНАКШЕ\nтри";
        let m2 = merge3(base, local, remote2, "dev-a", "dev-b");
        assert_eq!(m2.conflicts.len(), 1);
        assert!(m2.body.contains("<<<<<<< dev-a"));
        assert!(m2.body.contains("======="));
        assert!(m2.body.contains(">>>>>>> dev-b"));
        assert!(m2.body.contains("ДВА\n"));
        assert!(m2.body.contains("ДВА ІНАКШЕ"));

        // обидва додали різні абзаци в кінець → беруться обидва
        let m3 = merge3("base", "base\nла", "base\nб", "a", "b");
        assert!(m3.conflicts.is_empty());
        assert!(m3.body.contains("ла"));
        assert!(m3.body.contains("б"));
    }
}
