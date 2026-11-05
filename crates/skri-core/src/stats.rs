//! LitRPG Progression Engine — event-sourcing статів персонажів (§5.3).
//!
//! Стан персонажа = сума дельт по главах. Дельти автор пише прямо в рукописі
//! рядками виду `[stat: Кальдрін рівень +2]`, `[stat: Кальдрін hp -30]`,
//! `[stat: Кальдрін xp += 150]` (семантика +=/-=, значення int, атрибут будь-який).
//!
//! - `stat_snapshots(entity_name, attr, chapter, value)` — кумулятивний стан
//!   ПІСЛЯ глави `chapter`; перебудовується ідемпотентно з manuscript/.
//! - Ретроактивна правка (редагували дельту в ранній главі) → діф старих vs
//!   нових снапшотів по кожній главі → алерт `kind='stat-change'`.
//! - Stat-error radar: від'ємний hp/mp у будь-якому снапшоті → алерт
//!   `kind='stat-error'`.
//!
//! Залізне правило №1 дотримане: снапшоти перебудовуються з файлів будь-коли.

use crate::indexer;
use crate::store::ProjectStore;
use crate::Result;
use rusqlite::{params, OptionalExtension};
use std::collections::{BTreeMap, HashMap};

/// Одна дельта, спарсена з розмітки `[stat: ...]`.
#[derive(Debug, Clone, PartialEq)]
pub struct StatDelta {
    pub entity: String,
    pub attr: String,
    /// додатне = приріст, від'ємне = втрата (+=/-= семантика)
    pub value: i64,
}

/// Рядок stat sheet: стан персонажа, як читач бачить у главі N.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetRow {
    pub entity: String,
    pub attr: String,
    pub value: i64,
}

/// Зміна одного снапшота по главі (None = рядок з'явився/зник).
#[derive(Debug, Clone, PartialEq)]
pub struct DiffChange {
    pub chapter: i64,
    pub old: Option<i64>,
    pub new: Option<i64>,
}

/// Звіт перебудови.
#[derive(Debug, Default)]
pub struct RebuildReport {
    /// спарсених дельт
    pub deltas: usize,
    /// рядків у stat_snapshots
    pub snapshots: usize,
    /// (сутність, атрибут) зі змінами (ретроактивні правки)
    pub changes: usize,
    /// від'ємних hp/mp (радар)
    pub errors: usize,
}

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS stat_snapshots (\n  entity_name TEXT NOT NULL,\n  attr TEXT NOT NULL,\n  chapter INTEGER NOT NULL,\n  value INTEGER NOT NULL,\n  PRIMARY KEY (entity_name, attr, chapter)\n);";

