//! Acceptance-тести M7: EPUB-експорт з metadata scrubber (pen name management).
//!
//! Критерії приймання:
//! - EPUB структура: mimetype ПЕРШИМ записом, STORED (method 0), без extra-поля;
//!   META-INF/container.xml; content.opf парситься (package/title/language/uuid);
//! - XHTML екранує `&<>` (жодного сирого `<` з тексту);
//! - scrub full: dc:creator = pen_name і реальне ім'я з frontmatter ВІДСУТНЄ
//!   в усьому файлі EPUB (байти), generator відсутній; у light — генератор є;
//! - scrub-звіт перелічує заміни («dc:creator: ... → ...», «generator: _rm»);
//! - sha256 детермінований (дві збірки → той самий хеш) і записаний в epub_exports.

use skri_core::{scrub, ProjectStore};

/// Розібрати ZIP (STORED): повернути (назви записів у порядку, map назва→байти).
fn zip_entries(data: &[u8]) -> (Vec<String>, std::collections::BTreeMap<String, Vec<u8>>) {
    let mut names = Vec::new();
    let mut map = std::collections::BTreeMap::new();
    let mut p = 0usize;
    while p + 4 <= data.len() && data[p..p + 4] == [0x50, 0x4b, 0x03, 0x04] {
        let method = u16::from_le_bytes([data[p + 8], data[p + 9]]);
        let crc = u32::from_le_bytes([data[p + 14], data[p + 15], data[p + 16], data[p + 17]]);
        let size = u32::from_le_bytes([data[p + 22], data[p + 23], data[p + 24], data[p + 25]]);
        let name_len = u16::from_le_bytes([data[p + 26], data[p + 27]]) as usize;
        let extra_len = u16::from_le_bytes([data[p + 28], data[p + 29]]) as usize;
        let name = String::from_utf8_lossy(&data[p + 30..p + 30 + name_len]).to_string();
        let start = p + 30 + name_len + extra_len;
        let content = data[start..start + size as usize].to_vec();
        assert_eq!(method, 0, "запис «{name}» мусить бути STORED");
        assert_eq!(crc, scrub::crc32(&content), "CRC «{name}»");
        names.push(name.clone());
        map.insert(name, content);
        p = start + size as usize;
    }
    (names, map)
}

fn tmpdir(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("skri-m7-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    base
}

/// Проєкт з 2 глав: frontmatter містить РЕАЛЬНОГО автора + pen_name.
fn make_project(name: &str) -> (std::path::PathBuf, ProjectStore) {
    let root = tmpdir(name);
    let store = ProjectStore::init(&root, "novelist-uk", "uk").expect("init");
    std::fs::write(
        root.join("manuscript").join("01-початок.md"),
        "---\ntitle: Розділ 1 — Початок\nlang: uk\nauthor: Вялков В.\npen_name: Ash Thornwood\npublisher: Самвидав+\nemail: vyal@privat.email\ndescription: Роман автора Вялков В. про іншопланетні скарби.\n---\n\n# Початок\n\nКальдрін <вичитав> знак & зготував **зілля** і *курсив*.\n\n---\n\n## Тінь\n\nПродовження другої сцени.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript").join("02-розвиток.md"),
        "---\ntitle: Розділ 2 — Розвиток\nlang: uk\nauthor: Вялков В.\n---\n\nДруга глава. Автор Вялков В. підписав передмову.\n",
    )
    .unwrap();
    store.reindex().unwrap();
    (root, store)
}

fn build_default(root: &std::path::PathBuf, out: &str) -> scrub::ScrubReport {
    let store = ProjectStore::open(root).unwrap();
    scrub::build(
        &store,
        std::path::Path::new(out),
        &scrub::ScrubOptions::default(),
    )
    .unwrap()
}

