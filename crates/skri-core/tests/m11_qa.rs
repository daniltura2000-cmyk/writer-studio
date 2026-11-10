//! QA-звіт кінцевого користувача: регресійні acceptance-тести (M11).
//!
//! QA-01 (P1) — збереження не пошкоджує frontmatter: тіло без початкового
//! порожнього рядка («# Заголовок») не зливається з роздільником `---`;
//! інваріант title_preserved ловить будь-яке майбутнє пошкодження з'єднання.
//! QA-02 (P1) — рушій фактів досьє: витяг (вік, колір очей) з bible/*.md
//! і глав, конфлікти між главами (fact-conflict) і проти канону
//! (fact-vs-dossier), сумісність з alias-ами і стаб-нормалізацією відмінків.
//! QA-03 (P2) — відмінкові форми («Дэвид»/«Дэвида») — одне ім'я, не drift.

use skri_core::bible::{add_manual_card, ManualCard};
use skri_core::store::ProjectStore;
use skri_core::{bible, drift, facts, frontmatter, indexer, tests_common};

/// Повний конвеєр індексування (те саме, що `skri index`; FQA14: біблія —
/// джерело правди — синхронізується ДО скану рукопису).
fn pipeline(store: &ProjectStore) {
    store.reindex().expect("reindex");
    bible::sync_bible(store).expect("sync_bible");
    indexer::scan_project(store).expect("scan");
    bible::merge_auto_into_manual(store).expect("merge");
}

fn fact_alerts(store: &ProjectStore, kind: &str) -> Vec<(String, serde_json::Value)> {
    let mut stmt = store
        .conn()
        .prepare("SELECT payload_json FROM alerts WHERE kind=?1 AND resolved=0 ORDER BY id")
        .unwrap();
    let rows = stmt.query_map([kind], |r| r.get::<_, String>(0)).unwrap();
    rows.flatten()
        .map(|p| (kind.to_string(), serde_json::from_str(&p).unwrap()))
        .collect()
}

// ---------------------------------------------------------------- QA-01

