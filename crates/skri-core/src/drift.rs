//! Name-drift детектор: «Kaldrin» у гл. 3 vs «Kaldryn» у гл. 19 — це одна сутність?
//!
//! Правило: нормалізована схожість (Levenshtein) ≥ 0.82, довжина ≥ 5, різні написання
//! → група-кандидат. Рішення за автором (human-confirm), але згадки одразу
//! перешиваються на канонічну форму з більшою кількістю згадок.

use crate::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

/// Група схожих написань.
#[derive(Debug, Serialize)]
pub struct DriftGroup {
    /// канонічна форма (найчастіше)
    pub canonical: String,
    /// всі варіанти, вкл. канонічну
    pub variants: Vec<String>,
}

/// Мінімальна довжина слова, щоб вважати пари «дрейфом» (короткі слова зливаються помилково).
const MIN_LEN: usize = 5;
/// Поріг схожості.
const THRESHOLD: f64 = 0.82;

/// Стаб-нормалізація відмінків uk/ru (без лематизації, свідомо груба):
/// обрізає відмінкові закінчення (-ою/-ом/-ов/-а/-у/-и/-і/-е) зі слів,
/// основа яких ≥ 4 символи і закінчується на приголосну («чоловічі імена
/// на приголосній»: Олександр→Олександра→«олександр», Дэвид→Дэвида→«дэвид»).
/// Жіночі імена на -а/-я з короткою основою («Оля», «Міра») не чіпаються.
/// Використовується ТОЛЬКИ для порівняння/ключів — канонічна форма не змінюється.
pub fn strip_case_suffix(word: &str) -> String {
    let lower = word.to_lowercase();
    const SUFFIXES: [&str; 8] = ["ою", "ом", "ов", "а", "у", "и", "і", "е"];
    for suf in SUFFIXES {
        if let Some(base) = lower.strip_suffix(suf) {
            let chars: Vec<char> = base.chars().collect();
            if chars.len() >= 4 && chars.last().map_or(false, |c| is_consonant(*c)) {
                return base.to_string();
            }
        }
    }
    lower
}

/// Приголосні uk/ru (без м'якого знака й апострофа).
fn is_consonant(c: char) -> bool {
    matches!(
        c,
        'б' | 'в'
            | 'г'
            | 'ґ'
            | 'д'
            | 'ж'
            | 'з'
            | 'й'
            | 'к'
            | 'л'
            | 'м'
            | 'н'
            | 'п'
            | 'р'
            | 'с'
            | 'т'
            | 'ф'
            | 'х'
            | 'ц'
            | 'ч'
            | 'ш'
            | 'щ'
    )
}

/// Нормалізована схожість Левенштейна [0..1].
pub fn similarity(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    if a == b {
        return 1.0;
    }
    let max = a.len().max(b.len());
    if max == 0 {
        return 1.0;
    }
    let d = levenshtein(&a, &b);
    1.0 - (d as f64 / max as f64)
}

