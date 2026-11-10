//! Рушій фактів досьє (QA-02, головна зірка продукту).
//!
//! Досьє (картки `bible/*.md`) — канон: «Йому 25 років. Його очі зелені»
//! у нотатках персонажа означає, що вік 25 і очі зелені — ІСТИНА. Глави
//! рукопису перевіряються проти канону і проти одна одної:
//!
//! - `fact-conflict`   — два РІЗНІ значення одного факту однієї сутності
//!   у різних главах («25 років» у гл. 1 vs «40 років» у гл. 2);
//! - `fact-vs-dossier` — глава суперечить досьє (канону).
//!
//! Правила перші (детерміновано, без AI, std-only — токени, не regex):
//! - вік: «число + одиниця» (років/рік/лет/год…), укр+рос;
//! - колір очей: «очі <колір>» або «<колір> очі» (російські форми
//!   нормалізуються до укр. канону, щоб uk/ru текст не давав хибних
//!   конфліктів «зелені ↔ зеленые»);
//! - факти глави беруться ЛИШЕ з речень, де сутність або її alias
//!   згадується (порівняння — через наявний resolve з aliases +
//!   стаб-нормалізацію відмінків [`crate::drift::strip_case_suffix`]).
//!
//! Передумова: перед [`rebuild`] має бути виконаний конвеєр індексування
//! (reindex + scan_project + sync_bible), щоб entities/aliases були в БД.

use crate::drift;
use crate::store::{add_alert, ProjectStore};
use crate::Result;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};

/// Одиниці віку (укр/рос) — токен ПІСЛЯ числа.
const AGE_UNITS: &[&str] = &["років", "роки", "рік", "року", "лет", "год", "года"];

/// Слова «очі» укр/рос.
const EYE_WORDS: &[&str] = &["очі", "глаза", "очи"];

/// Кольори очей uk/ru (нижній регістр) → канонічне значення (укр).
const EYE_COLORS: &[(&str, &str)] = &[
    ("зелені", "зелені"),
    ("зеленые", "зелені"),
    ("блакитні", "блакитні"),
    ("голубые", "блакитні"),
    ("карі", "карі"),
    ("карие", "карі"),
    ("сірі", "сірі"),
    ("серые", "сірі"),
    ("сині", "сині"),
    ("синие", "сині"),
    ("green", "зелені"),
    ("blue", "блакитні"),
    ("gray", "сірі"),
    ("grey", "сірі"),
];

/// Витягнутий факт.
#[derive(Debug, Clone)]
pub struct Fact {
    pub doc_path: String,
    /// байтовий offset у тілі документа
    pub char_start: usize,
    /// канонічне ім'я сутності (з entities/досьє)
    pub entity: String,
    /// "age" | "eye_color"
    pub kind: String,
    pub value: String,
    pub sentence: String,
}

/// Звіт перебудови фактів.
#[derive(Debug, Default)]
pub struct FactsReport {
    pub facts: usize,
    pub conflicts: usize,
    pub vs_dossier: usize,
}

/// Людська назва kind факту («вік», «колір очей») — для звітів і повідомлень.
pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        "age" => "вік",
        "eye_color" => "колір очей",
        _ => "факт",
    }
}

/// Витягти факти з тексту: (kind, value, байтовий offset у тексті).
/// Працює і для цілого тіла досьє, і для одного речення глави.
pub fn extract_facts(text: &str) -> Vec<(String, String, usize)> {
    let toks = crate::indexer::tokenize(text);
    let mut out = Vec::new();
    for i in 0..toks.len() {
        let t = &toks[i];
        let lower = t.text.to_lowercase();
        // вік: «25 років», «йому було 25 років»
        if !t.text.is_empty() && t.text.chars().all(|c| c.is_ascii_digit()) {
            if let Some(next) = toks.get(i + 1) {
                if AGE_UNITS.contains(&next.text.to_lowercase().as_str()) {
                    out.push(("age".to_string(), t.text.clone(), t.start));
                    continue;
                }
            }
        }
        // колір очей: «очі зелені»
        if EYE_WORDS.contains(&lower.as_str()) {
            if let Some(next) = toks.get(i + 1) {
                if let Some((_, canon)) = EYE_COLORS
                    .iter()
                    .find(|(c, _)| *c == next.text.to_lowercase())
                {
                    out.push(("eye_color".to_string(), canon.to_string(), next.start));
                    continue;
                }
            }
        }
        // колір очей: «зелені очі»
        if let Some((_, canon)) = EYE_COLORS.iter().find(|(c, _)| *c == lower) {
            if let Some(next) = toks.get(i + 1) {
                if EYE_WORDS.contains(&next.text.to_lowercase().as_str()) {
                    out.push(("eye_color".to_string(), canon.to_string(), t.start));
                }
            }
        }
    }
    out
}

