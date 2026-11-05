//! Recap Generator (M4, специфікація §5.6 — Серійний командний центр).
//!
//! Rules-first: рекап будує Index Engine (entities/mentions) — детерміновано,
//! без AI. По діапазону глав (номер = порядок `manuscript/*.md` за іменем
//! файлу) збирає:
//! - топ-сутності по частотності згадок;
//! - нові сутності (перша згадка сутності в діапазоні = «новий персонаж»);
//! - кількість глав і слів.
//!
//! AI-режим (`--ai`) додає наративне резюме через ЛОКАЛЬНУ Ollama; модель —
//! з Model Router (задача `summarize`, авто = lapa-v0.1.2). Промпт = rules-дані
//! + текст діапазону (хвіст до 12000 символів на главу, максимум 3 глави).
//! Якщо Ollama недоступна — чесне повідомлення і fallback на rules-only
//! (файл рекапу все одно зберігається, помилкою це не є).

use crate::indexer;
use crate::models;
use crate::store::ProjectStore;
use crate::Result;
use std::collections::HashMap;
use std::path::PathBuf;

/// Максимум глав в AI-промпті.
pub const MAX_CHAPTERS_IN_PROMPT: usize = 3;
/// Максимум символів тексту ОДНІЄЇ глави в промпті (береться хвіст глави —
/// свіжа частина сюжету важливіша за початок).
pub const MAX_CHAPTER_CHARS: usize = 12_000;
/// Скільки топ-сутностей показувати в рекапі.
pub const TOP_LIMIT: usize = 5;

/// Глава рукопису: номер = позиція в сорті `manuscript/*.md` за іменем (1-based).
#[derive(Debug, Clone)]
pub struct ChapterRef {
    pub num: usize,
    /// шлях відносно кореня проєкту
    pub path: String,
    pub title: String,
    pub words: usize,
}

/// Сутність у рекапі з частотністю згадок у діапазоні.
#[derive(Debug, Clone)]
pub struct EntityFreq {
    pub name: String,
    pub mentions: usize,
    /// номер глави першої згадки сутності у ВСЬОМУ рукописі
    pub first_chapter: usize,
}

/// Rules-first рекап по діапазоні глав.
#[derive(Debug, Clone)]
pub struct RulesRecap {
    pub from: usize,
    pub to: usize,
    /// глави, що потрапили в діапазон
    pub chapters: Vec<ChapterRef>,
    /// сумарно слів у главах діапазону
    pub words: usize,
    /// топ-сутності по частотності згадок (спадання)
    pub top: Vec<EntityFreq>,
    /// сутності, чия ПЕРША згадка в рукописі — в цьому діапазоні
    pub new_entities: Vec<EntityFreq>,
}

/// Перелік глав рукопису: `manuscript/*.md`, сорт за іменем файлу,
/// номер глави = позиція в списку (1-based).
pub fn chapters(store: &ProjectStore) -> Result<Vec<ChapterRef>> {
    let dir = store.root.join("manuscript");
    let mut names: Vec<String> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.to_lowercase().ends_with(".md"))
            .collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    let mut out = Vec::with_capacity(names.len());
    for (i, name) in names.into_iter().enumerate() {
        let rel = format!("manuscript/{name}");
        let body = indexer::read_body(store, &rel)?;
        out.push(ChapterRef {
            num: i + 1,
            path: rel,
            title: name.trim_end_matches(".md").to_string(),
            words: indexer::count_words(&body),
        });
    }
    Ok(out)
}

