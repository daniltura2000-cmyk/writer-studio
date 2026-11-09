//! Живий Story Bible (M1): картки сутностей з тексту + assertions з `bible/*.md`
//! + конфлікт-детектор атрибутів («зелені очі» гл. 5 vs «карі» гл. 21).

use crate::store::{EntityCard, ProjectStore};
use crate::{drift, Result};
use rusqlite::{params, Connection, OptionalExtension};

/// Синхронізувати `bible/*.md` → entities (source=manual) + assertions.
/// Формат файлу біблії:
///
/// ```markdown
/// ---
/// name: Кальдрін Сумрачний
/// type: персонаж
/// ---
///
/// ## Опис
///
/// очі: зелені
/// зброя: двуручний меч «Тиша»
/// ```
///
/// Рядки `ключ: значення` у тілі стають assertions. Дублікати атрибутів
/// з різними значеннями = conflict-алерт.
pub fn sync_bible(store: &ProjectStore) -> Result<BibleReport> {
    let conn = store.conn();
    let mut report = BibleReport::default();
    let bible_dir = store.root.join("bible");
    if !bible_dir.exists() {
        return Ok(report);
    }

    for entry in std::fs::read_dir(&bible_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let raw = std::fs::read_to_string(&path)?;
        let parsed = crate::frontmatter::parse(&raw);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let name = parsed.meta.get("name").cloned().unwrap_or(stem);
        let etype = parsed
            .meta
            .get("type")
            .cloned()
            .unwrap_or_else(|| "персонаж".to_string());

        let entity_id = upsert_manual_entity(conn, &name, &etype)?;
        report.entities += 1;
        sync_aliases(conn, entity_id, &parsed.meta);

        let rel = path
            .strip_prefix(&store.root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let doc_id = find_or_create_doc(conn, store, &rel)?;

        // Ідемпотентність: перебудовуємо лише твердження ЦЬОГО файлу.
        // Твердження інших файлів біблії лишаються — саме так ловляться конфлікти.
        conn.execute(
            "DELETE FROM assertions WHERE entity_id=?1 AND doc_id=?2",
            params![entity_id, doc_id],
        )?;

        for line in parsed.body.lines() {
            let line = line.trim().trim_start_matches('-').trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (attr, value) = match line.split_once(':') {
                Some((a, v)) if !a.contains(' ') && !a.is_empty() => {
                    (a.trim().to_string(), v.trim().to_string())
                }
                _ => continue,
            };
            if attr.is_empty() || value.is_empty() {
                continue;
            }

            // Крос-файловий конфлікт канону: той самий атрибут, інше значення в іншому файлі
            let prev: Vec<String> = {
                let mut st = conn.prepare(
                    "SELECT DISTINCT value FROM assertions
                     WHERE entity_id=?1 AND attribute=?2 AND value != ?3",
                )?;
                let rows = st
                    .query_map(params![entity_id, attr, value], |r| r.get::<_, String>(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                rows
            };
            let status = if prev.is_empty() {
                "canonical"
            } else {
                report.conflicts += 1;
                // уникаємо дубля алерту при ресинхронізуванні
                conn.execute(
                    "DELETE FROM alerts WHERE kind='assertion-conflict'
                     AND entity_id=?1 AND json_extract(payload_json,'$.attribute')=?2",
                    params![entity_id, attr],
                )?;
                let mut values = prev.clone();
                values.push(value.clone());
                values.sort();
                values.dedup();
                let payload = serde_json::json!({
                    "entity": name,
                    "attribute": attr,
                    "values": values,
                    "doc": rel,
                    "message": format!("Конфлікт канону: у «{}» атрибут «{}» має різні значення: {}. Оберіть істинне або позначте retcon.", name, attr, values.join(" ↔ "))
                });
                conn.execute(
                    "INSERT INTO alerts(kind, entity_id, payload_json, created_at)
                     VALUES ('assertion-conflict', ?1, ?2, strftime('%s','now'))",
                    params![entity_id, serde_json::to_string(&payload)?],
                )?;
                "conflict"
            };
            conn.execute(
                "INSERT INTO assertions(entity_id, attribute, value, doc_id, status)
                 VALUES (?1,?2,?3,?4,?5)",
                params![entity_id, attr, value, doc_id, status],
            )?;
            report.assertions += 1;
        }
    }
    // фінальний стан: скільки НЕзвірених конфліктів канону в базі
    report.conflicts = conn.query_row(
        "SELECT COUNT(*) FROM alerts WHERE kind='assertion-conflict' AND resolved=0",
        [],
        |r| r.get::<_, i64>(0),
    )? as usize;
    Ok(report)
}

fn upsert_manual_entity(conn: &Connection, name: &str, etype: &str) -> Result<i64> {
    let existing: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, source FROM entities WHERE lower(canonical_form)=lower(?1)",
            params![name],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match existing {
        Some((id, _)) => {
            conn.execute(
                "UPDATE entities SET type=?2, source='manual' WHERE id=?1",
                params![id, etype],
            )?;
            Ok(id)
        }
        None => {
            conn.execute(
                "INSERT INTO entities(type, canonical_form, source, created_at)
                 VALUES (?2, ?1, 'manual', strftime('%s','now'))",
                params![name, etype],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

fn find_or_create_doc(conn: &Connection, store: &ProjectStore, rel: &str) -> Result<i64> {
    if let Some(id) = store.doc_id(rel) {
        return Ok(id);
    }
    let title = rel
        .rsplit('/')
        .next()
        .unwrap_or(rel)
        .trim_end_matches(".md")
        .to_string();
    conn.execute(
        "INSERT INTO docs(path, kind, title) VALUES (?1, 'bible', ?2)",
        params![rel, title],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Картки всіх сутностей (Story Bible), агреговані з mentions + assertions.
pub fn cards(store: &ProjectStore) -> Result<Vec<EntityCard>> {
    let conn = store.conn();
    let mut out = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT id, type, canonical_form, aliases_json FROM entities ORDER BY canonical_form",
    )?;
    let ents: Vec<(i64, String, String, String)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    for (id, etype, canonical, aliases_json) in ents {
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM mentions WHERE entity_id=?1",
            params![id],
            |r| r.get(0),
        )?;
        let mut docs_stmt = conn.prepare(
            "SELECT d.path, COUNT(*) FROM mentions m
             JOIN docs d ON d.id = m.doc_id
             WHERE m.entity_id=?1 GROUP BY d.path ORDER BY COUNT(*) DESC",
        )?;
        let docs: Vec<(String, i64)> = docs_stmt
            .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        let mut asm_stmt = conn.prepare(
            "SELECT attribute, value, status FROM assertions WHERE entity_id=?1 ORDER BY attribute",
        )?;
        let assertions: Vec<(String, String, String)> = asm_stmt
            .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();

        let aliases: Vec<String> = serde_json::from_str(&aliases_json).unwrap_or_default();
        out.push(EntityCard {
            entity_id: id,
            entity_type: etype,
            canonical,
            aliases,
            mention_total: total.max(0) as usize,
            docs,
            assertions,
        });
    }
    Ok(out)
}

/// Об'єднати aliases із frontmatter в entities.aliases_json (без дублів,
/// порядок збережено) — щоб картка біблії одразу показувала варіанти імені.
fn sync_aliases(
    conn: &Connection,
    entity_id: i64,
    meta: &std::collections::BTreeMap<String, String>,
) {
    let Some(raw) = meta.get("aliases") else {
        return;
    };
    let new_aliases: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if new_aliases.is_empty() {
        return;
    }
    let current: String = conn
        .query_row(
            "SELECT aliases_json FROM entities WHERE id=?1",
            params![entity_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "[]".to_string());
    let mut aliases: Vec<String> = serde_json::from_str(&current).unwrap_or_default();
    for a in new_aliases {
        if !aliases.iter().any(|x| x.eq_ignore_ascii_case(&a)) {
            aliases.push(a);
        }
    }
    if let Ok(json) = serde_json::to_string(&aliases) {
        let _ = conn.execute(
            "UPDATE entities SET aliases_json=?2 WHERE id=?1",
            params![entity_id, json],
        );
    }
}

/// Ручна картка досьє (M10): створює/оновлює `bible/<slug>.md`.
///
/// Досьє одразу потрапляє в Story Bible після наступного `sync_bible`
/// (картка з'являється як source=manual у EntityCard).
#[derive(Debug, Clone)]
pub struct ManualCard {
    /// ім'я сутності (канонічна форма)
    pub name: String,
    /// варіанти написання (aliases)
    pub aliases: Vec<String>,
    /// тип сутності («персонаж», «локація», …)
    pub kind: String,
    /// вільні нотатки — тіло файлу
    pub notes: String,
}

impl Default for ManualCard {
    fn default() -> Self {
        ManualCard {
            name: String::new(),
            aliases: Vec::new(),
            kind: "персонаж".to_string(),
            notes: String::new(),
        }
    }
}

/// Записати ручну картку досьє в `bible/`.
///
/// Формат файлу — frontmatter (name, aliases через кому, type) + нотатки
/// у тілі. Існуючий файл з тим самим name (без регістру) ПЕРЕЗАПИСУЄТЬСЯ —
/// це оновлення досьє, дублікатів не плодить.
pub fn add_manual_card(root: &str, card: &ManualCard) -> std::io::Result<()> {
    let bible_dir = std::path::Path::new(root).join("bible");
    std::fs::create_dir_all(&bible_dir)?;

    // оновлення: знайти існуючий файл із тим самим name (без регістру)
    let mut target: Option<std::path::PathBuf> = None;
    if let Ok(entries) = std::fs::read_dir(&bible_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let raw = match std::fs::read_to_string(&path) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let parsed = crate::frontmatter::parse(&raw);
            let existing = parsed.meta.get("name").cloned().unwrap_or_default();
            if existing.eq_ignore_ascii_case(&card.name) {
                target = Some(path);
                break;
            }
        }
    }

    let path = target.unwrap_or_else(|| {
        let slug = slugify(&card.name);
        bible_dir.join(format!("{slug}.md"))
    });

    let mut md = String::new();
    md.push_str("---\n");
    md.push_str(&format!("name: {}\n", card.name));
    if !card.aliases.is_empty() {
        md.push_str(&format!("aliases: {}\n", card.aliases.join(", ")));
    }
    md.push_str(&format!("type: {}\n", card.kind));
    md.push_str("---\n\n");
    md.push_str(card.notes.trim());
    md.push('\n');

    std::fs::write(&path, md)
}

/// Ім'я → безпечне ім'я файла: літери/цифри лишаються (вкл. кирилицю),
/// решта → '-', повзучі дефіси й крайові обрізаються.
fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-').to_lowercase();
    if slug.is_empty() {
        "card".to_string()
    } else {
        slug
    }
}

/// Звіт синхронізації біблії.
#[derive(Debug, Default)]
pub struct BibleReport {
    pub entities: usize,
    pub assertions: usize,
    pub conflicts: usize,
}

/// Об'єднати auto-кандидатів з ручними сутностями біблії:
/// якщо auto-кандидат збігається (або схожий ≥0.9) з ручним — згадки перешиваються.
pub fn merge_auto_into_manual(store: &ProjectStore) -> Result<usize> {
    let conn = store.conn();
    let mut merged = 0usize;
    let mut stmt = conn.prepare(
        "SELECT id, canonical_form FROM entities WHERE source='auto'",
    )?;
    let autos: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (auto_id, auto_name) in autos {
        let manual: Option<i64> = conn
            .query_row(
                "SELECT id FROM entities WHERE source='manual' AND lower(canonical_form)=lower(?1)",
                params![auto_name],
                |r| r.get(0),
            )
            .optional()?;
        let target = match manual {
            Some(id) => Some(id),
            None => {
                // fuzzy: схожість ≥ 0.9 з ручною сутністю
                let mut best: Option<(i64, f64)> = None;
                let mut mstmt = conn
                    .prepare("SELECT id, canonical_form FROM entities WHERE source='manual'")?;
                let mans: Vec<(i64, String)> = mstmt
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .filter_map(|r| r.ok())
                    .collect();
                for (mid, mname) in mans {
                    let s = drift::similarity(&auto_name, &mname);
                    if s >= 0.9 && best.map(|(_, bs)| s > bs).unwrap_or(true) {
                        best = Some((mid, s));
                    }
                }
                best.map(|(id, _)| id)
            }
        };
        if let Some(mid) = target {
            conn.execute(
                "UPDATE mentions SET entity_id=?2 WHERE entity_id=?1",
                params![auto_id, mid],
            )?;
            conn.execute("DELETE FROM entities WHERE id=?1", params![auto_id])?;
            merged += 1;
        }
    }
    Ok(merged)
}

/// Чи є сутність у рукописі (для валідатора «сутність оголошена, але не згадана»).
pub fn entity_is_mentioned(conn: &Connection, entity_id: i64) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM mentions WHERE entity_id=?1",
        params![entity_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}