/// Форми сутностей для матчингу згадок: canonical + aliases (нижній регістр).
struct EntityForms {
    id: i64,
    name: String,
    forms: Vec<String>,
}

fn load_entities(conn: &Connection) -> Result<Vec<EntityForms>> {
    let mut stmt =
        conn.prepare("SELECT id, canonical_form, aliases_json FROM entities ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })?;
    let mut out = Vec::new();
    for (id, name, aliases_json) in rows.flatten() {
        let mut forms = vec![name.to_lowercase()];
        if let Ok(list) = serde_json::from_str::<Vec<String>>(&aliases_json) {
            for a in list {
                let l = a.to_lowercase();
                if !forms.contains(&l) {
                    forms.push(l);
                }
            }
        }
        out.push(EntityForms { id, name, forms });
    }
    Ok(out)
}

/// Чи згадується сутність у реченні: одиночна форма — по токенах з
/// нормалізацією відмінків («Олександра» → база «олександр»), багатослівна
/// (прізвище-пара, alias) — як підрядок речення.
/// Повертає (сутність, гола_згадка): «голa» = не стоїть після прийменника
/// («по Києву», «у Львові» — прийменникова згадка локації, не персонаж).
fn sentence_mentions<'e>(
    sent_text: &str,
    toks: &[crate::indexer::Token],
    ents: &'e [EntityForms],
) -> Vec<(&'e EntityForms, bool)> {
    const PREPS: [&str; 14] = [
        "у", "в", "на", "по", "з", "із", "до", "к", "від", "біля", "над", "під", "про", "для",
    ];
    let sent_lower = sent_text.to_lowercase();
    let mut out: Vec<(&EntityForms, bool)> = Vec::new();
    for e in ents {
        let mut hit = false;
        let mut bare = false;
        for f in &e.forms {
            if f.contains(' ') {
                if sent_lower.contains(f) {
                    hit = true;
                    bare = true;
                    break;
                }
            } else {
                for (i, t) in toks.iter().enumerate() {
                    let l = t.text.to_lowercase();
                    let matches = l == *f || drift::strip_case_suffix(&l) == drift::strip_case_suffix(f);
                    if matches {
                        hit = true;
                        // гола згадка: перший токен або попередній — не прийменник
                        let prev_is_prep = i > 0 && {
                            let p = toks[i - 1].text.to_lowercase();
                            PREPS.contains(&p.as_str())
                        };
                        if !prev_is_prep {
                            bare = true;
                        }
                        break;
                    }
                }
                if hit {
                    break;
                }
            }
        }
        if hit {
            out.push((e, bare));
        }
    }
    out
}

/// Перебудувати extracted_facts + fact-алерти з файлів (ідемпотентно).
///
/// Передумова: entities/mentions уже в БД (конвеєр індексування виконано).

/// Абзац: блок рядків без порожнього розділювача, з offsetом у тілі.
struct Paragraph {
    start: usize,
    text: String,
}

fn split_paragraphs(body: &str) -> Vec<Paragraph> {
    let mut out = Vec::new();
    let mut cur_start: Option<usize> = None;
    let mut cur = String::new();
    let mut offset = 0usize;
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if let Some(s) = cur_start.take() {
                out.push(Paragraph { start: s, text: cur.clone() });
                cur.clear();
            }
        } else {
            if cur_start.is_none() {
                cur_start = Some(offset + (line.len() - line.trim_start().len()));
            }
            cur.push_str(line);
        }
        offset += line.len();
    }
    if let Some(s) = cur_start {
        out.push(Paragraph { start: s, text: cur });
    }
    out
}

