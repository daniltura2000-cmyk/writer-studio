//! Index Engine — «додаток, який читає рукопис» (M1).
//!
//! Правила перші (детерміновано, без AI, кирилиця+латиниця); локальна LLM —
//! підсилювач у M2. Слабкі кандидати (одинична згадка, лише на початку речення)
//! реєструються як `auto-weak`: у біблію вони не потрапляють, але беруть участь
//! у drift-детекції — опечатка при ПЕРШІЙ згадці імені теж має ловитись.

use crate::drift;
use crate::stopwords;
use crate::store::ProjectStore;
use crate::Result;
use rusqlite::{params, OptionalExtension};
use std::collections::{HashMap, HashSet};

/// Токен з байтовими offsets у тілі документа.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// Речення з offset'ом.
#[derive(Debug, Clone)]
pub struct Sentence {
    pub text: String,
    pub start: usize,
}

/// Кандидат у власні назви.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// канонічна форма (найчастіше написання)
    pub canonical: String,
    /// усе написання, що зустрічалося (нижній регістр → кількість)
    pub forms: HashMap<String, usize>,
    pub total_count: usize,
    /// згадок НЕ на початку речення (надійніший сигнал власної назви)
    pub non_initial: usize,
    /// сильний: non_initial ≥ 1, total ≥ 3 або пара «Ім'я Прізвище»; слабкий — інакше
    pub strong: bool,
}

/// Згадка для запису в БД.
#[derive(Debug)]
pub struct Mention {
    pub entity_key: String, // lowercase форми
    pub surface: String,
    pub start: usize,
    pub end: usize,
    pub sentence: String,
}

/// Результат витягу з одного документа.
#[derive(Debug, Default)]
pub struct Extraction {
    pub candidates: Vec<Candidate>,
    pub mentions: Vec<Mention>,
}

// --- Стоп-слова ---
// M10: великі списки uk/ru/en винесені в окремий модуль `stopwords.rs`
// (кожен список ≥ 100 слів: вводні, займенники, прислівники, частки,
// дієслова мовлення). Порівняння — у нижньому регістрі після нормалізації.

/// Сильний кандидат: non_initial згадок ≥ 1 АБО total ≥ 3 (підвищено з 2
/// у M10 — одиночне слово, згадане двічі, ще не персонаж).
const STRONG_TOTAL: usize = 3;

fn is_stopword(t: &str) -> bool {
    let lower = t.to_lowercase();
    lower
        .split_whitespace()
        .all(|w| stopwords::is_stopword(w))
        || stopwords::is_stopword(&lower)
}

/// Публічний фільтр службових слів для інших модулів (наприклад,
/// style.rs відфільтровує службові слова з repeat-перевірки).
pub fn is_stopword_word(word_lower: &str) -> bool {
    stopwords::is_stopword(word_lower)
}

/// Чи символ «словесний» (буква/цифра, вкл. кирилицю) — для внутрішніх апострофів/дефісів.
fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '\u{2019}' || c == '’' || c == '-' || c == '‑'
}

/// Розбити текст на токени з байтовими offsets.
pub fn tokenize(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = text[i..].chars().next().unwrap();
        // дефіс не може ПОЧИНАТИ токен (інакше «- Привет» робило «Привет»
        // non-initial-згадкою і сильним кандидатом); всередині слова — можно.
        if !word_char(c) || c == '-' || c == '‑' {
            i += c.len_utf8();
            continue;
        }
        let start = i;
        let mut end = i + c.len_utf8();
        let mut prev = c;
        let mut j = end;
        while j < bytes.len() {
            let ch = text[j..].chars().next().unwrap();
            let ok = if ch.is_alphanumeric() {
                true
            } else if ch == '\'' || ch == '\u{2019}' || ch == '-' {
                // апостроф/дефіс лише між літерами
                let next_alpha = text[j + ch.len_utf8()..]
                    .chars()
                    .next()
                    .map(|n| n.is_alphabetic())
                    .unwrap_or(false);
                prev.is_alphabetic() && next_alpha
            } else {
                false
            };
            if !ok {
                break;
            }
            end = j + ch.len_utf8();
            prev = ch;
            j = end;
        }
        tokens.push(Token {
            text: text[start..end].to_string(),
            start,
            end,
        });
        i = end;
    }
    tokens
}