#[test]
fn epub_structure_mimetype_first_stored() {
    let (root, _store) = make_project("structure");
    let out = root.join("book.epub");
    build_default(&root, out.to_str().unwrap());

    let bytes = std::fs::read(&out).unwrap();
    // mimetype — ПЕРШИМ записом
    assert_eq!(&bytes[0..4], &[0x50, 0x4b, 0x03, 0x04], "local header sig");
    let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
    let extra_len = u16::from_le_bytes([bytes[28], bytes[29]]) as usize;
    assert_eq!(&bytes[30..30 + name_len], b"mimetype");
    assert_eq!(extra_len, 0, "mimetype: без extra-поля");
    assert_eq!(
        &bytes[8..10],
        &[0, 0],
        "compression method 0 (STORED) у mimetype"
    );
    let size = u32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]) as usize;
    assert_eq!(&bytes[30 + name_len..30 + name_len + size], b"application/epub+zip");

    let (names, map) = zip_entries(&bytes);
    assert_eq!(names[0], "mimetype");
    for req in [
        "META-INF/container.xml",
        "OEBPS/content.opf",
        "OEBPS/nav.xhtml",
        "ch1.xhtml",
        "ch2.xhtml",
    ] {
        assert!(names.contains(&req.to_string()), "немає запису {req}");
    }
    // container вказує на opf
    let container = String::from_utf8(map["META-INF/container.xml"].clone()).unwrap();
    assert!(container.contains("full-path=\"OEBPS/content.opf\""));

    // opf парситься (мінімальна перевірка структури)
    let opf = String::from_utf8(map["OEBPS/content.opf"].clone()).unwrap();
    assert!(opf.starts_with("<?xml"));
    assert!(opf.contains("<package") && opf.contains("version=\"3.0\""));
    assert!(opf.contains("<dc:title>Розділ 1 — Початок</dc:title>"));
    assert!(opf.contains("<dc:language>uk</dc:language>"));
    assert!(opf.contains("urn:uuid:"));
    assert!(opf.contains("<dc:date>"));
    // spine/manifest: nav + 2 глави
    assert!(opf.contains("properties=\"nav\""));
    assert!(opf.contains("idref=\"ch1\"") && opf.contains("idref=\"ch2\""));
    // nav — EPUB3 toc
    let nav = String::from_utf8(map["OEBPS/nav.xhtml"].clone()).unwrap();
    assert!(nav.contains("epub:type=\"toc\""));
    assert!(nav.contains("<a href=\"ch1.xhtml\">"));
}

#[test]
fn xhtml_escapes_html_specials() {
    let (root, _store) = make_project("escape");
    let out = root.join("book.epub");
    build_default(&root, out.to_str().unwrap());

    let (_, map) = zip_entries(&std::fs::read(&out).unwrap());
    let ch1 = String::from_utf8(map["ch1.xhtml"].clone()).unwrap();
    // екранування &<> обов'язкове
    assert!(ch1.contains("&lt;вичитав&gt;"));
    assert!(ch1.contains("знак &amp;"));
    assert!(!ch1.contains("<вичитав>"), "сире <> з тексту не повинно бути тегом");
    // markdown-розмітка
    assert!(ch1.contains("<h1>Початок</h1>"));
    assert!(ch1.contains("<h2>Тінь</h2>"));
    assert!(ch1.contains("<hr/>"));
    assert!(ch1.contains("<strong>зілля</strong>"));
    assert!(ch1.contains("<em>курсив</em>"));
    assert!(ch1.contains("<p>Продовження другої сцени.</p>"));
    // frontmatter НЕ потрапляє в XHTML
    assert!(!ch1.contains("pen_name:"));
    assert!(!ch1.contains("privat.email"), "email автора не витікає");
}

#[test]
fn scrub_full_creator_is_pen_name_real_name_gone() {
    let (root, _store) = make_project("full-creator");
    let out = root.join("book.epub");
    build_default(&root, out.to_str().unwrap());

    let bytes = std::fs::read(&out).unwrap();
    let (_, map) = zip_entries(&bytes);
    let opf = String::from_utf8(map["OEBPS/content.opf"].clone()).unwrap();
    // dc:creator = тільки pen name
    assert!(opf.contains("<dc:creator>Ash Thornwood</dc:creator>"));
    assert!(!opf.contains("<dc:publisher>"), "publisher прибраний у full");
    // publisher з frontmatter теж не витікає
    assert!(!opf.contains("Самвидав"));
    // реальне ім'я НЕ є ніде в цілому EPUB (опис і тіло теж прачені)
    let real = "Вялков В.".as_bytes();
    assert!(
        !bytes.windows(real.len()).any(|w| w == real),
        "реальне ім'я автора не мусить бути в жодному байті full-EPUB"
    );
    // email теж геть
    let mail = b"privat.email";
    assert!(!bytes.windows(mail.len()).any(|w| w == mail));
    // dc:date — дата користувача, без інших дат-витоків (mtime тощо)
    // (uuid детермінований — див. окремий тест)
}

