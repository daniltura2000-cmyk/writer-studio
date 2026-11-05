//! Game Narrative Studio (M3): barks як першокласний об'єкт, аудитор змінних,
//! text-first діалог (Ink/Yarn-сумісний), досяжність нод.
//!
//! Лікує досліджені болі:
//! - *"the biggest nightmare was bark dialogue... 'You have to do that 10-15
//!    different ways. And that's just an ice bomb.'"* — усі сидять в Excel
//! - *"a simple sheet with a single row for each variable, noting where it is
//!    set and where it is checked"* — вручну; Yarn лише warning на опечатки
//! - *"the developer side takes over and the writer disappears"* — text-first

use crate::store::ProjectStore;
use crate::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

// --- Barks ---

/// Bark-запис: системний репліка-рядок, що запускається тригером.
#[derive(Debug, Clone, Serialize)]
pub struct Bark {
    pub id: i64,
    /// тригер: "ice-bomb-hit", "player-near-chest", "low-hp"...
    pub trigger: String,
    /// умови JSON: {"state":"alert","hp_below":30}
    pub conditions: String,
    pub priority: i64,
    /// cooldown у секундах (анти-спам)
    pub cooldown: i64,
    /// 10-15 варіантів одного тригера (випадковий вибір у грі)
    pub variants: Vec<String>,
    /// персонаж-носій (за потреби)
    pub entity_id: Option<i64>,
}

/// Створити/оновити bark (upsert за trigger+entity).
pub fn upsert_bark(store: &ProjectStore, b: &Bark) -> Result<i64> {
    if b.trigger.trim().is_empty() {
        return Err(Error::Invalid("порожній тригер".into()));
    }
    if b.variants.is_empty() {
        return Err(Error::Invalid("bark без варіантів реплік".into()));
    }
    let conn = store.conn();
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM barks WHERE trigger=?1 AND (entity_id IS ?2 OR (entity_id IS NULL AND ?2 IS NULL))",
            params![b.trigger, b.entity_id],
            |r| r.get(0),
        )
        .optional()?;
    let variants_json = serde_json::to_string(&b.variants)?;
    let id = match existing {
        Some(id) => {
            conn.execute(
                "UPDATE barks SET conditions=?2, priority=?3, cooldown=?4, variants=?5 WHERE id=?1",
                params![id, b.conditions, b.priority, b.cooldown, variants_json],
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO barks(trigger, conditions, priority, cooldown, variants, entity_id)
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![b.trigger, b.conditions, b.priority, b.cooldown, variants_json, b.entity_id],
            )?;
            conn.last_insert_rowid()
        }
    };
    // дублі-варіанти в одному bark — миттєвий сигнал (Excel цього не бачить)
    let mut seen = std::collections::BTreeSet::new();
    for v in &b.variants {
        if !seen.insert(v.trim().to_lowercase()) {
            let payload = serde_json::json!({
                "bark_id": id, "trigger": b.trigger, "duplicate": v,
                "message": format!("Bark «{}»: дубльований варіант репліки «{}»", b.trigger, v)
            });
            conn.execute(
                "INSERT INTO alerts(kind, entity_id, payload_json, created_at)
                 VALUES ('bark-duplicate', NULL, ?1, strftime('%s','now'))",
                params![serde_json::to_string(&payload)?],
            )?;
        }
    }
    Ok(id)
}