#[test]
fn qa01_body_starting_with_hash_stays_parseable() {
    let (root, store) = tests_common::tmp_project("qa01-hash");
    let path = root.join("manuscript/01.md");
    std::fs::create_dir_all(root.join("manuscript")).unwrap();
    std::fs::write(
        &path,
        "---\ntitle: QA Канон\nlang: uk\n---\n\nСтарий текст.\n",
    )
    .unwrap();

    // «редактор» надсилає тіло БЕЗ початкового порожнього рядка
    let raw = std::fs::read_to_string(&path).unwrap();
    let body = "# QA Канон\n\nНовий текст трьома словами.";
    let out = frontmatter::splice_body(&raw, body);
    assert!(
        frontmatter::title_preserved(&raw, &out),
        "інваріант з'єднання"
    );
    std::fs::write(&path, out).unwrap();

    // файл парситься, frontmatter на місці
    let after = std::fs::read_to_string(&path).unwrap();
    let parsed = frontmatter::parse(&after);
    assert_eq!(parsed.meta.get("title").unwrap(), "QA Канон");
    assert!(!after.contains("---#"), "роздільник не злитий з тілом");
    // frontmatter НЕ потрапляє в тіло
    assert!(
        !parsed.body.contains("title:"),
        "метадані в тілі: {:?}",
        parsed.body
    );
    assert!(
        !parsed.body.contains("lang:"),
        "метадані в тілі: {:?}",
        parsed.body
    );

    // word_count рахує ТІЛЬКИ тіло, без метаданих
    store.reindex().unwrap();
    let wc: i64 = store
        .conn()
        .query_row(
            "SELECT word_count FROM docs WHERE path='manuscript/01.md'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(wc as usize, indexer::count_words(body));
}

#[test]
fn qa01_body_with_leading_newline_is_stable() {
    // FQA-11: тіло з початковими переносами нормалізується (parse і splice
    // symmetric) — save→read→save байт-стабільний, редактор без «фантомних»
    // порожніх рядків після роздільника
    let raw = "---\ntitle: T\n---\n";
    let out = frontmatter::splice_body(raw, "\n\nТіло.");
    assert_eq!(out, "---\ntitle: T\n---\nТіло.");
    let p = frontmatter::parse(&out);
    assert_eq!(p.meta.get("title").unwrap(), "T");
    assert_eq!(p.body, "Тіло.");
    // повторне збереження — байт-ідентичний файл
    assert_eq!(frontmatter::splice_body(&out, "\n\nТіло."), out);
}

#[test]
fn qa01_merged_separator_is_reported_not_written() {
    // відтворення бага QA-01: злитий роздільник = файл більше не парситься —
    // інваріант має саме це ловити (бо write_doc відмовляє такий запис).
    let broken = "---\ntitle: QA Канон\n---# тіло, змите з роздільником";
    assert!(
        frontmatter::parse(broken).meta.is_empty(),
        "передумова: злитий роздільник непарсуваний"
    );
    assert!(
        !frontmatter::title_preserved("---\ntitle: QA Канон\n---\n\nок", broken),
        "інваріант ловить втрачений title"
    );
}

#[test]
fn qa01_splice_preserves_header_byte_for_byte() {
    let raw = "---\ntitle: Розділ 1\npub_status: draft\nlang: uk\n---\n\nСтаре.\n";
    let body = "# Новий заголовок\n\nТекст.";
    let out = frontmatter::splice_body(raw, body);
    // header байт-в-байт (frontmatter не чіпається)
    let head_before: String = raw.lines().take_while(|l| *l != "---").collect();
    let head_after: String = out.lines().take_while(|l| *l != "---").collect();
    assert_eq!(head_before, head_after);
    assert!(out.starts_with("---\ntitle: Розділ 1\npub_status: draft\nlang: uk\n---\n"));
    assert!(out.ends_with("# Новий заголовок\n\nТекст."));
}

// ---------------------------------------------------------------- QA-02

#[test]
fn qa02_age_dossier_vs_chapters_gives_two_alerts() {
    let (root, store) = tests_common::tmp_project("qa02-age");
    std::fs::create_dir_all(root.join("manuscript")).unwrap();
    // досьє: «Йому 25 років» — канон
    add_manual_card(
        &root.to_string_lossy(),
        &ManualCard {
            name: "Тарас".into(),
            aliases: vec![],
            kind: "персонаж".into(),
            notes: "Йому 25 років. Стежити за віком.".into(),
        },
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript/01.md"),
        "---\ntitle: Г1\n---\n\nТарасу було 25 років, і він усміхнувся.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript/02.md"),
        "---\ntitle: Г2\n---\n\nТарас подивився в дзеркало — йому виповнилося 40 років.\n",
    )
    .unwrap();

    pipeline(&store);
    let rep = facts::rebuild(&store).expect("facts rebuild");

    // факт витягнутий і з досьє, і з обох глав
    assert!(
        rep.facts >= 3,
        "має бути ≥3 факти (досьє + 2 глави): {rep:?}"
    );
    // очікування QA-сценарію: рівно 2 алерти
    let vs = fact_alerts(&store, "fact-vs-dossier");
    let conflict = fact_alerts(&store, "fact-conflict");
    assert_eq!(vs.len(), 1, "глава 40 суперечить досьє 25: {vs:?}");
    assert_eq!(
        conflict.len(),
        1,
        "конфлікт 25 ↔ 40 між главами: {conflict:?}"
    );

    // payload-и: 40 проти 25 (досьє)
    let v = &vs[0].1;
    assert_eq!(v.get("entity").and_then(|x| x.as_str()), Some("Тарас"));
    assert_eq!(v.get("value_a").and_then(|x| x.as_str()), Some("40"));
    assert_eq!(v.get("value_b").and_then(|x| x.as_str()), Some("25"));
    assert_eq!(v.get("kind").and_then(|x| x.as_str()), Some("age"));
    // FQA-03: перехід до глави — цитата речення, offset у тілі, глава-джерело
    let body_02 = indexer::read_body(&store, "manuscript/02.md").unwrap();
    assert_eq!(
        v.get("doc").and_then(|x| x.as_str()),
        Some("manuscript/02.md"),
        "doc = глава з відмінним значенням"
    );
    assert_eq!(v.get("kind_label").and_then(|x| x.as_str()), Some("вік"));
    let sent = v
        .get("sentence")
        .and_then(|x| x.as_str())
        .unwrap_or_default();
    assert!(!sent.trim().is_empty(), "цитата речення непорожня: {v:?}");
    assert!(sent.contains("40"), "цитата містить факт: {sent:?}");
    let cs = v.get("char_start").and_then(|x| x.as_i64()).unwrap_or(-1) as usize;
    assert!(
        cs < body_02.len(),
        "offset у межах тіла: {cs} проти {}",
        body_02.len()
    );
    assert_eq!(
        &body_02[cs..cs + "40".len()],
        "40",
        "char_start вказує точно на «40» у тілі глави"
    );
    let c = &conflict[0].1;
    assert_eq!(c.get("value_a").and_then(|x| x.as_str()), Some("25"));
    assert_eq!(c.get("value_b").and_then(|x| x.as_str()), Some("40"));
    // FQA-03: у fact-conflict doc = глава з відмінним (другим) значенням
    assert_eq!(
        c.get("doc").and_then(|x| x.as_str()),
        Some("manuscript/02.md"),
        "глава-джерело конфлікту: {c:?}"
    );
    let csent = c
        .get("sentence")
        .and_then(|x| x.as_str())
        .unwrap_or_default();
    assert!(
        !csent.trim().is_empty() && csent.contains("40"),
        "цитата конфлікту: {csent:?}"
    );
    let ccs = c.get("char_start").and_then(|x| x.as_i64()).unwrap_or(-1) as usize;
    assert!(ccs < body_02.len(), "offset у межах тіла: {ccs}");
    assert_eq!(&body_02[ccs..ccs + "40".len()], "40");

    // ідемпотентність: повторний прогін не плодить алертів
    let rep2 = facts::rebuild(&store).unwrap();
    assert_eq!(rep2.conflicts, 1);
    assert_eq!(rep2.vs_dossier, 1);
    assert_eq!(fact_alerts(&store, "fact-conflict").len(), 1);
    assert_eq!(fact_alerts(&store, "fact-vs-dossier").len(), 1);
}

