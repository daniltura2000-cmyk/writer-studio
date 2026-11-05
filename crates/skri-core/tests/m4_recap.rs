//! Acceptance-тести M4: Recap Generator (специфікація §5.6).
//!
//! - rules-only рекап: топ-сутності по частотності + нові-в-діапазоні + числа;
//! - діапазон поза межами рукопису → порожньо;
//! - AI-промпт будується БЕЗ виклику моделі (build_prompt публічна);
//! - AI-шлях: fallback на rules-only, якщо генератор недоступний (не падає).

use skri_core::{indexer, recap, ProjectStore};
use std::path::PathBuf;

const CH1: &str = "\
---
title: Розділ 1
lang: uk
---

Кальдрін Сумрачний вийшов на стіни фортеці. Вітер ніс попіл з долини Мора,
і Кальдрін заплющив очі. Меч «Тиша» висів на спині, як завжди.
Старий Велес чекав біля воріт.
";

const CH2: &str = "\
---
title: Розділ 2
lang: uk
---

Кальдрін повів загрізок уночі. Кальдрін Сумрачний більше не розмовляв з Велесом.
Кальдрін дивився на долину Мора, і Кальдрін не повернувся.
";

fn tmpdir(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("skri-recap-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    base
}

fn make_project(name: &str) -> (PathBuf, ProjectStore) {
    let root = tmpdir(name);
    let store = ProjectStore::init(&root, "novelist-uk", "uk").expect("init");
    std::fs::write(root.join("manuscript").join("01.md"), CH1).unwrap();
    std::fs::write(root.join("manuscript").join("02.md"), CH2).unwrap();
    store.reindex().unwrap();
    indexer::scan_project(&store).unwrap();
    (root, store)
}

#[test]
fn rules_recap_top_entities_and_numbers() {
    let (root, store) = make_project("top");

    let r = recap::rules_recap(&store, 1, 2).expect("rules_recap");
    // числа: 2 глави в діапазоні, слова = сума слів тіл (без frontmatter)
    assert_eq!(r.chapters.len(), 2);
    let expected_words = r
        .chapters
        .iter()
        .map(|c| indexer::count_words(&indexer::read_body(&store, &c.path).unwrap()))
        .sum::<usize>();
    assert_eq!(r.words, expected_words, "слів пораховано по тілах глав");
    assert!(r.words > 20);

    // топ-сутності: «Кальдрін» — 4 згадки в діапазоні (1 у гл.1 + 3 у гл.2),
    // «Кальдрін Сумрачний» — 2, «Мора» — 2
    assert_eq!(r.top[0].name, "Кальдрін");
    assert_eq!(r.top[0].mentions, 4);
    assert!(r.top.iter().any(|e| e.name == "Кальдрін Сумрачний" && e.mentions == 2));
    assert!(r.top.iter().any(|e| e.name == "Мора" && e.mentions == 2));
    // перша поява топ-сутностей — гл. 1
    assert!(r.top.iter().all(|e| e.first_chapter >= 1));

    // markdown-вивід містить усе це
    let md = recap::recap_markdown(&r);
    assert!(md.contains("# Рекап: глави 1–2"));
    assert!(md.contains("**Кальдрін** — 4 згадок"));
    assert!(md.contains("## Топ-сутності"));
    assert!(md.contains("## Нові персонажі"));

    // rules-only файл у папці проєкту
    let rep = recap::write_rules(&store, 1, 2).unwrap();
    assert!(!rep.used_ai);
    assert!(rep.path.exists());
    let written = std::fs::read_to_string(&rep.path).unwrap();
    assert_eq!(written, md, "файл = markdown рекапу");
    assert!(rep.path.ends_with("recap-1-2.md"));
    let _ = root;
}

#[test]
fn new_entities_are_those_first_seen_in_range() {
    let (_root, store) = make_project("new");

    // «Велесом» уперше з'являється в гл. 2 → новий у діапазоні 2–2
    let r2 = recap::rules_recap(&store, 2, 2).unwrap();
    assert!(r2.new_entities.iter().any(|e| e.name == "Велесом"),
        "нові в 2–2: {:?}", r2.new_entities.iter().map(|e| e.name.clone()).collect::<Vec<_>>());
    // «Мора» вперше з'явилась у гл. 1 → НЕ нова в діапазоні 2–2
    assert!(!r2.new_entities.iter().any(|e| e.name == "Мора"));
    // нова сутність має першу появу рівно в діапазоні
    assert!(r2.new_entities.iter().all(|e| e.first_chapter == 2));

    // у діапазоні 1–2 «Велесом» теж нова (перша поява гл. 2 належить діапазону)
    let r12 = recap::rules_recap(&store, 1, 2).unwrap();
    assert!(r12.new_entities.iter().any(|e| e.name == "Велесом"));
}

#[test]
fn empty_range_outside_manuscript() {
    let (_root, store) = make_project("empty");

    let r = recap::rules_recap(&store, 9, 12).unwrap();
    assert!(r.chapters.is_empty(), "глава 9 не існує");
    assert_eq!(r.words, 0);
    assert!(r.top.is_empty());
    assert!(r.new_entities.is_empty());

    // markdown — чесне «немає глав», а не порожній файл
    let md = recap::recap_markdown(&r);
    assert!(md.contains("(у діапазоні немає глав)"));

    // і файл зберігається з цим позначенням
    let rep = recap::write_rules(&store, 9, 12).unwrap();
    let written = std::fs::read_to_string(&rep.path).unwrap();
    assert!(written.contains("(у діапазоні немає глав)"));
}

#[test]
fn prompt_builds_without_model_call() {
    let (_root, store) = make_project("prompt");

    let r = recap::rules_recap(&store, 1, 2).unwrap();
    let texts = vec![(2usize, "Кальдрін дивився на долину Мора.".to_string())];
    let p = recap::build_prompt(&r, &texts);
    // rules-дані в промпті
    assert!(p.contains("Дані індексу рукопису"));
    assert!(p.contains("**Кальдрін** — 4 згадок"));
    // текст глави в промпті
    assert!(p.contains("### Глава 2"));
    assert!(p.contains("долину Мора"));
    // інструкція мовою проєкту
    assert!(p.contains("УКРАЇНСЬКОЮ"));
}

#[test]
fn chapter_texts_last_three_clipped_to_limit() {
    let (root, _store) = make_project("clip");
    // 5 глав, глава 4 — довша за ліміт (13000 укр. символів)
    for n in 3..=6 {
        let body = format!("---\ntitle: Гл {n}\nlang: uk\n---\n\n{}\n", "т".repeat(30));
        std::fs::write(root.join("manuscript").join(format!("0{n}.md")), body).unwrap();
    }
    let long_body = format!("---\ntitle: Гл довга\nlang: uk\n---\n\n{}\n", "К".repeat(13_000));
    std::fs::write(root.join("manuscript").join("07.md"), long_body).unwrap();
    let store = ProjectStore::open(&root).unwrap();
    store.reindex().unwrap();
    indexer::scan_project(&store).unwrap();

    let r = recap::rules_recap(&store, 1, 7).unwrap();
    assert_eq!(r.chapters.len(), 7);

    let texts = recap::chapter_texts(&store, &r).unwrap();
    // останні 3 глави діапазону: 5, 6, 7
    assert_eq!(texts.len(), recap::MAX_CHAPTERS_IN_PROMPT);
    assert_eq!(texts[0].0, 5);
    assert_eq!(texts[2].0, 7);
    // хвіст довгої глави обрізано до ліміту символів (по char boundaries)
    assert_eq!(texts[2].1.chars().count(), recap::MAX_CHAPTER_CHARS);
    assert!(texts[2].1.trim_end().ends_with('К'));
    // короткі глави не обрізано
    assert!(texts[0].1.chars().count() < recap::MAX_CHAPTER_CHARS);
}

#[test]
fn ai_path_falls_back_to_rules_when_unavailable() {
    let (_root, store) = make_project("fallback");

    // генератор «недоступний» (як Ollama вниз) → чесне повідомлення + rules-only
    let fail = |_: &str| -> skri_core::Result<String> {
        Err(skri_core::Error::Invalid(
            "Ollama недоступна на 127.0.0.1:11434".into(),
        ))
    };
    let rep = recap::generate_ai(&store, 1, 2, Some(&fail)).unwrap();
    assert!(!rep.used_ai, "fallback на rules-only, а не помилка");
    assert!(rep.note.as_deref().unwrap().contains("Ollama недоступна"));
    let written = std::fs::read_to_string(&rep.path).unwrap();
    assert!(written.contains("# Рекап: глави 1–2"));
    assert!(written.contains("**Кальдрін** — 4 згадок"));

    // успішний AI-генератор (стаб) → відповідь моделі у файлі
    let ok = |_: &str| -> skri_core::Result<String> { Ok("ШІ-РЕКАП: Кальдрін повернувся.".into()) };
    let rep2 = recap::generate_ai(&store, 1, 2, Some(&ok)).unwrap();
    assert!(rep2.used_ai);
    assert!(rep2.note.is_none());
    assert_eq!(rep2.model, "lapa-v0.1.2", "авто-модель задачі summarize = lapa-v0.1.2");
    let written2 = std::fs::read_to_string(&rep2.path).unwrap();
    assert!(written2.contains("ШІ-РЕКАП: Кальдрін повернувся."));
    assert!(written2.contains("(AI, модель lapa-v0.1.2)"));
}
