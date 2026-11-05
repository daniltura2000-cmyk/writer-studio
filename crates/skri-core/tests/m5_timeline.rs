//! Acceptance-тести M5: Timeline Engine (§5.5) — укр. фікстури.
//! - парсинг маркерів (inline + природні фрази укр./англ., явні дати)
//! - накопичення діб по главах з day_start
//! - стрибок назад = алерт (і без алерту з [time: flashback])
//! - глава без маркерів = ПОПЕРЕДЖЕННЯ (не алерт)
//! - локаційний календар: зведення + конфлікт в одну добу

use skri_core::{timeline, ProjectStore};
use timeline::TimeExpr;

fn tmpdir(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("skri-m5tl-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    base
}

fn make_project(name: &str, chapters: &[(&str, &str)]) -> (std::path::PathBuf, ProjectStore) {
    let root = tmpdir(name);
    let store = ProjectStore::init(&root, "novelist-uk", "uk").expect("init");
    for (fname, content) in chapters {
        std::fs::write(root.join("manuscript").join(fname), content).unwrap();
    }
    (root, store)
}

fn exprs_of(body: &str) -> Vec<TimeExpr> {
    timeline::parse_time_markers(body)
        .into_iter()
        .map(|t| t.expr)
        .collect()
}

#[test]
fn parses_marker_forms() {
    // inline-теги
    let e = exprs_of("[time: +3 днів]");
    assert_eq!(e, vec![TimeExpr::OffsetDays(3)], "+3 днів → OffsetDays(3)");

    let e = exprs_of("[time: день 12]");
    assert_eq!(e, vec![TimeExpr::AbsoluteDay(12)], "день 12 → AbsoluteDay(12)");

    let e = exprs_of("[time: наступний ранок]");
    assert_eq!(
        e,
        vec![TimeExpr::NextDay { tod: Some("ранок") }],
        "наступний ранок → NextDay(ранок)"
    );

    let e = exprs_of("[time: flashback]");
    assert_eq!(e, vec![TimeExpr::Flashback]);

    // природні фрази (укр.)
    let e = exprs_of("Наступного ранку він вийшов.");
    assert!(e.contains(&TimeExpr::NextDay { tod: Some("ранок") }), "наступного ранку");

    let e = exprs_of("Він повернувся через три дні.");
    assert!(e.contains(&TimeExpr::OffsetDays(3)), "через три дні");

    let e = exprs_of("За тиждень усе скінчилось.");
    assert!(e.contains(&TimeExpr::OffsetDays(7)), "за тиждень");

    let e = exprs_of("Ввечері того ж дня вони зустрілись.");
    assert!(
        e.contains(&TimeExpr::SameDay { tod: Some("вечір") }),
        "ввечері того ж дня → SameDay(вечір)"
    );

    let e = exprs_of("На світанку горизонт рожевів.");
    assert!(e.contains(&TimeExpr::SameDay { tod: Some("ранок") }), "на світанку");

    let e = exprs_of("Пізніше він замислився.");
    assert!(e.contains(&TimeExpr::SameDay { tod: None }), "пізніше");

    // явна дата «12 жовтня» → день-у-році григоріанського календаря
    let e = exprs_of("Це сталося 12 жовтня.");
    assert!(
        e.contains(&TimeExpr::AbsoluteDay(285)),
        "12 жовтня = день 285 (нема високосного зсуву), є {:?}",
        e
    );

    // англійські фрази
    let e = exprs_of("The next morning, he left.");
    assert!(e.contains(&TimeExpr::NextDay { tod: Some("ранок") }), "next morning");

    let e = exprs_of("Three days later, the letter arrived.");
    assert!(e.contains(&TimeExpr::OffsetDays(3)), "three days later");

    let e = exprs_of("At dawn the horns sounded.");
    assert!(e.contains(&TimeExpr::SameDay { tod: Some("ранок") }), "at dawn");

    // звичайна проза без часових фраз — маркерів немає
    let e = exprs_of("Кальдрін пішов до брами і довго дивився на долину Мора.");
    assert!(e.is_empty(), "звичайна проза не має давати маркерів, є {e:?}");
}

#[test]
fn accumulates_days_across_chapters() {
    let ch1 = "---\ntitle: Розділ 1\n---\n\nКальдрін прокинувся у фортеці.\n\n[time: +2 дні]\n\nВін зібрав загін.\n";
    let ch2 = "---\ntitle: Розділ 2\n---\n\nНаступного ранку загін рушив у долину.\n";
    let ch3 = "---\ntitle: Розділ 3\nday_start: 10\n---\n\n[time: день 15]\n\nШтурм розпочався.\n";
    let (_root, store) = make_project("accum", &[("01.md", ch1), ("02.md", ch2), ("03.md", ch3)]);
    store.reindex().unwrap();
    let rep = timeline::rebuild(&store).unwrap();

    assert_eq!(rep.chapters, 3);
    assert_eq!(rep.points, 6, "по 2 точки на главу (початок + маркер): {rep:?}");

    let pts = timeline::points(&store).unwrap();
    let days: Vec<i64> = pts.iter().map(|p| p.day).collect();
    assert_eq!(
        days,
        vec![1, 3, 3, 4, 10, 15],
        "гл.1: 1→(+2)→3; гл.2: продовжує з 3→наступного ранку→4; гл.3: day_start 10→день 15"
    );
    // час доби поширюється з маркера
    let morning = pts.iter().find(|p| p.time_of_day == "ранок").expect("ранок у гл. 2");
    assert_eq!(morning.day, 4);
    assert_eq!(morning.doc_path, "manuscript/02.md");
}

#[test]
fn backwards_jump_alerts_and_flashback_resolves() {
    let ch1 = "---\ntitle: Розділ 1\nday_start: 5\n---\n\nКальдрін штурмував ворота.\n";
    let ch2 = "---\ntitle: Розділ 2\n---\n\n[time: день 2]\n\nТри дні потому все було спокійно.\n";
    let ch3 = "---\ntitle: Розділ 3\nday_start: 5\n---\n\n[time: flashback]\n\n[time: день 2]\n\nФлешбек у дитинство.\n";
    let (_root, store) = make_project("back", &[("01.md", ch1), ("02.md", ch2), ("03.md", ch3)]);
    store.reindex().unwrap();

    let rep = timeline::rebuild(&store).unwrap();
    assert_eq!(rep.backwards, 1, "тільки гл.2 (5→2) без флешбеку — алерт; {rep:?}");
    let alerts = timeline::backwards_alerts(&store).unwrap();
    assert_eq!(alerts.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&alerts[0].1).unwrap();
    assert_eq!(payload["from"], 5);
    assert_eq!(payload["to"], 2);
    assert!(payload["doc"].as_str().unwrap().contains("02.md"));

    // ідемпотентність: повторний прогін без правок не плодить алертів
    let rep2 = timeline::rebuild(&store).unwrap();
    assert_eq!(rep2.backwards, 1);
    assert_eq!(timeline::backwards_alerts(&store).unwrap().len(), 1);
}

#[test]
fn chapter_without_markers_is_warning_not_alert() {
    let ch1 = "---\ntitle: Розділ 1\n---\n\n[time: +1 день]\n\nКальдрін рушив у путь.\n";
    let ch2 = "---\ntitle: Розділ 2\n---\n\nТихий вечір без пригод, нічого не сталося.\n";
    let (_root, store) = make_project("missing", &[("01.md", ch1), ("02.md", ch2)]);
    store.reindex().unwrap();

    let rep = timeline::validate(&store).unwrap();
    assert_eq!(rep.missing.len(), 1, "тільки гл.2 без маркерів");
    assert_eq!(rep.missing[0].0, "manuscript/02.md");
    // це ПОПЕРЕДЖЕННЯ: у БД алертів timeline-missing немає
    let count: i64 = store
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM alerts WHERE kind='timeline-missing'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0, "timeline-missing не має бути алертом у БД");
    assert!(rep.backwards.is_empty());
    assert!(rep.conflicts.is_empty());
}

