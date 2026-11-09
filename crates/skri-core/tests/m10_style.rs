//! M10: стоп-слова в Index Engine + перевірки стилю + ручні досьє біблії.
//!
//! Два реальні дефекти з рукопису російською, які тут фіксуються тестами:
//! 1. «Казалось» і «Почему» витягнулись як персонажі (стоп-слова не
//!    відфільтровані, capitalized лише через позицію в реченні);
//! 2. «Знайдені проблеми: 0» — жодних перевірок стилю/грамотності.

use skri_core::alerts_meta;
use skri_core::bible;
use skri_core::indexer;
use skri_core::stopwords;
use skri_core::style;
use skri_core::tests_common::tmp_project;
use skri_core::ProjectStore;

// --- A. Index Engine: стоп-слова і розумніша евристика ---

#[test]
fn kazalos_at_sentence_start_is_not_candidate() {
    // «Казалось» з великої ТІЛЬКИ тому, що стоїть на початку речення,
    // а нижнім регістром слово зустрічається десь у тексті.
    let body = "Казалось, все скінчено. Он нічого й не казалось, просто тиша стояла.";
    let ex = indexer::extract_candidates(body);
    assert!(
        ex.candidates.is_empty(),
        "«Казалось» не має бути кандидатом, але: {:?}",
        ex.candidates
            .iter()
            .map(|c| c.canonical.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn kazalos_mid_sentence_capital_also_not_candidate() {
    // «Казалось» всередині речення з великої, але в нижньому регістрі
    // воно деінсь у тексті (та це й стоп-слово) — не кандидат.
    let body = "Він зітхнув: Казалось, кінець. А насправді ніхто не казалось йому про це.";
    let ex = indexer::extract_candidates(body);
    assert!(
        !ex.candidates.iter().any(|c| c.canonical.to_lowercase() == "казалось"),
        "«Казалось» всередині речення теж не кандидат"
    );
}

#[test]
fn full_name_pair_is_still_candidate() {
    // ПІБ пара «Ім'я Прізвище» — сильний кандидат навіть після нових правил.
    let body = "Марія Коваленко підняла голову. Далі Марія Коваленко заговорила тихо.";
    let ex = indexer::extract_candidates(body);
    let hit = ex
        .candidates
        .iter()
        .find(|c| c.canonical == "Марія Коваленко")
        .expect("пара «Ім'я Прізвище» має бути кандидатом");
    assert!(hit.strong, "пара має бути сильним кандидатом");
}

#[test]
fn single_word_needs_three_mentions_or_non_initial() {
    // одиночне слово, 2 згадки (обидві не на початку речення) — сильне
    // (non_initial ≥ 1); а 2 згадки ТІЛЬКИ на початках речень, без
    // нижнього регістру деінсь — теж кандидат, але тепер слабкий.
    let strong_body = "Велес мовчав. Далі Велес повернувся і Велес зник.";
    let ex = indexer::extract_candidates(strong_body);
    let hit = ex
        .candidates
        .iter()
        .find(|c| c.canonical == "Велес")
        .expect("Велес має бути кандидатом");
    assert!(hit.strong);

    // поріг total підвищено з 2 до 3: 2 згадки, обидві на початку речення,
    // слово ніде нижнім регістром → слабкий кандидат (auto-weak, не в біблію)
    let weak_body = "Збруя блиснула. Збруя впала.";
    let ex = indexer::extract_candidates(weak_body);
    let hit = ex
        .candidates
        .iter()
        .find(|c| c.canonical == "Збруя")
        .expect("Збруя має лишитись слабким кандидатом для drift");
    assert!(!hit.strong, "2 згадки на початках речень — не сильний кандидат");
}

// --- B. style.rs: перевірки стилю і грамотності ---

#[test]
fn tautology_maslo_maslyane_caught() {
    let f = style::check_text("Масло масляне вийшло зовсім неочікувано.");
    let hit = f.iter().find(|x| x.kind == "style-tautology");
    assert!(hit.is_some(), "«масло масляне» має ловитись як тавтологія");
    assert!(!hit.unwrap().sentence.is_empty());
}

#[test]
fn repeat_three_times_in_one_sentence_caught() {
    let f = style::check_text("Тіні повзли, тіні росли, і тіні зникали.");
    let hit = f.iter().find(|x| x.kind == "style-repeat-word");
    assert!(hit.is_some(), "повтор ≥3 разів у реченні має ловитись");
}

#[test]
fn service_word_said_three_times_not_caught() {
    // службове слово «сказав» — стоп-фільтр, настирливий повтор НЕ ловиться
    let f = style::check_text("Сказав він, сказав вона, сказав я і замовк.");
    assert!(
        f.iter().all(|x| x.kind != "style-repeat-word"),
        "службові слова не мають ловитись repeat-перевіркою"
    );
}

#[test]
fn double_space_caught() {
    let f = style::check_text("Перше речення.  Друге речення йде слідом.");
    assert!(f.iter().any(|x| x.kind == "style-double-space"));
}

#[test]
fn dash_orphan_is_tip_finding() {
    let f = style::check_text("- Привіт, сказав він тихо.");
    let hit = f.iter().find(|x| x.kind == "style-dash-orphan");
    assert!(hit.is_some(), "дефіс замість тире має ловитись");
}

// --- Ідемпотентність run_style_checks і alerts_meta ---

#[test]
fn run_style_checks_is_idempotent_and_stores_alerts() {
    let (root, store) = tmp_project("m10-style-idem");
    std::fs::write(
        root.join("manuscript/01.md"),
        "Масло масляне. Тіні над тінями і тінями зросли.  Подвійний пробіл теж.",
    )
    .unwrap();
    store.reindex().unwrap();

    let first = style::run_style_checks(root.to_str().unwrap()).unwrap();
    assert!(!first.is_empty(), "має бути хоч одна знахідка");
    assert!(first.iter().all(|a| a.kind.starts_with("style-")));

    let second = style::run_style_checks(root.to_str().unwrap()).unwrap();
    assert_eq!(
        first.len(),
        second.len(),
        "повторний прогін не має плодити алертів"
    );
    // усі 4 kind описані в каталозі людською
    let cat = alerts_meta::catalog_json();
    for kind in [
        "style-tautology",
        "style-repeat-word",
        "style-double-space",
        "style-dash-orphan",
    ] {
        assert!(
            cat.as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == kind),
            "каталог не містить {kind}"
        );
    }
}

// --- D. bible.rs: ручні досьє ---

#[test]
fn manual_card_adds_entity_without_alerts() {
    let (root, store) = tmp_project("m10-dossier");
    let card = bible::ManualCard {
        name: "Кальдрін Сумрачний".into(),
        aliases: vec!["Кальдрін".into(), "Каль".into()],
        kind: "персонаж".into(),
        notes: "очі: сірі\nзброя: двуручний меч «Тиша»\nВільний текст-нотатка без ключа.".into(),
    };
    bible::add_manual_card(root.to_str().unwrap(), &card).expect("add_manual_card");

    // файл на диску з правильним frontmatter
    let raw = std::fs::read_to_string(root.join("bible/кальдрін-сумрачний.md")).unwrap();
    assert!(raw.contains("name: Кальдрін Сумрачний"));
    assert!(raw.contains("aliases: Кальдрін, Каль"));
    assert!(raw.contains("type: персонаж"));

    // sync_bible → EntityCard присутня
    store.reindex().unwrap();
    let rep = bible::sync_bible(&store).unwrap();
    assert!(rep.entities >= 1);
    let cards = bible::cards(&store).unwrap();
    let hit = cards
        .iter()
        .find(|c| c.canonical == "Кальдрін Сумрачний")
        .expect("картка досьє має з'явитись у біблії");
    assert_eq!(hit.entity_type, "персонаж");
    assert!(hit.aliases.contains(&"Каль".to_string()));
    assert!(
        hit.assertions.iter().any(|(a, v, _)| a == "очі" && v == "сірі"),
        "notes з ключем: значення стають assertions"
    );

    // оновлення: той самий name перезаписує файл, дубліката немає
    let updated = bible::ManualCard {
        name: "Кальдрін Сумрачний".into(),
        aliases: vec!["Кальдрін".into()],
        kind: "персонаж".into(),
        notes: "очі: зелені".into(),
    };
    bible::add_manual_card(root.to_str().unwrap(), &updated).unwrap();
    let md_files: Vec<_> = std::fs::read_dir(root.join("bible"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(md_files.len(), 1, "оновлення досьє не має плодити файлів");

    // від картки досьє НЕ має бути алертів
    let n_alerts: i64 = store
        .conn()
        .query_row("SELECT COUNT(*) FROM alerts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n_alerts, 0, "ручна картка без конфліктів не має давати алертів");
}

#[test]
fn stopwords_lists_are_loaded_and_russian_input_clean() {
    // инваріант: великі списки завантажені
    assert!(stopwords::is_stopword("казалось"));
    assert!(stopwords::is_stopword("почему"));
    // рукопис російською: звичайні слова НЕ кандидати, ім'я — кандидат
    let body = "Почему он ушёл, казалось, никто не понял. Всё-таки Иван Петров вернулся.";
    let ex = indexer::extract_candidates(body);
    let names: Vec<String> = ex
        .candidates
        .iter()
        .map(|c| c.canonical.clone())
        .collect();
    assert!(
        names.iter().all(|n| n == "Иван Петров"),
        "зі стоп-рукопису має лишитись лише ПІБ, а не {names:?}"
    );
}

#[test]
fn project_scan_no_longer_extracts_kazalos() {
    // кінцева перевірка дефекту: повний scan_project проєкту з російським
    // рукописом не витягує «Казалось» як сутність (алертів від нього немає).
    let (root, store) = tmp_project("m10-scan");
    std::fs::write(
        root.join("manuscript/01.md"),
        "Казалось, конец. Почему он вернулся? Дальше ничего не случилось.\n\
         Иван Петров ждал у двери. Иван Петров закурил.",
    )
    .unwrap();
    store.reindex().unwrap();
    indexer::scan_project(&store).unwrap();
    let conn = store.conn();
    let kazalos: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE lower(canonical_form) IN ('казалось','почему','дальше')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kazalos, 0, "стоп-слова не мають стати сутностями");
    let ivan: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entities WHERE canonical_form = 'Иван Петров'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ivan, 1, "ПІБ пара має бути витягнута як і раніше");
    let _ = ProjectStore::open(&root).unwrap();
}