/// Розбити на речення (з офсетами). Розділювачі: . ! ? … та новий рядок.
pub fn sentences(text: &str) -> Vec<Sentence> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, c) in text.char_indices() {
        let boundary = matches!(c, '.' | '!' | '?' | '…' | '\n');
        if boundary {
            let s = text[start..i].trim();
            if !s.is_empty() {
                let lead_ws = text[start..].len() - text[start..].trim_start().len();
                out.push(Sentence {
                    text: s.to_string(),
                    start: start + lead_ws,
                });
            }
            start = i + c.len_utf8();
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        let lead_ws = text[start..].len() - text[start..].trim_start().len();
        out.push(Sentence {
            text: tail.to_string(),
            start: start + lead_ws,
        });
    }
    out
}

pub fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Витягти кандидати у власні назви + згадки з тіла документа.
///
/// Сигнали власної назви:
/// 1) велика літера НЕ на початку речення;
/// 2) частотність ≥ 3 (для одиночних слів);
/// 3) пара «Ім'я Прізвище» (дві великі підряд).
///
/// Правила M10 (фікс «Казалось/Почему»):
/// - стоп-слова uk/ru/en (модуль [`crate::stopwords`]) не кандидати;
/// - слово, що зустрічається ЛИШЕ на початку речення і де-небудь у тексті
///   нижнім регістром, — не кандидат (це звичайне слово, capitalized тільки
///   через позицію в реченні).
/// Усе інше велике одиночне — слабкий кандидат (лише для drift-детекції).
pub fn extract_candidates(body: &str) -> Extraction {
    let mut candidates: HashMap<String, Candidate> = HashMap::new();
    let mut mentions: Vec<Mention> = Vec::new();

    // Двопрохідний скан: спершу збираємо всі токени (з offsetами), щоб
    // знати, які форми зустрічаються нижнім регістром десь у тексті.
    let sents: Vec<(String, usize, Vec<Token>)> = sentences(body)
        .into_iter()
        .map(|s| {
            let toks = tokenize(&s.text);
            (s.text, s.start, toks)
        })
        .collect();

    let mut lower_seen: HashSet<String> = HashSet::new();
    for (_, _, toks) in &sents {
        for tok in toks {
            if let Some(first) = tok.text.chars().next() {
                if first.is_alphabetic() && first.is_lowercase() {
                    lower_seen.insert(tok.text.to_lowercase());
                }
            }
        }
    }

    let mut pair_keys: HashSet<String> = HashSet::new();

    for (sent_text, sent_start, toks) in &sents {
        let mut i = 0usize;
        while i < toks.len() {
            let tok = &toks[i];
            if !starts_upper(&tok.text) || tok.text.chars().any(|c| c.is_numeric()) {
                i += 1;
                continue;
            }
            // пара «Ім'я Прізвище»?
            if i + 1 < toks.len()
                && starts_upper(&toks[i + 1].text)
                && !is_stopword(&tok.text)
            {
                let pair = format!("{} {}", tok.text, toks[i + 1].text);
                let abs_start = sent_start + tok.start;
                let abs_end = sent_start + toks[i + 1].end;
                let key = pair.to_lowercase();
                pair_keys.insert(key.clone());
                // пара «Ім'я Прізвище» — завжди кандидат (signal сильний)
                register(
                    &mut candidates,
                    &mut mentions,
                    &pair,
                    abs_start,
                    abs_end,
                    sent_text.clone(),
                    true,
                );
                i += 2;
                continue;
            }
            if !is_stopword(&tok.text) {
                let lower = tok.text.to_lowercase();
                let initial = i == 0;
                // ГОЛОВНИЙ детектор «Казалось»: слово з великої ТІЛЬКИ тому,
                // що стоїть на початку речення, а нижнім регістром воно
                // зустрічається десь у тексті → це не власна назва.
                if initial && lower_seen.contains(&lower) {
                    i += 1;
                    continue;
                }
                let abs_start = sent_start + tok.start;
                let abs_end = sent_start + tok.end;
                register(
                    &mut candidates,
                    &mut mentions,
                    &tok.text,
                    abs_start,
                    abs_end,
                    sent_text.clone(),
                    !initial,
                );
            }
            i += 1;
        }
    }

    // сила кандидата: згадка не на початку речення АБО згадок ≥ 3 (M10:
    // поріг піднято з 2); пара «Ім'я Прізвище» — завжди сильна.
    // Слабкі не викидаємо — вони йдуть у БД як `auto-weak` для drift-детекції.
    let mut out: Vec<Candidate> = candidates
        .into_values()
        .map(|mut c| {
            c.strong = c.non_initial >= 1
                || c.total_count >= STRONG_TOTAL
                || pair_keys.contains(&c.canonical.to_lowercase());
            c
        })
        .collect();
    out.sort_by(|a, b| {
        b.total_count
            .cmp(&a.total_count)
            .then(a.canonical.cmp(&b.canonical))
    });
    Extraction {
        candidates: out,
        mentions,
    }
}