#[test]
fn loc_calendar_aggregates_by_chapters() {
    let ch1 = "---\ntitle: Розділ 1\n---\n\n[loc: Кальдрін = фортеця]\n\n[loc: Велес = брама]\n\nБій.\n";
    let ch2 = "---\ntitle: Розділ 2\n---\n\n[loc: Кальдрін = долина (шлях з фортеці)]\n\nПохід.\n";
    let (_root, store) = make_project("locs", &[("01.md", ch1), ("02.md", ch2)]);
    store.reindex().unwrap();
    timeline::rebuild(&store).unwrap();

    let rows = timeline::locs(&store).unwrap();
    assert_eq!(rows.len(), 3);
    let k1 = rows.iter().find(|r| r.doc_path.contains("01.md") && r.entity == "Кальдрін").unwrap();
    assert_eq!(k1.location, "фортеця");
    assert!(!k1.moved);
    let k2 = rows.iter().find(|r| r.doc_path.contains("02.md") && r.entity == "Кальдрін").unwrap();
    assert_eq!(k2.location, "долина");
    assert!(k2.moved, "(шлях ...) = маркер переміщення");
    // шлях переміщення видно в тезі маркера
    let tags = timeline::parse_loc_markers("Кальдрін пішов. [loc: Кальдрін = долина (шлях з фортеці)]");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].path.as_deref(), Some("шлях з фортеці"));
    let v = rows.iter().find(|r| r.entity == "Велес").unwrap();
    assert_eq!(v.location, "брама");
}