pub fn rebuild(store: &ProjectStore) -> Result<FactsReport> {
    let conn = store.conn();
    let mut report = FactsReport::default();
    let mut facts: Vec<Fact> = Vec::new();

    // 1) досьє (bible/*.md): факти нотаток = канон сутності
    let bible_dir = store.root.join("bible");
    if let Ok(entries) = std::fs::read_dir(&bible_dir) {
        let mut paths: Vec<std::path::PathBuf> =
            entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            let parsed = crate::frontmatter::parse(&raw);
            let fname = path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            let name = parsed
                .meta
                .get("name")
                .cloned()
                .unwrap_or_else(|| fname.trim_end_matches(".md").to_string());
            for (kind, value, off) in extract_facts(&parsed.body) {
                facts.push(Fact {
                    doc_path: format!("bible/{fname}"),
                    char_start: off,
                    entity: name.clone(),
                    kind,
                    value,
                    sentence: String::new(),
                });
            }
        }
    }

    // 2) глави: факти з речень, де сутність (або alias) згадується
    let ents = load_entities(conn)?;
    let mut stmt = conn.prepare("SELECT path FROM docs WHERE kind='manuscript' ORDER BY path")?;
    let docs: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    for rel in docs {
        let body = crate::indexer::read_body(store, &rel)?;
        // Факти часто стоять у сусідньому реченні через займенник:
        // «Олександр йшов. Йому 25 років.» — тому вікно атрибуції =
        // абзац (блок рядків без порожнього розділювача), а не одне речення.
        for para in split_paragraphs(&body) {
            let sents = crate::indexer::sentences(&para.text);
            // сутності з ГОЛОЮ згадкою десь у абзаці (прийменникові
            // «по Києву»/«у Львові» — не претенденти на факти особи)
            let mut para_ents: Vec<&EntityForms> = Vec::new();
            for sent in &sents {
                let toks = crate::indexer::tokenize(&sent.text);
                for (e, bare) in sentence_mentions(&sent.text, &toks, &ents) {
                    if bare && !para_ents.iter().any(|x| x.id == e.id) {
                        para_ents.push(e);
                    }
                }
            }
            for sent in &sents {
                for (kind, value, off) in extract_facts(&sent.text) {
                    for e in &para_ents {
                        facts.push(Fact {
                            doc_path: rel.clone(),
                            char_start: para.start + sent.start + off,
                            entity: e.name.clone(),
                            kind: kind.clone(),
                            value: value.clone(),
                            sentence: sent.text.clone(),
                        });
                    }
                }
            }
        }
    }

    // 3) запис (ідемпотентно: той самий вміст — той самий набір рядків)
    conn.execute("DELETE FROM extracted_facts", [])?;
    let mut seen: HashSet<(String, String, String, String)> = HashSet::new();
    let mut unique: Vec<&Fact> = Vec::new();
    for f in &facts {
        let key = (
            f.doc_path.clone(),
            f.entity.to_lowercase(),
            f.kind.clone(),
            f.value.to_lowercase(),
        );
        if seen.insert(key) {
            unique.push(f);
        }
    }
    for f in &unique {
        conn.execute(
            "INSERT INTO extracted_facts(doc_path, char_start, entity, kind, value, sentence)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                f.doc_path,
                f.char_start as i64,
                f.entity,
                f.kind,
                f.value,
                f.sentence
            ],
        )?;
    }
    report.facts = unique.len();

    // 4) fact-алерти (ідемпотентно: перебудова щоразу)
    conn.execute(
        "DELETE FROM alerts WHERE kind IN ('fact-conflict','fact-vs-dossier')",
        [],
    )?;

    // групування: (нормалізований ключ сутності, kind) → записи
    struct Group {
        name: String,
        kind_field: String,
        entity_id: Option<i64>,
        /// запис факту: doc, значення (lower), чи це досьє, цитата речення,
        /// байтовий offset у тілі глави (FQA-03: для кнопки «Перейти»)
        entries: Vec<(String, String, bool, String, usize)>,
    }
    let mut groups: HashMap<(String, String), Group> = HashMap::new();
    for f in &unique {
        let key = (drift::strip_case_suffix(&f.entity), f.kind.clone());
        let g = groups.entry(key).or_insert_with(|| Group {
            name: f.entity.clone(),
            kind_field: f.kind.clone(),
            entity_id: ents.iter().find(|e| e.name == f.entity).map(|e| e.id),
            entries: Vec::new(),
        });
        g.entries.push((
            f.doc_path.clone(),
            f.value.to_lowercase(),
            f.doc_path.starts_with("bible/"),
            f.sentence.clone(),
            f.char_start,
        ));
    }

    for (_, g) in groups {
        // досьє-канон: (значення → перший документ досьє)
        let mut dossier: HashMap<String, String> = HashMap::new();
        for (doc, value, is_dossier, _, _) in &g.entries {
            if *is_dossier {
                dossier.entry(value.clone()).or_insert_with(|| doc.clone());
            }
        }
        // глави: значення в порядку першої появи + перший документ
        // + мета для переходу: цитата речення й offset у тілі (FQA-03)
        let mut chapter_order: Vec<String> = Vec::new();
        let mut chapter_doc: HashMap<String, String> = HashMap::new();
        let mut chapter_meta: HashMap<String, (String, usize)> = HashMap::new();
        for (doc, value, is_dossier, sentence, char_start) in &g.entries {
            if *is_dossier {
                continue;
            }
            if !chapter_doc.contains_key(value) {
                chapter_order.push(value.clone());
                chapter_doc.insert(value.clone(), doc.clone());
                chapter_meta.insert(value.clone(), (sentence.clone(), *char_start));
            }
        }
        if chapter_order.is_empty() {
            continue;
        }

        // 4a) глава суперечить досьє
        if !dossier.is_empty() {
            let mut wrong: Vec<String> = chapter_order
                .iter()
                .filter(|v| !dossier.contains_key(*v))
                .cloned()
                .collect();
            wrong.sort();
            for value in wrong {
                let doc_a = chapter_doc[&value].clone();
                let (sent_a, cs_a) = chapter_meta[&value].clone();
                let (value_b, doc_b) = dossier.iter().next().unwrap();
                let payload = serde_json::json!({
                    "entity": g.name,
                    "kind": g.kind_field,
                    "kind_label": kind_label(&g.kind_field),
                    "value_a": value,
                    "doc_a": doc_a,
                    "value_b": value_b,
                    "doc_b": doc_b,
                    // FQA-03: перехід до місця в главі-джерелі (глава з
                    // відмінним значенням): цитата речення + offset у тілі
                    "doc": doc_a,
                    "sentence": sent_a,
                    "char_start": cs_a,
                    "message": format!(
                        "«{}» у главі «{}»: {} — «{}», а досьє каже «{}». Одне з двох — помилка.",
                        g.name, doc_a, kind_label(&g.kind_field), value, value_b
                    ),
                });
                add_alert(conn, "fact-vs-dossier", g.entity_id, &payload)?;
                report.vs_dossier += 1;
            }
        }

        // 4b) конфлікт між главами: ≥2 різних значення
        if chapter_order.len() >= 2 {
            let value_a = &chapter_order[0];
            let value_b = &chapter_order[1];
            // FQA-03: глава-джерело конфлікту = глава з ВІДМІННИМ значенням
            // (value_b — друге, відмінне від першопоявеного, значення)
            let doc_b = chapter_doc[value_b].clone();
            let (sent_b, cs_b) = chapter_meta[value_b].clone();
            let payload = serde_json::json!({
                "entity": g.name,
                "kind": g.kind_field,
                "kind_label": kind_label(&g.kind_field),
                "value_a": value_a,
                "doc_a": chapter_doc[value_a],
                "value_b": value_b,
                "doc_b": doc_b,
                "doc": doc_b,
                "sentence": sent_b,
                "char_start": cs_b,
                "message": format!(
                    "Конфлікт між главами: «{}» — {} «{}» в одній главі і «{}» в іншій. Оберіть істинне значення.",
                    g.name, kind_label(&g.kind_field), value_a, value_b
                ),
            });
            add_alert(conn, "fact-conflict", g.entity_id, &payload)?;
            report.conflicts += 1;
        }
    }

    Ok(report)
}