/// Rules-first рекап по діапазону глав.
///
/// Дані беруться з Index Engine (таблиці `entities`/`mentions` після
/// `indexer::scan_project`) — жодних AI-викликів, повна детермінованість.
pub fn rules_recap(store: &ProjectStore, from: usize, to: usize) -> Result<RulesRecap> {
    let all = chapters(store)?;

    // Згадки Index Engine: (сутність, файл глави, кількість згадок у файлі)
    let mut stmt = store.conn().prepare(
        "SELECT e.canonical_form, d.path, COUNT(*) AS n
         FROM mentions m
         JOIN entities e ON e.id = m.entity_id
         JOIN docs d ON d.id = m.doc_id
         WHERE d.kind = 'manuscript'
         GROUP BY e.id, d.path",
    )?;
    let rows: Vec<(String, String, usize)> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)? as usize,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let chapter_num = |path: &str| -> Option<usize> {
        all.iter().find(|c| c.path == path).map(|c| c.num)
    };

    // перша поява кожної сутності по всіх главах рукопису
    let mut first_seen: HashMap<String, usize> = HashMap::new();
    // частотність згадок у діапазоні
    let mut counts: HashMap<String, (String, usize)> = HashMap::new();
    for (canonical, path, n) in rows {
        let Some(num) = chapter_num(&path) else { continue };
        let key = canonical.to_lowercase();
        first_seen.entry(key.clone()).or_insert(num);
        if num >= from && num <= to {
            let e = counts.entry(key).or_insert_with(|| (canonical.clone(), 0));
            e.1 += n;
        }
    }

    let mut entities: Vec<EntityFreq> = counts
        .into_iter()
        .map(|(key, (name, mentions))| EntityFreq {
            name,
            mentions,
            first_chapter: first_seen.get(&key).copied().unwrap_or(0),
        })
        .collect();

    // нові в діапазоні: ПЕРША згадка сутності (у всьому рукописі) — тут
    let mut new_entities: Vec<EntityFreq> = entities
        .iter()
        .filter(|e| e.first_chapter >= from && e.first_chapter <= to)
        .cloned()
        .collect();
    new_entities.sort_by(|a, b| a.first_chapter.cmp(&b.first_chapter).then(a.name.cmp(&b.name)));

    // топ по частотності
    entities.sort_by(|a, b| b.mentions.cmp(&a.mentions).then(a.name.cmp(&b.name)));
    entities.truncate(TOP_LIMIT);

    let mut chapters_in_range = Vec::new();
    let mut words = 0usize;
    for ch in &all {
        if ch.num >= from && ch.num <= to {
            words += ch.words;
            chapters_in_range.push(ch.clone());
        }
    }

    Ok(RulesRecap {
        from,
        to,
        chapters: chapters_in_range,
        words,
        top: entities,
        new_entities,
    })
}

/// Rules-рекап у вигляді markdown-списку.
pub fn recap_markdown(r: &RulesRecap) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Рекап: глави {}–{}\n\n", r.from, r.to));
    if r.chapters.is_empty() {
        out.push_str("(у діапазоні немає глав)\n");
        return out;
    }
    let list = r
        .chapters
        .iter()
        .map(|c| c.title.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!(
        "- Глав у діапазоні: {} ({})\n- Слів: {}\n",
        r.chapters.len(),
        list,
        r.words
    ));
    out.push_str("\n## Топ-сутності (згадки в діапазоні)\n\n");
    if r.top.is_empty() {
        out.push_str("(сутностей не знайдено)\n");
    }
    for (i, e) in r.top.iter().enumerate() {
        out.push_str(&format!("{}. **{}** — {} згадок\n", i + 1, e.name, e.mentions));
    }
    out.push_str("\n## Нові персонажі (перша поява в діапазоні)\n\n");
    if r.new_entities.is_empty() {
        out.push_str("(нових сутностей немає)\n");
    }
    for e in &r.new_entities {
        out.push_str(&format!("- **{}** (перша поява: гл. {})\n", e.name, e.first_chapter));
    }
    out
}

/// Хвіст рядка довжиною до `max_chars` символів (безпечний по char boundaries —
/// кирилиця багатобайтна, зріз «по байтах» тут заборонений).
pub fn clip_tail(s: &str, max_chars: usize) -> String {
    let cnt = s.chars().count();
    if cnt <= max_chars {
        return s.to_string();
    }
    s.chars().skip(cnt - max_chars).collect()
}

/// Текст глав для AI-промпта: ОСТАННІ до 3 глав діапазону,
/// кожна обрізана до останніх 12000 символів.
pub fn chapter_texts(store: &ProjectStore, recap: &RulesRecap) -> Result<Vec<(usize, String)>> {
    let sel: Vec<&ChapterRef> = recap
        .chapters
        .iter()
        .rev()
        .take(MAX_CHAPTERS_IN_PROMPT)
        .collect();
    let mut out = Vec::with_capacity(sel.len());
    for ch in sel.into_iter().rev() {
        let body = indexer::read_body(store, &ch.path)?;
        out.push((ch.num, clip_tail(&body, MAX_CHAPTER_CHARS)));
    }
    Ok(out)
}