#[test]
fn qa02_eye_color_dossier_vs_chapter_alerts() {
    let (root, store) = tests_common::tmp_project("qa02-eyes");
    std::fs::create_dir_all(root.join("manuscript")).unwrap();
    add_manual_card(
        &root.to_string_lossy(),
        &ManualCard {
            name: "Тарас".into(),
            aliases: vec![],
            kind: "персонаж".into(),
            notes: "Його очі зелені. Стежити за кольором очей.".into(),
        },
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript/01.md"),
        "---\ntitle: Г1\n---\n\nТарас мружився: очі зелені.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript/02.md"),
        "---\ntitle: Г2\n---\n\nТарас зиркнув: очі блакитні.\n",
    )
    .unwrap();

    pipeline(&store);
    let rep = facts::rebuild(&store).unwrap();

    let vs = fact_alerts(&store, "fact-vs-dossier");
    assert_eq!(vs.len(), 1, "блакитні проти канону «зелені»: {vs:?}");
    let v = &vs[0].1;
    assert_eq!(v.get("value_a").and_then(|x| x.as_str()), Some("блакитні"));
    assert_eq!(v.get("value_b").and_then(|x| x.as_str()), Some("зелені"));
    assert_eq!(v.get("kind").and_then(|x| x.as_str()), Some("eye_color"));
    // FQA-03: перехід — doc = глава з «блакитні», offset точно на слово
    assert_eq!(
        v.get("doc").and_then(|x| x.as_str()),
        Some("manuscript/02.md")
    );
    let fsent = v
        .get("sentence")
        .and_then(|x| x.as_str())
        .unwrap_or_default();
    assert!(fsent.contains("блакитні"), "цитата речення: {fsent:?}");
    let fbody = indexer::read_body(&store, "manuscript/02.md").unwrap();
    let fcs = v.get("char_start").and_then(|x| x.as_i64()).unwrap_or(-1) as usize;
    assert!(fcs < fbody.len(), "offset у межах тіла: {fcs}");
    assert_eq!(&fbody[fcs..fcs + "блакитні".len()], "блакитні");
    // гл. 1 (зелені) узгоджена з досьє — алертів зайвих немає
    assert_eq!(rep.vs_dossier, 1);
    assert_eq!(
        fact_alerts(&store, "fact-conflict").len(),
        1,
        "зелені ↔ блакитні між главами"
    );
}

