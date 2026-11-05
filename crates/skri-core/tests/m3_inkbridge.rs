//! Acceptance-тести M3: round-trip міст Ink/Yarn ↔ markdown (inkbridge).
//!
//! Фікстури — укр. (Ink) та англ. (Yarn) текстом, вбудовані в файл
//! (незалежність від шляхів). Round-trip: import → markdown → export →
//! re-import має давати семантично еквівалентний файл.

use skri_core::inkbridge::{self, Format};
use skri_core::narrative;
use skri_core::tests_common::tmp_project;

/// Укр. Ink-фікстура: ноди, диверти, вибір, VAR + ~ set/check.
const INK_UK: &str = "\
// Діалог: Мудрець біля брами (укр.)
VAR trust = 0

== мудрець ===
Мудрець підводить очі.
~ trust = trust + 1
Ти ще тут? -> питання

== питання ===
{ trust }
* [Так] -> розвилка

== розвилка ===
Мудрець киває.
-> кінець

== кінець ===
Кінець розмови.
";

/// Англ. Yarn-фікстура: ноди, посилання, <<set>>/<<if>>.
const YARN_EN: &str = "\
title: Gate
---
The gate creaks in the wind.
<<set $key_found = true>>
<<set $coins = 12>>
<<if $key_found>>
The gate is open: [[Stairs]]
<<endif>>
===

title: Stairs
---
You climb the narrow stairs.
[[Hall]]
===

title: Hall
---
<<if $coins>>
The hall remembers your debt.
[[Gate]]
===
";

#[test]
fn ink_roundtrip_preserves_semantics() {
    let doc1 = inkbridge::parse_ink(INK_UK).unwrap();
    let md = inkbridge::to_markdown(&doc1);
    let doc2 = inkbridge::parse_markdown_file(&md).unwrap();
    assert_eq!(
        inkbridge::semantic_signature(&doc1),
        inkbridge::semantic_signature(&doc2),
        "ink → markdown → re-parse: семантика має зберегтися"
    );
    let ink2 = inkbridge::export_source(&doc2, Format::Ink).unwrap();
    let doc3 = inkbridge::parse_ink(&ink2).unwrap();
    assert_eq!(
        inkbridge::semantic_signature(&doc1),
        inkbridge::semantic_signature(&doc3),
        "round-trip ink: import → export → import = еквівалентно"
    );
    // ноди й диверти на місці
    let names: Vec<&str> = doc3.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["мудрець", "питання", "розвилка", "кінець"]);
    let pit = &doc3.nodes[1];
    assert!(pit.lines.iter().any(|l| l.contains("[goto розвилка]")), "диверт вибору зберігся");
    // markdown-тіло сумісне з narrative::graph_audit
    let body = inkbridge::to_markdown(&doc3);
    let audit = narrative::graph_audit(&body);
    assert!(audit.dangling_gotos.is_empty(), "усі goto цільові: {:?}", audit.dangling_gotos);
}

#[test]
fn yarn_roundtrip_preserves_semantics() {
    let doc1 = inkbridge::parse_yarn(YARN_EN).unwrap();
    let md = inkbridge::to_markdown(&doc1);
    let doc2 = inkbridge::parse_markdown_file(&md).unwrap();
    assert_eq!(
        inkbridge::semantic_signature(&doc1),
        inkbridge::semantic_signature(&doc2),
        "yarn → markdown → re-parse: семантика має зберегтися"
    );
    let yarn2 = inkbridge::export_source(&doc2, Format::Yarn).unwrap();
    let doc3 = inkbridge::parse_yarn(&yarn2).unwrap();
    assert_eq!(
        inkbridge::semantic_signature(&doc1),
        inkbridge::semantic_signature(&doc3),
        "round-trip yarn: import → export → import = еквівалентно"
    );
    let names: Vec<&str> = doc3.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["Gate", "Stairs", "Hall"]);
    let gate = &doc3.nodes[0];
    assert!(gate.lines.iter().any(|l| l.contains("[goto Stairs]")), "посилання yarn збереглося");
    assert!(yarn2.contains("<<set $key_found = true>>"), "set-команда повертається у yarn");
    assert!(yarn2.contains("<<if $coins>>"), "if-умова повертається у yarn");
}