fn starts_upper(t: &str) -> bool {
    t.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
}

fn register(
    candidates: &mut HashMap<String, Candidate>,
    mentions: &mut Vec<Mention>,
    form: &str,
    start: usize,
    end: usize,
    sentence: String,
    non_initial: bool,
) {
    let key = form.to_lowercase();
    let entry = candidates.entry(key.clone()).or_insert_with(|| Candidate {
        canonical: form.to_string(),
        forms: HashMap::new(),
        total_count: 0,
        non_initial: 0,
        strong: false,
    });
    *entry.forms.entry(key.clone()).or_insert(0) += 1;
    entry.total_count += 1;
    if non_initial {
        entry.non_initial += 1;
    }
    mentions.push(Mention {
        entity_key: key,
        surface: form.to_string(),
        start,
        end,
        sentence,
    });
}

/// Просканувати рукопис проєкту: перебудувати entities+mentions (source=auto*)
/// і прогнати drift-детектор. Слабкі кандидати, що не злилися з сильними, видаляються.
///
/// FQA14 (ідемпотентність згадок): таблиця `mentions` ПОВНІСТЮ перебудовується
/// щоразу (єдиний записувач — ця функція). Раніше авто-сутності видалялись
/// каскадом, але сутність, що збігається з manual-досьє («Олександр»), була
/// крізь `upsert_entity` і її СТАРІ згадки лишались — повторні Check накопичували
/// й множили збіги (26 → 30 → 32 при незмінених файлах). Тепер згадки = реальна
/// кількість входжень форм імені у главу, повторні rebuild/reindex/check
/// число не змінюють.
pub fn scan_project(store: &ProjectStore) -> Result<ScanReport> {
    let conn = store.conn();
    // FQA14: чистий перебудов — згадки НЕ накопичуються між прогонами
    conn.execute("DELETE FROM mentions", [])?;
    conn.execute("DELETE FROM entities WHERE source LIKE 'auto%'", [])?;
    // авто-алерти перебудовуються щоразу (assertion-conflict біблії це не стосується)
    conn.execute("DELETE FROM alerts WHERE kind='name-drift'", [])?;
    let mut report = ScanReport::default();

    let mut stmt = conn.prepare("SELECT id, path FROM docs WHERE kind='manuscript'")?;
    let docs: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (doc_id, rel) in docs {
        let body = read_body(store, &rel)?;
        let ex = extract_candidates(&body);
        for c in &ex.candidates {
            let entity_id = upsert_entity(conn, &c.canonical, c.strong)?;
            for m in ex
                .mentions
                .iter()
                .filter(|m| m.entity_key == c.canonical.to_lowercase())
            {
                conn.execute(
                    "INSERT INTO mentions(entity_id, doc_id, char_start, char_end, surface_form, sentence)
                     VALUES (?1,?2,?3,?4,?5,?6)",
                    params![entity_id, doc_id, m.start as i64, m.end as i64, m.surface, m.sentence],
                )?;
            }
        }
        report.candidates += ex.candidates.len();
        report.mentions += ex.mentions.len();
    }

    // drift по всіх auto-сутностях (вкл. слабкі — ловить опечатку при першій згадці);
    // алерти створює сам drift::detect
    report.drift_groups = drift::detect(conn)?.len();
    // FQA14: слабкі, що не злилися з сильними — шум у БІБЛІЇ, але їхні згадки —
    // реальні входження форм імені. Якщо нормалізована основа weak-форми
    // («Олександра» → «олександр») збігається з manual-досьє, згадка
    // перешивається на нього (форма додається в aliases без дубля) — інакше
    // повторний скан «втрачав» і потім знову накопичував входження форм імені.
    {
        let weak: Vec<(i64, String)> = {
            let mut stmt =
                conn.prepare("SELECT id, canonical_form FROM entities WHERE source='auto-weak'")?;
            let rows: Vec<(i64, String)> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };
        let manuals: Vec<(i64, String)> = {
            let mut stmt = conn
                .prepare("SELECT id, canonical_form FROM entities WHERE source='manual'")?;
            let rows: Vec<(i64, String)> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };
        for (wid, wname) in weak {
            let norm = drift::strip_case_suffix(&wname);
            let target = manuals
                .iter()
                .find(|(_, n)| drift::strip_case_suffix(n) == norm)
                .map(|(id, _)| *id);
            if let Some(mid) = target {
                conn.execute(
                    "UPDATE mentions SET entity_id=?2 WHERE entity_id=?1",
                    params![wid, mid],
                )?;
                // форма → aliases manual-сутності (без дубля)
                let current: String = conn
                    .query_row(
                        "SELECT aliases_json FROM entities WHERE id=?1",
                        params![mid],
                        |r| r.get(0),
                    )
                    .unwrap_or_else(|_| "[]".to_string());
                let mut aliases: Vec<String> =
                    serde_json::from_str(&current).unwrap_or_default();
                if !aliases.iter().any(|a| a.eq_ignore_ascii_case(&wname)) {
                    aliases.push(wname.clone());
                    conn.execute(
                        "UPDATE entities SET aliases_json=?2 WHERE id=?1",
                        params![mid, serde_json::to_string(&aliases)?],
                    )?;
                }
            }
            // слабка сутність у біблію не потрапляє; незлі згадки каскадно зникають
            conn.execute("DELETE FROM entities WHERE id=?1", params![wid])?;
        }
    }
    Ok(report)
}

/// Прочитати тіло документа (без frontmatter) з диска.
pub fn read_body(store: &ProjectStore, rel: &str) -> Result<String> {
    let raw = std::fs::read_to_string(store.root.join(rel))?;
    let parsed = crate::frontmatter::parse(&raw);
    Ok(parsed.body)
}

fn upsert_entity(conn: &rusqlite::Connection, canonical: &str, strong: bool) -> Result<i64> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM entities WHERE lower(canonical_form)=lower(?1)",
            params![canonical],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        if strong {
            // слабка згадка в одному документі + сильна в іншому → підсилюємо
            conn.execute("UPDATE entities SET source='auto' WHERE id=?1", params![id])?;
        }
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO entities(type, canonical_form, source, created_at)
         VALUES ('персонаж', ?1, ?2, strftime('%s','now'))",
        params![canonical, if strong { "auto" } else { "auto-weak" }],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Звіт сканування.
#[derive(Debug, Default)]
pub struct ScanReport {
    pub candidates: usize,
    pub mentions: usize,
    pub drift_groups: usize,
}
