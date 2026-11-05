//! Acceptance-тести M2: глосарій серії, локалізаційні ліміти, T-V реєстр,
//! композитор пресетів, перекладний пайплайн (dry-run без Ollama).

use skri_core::{glossary, loc, presets, translate, ProjectStore};
use std::path::PathBuf;

fn tmpstore(name: &str) -> (PathBuf, ProjectStore) {
    let root = std::env::temp_dir().join(format!("skri-m2-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = ProjectStore::init(&root, "novelist-uk", "uk").expect("init");
    (root, store)
}

#[test]
fn glossary_series_wide_survives_across_projects() {
    let (_r1, s1) = tmpstore("g1");
    let (_r2, s2) = tmpstore("g2");

    glossary::upsert(&s1, &glossary::GlossaryEntry {
        id: 0,
        term: "Кальдрін Сумрачний".into(),
        source_lang: "uk".into(),
        target_lang: "en".into(),
        canonical_translation: "Kaldrin the Gloom".into(),
        rule: "translate".into(),
        notes: "головний герой".into(),
        series_wide: true,
    })
    .unwrap();
    glossary::upsert(&s1, &glossary::GlossaryEntry {
        id: 0,
        term: "Скіф".into(),
        source_lang: "uk".into(),
        target_lang: "en".into(),
        canonical_translation: String::new(), // ще не вирішено
        rule: "transliterate".into(),
        notes: "козак-розвідник".into(),
        series_wide: false, // лише цей том
    })
    .unwrap();

    // upsert оновлює, не дублює
    glossary::upsert(&s1, &glossary::GlossaryEntry {
        id: 0,
        term: "Скіф".into(),
        source_lang: "uk".into(),
        target_lang: "en".into(),
        canonical_translation: "Skif".into(),
        rule: "transliterate".into(),
        notes: String::new(),
        series_wide: false,
    })
    .unwrap();
    assert_eq!(glossary::list(&s1, "uk", "en").unwrap().len(), 2);

    // series_wide переноситься в наступний том (порожній проєкт)
    let carried = glossary::series_export(&s1).unwrap();
    assert_eq!(carried.len(), 1, "series_wide=1 — лише Кальдрін");
    glossary::upsert(&s2, &carried[0]).unwrap();
    let in_v2 = glossary::find(&s2, "Кальдрін Сумрачний", "uk", "en").unwrap().unwrap();
    assert_eq!(in_v2.canonical_translation, "Kaldrin the Gloom");
}

#[test]
fn glossary_validation_blocks_wrong_names() {
    let (_r, store) = tmpstore("gv");
    glossary::upsert(&store, &glossary::GlossaryEntry {
        id: 0, term: "Кальдрін".into(), source_lang: "uk".into(), target_lang: "en".into(),
        canonical_translation: "Kaldrin".into(), rule: "translate".into(),
        notes: String::new(), series_wide: true,
    }).unwrap();

    // „Alice becoming Elise“ кейс: перекладач перейменував — ловиться
    let bad = glossary::validate_translation(&store, "Кальдрін зітхнув.", "Caldrin sighed.", "uk", "en").unwrap();
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].expected, "Kaldrin");
    assert!(!bad[0].unresolved);

    // невизначений термін → окремий тип порушення
    glossary::upsert(&store, &glossary::GlossaryEntry {
        id: 0, term: "Мора".into(), source_lang: "uk".into(), target_lang: "en".into(),
        canonical_translation: String::new(), rule: "translate".into(),
        notes: String::new(), series_wide: true,
    }).unwrap();
    let unres = glossary::validate_translation(&store, "долина Мора", "the valley", "uk", "en").unwrap();
    assert!(unres.iter().any(|v| v.unresolved), "потрібне рішення автора");

    // правильний переклад — чисто
    let ok = glossary::validate_translation(&store, "Кальдрін зітхнув.", "Kaldrin sighed.", "uk", "en").unwrap();
    assert!(ok.is_empty());
}

#[test]
fn limits_warn_writer_before_publishing() {
    // німецький +30%: рядок 42 симв з лімітом 40 → алерт в райтера
    let s = loc::LocalizedString {
        key: "d01_l03".into(),
        source_text: "Ти не пройдеш повз цю браму, мандрівнику!".into(),
        source_lang: "uk".into(),
        limits: [("de".to_string(), 40usize)].into_iter().collect(),
        context: [("speaker".to_string(), "вартовий".to_string())].into_iter().collect(),
    };
    let de = loc::check_limits(&s).remove(0);
    assert!(de.over && de.overflow > 0);
    assert!(de.advice.contains("скорочено"));
}