#[test]
fn qa02_no_dossier_but_chapter_conflict_still_alerts() {
    let (root, store) = tests_common::tmp_project("qa02-nodossier");
    std::fs::create_dir_all(root.join("manuscript")).unwrap();
    std::fs::write(
        root.join("manuscript/01.md"),
        "---\ntitle: Г1\n---\n\nУ дзеркало глянув Марко: йому 25 років.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript/02.md"),
        "---\ntitle: Г2\n---\n\nПостарів за рік Марко — йому вже 40 років.\n",
    )
    .unwrap();

    pipeline(&store);
    let rep = facts::rebuild(&store).unwrap();

    assert_eq!(
        rep.conflicts, 1,
        "конфлікт між главами ловиться і без досьє"
    );
    assert_eq!(fact_alerts(&store, "fact-conflict").len(), 1);
    // fact-vs-dossier нема — досьє нема
    assert_eq!(fact_alerts(&store, "fact-vs-dossier").len(), 0);
}

#[test]
fn qa02_case_stub_resolves_one_entity() {
    // стаб-нормалізація відмінків: «Олександра» → база «Олександр»
    let (root, store) = tests_common::tmp_project("qa02-case");
    std::fs::create_dir_all(root.join("manuscript")).unwrap();
    std::fs::write(
        root.join("manuscript/01.md"),
        "---\ntitle: Г1\n---\n\nУ кімнату увійшов Олександр.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript/02.md"),
        "---\ntitle: Г2\n---\n\nОлександра не було поруч.\n",
    )
    .unwrap();

    pipeline(&store);

    // одна сутність, друга форма — в aliases
    let mut stmt = store
        .conn()
        .prepare("SELECT canonical_form, aliases_json FROM entities WHERE source LIKE 'auto%'")
        .unwrap();
    let ents: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    let named: Vec<&(String, String)> = ents
        .iter()
        .filter(|(c, _)| drift::strip_case_suffix(c) == "олександр")
        .collect();
    assert_eq!(
        named.len(),
        1,
        "Олександр/Олександра = одна сутність: {ents:?}"
    );
    assert!(
        named[0].1.contains("Олександра"),
        "відмінкова форма в aliases: {}",
        named[0].1
    );
    assert_eq!(
        fact_alerts(&store, "name-drift").len(),
        0,
        "без drift-алерту"
    );
}

#[test]
fn qa02_ru_colors_normalize_to_uk_canon() {
    // «зеленые» (ru) і «зелені» (uk) — ОДНЕ значення, не хибний конфлікт
    let f = facts::extract_facts("Його глаза зеленые.");
    assert_eq!(f.len(), 1, "очі+колір (ru): {f:?}");
    assert_eq!(f[0].1, "зелені", "рос. форма нормалізується до укр. канону");
    assert_eq!(f[0].0, "eye_color");

    // uk-варіант «зелені очі» (колір ПЕРЕД словом) теж ловиться
    let f = facts::extract_facts("Його зелені очі світлися.");
    assert_eq!(f.len(), 1, "колір перед «очі»: {f:?}");
    assert_eq!(f[0].1, "зелені");

    // вік: «йому 25 років» і «йому було 25 років»
    let f = facts::extract_facts("Йому 25 років.");
    assert_eq!(f.len(), 1, "число+одиниця: {f:?}");
    assert_eq!(f[0].0, "age");
    assert_eq!(f[0].1, "25");
    let f = facts::extract_facts("Йому було 25 років.");
    assert!(
        f.iter().any(|(k, v, _)| k == "age" && v == "25"),
        "з дієсловом між: {f:?}"
    );
}

