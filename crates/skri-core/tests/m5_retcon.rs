//! Acceptance-тести M5: Retcon Manager (специфікація §5.4).
//!
//! - publish фіксує published-снапшот тіла (sha256 тіла, версії ростуть)
//! - diff після правки показує змінені рядки
//! - правка згадки сутності → черга ревізій пізніших глав (Кальдрін гл.1-2)
//! - ідемпотентність черги: повторний перебіл не плодить open-рядків
//! - review позначає reviewed і рядок не зникає при реіндексі

use skri_core::{frontmatter, indexer, inkbridge, retcon, ProjectStore};
use std::path::PathBuf;

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

Кальдрін повів загін уночі. Кальдрін Сумрачний більше не розмовляв з Велесом.
Кальдрін дивився на долину Мора, і Кальдрін не повернувся.
";

fn tmpdir(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("skri-retcon-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    base
}

fn make_project(name: &str, ch1: &str, ch2: &str) -> (PathBuf, ProjectStore) {
    let root = tmpdir(name);
    let store = ProjectStore::init(&root, "novelist-uk", "uk").expect("init");
    std::fs::write(root.join("manuscript").join("01.md"), ch1).unwrap();
    std::fs::write(root.join("manuscript").join("02.md"), ch2).unwrap();
    (root, store)
}

/// Повний прогін як у CLI: reindex + scan перед роботою з чергою.
fn sync(store: &ProjectStore) {
    store.reindex().unwrap();
    indexer::scan_project(store).unwrap();
}

#[test]
fn publish_records_snapshot_with_sha_and_versions() {
    let (_root, store) = make_project("publish", CH1, CH2);
    sync(&store);

    let rep = retcon::publish(&store, "manuscript/01.md").unwrap();
    assert_eq!(rep.version, 1);
    // sha256 береться з inkbridge (той самий std-only алгоритм)
    let raw = std::fs::read_to_string(_root.join("manuscript").join("01.md")).unwrap();
    let expect_sha = inkbridge::sha256_hex(frontmatter::parse(&raw).body.as_bytes());
    assert_eq!(rep.body_sha256, expect_sha);

    let (ver, sha, body) = retcon::latest_snapshot(&store, "manuscript/01.md")
        .unwrap()
        .expect("снапшот існує");
    assert_eq!(ver, 1);
    assert_eq!(sha, expect_sha);
    assert_eq!(body, frontmatter::parse(&raw).body, "тіло зафіксоване як є");

    // друга публікація після правки = нова версія, тіло в БД не втрачається
    let edited = CH1.replace("заплющив очі", "затулив очі");
    std::fs::write(_root.join("manuscript").join("01.md"), edited).unwrap();
    sync(&store);
    let rep2 = retcon::publish(&store, "manuscript/01.md").unwrap();
    assert_eq!(rep2.version, 2);
    let (_, _, body1) = retcon::latest_snapshot(&store, "manuscript/01.md").unwrap().unwrap();
    assert!(body1.contains("затулив очі"), "останній снапшот = нова версія");
    // перша версія досі в БД
    let old: String = store
        .conn()
        .query_row(
            "SELECT body FROM published_snapshots WHERE doc_path='manuscript/01.md' AND version=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(old.contains("заплющив очі"));
}

#[test]
fn diff_after_edit_shows_changed_lines() {
    let (root, store) = make_project("diff", CH1, CH2);
    sync(&store);
    retcon::publish(&store, "manuscript/01.md").unwrap();

    // без правок — diff порожній
    sync(&store);
    let d0 = retcon::diff(&store, "manuscript/01.md").unwrap();
    assert!(d0.changes.is_empty(), "без правок змін немає");

    // правка одного рядка
    let edited = CH1.replace("Старий Велес чекав біля воріт.", "Старий Велес чекав за рогом.");
    std::fs::write(root.join("manuscript").join("01.md"), edited).unwrap();
    sync(&store);
    let d1 = retcon::diff(&store, "manuscript/01.md").unwrap();
    assert_eq!(d1.version, 1, "diff іде проти останнього снапшоту");
    assert!(!d1.changes.is_empty());
    assert!(d1
        .changes
        .iter()
        .any(|c| c.kind == "added" && c.text.contains("за рогом")),
        "added-рядок містить новий текст: {:?}", d1.changes);
    assert!(d1
        .changes
        .iter()
        .any(|c| c.kind == "removed" && c.text.contains("біля воріт")),
        "removed-рядок містить старий текст: {:?}", d1.changes);

    // неіснуючий снапшот → чесна помилка
    assert!(retcon::diff(&store, "manuscript/02.md").is_err());
}

#[test]
fn entity_edit_enqueues_later_chapters() {
    let (root, store) = make_project("queue", CH1, CH2);
    sync(&store);
    retcon::publish(&store, "manuscript/01.md").unwrap();
    retcon::publish(&store, "manuscript/02.md").unwrap();

    // правка в гл. 1 торкається «Кальдріна» → згадки в гл. 2 у черзі
    let edited = CH1.replace("і Кальдрін заплющив очі", "і Кальдрін затулив очі");
    std::fs::write(root.join("manuscript").join("01.md"), edited).unwrap();
    let rep = retcon::rebuild(&store).unwrap();
    assert!(rep.open >= 1, "черга не порожня після правки сутності");

    let rows = retcon::queue_rows(&store).unwrap();
    assert!(!rows.is_empty());
    for r in &rows {
        assert_eq!(r.status, "open");
        assert_eq!(r.source_doc, "manuscript/01.md");
        assert_eq!(r.affected_doc, "manuscript/02.md", "пізніша глава в черзі");
        assert!(r.affected_pos >= 0);
        assert!(!r.sentence.is_empty(), "речення згадки збережене");
        assert!(r.entity_name.starts_with("Кальдрін"), "сутність: {}", r.entity_name);
    }
    // усі одиночні згадки «Кальдріна» в гл. 2 (3 речення) потрапили в чергу
    let kaldrin_rows: Vec<_> = rows.iter().filter(|r| r.entity_name == "Кальдрін").collect();
    assert_eq!(kaldrin_rows.len(), 3, "усі згадки сутності в гл. 2: {kaldrin_rows:?}");
}

#[test]
fn queue_rebuild_is_idempotent() {
    let (root, store) = make_project("idem", CH1, CH2);
    sync(&store);
    retcon::publish(&store, "manuscript/01.md").unwrap();
    retcon::publish(&store, "manuscript/02.md").unwrap();

    let edited = CH1.replace("і Кальдрін заплющив очі", "і Кальдрін затулив очі");
    std::fs::write(root.join("manuscript").join("01.md"), edited).unwrap();

    let r1 = retcon::rebuild(&store).unwrap();
    let rows1 = retcon::queue_rows(&store).unwrap();
    let r2 = retcon::rebuild(&store).unwrap();
    let rows2 = retcon::queue_rows(&store).unwrap();

    assert!(r1.open >= 1);
    assert_eq!(r1.open, r2.open, "повторний перебіл не плодить open-рядків");
    assert_eq!(rows1.len(), rows2.len());
    // ключі (сутність, джерело, документ, позиція) не дублюються
    let mut keys: Vec<(String, String, String, i64)> = rows2
        .iter()
        .map(|r| {
            (
                r.entity_name.clone(),
                r.source_doc.clone(),
                r.affected_doc.clone(),
                r.affected_pos,
            )
        })
        .collect();
    keys.sort();
    let before = keys.len();
    keys.dedup();
    assert_eq!(before, keys.len(), "дублікати ключів: {keys:?}");
}

#[test]
fn review_marks_and_survives_reindex() {
    let (root, store) = make_project("review", CH1, CH2);
    sync(&store);
    retcon::publish(&store, "manuscript/01.md").unwrap();
    retcon::publish(&store, "manuscript/02.md").unwrap();

    let edited = CH1.replace("і Кальдрін заплющив очі", "і Кальдрін затулив очі");
    std::fs::write(root.join("manuscript").join("01.md"), edited).unwrap();
    let rows = retcon::queue(&store).unwrap();
    assert!(rows.iter().any(|r| r.status == "open"));

    let id = rows[0].id;
    let (entity, source, affected, pos) = (
        rows[0].entity_name.clone(),
        rows[0].source_doc.clone(),
        rows[0].affected_doc.clone(),
        rows[0].affected_pos,
    );
    let reviewed = retcon::review(&store, id).unwrap();
    assert_eq!(reviewed.status, "reviewed");

    // реіндекс: reviewed-рядок НЕ зникає і НЕ перетворюється назад на open
    let rows2 = retcon::queue(&store).unwrap();
    let same = rows2
        .iter()
        .find(|r| {
            r.entity_name == entity
                && r.source_doc == source
                && r.affected_doc == affected
                && r.affected_pos == pos
        })
        .expect("reviewed-рядок пережив реіндекс");
    assert_eq!(same.id, id);
    assert_eq!(same.status, "reviewed");
    // і не задублився новим open-рядком на тій самій позиції
    let open_same: Vec<_> = rows2
        .iter()
        .filter(|r| {
            r.status == "open"
                && r.entity_name == entity
                && r.source_doc == source
                && r.affected_doc == affected
                && r.affected_pos == pos
        })
        .collect();
    assert!(open_same.is_empty(), "reviewed не перетворюється назад на open");
    // неіснуючий id → помилка
    assert!(retcon::review(&store, 999_999).is_err());
}

#[test]
fn edit_without_entities_leaves_queue_empty() {
    let (root, store) = make_project("noent", CH1, CH2);
    sync(&store);
    retcon::publish(&store, "manuscript/01.md").unwrap();
    retcon::publish(&store, "manuscript/02.md").unwrap();

    // правка ДОДАЄ рядок без жодної назви (жодна сутність не торкнута)
    let mut edited = CH1.to_string();
    edited.push_str("Небо було сірим і тихим.\n");
    std::fs::write(root.join("manuscript").join("01.md"), edited).unwrap();

    let rep = retcon::rebuild(&store).unwrap();
    assert_eq!(rep.open, 0, "правка без сутностей не створює ревізій");
    assert_eq!(rep.reviewed, 0);

    // але diff це бачить
    let d = retcon::diff(&store, "manuscript/01.md").unwrap();
    assert!(d
        .changes
        .iter()
        .any(|c| c.kind == "added" && c.text.contains("сірим і тихим")));
}
