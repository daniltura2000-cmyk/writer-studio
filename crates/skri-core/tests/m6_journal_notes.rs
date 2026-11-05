//! Acceptance-тести M6: Журналістика (pitch CRM + факт-леджер) і Нотатник
//! (беклінки, daily notes, сироти/stale) — §5.11 і §5.10.
//!
//! Критерії приймання:
//! - pitch lifecycle: переходи статусів пишуться в pitch_log, sent_date
//!   фіксується при `pitched`, followup-кандидати — чиста логіка від
//!   (sent_date, followup_after_days, last_touch);
//! - pitch stats: конверсія idea→published і середній час у статусах;
//! - факт-леджер: unverified-звіт перед здачею, verify, content decay;
//! - вікі-лінки: парсинг укр+латиниця з байтовими offsets, резолв по імені
//!   файла, broken-звіт, сироти, stale по mtime;
//! - daily note: створення з шаблону пресету з посиланням на вчорашній,
//!   існуючий файл не перезаписується;
//! - пресети: journalist-uk (fact-unverified-check, pitch-followup,
//!   pitch-card, fact-entry) і note-taker-uk (broken/orphan/stale,
//!   daily-note, topic-note).

use skri_core::{notes, pitch, presets, release, ProjectStore};

const DAY: i64 = 86_400;

