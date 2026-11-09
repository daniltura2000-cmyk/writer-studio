//! M9: людські метадані — каталог видів алертів і людські описи пресетів.
//!
//! - `alerts_meta::catalog_json()` — усі kind системи з поясненнями uk/en/ru;
//! - `webnovelist-uk` — новий вбудований пресет серіаліста;
//! - усі вбудовані пресети мають людський `desc` і `best_for`.

use skri_core::alerts_meta;
use skri_core::presets;

/// Усі kind, які мають бути в каталозі (звірені з кодом: drift.rs, bible.rs,
/// narrative.rs, animation.rs, timeline.rs, sync.rs, crosspost.rs, stats.rs,
/// inkbridge.rs, pitch.rs, notes.rs та пресетами).
const EXPECTED_KINDS: &[&str] = &[
    "name-drift",
    "assertion-conflict",
    "bark-duplicate",
    "crosspost-stale",
    "timeline-backwards",
    "timeline-missing",
    "location-conflict",
    "sync-conflict",
    "fact-unverified",
    "episode-structure",
    "dialogue-streak",
    "mute-test",
    "graph-unreachable",
    "graph-dangling-goto",
    "var-orphan-check",
    "var-dead-flag",
    "stat-change",
    "stat-error",
    "episode-check",
    "ink-external-change",
    "pitch-followup",
    "broken-links",
    "orphan-notes",
    "stale-notes",
];

#[test]
fn catalog_covers_all_kinds_in_three_languages() {
    let cat = alerts_meta::catalog_json();
    let arr = cat.as_array().expect("catalog_json має бути масивом");
    assert!(arr.len() >= EXPECTED_KINDS.len(), "каталог менший за очікуване");

    for kind in EXPECTED_KINDS {
        let entry = arr
            .iter()
            .find(|e| e.get("kind").and_then(|k| k.as_str()) == Some(*kind))
            .unwrap_or_else(|| panic!("каталог не містить kind «{kind}»"));
        for field in [
            "title_uk",
            "title_en",
            "title_ru",
            "explain_uk",
            "explain_en",
            "explain_ru",
            "action_uk",
            "action_en",
            "action_ru",
        ] {
            let v = entry
                .get(field)
                .and_then(|x| x.as_str())
                .unwrap_or_default();
            assert!(
                !v.trim().is_empty(),
                "kind «{kind}»: порожнє поле {field}"
            );
        }
    }
}

#[test]
fn webnovelist_uk_is_valid_builtin() {
    let p = presets::builtin("webnovelist-uk")
        .unwrap_or_else(|| panic!("webnovelist-uk має бути вбудованим пресетом"));
    assert_eq!(p.language, "uk");
    assert!(!p.entities.is_empty());
    assert!(
        presets::BUILTIN.iter().any(|(n, _)| *n == "webnovelist-uk"),
        "webnovelist-uk не зареєстрований у BUILTIN"
    );
    for v in ["name-drift", "timeline", "crosspost-stale", "release"] {
        assert!(
            p.validators.get(v).copied().unwrap_or(false),
            "webnovelist-uk: валідатор {v} має бути увімкнений"
        );
    }
    assert!(p.desc.contains("іменами"), "desc має бути людським");
}

#[test]
fn all_builtin_presets_have_human_desc_and_best_for() {
    assert!(presets::BUILTIN.len() >= 6, "має бути щонайменше 6 пресетів");
    for (name, json) in presets::BUILTIN {
        let p = presets::PresetManifest::from_json(json)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!p.desc.trim().is_empty(), "{name}: порожній desc");
        assert!(!p.best_for.trim().is_empty(), "{name}: порожній best_for");
        assert!(
            p.desc.contains("Програма") || p.desc.contains("програма"),
            "{name}: desc має описувати, ЩО ПРОГРАМА РОБИТЬ, а не технічні терміни: «{}»",
            p.desc
        );
        assert!(
            p.best_for.starts_with("Для кого:"),
            "{name}: best_for має починатися з «Для кого:»: «{}»",
            p.best_for
        );
    }
    // builtin_list віддає людський опис (те саме джерело живить CLI і UI)
    let list = presets::builtin_list();
    let novelist = list
        .iter()
        .find(|(n, _)| n == "novelist-uk")
        .expect("novelist-uk у списку");
    assert!(
        novelist.1.contains("Для кого:"),
        "builtin_list має повертати людський опис, а не «мова: …; сутності: …»: «{}»",
        novelist.1
    );
}

#[test]
fn catalog_json_is_pure() {
    let a = alerts_meta::catalog_json();
    let b = alerts_meta::catalog_json();
    assert_eq!(a, b, "catalog_json має бути чистою функцією (ідемпотентною)");
    // і валідний JSON, який можна перечитати
    let text = serde_json::to_string(&a).expect("серіалізація");
    let reparsed: serde_json::Value = serde_json::from_str(&text).expect("ре-парсинг");
    assert_eq!(reparsed, a);
}
