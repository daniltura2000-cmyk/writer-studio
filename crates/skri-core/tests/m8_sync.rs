//! Acceptance-тести M8: легкий CRDT-синк (векторні годинники, merge, конфлікти).
//!
//! - device_id у .skri/device.json стабільний; годинник інкрементиться при save_doc
//! - fast-forward: вектор, що домінує, перемагає
//! - непорівнювані вектори + конфліктний рядок → маркери у файлі + алерт + sync_conflicts
//! - непорівнювані без конфлікту (правили різні рядки) → автозлиття обох
//! - export → import раунд-трип на другому device_id
//! - журнал синхронізації пишеться (merged|fast-forward|conflict)

use skri_core::{frontmatter, sync, ProjectStore};
use std::path::PathBuf;

const V0: &str = "Рядок перший.\nРядок другий.\nРядок третій.\n";

fn tmpdir(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("skri-sync-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    base
}

/// Проєкт із одним доком manuscript/01.md (тіло V0, без save_doc).
fn make_project(name: &str, body: &str) -> (PathBuf, ProjectStore) {
    let root = tmpdir(name);
    let store = ProjectStore::init(&root, "novelist-uk", "uk").expect("init");
    std::fs::write(
        root.join("manuscript").join("01.md"),
        format!("---\ntitle: Розділ 1\nlang: uk\n---\n\n{body}"),
    )
    .unwrap();
    (root, store)
}

fn body_of(root: &PathBuf, rel: &str) -> String {
    let raw = std::fs::read_to_string(root.join(rel)).unwrap();
    let b = frontmatter::parse(&raw).body;
    // ведучий "\n" закриваючого "---" — частина формату, порівнюємо прозу
    b.strip_prefix('\n').unwrap_or(&b).to_string()
}

fn clock_of(root: &PathBuf, rel: &str) -> sync::VClock {
    let raw = std::fs::read_to_string(root.join(rel)).unwrap();
    let p = frontmatter::parse(&raw);
    sync::parse_clock(p.meta.get("vclock").map(|s| s.as_str()).unwrap_or(""))
}

/// Тестовий пакет: один док з вектором {device: counter}.
fn packet_with(device: &str, path: &str, counter: u64, body: &str) -> serde_json::Value {
    let mut clock = serde_json::Map::new();
    clock.insert(device.to_string(), serde_json::json!(counter));
    serde_json::json!({
        "format": sync::PACKET_FORMAT,
        "version": 1,
        "device": device,
        "created_at": 1,
        "docs": [{
            "path": path,
            "clock": serde_json::Value::Object(clock),
            "body": body,
        }]
    })
}

#[test]
fn clock_increments_on_save_and_device_persists() {
    let (root, store) = make_project("clock", V0);
    let store = &store;

    // device_id стабільний і зберігається у .skri/device.json
    let dev1 = sync::device_id(&root).unwrap();
    let dev2 = sync::device_id(&root).unwrap();
    assert_eq!(dev1, dev2, "device_id не має перегенеровуватися");
    assert!(root.join(".skri").join("device.json").exists());
    let raw = std::fs::read_to_string(root.join(".skri").join("device.json")).unwrap();
    assert!(raw.contains(&dev1), "device_id у device.json: {raw}");

    // перше збереження: лічильник 1, frontmatter збережено
    let edited1 = V0.replace("Рядок другий.", "Рядок другий (правка 1).");
    let c1 = sync::save_doc(store, "manuscript/01.md", &edited1).unwrap();
    assert_eq!(c1.get(&dev1), Some(&1), "перший save: лічильник 1, {c1:?}");
    assert_eq!(body_of(&root, "manuscript/01.md"), edited1);
    let clock_str = clock_of(&root, "manuscript/01.md");
    assert_eq!(clock_str.get(&dev1), Some(&1), "годинник у frontmatter");

    // фронтматтер (title/lang) пережив збереження
    let raw = std::fs::read_to_string(root.join("manuscript").join("01.md")).unwrap();
    let p = frontmatter::parse(&raw);
    assert_eq!(p.meta.get("title").map(String::as_str), Some("Розділ 1"));
    assert_eq!(p.meta.get("lang").map(String::as_str), Some("uk"));

    // друге збереження: лічильник 2
    let edited2 = edited1.replace("(правка 1).", "(правка 2).");
    let c2 = sync::save_doc(store, "manuscript/01.md", &edited2).unwrap();
    assert_eq!(c2.get(&dev1), Some(&2), "другий save: лічильник 2, {c2:?}");

    // новий док через save_doc: годинник з нуля
    let c3 = sync::save_doc(store, "manuscript/02.md", "Свіжий розділ.\n").unwrap();
    assert_eq!(c3.get(&dev1), Some(&1));
    assert!(root.join("manuscript").join("02.md").exists());
}

#[test]
fn fast_forward_takes_dominating_version() {
    let (root, store) = make_project("ff", V0);
    let store = &store;
    let dev = sync::device_id(&root).unwrap();

    // локальна версія: годинник {dev: 1}
    sync::save_doc(store, "manuscript/01.md", V0).unwrap();
    let c = clock_of(&root, "manuscript/01.md");
    assert_eq!(c.get(&dev), Some(&1), "санітарна перевірка: {c:?}");

    // пакет від того ж пристрою з ДОМІНУЮЧИМ вектором {dev: 2} і новим тілом
    let newer = V0.replace("Рядок другий.", "Рядок другий ОНОВЛЕНИЙ.");
    let packet = packet_with(dev.as_str(), "manuscript/01.md", 2, &newer);
    let ppath = root.join("packet-ff.json");
    std::fs::write(&ppath, serde_json::to_string_pretty(&packet).unwrap()).unwrap();

    let rep = sync::import_packet(store, &ppath).unwrap();
    assert_eq!(rep.fast_forward, 1, "домінування → fast-forward: {rep:?}");
    assert_eq!(rep.merged + rep.conflicts, 0);
    assert!(body_of(&root, "manuscript/01.md").contains("ОНОВЛЕНИЙ"), "тіло взято з пакета");
    let c = clock_of(&root, "manuscript/01.md");
    assert_eq!(c.get(&dev), Some(&2), "годинник = вектор пакета");

    // журнал: fast-forward записано
    let log = sync::log_rows(store, 10).unwrap();
    assert!(log.iter().any(|r| r.action == "fast-forward" && r.doc_path == "manuscript/01.md"));

    // локальний новіший (локальний домінує) → skip, тіло не змінюється
    sync::save_doc(store, "manuscript/01.md", "Локальна новіша правка.\n").unwrap();
    let older_packet = packet_with(dev.as_str(), "manuscript/01.md", 2, &newer);
    let ppath2 = root.join("packet-older.json");
    std::fs::write(&ppath2, serde_json::to_string_pretty(&older_packet).unwrap()).unwrap();
    let rep2 = sync::import_packet(store, &ppath2).unwrap();
    assert_eq!(rep2.skipped, 1, "локальний домінує → пропущено: {rep2:?}");
    assert!(body_of(&root, "manuscript/01.md").contains("Локальна новіша правка"));
}

#[test]
fn concurrent_conflicting_line_gets_markers_alert_and_row() {
    // дві машини редагують ТОЙ САМИЙ рядок незалежно (непорівнювані вектори)
    let (root_a, store_a) = make_project("conf-a", V0);
    let (root_b, store_b) = make_project("conf-b", V0);

    let body_a = V0.replace("Рядок другий.", "Рядок другий — правка пристрою A.");
    let body_b = V0.replace("Рядок другий.", "Рядок другий — правка пристрою B.");
    sync::save_doc(&store_a, "manuscript/01.md", &body_a).unwrap();
    sync::save_doc(&store_b, "manuscript/01.md", &body_b).unwrap();

    let dev_a = sync::device_id(&root_a).unwrap();
    let dev_b = sync::device_id(&root_b).unwrap();
    assert_ne!(dev_a, dev_b, "у різних проєктів різні device_id");

    // B експортує свій пакет, A імпортує
    let packet_path = tmpdir("conf-packet").join("packet.json");
    let rep_exp = sync::export_packet(&store_b, &packet_path).unwrap();
    assert_eq!(rep_exp.docs, 1, "змінений док у пакеті");
    let rep = sync::import_packet(&store_a, &packet_path).unwrap();
    assert_eq!(rep.conflicts, 1, "одночасна правка одного рядка → конфлікт: {rep:?}");

    // маркери прямо у файлі з обома варіантами
    let merged = body_of(&root_a, "manuscript/01.md");
    assert!(merged.contains("<<<<<<< "), "маркер початку: {merged}");
    assert!(merged.contains(&format!("<<<<<<< {dev_a}")), "маркер із device_id локального");
    assert!(merged.contains("======="));
    assert!(merged.contains(&format!(">>>>>>> {dev_b}")), "маркер із device_id віддаленого");
    assert!(merged.contains("правка пристрою A."), "локальний варіант у файлі");
    assert!(merged.contains("правка пристрою B."), "віддалений варіант у файлі");

    // рядок у sync_conflicts
    let rows = sync::conflicts(&store_a).unwrap();
    assert_eq!(rows.len(), 1, "один конфліктний фрагмент: {rows:?}");
    assert_eq!(rows[0].doc_path, "manuscript/01.md");
    assert_eq!(rows[0].local_device, dev_a);
    assert_eq!(rows[0].remote_device, dev_b);
    assert!(rows[0].local_text.contains("пристрою A"));
    assert!(rows[0].remote_text.contains("пристрою B"));
    // 3-й рядок тіла: перший рядок тіла — порожній (перенос закриваючого "---")
    assert_eq!(rows[0].local_line, Some(3), "конфлікт на рядку «Рядок другий»");

    // алерт kind='sync-conflict' (нерозв'язаний)
    let alerts: Vec<(String, String)> = {
        let mut stmt = store_a
            .conn()
            .prepare("SELECT kind, payload_json FROM alerts WHERE kind='sync-conflict' AND resolved=0")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    assert_eq!(alerts.len(), 1, "рівно один відкритий sync-conflict алерт: {alerts:?}");
    assert!(alerts[0].1.contains("manuscript/01.md"));

    // журнал: conflict
    let log = sync::log_rows(&store_a, 10).unwrap();
    assert!(log.iter().any(|r| r.action == "conflict" && r.doc_path == "manuscript/01.md"));

    // годинник після конфлікту = max по компонентах
    let c = clock_of(&root_a, "manuscript/01.md");
    assert_eq!(c.get(&dev_a), Some(&1));
    assert_eq!(c.get(&dev_b), Some(&1));
}

#[test]
fn concurrent_nonconflicting_edits_merge_both() {
    let (root_a, store_a) = make_project("nc-a", V0);
    let (_root_b, store_b) = make_project("nc-b", V0);

    // A править 2-й рядок, B — 3-й (різні рядки)
    let body_a = V0.replace("Рядок другий.", "Рядок другий (правка A).");
    let body_b = V0.replace("Рядок третій.", "Рядок третій (правка B).");
    sync::save_doc(&store_a, "manuscript/01.md", &body_a).unwrap();
    sync::save_doc(&store_b, "manuscript/01.md", &body_b).unwrap();

    let packet_path = tmpdir("nc-packet").join("packet.json");
    sync::export_packet(&store_b, &packet_path).unwrap();
    let rep = sync::import_packet(&store_a, &packet_path).unwrap();
    assert_eq!(rep.merged, 1, "автозлиття: {rep:?}");
    assert_eq!(rep.conflicts, 0);

    let merged = body_of(&root_a, "manuscript/01.md");
    assert!(merged.contains("(правка A)"), "правка A застосована: {merged}");
    assert!(merged.contains("(правка B)"), "правка B застосована");
    assert!(merged.contains("Рядок перший."), "незмінений рядок на місці");
    assert!(!merged.contains("<<<<<<<"), "без маркерів: {merged}");

    // конфліктів у БД немає
    assert!(sync::conflicts(&store_a).unwrap().is_empty());
    // журнал: merged
    let log = sync::log_rows(&store_a, 10).unwrap();
    assert!(log.iter().any(|r| r.action == "merged" && r.doc_path == "manuscript/01.md"));
}

#[test]
fn export_import_round_trip_on_second_device() {
    let (root_a, store_a) = make_project("rt-a", V0);
    let (root_b, store_b) = make_project("rt-b", "");

    let dev_a = sync::device_id(&root_a).unwrap();
    let dev_b = sync::device_id(&root_b).unwrap();
    assert_ne!(dev_a, dev_b);

    // A: правка і експорт
    let body_a = V0.replace("Рядок перший.", "Рядок перший (версія A).");
    sync::save_doc(&store_a, "manuscript/01.md", &body_a).unwrap();
    let packet1 = tmpdir("rt-p1").join("a-to-b.json");
    let e1 = sync::export_packet(&store_a, &packet1).unwrap();
    assert_eq!(e1.device, dev_a);
    assert_eq!(e1.docs, 1);

    // B: імпорт → док створюється (fast-forward add)
    let rep1 = sync::import_packet(&store_b, &packet1).unwrap();
    assert_eq!(rep1.fast_forward, 1, "{rep1:?}");
    assert_eq!(body_of(&root_b, "manuscript/01.md"), body_a, "тіло перенесене як є");
    let cb = clock_of(&root_b, "manuscript/01.md");
    assert_eq!(cb.get(&dev_a), Some(&1), "годинник A у B");

    // B: СВОЯ правка поверх і експорт назад
    let body_b = body_a.replace("Рядок третій.", "Рядок третій (версія B).");
    sync::save_doc(&store_b, "manuscript/01.md", &body_b).unwrap();
    let cb2 = clock_of(&root_b, "manuscript/01.md");
    assert_eq!(cb2.get(&dev_b), Some(&1), "лічильник B: 1");
    let packet2 = tmpdir("rt-p2").join("b-to-a.json");
    sync::export_packet(&store_b, &packet2).unwrap();

    // A: імпорт відповіді — вектор B домінує → fast-forward
    let rep2 = sync::import_packet(&store_a, &packet2).unwrap();
    assert_eq!(rep2.fast_forward, 1, "{rep2:?}");
    assert_eq!(body_of(&root_a, "manuscript/01.md"), body_b, "round-trip: A отримав правку B");
    let ca2 = clock_of(&root_a, "manuscript/01.md");
    assert_eq!(ca2.get(&dev_a), Some(&1), "лічильник A не здувається");
    assert_eq!(ca2.get(&dev_b), Some(&1), "лічильник B переноситься");

    // тепер A править і відправляє B — B отримує без конфліктів
    let body_a2 = body_b.replace("Рядок другий.", "Рядок другий (версія A2).");
    sync::save_doc(&store_a, "manuscript/01.md", &body_a2).unwrap();
    let packet3 = tmpdir("rt-p3").join("a2.json");
    sync::export_packet(&store_a, &packet3).unwrap();
    let rep3 = sync::import_packet(&store_b, &packet3).unwrap();
    assert_eq!(rep3.merged + rep3.fast_forward, 1, "{rep3:?}");
    assert_eq!(rep3.conflicts, 0);
    assert_eq!(body_of(&root_b, "manuscript/01.md"), body_a2, "другий раунд без конфлікту");
}

#[test]
fn sync_log_is_written_and_status_reports() {
    let (root_a, store_a) = make_project("log-a", V0);
    let (_root_b, store_b) = make_project("log-b", V0);

    let body_a = V0.replace("Рядок другий.", "Рядок другий (A).");
    let body_b = V0.replace("Рядок третій.", "Рядок третій (B).");
    sync::save_doc(&store_a, "manuscript/01.md", &body_a).unwrap();
    sync::save_doc(&store_b, "manuscript/01.md", &body_b).unwrap();

    let packet = tmpdir("log-packet").join("p.json");
    sync::export_packet(&store_b, &packet).unwrap();
    sync::import_packet(&store_a, &packet).unwrap(); // merged

    let log = sync::log_rows(&store_a, 10).unwrap();
    assert!(!log.is_empty(), "журнал не порожній");
    assert!(
        log.iter().all(|r| matches!(r.action.as_str(), "merged" | "fast-forward" | "conflict")),
        "лише дозволені дії: {log:?}"
    );
    let merged_row = log.iter().find(|r| r.action == "merged").expect("запис merged");
    assert_eq!(merged_row.doc_path, "manuscript/01.md");
    assert!(!merged_row.device.is_empty(), "device у журналі");
    assert!(merged_row.ts > 0);

    // status: device_id + годинники доків + журнал
    let st = sync::status(&store_a).unwrap();
    assert_eq!(st.device_id, sync::device_id(&root_a).unwrap());
    assert!(st
        .docs
        .iter()
        .any(|d| d.path == "manuscript/01.md" && d.clock.contains("=1")),
        "годинник дока у status: {:?}", st.docs);
    assert!(!st.log.is_empty(), "журнал у status");
    // хронологія: перший запис — найдавніший
    if st.log.len() >= 2 {
        assert!(st.log[0].id < st.log[1].id, "журнал у хронологічному порядку");
    }
}
