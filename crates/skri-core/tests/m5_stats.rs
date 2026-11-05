//! Acceptance-тести M5: LitRPG Progression Engine (event-sourcing статів, §5.3).
//!
//! Критерії приймання:
//! - дельти `[stat: ...]` парсяться з manuscript-розмітки (+=/-= семантика);
//! - stat sheet на гл. N = сума дельт глав 1..N;
//! - ретроактивна правка ранньої глави → діф-звіт (глава, атрибут, старе → нове);
//! - від'ємні hp/mp → stat-error radar.

use skri_core::{stats, ProjectStore};

const CH1: &str = "\
---
title: Розділ 1
lang: uk
---

Кальдрін Сумрачний вийшов на стіни фортеці.
[stat: Кальдрін рівень +2]
[stat: Кальдрін hp +100]
[stat: Кальдрін xp += 150]
Старий Велес чекав біля воріт.
";

const CH2: &str = "\
---
title: Розділ 2
lang: uk
---

Бій у долині Мора був коротким.
[stat: Кальдрін hp -30]
[stat: Кальдрін xp +150]
[stat: Кальдрін mp -= 10]
";

fn tmpdir(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("skri-m5-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    base
}

fn make_project(name: &str) -> (std::path::PathBuf, ProjectStore) {
    let root = tmpdir(name);
    let store = ProjectStore::init(&root, "novelist-uk", "uk").expect("init");
    std::fs::write(root.join("manuscript").join("01.md"), CH1).unwrap();
    std::fs::write(root.join("manuscript").join("02.md"), CH2).unwrap();
    (root, store)
}

fn sheet_value(rows: &[stats::SheetRow], entity: &str, attr: &str) -> i64 {
    rows.iter()
        .find(|r| r.entity == entity && r.attr == attr)
        .map(|r| r.value)
        .unwrap_or_else(|| panic!("немає {entity}/{attr} у sheet"))
}

#[test]
fn parses_stat_deltas_with_add_sub_semantics() {
    let body = "[stat: Кальдрін рівень +2]\n\
                [stat: Кальдрін hp -30]\n\
                [stat: Кальдрін xp += 150]\n\
                [stat: Кальдрін mp -= 10]\n\
                Звичайний текст речення тут.\n\
                [stat: Кальдрін Сумрачний xp 25]\n";
    let d = stats::parse_deltas(body);
    assert_eq!(d.len(), 5, "усі 5 тегів мають спарситися: {d:?}");
    assert_eq!(d[0], stats::StatDelta { entity: "Кальдрін".into(), attr: "рівень".into(), value: 2 });
    assert_eq!(d[1].value, -30);
    assert_eq!(d[2].value, 150, "+= семантика");
    assert_eq!(d[3].value, -10, "-= семантика");
    // голе число = приріст; сутність з пробілом = все перед атрибутом
    assert_eq!(d[4], stats::StatDelta { entity: "Кальдрін Сумрачний".into(), attr: "xp".into(), value: 25 });
    // некоректні теги не падають парсер
    assert!(stats::parse_deltas("[stat: лишатри]").is_empty());
    assert!(stats::parse_deltas("[stat: Кальдрін рівень багато]").is_empty());
}

#[test]
fn snapshot_is_cumulative_sum_of_deltas() {
    let (_root, store) = make_project("cumul");
    store.reindex().unwrap();
    let rep = stats::rebuild(&store).unwrap();
    assert_eq!(rep.deltas, 6, "3 дельти в гл. 1 + 3 у гл. 2");

    // глава 1: рівень 2, hp 100, xp 150 (mp ще не існує)
    let s1 = stats::sheet(&store, 1).unwrap();
    assert_eq!(sheet_value(&s1, "Кальдрін", "рівень"), 2);
    assert_eq!(sheet_value(&s1, "Кальдрін", "hp"), 100);
    assert_eq!(sheet_value(&s1, "Кальдрін", "xp"), 150);
    assert!(!s1.iter().any(|r| r.attr == "mp"), "mp з'являється лише з гл. 2");

    // глава 2 = сума дельт глав 1..2
    let s2 = stats::sheet(&store, 2).unwrap();
    assert_eq!(sheet_value(&s2, "Кальдрін", "рівень"), 2);
    assert_eq!(sheet_value(&s2, "Кальдрін", "hp"), 70, "100 - 30");
    assert_eq!(sheet_value(&s2, "Кальдрін", "xp"), 300, "150 + 150");
    assert_eq!(sheet_value(&s2, "Кальдрін", "mp"), -10);

    // перебудова ідемпотентна: других алертів немає; mp=-10 у гл. 2 —
    // чесний спрацьований stat-error radar, а не помилка перебудови
    let rep2 = stats::rebuild(&store).unwrap();
    assert_eq!(rep2.changes, 0, "без правок — діф порожній");
    assert_eq!(rep2.errors, 1, "mp = -10 → радар");
    let alerts = stats::radar_alerts(&store).unwrap();
    assert_eq!(alerts.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&alerts[0].1).unwrap();
    assert_eq!(payload["attr"], "mp");
}

#[test]
fn retroactive_edit_produces_diff_report() {
    let (root, store) = make_project("retro");
    store.reindex().unwrap();
    stats::rebuild(&store).unwrap();

    // ретроактивна правка: у гл. 1 рівень +2 → +5
    std::fs::write(
        root.join("manuscript").join("01.md"),
        CH1.replace("[stat: Кальдрін рівень +2]", "[stat: Кальдрін рівень +5]"),
    )
    .unwrap();
    store.reindex().unwrap();
    let rep = stats::rebuild(&store).unwrap();
    assert_eq!(rep.changes, 1, "один (сутність, атрибут) зі змінами");

    let alerts = stats::change_alerts(&store).unwrap();
    assert_eq!(alerts.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&alerts[0].1).unwrap();
    assert_eq!(payload["entity"], "Кальдрін");
    assert_eq!(payload["attr"], "рівень");
    let changes = payload["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1, "рівень змінювався лише в гл. 1 → один змінений снапшот");
    assert_eq!(changes[0]["chapter"], 1);
    assert_eq!(changes[0]["old"], 2);
    assert_eq!(changes[0]["new"], 5);

    // sheet після правки
    let s2 = stats::sheet(&store, 2).unwrap();
    assert_eq!(sheet_value(&s2, "Кальдрін", "рівень"), 5);

    // повторна перебудова без змін — нових алертів немає (ідемпотентно)
    let rep2 = stats::rebuild(&store).unwrap();
    assert_eq!(rep2.changes, 0);
    assert!(stats::change_alerts(&store).unwrap().is_empty());
}

#[test]
fn negative_hp_triggers_radar() {
    let (root, store) = make_project("radar");
    // окремий рукопис: у гл. 1 велика втрата hp без попереднього приросту
    std::fs::write(
        root.join("manuscript").join("01.md"),
        "---\ntitle: Розділ 1\nlang: uk\n---\n\nДракон побив Кальдріна.\n[stat: Кальдрін hp -150]\n[stat: Кальдрін xp +50]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript").join("02.md"),
        "---\ntitle: Розділ 2\nlang: uk\n---\n\nКальдрін відпочивав.\n[stat: Кальдрін xp +10]\n",
    )
    .unwrap();
    store.reindex().unwrap();
    let rep = stats::rebuild(&store).unwrap();
    assert_eq!(rep.errors, 1, "hp = -150 → радар (один алерт на атрибут)");

    let alerts = stats::radar_alerts(&store).unwrap();
    assert_eq!(alerts.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&alerts[0].1).unwrap();
    assert_eq!(payload["entity"], "Кальдрін");
    assert_eq!(payload["attr"], "hp");
    let entries = payload["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["chapter"], 1);
    assert_eq!(entries[0]["value"], -150);

    // xp не в радарі (лише hp/mp), снапшоти не зіпсовані
    let s2 = stats::sheet(&store, 2).unwrap();
    assert_eq!(sheet_value(&s2, "Кальдрін", "xp"), 60);

    // лікуємо правкою — радар чистий
    std::fs::write(
        root.join("manuscript").join("01.md"),
        "---\ntitle: Розділ 1\nlang: uk\n---\n\nДракон побив Кальдріна.\n[stat: Кальдрін hp +150]\n[stat: Кальдрін xp +50]\n",
    )
    .unwrap();
    store.reindex().unwrap();
    let rep2 = stats::rebuild(&store).unwrap();
    assert_eq!(rep2.errors, 0);
    assert!(stats::radar_alerts(&store).unwrap().is_empty());
}