/// Зібрати AI-промпт рекапу: rules-дані індексу + текст глав.
/// Публічна і чиста — будується без будь-якого виклику моделі (тестується окремо).
pub fn build_prompt(recap: &RulesRecap, texts: &[(usize, String)]) -> String {
    let mut p = String::new();
    p.push_str(
        "Ти помічник письменника. Напиши стислий рекап серії для читачів \
         за даними індексу рукопису та текстом глав.\n\n",
    );
    p.push_str("## Дані індексу рукопису (rules-first, детерміновано)\n\n");
    p.push_str(&recap_markdown(recap));
    p.push_str("\n## Текст глав (хвіст до 12000 символів на главу, максимум 3 глави)\n");
    for (num, text) in texts {
        p.push_str(&format!("\n### Глава {num}\n\n{text}\n"));
    }
    p.push_str(
        "\nНапиши рекап УКРАЇНСЬКОЮ: 2–4 абзаци подій + перелік нових персонажів. \
         Спирайся ЛИШЕ на наданий текст і дані індексу — нічого не вигадуй.",
    );
    p
}

/// Результат генерації рекапу.
#[derive(Debug)]
pub struct GenerateReport {
    pub path: PathBuf,
    /// чи встигла AI-модель (false = rules-only fallback)
    pub used_ai: bool,
    /// модель задачі «summarize» (Model Router), порожньо для rules-only
    pub model: String,
    /// чесне повідомлення про fallback (якщо AI не вдалося)
    pub note: Option<String>,
}

fn recap_path(store: &ProjectStore, from: usize, to: usize) -> PathBuf {
    store.root.join(format!("recap-{}-{}.md", from, to))
}

/// Модель задачі «summarize»: перевизначення користувача або авто-вибір
/// (перша опція каталогу = lapa-v0.1.2).
fn resolve_model(store: &ProjectStore) -> Result<String> {
    Ok(match store.model_for("summarize")? {
        Some((m, _)) => m,
        None => models::recommend("summarize")
            .map(|o| o.id)
            .unwrap_or_else(|| "lapa-v0.1.2".to_string()),
    })
}

/// Rules-only рекап: markdown у файл `recap-<від>-<до>.md` у папці проєкту.
/// Без AI, без мережі, детерміновано.
pub fn write_rules(store: &ProjectStore, from: usize, to: usize) -> Result<GenerateReport> {
    let recap = rules_recap(store, from, to)?;
    let md = recap_markdown(&recap);
    let path = recap_path(store, from, to);
    std::fs::write(&path, md)?;
    Ok(GenerateReport {
        path,
        used_ai: false,
        model: String::new(),
        note: None,
    })
}

/// AI-рекап через ЛОКАЛЬНУ Ollama. Модель — з Model Router (задача
/// «summarize», авто = lapa-v0.1.2). Якщо Ollama недоступна або модель
/// повернула порожнє — чесне повідомлення в `note` і fallback на rules-only;
/// файл рекапу зберігається в будь-якому разі (не падає).
///
/// `ai` — ін'єкція генератора для тестів (None = справжня Ollama).
pub fn generate_ai(
    store: &ProjectStore,
    from: usize,
    to: usize,
    ai: Option<&dyn Fn(&str) -> Result<String>>,
) -> Result<GenerateReport> {
    let recap = rules_recap(store, from, to)?;
    let texts = chapter_texts(store, &recap)?;
    let prompt = build_prompt(&recap, &texts);
    let model = resolve_model(store)?;
    let rules_md = recap_markdown(&recap);

    let generated: std::result::Result<String, String> = match ai {
        Some(f) => f(&prompt).map_err(|e| e.to_string()),
        None => crate::translate::ollama_generate(&model, &prompt).map_err(|e| e.to_string()),
    };

    let (used_ai, note, content) = match generated {
        Ok(t) if !t.trim().is_empty() => {
            let header = format!("# Рекап: глави {}–{} (AI, модель {model})\n\n", from, to);
            (true, None, format!("{header}{t}"))
        }
        Ok(_) => (
            false,
            Some("модель повернула порожню відповідь — збережено rules-only рекап".to_string()),
            rules_md,
        ),
        Err(e) => (
            false,
            Some(format!(
                "⚠ Ollama недоступна ({e}) — збережено rules-only рекап (без AI)"
            )),
            rules_md,
        ),
    };

    let path = recap_path(store, from, to);
    std::fs::write(&path, content)?;
    Ok(GenerateReport {
        path,
        used_ai,
        model,
        note,
    })
}