// ---------------------------------------------------------------- QA-03

#[test]
fn qa03_case_forms_are_one_name_not_drift() {
    let (root, store) = tests_common::tmp_project("qa03-david");
    std::fs::create_dir_all(root.join("manuscript")).unwrap();
    std::fs::write(
        root.join("manuscript/01.md"),
        "---\ntitle: Г1\n---\n\nИз-за угла вышел Дэвид.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript/02.md"),
        "---\ntitle: Г2\n---\n\nДэвида не было рядом.\n",
    )
    .unwrap();

    pipeline(&store);

    // 0 drift-алертів: «Дэвид» ↔ «Дэвида» — одне ім'я
    assert_eq!(
        fact_alerts(&store, "name-drift").len(),
        0,
        "відмінкові форми не мають бути drift-алертом"
    );
    // одна сутність, обидві форми згадок належать їй
    let mut stmt = store
        .conn()
        .prepare(
            "SELECT e.canonical_form, e.aliases_json, COUNT(m.id)
             FROM entities e LEFT JOIN mentions m ON m.entity_id = e.id
             WHERE e.source LIKE 'auto%' GROUP BY e.id",
        )
        .unwrap();
    let ents: Vec<(String, String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    let david: Vec<&(String, String, i64)> = ents.iter().filter(|(c, _, _)| c == "Дэвид").collect();
    assert_eq!(david.len(), 1, "рівно одна сутність «Дэвид»: {ents:?}");
    assert!(
        david[0].1.contains("Дэвида"),
        "«Дэвида» в aliases: {}",
        david[0].1
    );
    assert!(david[0].2 >= 2, "обидві згадки перешиті на одну сутність");
}

// Репро QA користувача: факт у сусідньому реченні через займенник
// («Олександр йшов. Йому 25 років.») має атрибуціюватися до сутності абзацу.
#[test]
fn qa02_pronoun_fact_in_next_sentence_attributed() {
    let (root, store) = tests_common::tmp_project("qa02-pronoun");
    std::fs::create_dir_all(root.join("manuscript")).unwrap();
    // досьє, як у сценарії користувача — робить сутність канонічною
    skri_core::bible::add_manual_card(
        &root.to_string_lossy(),
        &skri_core::bible::ManualCard {
            name: "Олександр".into(),
            aliases: vec![],
            kind: "персонаж".into(),
            notes: "Стежити за віком і кольором очей.".into(),
        },
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript/01.md"),
        "---\ntitle: Г1\n---\n\nОлександр йшов по Києву. Йому 25 років. Його очі зелені.\n",
    )
    .unwrap();
    pipeline(&store);
    facts::rebuild(&store).unwrap();
    let conn = store.conn();
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM extracted_facts WHERE entity='Олександр'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        n >= 2,
        "вік і очі через займенник мають бути витягнуті: {n}"
    );
}

// Локація після прийменника («по Києву») не претендує на факти особи.
#[test]
fn qa02_prep_location_gets_no_facts() {
    let (root, store) = tests_common::tmp_project("qa02-prep");
    std::fs::create_dir_all(root.join("manuscript")).unwrap();
    std::fs::write(
        root.join("manuscript/01.md"),
        "---\ntitle: Г1\n---\n\nОлександр йшов по Києву. Йому 25 років.\n",
    )
    .unwrap();
    pipeline(&store);
    facts::rebuild(&store).unwrap();
    let conn = store.conn();
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM extracted_facts WHERE entity LIKE 'Київ%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 0, "«Києву» після прийменника не отримує фактів особи");
}