/// ASCII-незалежний від регістру пошук `[stat:` (шаблон — чистий ASCII,
/// тому порівняння по байтах безпечне і не ламає кириличні offset-и).
fn find_stat_tag(line: &str, from: usize) -> Option<usize> {
    const PAT: &[u8] = b"[stat:";
    let bytes = line.as_bytes();
    let mut i = from;
    while i + PAT.len() <= bytes.len() {
        if bytes[i..i + PAT.len()]
            .iter()
            .zip(PAT)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Спарсити всі дельти `[stat: <сутність> <атрибут> <значення>]` з тіла глави.
///
/// Форми значення: `+2`, `-30`, `+=150`, `-=30`; голе число = приріст.
/// Сутність може містити пробіли («Кальдрін Сумрачний») — атрибут і значення
/// завжди два останні токени. Некоректні рядки мовчки пропускаються.
pub fn parse_deltas(body: &str) -> Vec<StatDelta> {
    let mut out = Vec::new();
    for line in body.lines() {
        let mut from = 0usize;
        while let Some(tag) = find_stat_tag(line, from) {
            let start = tag + "[stat:".len();
            let Some(end_rel) = line[start..].find(']') else {
                break; // незакритий тег — далі по рядку шукати нічого
            };
            let end = start + end_rel;
            from = end + 1;
            let toks: Vec<&str> = line[start..end].split_whitespace().collect();
            if toks.len() < 3 {
                continue;
            }
            let mut value_tok = toks[toks.len() - 1];
            let mut attr_idx = toks.len() - 2;
            let (mut sign, mut glued) = (1i64, true);
            if let Some(d) = value_tok.strip_prefix("+=") {
                value_tok = d;
            } else if let Some(d) = value_tok.strip_prefix("-=") {
                sign = -1;
                value_tok = d;
            } else if value_tok.starts_with('+') {
                value_tok = &value_tok[1..];
            } else if value_tok.starts_with('-') {
                sign = -1;
                value_tok = &value_tok[1..];
            } else if toks[attr_idx] == "+=" {
                glued = false; // оператор окремим токеном: «xp += 150»
                attr_idx -= 1;
            } else if toks[attr_idx] == "-=" {
                sign = -1;
                glued = false;
                attr_idx -= 1;
            }
            if !glued && attr_idx == 0 {
                continue; // немає куди покласти сутність
            }
            let attr = toks[attr_idx].to_lowercase();
            let entity = toks[..attr_idx].join(" ");
            let Ok(n) = value_tok.parse::<i64>() else {
                continue;
            };
            out.push(StatDelta {
                entity,
                attr,
                value: sign * n,
            });
        }
    }
    out
}

/// Номер глави з імені файла: ведучі цифри («03-розділ.md» → 3).
fn chapter_from_path(rel: &str) -> Option<i64> {
    let stem = rel.rsplit('/').next()?;
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Перебудувати stat_snapshots з manuscript/ (ідемпотентно) і порівняти
/// зі старими снапшотами: ретроактивні правки → алерти `stat-change`,
/// від'ємні hp/mp → алерти `stat-error`.
pub fn rebuild(store: &ProjectStore) -> Result<RebuildReport> {
    let conn = store.conn();
    conn.execute_batch(SCHEMA)?;
    let mut report = RebuildReport::default();

    // 1) старі снапшоти (до перебудови)
    let mut old: HashMap<(String, String, i64), i64> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT entity_name, attr, chapter, value FROM stat_snapshots")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
            })?
            .filter_map(|r| r.ok());
        for (e, a, c, v) in rows {
            old.insert((e.to_lowercase(), a.to_lowercase(), c), v);
        }
    }

    // 2) дельти з усіх manuscript-документів
    let mut stmt = conn.prepare("SELECT path FROM docs WHERE kind='manuscript' ORDER BY path")?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    // агреговані дельти по (сутність, атрибут, глава)
    let mut deltas: BTreeMap<(String, String, i64), i64> = BTreeMap::new();
    // display-форми (перша зустрінута) для запису в БД
    let mut display: HashMap<(String, String), (String, String)> = HashMap::new();
    for (i, rel) in paths.iter().enumerate() {
        let chapter = chapter_from_path(rel).unwrap_or((i + 1) as i64);
        let body = indexer::read_body(store, rel)?;
        for d in parse_deltas(&body) {
            report.deltas += 1;
            let key = (d.entity.to_lowercase(), d.attr.to_lowercase(), chapter);
            *deltas.entry(key).or_insert(0) += d.value;
            display
                .entry((d.entity.to_lowercase(), d.attr.to_lowercase()))
                .or_insert_with(|| (d.entity.clone(), d.attr.clone()));
        }
    }

    // 3) кумулятивні снапшоти: стан ПІСЛЯ кожної глави, де атрибут змінювався
    let mut cumulative: HashMap<(String, String), i64> = HashMap::new();
    let mut new: HashMap<(String, String, i64), i64> = HashMap::new();
    for ((ek, ak, ch), v) in &deltas {
        let entry = cumulative.entry((ek.clone(), ak.clone())).or_insert(0);
        *entry += v;
        new.insert((ek.clone(), ak.clone(), *ch), *entry);
    }

    conn.execute("DELETE FROM stat_snapshots", [])?;
    for ((ek, ak, ch), v) in &new {
        let (e_disp, a_disp) = display
            .get(&(ek.clone(), ak.clone()))
            .cloned()
            .unwrap_or_else(|| (ek.clone(), ak.clone()));
        conn.execute(
            "INSERT INTO stat_snapshots(entity_name, attr, chapter, value) VALUES (?1,?2,?3,?4)",
            params![e_disp, a_disp, ch, v],
        )?;
    }
    report.snapshots = new.len();

    // 4) ретроактивні правки: діф старих vs нових по кожній главі
    let mut keys: Vec<(String, String)> = old
        .keys()
        .map(|k| (k.0.clone(), k.1.clone()))
        .chain(new.keys().map(|k| (k.0.clone(), k.1.clone())))
        .collect();
    keys.sort();
    keys.dedup();

    conn.execute("DELETE FROM alerts WHERE kind='stat-change' AND resolved=0", [])?;
    for (ek, ak) in keys {
        let mut chapters: Vec<i64> = old
            .keys()
            .chain(new.keys())
            .filter(|k| k.0 == ek && k.1 == ak)
            .map(|k| k.2)
            .collect();
        chapters.sort();
        chapters.dedup();
        let changes: Vec<DiffChange> = chapters
            .into_iter()
            .map(|ch| {
                let o = old.get(&(ek.clone(), ak.clone(), ch)).copied();
                let n = new.get(&(ek.clone(), ak.clone(), ch)).copied();
                DiffChange { chapter: ch, old: o, new: n }
            })
            .filter(|c| c.old != c.new)
            .collect();
        if changes.is_empty() {
            continue;
        }
        report.changes += 1;
        let (e_disp, a_disp) = display
            .get(&(ek.clone(), ak.clone()))
            .cloned()
            .unwrap_or_else(|| (ek.clone(), ak.clone()));
        let parts: Vec<String> = changes
            .iter()
            .map(|c| {
                format!(
                    "глава {}: {} → {}",
                    c.chapter,
                    c.old.map(|v| v.to_string()).unwrap_or_else(|| "—".into()),
                    c.new.map(|v| v.to_string()).unwrap_or_else(|| "—".into())
                )
            })
            .collect();
        let payload = serde_json::json!({
            "entity": e_disp,
            "attr": a_disp,
            "changes": changes.iter().map(|c| serde_json::json!({
                "chapter": c.chapter,
                "old": c.old,
                "new": c.new,
            })).collect::<Vec<_>>(),
            "message": format!(
                "Ретроактивна правка статів «{}» ({}): {}",
                e_disp, a_disp, parts.join("; ")
            ),
        });
        let entity_id = drift_entity_id(conn, &e_disp);
        conn.execute(
            "INSERT INTO alerts(kind, entity_id, payload_json, created_at)
             VALUES ('stat-change', ?1, ?2, strftime('%s','now'))",
            params![entity_id, serde_json::to_string(&payload)?],
        )?;
    }

    // 5) stat-error radar: від'ємні hp/mp у будь-якому снапшоті
    //    (один алерт на (сутність, атрибут) з усіма главами-порушеннями)
    conn.execute("DELETE FROM alerts WHERE kind='stat-error' AND resolved=0", [])?;
    let mut stmt = conn.prepare(
        "SELECT entity_name, attr, chapter, value FROM stat_snapshots
         WHERE lower(attr) IN ('hp','mp') AND value < 0
         ORDER BY entity_name, chapter",
    )?;
    let bad: Vec<(String, String, i64, i64)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    let mut negatives: BTreeMap<(String, String), Vec<(i64, i64)>> = BTreeMap::new();
    for (e, a, ch, v) in bad {
        negatives.entry((e, a)).or_default().push((ch, v));
    }
    for ((e, a), entries) in negatives {
        report.errors += 1;
        let parts: Vec<String> = entries
            .iter()
            .map(|(ch, v)| format!("{a}={v} (глава {ch})"))
            .collect();
        let payload = serde_json::json!({
            "entity": e,
            "attr": a,
            "entries": entries.iter().map(|(ch, v)| serde_json::json!({
                "chapter": ch,
                "value": v,
            })).collect::<Vec<_>>(),
            "message": format!(
                "Stat-error radar: «{}» має від'ємне значення життєвої сили: {}. Перевірте дельти.",
                e, parts.join(", ")
            ),
        });
        let entity_id = drift_entity_id(conn, &e);
        conn.execute(
            "INSERT INTO alerts(kind, entity_id, payload_json, created_at)
             VALUES ('stat-error', ?1, ?2, strftime('%s','now'))",
            params![entity_id, serde_json::to_string(&payload)?],
        )?;
    }

    Ok(report)
}

