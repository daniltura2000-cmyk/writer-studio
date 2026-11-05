//! Acceptance-тести M4: Crosspost Orchestrator + роялті-дашборд (§5.6).
//!
//! Пакування 5 платформ у .skri/outbound/, html-трансформація, журнал
//! pubлікацій + mark, stale-алерт після правки канону, імпорт роялті
//! CSV ($1,234.56) і агрегати по платформах/місяцях.

use skri_core::crosspost;
use skri_core::royalty::{self, By};
use skri_core::tests_common::tmp_project;
use std::path::Path;

const CHAPTER: &str = "---\ntitle: Розділ 1\nlang: uk\ntags: фентезі, літргп\nnotes: Дякую за читання!\n---\n\nВелес ступив у **Сумрячний ліс** і зітхнув. *Тиша* була підозрілою.\n\nДругий абзац про дорогу.\n\n---\n\nКінець глави.\n";

fn write_chapter(root: &Path, body: &str) {
    let dir = root.join("manuscript");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("01-розділ.md"), body).unwrap();
}

#[test]
fn pack_creates_all_five_platforms() {
    let (root, store) = tmp_project("xpack");
    write_chapter(&root, CHAPTER);

    let rep = crosspost::pack(&store, "manuscript/01-розділ.md").unwrap();
    assert_eq!(rep.platforms, vec!["royalroad", "scribblehub", "wattpad", "ao3", "patreon"]);
    assert_eq!(rep.chapter, "01-розділ");
    assert!(!rep.version_hash.is_empty());

    for platform in crosspost::PLATFORMS {
        let pkg_path = rep
            .dir
            .join(platform)
            .join("package.json");
        assert!(pkg_path.exists(), "пакунок {platform} на місці");
        let pkg: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&pkg_path).unwrap()).unwrap();
        assert_eq!(pkg["platform"], *platform);
        assert_eq!(pkg["title"], "Розділ 1", "title з frontmatter");
        assert!(
            pkg["html_body"].as_str().unwrap().contains("<strong>Сумрячний ліс</strong>"),
            "html-тіло в пакунку {platform}"
        );
        assert_eq!(pkg["notes"], "Дякую за читання!");
        assert_eq!(pkg["tags_json"], r#"["фентезі","літргп"]"#);
        assert!(rep.dir.join(platform).join("chapter.html").exists(), "готовий html {platform}");
    }

    // журнал: 5 pending-рядків з хешем версії
    let rows = crosspost::log_rows(&store, None).unwrap();
    assert_eq!(rows.len(), 5);
    assert!(rows.iter().all(|r| r.status == "pending" && r.version_hash == rep.version_hash));

    // повторний pack — ідемпотентний (posted не чіпається перевіряється нижче)
    let rep2 = crosspost::pack(&store, "manuscript/01-розділ.md").unwrap();
    assert!(rep2.skipped_posted.is_empty());
    assert_eq!(crosspost::log_rows(&store, None).unwrap().len(), 5, "рядки не плодяться");
}

#[test]
fn html_transformation_bold_italic_paragraphs_hr() {
    let html = crosspost::markdown_to_html(
        "Вступ із **жирним** та *курсивом*.\n\nДругий абзац.\n\n---\n\nХвіст із <тегом>.",
    );
    assert!(html.contains("<p>Вступ із <strong>жирним</strong> та <em>курсивом</em>.</p>"), "{html}");
    assert!(html.contains("<p>Другий абзац.</p>"), "{html}");
    assert!(html.contains("<hr>"), "розділювач --- → hr: {html}");
    assert!(html.contains("Хвіст із &lt;тегом&gt;."), "escape html: {html}");
    // два параграфи + хвостовий параграф
    assert_eq!(html.matches("<p>").count(), 3, "{html}");
}

#[test]
fn journal_and_mark_posted() {
    let (root, store) = tmp_project("xmark");
    write_chapter(&root, CHAPTER);
    crosspost::pack(&store, "manuscript/01-розділ.md").unwrap();

    let row = crosspost::mark(&store, "manuscript/01-розділ.md", "royalroad", "posted").unwrap();
    assert_eq!(row.status, "posted");
    assert!(row.published_at.is_some(), "published_at фіксується");

    // невідома платформа/статус — помилка
    assert!(crosspost::mark(&store, "manuscript/01-розділ.md", "goodreads", "posted").is_err());
    assert!(crosspost::mark(&store, "manuscript/01-розділ.md", "ao3", "archived").is_err());

    // інші платформи лишаються pending
    let rows = crosspost::log_rows(&store, Some("manuscript/01-розділ.md")).unwrap();
    let rr = rows.iter().find(|r| r.platform == "royalroad").unwrap();
    assert_eq!(rr.status, "posted");
    assert_eq!(rows.iter().filter(|r| r.status == "pending").count(), 4);

    // повторний pack не скидає posted-рядок (версію вже опубліковано)
    let rep = crosspost::pack(&store, "manuscript/01-розділ.md").unwrap();
    assert_eq!(rep.skipped_posted, vec!["royalroad"]);
    let rr = crosspost::log_rows(&store, Some("manuscript/01-розділ.md"))
        .unwrap()
        .into_iter()
        .find(|r| r.platform == "royalroad")
        .unwrap();
    assert_eq!(rr.status, "posted", "posted не перезаписується pack'ом");
    assert!(rr.published_at.is_some());
}