#[test]
fn generator_removed_in_full_present_in_light() {
    let (root, _store) = make_project("generator");
    let full_out = root.join("full.epub");
    build_default(&root, full_out.to_str().unwrap());
    let store = ProjectStore::open(&root).unwrap();
    let light_out = root.join("light.epub");
    scrub::build(
        &store,
        &light_out,
        &scrub::ScrubOptions {
            mode: scrub::ScrubMode::Light,
            ..Default::default()
        },
    )
    .unwrap();

    let full = String::from_utf8(zip_entries(&std::fs::read(&full_out).unwrap()).1["ch1.xhtml"].clone()).unwrap();
    assert!(!full.contains("name=\"generator\""), "full: generator прибраний");

    let light = String::from_utf8(zip_entries(&std::fs::read(&light_out).unwrap()).1["ch1.xhtml"].clone()).unwrap();
    assert!(light.contains("name=\"generator\""), "light: generator залишено");
    // light лишає звичайні метадані: реальний автор у opf
    let lopf = String::from_utf8(zip_entries(&std::fs::read(&light_out).unwrap()).1["OEBPS/content.opf"].clone()).unwrap();
    assert!(lopf.contains("<dc:creator>Вялков В.</dc:creator>"));
}

#[test]
fn scrub_report_lists_replacements() {
    let (root, store) = make_project("report");
    let out = root.join("book.epub");
    let rep = build_default(&root, out.to_str().unwrap());

    assert_eq!(rep.mode, "full");
    assert_eq!(rep.pen_name, "Ash Thornwood");
    assert!(rep
        .changes
        .iter()
        .any(|c| c.contains("dc:creator: Вялков В. → Ash Thornwood")), "звіт: заміна creator");
    assert!(rep
        .changes
        .iter()
        .any(|c| c.starts_with("generator: _rm")), "звіт: generator _rm");
    assert!(rep
        .changes
        .iter()
        .any(|c| c.contains("dc:publisher: Самвидав") && c.ends_with("→ _rm")));
    assert!(rep
        .changes
        .iter()
        .any(|c| c.contains("dc:description: Вялков В. → Ash Thornwood")));
    assert!(rep
        .changes
        .iter()
        .any(|c| c.contains("тіло глав") && c.contains("→ «Ash Thornwood»")));
    assert!(rep.chapters.contains(&"manuscript/01-початок.md".to_string()));

    // останній звіт = те саме (зберігається в .skri/scrub-report.json)
    let last = scrub::last_report(&store).unwrap().expect("звіт збережено");
    assert_eq!(last.sha256, rep.sha256);
    assert_eq!(last.changes, rep.changes);
}

#[test]
fn sha256_deterministic_and_logged() {
    let (root, _store) = make_project("deterministic");
    let out1 = root.join("a.epub");
    let out2 = root.join("b.epub");
    let r1 = build_default(&root, out1.to_str().unwrap());
    let r2 = build_default(&root, out2.to_str().unwrap());

    assert_eq!(r1.sha256, r2.sha256, "дві збірки того самого рукопису → той самий sha256");
    assert_eq!(std::fs::read(&out1).unwrap(), std::fs::read(&out2).unwrap(), "EPUB байт-в-байт однаковий");
    assert_eq!(r1.sha256.len(), 64);

    // журнал epub_exports: pen_name, scrub_mode, sha256
    let store = ProjectStore::open(&root).unwrap();
    let (pen, mode, sha): (String, String, String) = store
        .conn()
        .query_row(
            "SELECT pen_name, scrub_mode, sha256 FROM epub_exports ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(pen, "Ash Thornwood");
    assert_eq!(mode, "full");
    assert_eq!(sha, r1.sha256);
}

#[test]
fn light_report_and_no_full_changes() {
    // контраст: light не рапортує full-заміни
    let (root, store) = make_project("light-report");
    let out = root.join("l.epub");
    let rep = scrub::build(
        &store,
        &out,
        &scrub::ScrubOptions {
            mode: scrub::ScrubMode::Light,
            pen_name: Some("Ash Thornwood".into()),
            date: Some("2026-11-05".into()),
        },
    )
    .unwrap();
    assert_eq!(rep.mode, "light");
    assert!(!rep.changes.iter().any(|c| c.contains("_rm")));
    assert!(rep.changes.iter().any(|c| c.contains("light")));
    // дата користувача потрапила в opf
    let bytes = std::fs::read(&out).unwrap();
    let s = String::from_utf8_lossy(&bytes).to_string();
    assert!(s.contains("<dc:date>2026-11-05</dc:date>"));
    // невалідна дата — помилка, не мовчазний витік
    assert!(scrub::build(
        &store,
        &out,
        &scrub::ScrubOptions {
            mode: scrub::ScrubMode::Light,
            date: Some("не-дата".into()),
            ..Default::default()
        },
    )
    .is_err());
}
