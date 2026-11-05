//! Acceptance-тести M0+M1 (укр. fixtures).
//! Відповідають критеріям приймання зі специфікації:
//! - консистентність: зміна імені ловиться у всіх згадках (§9.1 у спрощенні)
//! - drift: Kaldrin/Kaldryn групується
//! - біблія: assertions + конфлікт атрибута
//! - пресети: вбудовані валідні, імпорт файлом

use skri_core::{bible, drift, frontmatter, indexer, models, presets, ProjectStore};
use std::path::Path;

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

/// Дрейф-варіант: «Кальдрін» vs «Кальдрин» (різне написання).
const CH3: &str = "\
---
title: Розділ 3
lang: uk
---

Кальдрин знову повів загрізок, хоч ніхто не просив.
Долина Мора дихала жаром.
";

fn tmpdir(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("skri-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    base
}

fn make_project(name: &str, ch1: &str, ch2: &str, ch3: Option<&str>) -> (std::path::PathBuf, ProjectStore) {
    let root = tmpdir(name);
    let store = ProjectStore::init(&root, "novelist-uk", "uk").expect("init");
    std::fs::write(root.join("manuscript").join("01.md"), ch1).unwrap();
    std::fs::write(root.join("manuscript").join("02.md"), ch2).unwrap();
    if let Some(ch3) = ch3 {
        std::fs::write(root.join("manuscript").join("03.md"), ch3).unwrap();
    }
    (root, store)
}

#[test]
fn frontmatter_and_fts_search() {
    let (_root, store) = make_project("fts", CH1, CH2, None);
    let stats = store.reindex().expect("reindex");
    assert_eq!(stats.docs, 2);
    assert!(stats.words > 10);

    let hits = store.search("Тиша", 5).expect("search");
    assert!(!hits.is_empty(), "FTS має знайти «Тиша»");
    assert!(hits.iter().any(|(p, _, _)| p.contains("01.md")));
}

#[test]
fn extracts_entities_and_mentions_with_offsets() {
    let (_root, store) = make_project("ents", CH1, CH2, None);
    store.reindex().unwrap();
    let report = indexer::scan_project(&store).unwrap();
    assert!(report.candidates >= 3, "має знайти Кальдрін Сумрачний, Кальдрін, Велес, Мора");
    assert!(report.mentions >= 5);

    let cards = bible::cards(&store).unwrap();
    let kaldrin = cards.iter().find(|c| c.canonical.starts_with("Кальдрін")).expect("Кальдрін у біблії");
    assert!(kaldrin.mention_total >= 3);
    // згадка в обох документах
    assert!(kaldrin.docs.iter().any(|(p, _)| p.contains("01.md")));
    assert!(kaldrin.docs.iter().any(|(p, _)| p.contains("02.md")));
}

#[test]
fn detects_name_drift_variants() {
    let (_root, store) = make_project("drift", CH1, CH2, Some(CH3));
    store.reindex().unwrap();
    let report = indexer::scan_project(&store).unwrap();
    assert_eq!(report.drift_groups, 1, "Кальдрін/Кальдрин = одна drift-група");

    let alerts = drift::unresolved_alerts(store.conn()).unwrap();
    assert!(!alerts.is_empty());
    let payload: serde_json::Value = serde_json::from_str(&alerts[0].1).unwrap();
    let variants: Vec<&str> = payload["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(variants.len() == 2, "варіанти: {variants:?}");
}

#[test]
fn bible_assertions_and_conflict() {
    let (root, store) = make_project("bible", CH1, CH2, None);
    std::fs::write(
        root.join("bible").join("kaldrin.md"),
        "---\nname: Кальдрін Сумрачний\ntype: персонаж\n---\n\nочі: зелені\nзброя: меч «Тиша»\n",
    )
    .unwrap();
    store.reindex().unwrap();
    indexer::scan_project(&store).unwrap();
    bible::merge_auto_into_manual(&store).unwrap();
    let rep = bible::sync_bible(&store).unwrap();
    assert_eq!(rep.entities, 1);
    assert_eq!(rep.assertions, 2);
    assert_eq!(rep.conflicts, 0);

    let cards = bible::cards(&store).unwrap();
    let k = cards.iter().find(|c| c.canonical == "Кальдрін Сумрачний").unwrap();
    assert!(k.assertions.contains(&("очі".into(), "зелені".into(), "canonical".into())));

    // тепер конфлікт: очі карі в другому джерелі біблії
    std::fs::write(
        root.join("bible").join("kaldrin-2.md"),
        "---\nname: Кальдрін Сумрачний\ntype: персонаж\n---\n\nочі: карі\n",
    )
    .unwrap();
    let rep2 = bible::sync_bible(&store).unwrap();
    assert_eq!(rep2.conflicts, 1, "очі зелені vs карі = конфлікт канону");
}

#[test]
fn manual_entity_absorbs_auto_candidates() {
    let (root, store) = make_project("merge", CH1, CH2, None);
    std::fs::write(
        root.join("bible").join("kaldrin.md"),
        "---\nname: Кальдрін Сумрачний\ntype: персонаж\n---\n\nстатус: живий\n",
    )
    .unwrap();
    store.reindex().unwrap();
    indexer::scan_project(&store).unwrap();
    // спершу біблія: manual-картка поглинає auto-сутність тієї ж назви
    bible::sync_bible(&store).unwrap();
    let merged = bible::merge_auto_into_manual(&store).unwrap();
    let _ = merged;
    let cards = bible::cards(&store).unwrap();
    let matches: Vec<_> = cards
        .iter()
        .filter(|c| c.canonical.eq_ignore_ascii_case("Кальдрін Сумрачний"))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "повна форма має існувати ОДНА (manual поглинув auto-дублікат)"
    );
    assert!(
        matches[0].mention_total >= 2,
        "згадки-auto перешиті на manual-сутність"
    );
    let _ = root;
}

#[test]
fn presets_builtin_and_file() {
    for (name, _) in presets::BUILTIN {
        assert!(presets::builtin(name).is_some(), "{name} валідний");
    }
    assert!(presets::load("неіснуючий-пресет").is_err());

    // імпорт файлом
    let path = std::env::temp_dir().join(format!("custom-preset-{}.json", std::process::id()));
    std::fs::write(
        &path,
        r#"{"preset":"custom","language":"uk","entities":[{"name":"сцена"}]}"#,
    )
    .unwrap();
    let p = presets::load(path.to_str().unwrap()).expect("custom preset з файлу");
    assert_eq!(p.preset, "custom");
    let _ = std::fs::remove_file(path);
}

#[test]
fn reindex_is_idempotent() {
    let (root, store) = make_project("idem", CH1, CH2, None);
    let s1 = store.reindex().unwrap();
    let s2 = store.reindex().unwrap();
    assert_eq!(s1.docs, s2.docs);
    assert_eq!(s2.reindexed, 0, "без змін — нічого не переіндексовується");
    let _ = root;
}

#[test]
fn similarity_thresholds() {
    assert!(drift::similarity("Кальдрін", "Кальдрин") >= 0.82);
    assert!(drift::similarity("Кальдрін", "Велес") < 0.5);
    assert_eq!(drift::similarity("меч", "меч"), 1.0);
}

#[test]
fn sentences_and_tokens_keep_offsets() {
    let body = "Кальдрін пішов. Велес чекав.";
    let sents = indexer::sentences(body);
    assert_eq!(sents.len(), 2);
    let toks = indexer::tokenize(body);
    let first = &toks[0];
    assert_eq!(&body[first.start..first.end], "Кальдрін");
}

#[test]
fn model_router_auto_and_manual_buttons() {
    let (_root, store) = make_project("models", CH1, CH2, None);

    // автоматичний вибір по суті задачі: рекомендована = перша локальна
    let rec = models::recommend("translate").expect("translate у каталозі");
    assert!(rec.local, "автовибір ніколи не хмара");
    store.set_model("translate", &rec.id, true).unwrap();
    assert_eq!(
        store.model_for("translate").unwrap(),
        Some((rec.id.clone(), true))
    );

    // ручний вибір «кнопкою»: перевизначення
    store.set_model("translate", "lapa-v0.1.2", false).unwrap();
    assert_eq!(
        store.model_for("translate").unwrap(),
        Some(("lapa-v0.1.2".into(), false))
    );

    // каталог повний: кожна кнопка має опції, авто-вибір валідний
    for t in models::catalog() {
        assert!(!t.options.is_empty(), "{} без опцій", t.id);
        assert!(models::recommend(&t.id).is_some());
        assert!(t.options[0].local, "{}: авто = локальна", t.id);
    }
    // невідома задача відкидається
    assert!(store.set_model("неіснуюча", "x", true).is_err());
}

#[test]
fn frontmatter_body_offset_matches() {
    let doc = "---\ntitle: X\n---\n\nТіло";
    let p = frontmatter::parse(doc);
    assert_eq!(&doc[p.body_start..], "\n\nТіло");
    let _ = Path::new("");
}