pub fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Знайти drift-групи серед auto-сутностей (вкл. слабких `auto-weak`),
/// перешити згадки на канон (найчастіше згадувана форма), записати алерти.
///
/// QA-03: ПЕРЕД порівнянням схожості відмінкові форми («Олександр» /
/// «Олександра», «Дэвид» / «Дэвида») зливаються ТИХО — це одне ім'я, а не
/// drift-алерт. Див. [`strip_case_suffix`].
pub fn detect(conn: &Connection) -> Result<Vec<DriftGroup>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.canonical_form, e.aliases_json,
                (SELECT COUNT(*) FROM mentions m WHERE m.entity_id = e.id) AS mc
         FROM entities e WHERE e.source LIKE 'auto%'
         ORDER BY mc DESC, e.canonical_form",
    )?;
    let mut entities: Vec<(i64, String, String, i64)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // --- QA-03: відмінкові форми — те саме ім'я (без алерту) ---
    // Групуємо auto-сутності за нормалізованою основою; групи з ≥2 членів
    // зливаємо в найчастішу форму (перша — список відсортований за mc DESC):
    // згадки перешиваються, форма-дубель додається в aliases. Жодного
    // name-drift алерту це не створює.
    let mut consumed = vec![false; entities.len()];
    {
        let mut by_norm: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        for i in 0..entities.len() {
            let key = strip_case_suffix(&entities[i].1);
            by_norm.entry(key).or_default().push(i);
        }
        let groups: Vec<Vec<usize>> = by_norm.into_values().filter(|g| g.len() >= 2).collect();
        for idxs in groups {
            let canon_idx = idxs[0]; // найчастіша форма
            let canon_id = entities[canon_idx].0;
            let canon_name = entities[canon_idx].1.clone();
            for &i in idxs.iter().skip(1) {
                let id = entities[i].0;
                conn.execute(
                    "UPDATE mentions SET entity_id = ?2 WHERE entity_id = ?1",
                    params![id, canon_id],
                )?;
                conn.execute("DELETE FROM entities WHERE id = ?1", params![id])?;
                let aliases: Vec<String> =
                    serde_json::from_str(&entities[canon_idx].2).unwrap_or_default();
                let form = entities[i].1.clone();
                if !form.eq_ignore_ascii_case(&canon_name)
                    && !aliases.iter().any(|a| a.eq_ignore_ascii_case(&form))
                {
                    let mut updated = aliases.clone();
                    updated.push(form);
                    conn.execute(
                        "UPDATE entities SET aliases_json = ?2 WHERE id = ?1",
                        params![canon_id, serde_json::to_string(&updated)?],
                    )?;
                    entities[canon_idx].2 = serde_json::to_string(&updated)?;
                }
                consumed[i] = true;
            }
        }
    }

    // жадібне групування: перший (найчастіший) член групи = канонічна форма
    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new(); // (індекс канону в entities, індекси членів)
    for i in 0..entities.len() {
        if consumed[i] {
            continue; // вже злитий як відмінкова форма
        }
        let name = &entities[i].1;
        if name.chars().count() < MIN_LEN {
            continue;
        }
        let mut placed = false;
        for g in &mut groups {
            let canon_name = &entities[g.0].1;
            if canon_name.to_lowercase() != name.to_lowercase()
                && similarity(canon_name, name) >= THRESHOLD
            {
                g.1.push(i);
                placed = true;
                break;
            }
        }
        if !placed {
            groups.push((i, vec![i]));
        }
    }

    let mut out = Vec::new();
    for (canon_idx, member_idxs) in groups {
        if member_idxs.len() < 2 {
            continue;
        }
        let canon_id = entities[canon_idx].0;
        let canon_name = entities[canon_idx].1.clone();
        let mut variants: Vec<String> = member_idxs
            .iter()
            .map(|&i| entities[i].1.clone())
            .collect();
        variants.sort();
        variants.dedup();

        // перешити згадки варіантів на канонічну сутність, варіанти видалити
        for &i in &member_idxs {
            let id = entities[i].0;
            if id == canon_id {
                continue;
            }
            conn.execute(
                "UPDATE mentions SET entity_id = ?2 WHERE entity_id = ?1",
                params![id, canon_id],
            )?;
            conn.execute("DELETE FROM entities WHERE id = ?1", params![id])?;
        }

        // aliases канону ← старі + варіанти (без самого канону)
        let mut aliases: Vec<String> =
            serde_json::from_str(&entities[canon_idx].2).unwrap_or_default();
        for v in &variants {
            if !v.eq_ignore_ascii_case(&canon_name)
                && !aliases.iter().any(|a| a.eq_ignore_ascii_case(v))
            {
                aliases.push(v.clone());
            }
        }
        conn.execute(
            "UPDATE entities SET aliases_json = ?2 WHERE id = ?1",
            params![canon_id, serde_json::to_string(&aliases)?],
        )?;

        let payload = serde_json::json!({
            "variants": variants,
            "canonical": canon_name,
            "message": format!(
                "Ймовірно, це одна сутність у різних написаннях: {}. Канонічна форма обрана за кількістю згадок — «{}». Перевірте і виправте, якщо потрібно.",
                variants.join(" ↔ "), canon_name
            )
        });
        conn.execute(
            "INSERT INTO alerts(kind, entity_id, payload_json, created_at)
             VALUES ('name-drift', ?1, ?2, strftime('%s','now'))",
            params![canon_id, serde_json::to_string(&payload)?],
        )?;
        out.push(DriftGroup {
            canonical: canon_name,
            variants,
        });
    }
    Ok(out)
}

/// Незвіреними алерти name-drift.
pub fn unresolved_alerts(conn: &Connection) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn
        .prepare("SELECT id, payload_json FROM alerts WHERE kind='name-drift' AND resolved=0 ORDER BY id")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Позначити алервіреним.
pub fn resolve_alert(conn: &Connection, alert_id: i64) -> Result<bool> {
    let n = conn.execute(
        "UPDATE alerts SET resolved=1 WHERE id=?1",
        params![alert_id],
    )?;
    Ok(n > 0)
}

/// Знайти entity_id за канонічною формою (без регістру).
pub fn entity_id_by_name(conn: &Connection, name: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM entities WHERE lower(canonical_form)=lower(?1)",
            params![name],
            |r| r.get(0),
        )
        .optional()?)
}
