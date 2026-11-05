//! Index Engine — «додаток, який читає рукопис» (M1).
//!
//! Правила перші (детерміновано, без AI, кирилиця+латиниця); локальна LLM —
//! підсилювач у M2. Слабкі кандидати (одинична згадка, лише на початку речення)
//! реєструються як `auto-weak`: у біблію вони не потрапляють, але беруть участь
//! у drift-детекції — опечатка при ПЕРШІЙ згадці імені теж має ловитись.

use crate::drift;
use crate::store::ProjectStore;
use crate::Result;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;

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
    /// сильний: non_initial ≥ 1 або total ≥ 2; слабкий — інакше
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

// --- Стоп-слова: UA + EN (мінімум, розширюється пресетами) ---
const STOPWORDS: &[&str] = &[
    // укр.
    "а", "б", "без", "би", "був", "була", "було", "були", "бути", "в", "вже", "він", "вона",
    "воно", "все", "всі", "всього", "де", "для", "до", "є", "й", "і", "із", "або", "же", "з",
    "за", "зі", "як", "який", "яка", "яке", "коли", "хто", "що", "чого", "так", "такий", "там",
    "той", "та", "те", "ті", "ти", "ви", "у", "це", "цей", "ця", "ці", "її", "їх", "його",
    "їм", "ним", "нею", "ми", "вони", "я", "мене", "мені", "мій", "моя", "не", "ні",
    "ну", "про", "під", "по", "при", "сьогодні", "тоді", "теж", "тільки", "тут", "хоч",
    "чи", "через", "щоб", "ось", "дуже", "ще", "мабуть", "може", "можна", "коли", "дуже",
    "розділ", "частина", "глава",
    // англ.
    "the", "a", "an", "and", "or", "but", "if", "of", "to", "in", "on", "at", "for", "with",
    "from", "by", "is", "are", "was", "were", "be", "been", "it", "its", "this", "that", "he",
    "she", "they", "them", "his", "her", "their", "i", "you", "we", "my", "your", "our", "as",
    "not", "no", "so", "do", "did", "has", "have", "had", "will", "would", "can", "could",
    "there", "here", "when", "where", "who", "what", "why", "how", "all", "any", "each", "then",
    "than", "into", "over", "after", "before", "about", "chapter", "book",
];

fn is_stopword(t: &str) -> bool {
    let lower = t.to_lowercase();
    lower
        .split_whitespace()
        .all(|w| STOPWORDS.contains(&w))
        || STOPWORDS.contains(&lower.as_str())
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
        if !word_char(c) {
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
/// 2) частотність ≥ 2;
/// 3) пара «Ім'я Прізвище» (дві великі підряд).
/// Усе інше велике одиночне — слабкий кандидат (лише для drift-детекції).
pub fn extract_candidates(body: &str) -> Extraction {
    let mut candidates: HashMap<String, Candidate> = HashMap::new();
    let mut mentions: Vec<Mention> = Vec::new();

    for sent in sentences(body) {
        let toks = tokenize(&sent.text);
        let mut i = 0usize;
        while i < toks.len() {
            let tok = &toks[i];
            if !starts_upper(&tok.text) || tok.text.chars().any(|c| c.is_numeric()) {
                i += 1;
                continue;
            }
            // пара «Ім'я Прізвище»?
            if i + 1 < toks.len() && starts_upper(&toks[i + 1].text) && !is_stopword(&tok.text)
            {
                let pair = format!("{} {}", tok.text, toks[i + 1].text);
                let abs_start = sent.start + tok.start;
                let abs_end = sent.start + toks[i + 1].end;
                // пара «Ім'я Прізвище» — завжди кандидат (signal сильний)
                let non_initial = true;
                register(
                    &mut candidates,
                    &mut mentions,
                    &pair,
                    abs_start,
                    abs_end,
                    sent.text.clone(),
                    non_initial,
                );
                i += 2;
                continue;
            }
            if !is_stopword(&tok.text) {
                let abs_start = sent.start + tok.start;
                let abs_end = sent.start + tok.end;
                let non_initial = i > 0;
                register(
                    &mut candidates,
                    &mut mentions,
                    &tok.text,
                    abs_start,
                    abs_end,
                    sent.text.clone(),
                    non_initial,
                );
            }
            i += 1;
        }
    }

    // сила кандидата: згадка не на початку речення АБО згадок ≥ 2.
    // Слабкі не викидаємо — вони йдуть у БД як `auto-weak` для drift-детекції.
    let mut out: Vec<Candidate> = candidates
        .into_values()
        .map(|mut c| {
            c.strong = c.non_initial >= 1 || c.total_count >= 2;
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
pub fn scan_project(store: &ProjectStore) -> Result<ScanReport> {
    let conn = store.conn();
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
    // слабкі, що не злилися з сильними — шум; згадки каскадно видаляються
    conn.execute("DELETE FROM entities WHERE source='auto-weak'", [])?;
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
