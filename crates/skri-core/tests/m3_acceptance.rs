//! Acceptance-тести M3: barks, аудитор змінних, text-first граф наративу,
//! валідатор анімаційного епізоду, мют-тест, правила Show Bible.

use skri_core::{animation, narrative, ProjectStore};

#[test]
fn barks_first_class_with_duplicate_detection() {
    let (root, store) = skri_core::tests_common::tmp_project("barks");

    let b = narrative::Bark {
        id: 0,
        trigger: "ice-bomb-hit".into(),
        conditions: r#"{"state":"alert"}"#.into(),
        priority: 5,
        cooldown: 30,
        variants: vec![
            "Крижана бомба! Він дістав мене!".into(),
            "Лід! Мій плащ скував!".into(),
            "Холод... в кістках...".into(),
        ],
        entity_id: None,
    };
    let id = narrative::upsert_bark(&store, &b).expect("upsert");
    assert!(id > 0);

    // upsert за trigger — не створює дубля
    let b2 = narrative::Bark { variants: vec!["Новий варіант".into()], ..b.clone() };
    let id2 = narrative::upsert_bark(&store, &b2).unwrap();
    assert_eq!(id, id2);
    assert_eq!(narrative::barks(&store).unwrap().len(), 1);

    // дубль-варіант → алерт
    let b3 = narrative::Bark {
        variants: vec!["Те саме. Двічі.".into(), "те саме. двічі.".into()],
        ..b
    };
    narrative::upsert_bark(&store, &b3).unwrap();
    let dup_alerts: Vec<(i64, String)> = {
        use rusqlite::params;
        let mut st = store
            .conn()
            .prepare("SELECT id, payload_json FROM alerts WHERE kind='bark-duplicate'")
            .unwrap();
        st.query_map(params![], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    assert!(!dup_alerts.is_empty(), "дубль-варіант має дати алерт");

    // CSV-експорт: валідні колонки; b3 перезаписав ice-bomb-hit (2 варіанти) + 12 нових
    let b10 = narrative::Bark {
        id: 0,
        trigger: "low-hp".into(),
        conditions: "{}".into(),
        priority: 10,
        cooldown: 5,
        variants: (1..=12).map(|i| format!("Я мало не впав, номер {i}")).collect(),
        entity_id: None,
    };
    narrative::upsert_bark(&store, &b10).unwrap();
    let csv = narrative::export_barks_csv(&store).unwrap();
    assert!(csv.starts_with("trigger,conditions,priority,cooldown,variant_index,variant"));
    assert_eq!(csv.lines().count() - 1, 14, "2 (ice-bomb-hit після перезапису) + 12 (low-hp)");
    let _ = root;
}

#[test]
fn var_audit_matrix_replaces_manual_sheet() {
    let (root, store) = skri_core::tests_common::tmp_project("vars");
    std::fs::create_dir_all(root.join("dialogue")).unwrap();
    std::fs::write(
        root.join("dialogue").join("quest.md"),
        "%SET met_wise\nМудрець киває.\n%CHECK met_wise\n\n%SET sword_taken\nМеч забрано.\n%CHECK ghost_gate\nВорота дивляться.\n",
    ).unwrap();
    store.reindex().unwrap();

    let report = narrative::scan_dialogue(&store).unwrap();
    assert_eq!(report.ops, 4);

    let audit = narrative::var_audit(&store).unwrap();
    let met = audit.iter().find(|a| a.var_name == "met_wise").unwrap();
    assert!(!met.orphan_check && !met.dead_flag, "set+check = норма");
    let sword = audit.iter().find(|a| a.var_name == "sword_taken").unwrap();
    assert!(sword.dead_flag, "set без check = мертвий прапорець");
    let ghost = audit.iter().find(|a| a.var_name == "ghost_gate").unwrap();
    assert!(ghost.orphan_check, "check без set = сирота");
    // проблемні — зверху списку
    assert!(audit[0].dead_flag || audit[0].orphan_check);
    let _ = root;
}

#[test]
fn graph_audit_finds_unreachable_and_dangling() {
    let body = "\
-> початок
Мудрець мовчить.
[goto розвилка]
-> розвилка
[goto початок]
[goto невідомий_вузол]
-> прихована
Ніхто сюди не веде.
";
    let a = narrative::graph_audit(body);
    assert_eq!(a.total_nodes, 3);
    assert_eq!(a.entry_nodes, vec!["початок".to_string()]);
    assert!(a.dangling_gotos.contains(&"невідомий_вузол".to_string()), "goto в неіснуючу ноду");
    assert!(a.unreachable.contains(&"прихована".to_string()), "нода без входу = недосяжна");
}

#[test]
fn animation_episode_validator_and_mute_test() {
    let ep = "\
## СЦЕНА: COLD OPEN — Дах башти
КАЛЬДРІН: Знову ти?
ГРАФ: Небо не пам'ятає імен.
Вітер рвав плащ. Граф підвився з парапету.

## СЦЕНА: Дорога на захід
Вцілілі йшли мовчки. Попіл ложився на плечі.
Кінь хрипнув і впав.

## СЦЕНА: ФІНАЛ — Гачок
КАЛЬДРІН: Небо ж бо... воно пам'ятає все.
";
    let fmt = animation::EpisodeFormat::default();
    let check = animation::validate_episode(ep, &fmt);
    assert!(check.cold_open.is_some());
    assert!(check.hook.is_some());
    assert!(check.turns >= 2);

    // сцена-глухий діалог гасне в мют-тесті
    let talky = "## СЦЕНА: Крамниця\nА: мов.\nБ: мов.\nВ: мов.\nГ: мов.\nдія.\n";
    let beats = animation::mute_test(talky);
    assert_eq!(beats.len(), 1);
    assert!(beats[0].visual_ratio < 0.3, "ratio={}", beats[0].visual_ratio);
    assert!(beats[0].verdict.contains("гасне"));
}

#[test]
fn show_bible_rules_flag_possible_retcon() {
    let (root, store) = skri_core::tests_common::tmp_project("rules");
    std::fs::write(
        root.join("bible").join("rules.md"),
        "- Мертві не оживають у цьому світі :: мертвяк, оживання, некромантія\n- Магія не працює після заходу :: нічна магія\n",
    ).unwrap();
    store.reindex().unwrap();

    let clean = animation::check_bible_rules(&store, "Він ішов цілу ніч пішки.").unwrap();
    assert!(clean.is_empty());

    let violation = animation::check_bible_rules(&store, "Некромантія гриміла в долині.").unwrap();
    assert_eq!(violation.len(), 1);
    assert!(violation[0].contains("Мертві не оживають"));
    assert!(violation[0].contains("retcon чи помилка"));
    let _ = root;
}

#[test]
fn presets_game_and_animation_builtin_valid() {
    for name in ["game-narrative-uk", "animation-uk"] {
        let p = skri_core::presets::builtin(name)
            .unwrap_or_else(|| panic!("{name} має бути вбудованим"));
        assert_eq!(p.language, "uk");
        assert!(!p.entities.is_empty());
    }
    // опис містить типи сутностей
    let list = skri_core::presets::builtin_list();
    assert!(list.iter().any(|(n, _)| n == "game-narrative-uk"));
    assert!(list.iter().any(|(n, _)| n == "animation-uk"));
}
