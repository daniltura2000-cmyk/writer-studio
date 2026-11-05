//! Acceptance-тести M4: Release Planner (§5.6) — Kanban глав + прогноз буфера.
//!
//! Критерії приймання:
//! - реєстр глав будуюється з frontmatter (`pub_status`/`pub_date`/`pub_platforms`),
//!   auto-scan manuscript/*.md, порядок з ведучих цифр імені файла;
//! - перебудова ідемпотентна (другий прогін не міняє рядків і не плодить переходів);
//! - зміна статусу в frontmatter → перехід у журналі chapters_log + зсув по канбану;
//! - прогноз буфера з фіксованою швидкістю: слів/день з mtime-дельт, дні запасу,
//!   кінцева дата YYYY-MM-DD коректна (std-only, epoch-арифметика).

use skri_core::{release, ProjectStore};

const DAY: i64 = 86_400;

fn doc(status: &str, extra: &str, body: &str) -> String {
    format!("---\ntitle: {extra}\nlang: uk\n{status}---\n\n{body}")
}

fn ch(n: u32, status: &str, extra: &str, words: usize) -> String {
    let filler = "Слово ".repeat(words.max(1)); // word_count ≥ 1 для історії
    doc(status, extra, &filler)
        .replacen("title: ", &format!("title: Розділ {n} — "), 1)
}