/// Усі barks.
pub fn barks(store: &ProjectStore) -> Result<Vec<Bark>> {
    let conn = store.conn();
    let mut stmt = conn.prepare(
        "SELECT id, trigger, conditions, priority, cooldown, variants, entity_id
         FROM barks ORDER BY trigger, priority DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let variants_json: String = r.get(5)?;
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                variants_json,
                r.get::<_, Option<i64>>(6)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .map(|(id, trigger, conditions, priority, cooldown, variants_json, entity_id)| Bark {
            id,
            trigger,
            conditions,
            priority,
            cooldown,
            variants: serde_json::from_str(&variants_json).unwrap_or_default(),
            entity_id,
        })
        .collect();
    Ok(rows)
}

/// CSV-експорт barks (движок-імпорт; заміна Excel-болю).
pub fn export_barks_csv(store: &ProjectStore) -> Result<String> {
    let mut out = String::from("trigger,conditions,priority,cooldown,variant_index,variant\n");
    for b in barks(store)? {
        let conds = b.conditions.replace(',', ";");
        for (i, v) in b.variants.iter().enumerate() {
            let line = v.replace('"', "\"\"").replace('\n', " ");
            out.push_str(&format!(
                "{},\"{}\",{},{},{},\"{}\"\n",
                b.trigger, conds, b.priority, b.cooldown, i + 1, line
            ));
        }
    }
    Ok(out)
}

// --- Аудитор змінних (vars) ---

/// Операція над змінною у документі.
#[derive(Debug, Clone, Serialize)]
pub struct VarOp {
    pub id: i64,
    pub var_name: String,
    /// set | check | unset
    pub op: String,
    pub doc_path: String,
    /// позиція в тексті (для переходів)
    pub char_start: i64,
}

/// Розпарсити операції над змінними з text-first діалогу.
///
/// Синтаксис (Ink/Yarn-дружній, сумісний із звичайним письмом):
///   %SET flag_name            — встановити
///   %CHECK flag_name          — перевірити
///   %UNSET flag_name          — зняти
/// (рядки починаються з %; інший текст — звичайна проза)
pub fn parse_var_ops(body: &str, doc_path: &str) -> Vec<(String, String, usize)> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    for line in body.split_inclusive('\n') {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("%SET ") {
            out.push(("set".into(), rest.trim().to_string(), offset + line.len() - line.trim_start().len()));
        } else if let Some(rest) = t.strip_prefix("%CHECK ") {
            out.push(("check".into(), rest.trim().to_string(), offset + line.len() - line.trim_start().len()));
        } else if let Some(rest) = t.strip_prefix("%UNSET ") {
            out.push(("unset".into(), rest.trim().to_string(), offset + line.len() - line.trim_start().len()));
        }
        offset += line.len();
    }
    let _ = doc_path;
    out
}

/// Пересканувати діалогові документи (`dialogue/`): таблиця var_ops + аудитор.
pub fn scan_dialogue(store: &ProjectStore) -> Result<DialogueReport> {
    ensure_tables(store.conn())?;
    let conn = store.conn();
    conn.execute("DELETE FROM var_ops", [])?;
    let mut report = DialogueReport::default();

    let dialogue_dir = store.root.join("dialogue");
    if !dialogue_dir.exists() {
        return Ok(report);
    }
    for entry in std::fs::read_dir(&dialogue_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let rel = path
            .strip_prefix(&store.root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let raw = std::fs::read_to_string(&path)?;
        let body = crate::frontmatter::parse(&raw).body;
        for (op, name, pos) in parse_var_ops(&body, &rel) {
            conn.execute(
                "INSERT INTO var_ops(var_name, op, doc_path, char_start) VALUES (?1,?2,?3,?4)",
                params![name, op, rel, pos as i64],
            )?;
            report.ops += 1;
        }
    }
    Ok(report)
}

fn ensure_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS barks (
           id INTEGER PRIMARY KEY,
           trigger TEXT NOT NULL,
           conditions TEXT NOT NULL DEFAULT '{}',
           priority INTEGER NOT NULL DEFAULT 0,
           cooldown INTEGER NOT NULL DEFAULT 0,
           variants TEXT NOT NULL DEFAULT '[]',
           entity_id INTEGER
         );
         CREATE TABLE IF NOT EXISTS var_ops (
           id INTEGER PRIMARY KEY,
           var_name TEXT NOT NULL,
           op TEXT NOT NULL,
           doc_path TEXT NOT NULL,
           char_start INTEGER NOT NULL DEFAULT 0
         );",
    )?;
    Ok(())
}

/// Аудитор: матриця set/check по змінних (заміна ручного Excel-листа).
#[derive(Debug, Clone, Serialize)]
pub struct VarAudit {
    pub var_name: String,
    pub sets: Vec<String>,
    pub checks: Vec<String>,
    pub unsets: Vec<String>,
    /// set-ів немає, але check-и є (використовується, ніде не встановлюється)
    pub orphan_check: bool,
    /// set є, check-ів немає (мертвий прапорець)
    pub dead_flag: bool,
}