#[test]
fn tv_register_flags_mixing() {
    let r = loc::analyze_tv("Ти чого забрів? Вам же казали: ваш шлях тут закритий.");
    assert!(r.mixed);
    assert!(r.ty_hits > 0 && r.vy_hits > 0);
    assert_eq!(loc::analyze_tv("Твій шлях — твій вибір.").dominant, "ty");
    assert_eq!(loc::analyze_tv("Ваш вибір, панове.").dominant, "vy");
}

#[test]
fn po_export_carries_context() {
    let s = loc::LocalizedString {
        key: "d01_l01".into(),
        source_text: "Скажи їй \"ні\".".into(),
        source_lang: "uk".into(),
        limits: Default::default(),
        context: [("tone".to_string(), "ironic".to_string())].into_iter().collect(),
    };
    let po = loc::export_po(&[s], "en").unwrap();
    assert!(po.contains("#. tone: ironic"));
    assert!(po.contains("\\\"ні\\\""), "лапки екрановані");
}

#[test]
fn preset_compose_mixes_assets() {
    // структура з романіста + чекліст з нотатника + валідатори з обох
    let p = presets::compose(
        "entities:novelist-uk structure:novelist-uk checklists:note-taker-uk validators:note-taker-uk",
        "my-hybrid",
        None,
    )
    .expect("композит валідний");
    assert_eq!(p.preset, "my-hybrid");
    assert!(p.entities.iter().any(|e| e.name == "персонаж"), "entities з романіста");
    assert!(p.checklists.iter().any(|c| c.name == "оформлення нотатки"), "чекліст з нотатника");

    // зберегти/завантажити файлом
    let path = std::env::temp_dir().join(format!("hybrid-{}.json", std::process::id()));
    presets::save_to_json(&p, &path).unwrap();
    let reloaded = presets::load(path.to_str().unwrap()).unwrap();
    assert_eq!(reloaded.preset, "my-hybrid");
    let _ = std::fs::remove_file(path);

    // без entities — відмова з підказкою
    assert!(presets::compose("checklists:novelist-uk", "bad", None).is_err());
    // невідомий вид ассета — відмова
    assert!(presets::compose("що-то:novelist-uk", "bad2", None).is_err());
}

#[test]
fn translate_pipeline_dry_run_and_statuses() {
    let (root, store) = tmpstore("tr");
    std::fs::write(
        root.join("manuscript").join("01.md"),
        "---\ntitle: Гл 1\nlang: uk\n---\n\nКальдрін вийшов на стіни.\n\nМеч «Тиша» висів на спині.\n",
    ).unwrap();
    store.reindex().unwrap();

    // dry-run: промпти з глосарієм, БЕЗ виклику моделі (Ollama не потрібна)
    glossary::upsert(&store, &glossary::GlossaryEntry {
        id: 0, term: "Тиша".into(), source_lang: "uk".into(), target_lang: "en".into(),
        canonical_translation: "the Hush".into(), rule: "translate".into(),
        notes: "меч".into(), series_wide: true,
    }).unwrap();
    let prompts = translate::dry_run(&store, "manuscript/01.md", "uk", "en").unwrap();
    assert_eq!(prompts.len(), 2);
    assert!(prompts[0].contains("ГЛОСАРІЙ СЕРІЇ") || prompts[1].contains("ГЛОСАРІЙ СЕРІЇ"));

    // статуси: human → approved
    let segs = translate::prepare(&store, "manuscript/01.md", "en").unwrap();
    assert_eq!(segs[0].status, translate::STATUS_DRAFT);
    translate::set_translation(&store, segs[0].id, "Kaldrin walked the walls.", translate::STATUS_HUMAN).unwrap();
    let segs = translate::segments(&store, "manuscript/01.md", "en").unwrap();
    assert_eq!(segs[0].status, translate::STATUS_HUMAN);
    translate::set_translation(&store, segs[0].id, &segs[0].translated_text.clone(), translate::STATUS_APPROVED).unwrap();
    assert_eq!(
        translate::segments(&store, "manuscript/01.md", "en").unwrap()[0].status,
        translate::STATUS_APPROVED
    );
    // невідомий статус відкидається
    assert!(translate::set_translation(&store, segs[0].id, "x", "boh-znae-shcho").is_err());
}

#[test]
fn model_router_connects_to_translate_pipeline() {
    let (_r, store) = tmpstore("mr");
    // перевизначена модель перекладу читається пайплайном
    store.set_model("translate", "lapa-v0.1.2", false).unwrap();
    assert_eq!(store.model_for("translate").unwrap().unwrap().0, "lapa-v0.1.2");
}