fn tmpdir(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("skri-m4-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    base
}

fn make_project(name: &str) -> (std::path::PathBuf, ProjectStore) {
    let root = tmpdir(name);
    let store = ProjectStore::init(&root, "novelist-uk", "uk").expect("init");
    (root, store)
}

fn registry_row(store: &ProjectStore, doc_path: &str) -> (i64, String, Option<i64>, String) {
    store
        .conn()
        .query_row(
            "SELECT order_index, pub_status, pub_date, platforms_json FROM chapters_registry WHERE doc_path = ?1",
            [doc_path],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap()
}

fn column_of(rep: &release::KanbanReport, status: &str) -> usize {
    rep.columns
        .iter()
        .find(|(s, _)| s == status)
        .map(|(_, c)| c.len())
        .unwrap_or(0)
}

#[test]
fn registry_built_from_frontmatter() {
    let (root, store) = make_project("registry");
    std::fs::write(
        root.join("manuscript").join("01-початок.md"),
        "---\ntitle: Розділ 1 — Початок\nlang: uk\npub_status: buffer\npub_date: 2026-11-20\npub_platforms: royalroad, wattpad\n---\n\nТекст першої глави достатньої довжини.",
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript").join("02.md"),
        doc("pub_status: edit\n", "Розділ 2", "Текст другої глави."),
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript").join("03.md"),
        doc("", "Розділ 3", "Текст третьої глави без статусу."),
    )
    .unwrap();

    store.reindex().unwrap();
    let rep = release::rebuild(&store).unwrap();
    assert_eq!(rep.chapters, 3);
    assert_eq!(rep.transitions, 0, "перший скан — переходів немає");

    // статус/порядок/дата/платформи з frontmatter
    let (order1, st1, d1, pl1) = registry_row(&store, "manuscript/01-початок.md");
    assert_eq!(st1, "buffer");
    assert_eq!(order1, 1, "ведучі цифри імені файла");
    assert_eq!(d1, Some(release::parse_ymd("2026-11-20").unwrap()), "pub_date → epoch UTC");
    assert_eq!(pl1, r#"["royalroad","wattpad"]"#);
    let (_, st2, _, _) = registry_row(&store, "manuscript/02.md");
    assert_eq!(st2, "edit");
    // без pub_status у frontmatter → draft
    let (_, st3, _, _) = registry_row(&store, "manuscript/03.md");
    assert_eq!(st3, "draft");

    // kanban-групування
    let kan = release::kanban(&store).unwrap();
    assert_eq!(column_of(&kan, "buffer"), 1);
    assert_eq!(column_of(&kan, "edit"), 1);
    assert_eq!(column_of(&kan, "draft"), 1);
    assert_eq!(column_of(&kan, "scheduled"), 0);
    assert_eq!(column_of(&kan, "published"), 0);
    let buf = &kan.columns.iter().find(|(s, _)| s == "buffer").unwrap().1;
    assert_eq!(buf[0].title, "Розділ 1 — Початок");
    assert_eq!(buf[0].platforms, vec!["royalroad".to_string(), "wattpad".to_string()]);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn rebuild_is_idempotent() {
    let (root, store) = make_project("idem");
    for n in 1..=3 {
        std::fs::write(
            root.join("manuscript").join(format!("{n:02}.md")),
            ch(n, "pub_status: draft\n", "", 50),
        )
        .unwrap();
    }
    store.reindex().unwrap();
    let first = release::rebuild(&store).unwrap();
    let snap: Vec<String> = {
        let mut stmt = store.conn().prepare("SELECT doc_path, pub_status FROM chapters_registry ORDER BY doc_path").unwrap();
        stmt.query_map([], |r| Ok(format!("{}={}", r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };

    // другий і третій прогін: нічого не міняється
    let second = release::rebuild(&store).unwrap();
    let third = release::rebuild(&store).unwrap();
    assert_eq!(second.chapters, first.chapters);
    assert_eq!(second.transitions, 0, "ідемпотентність: нових переходів немає");
    assert_eq!(third.transitions, 0);
    let rows: i64 = store.conn().query_row("SELECT COUNT(*) FROM chapters_registry", [], |r| r.get(0)).unwrap();
    assert_eq!(rows as usize, first.chapters, "дублів немає");
    let logs: i64 = store.conn().query_row("SELECT COUNT(*) FROM chapters_log", [], |r| r.get(0)).unwrap();
    assert_eq!(logs, 0);
    let snap2: Vec<String> = {
        let mut stmt = store.conn().prepare("SELECT doc_path, pub_status FROM chapters_registry ORDER BY doc_path").unwrap();
        stmt.query_map([], |r| Ok(format!("{}={}", r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    assert_eq!(snap2, snap);

    // видалення файлу → рядок зникає з реєстру
    std::fs::remove_file(root.join("manuscript").join("03.md")).unwrap();
    store.reindex().unwrap();
    release::rebuild(&store).unwrap();
    let rows: i64 = store.conn().query_row("SELECT COUNT(*) FROM chapters_registry", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 2);
    assert!(store.doc_id("manuscript/03.md").is_none());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn status_change_recorded_as_transition() {
    let (root, store) = make_project("transition");
    std::fs::write(root.join("manuscript").join("01.md"), ch(1, "pub_status: draft\n", "", 50)).unwrap();
    store.reindex().unwrap();
    release::rebuild(&store).unwrap();

    // статус міняється ПРЯМО у frontmatter файлу
    release::set_status(&store, "manuscript/01.md", "edit", None, &[]).unwrap();
    let raw = std::fs::read_to_string(root.join("manuscript").join("01.md")).unwrap();
    assert!(raw.contains("pub_status: edit"), "статус у файлі — джерело правди");
    assert!(raw.starts_with("---\ntitle:"), "решта frontmatter не пошкоджена");

    store.reindex().unwrap();
    let rep = release::rebuild(&store).unwrap();
    assert_eq!(rep.transitions, 1);
    let kan = release::kanban(&store).unwrap();
    assert_eq!(column_of(&kan, "draft"), 0);
    assert_eq!(column_of(&kan, "edit"), 1);
    assert_eq!(kan.transitions.len(), 1);
    assert_eq!(kan.transitions[0].doc_path, "manuscript/01.md");
    assert_eq!(kan.transitions[0].from.as_deref(), Some("draft"));
    assert_eq!(kan.transitions[0].to, "edit");

    // повторний прогін без правок — переходи не плодяться
    let again = release::rebuild(&store).unwrap();
    assert_eq!(again.transitions, 0);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn forecast_with_fixed_speed() {
    let day = DAY;
    let t0 = 20_000 * day; // «зараз» — опівнічі, щоб арифметика була точною
    let history = [(t0, 1000i64), (t0 + day, 1000), (t0 + 2 * day, 1000)];
    let f = release::forecast_with(&history, 5, 5_000, t0 + 2 * day, 7);
    assert_eq!(f.words_per_day, 1000.0, "фіксована швидкість 1000 слів/день");
    assert_eq!(f.samples, 2);
    assert_eq!(f.buffer_chapters, 5);
    assert_eq!(f.days_left, Some(5.0), "5000 слів / 1000 слів на день = 5 днів");
    assert_eq!(f.days_per_chapter, 1.0);
    assert_eq!(
        f.exhaustion_date.as_deref(),
        Some(release::fmt_date(t0 + 2 * day + 5 * day).as_str()),
        "кінцева дата = зараз + запас"
    );
    assert_eq!(f.exhaustion_date.as_deref().unwrap().len(), 10, "формат YYYY-MM-DD");

    // вікно 1: лише остання глава → та сама швидкість
    let f1 = release::forecast_with(&history, 5, 5_000, t0 + 2 * day, 1);
    assert_eq!(f1.words_per_day, 1000.0);

    // швидкість змінилася: остання глава довша → прогноз зсувається
    let h2 = [(t0, 1000i64), (t0 + day, 1000), (t0 + 2 * day, 2000)];
    let f2 = release::forecast_with(&h2, 2, 2_000, t0 + 2 * day, 2);
    assert!((f2.words_per_day - 1500.0).abs() < 1e-9, "середнє 1000 і 2000");
    assert!((f2.days_left.unwrap() - 4.0 / 3.0).abs() < 1e-9);

    // без історії — прогноз недоступний, але буфер порахований
    let f0 = release::forecast_with(&[], 3, 3_000, t0, 7);
    assert_eq!(f0.words_per_day, 0.0);
    assert_eq!(f0.days_left, None);
    assert_eq!(f0.exhaustion_date, None);
}

#[test]
fn forecast_from_files_uses_mtimes_and_statuses() {
    let (root, store) = make_project("files");
    // 6 глав по 50 слів, mtime рівно через день (1-4 published, 5-6 у буфері):
    // історія письма = усі глави, буфер = дві останні.
    for n in 1..=6u32 {
        let status = if n <= 4 { "pub_status: published\n" } else { "pub_status: buffer\n" };
        let p = root.join("manuscript").join(format!("{n:02}.md"));
        std::fs::write(&p, ch(n, status, "", 50)).unwrap();
        let mtime = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs((20_000u64 + n as u64) * DAY as u64);
        std::fs::OpenOptions::new().append(true).open(&p).unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(mtime)).unwrap();
    }
    store.reindex().unwrap();
    let f = release::forecast(&store, 7).unwrap();
    assert_eq!(f.samples, 5, "6 глав → 5 дельт");
    assert!((f.words_per_day - 50.0).abs() < 1e-6, "50 слів/день з mtime-дельт");
    assert_eq!(f.buffer_chapters, 2);
    assert_eq!(f.buffer_words, 100);
    assert!((f.days_left.unwrap() - 2.0).abs() < 1e-6, "100/50 = 2 дні запасу");
    // кінцева дата = зараз + 2 дні (ceil), рахуємо від незалежного «зараз»
    let before = release::fmt_date(release_now() + 2 * DAY);
    let after = release::fmt_date(release_now() + 2 * DAY);
    assert!(
        f.exhaustion_date.as_deref() == Some(before.as_str()) || f.exhaustion_date.as_deref() == Some(after.as_str()),
        "кінцева дата коректна відносно поточного дня"
    );
    std::fs::remove_dir_all(&root).ok();
}

fn release_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