/// Повна матриця set/check/unset.
pub fn var_audit(store: &ProjectStore) -> Result<Vec<VarAudit>> {
    let conn = store.conn();
    let mut stmt = conn.prepare(
        "SELECT var_name, op, doc_path || ':' || char_start
         FROM var_ops ORDER BY var_name, doc_path, char_start",
    )?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();

    let mut names: Vec<String> = Vec::new();
    for (n, _, _) in &rows {
        if !names.contains(n) {
            names.push(n.clone());
        }
    }

    let mut out = Vec::new();
    for name in names {
        let a = VarAudit {
            var_name: name.clone(),
            sets: rows.iter().filter(|(n, op, _)| n == &name && op == "set").map(|(_, _, l)| l.clone()).collect(),
            checks: rows.iter().filter(|(n, op, _)| n == &name && op == "check").map(|(_, _, l)| l.clone()).collect(),
            unsets: rows.iter().filter(|(n, op, _)| n == &name && op == "unset").map(|(_, _, l)| l.clone()).collect(),
            orphan_check: false,
            dead_flag: false,
        };
        let orphan = a.sets.is_empty() && !a.checks.is_empty();
        let dead = !a.sets.is_empty() && a.checks.is_empty() && a.unsets.is_empty();
        out.push(VarAudit { orphan_check: orphan, dead_flag: dead, ..a });
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.orphan_check || a.dead_flag));
    Ok(out)
}

// --- Досяжність нод (text-first граф) ---

/// Нода: `-> NODE_NAME` розпочинає, `[goto NODE_NAME]` переходить.
/// Нода без входу (не перша і ніхто на неї не посилається) = недосяжна.
#[derive(Debug, Clone, Serialize)]
pub struct GraphAudit {
    pub total_nodes: usize,
    pub entry_nodes: Vec<String>,
    pub unreachable: Vec<String>,
    pub dangling_gotos: Vec<String>,
}

/// Проаналізувати розгалуження в одному діалоговому документі.
pub fn graph_audit(body: &str) -> GraphAudit {
    let mut declared: Vec<String> = Vec::new();
    let mut gotos: Vec<String> = Vec::new();
    let mut first_node: Option<String> = None;

    for (idx, line) in body.lines().enumerate() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("-> ") {
            let name = rest.trim().to_string();
            if first_node.is_none() {
                first_node = Some(name.clone());
            }
            if !declared.contains(&name) {
                declared.push(name);
            }
        }
        // [goto X] де завгодно в рядку
        let mut search = 0usize;
        while let Some(rel) = t[search..].find("[goto ") {
            let abs = search + rel + 6;
            if let Some(end) = t[abs..].find(']') {
                let name = t[abs..abs + end].trim().to_string();
                if !name.is_empty() {
                    gotos.push(name);
                }
                search = abs + end;
            } else {
                break;
            }
        }
        let _ = idx;
    }

    let dangling: Vec<String> = gotos
        .iter()
        .filter(|g| !declared.iter().any(|d| d.eq_ignore_ascii_case(g)))
        .cloned()
        .collect();
    let reachable_from: Vec<String> = gotos
        .iter()
        .filter(|g| declared.iter().any(|d| d.eq_ignore_ascii_case(g)))
        .cloned()
        .collect();
    let unreachable: Vec<String> = declared
        .iter()
        .filter(|d| {
            !first_node
                .as_ref()
                .map(|f| f.eq_ignore_ascii_case(d))
                .unwrap_or(false)
                && !reachable_from.iter().any(|g| g.eq_ignore_ascii_case(d))
        })
        .cloned()
        .collect();

    GraphAudit {
        total_nodes: declared.len(),
        entry_nodes: first_node.iter().cloned().collect(),
        unreachable,
        dangling_gotos: dangling,
    }
}

/// Звіт сканування діалогів.
#[derive(Debug, Default)]
pub struct DialogueReport {
    pub ops: usize,
}