fn tmpdir(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("skri-m6-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    base
}

fn make_project(name: &str, preset: &str) -> (std::path::PathBuf, ProjectStore) {
    let root = tmpdir(name);
    let store = ProjectStore::init(&root, preset, "uk").expect("init");
    (root, store)
}

fn epoch(y: i64, m: i64, d: i64, sec: i64) -> i64 {
    release::parse_ymd(&format!("{y:04}-{m:02}-{d:02}")).unwrap() + sec
}

// --- Pitch CRM ---

#[test]
fn pitch_lifecycle_transitions_and_log() {
    let (root, store) = make_project("pitch-lifecycle", "journalist-uk");
    let created = epoch(2026, 9, 15, 0);
    let id = pitch::add(&store, "Розслідування про мости", "Українська правда", Some("2026-12-01"), 14, Some("notes/pitch-mости.md"), created).unwrap();

    let p0 = pitch::get(&store, id).unwrap();
    assert_eq!(p0.status, "idea");
    assert_eq!(p0.due_date, Some(epoch(2026, 12, 1, 0)));
    assert_eq!(p0.followup_after_days, 14);
    assert_eq!(p0.note_doc.as_deref(), Some("notes/pitch-mости.md"));
    assert!(p0.sent_date.is_none());

    // відправка: sent_date фіксується з дати (а не з at, бо передали явно)
    let sent_at = epoch(2026, 10, 1, 0);
    let p1 = pitch::set_status(&store, id, "pitched", Some("2026-10-01"), sent_at).unwrap();
    assert_eq!(p1.status, "pitched");
    assert_eq!(p1.sent_date, Some(sent_at));
    assert_eq!(p1.last_touch, sent_at);

    // перехід далі: last_touch оновлюється
    let p2 = pitch::set_status(&store, id, "assigned", None, sent_at + 5 * DAY).unwrap();
    assert_eq!(p2.status, "assigned");
    assert_eq!(p2.last_touch, sent_at + 5 * DAY);
    assert_eq!(p2.sent_date, Some(sent_at), "sent_date не скидається");

    // журнал переходів: створення + 2 переходи
    let log: Vec<(Option<String>, String, i64)> = {
        let mut stmt = store.conn().prepare(
            "SELECT old_status, new_status, at FROM pitch_log WHERE pitch_id = ?1 ORDER BY at, id",
        ).unwrap();
        stmt.query_map([id], |r| {
            Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?, r.get(2)?))
        }).unwrap().filter_map(|r| r.ok()).collect()
    };
    assert_eq!(log.len(), 3);
    assert_eq!(log[0].1, "idea");
    assert_eq!(log[1], (Some("idea".into()), "pitched".into(), sent_at));
    assert_eq!(log[2], (Some("pitched".into()), "assigned".into(), sent_at + 5 * DAY));

    // невідомий статус — помилка
    assert!(pitch::set_status(&store, id, "зраджено", None, 0).is_err());
    // неіснуючий пітч — помилка
    assert!(pitch::get(&store, 999).is_err());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn pitch_followup_candidates() {
    let (root, store) = make_project("pitch-followup", "journalist-uk");
    let now = epoch(2026, 11, 5, 12);

    // A: sent 30 днів тому, норма 14 → due 16 днів тому, без дотиків → кандидат
    let a = pitch::add(&store, "Пітч A", "Видання A", None, 14, None, now - 40 * DAY).unwrap();
    pitch::set_status(&store, a, "pitched", None, now - 30 * DAY).unwrap();
    // B: sent так само, але доторкнулися 1 день тому (followed_up) → не кандидат
    let b = pitch::add(&store, "Пітч B", "Видання B", None, 14, None, now - 40 * DAY).unwrap();
    pitch::set_status(&store, b, "pitched", None, now - 30 * DAY).unwrap();
    pitch::set_status(&store, b, "followed_up", None, now - DAY).unwrap();
    // C: опублікований (термінальний) → не кандидат
    let c = pitch::add(&store, "Пітч C", "Видання C", None, 14, None, now - 70 * DAY).unwrap();
    pitch::set_status(&store, c, "pitched", None, now - 60 * DAY).unwrap();
    pitch::set_status(&store, c, "published", None, now - 10 * DAY).unwrap();
    // D: ще ідея, не відправлений → не кандидат
    pitch::add(&store, "Пітч D", "Видання D", None, 14, None, now - 40 * DAY).unwrap();

    let cand = pitch::followup_candidates(&store, now).unwrap();
    assert_eq!(cand.iter().map(|p| p.id).collect::<Vec<_>>(), vec![a], "тільки A — кандидат");
    // зміна норми фолоу-апу впливає на кандидатів: 45 днів → due ще не настала
    pitch::set_followup(&store, a, 45).unwrap();
    assert!(pitch::followup_candidates(&store, now).unwrap().is_empty());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn pitch_stats_conversion_and_avg_time() {
    let (root, store) = make_project("pitch-stats", "journalist-uk");
    let now = 20 * DAY; // умовний «зараз»

    let p1 = pitch::add(&store, "P1", "V1", None, 14, None, 0).unwrap();
    pitch::set_status(&store, p1, "pitched", None, 2 * DAY).unwrap();
    pitch::set_status(&store, p1, "published", None, 12 * DAY).unwrap();
    let p2 = pitch::add(&store, "P2", "V2", None, 14, None, 0).unwrap();
    pitch::set_status(&store, p2, "pitched", None, 4 * DAY).unwrap();
    pitch::set_status(&store, p2, "published", None, 14 * DAY).unwrap();
    let p3 = pitch::add(&store, "P3", "V3", None, 14, None, 0).unwrap();
    pitch::set_status(&store, p3, "rejected", None, DAY).unwrap();
    pitch::add(&store, "P4", "V4", None, 14, None, 0).unwrap(); // залишається idea

    let s = pitch::stats(&store, now).unwrap();
    assert_eq!(s.total, 4);
    assert_eq!(s.published, 2);
    assert!((s.conversion - 50.0).abs() < 1e-9, "конверсія idea→published = 2/4");

    let get = |rep: &[(String, f64)], k: &str| rep.iter().find(|(n, _)| n == k).map(|(_, v)| *v).unwrap();
    let getn = |cols: &[(String, usize)], k: &str| cols.iter().find(|(n, _)| n == k).map(|(_, v)| *v).unwrap();
    assert_eq!(getn(&s.by_status, "published"), 2);
    assert_eq!(getn(&s.by_status, "idea"), 1);
    assert_eq!(getn(&s.by_status, "rejected"), 1);
    // idea: [0,2]+[0,4]+[0,1]+[0,now=20] → (2+4+1+20)/4
    assert!((get(&s.avg_days_in_status, "idea") - 27.0 / 4.0).abs() < 1e-9);
    // pitched: [2,12] + [4,14] → 10 днів у середньому
    assert!((get(&s.avg_days_in_status, "pitched") - 10.0).abs() < 1e-9);
    // published: [12,20]+[14,20] → 7
    assert!((get(&s.avg_days_in_status, "published") - 7.0).abs() < 1e-9);

    std::fs::remove_dir_all(&root).ok();
}

// --- Fact ledger ---

#[test]
fn facts_unverified_verify_and_decay() {
    let (root, store) = make_project("facts", "journalist-uk");
    let body = "Міст збудовано 1898 року. Ціна газу зросла на 12%.";
    let char_start = body.find("1898").unwrap() as i64;

    let f1 = pitch::fact_add(&store, "Міст збудовано 1898 року", "ДЕРЖАРХІВ, ф. 12", Some("manuscript/стаття.md"), Some(char_start)).unwrap();
    let f2 = pitch::fact_add(&store, "Ціна газу зросла на 12%", "Держстат, жовтень 2026", None, None).unwrap();

    let un = pitch::unverified_facts(&store).unwrap();
    assert_eq!(un.len(), 2, "обидва неперевірені");
    // прив'язані до рукопису — першими
    assert_eq!(un[0].id, f1);
    assert_eq!(un[0].doc_path.as_deref(), Some("manuscript/стаття.md"));
    assert_eq!(un[0].char_start, Some(char_start));

    let now = epoch(2026, 11, 5, 0);
    pitch::fact_verify(&store, f1, now).unwrap();
    let un2 = pitch::unverified_facts(&store).unwrap();
    assert_eq!(un2.len(), 1);
    assert_eq!(un2[0].id, f2);

    let all = pitch::facts(&store).unwrap();
    assert!(all.iter().find(|f| f.id == f1).unwrap().verified);

    // content decay: верифікація старша за 365 днів → повертається
    let old_at = now - 400 * DAY;
    pitch::fact_verify(&store, f2, old_at).unwrap();
    let decayed = pitch::decayed_facts(&store, now, pitch::DEFAULT_DECAY_DAYS).unwrap();
    assert_eq!(decayed.iter().map(|f| f.id).collect::<Vec<_>>(), vec![f2]);
    // свіжа верифікація — decay чистий
    pitch::fact_verify(&store, f2, now - 10 * DAY).unwrap();
    assert!(pitch::decayed_facts(&store, now, pitch::DEFAULT_DECAY_DAYS).unwrap().is_empty());

    std::fs::remove_dir_all(&root).ok();
}

// --- Нотатник ---

#[test]
fn wiki_links_offsets_uk_and_latin() {
    let body = "Див. [[Київ]] і [[Kyiv Post|джерело]], також [[Київ#Історія]] та [[щонота]] наприкінці.";
    let ls = notes::parse_wiki_links(body);
    assert_eq!(ls.len(), 4);
    let titles: Vec<&str> = ls.iter().map(|l| l.to_title.as_str()).collect();
    assert_eq!(titles, vec!["Київ", "Kyiv Post", "Київ", "щонота"]);
    // offsetи точні: зріз повертає сам лінк (байтові, на char boundary)
    assert_eq!(&body[ls[0].char_start..ls[0].char_end], "[[Київ]]");
    assert_eq!(&body[ls[1].char_start..ls[1].char_end], "[[Kyiv Post|джерело]]");
    assert_eq!(&body[ls[2].char_start..ls[2].char_end], "[[Київ#Історія]]");
    assert_eq!(ls[2].to_title, "Київ", "суфікс #розділ відрізано");
    // offsetи зростають
    assert!(ls.windows(2).all(|w| w[0].char_start < w[1].char_start));
}

#[test]
fn links_resolve_broken_orphans_and_rebuild_idempotent() {
    let (root, store) = make_project("links", "note-taker-uk");
    std::fs::write(root.join("notes").join("kyiv.md"), "---\ntitle: Київ\n---\n\nДив. [[bridge-study]] і [[Kyiv Post]].\n").unwrap();
    std::fs::write(root.join("notes").join("bridge-study.md"), "Аналіз мостів → [[kyiv]].\n").unwrap();
    std::fs::write(root.join("notes").join("Kyiv Post.md"), "Зовнішнє видання.\n").unwrap();
    std::fs::write(root.join("notes").join("isolated.md"), "Теж дивиться на [[kyiv]].\n").unwrap();
    std::fs::write(root.join("notes").join("gaps.md"), "Біла пляма: [[missing-note]].\n").unwrap();
    store.reindex().unwrap();

    let n = notes::rebuild_links(&store).unwrap();
    assert_eq!(n, 5, "2+1+0+1+1 лінків");

    let rows = notes::links(&store).unwrap();
    let resolve = |from: &str, title: &str| -> Option<Option<String>> {
        rows.iter()
            .find(|r| r.from_doc == from && r.to_title == title)
            .map(|r| r.target_doc.clone())
    };
    assert_eq!(resolve("notes/kyiv.md", "bridge-study").unwrap().as_deref(), Some("notes/bridge-study.md"));
    assert_eq!(resolve("notes/kyiv.md", "Kyiv Post").unwrap().as_deref(), Some("notes/Kyiv Post.md"));
    assert_eq!(resolve("notes/bridge-study.md", "kyiv").unwrap().as_deref(), Some("notes/kyiv.md"));
    assert_eq!(resolve("notes/gaps.md", "missing-note").unwrap(), None, "broken → NULL");

    let rep = notes::report(&store).unwrap();
    assert_eq!(rep.total, 5);
    assert_eq!(rep.resolved, 4);
    assert_eq!(rep.broken.len(), 1);
    assert_eq!(rep.broken[0].to_title, "missing-note");
    assert_eq!(rep.inbound.get("notes/kyiv.md"), Some(&2), "вхідні з bridge-study і isolated");
    // сироти: notes-документи без вхідних — gaps.md і isolated.md
    assert_eq!(rep.orphans, vec!["notes/gaps.md".to_string(), "notes/isolated.md".to_string()]);
    assert_eq!(notes::orphans(&store).unwrap(), rep.orphans);

    // ідемпотентність: другий прогін — той самий граф
    notes::rebuild_links(&store).unwrap();
    let rows2 = notes::links(&store).unwrap();
    assert_eq!(rows.len(), rows2.len());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn notes_stale_by_mtime() {
    let (root, store) = make_project("stale", "note-taker-uk");
    std::fs::write(root.join("notes").join("свіжа.md"), "Свіжа нотатка.\n").unwrap();
    std::fs::write(root.join("notes").join("давня.md"), "Давня нотатка.\n").unwrap();
    std::fs::write(root.join("notes").join("найдавніша.md"), "Ще давніша.\n").unwrap();
    // рукопис зі старим mtime — НЕ має потрапити в stale нотатника
    std::fs::write(root.join("manuscript").join("01.md"), "Глава.\n").unwrap();
    store.reindex().unwrap();

    let now = epoch(2026, 11, 5, 0);
    // mtime — похідний індекс; для тесту підставляємо фіксовані значення
    // (один reindex уже відпрацював — далі його не викликаємо, інакше
    // реальні mtime файлів перезапишуть синтетичні)
    store.conn().execute("UPDATE docs SET mtime = ?1 WHERE path LIKE '%свіжа%'", [now - DAY]).unwrap();
    store.conn().execute("UPDATE docs SET mtime = ?1 WHERE path LIKE '%давня%'", [now - 40 * DAY]).unwrap();
    store.conn().execute("UPDATE docs SET mtime = ?1 WHERE path LIKE '%найдавніша%'", [now - 100 * DAY]).unwrap();
    store.conn().execute("UPDATE docs SET mtime = ?1 WHERE kind = 'manuscript'", [now - 90 * DAY]).unwrap();

    let stale = notes::stale(&store, now, 30).unwrap();
    assert_eq!(stale.len(), 2, "свіжа — не stale, manuscript — не в звіті");
    // найстаріші першими
    assert!(stale[0].1 >= stale[1].1);
    assert_eq!(stale[1].1, 40, "вік давньої — 40 днів");
    assert!(stale.iter().all(|(p, _)| p.starts_with("notes/")));
    // вікно 200 днів → порожньо
    assert!(notes::stale(&store, now, 200).unwrap().is_empty());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn daily_note_creates_with_yesterday_link() {
    let (root, store) = make_project("daily", "note-taker-uk");
    let now = epoch(2026, 11, 5, 43_200); // полудень UTC

    // спершу вчорашній щоденник, потім сьогоднішній
    let y = notes::daily(&store, now - DAY).unwrap();
    assert!(y.created);
    assert_eq!(y.path, "notes/2026-11-04.md");

    let d = notes::daily(&store, now).unwrap();
    assert!(d.created);
    assert_eq!(d.path, "notes/2026-11-05.md");
    assert_eq!(d.yesterday, "2026-11-04");
    let body = std::fs::read_to_string(root.join("notes").join("2026-11-05.md")).unwrap();
    assert!(body.contains("[[2026-11-04]]"), "auto-insert посилання на вчорашній: {body}");
    assert!(body.contains("date: 2026-11-05"), "frontmatter з датою (шаблон пресету)");

    // щоденники потрапляють в індекс як notes і беруть участь у графі лінків
    store.reindex().unwrap();
    notes::rebuild_links(&store).unwrap();
    let rows = notes::links(&store).unwrap();
    assert!(rows.iter().any(|r| r.from_doc == "notes/2026-11-05.md" && r.target_doc.as_deref() == Some("notes/2026-11-04.md")));

    // повторний виклик — не перезаписує
    let marker = "МІЙ МАРКЕР";
    std::fs::write(root.join("notes").join("2026-11-05.md"), format!("{marker}\n")).unwrap();
    let d2 = notes::daily(&store, now).unwrap();
    assert!(!d2.created);
    assert_eq!(std::fs::read_to_string(root.join("notes").join("2026-11-05.md")).unwrap(), format!("{marker}\n"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn journal_and_notes_presets_valid() {
    let j = presets::load("journalist-uk").expect("journalist-uk валідний");
    assert!(j.validators.get("fact-unverified-check").copied().unwrap_or(false));
    assert!(j.validators.get("pitch-followup").copied().unwrap_or(false));
    assert!(j.templates.contains_key("pitch-card"));
    assert!(j.templates.contains_key("fact-entry"));

    let n = presets::load("note-taker-uk").expect("note-taker-uk валідний");
    for k in ["broken-links", "orphan-notes", "stale-notes"] {
        assert!(n.validators.get(k).copied().unwrap_or(false), "validator {k}");
    }
    assert!(n.templates.contains_key("daily-note"));
    assert!(n.templates.contains_key("topic-note"));

    // daily-note з пресету note-taker-uk містить плейсхолдери yesterday/tomorrow
    let t = n.templates.get("daily-note").unwrap();
    assert!(t.contains("{{yesterday}}") && t.contains("{{tomorrow}}"));
}