// ---------------------------------------------------------------- FQA14
// FQA14 (P2): неідемпотентна кількість згадок ручного персонажа — повторні
// Check/index накопичували збіги для manual-сутності (26 → 30 → 32 при
// незмінених файлах). Очікування: згадки = реальна кількість входжень форм
// імені у главу; повторні rebuild/reindex/check число не змінюють.
#[test]
fn fqa14_manual_mentions_idempotent_across_rebuilds() {
    let (root, store) = tests_common::tmp_project("fqa14-mentions");
    std::fs::create_dir_all(root.join("manuscript")).unwrap();
    add_manual_card(
        &root.to_string_lossy(),
        &ManualCard {
            name: "Олександр".into(),
            aliases: vec![],
            kind: "персонаж".into(),
            notes: "Йому 25 років. Його очі зелені.".into(),
        },
    )
    .unwrap();
    // 2 глави по 1 входженню «Олександр»
    std::fs::write(
        root.join("manuscript/01.md"),
        "---\ntitle: Г1\n---\n\nОлександр увійшов до кімнати.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript/02.md"),
        "---\ntitle: Г2\n---\n\nОлександр глянув у дзеркало.\n",
    )
    .unwrap();

    let count = |store: &ProjectStore| -> Vec<(String, i64)> {
        let mut stmt = store
            .conn()
            .prepare(
                "SELECT d.path, COUNT(*) FROM mentions m
                 JOIN entities e ON e.id = m.entity_id
                 JOIN docs d ON d.id = m.doc_id
                 WHERE e.canonical_form = 'Олександр'
                 GROUP BY d.path ORDER BY d.path",
            )
            .unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    let aliases = |store: &ProjectStore| -> String {
        store
            .conn()
            .query_row(
                "SELECT aliases_json FROM entities WHERE canonical_form='Олександр'",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap()
    };

    pipeline(&store);
    let first = count(&store);
    assert_eq!(
        first,
        vec![
            ("manuscript/01.md".to_string(), 1),
            ("manuscript/02.md".to_string(), 1)
        ],
        "по 1 реальній згадці на главу: {first:?}"
    );
    let aliases_first = aliases(&store);

    // rebuild ×3 — число стабільне
    pipeline(&store);
    pipeline(&store);
    assert_eq!(
        count(&store),
        first,
        "повторні rebuild не змінюють кількість згадок manual-сутності"
    );
    assert_eq!(
        aliases(&store),
        aliases_first,
        "aliases/форми не дублюються між перебудовами"
    );

    // збереження другого досьє («Марія») не змінює лічильник першого
    add_manual_card(
        &root.to_string_lossy(),
        &ManualCard {
            name: "Марія".into(),
            aliases: vec![],
            kind: "персонаж".into(),
            notes: "Їй 40 років. Її очі блакитні.".into(),
        },
    )
    .unwrap();
    pipeline(&store);
    assert_eq!(
        count(&store),
        first,
        "нове досьє не множить згадки іншої сутності"
    );
    // сумарно рівно 2 згадки (жодного накопичення/подвоєння)
    let total: i64 = store
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM mentions m JOIN entities e ON e.id=m.entity_id
             WHERE e.canonical_form='Олександр'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(total, 2, "разом 2 згадки, не 26/30/32");
}

// ---------------------------------------------------------------- FQA15
// FQA15 (P2): абзац-атрибуція приписувала факти ВСІМ персонажам абзацу.
// (а) два імена, два речення, один абзац — кожен факт своєму персонажу,
// 0 хибних алертів (текст узгоджений з каноном).
#[test]
fn fqa15_two_characters_each_fact_to_own_entity() {
    let (root, store) = tests_common::tmp_project("fqa15-two-chars");
    std::fs::create_dir_all(root.join("manuscript")).unwrap();
    add_manual_card(
        &root.to_string_lossy(),
        &ManualCard {
            name: "Олександр".into(),
            aliases: vec![],
            kind: "персонаж".into(),
            notes: "Йому 25 років. Його очі зелені.".into(),
        },
    )
    .unwrap();
    add_manual_card(
        &root.to_string_lossy(),
        &ManualCard {
            name: "Марія".into(),
            aliases: vec![],
            kind: "персонаж".into(),
            notes: "Їй 40 років. Її очі блакитні.".into(),
        },
    )
    .unwrap();
    // текст УЗГОДЖЕНИЙ з каноном — жодних алертів бути не має
    std::fs::write(
        root.join("manuscript/004-QA Два персонажі.md"),
        "---\ntitle: QA Два персонажі\n---\n\n# QA Два персонажі\n\nОлександр має 25 років, його очі зелені. Марія має 40 років, її очі блакитні.\n",
    )
    .unwrap();

    pipeline(&store);
    facts::rebuild(&store).unwrap();

    let vs = fact_alerts(&store, "fact-vs-dossier");
    let conflict = fact_alerts(&store, "fact-conflict");
    assert!(
        vs.is_empty(),
        "узгоджений текст не має давати fact-vs-dossier: {vs:?}"
    );
    assert!(
        conflict.is_empty(),
        "узгоджений текст не має давати fact-conflict: {conflict:?}"
    );

    // кожен факт — своєму персонажу, без крос-атрибуції
    let ents = |store: &ProjectStore, entity: &str| -> Vec<(String, String)> {
        let mut stmt = store
            .conn()
            .prepare(
                "SELECT kind, value FROM extracted_facts
                 WHERE doc_path LIKE 'manuscript%' AND entity = ?1",
            )
            .unwrap();
        stmt.query_map([entity], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    };
    let oleksandr = ents(&store, "Олександр");
    let mariia = ents(&store, "Марія");
    assert!(
        oleksandr.contains(&("age".to_string(), "25".to_string()))
            && oleksandr.contains(&("eye_color".to_string(), "зелені".to_string())),
        "факти «25/зелені» — Олександру: {oleksandr:?}"
    );
    assert!(
        mariia.contains(&("age".to_string(), "40".to_string()))
            && mariia.contains(&("eye_color".to_string(), "блакитні".to_string())),
        "факти «40/блакитні» — Марії: {mariia:?}"
    );
    assert!(
        !oleksandr.iter().any(|(_, v)| v == "40" || v == "блакитні"),
        "Олександру не приписані факти Марії: {oleksandr:?}"
    );
    assert!(
        !mariia.iter().any(|(_, v)| v == "25" || v == "зелені"),
        "Марії не приписані факти Олександра: {mariia:?}"
    );
}

// (б) одне речення з ДВОМА явними суб'єктами — факти чесно пропускаються,
// нікому не приписуються (25 не потрапляє Марії, 40 — Олександру).
#[test]
fn fqa15_compound_sentence_two_subjects_skipped() {
    let (root, store) = tests_common::tmp_project("fqa15-compound");
    std::fs::create_dir_all(root.join("manuscript")).unwrap();
    add_manual_card(
        &root.to_string_lossy(),
        &ManualCard {
            name: "Олександр".into(),
            aliases: vec![],
            kind: "персонаж".into(),
            notes: "Йому 25 років. Його очі зелені.".into(),
        },
    )
    .unwrap();
    add_manual_card(
        &root.to_string_lossy(),
        &ManualCard {
            name: "Марія".into(),
            aliases: vec![],
            kind: "персонаж".into(),
            notes: "Їй 40 років. Її очі блакитні.".into(),
        },
    )
    .unwrap();
    std::fs::write(
        root.join("manuscript/01.md"),
        "---\ntitle: Г1\n---\n\nОлександр має 25 років, а Марія має 40 років.\n",
    )
    .unwrap();

    pipeline(&store);
    facts::rebuild(&store).unwrap();

    let n: i64 = store
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM extracted_facts WHERE doc_path LIKE 'manuscript%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        n, 0,
        "речення з 2 явними суб'єктами: факти не атрибуціюються нікому"
    );
    let cross: i64 = store
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM extracted_facts
             WHERE (entity='Марія' AND value='25') OR (entity='Олександр' AND value='40')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cross, 0, "крос-атрибуція заборонена");
    // досьє-факти лишаються (канон), але алертів немає — глава не суперечить
    let dossier_facts: i64 = store
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM extracted_facts WHERE doc_path LIKE 'bible/%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(dossier_facts >= 4, "факти обох досьє на місці: {dossier_facts}");
    assert_eq!(fact_alerts(&store, "fact-vs-dossier").len(), 0);
    assert_eq!(fact_alerts(&store, "fact-conflict").len(), 0);
}