#[test]
fn location_conflict_same_day() {
    // А: одна доба, дві локації без переміщення → алерт
    let ch1 = "---\ntitle: Розділ 1\nday_start: 4\n---\n\n[loc: Кальдрін = фортеця]\n\nОборона.\n";
    let ch2 = "---\ntitle: Розділ 2\nday_start: 4\n---\n\n[loc: Кальдрін = долина]\n\nПатруль.\n";
    let (root_a, store_a) = make_project("conflict", &[("01.md", ch1), ("02.md", ch2)]);
    store_a.reindex().unwrap();
    let rep = timeline::rebuild(&store_a).unwrap();
    assert_eq!(rep.loc_conflicts, 1, "фортеця vs долина в добу 4 без переміщення");
    let alerts = timeline::conflict_alerts(&store_a).unwrap();
    assert_eq!(alerts.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&alerts[0].1).unwrap();
    assert_eq!(payload["entity"], "Кальдрін");
    assert_eq!(payload["day"], 4);
    let _ = root_a;

    // Б: той самий конфлікт, але з маркером переміщення (шлях) → чисто
    let ch2m = "---\ntitle: Розділ 2\nday_start: 4\n---\n\n[loc: Кальдрін = долина (шлях)]\n\nПатруль.\n";
    let (_root_b, store_b) = make_project("conflict-moved", &[("01.md", ch1), ("02.md", ch2m)]);
    store_b.reindex().unwrap();
    let rep2 = timeline::rebuild(&store_b).unwrap();
    assert_eq!(rep2.loc_conflicts, 0, "маркер переміщення знімає конфлікт");
}

#[test]
fn world_calendar_json_md_and_default() {
    // 1) .skri/calendar.json
    let (root, store) = make_project("cal-json", &[("01.md", "---\ntitle: Р.1\n---\n\n[time: +3 дні]\n")]);
    std::fs::write(
        root.join(".skri").join("calendar.json"),
        r#"{"months":[{"name":"Тарвен","days":30},{"name":"Вален","days":40}],"days_of_week":["День Попелу","День Води"]}"#,
    )
    .unwrap();
    store.reindex().unwrap();
    let cal = timeline::load_calendar(&root).unwrap();
    assert_eq!(cal.source, ".skri/calendar.json");
    assert_eq!(cal.year_length(), 70);
    assert!(cal.format_day(35).contains("5 Вален"), "{:?}", cal.format_day(35));
    // явна дата в кастомному календарі: «12 Тарвеня» → день-у-році 12
    let tags = timeline::parse_time_markers_in("Це сталося 12 Тарвеня.", &cal);
    assert_eq!(
        tags.iter().map(|t| t.expr.clone()).collect::<Vec<_>>(),
        vec![TimeExpr::AbsoluteDay(12)],
        "дата парситься у календарі світу"
    );
    // явна дата в кастомному календарі через day_of_year працює
    assert_eq!(cal.day_of_year(1, 5), Some(35));
    assert_eq!(cal.day_of_year(0, 31), None, "у Тарвені лише 30 днів");

    // 2) bible/calendar.md з frontmatter
    let (root2, _s2) = make_project("cal-md", &[("01.md", "---\ntitle: Р.1\n---\n\nТекст.\n")]);
    std::fs::write(
        root2.join("bible").join("calendar.md"),
        "---\ntype: calendar\nmonths: Тарвен=30, Вален=40\ndays_of_week: День Попелу, День Води\n---\n\nКалендар світу.\n",
    )
    .unwrap();
    let cal2 = timeline::load_calendar(&root2).unwrap();
    assert_eq!(cal2.source, "bible/calendar.md");
    assert_eq!(cal2.year_length(), 70);

    // 3) за замовчуванням — григоріанський
    let (root3, _s3) = make_project("cal-def", &[("01.md", "---\ntitle: Р.1\n---\n\nТекст.\n")]);
    let cal3 = timeline::load_calendar(&root3).unwrap();
    assert_eq!(cal3.source, "default-gregorian");
    assert!(cal3.format_day(285).starts_with("12 жовтня"), "{}", cal3.format_day(285));
}