#[test]
fn stale_alert_after_canon_edit() {
    let (root, store) = tmp_project("xstale");
    write_chapter(&root, CHAPTER);
    crosspost::pack(&store, "manuscript/01-розділ.md").unwrap();
    crosspost::mark(&store, "manuscript/01-розділ.md", "wattpad", "posted").unwrap();

    // до правки — чисто
    assert!(crosspost::check_stale(&store).unwrap().stale.is_empty());

    // правка канону після публікації
    write_chapter(
        &root,
        &CHAPTER.replace("Другий абзац про дорогу.", "Другий абзац, переписаний наново."),
    );

    let rep = crosspost::check_stale(&store).unwrap();
    assert_eq!(rep.stale.len(), 1, "лише posted-платформа: {:?}", rep.stale);
    assert_eq!(rep.stale[0].1, "wattpad");

    // алерт у таблиці alerts, і повторний прогін його не плодить
    let conn = store.conn();
    let n: i64 = conn
        .query_row("SELECT count(*) FROM alerts WHERE kind='crosspost-stale'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    let rep2 = crosspost::check_stale(&store).unwrap();
    assert_eq!(rep2.stale.len(), 1);
    let n2: i64 = conn
        .query_row("SELECT count(*) FROM alerts WHERE kind='crosspost-stale'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n2, 1, "ідемпотентно: алерт не дублюється");

    // mark posted заново (перевідправка) — алерт розв'язується
    crosspost::mark(&store, "manuscript/01-розділ.md", "wattpad", "posted").unwrap();
    let open: i64 = conn
        .query_row(
            "SELECT count(*) FROM alerts WHERE kind='crosspost-stale' AND resolved=0",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(open, 0, "перевідправка розв'язує stale-алерт");
    assert!(crosspost::check_stale(&store).unwrap().stale.is_empty());
}

// --- роялті ---

/// CSV у стилі KDP/D2D: $, тисячні коми, різні назви колонок,
/// невідома колонка (ігурується), рядок без суми (пропускається).
const ROYALTY_CSV: &str = "Order Date,Marketplace,Title,Units Sold,Royalty (USD),Royalty Type\n\
2026-01-05,Amazon.com,Тінь Велеса,\"1,205\",\"$1,234.56\",Net\n\
2026-01-19,Amazon.de,Тінь Велеса,40,$88.40,Net\n\
2026-02-02,Amazon.com,Тінь Велеса,55,$121.00,Net\n\
2026-02-10,ScribbleHub,Тінь Велеса,10,n/a,Tip\n\
2026-02-14,Apple Books,Тінь Велеса,12,$18.24,Net\n";

#[test]
fn royalty_csv_import_and_aggregates() {
    let (root, store) = tmp_project("xroyalty");
    let csv = root.join("kdp-jan.csv");
    std::fs::write(&csv, ROYALTY_CSV).unwrap();

    let rep = royalty::import_csv(&store, &csv).unwrap();
    assert_eq!(rep.source_file, "kdp-jan.csv");
    assert_eq!(rep.rows, 4, "рядок n/a пропущено: {:?}", rep.mapping);
    assert_eq!(rep.skipped, 1);
    assert!(rep.mapping.iter().any(|(h, f)| h == "Royalty (USD)" && f == "revenue"));
    assert!(rep.mapping.iter().any(|(h, f)| h == "Marketplace" && f == "platform"));
    assert!(rep.mapping.iter().any(|(h, f)| h == "Order Date" && f == "date"));

    let rows = royalty::rows(&store).unwrap();
    assert_eq!(rows.len(), 4);
    let big = rows.iter().find(|r| r.revenue_cents == 123_456).unwrap();
    assert_eq!(big.units, 1_205, "тисячні коми в одиницях");
    assert_eq!(big.platform, "Amazon.com");
    assert_eq!(big.book, "Тінь Велеса");
    assert_eq!(big.date, "2026-01-05");

    // агрегат по платформах
    let (aggs, total_units, total_cents) = royalty::summary(&store, By::Platform).unwrap();
    assert_eq!(total_units, 1_205 + 40 + 55 + 12);
    assert_eq!(total_cents, 123_456 + 8_840 + 12_100 + 1_824);
    let amz = aggs.iter().find(|a| a.key == "Amazon.com").unwrap();
    assert_eq!(amz.revenue_cents, 123_456 + 12_100);
    assert_eq!(amz.units, 1_205 + 55);
    assert!(aggs.iter().any(|a| a.key == "Apple Books" && a.revenue_cents == 1_824));

    // агрегат по місяцях
    let (months, _, mc) = royalty::summary(&store, By::Month).unwrap();
    assert_eq!(mc, total_cents);
    let jan = months.iter().find(|a| a.key == "2026-01").unwrap();
    assert_eq!(jan.revenue_cents, 123_456 + 8_840);
    let feb = months.iter().find(|a| a.key == "2026-02").unwrap();
    assert_eq!(feb.revenue_cents, 12_100 + 1_824);

    // по книгах
    let (books, _, _) = royalty::summary(&store, By::Book).unwrap();
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].key, "Тінь Велеса");
}

#[test]
fn royalty_reimport_is_idempotent() {
    let (root, store) = tmp_project("xroyalty2");
    let csv = root.join("patreon.csv");
    std::fs::write(&csv, "Month,Platform,Book,Units,Earnings\n2026-03-01,Patreon,Тінь Велеса,30,$45.00\n").unwrap();
    royalty::import_csv(&store, &csv).unwrap();
    royalty::import_csv(&store, &csv).unwrap(); // повторний імпорт того ж файла
    let rows = royalty::rows(&store).unwrap();
    assert_eq!(rows.len(), 1, "перезапис, не дублювання");
    assert_eq!(rows[0].revenue_cents, 4_500);
    assert_eq!(rows[0].platform, "Patreon");
    assert_eq!(rows[0].date, "2026-03-01");
}