fn drift_entity_id(conn: &rusqlite::Connection, name: &str) -> Option<i64> {
    conn.query_row(
        "SELECT id FROM entities WHERE lower(canonical_form)=lower(?1)",
        params![name],
        |r| r.get(0),
    )
    .optional()
    .ok()
    .flatten()
}

/// Stat sheet preview: стан УСІХ персонажів на главу N (як читач бачить).
/// Атрибут без дельт до глави N у таблицю не потрапляє.
pub fn sheet(store: &ProjectStore, chapter: i64) -> Result<Vec<SheetRow>> {
    store.conn().execute_batch(SCHEMA)?;
    let mut stmt = store.conn().prepare(
        "SELECT entity_name, attr, value FROM stat_snapshots s
         WHERE chapter = (SELECT MAX(chapter) FROM stat_snapshots s2
                          WHERE s2.entity_name = s.entity_name
                            AND s2.attr = s.attr
                            AND s2.chapter <= ?1)
         ORDER BY entity_name, attr",
    )?;
    let rows = stmt
        .query_map(params![chapter], |r| {
            Ok(SheetRow {
                entity: r.get(0)?,
                attr: r.get(1)?,
                value: r.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Незвіреними алерти stat-error radar: (alert_id, payload_json).
pub fn radar_alerts(store: &ProjectStore) -> Result<Vec<(i64, String)>> {
    let mut stmt = store.conn().prepare(
        "SELECT id, payload_json FROM alerts WHERE kind='stat-error' AND resolved=0 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Незвіреними алерти ретроактивних правок: (alert_id, payload_json).
pub fn change_alerts(store: &ProjectStore) -> Result<Vec<(i64, String)>> {
    let mut stmt = store.conn().prepare(
        "SELECT id, payload_json FROM alerts WHERE kind='stat-change' AND resolved=0 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}