#[test]
fn variables_survive_roundtrip_and_reach_auditor() {
    // Ink: VAR-декларації + ~ set + { check }
    let doc = inkbridge::parse_ink(INK_UK).unwrap();
    assert_eq!(doc.vars, vec![("trust".to_string(), "0".to_string())], "VAR-декларації зберігаються");
    let md = inkbridge::to_markdown(&doc);
    assert!(md.contains("%SET trust = trust + 1"), "значення set-операції не втрачається");
    assert!(md.contains("%CHECK trust"), "check-маркер на місці");

    // зворотний бік: %SET із = → ~ x = v
    let ink2 = inkbridge::export_source(&doc, Format::Ink).unwrap();
    assert!(ink2.contains("VAR trust = 0"));
    assert!(ink2.contains("~ trust = trust + 1"));
    assert!(ink2.contains("{ trust }"), "%CHECK → {{ x }}");

    // Yarn: <<set>>/<<if>> → %SET/%CHECK → назад у <<set>>/<<if>>
    let ydoc = inkbridge::parse_yarn(YARN_EN).unwrap();
    let ymd = inkbridge::to_markdown(&ydoc);
    assert!(ymd.contains("%SET key_found"), "<<set $x = true>> → голий %SET");
    assert!(ymd.contains("%SET coins = 12"));
    assert!(ymd.contains("%CHECK key_found"));
    assert!(ymd.contains("%CHECK coins"));

    // інтеграція з аудитором змінних: parse_var_ops бачить маркери
    // (увага: існуючий parse_var_ops бере весь хвіст після %SET як «ім'я»,
    // тому set зі значенням дає "coins = 12" — но check дає чисте ім'я)
    let parsed = skri_core::frontmatter::parse(&ymd);
    let ops = narrative::parse_var_ops(&parsed.body, "dialogue/test.md");
    assert!(ops.iter().any(|(op, n, _)| op == "set" && n.starts_with("key_found")), "аудитор бачить yarn-змінну: {ops:?}");
    assert!(ops.iter().any(|(op, n, _)| op == "set" && n.starts_with("coins")));
    assert!(ops.iter().any(|(op, n, _)| op == "check" && n == "coins"), "check дає чисте ім'я змінної");

    let parsed2 = skri_core::frontmatter::parse(&md);
    let ops2 = narrative::parse_var_ops(&parsed2.body, "dialogue/test.md");
    assert!(ops2.iter().any(|(op, n, _)| op == "set" && n.starts_with("trust")), "set trust дійшов до аудитора");
    assert!(ops2.iter().any(|(op, n, _)| op == "check" && n == "trust"));
}

#[test]
fn external_changes_flagged_as_incoming_edits() {
    let (root, store) = tmp_project("ink-external");
    let src = root.join("Мудрець.ink");
    std::fs::write(&src, INK_UK).unwrap();

    // перший імпорт — чистий
    let rep1 = inkbridge::import_file(&store, &src).unwrap();
    assert!(!rep1.external_change, "перший імпорт не має давати алерт");
    assert_eq!(rep1.nodes, 4);
    assert!(root.join("dialogue").join("Мудрець.md").exists());

    // повторний імпорт без змін — алертів немає
    let rep2 = inkbridge::import_file(&store, &src).unwrap();
    assert!(!rep2.external_change);

    // «хтось інший» править .ink (git diff рушія) → повторний імпорт флагує
    let mut changed = INK_UK.to_string();
    changed.push_str("== нова_нода ===\nГраф мовчить.\n-> мудрець\n");
    std::fs::write(&src, &changed).unwrap();
    let rep3 = inkbridge::import_file(&store, &src).unwrap();
    assert!(rep3.external_change, "змінений файл має дати «вхідні правки»");

    let alerts: Vec<(i64, String)> = {
        let mut st = store
            .conn()
            .prepare("SELECT id, payload_json FROM alerts WHERE kind='ink-external-change'")
            .unwrap();
        st.query_map(rusqlite::params![], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    assert_eq!(alerts.len(), 1, "рівно один алерт зовнішньої зміни");
    let payload: serde_json::Value = serde_json::from_str(&alerts[0].1).unwrap();
    assert!(payload["src"].as_str().unwrap().ends_with("Мудрець.ink"));
    assert!(payload["old_hash"].as_str().unwrap() != payload["new_hash"].as_str().unwrap());
    assert!(payload["message"].as_str().unwrap().contains("вхідні правки"));

    // імпорт незміненого файлу знову — нових алертів немає
    let rep4 = inkbridge::import_file(&store, &src).unwrap();
    assert!(!rep4.external_change);
    let count: i64 = store
        .conn()
        .query_row("SELECT count(*) FROM alerts WHERE kind='ink-external-change'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "без змін — без нових алертів");

    // імпорт yarn теж працює через той самий шлях
    let ysrc = root.join("gate.yarn");
    std::fs::write(&ysrc, YARN_EN).unwrap();
    let yrep = inkbridge::import_file(&store, &ysrc).unwrap();
    assert_eq!(yrep.nodes, 3);
    assert!(root.join("dialogue").join("gate.md").exists());
    let _ = root;
}
