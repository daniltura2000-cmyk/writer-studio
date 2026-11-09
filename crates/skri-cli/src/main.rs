//! Writer Studio CLI (`skri`): init / index / bible / drift / preset / search.
//! Локально, безкоштовно, українською.

use skri_core::{bible, drift, indexer, models, presets, scrub, style, sync, ProjectStore};
use std::path::{Path, PathBuf};
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        exit(2);
    }
    if let Err(e) = run(&args) {
        eprintln!("✖ помилка: {e}");
        exit(1);
    }
}

fn usage() {
    println!(
        "skri — Writer Studio CLI (локально і безкоштовно)\n\
         \n\
         Використання:\n\
         \x20 skri init <папка> [--preset novelist-uk] [--lang uk]   створити проєкт\n\
         \x20 skri index <папка>                                    проіндексувати рукопис\n\
         \x20 skri bible <папка>                                    показати Story Bible\n\
         \x20 skri drift <папка>                                    алерти консистентності\n\
         \x20 skri search <папка> <запит>                           повнотекстовий пошук\n\
         \x20 skri preset list|show <назва>                         пресети\n\
         \x20 skri model menu <папка>                               вибір моделі по суті задачі (меню-кнопки)\n\
         \x20 skri model auto <папка> <задача>                      автоматичний вибір рекомендованої\n\
         \x20 skri model set <папка> <задача> <модель>              перевизначити кнопкою\n\
         \x20 skri model status <папка>                            поточні призначення\n\
         \x20 skri model tasks                                     список задач (кнопок)\n\
         \x20 skri glossary add <папка> <term> <uk|en...> <uk|en...> [translation] [keep|translate|transliterate]\n\
         \x20 skri glossary list <папка> <src> <tgt>                показати глосарій серії\n\
         \x20 skri glossary series <папка>                          series_wide записи (для наступного тому)\n\
         \x20 skri translate dry <папка> <док.md> <tgt>             промпти перекладу без виклику моделі\n\
         \x20 skri translate segments <папка> <док.md> <tgt>        side-by-side сегменти зі статусами\n\
         \x20 skri translate set <папка> <seg_id> <статус> <текст>  human/approved\n\
         \x20 skri limits <текст> <tgt1:N,tgt2:N...>               перевірка лімітів ДО публікації\n\
         \x20 skri tv <файл.md>                                    аналіз реєстру ти/ви\n\
         \x20 skri stats sheet <папка> <глава>                    stat sheet: стан персонажів у гл. N\n\
         \x20 skri stats diff <папка>                             діф снапшотів після ретроактивної правки\n\
         \x20 skri stats radar <папка>                            stat-error radar (від'ємні hp/mp)\n\
         \x20 skri ink import <папка> <файл.ink|.yarn>             round-trip імпорт діалогу → dialogue/*.md\\n\\
         \x20 skri ink export <папка> <док.md> <файл.ink|.yarn>     експорт markdown → Ink/Yarn\\\\n\\\\
         \x20 skri recap <папка> <від-глави> <до-глави> [--ai]       рекап діапазону глав (rules-first; --ai = локальна Ollama)\\n\\\\
         \x20 skri crosspost pack <папка> <док.md>               пакунки для 5 платформ (.skri/outbound/)\\n\\
         \x20 skri crosspost status <папка> [док.md]              журнал публікацій + stale-перевірка\n\
         \x20 skri crosspost mark <папка> <док> <платформа> posted|failed\n\
         \x20 skri royalty import <папка> <файл.csv>              імпорт CSV KDP/D2D/Patreon\n\
         \x20 skri royalty summary <папка> [--by platform|book|month]\n\
         \x20 skri release kanban <папка>                        Kanban глав по статусах + переходи\n\
         \x20 skri release forecast <папка> [--window 7]           прогноз вичерпання буфера (слів/день)\n\
         \x20 skri release set <папка> <док.md> <статус> [YYYY-MM-DD] [платформи,...]  статус у frontmatter\n\
         \x20 skri retcon publish <папка> <док.md>            зафіксувати published-снапшот глави\n\
         \x20 skri retcon diff <папка> <док.md>                diff чернетки vs останній published\n\
         \x20 skri retcon queue <папка>                         черга retcon-ревізій пізніших глав\n\
         \x20 skri retcon review <папка> <id>                   позначити ревізію розглянутою\n\
         \x20 skri timeline show <папка>                        лінійка днів по главах (календар світу)\n\
         \x20 skri timeline validate <папка>                     стрибки назад, глави без маркерів, конфлікти локацій\n\
         \x20 skri timeline locs <папка>                         локаційний календар: хто де по главах\n\
         \x20 skri pitch add <папка> <назва> [видання] [YYYY-MM-DD] [--followup N] [--note док.md]  пітч у видання\n\
         \x20 skri pitch list <папка>                             реєстр пітчів\n\
         \x20 skri pitch set <папка> <id> <статус> [YYYY-MM-DD]    перехід статусу (sent_date) або followup <N>\n\
         \x20 skri pitch followup <папка>                         кандидати на follow-up лист\n\
         \x20 skri pitch stats <папка>                            конверсія idea→published + час у статусах\n\
         \x20 skri fact add <папка> <твердження> [джерело] [док.md [offset]]  факт у леджер (неперевірений)\n\
         \x20 skri fact list <папка>                              факт-леджер\n\
         \x20 skri fact unverified <папка>                        твердження без verified=1 перед здачею\n\
         \x20 skri fact verify <папка> <id>                       позначити перевіреним\n\
         \x20 skri fact decay <папка> [--days 365]                перевірене, але застаріле (content decay)\n\
         \x20 skri notes links <папка>                            граф [[вікі-лінків]]: беклінки, broken, сироти\n\
         \x20 skri notes orphans <папка>                          нотатки без вхідних лінків\n\
         \x20 skri notes stale <папка> [--days 30]                нотатки без змін N днів\n\
         \x20 skri notes daily <папка>                            щоденник notes/YYYY-MM-DD.md з лінком на вчора\n\
         \x20 skri notes new <папка> <назва>                      тематична нотатка з шаблону topic-note\n\
         \x20 skri scrub build <папка> <вихід.epub> [--pen «Ім'я»] [--light] [--date YYYY-MM-DD]  EPUB зі scrub метаданих\n\
         \x20 skri scrub report <папка>                          останній scrub-звіт (що саме замінено)\n\
         \x20 skri sync status <папка>                          векторні годинники по доках + журнал синку\n\
         \x20 skri sync export <папка> <вихід.json>             пакет обміну (флешка/спільна папка)\n\
         \x20 skri sync import <папка> <пакет.json>             злити пакет у локальний проєкт\n\
         \x20 skri style <папка>                                  перевірки стилю і грамотності (тавтологія, повтори, пробіли, тире)\n\
         \x20 skri facts <папка>                                    факти досьє: витяг з біблії і глав, конфлікти між собою і з каноном\n\
         \x20 skri dossier add <папка> <ім'я> [--aliases а,б] [--kind персонаж] [--notes файл]  ручна картка досьє в біблію\n\
         \x20 skri dossier list <папка>                            картки досьє (Story Bible)\n\
         \x20 skri preset compose <назва> <kind:preset...>         зібрати свій пресет з ассетів"

    );
}

fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match args[0].as_str() {
        "init" => cmd_init(args)?,
        "index" => cmd_index(args)?,
        "bible" => cmd_bible(args)?,
        "drift" => cmd_drift(args)?,
        "search" => cmd_search(args)?,
        "preset" => cmd_preset(&args[1..])?,
        "model" => cmd_model(&args[1..])?,
        "glossary" => cmd_glossary(&args[1..])?,
        "translate" => cmd_translate(&args[1..])?,
        "limits" => cmd_limits(&args[1..])?,
        "tv" => cmd_tv(&args[1..])?,
        "bark" => cmd_bark(&args[1..])?,
        "vars" => cmd_vars(&args[1..])?,
        "graph" => cmd_graph(&args[1..])?,
        "episode" => cmd_episode(&args[1..])?,
        "stats" => cmd_stats(&args[1..])?,
        "ink" => cmd_ink(&args[1..])?,
        "crosspost" => cmd_crosspost(&args[1..])?,
        "royalty" => cmd_royalty(&args[1..])?,
        "release" => cmd_release(&args[1..])?,
        "retcon" => cmd_retcon(&args[1..])?,
        "recap" => cmd_recap(args)?,
        "timeline" => cmd_timeline(&args[1..])?,
        "pitch" => cmd_pitch(&args[1..])?,
        "fact" => cmd_fact(&args[1..])?,
        "notes" => cmd_notes(&args[1..])?,
        "scrub" => cmd_scrub(&args[1..])?,
        "sync" => cmd_sync(&args[1..])?,
        "style" => cmd_style(args)?,
        "facts" => cmd_facts(args)?,
        "dossier" => cmd_dossier(&args[1..])?,
        _ => usage(),
    }
    Ok(())
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn cmd_init(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(
        args.get(1)
            .cloned()
            .unwrap_or_else(|| usage_exit("init: вкажіть папку")),
    );
    let preset = flag(args, "--preset").unwrap_or_else(|| "novelist-uk".into());
    let lang = flag(args, "--lang").unwrap_or_else(|| {
        presets::load(&preset)
            .map(|p| p.language)
            .unwrap_or_else(|_| "uk".into())
    });
    // валідація пресету одразу
    presets::load(&preset)?;
    let store = ProjectStore::init(&dir, &preset, &lang)?;
    println!("✔ проєкт створено: {}", store.root.display());
    println!("  пресет: {preset}, мова: {lang}");
    println!("  покладіть глави у manuscript/, канон — у bible/, нотатки — у notes/");
    print_directions(&store)?;
    Ok(())
}

/// Огляд напрямків: «якою буде кінцева програма» для кожного вбудованого пресету.
/// Користувач бачить шляхи одразу після init і може додати їх будь-коли
/// (`skri preset compose` або повторний init нового проєкту).
fn print_directions(store: &ProjectStore) -> Result<(), Box<dyn std::error::Error>> {
    let paths: &[(&str, &str, &str)] = &[
        ("novelist-uk", "Романіст / вебновел",
         " Manuscript + живий Story Bible + name-drift + канон-конфлікти. Кінцева програма: редактор глав, що сам читає рукопис."),
        ("game-narrative-uk", "Гейм-наратив",
         " dialogue/ з text-first нодами, bark-таблиці замість Excel, аудитор змінних, ліміти локалізації. Кінцева програма: студія діалогів із валідатором."),
        ("animation-uk", "Сценарист анімації",
         " Show Bible з правилами світу, валідатор 20-хв епізоду (cold open + 2 turns + hook), мют-тест сцен. Кінцева програма: рев'юер епізодів."),
        ("note-taker-uk", "Нотатник / Zettelkasten",
         " Атомарні нотатки + композитор рукопису. Кінцева програма: база знань, що зшивається в тексти."),
        ("journalist-uk", "Журналіст / нон-фікшн",
         " Pitch CRM (idea→published + follow-up), факт-леджер з верифікацією, беклінки нотаток. Кінцева програма: матеріал, здатний до публікації без неперевірених тверджень."),
    ];
    println!("\nНапрямки, які можна увімкнути (кожен — пресет з ассетів):");
    for (id, title, desc) in paths {
        let cur = if store.settings()?.get("preset").and_then(|v| v.as_str()) == Some(*id) { " ← ваш" } else { "" };
        println!("  ● {id} — {title}{cur}");
        println!("      {desc}");
    }
    println!("\n  скомбінувати свій: skri preset compose <назва> entities:<пресет> checklists:<пресет> ...");
    Ok(())
}

fn usage_exit(msg: &str) -> ! {
    eprintln!("✖ {msg}");
    std::process::exit(2);
}

fn open_project(args: &[String]) -> ProjectStore {
    let dir = match args.get(1) {
        Some(d) => PathBuf::from(d),
        None => usage_exit("вкажіть папку проєкту"),
    };
    match ProjectStore::open(&dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✖ {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_index(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let store = open_project(args);
    let stats = store.reindex()?;
    println!(
        "✔ документів: {}, слів: {}, переіндексовано: {}",
        stats.docs, stats.words, stats.reindexed
    );
    let scan = indexer::scan_project(&store)?;
    let bib = bible::sync_bible(&store)?;
    let merged = bible::merge_auto_into_manual(&store)?;
    println!(
        "✔ кандидатів-сутностей: {}, згадок: {}, drift-груп: {}",
        scan.candidates, scan.mentions, scan.drift_groups
    );
    println!(
        "✔ біблія: сутностей {}, тверджень {}, конфліктів {} (злито з auto: {})",
        bib.entities, bib.assertions, bib.conflicts, merged
    );
    Ok(())
}

fn cmd_bible(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let store = open_project(args);
    store.reindex()?;
    indexer::scan_project(&store)?;
    bible::sync_bible(&store)?;
    bible::merge_auto_into_manual(&store)?;
    for c in bible::cards(&store)? {
        println!("\n● {} [{}] — {} згадок", c.canonical, c.entity_type, c.mention_total);
        if !c.aliases.is_empty() {
            println!("  варіанти: {}", c.aliases.join(", "));
        }
        if !c.docs.is_empty() {
            let docs: Vec<String> = c
                .docs
                .iter()
                .map(|(p, n)| format!("{p}×{n}"))
                .collect();
            println!("  де: {}", docs.join(", "));
        }
        for (a, v, s) in &c.assertions {
            let mark = if s == "conflict" { "⚠" } else { "•" };
            println!("  {mark} {a}: {v}");
        }
    }
    Ok(())
}

fn cmd_drift(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let store = open_project(args);
    let alerts = drift::unresolved_alerts(store.conn())?;
    if alerts.is_empty() {
        println!("✔ алертів консистентності немає");
        return Ok(());
    }
    println!("⚠ алертів: {}", alerts.len());
    for (id, payload) in alerts {
        let v: serde_json::Value = serde_json::from_str(&payload)?;
        println!("\n[{id}] {}", v.get("message").and_then(|m| m.as_str()).unwrap_or(""));
    }
    Ok(())
}

fn cmd_search(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let store = open_project(args);
    store.reindex()?;
    let query = args
        .get(2)
        .map(|s| s.replace(' ', " OR "))
        .unwrap_or_else(|| usage_exit("search: вкажіть запит"));
    for (path, title, snip) in store.search(&query, 10)? {
        println!("\n● {title} ({path})\n  {snip}");
    }
    Ok(())
}

fn cmd_preset(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match rest.first().map(|s| s.as_str()) {
        Some("list") | None => {
            println!("Вбудовані пресети:");
            for (name, desc) in presets::builtin_list() {
                println!("  ● {name} — {desc}");
            }
        }
        Some("show") => {
            let name = rest
                .get(1)
                .map(String::as_str)
                .unwrap_or_else(|| usage_exit("preset show: вкажіть назву"));
            let p = presets::load(name)?;
            println!("{}", serde_json::to_string_pretty(&p)?);
        }
        Some("compose") => {
            let name = rest
                .get(1)
                .cloned()
                .unwrap_or_else(|| usage_exit("preset compose: вкажіть назву нового пресету"));
            let spec = rest
                .get(2)
                .cloned()
                .unwrap_or_else(|| usage_exit("preset compose: вкажіть ассети (kind:preset, напр. entities:novelist-uk checklists:note-taker-uk)"));
            let p = presets::compose(&spec, &name, None)?;
            let out = format!("{name}.preset.json");
            presets::save_to_json(&p, std::path::Path::new(&out))?;
            println!("✔ композитний пресет «{name}» → {out}");
            println!("  сутності: {}", p.entities.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join(", "));
            println!("  чеклісти: {}", p.checklists.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "));
        }
        other => {
            eprintln!("✖ невідома підкоманда preset: {:?}", other.unwrap_or(""));
            exit(2);
        }
    }
    Ok(())
}

/// Перевірити, що папка виглядає як проєкт (використовується тестами CLI).
pub fn looks_like_project(p: &Path) -> bool {
    p.join(".skri").join("settings.json").exists()
}

// --- Вибір моделі «по суті задачі» (кнопковий каталог) ---

/// Один рядок вводу; None = EOF/помилка (неінтерактивне середовище).
fn ask(prompt: &str) -> Option<String> {
    use std::io::Write;
    print!("{prompt}");
    std::io::stdout().flush().ok()?;
    let mut s = String::new();
    match std::io::stdin().read_line(&mut s) {
        Ok(0) => None,
        Ok(_) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        Err(_) => None,
    }
}

fn open_at(dir: &str) -> ProjectStore {
    match ProjectStore::open(Path::new(dir)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✖ {e}");
            exit(1);
        }
    }
}

fn cmd_model(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let sub = rest.first().map(String::as_str).unwrap_or("menu");
    match sub {
        "tasks" => {
            println!("Кнопки задач (автовибір = перша модель у списку):");
            for t in models::catalog() {
                println!("  ● {} [{}] — {}", t.label, t.id, t.hint);
            }
        }
        "status" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("model status: вкажіть папку"));
            let root = open_at(dir).root;
            println!("Призначення моделей ({}):", dir);
            for (t, cur) in models::assignments(&root)? {
                match cur {
                    Some((m, true)) => println!("  ● {} → {m} [авто]", t.label),
                    Some((m, false)) => println!("  ● {} → {m} [вручну]", t.label),
                    None => println!("  ● {} → (не обрано; за замовчуванням — авто)", t.label),
                }
            }
        }
        "auto" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("model auto: вкажіть папку"));
            let task_id = rest.get(2).map(String::as_str).unwrap_or_else(|| usage_exit("model auto: вкажіть задачу"));
            let best = models::recommend(task_id).unwrap_or_else(|| usage_exit("невідома задача (див. `skri model tasks`)"));
            models::set_model(Path::new(dir), task_id, &best.id, true)?;
            println!("✔ авто: {} → {}", best.label, best.id);
            println!("  {}", best.note);
        }
        "set" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("model set: вкажіть папку"));
            let task_id = rest.get(2).map(String::as_str).unwrap_or_else(|| usage_exit("model set: вкажіть задачу"));
            let model_id = rest.get(3).map(String::as_str).unwrap_or_else(|| usage_exit("model set: вкажіть модель"));
            let t = models::task(task_id).unwrap_or_else(|| usage_exit("невідома задача (див. `skri model tasks`)"));
            if !t.options.iter().any(|o| o.id == model_id) {
                usage_exit("невідома модель для цієї задачі (див. `skri model menu`)");
            }
            models::set_model(Path::new(dir), task_id, model_id, false)?;
            println!("✔ вручну: {task_id} → {model_id}");
        }
        "menu" | _ => {
            let dir = rest
                .iter()
                .skip(1)
                .find(|a| !a.starts_with('-'))
                .map(String::as_str)
                .unwrap_or_else(|| usage_exit("model menu: вкажіть папку"));
            let root = open_at(dir).root;
            println!("Оберіть ЗАДАЧУ (кнопка):");
            let tasks = models::catalog();
            for (i, t) in tasks.iter().enumerate() {
                println!("  {}) {} — {}", i + 1, t.label, t.hint);
            }
            let pick = ask("№ кнопки (Enter = вихід): ").unwrap_or_else(|| exit(0));
            let idx: usize = match pick.parse() {
                Ok(n) if (1..=tasks.len()).contains(&n) => n - 1,
                _ => usage_exit("некоректний номер"),
            };
            let t = &tasks[idx];

            println!("\n«{}» — оберіть МОДЕЛЬ:", t.label);
            for (i, o) in t.options.iter().enumerate() {
                let star = if i == 0 { "★ автоматично" } else { " " };
                let loc = if o.local { "локальна" } else { "хмара (BYOK)" };
                println!("  {}) {} — {} [{loc}]\n       {}", i + 1, o.label, star, o.note);
            }
            let pick2 = ask("№ моделі (Enter = автоматичний вибір): ");
            let chosen = match pick2 {
                None => None, // Enter = авто
                Some(s) => {
                    let n: usize = s.parse().map_err(|_| "некоректний номер")?;
                    if !(1..=t.options.len()).contains(&n) {
                        usage_exit("номер поза списком");
                    }
                    Some(n - 1)
                }
            };
            let (model_id, auto) = match chosen {
                None => (t.options[0].id.clone(), true),
                Some(i) => (t.options[i].id.clone(), false),
            };
            models::set_model(&root, &t.id, &model_id, auto)?;
            if auto {
                println!("✔ збережено (авто): {} → {model_id}", t.label);
            } else {
                println!("✔ збережено (вручну): {} → {model_id}", t.label);
            }
        }
    }
    Ok(())
}

// --- M2: глосарій серії, перекладний пайплайн, ліміти, T-V ---

use skri_core::glossary::{self, GlossaryEntry};
use skri_core::loc::{self, LocalizedString};
use skri_core::translate;
use std::collections::BTreeMap;

fn cmd_glossary(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match rest.first().map(String::as_str) {
        Some("add") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("glossary add: вкажіть папку"));
            let store = open_at(dir);
            let term = rest.get(2).cloned().unwrap_or_else(|| usage_exit("glossary add: вкажіть термін"));
            let src = rest.get(3).cloned().unwrap_or_else(|| usage_exit("glossary add: мова джерела"));
            let tgt = rest.get(4).cloned().unwrap_or_else(|| usage_exit("glossary add: мова перекладу"));
            let translation = rest.get(5).cloned().unwrap_or_default();
            let rule = rest.get(6).cloned().unwrap_or_else(|| {
                if translation.is_empty() { "keep".into() } else { "translate".into() }
            });
            let id = glossary::upsert(&store, &GlossaryEntry {
                id: 0, term, source_lang: src, target_lang: tgt,
                canonical_translation: translation, rule,
                notes: String::new(),
                series_wide: true, // за замовчуванням — для всієї серії
            })?;
            println!("✔ глосарій: запис #{id} збережено (series_wide)");
        }
        Some("list") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("glossary list: вкажіть папку"));
            let src = rest.get(2).map(String::as_str).unwrap_or_else(|| usage_exit("glossary list: мова джерела"));
            let tgt = rest.get(3).map(String::as_str).unwrap_or_else(|| usage_exit("glossary list: мова перекладу"));
            let store = open_at(dir);
            for e in glossary::list(&store, src, tgt)? {
                let mark = if e.series_wide { "⏳ серія" } else { "📄 том" };
                println!("  ● {} → {} [{}] {mark} {}", e.term,
                    if e.canonical_translation.is_empty() { "<НЕВИЗНАЧЕНО>" } else { &e.canonical_translation },
                    e.rule, if e.notes.is_empty() { String::new() } else { format!("[{}]", e.notes) });
            }
        }
        Some("series") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("glossary series: вкажіть папку"));
            let store = open_at(dir);
            let entries = glossary::series_export(&store)?;
            println!("series_wide записів: {} (переноситься в наступний том)", entries.len());
            let json = serde_json::to_string_pretty(&entries)?;
            let out = store.root.join(".skri").join("glossary-series.json");
            std::fs::write(&out, json)?;
            println!("  експорт: {}", out.display());
        }
        _ => usage(),
    }
    Ok(())
}

fn cmd_translate(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let sub = rest.first().map(String::as_str).unwrap_or("segments");
    match sub {
        "dry" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("translate dry: вкажіть папку"));
            let doc = rest.get(2).cloned().unwrap_or_else(|| usage_exit("translate dry: вкажіть документ"));
            let tgt = rest.get(3).cloned().unwrap_or_else(|| usage_exit("translate dry: вкажіть цільову мову"));
            let store = open_at(dir);
            let prompts = translate::dry_run(&store, &doc, "uk", &tgt)?;
            for (i, p) in prompts.iter().enumerate() {
                println!("── сегмент {} ──\n{p}\n", i + 1);
            }
            println!("(dry-run: модель не викликалась; для машинного перекладу потрібна локальна Ollama)");
        }
        "segments" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("translate segments: вкажіть папку"));
            let doc = rest.get(2).cloned().unwrap_or_else(|| usage_exit("translate segments: вкажіть документ"));
            let tgt = rest.get(3).cloned().unwrap_or_else(|| usage_exit("translate segments: вкажіть мову"));
            let store = open_at(dir);
            for s in translate::segments(&store, &doc, &tgt)? {
                println!("── #{} [{}, {}] ──", s.id, s.para_idx, s.status);
                println!("  UK: {}", s.source_text.replace('\n', " "));
                println!("  →: {}", if s.translated_text.is_empty() { "—" } else { &s.translated_text });
            }
        }
        "set" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("translate set: вкажіть папку"));
            let id: i64 = rest.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage_exit("translate set: id сегмента"));
            let status = rest.get(3).cloned().unwrap_or_else(|| usage_exit("translate set: статус (human|approved|mt|draft)"));
            let text = rest.get(4..).map(|v| v.join(" ")).unwrap_or_default();
            let store = open_at(dir);
            translate::set_translation(&store, id, &text, &status)?;
            println!("✔ сегмент #{id} → {status}");
        }
        _ => usage(),
    }
    Ok(())
}

fn cmd_limits(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let text = rest.first().cloned().unwrap_or_else(|| usage_exit("limits: вкажіть текст у лапках"));
    let limits_arg = rest.get(1).cloned().unwrap_or_else(|| usage_exit("limits: вкажіть ліміти виду de:40,ja:25"));
    let mut limits = BTreeMap::new();
    for pair in limits_arg.split(',') {
        let (lang, n) = pair.split_once(':').ok_or("формат ліміту: de:40")?;
        limits.insert(lang.trim().to_string(), n.trim().parse::<usize>()?);
    }
    let s = LocalizedString {
        key: "cli".into(),
        source_text: text,
        source_lang: "uk".into(),
        limits,
        context: BTreeMap::new(),
    };
    for c in loc::check_limits(&s) {
        let flag = if c.over { "⚠ ПЕРЕВИЩЕННЯ" } else { "✔ ок" };
        println!("  [{flag}] {} ×{:.2} → {} / ліміт {} ({})", c.lang, c.factor, c.projected_len, c.limit, c.advice);
    }
    Ok(())
}

fn cmd_tv(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = rest.first().cloned().unwrap_or_else(|| usage_exit("tv: вкажіть файл .md"));
    let raw = std::fs::read_to_string(&path)?;
    let body = skri_core::frontmatter::parse(&raw).body;
    let r = loc::analyze_tv(&body);
    if r.dominant == "none" {
        println!("✔ T-V: прямих звернень ти/ви не знайдено");
        return Ok(());
    }
    let dom = match r.dominant.as_str() {
        "ty" => "«ти» (неформальний)",
        "vy" => "«ви» (ввічливий/множина)",
        _ => "ЗМІШАНИЙ реєстр",
    };
    println!("T-V: {dom} (ти-форм: {}, ви-форм: {})", r.ty_hits, r.vy_hits);
    if r.mixed {
        println!("⚠ Змішування реєстру — модель перекладу обирає ти/ви навмання:");
        for ex in &r.examples {
            println!("   «{ex}»");
        }
    }
    Ok(())
}

// --- M3: гейм-наратив і анімація ---

use skri_core::animation;
use skri_core::inkbridge;
use skri_core::narrative;

fn cmd_bark(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match rest.first().map(String::as_str) {
        Some("add") => {
            // bark add <папка> <тригер> [пріоритет] [cooldown] -- "варіант 1" "варіант 2"...
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("bark add: вкажіть папку"));
            let store = open_at(dir);
            let trigger = rest.get(2).cloned().unwrap_or_else(|| usage_exit("bark add: вкажіть тригер"));
            let priority: i64 = rest.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            let cooldown: i64 = rest.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
            let variants: Vec<String> = rest.iter().skip(5).map(|s| s.trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect();
            let id = narrative::upsert_bark(&store, &narrative::Bark {
                id: 0, trigger: trigger.clone(), conditions: "{}".into(),
                priority, cooldown, variants, entity_id: None,
            })?;
            println!("✔ bark #{id} «{trigger}» збережено (варіантів: {})", narrative::barks(&store)?.iter().find(|b| b.id == id).map(|b| b.variants.len()).unwrap_or(0));
            Ok(())
        }
        Some("list") | Some("csv") | _ => {
            let dir = rest.get(1).cloned().unwrap_or_else(|| usage_exit("bark: вкажіть папку"));
            let store = open_at(&dir);
            if rest.first().map(String::as_str) == Some("csv") {
                let out = store.root.join(".skri").join("barks.csv");
                std::fs::write(&out, narrative::export_barks_csv(&store)?)?;
                println!("✔ CSV: {}", out.display());
            } else {
                for b in narrative::barks(&store)? {
                    println!("● [{}] пріоритет={} cooldown={}с ({} вар.)", b.trigger, b.priority, b.cooldown, b.variants.len());
                    for v in &b.variants {
                        println!("    — {v}");
                    }
                }
            }
            Ok(())
        }
    }
}

fn cmd_vars(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = rest.first().map(String::as_str).unwrap_or_else(|| usage_exit("vars: вкажіть папку"));
    let store = open_at(dir);
    let report = narrative::scan_dialogue(&store)?;
    let audit = narrative::var_audit(&store)?;
    println!("операцій над змінними: {} (dialogue/*.md)", report.ops);
    if audit.is_empty() {
        println!("✔ змінних немає (або dialogue/ порожня)");
        return Ok(());
    }
    println!("\nМатриця set/check (проблемні — зверху):");
    for a in audit {
        let flag = if a.orphan_check {
            "⚠ сирота: check без set"
        } else if a.dead_flag {
            "⚠ мертвий: set без check"
        } else {
            "✔"
        };
        println!("  {flag} {}", a.var_name);
        if !a.sets.is_empty() { println!("      set:   {}", a.sets.join(", ")); }
        if !a.checks.is_empty() { println!("      check: {}", a.checks.join(", ")); }
    }
    Ok(())
}

fn cmd_graph(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = rest.first().map(String::as_str).unwrap_or_else(|| usage_exit("graph: вкажіть папку"));
    let store = open_at(dir);
    let ddir = store.root.join("dialogue");
    if !ddir.exists() {
        println!("диалогових документів немає (папка dialogue/)");
        return Ok(());
    }
    for entry in std::fs::read_dir(&ddir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") { continue; }
        let raw = std::fs::read_to_string(&path)?;
        let body = skri_core::frontmatter::parse(&raw).body;
        let a = narrative::graph_audit(&body);
        println!("\n● {} — нод: {}", path.file_name().map(|s| s.to_string_lossy()).unwrap_or_default(), a.total_nodes);
        if !a.unreachable.is_empty() {
            println!("  ⚠ недосяжні (немає входу): {}", a.unreachable.join(", "));
        }
        if !a.dangling_gotos.is_empty() {
            println!("  ⚠ goto в неіснуючі ноди: {}", a.dangling_gotos.join(", "));
        }
        if a.unreachable.is_empty() && a.dangling_gotos.is_empty() {
            println!("  ✔ граф чистий");
        }
    }
    Ok(())
}

fn cmd_episode(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = rest.first().cloned().unwrap_or_else(|| usage_exit("episode: вкажіть файл епізоду .md"));
    let raw = std::fs::read_to_string(&path)?;
    let body = skri_core::frontmatter::parse(&raw).body;
    let check = animation::validate_episode(&body, &animation::EpisodeFormat::default());
    println!("cold open: {}", check.cold_open.as_deref().unwrap_or("— НІ"));
    println!("turns: {}", check.turns);
    println!("hook: {}", check.hook.as_deref().unwrap_or("— НІ"));
    if check.passed {
        println!("✔ структура 20-хв епізоду валідна");
    } else {
        for i in &check.issues { println!("⚠ {i}"); }
    }
    println!("\nМют-тест (сцена без звуку):");
    for b in animation::mute_test(&body) {
        let flag = if b.verdict.contains("гасне") { "⚠" } else { " " };
        println!("  {flag} {} — дія:{}/діалог:{} ratio:{} — {}", b.scene, b.action_lines, b.dialogue_lines, b.visual_ratio, b.verdict);
    }
    Ok(())
}

// --- M5: LitRPG Progression Engine (event-sourcing статів) ---

use skri_core::stats;

fn cmd_stats(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let sub = rest.first().map(String::as_str).unwrap_or("sheet");
    match sub {
        "sheet" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("stats sheet: вкажіть папку"));
            let chapter: i64 = rest.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage_exit("stats sheet: вкажіть номер глави"));
            let store = open_at(dir);
            store.reindex()?;
            let rep = stats::rebuild(&store)?;
            let rows = stats::sheet(&store, chapter)?;
            if rows.is_empty() {
                println!("(станів на главу {chapter} немає — дельти [stat: ...] не знайдено; дельт у рукописі: {})", rep.deltas);
                return Ok(());
            }
            let mut cur = String::new();
            for r in rows {
                if r.entity != cur {
                    println!("\n● {} — стан на главу {chapter}:", r.entity);
                    cur = r.entity.clone();
                }
                println!("  {}: {}", r.attr, r.value);
            }
            Ok(())
        }
        "diff" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("stats diff: вкажіть папку"));
            let store = open_at(dir);
            store.reindex()?;
            let rep = stats::rebuild(&store)?;
            println!("✔ снапшотів: {}, дельт: {}", rep.snapshots, rep.deltas);
            let alerts = stats::change_alerts(&store)?;
            if alerts.is_empty() {
                println!("✔ ретроактивних змін немає");
            } else {
                println!("⚠ ретроактивних правок: {}", alerts.len());
                for (id, payload) in alerts {
                    let v: serde_json::Value = serde_json::from_str(&payload)?;
                    println!("\n[{id}] {}", v.get("message").and_then(|m| m.as_str()).unwrap_or(""));
                }
            }
            Ok(())
        }
        "radar" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("stats radar: вкажіть папку"));
            let store = open_at(dir);
            store.reindex()?;
            stats::rebuild(&store)?;
            let alerts = stats::radar_alerts(&store)?;
            if alerts.is_empty() {
                println!("✔ stat-error radar чистий (від'ємних hp/mp немає)");
            } else {
                println!("⚠ stat-error: {}", alerts.len());
                for (id, payload) in alerts {
                    let v: serde_json::Value = serde_json::from_str(&payload)?;
                    println!("\n[{id}] {}", v.get("message").and_then(|m| m.as_str()).unwrap_or(""));
                }
            }
            Ok(())
        }
        other => {
            eprintln!("✖ невідома підкоманда stats: {other} (sheet|diff|radar)");
            exit(2);
        }
    }
}

fn cmd_ink(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match rest.first().map(String::as_str) {
        Some("import") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("ink import: вкажіть папку"));
            let src = rest.get(2).cloned().unwrap_or_else(|| usage_exit("ink import: вкажіть файл .ink або .yarn"));
            let store = open_at(dir);
            let rep = inkbridge::import_file(&store, std::path::Path::new(&src))?;
            println!(
                "✔ імпортовано {} → dialogue/ (нод: {}, змінних: {})",
                src, rep.nodes, rep.vars
            );
            if rep.external_change {
                println!("⚠ вхідні правки: файл змінився з останнього імпорту (див. алерти ink-external-change)");
            }
            Ok(())
        }
        Some("export") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("ink export: вкажіть папку"));
            let doc = rest.get(2).cloned().unwrap_or_else(|| usage_exit("ink export: вкажіть документ .md"));
            let out = rest.get(3).cloned().unwrap_or_else(|| usage_exit("ink export: вкажіть файл .ink або .yarn"));
            let store = open_at(dir);
            inkbridge::export_file(&store, &doc, std::path::Path::new(&out))?;
            println!("✔ експортовано {doc} → {out}");
            Ok(())
        }
        other => {
            eprintln!("✖ невідома підкоманда ink: {:?} (import|export)", other);
            exit(2);
        }
    }
}

fn cmd_crosspost(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use skri_core::crosspost;
    match rest.first().map(String::as_str) {
        Some("pack") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("crosspost pack: вкажіть папку"));
            let doc = rest.get(2).cloned().unwrap_or_else(|| usage_exit("crosspost pack: вкажіть документ .md"));
            let store = open_at(dir);
            let rep = crosspost::pack(&store, &doc)?;
            println!(
                "✔ {} → .skri/outbound/{}/ (версія {})",
                rep.doc_path,
                rep.chapter,
                &rep.version_hash[..8.min(rep.version_hash.len())]
            );
            println!("  пакунки: {} платформ ({}; package.json + chapter.html)", rep.platforms.len(), rep.platforms.join(", "));
            if !rep.skipped_posted.is_empty() {
                println!("  ⚠ пропущено (posted): {} — спершу перевідправте або перевірте stale", rep.skipped_posted.join(", "));
            }
            Ok(())
        }
        Some("status") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("crosspost status: вкажіть папку"));
            let doc = rest.get(2).map(String::as_str);
            let store = open_at(dir);
            let stale = crosspost::check_stale(&store)?;
            let rows = crosspost::log_rows(&store, doc)?;
            if rows.is_empty() {
                println!("журнал порожній — спершу `skri crosspost pack`");
                return Ok(());
            }
            println!("{:<28} {:<12} {:<8} {}", "документ", "платформа", "статус", "версія/час");
            for r in &rows {
                let extra = match r.status.as_str() {
                    "posted" => format!("опубл. {}", r.published_at.unwrap_or(0)),
                    "pending" => format!("запак. {}", r.packed_at.unwrap_or(0)),
                    _ => "—".to_string(),
                };
                println!(
                    "{:<28} {:<12} {:<8} {} ({})",
                    r.doc_path,
                    r.platform,
                    r.status,
                    &r.version_hash[..8.min(r.version_hash.len())],
                    extra
                );
            }
            if !stale.stale.is_empty() {
                println!("\n⚠ застарілі публікації (канон змінено після posted):");
                for (doc, platform, old, new) in &stale.stale {
                    println!("  • {doc} @ {platform}: {} → {} (алерт crosspost-stale)", &old[..8], &new[..8]);
                }
            }
            Ok(())
        }
        Some("mark") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("crosspost mark: вкажіть папку"));
            let doc = rest.get(2).cloned().unwrap_or_else(|| usage_exit("crosspost mark: вкажіть документ"));
            let platform = rest.get(3).cloned().unwrap_or_else(|| usage_exit("crosspost mark: вкажіть платформу"));
            let status = rest.get(4).map(String::as_str).unwrap_or_else(|| usage_exit("crosspost mark: вкажіть статус posted|failed|pending"));
            let store = open_at(dir);
            let row = crosspost::mark(&store, &doc, &platform, status)?;
            println!("✔ {} @ {} → {}", row.doc_path, row.platform, row.status);
            if status == "posted" {
                println!("  зафіксована версія {} (published_at = {})", &row.version_hash[..8], row.published_at.unwrap_or(0));
            }
            Ok(())
        }
        other => {
            eprintln!("✖ невідома підкоманда crosspost: {:?} (pack|status|mark)", other);
            exit(2);
        }
    }
}

fn cmd_royalty(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use skri_core::royalty;
    match rest.first().map(String::as_str) {
        Some("import") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("royalty import: вкажіть папку"));
            let file = rest.get(2).cloned().unwrap_or_else(|| usage_exit("royalty import: вкажіть файл .csv"));
            let store = open_at(dir);
            let rep = royalty::import_csv(&store, std::path::Path::new(&file))?;
            println!("✔ {}: {} рядків, пропущено {}", rep.source_file, rep.rows, rep.skipped);
            for (header, field) in &rep.mapping {
                println!("  «{header}» → {field}");
            }
            Ok(())
        }
        Some("summary") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("royalty summary: вкажіть папку"));
            let by_s = flag(rest, "--by").unwrap_or_else(|| "platform".into());
            let by = royalty::By::parse(&by_s)?;
            let store = open_at(dir);
            let (aggs, total_units, total_cents) = royalty::summary(&store, by)?;
            if aggs.is_empty() {
                println!("даних немає — спершу `skri royalty import`");
                return Ok(());
            }
            println!("Роялті по {} (разом: {} одиниць, {}):", by.label(), total_units, royalty::fmt_money(total_cents));
            for a in &aggs {
                println!("  {:<32} {:>6} од.  {:>12}", a.key, a.units, royalty::fmt_money(a.revenue_cents));
            }
            Ok(())
        }
        other => {
            eprintln!("✖ невідома підкоманда royalty: {:?} (import|summary)", other);
            exit(2);
        }
    }
}

fn cmd_release(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use skri_core::release;
    match rest.first().map(String::as_str) {
        Some("kanban") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("release kanban: вкажіть папку"));
            let store = open_at(dir);
            store.reindex()?;
            let rep = release::kanban(&store)?;
            let mut total = 0usize;
            for (status, chapters) in &rep.columns {
                if chapters.is_empty() {
                    continue;
                }
                total += chapters.len();
                println!("\n● {status} ({})", chapters.len());
                for c in chapters {
                    let date = c.pub_date.map(release::fmt_date).unwrap_or_else(|| "—".into());
                    let plat = if c.platforms.is_empty() {
                        String::new()
                    } else {
                        format!("  [{}]", c.platforms.join(", "))
                    };
                    println!("  {:>3}. {} — {} слів, дата {date}{plat}", c.order, c.title, c.word_count);
                }
            }
            if total == 0 {
                println!("(рукопис порожній — глав у manuscript/ немає)");
                return Ok(());
            }
            if rep.transitions.is_empty() {
                println!("\n✔ переходів статусу ще не було");
            } else {
                println!("\nПереходи (останні {}):", rep.transitions.len());
                for t in &rep.transitions {
                    let from = t.from.clone().unwrap_or_else(|| "—".into());
                    println!("  {} {}: {} → {}", release::fmt_date(t.at), t.doc_path, from, t.to);
                }
            }
            Ok(())
        }
        Some("forecast") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("release forecast: вкажіть папку"));
            let window: usize = flag(rest, "--window").and_then(|w| w.parse().ok()).unwrap_or(7);
            let store = open_at(dir);
            store.reindex()?;
            let f = release::forecast(&store, window)?;
            if f.samples == 0 {
                println!("✖ недостатньо історії: потрібно ≥2 глави manuscript/ з різними mtime і словами");
                return Ok(());
            }
            println!(
                "Швидкість (ковзне середнє по останніх {} главах): {:.0} слів/день (точок: {})",
                f.window, f.words_per_day, f.samples
            );
            if f.buffer_chapters == 0 {
                println!("⚠ буфер порожній (0 глав у buffer/scheduled) — пиши наперед!");
                return Ok(());
            }
            println!(
                "Буфер: {} {} ({} слів; {} днів на главу)",
                f.buffer_chapters,
                if f.buffer_chapters == 1 { "глава" } else { "глави" },
                f.buffer_words,
                format!("{:.1}", f.days_per_chapter)
            );
            if let (Some(days), Some(date)) = (f.days_left, f.exhaustion_date.clone()) {
                println!(
                    "Запас вичерпається через ~{:.1} {} — {} (UTC)",
                    days,
                    if (days - 1.0).abs() < 0.05 { "день" } else { "днів" },
                    date
                );
            }
            Ok(())
        }
        Some("set") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("release set: вкажіть папку"));
            let doc = rest.get(2).cloned().unwrap_or_else(|| usage_exit("release set: вкажіть документ .md"));
            let status = rest.get(3).cloned().unwrap_or_else(|| usage_exit("release set: вкажіть статус (draft|edit|buffer|scheduled|published)"));
            let date = rest.get(4).map(String::as_str);
            let platforms: Vec<String> = rest
                .get(5)
                .map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
                .unwrap_or_default();
            let store = open_at(dir);
            release::set_status(&store, &doc, &status, date, &platforms)?;
            store.reindex()?;
            release::rebuild(&store)?;
            println!("✔ {doc}: pub_status = {status} (у frontmatter файлу)");
            Ok(())
        }
        other => {
            eprintln!("✖ невідома підкоманда release: {:?} (kanban|forecast|set)", other);
            exit(2);
        }
    }
}

// --- M4: Recap Generator (специфікація §5.6) ---

use skri_core::recap;

fn cmd_recap(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let _dir = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| usage_exit("recap: вкажіть папку проєкту"));
    let from: usize = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| usage_exit("recap: вкажіть від-главу"));
    let to: usize = args
        .get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| usage_exit("recap: вкажіть до-главу"));
    let use_ai = args.iter().any(|a| a == "--ai");
    let store = open_project(args);
    store.reindex()?;
    indexer::scan_project(&store)?;
    let rep = if use_ai {
        let r = recap::generate_ai(&store, from, to, None)?;
        if let Some(note) = &r.note {
            println!("{note}");
        }
        r
    } else {
        recap::write_rules(&store, from, to)?
    };
    println!(
        "✔ рекап [{}] → {}",
        if rep.used_ai { "AI" } else { "rules-only" },
        rep.path.display()
    );
    Ok(())
}

// --- M5: Retcon Manager (специфікація §5.4) ---

fn cmd_retcon(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use skri_core::retcon;
    match rest.first().map(String::as_str) {
        Some("publish") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("retcon publish: вкажіть папку"));
            let doc = rest.get(2).cloned().unwrap_or_else(|| usage_exit("retcon publish: вкажіть документ .md"));
            let store = open_at(dir);
            let rep = retcon::publish(&store, &doc)?;
            println!(
                "✔ {} → published-версія {} (sha {}, фіксовано {})",
                rep.doc_path,
                rep.version,
                &rep.body_sha256[..8.min(rep.body_sha256.len())],
                rep.published_at
            );
            Ok(())
        }
        Some("diff") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("retcon diff: вкажіть папку"));
            let doc = rest.get(2).cloned().unwrap_or_else(|| usage_exit("retcon diff: вкажіть документ .md"));
            let store = open_at(dir);
            let rep = retcon::diff(&store, &doc)?;
            println!(
                "{}: чернетка проти published-версії {} ({} → {})",
                rep.doc_path,
                rep.version,
                &rep.body_sha256[..8.min(rep.body_sha256.len())],
                &rep.draft_sha256[..8.min(rep.draft_sha256.len())]
            );
            if rep.changes.is_empty() {
                println!("✔ змін немає — чернетка збігається з published");
                return Ok(());
            }
            for c in &rep.changes {
                match c.kind {
                    "added" => println!("  +{:>4} {}", c.draft_line.unwrap_or(0), c.text),
                    _ => println!("  -{:>4} {}", c.published_line.unwrap_or(0), c.text),
                }
            }
            println!("  разом рядків у дельті: {}", rep.changes.len());
            Ok(())
        }
        Some("queue") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("retcon queue: вкажіть папку"));
            let store = open_at(dir);
            let rows = retcon::queue(&store)?;
            if rows.is_empty() {
                println!("✔ черга ревізій порожня — retcon-зобов'язань немає");
                return Ok(());
            }
            println!("{:<4} {:<9} {:<22} {:<22} {}", "id", "статус", "сутність", "джерело →", "згадка");
            for r in &rows {
                println!(
                    "{:<4} {:<9} {:<22} {:<22} {}",
                    r.id,
                    r.status,
                    r.entity_name,
                    r.source_doc,
                    truncate(&r.sentence, 48)
                );
                println!("      ↳ {} @{}", r.affected_doc, r.affected_pos);
            }
            let open = rows.iter().filter(|r| r.status == "open").count();
            println!("\nразом: {} рядків (open: {}, reviewed: {})", rows.len(), open, rows.len() - open);
            Ok(())
        }
        Some("review") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("retcon review: вкажіть папку"));
            let id: i64 = rest.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage_exit("retcon review: вкажіть id рядка"));
            let store = open_at(dir);
            let row = retcon::review(&store, id)?;
            println!("✔ #{} [{}] розглянуто: {} у {} @{}", row.id, row.status, row.entity_name, row.affected_doc, row.affected_pos);
            Ok(())
        }
        other => {
            eprintln!("✖ невідома підкоманда retcon: {:?} (publish|diff|queue|review)", other);
            exit(2);
        }
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max_chars).collect();
        format!("{cut}…")
    }
}

// --- M5: Timeline Engine (календар світу + часові маркери + локації) ---

use skri_core::timeline;

fn cmd_timeline(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let sub = rest.first().map(String::as_str).unwrap_or("show");
    match sub {
        "show" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("timeline show: вкажіть папку"));
            let store = open_at(dir);
            store.reindex()?;
            let rep = timeline::rebuild(&store)?;
            let cal = timeline::load_calendar(&store.root)?;
            println!("Календар: {} (місяців: {}, днів у році: {})", cal.source, cal.months.len(), cal.year_length());
            println!("Точок таймлайну: {}, глав: {}", rep.points, rep.chapters);
            let pts = timeline::points(&store)?;
            let mut cur = String::new();
            for p in &pts {
                if p.doc_path != cur {
                    cur = p.doc_path.clone();
                    println!("\n● {cur} (глава {})", p.ord);
                }
                if p.marker_text.is_empty() {
                    println!("  ● доба {} — початок глави", p.day);
                } else {
                    let tod = if p.time_of_day.is_empty() { String::new() } else { format!(", {}", p.time_of_day) };
                    println!("  ● доба {} ({}{}) — {}", p.day, cal.format_day(p.day), tod, p.marker_text);
                }
            }
            Ok(())
        }
        "validate" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("timeline validate: вкажіть папку"));
            let store = open_at(dir);
            store.reindex()?;
            let rep = timeline::validate(&store)?;
            if rep.is_clean() {
                println!("✔ timeline чистий: стрибків назад і конфліктів локацій немає, усі глави марковані");
                return Ok(());
            }
            if !rep.backwards.is_empty() {
                println!("⚠ стрибків назад у часі: {}", rep.backwards.len());
                for (_, v) in &rep.backwards {
                    println!("  ● {}", v.get("message").and_then(|m| m.as_str()).unwrap_or(""));
                }
            }
            if !rep.conflicts.is_empty() {
                println!("⚠ конфліктів локацій: {}", rep.conflicts.len());
                for (_, v) in &rep.conflicts {
                    println!("  ● {}", v.get("message").and_then(|m| m.as_str()).unwrap_or(""));
                }
            }
            if !rep.missing.is_empty() {
                println!("△ попереджень (глава без жодного time-маркера): {}", rep.missing.len());
                for (doc, ord) in &rep.missing {
                    println!("  ● {doc} (глава {ord}) — додайте [time: ...] або day_start у frontmatter");
                }
            }
            println!("\nзведення: алертів {}, конфліктів {}, попереджень {} (глав: {})",
                rep.backwards.len(), rep.conflicts.len(), rep.missing.len(), rep.chapters);
            Ok(())
        }
        "locs" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("timeline locs: вкажіть папку"));
            let store = open_at(dir);
            store.reindex()?;
            let rep = timeline::rebuild(&store)?;
            let rows = timeline::locs(&store)?;
            if rows.is_empty() {
                println!("(локаційних маркерів [loc: ...] не знайдено; маркерів часу: {})", rep.points);
                return Ok(());
            }
            let mut cur = String::new();
            for l in &rows {
                if l.doc_path != cur {
                    cur = l.doc_path.clone();
                    println!("\n● {cur} (глава {}, доба {}):", l.ord, l.day);
                }
                let mv = if l.moved { " [переміщення]" } else { "" };
                println!("  {} — {}{}", l.entity, l.location, mv);
            }
            let conf = timeline::conflict_alerts(&store)?;
            if !conf.is_empty() {
                println!("\n⚠ конфліктів локацій в одну добу: {}", conf.len());
                for (_, payload) in conf {
                    let v: serde_json::Value = serde_json::from_str(&payload)?;
                    println!("  ● {}", v.get("message").and_then(|m| m.as_str()).unwrap_or(""));
                }
            }
            Ok(())
        }
        other => {
            eprintln!("✖ невідома підкоманда timeline: {other} (show|validate|locs)");
            exit(2);
        }
    }
}

// --- M6: Журналістика — pitch CRM + факт-леджер (специфікація §5.11) ---

fn cmd_pitch(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use skri_core::pitch;
    use skri_core::release;
    match rest.first().map(String::as_str) {
        Some("add") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("pitch add: вкажіть папку"));
            let title = rest.get(2).cloned().unwrap_or_else(|| usage_exit("pitch add: вкажіть назву пітча"));
            let outlet = rest.get(3).cloned().unwrap_or_default();
            // 4-й позиційний аргумент — дедлайн, лише якщо це справді дата
            let due = rest.get(4).map(String::as_str).filter(|s| release::parse_ymd(s).is_some());
            let followup: i64 = flag(rest, "--followup").and_then(|s| s.parse().ok()).unwrap_or(pitch::DEFAULT_FOLLOWUP_DAYS);
            let note = flag(rest, "--note");
            let store = open_at(dir);
            let id = pitch::add(&store, &title, &outlet, due, followup, note.as_deref(), pitch::now_epoch())?;
            println!("✔ пітч #{id} «{title}» → {} (статус idea)", if outlet.is_empty() { "—" } else { &outlet });
            Ok(())
        }
        Some("list") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("pitch list: вкажіть папку"));
            let store = open_at(dir);
            let rows = pitch::list(&store)?;
            if rows.is_empty() {
                println!("пітчів немає — skri pitch add <папка> <назва> [видання]");
                return Ok(());
            }
            println!("{:<4} {:<12} {:<30} {:<18} {:<11} {:<11} {}", "id", "статус", "назва", "видання", "sent", "due", "фолоу");
            for p in &rows {
                println!(
                    "{:<4} {:<12} {:<30} {:<18} {:<11} {:<11} {}д",
                    p.id,
                    p.status,
                    truncate(&p.title, 28),
                    truncate(&p.outlet, 16),
                    p.sent_date.map(release::fmt_date).unwrap_or_else(|| "—".into()),
                    p.due_date.map(release::fmt_date).unwrap_or_else(|| "—".into()),
                    p.followup_after_days,
                );
            }
            println!("\nразом: {} (статуси: {})", rows.len(), pitch::STATUSES.join(" → "));
            Ok(())
        }
        Some("set") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("pitch set: вкажіть папку"));
            let id: i64 = rest.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage_exit("pitch set: вкажіть id пітча"));
            let what = rest.get(3).cloned().unwrap_or_else(|| usage_exit("pitch set: вкажіть статус або followup"));
            let val = rest.get(4).cloned();
            let store = open_at(dir);
            if what == "followup" {
                let days: i64 = val.and_then(|s| s.parse().ok()).unwrap_or_else(|| usage_exit("pitch set followup: вкажіть кількість днів"));
                pitch::set_followup(&store, id, days)?;
                println!("✔ норма фолоу-апу пітча #{id} → {days} днів");
            } else {
                let date = val.filter(|s| release::parse_ymd(s).is_some());
                let p = pitch::set_status(&store, id, &what, date.as_deref(), pitch::now_epoch())?;
                let sent = p.sent_date.map(release::fmt_date).unwrap_or_else(|| "—".into());
                println!("✔ пітч #{id} «{}» → {} (sent: {sent})", p.title, p.status);
            }
            Ok(())
        }
        Some("followup") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("pitch followup: вкажіть папку"));
            let store = open_at(dir);
            let cand = pitch::followup_candidates(&store, pitch::now_epoch())?;
            if cand.is_empty() {
                println!("✔ кандидатів на follow-up немає — усі дотики свіжі");
                return Ok(());
            }
            println!("⚠ кандидатів на follow-up лист: {}", cand.len());
            for p in &cand {
                let sent = p.sent_date.map(release::fmt_date).unwrap_or_default();
                let overdue = (pitch::now_epoch() - (p.sent_date.unwrap_or(0) + p.followup_after_days * 86_400)) / 86_400;
                println!(
                    "  #{id} «{title}» — {outlet}, sent {sent}, норма {n}д, прострочено ~{overdue}д",
                    id = p.id, title = p.title, outlet = p.outlet, n = p.followup_after_days
                );
            }
            Ok(())
        }
        Some("stats") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("pitch stats: вкажіть папку"));
            let store = open_at(dir);
            let s = pitch::stats(&store, pitch::now_epoch())?;
            if s.total == 0 {
                println!("пітчів немає — статистики поки немає");
                return Ok(());
            }
            println!("Пітчів: {}, published: {}, конверсія idea→published: {:.1}%", s.total, s.published, s.conversion);
            let by: Vec<String> = s.by_status.iter().map(|(k, n)| format!("{k}: {n}")).collect();
            println!("  по статусах: {}", by.join(", "));
            if !s.avg_days_in_status.is_empty() {
                println!("  середній час у статусах:");
                for (k, days) in &s.avg_days_in_status {
                    println!("    {k:<14} {:.1} днів", days);
                }
            }
            Ok(())
        }
        other => {
            eprintln!("✖ невідома підкоманда pitch: {other:?} (add|list|set|followup|stats)");
            exit(2);
        }
    }
}

fn cmd_fact(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use skri_core::pitch;
    match rest.first().map(String::as_str) {
        Some("add") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("fact add: вкажіть папку"));
            let claim = rest.get(2).cloned().unwrap_or_else(|| usage_exit("fact add: вкажіть твердження (в лапках)")).trim_matches('"').to_string();
            let source = rest.get(3).cloned().unwrap_or_default();
            let doc = rest.get(4).cloned();
            let offset: Option<i64> = rest.get(5).and_then(|s| s.parse().ok());
            let store = open_at(dir);
            let id = pitch::fact_add(&store, &claim, &source, doc.as_deref(), offset)?;
            println!("✔ факт #{id} додано (неперевірений): {claim}");
            Ok(())
        }
        Some("list") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("fact list: вкажіть папку"));
            let store = open_at(dir);
            let rows = pitch::facts(&store)?;
            if rows.is_empty() {
                println!("факт-леджер порожній — skri fact add <папка> \"<твердження>\" [джерело]");
                return Ok(());
            }
            for f in &rows {
                let mark = if f.verified { "✔" } else { "⚠" };
                println!("{mark} #{id} {claim}", id = f.id, claim = truncate(&f.claim, 60));
                if !f.source_ref.is_empty() {
                    println!("    джерело: {}", f.source_ref);
                }
                if let Some(d) = &f.doc_path {
                    println!("    де: {d} @{}", f.char_start.unwrap_or(0));
                }
            }
            let un = rows.iter().filter(|f| !f.verified).count();
            println!("\nразом: {} (неперевірених: {})", rows.len(), un);
            Ok(())
        }
        Some("unverified") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("fact unverified: вкажіть папку"));
            let store = open_at(dir);
            let rows = pitch::unverified_facts(&store)?;
            if rows.is_empty() {
                println!("✔ усі твердження перевірені — можна здавати");
                return Ok(());
            }
            println!("⚠ неперевірених тверджень: {} (верифікація перед публікацією)", rows.len());
            for f in &rows {
                match (&f.doc_path, f.char_start) {
                    (Some(d), o) => println!("  ⚠ #{} {} — {} @{}", f.id, truncate(&f.claim, 50), d, o.unwrap_or(0)),
                    (None, _) => println!("  ⚠ #{} {} (без прив'язки до рукопису)", f.id, truncate(&f.claim, 50)),
                }
                if !f.source_ref.is_empty() {
                    println!("      джерело: {}", f.source_ref);
                }
            }
            Ok(())
        }
        Some("verify") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("fact verify: вкажіть папку"));
            let id: i64 = rest.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage_exit("fact verify: вкажіть id факта"));
            let store = open_at(dir);
            pitch::fact_verify(&store, id, pitch::now_epoch())?;
            println!("✔ факт #{id} позначено перевіреним");
            Ok(())
        }
        Some("decay") => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("fact decay: вкажіть папку"));
            let days: i64 = flag(rest, "--days").and_then(|s| s.parse().ok()).unwrap_or(pitch::DEFAULT_DECAY_DAYS);
            let store = open_at(dir);
            let rows = pitch::decayed_facts(&store, pitch::now_epoch(), days)?;
            if rows.is_empty() {
                println!("✔ застарілих перевірених тверджень немає (вікно {days} днів)");
                return Ok(());
            }
            println!("⚠ перевірених тверджень старше {days} днів: {} — пере-перевірити", rows.len());
            for f in &rows {
                println!("  ⚠ #{} {} (верифіковано {} днів тому)", f.id, truncate(&f.claim, 50), (pitch::now_epoch() - f.verified_at.unwrap_or(0)) / 86_400);
            }
            Ok(())
        }
        other => {
            eprintln!("✖ невідома підкоманда fact: {other:?} (add|list|unverified|verify|decay)");
            exit(2);
        }
    }
}

// --- M6: Нотатник — беклінки, daily, сироти/stale (специфікація §5.10) ---

fn cmd_notes(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use skri_core::notes;
    use skri_core::pitch;
    let sub = rest.first().map(String::as_str).unwrap_or("links");
    match sub {
        "links" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("notes links: вкажіть папку"));
            let store = open_at(dir);
            store.reindex()?;
            let n = notes::rebuild_links(&store)?;
            if n == 0 {
                println!("[[вікі-лінків]] у notes/ немає");
                return Ok(());
            }
            let rep = notes::report(&store)?;
            println!("Лінків: {} (резолвнуто {}, broken {})", rep.total, rep.resolved, rep.broken.len());
            let mut cur = String::new();
            for r in notes::links(&store)? {
                if r.from_doc != cur {
                    cur = r.from_doc.clone();
                    println!("\n● {cur}");
                }
                match &r.target_doc {
                    Some(t) => println!("  → [[{}]] → {t}", r.to_title),
                    None => println!("  → [[{}]] ⚠ broken", r.to_title),
                }
            }
            if !rep.broken.is_empty() {
                println!("\n⚠ broken-лінків: {}", rep.broken.len());
            }
            if !rep.orphans.is_empty() {
                println!(" сиріт (без вхідних): {} — skri notes orphans", rep.orphans.len());
            }
            Ok(())
        }
        "orphans" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("notes orphans: вкажіть папку"));
            let store = open_at(dir);
            store.reindex()?;
            notes::rebuild_links(&store)?;
            let rows = notes::orphans(&store)?;
            if rows.is_empty() {
                println!("✔ сиріт немає — у кожної нотатки є вхідні лінки");
                return Ok(());
            }
            println!("⚠ нотаток без вхідних лінків: {}", rows.len());
            for p in &rows {
                println!("  ● {p}");
            }
            Ok(())
        }
        "stale" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("notes stale: вкажіть папку"));
            let days: i64 = flag(rest, "--days").and_then(|s| s.parse().ok()).unwrap_or(30);
            let store = open_at(dir);
            store.reindex()?;
            let rows = notes::stale(&store, pitch::now_epoch(), days)?;
            if rows.is_empty() {
                println!("✔ нотаток без змін {days}+ днів немає");
                return Ok(());
            }
            println!("⚠ нотаток без змін {days}+ днів: {}", rows.len());
            for (p, age) in &rows {
                println!("  ● {p} — {age} днів без змін");
            }
            Ok(())
        }
        "daily" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("notes daily: вкажіть папку"));
            let store = open_at(dir);
            let d = notes::daily(&store, pitch::now_epoch())?;
            if d.created {
                store.reindex()?;
                println!("✔ щоденник створено: {} (← [[{}]], → [[{}]])", d.path, d.yesterday, d.tomorrow);
            } else {
                println!("щоденник уже існує: {} (не перезаписано)", d.path);
            }
            Ok(())
        }
        "new" => {
            let dir = rest.get(1).map(String::as_str).unwrap_or_else(|| usage_exit("notes new: вкажіть папку"));
            let title = rest.get(2).cloned().unwrap_or_else(|| usage_exit("notes new: вкажіть назву нотатки"));
            let store = open_at(dir);
            let rel = notes::new_note(&store, &title, pitch::now_epoch())?;
            store.reindex()?;
            println!("✔ нотатку створено: {rel}");
            Ok(())
        }
        other => {
            eprintln!("✖ невідома підкоманда notes: {other} (links|orphans|stale|daily|new)");
            exit(2);
        }
    }
}

/// `skri scrub build <папка> <вихід.epub> [--pen «Ash Thornwood»] [--light] [--date YYYY-MM-DD]`
/// `skri scrub report <папка>` — останній scrub-звіт.
fn cmd_scrub(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match rest.first().map(String::as_str) {
        Some("build") => {
            let dir = rest
                .get(1)
                .map(String::as_str)
                .unwrap_or_else(|| usage_exit("scrub build: вкажіть папку проєкту"));
            let out = rest
                .get(2)
                .cloned()
                .unwrap_or_else(|| usage_exit("scrub build: вкажіть вихідний файл .epub"));
            if !out.ends_with(".epub") {
                usage_exit("scrub build: вихідний файл мусить мати розширення .epub");
            }
            let store = open_at(dir);
            let mode = if rest.iter().any(|a| a == "--light") {
                scrub::ScrubMode::Light
            } else {
                scrub::ScrubMode::Full
            };
            let opts = scrub::ScrubOptions {
                mode,
                pen_name: flag(rest, "--pen"),
                date: flag(rest, "--date"),
            };
            let rep = scrub::build(&store, Path::new(&out), &opts)?;
            println!(
                "✔ EPUB зібрано: {} ({} глав, sha256 {})",
                rep.output,
                rep.chapters.len(),
                &rep.sha256[..16]
            );
            println!(
                "  режим scrub: {}; pen name: «{}»; назва: «{}»; дата: {}",
                rep.mode, rep.pen_name, rep.title, rep.date
            );
            if rep.changes.is_empty() {
                println!("  замін метаданих немає");
            } else {
                println!("  scrub-звіт:");
                for c in &rep.changes {
                    println!("    • {c}");
                }
            }
            println!("  повний звіт: skri scrub report {}", dir);
            Ok(())
        }
        Some("report") => {
            let dir = rest
                .get(1)
                .map(String::as_str)
                .unwrap_or_else(|| usage_exit("scrub report: вкажіть папку проєкту"));
            let store = open_at(dir);
            match scrub::last_report(&store)? {
                Some(rep) => {
                    println!(
                        "Останній scrub-звіт ({}, EPUB {}):",
                        rep.mode, rep.output
                    );
                    println!("  назва: «{}», pen name: «{}», мова: {}, дата: {}", rep.title, rep.pen_name, rep.language, rep.date);
                    println!("  глав: {}, sha256: {}", rep.chapters.len(), rep.sha256);
                    if rep.changes.is_empty() {
                        println!("  замін метаданих немає");
                    } else {
                        println!("  заміни/видалення:");
                        for c in &rep.changes {
                            println!("    • {c}");
                        }
                    }
                    Ok(())
                }
                None => {
                    println!("звіту ще немає — спершу: skri scrub build <папка> <вихід.epub>");
                    Ok(())
                }
            }
        }
        other => {
            eprintln!("✖ невідома підкоманда scrub: {:?} (build|report)", other.unwrap_or(""));
            exit(2);
        }
    }
}

fn cmd_sync(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match rest.first().map(String::as_str) {
        Some("status") | None => {
            let dir = rest
                .get(1)
                .map(String::as_str)
                .unwrap_or_else(|| usage_exit("sync status: вкажіть папку проєкту"));
            let store = open_at(dir);
            let st = sync::status(&store)?;
            println!("Пристрій: {}", st.device_id);
            println!("\nВекторні годинники ({} доків):", st.docs.len());
            for d in &st.docs {
                if d.clock.is_empty() {
                    println!("  {} → ∅ (ще не зберігалося через save_doc)", d.path);
                } else {
                    println!("  {} → {}", d.path, d.clock);
                }
            }
            println!("\nОстанні події журналу ({}):", st.log.len());
            if st.log.is_empty() {
                println!("  (журнал порожній — синку ще не було)");
            }
            for r in &st.log {
                println!("  ● {} {} {} — {}", r.ts, r.device, r.doc_path, r.action);
            }
            let conf = sync::conflicts(&store)?;
            if !conf.is_empty() {
                println!("\n⚠ нерозв'язаних конфліктів: {}", conf.len());
                for c in &conf {
                    println!(
                        "  ● {} рядок {} (локальний {}) vs рядок {} ({})",
                        c.doc_path,
                        c.local_line.map(|l| l.to_string()).unwrap_or("?".into()),
                        c.local_device,
                        c.remote_line.map(|l| l.to_string()).unwrap_or("?".into()),
                        c.remote_device
                    );
                }
                println!("  маркери <<<<<<</>>>>>>> у файлах — розв'яжіть вручну і збережіть");
            }
            Ok(())
        }
        Some("export") => {
            let dir = rest
                .get(1)
                .map(String::as_str)
                .unwrap_or_else(|| usage_exit("sync export: вкажіть папку проєкту"));
            let out = rest
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(|| usage_exit("sync export: вкажіть вихідний .json"));
            let store = open_at(dir);
            let rep = sync::export_packet(&store, &out)?;
            println!(
                "✔ пакет {} (пристрій {}, доків: {})",
                rep.out_path.display(),
                rep.device,
                rep.docs
            );
            for p in &rep.paths {
                println!("  • {p}");
            }
            Ok(())
        }
        Some("import") => {
            let dir = rest
                .get(1)
                .map(String::as_str)
                .unwrap_or_else(|| usage_exit("sync import: вкажіть папку проєкту"));
            let packet = rest
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(|| usage_exit("sync import: вкажіть пакет.json"));
            let store = open_at(dir);
            let rep = sync::import_packet(&store, &packet)?;
            println!(
                "✔ пакет від пристрою {}: fast-forward {}, автозлиттями {}, конфліктів {}, пропущено {}",
                rep.device, rep.fast_forward, rep.merged, rep.conflicts, rep.skipped
            );
            for d in &rep.conflict_docs {
                println!("  ⚠ конфлікт у {d} — маркери у файлі, алерт створено");
            }
            Ok(())
        }
        other => {
            eprintln!(
                "✖ невідома підкоманда sync: {:?} (status|export|import)",
                other.unwrap_or("")
            );
            exit(2);
        }
    }
}

// --- M10: перевірки стилю і ручні досьє (bible) ---

fn cmd_style(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use skri_core::alerts_meta;
    let dir = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| usage_exit("style: вкажіть папку проєкту"));
    let store = open_at(&dir);
    store.reindex()?;
    let alerts = style::run_style_checks(&dir)?;
    if alerts.is_empty() {
        println!("✔ перевірки стилю чисті: тавтологій, настирливих повторів, подвійних пробілів і дефісів-сиріт немає");
        return Ok(());
    }
    println!("⚠ знахідок стилю: {}", alerts.len());
    for a in &alerts {
        let meta = alerts_meta::catalog()
            .into_iter()
            .find(|m| m.kind == a.kind);
        let title = meta
            .as_ref()
            .map(|m| m.title_uk.clone())
            .unwrap_or_else(|| a.kind.clone());
        let severity = meta.as_ref().map(|m| m.severity).unwrap_or("tip");
        println!("\n[{severity}] {title} (#{id})", id = a.id);
        if let Some(m) = a.payload.get("message").and_then(|m| m.as_str()) {
            println!("  ● {m}");
        }
        if let Some(d) = a.payload.get("doc").and_then(|m| m.as_str()) {
            let pos = a.payload.get("char_start").and_then(|c| c.as_i64()).unwrap_or(0);
            println!("    де: {d} @offset {pos}");
        }
        if let Some(s) = a.payload.get("sentence").and_then(|m| m.as_str()) {
            if !s.is_empty() {
                println!("    речення: {}", truncate(s, 90));
            }
        }
    }
    let problems = alerts.len();
    println!("\nзведення: {} знахідок (style-tautology = problem, решта — поради)", problems);
    Ok(())
}

fn cmd_dossier(rest: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use skri_core::bible::ManualCard;
    match rest.first().map(String::as_str) {
        Some("add") => {
            let dir = rest
                .get(1)
                .cloned()
                .unwrap_or_else(|| usage_exit("dossier add: вкажіть папку"));
            let name = rest
                .get(2)
                .cloned()
                .unwrap_or_else(|| usage_exit("dossier add: вкажіть ім'я сутності"));
            let aliases: Vec<String> = flag(rest, "--aliases")
                .map(|s| {
                    s.split(',')
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let kind = flag(rest, "--kind").unwrap_or_else(|| "персонаж".into());
            let notes = match flag(rest, "--notes") {
                Some(file) => std::fs::read_to_string(&file)
                    .unwrap_or_else(|e| usage_exit(&format!("dossier add: не вдалося прочитати --notes {file}: {e}"))),
                None => String::new(),
            };
            let card = ManualCard {
                name,
                aliases,
                kind: kind.clone(),
                notes,
            };
            bible::add_manual_card(&dir, &card)?;
            // одразу показати в біблії
            let store = open_at(&dir);
            let rep = bible::sync_bible(&store)?;
            println!(
                "✔ досьє «{}» → bible/ (type: {kind}); біблія: сутностей {}, тверджень {}",
                card.name, rep.entities, rep.assertions
            );
            Ok(())
        }
        Some("list") => {
            let dir = rest
                .get(1)
                .map(String::as_str)
                .unwrap_or_else(|| usage_exit("dossier list: вкажіть папку"));
            let store = open_at(dir);
            let cards = bible::cards(&store)?;
            if cards.is_empty() {
                println!("досьє порожнє — skri dossier add <папка> <ім'я>");
                return Ok(());
            }
            for c in cards {
                println!(
                    "● {} [{}] — {} згадок",
                    c.canonical, c.entity_type, c.mention_total
                );
                if !c.aliases.is_empty() {
                    println!("  варіанти: {}", c.aliases.join(", "));
                }
                for (attr, value, status) in &c.assertions {
                    let mark = if status == "conflict" { "⚠" } else { "•" };
                    println!("  {mark} {attr}: {value}");
                }
            }
            Ok(())
        }
        other => {
            eprintln!("✖ невідома підкоманда dossier: {:?} (add|list)", other.unwrap_or(""));
            exit(2);
        }
    }
}

/// `skri facts <папка>` — рушій фактів досьє (QA-02): звіт витягнутих
/// фактів + конфлікти між главами і проти канону досьє.
fn cmd_facts(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use skri_core::{alerts_meta, facts};
    let dir = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| usage_exit("facts: вкажіть папку проєкту"));
    let store = open_at(&dir);
    // конвеєр індексування (передумова rebuild): entities/aliases у БД
    store.reindex()?;
    indexer::scan_project(&store)?;
    bible::sync_bible(&store)?;
    bible::merge_auto_into_manual(&store)?;
    let rep = facts::rebuild(&store)?;

    println!("✔ витягнуто фактів: {} (конфліктів між главами: {}, проти досьє: {})", rep.facts, rep.conflicts, rep.vs_dossier);
    let mut stmt = store.conn().prepare(
        "SELECT doc_path, char_start, entity, kind, value FROM extracted_facts
         ORDER BY entity, doc_path, char_start",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    for (doc, start, entity, kind, value) in rows.flatten() {
        let src = if doc.starts_with("bible/") { "канон" } else { "глава" };
        println!(
            "● {} — {}: «{}»  [{src} {doc} @offset {start}]",
            entity,
            facts::kind_label(&kind),
            value
        );
    }

    // fact-алерти з людськими поясненнями
    let mut stmt = store.conn().prepare(
        "SELECT id, kind, payload_json FROM alerts
         WHERE kind IN ('fact-conflict','fact-vs-dossier') AND resolved=0 ORDER BY id",
    )?;
    let alerts: Vec<(i64, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();
    if !alerts.is_empty() {
        println!("\n⚠ факт-конфліктів: {}", alerts.len());
        for (id, kind, payload_json) in &alerts {
            let meta = alerts_meta::catalog().into_iter().find(|m| m.kind == *kind);
            let title = meta
                .as_ref()
                .map(|m| m.title_uk.clone())
                .unwrap_or_else(|| kind.clone());
            println!("\n[{title} #{id}]");
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(payload_json) {
                if let Some(m) = payload.get("message").and_then(|m| m.as_str()) {
                    println!("  ● {m}");
                }
                if let (Some(a), Some(b)) = (
                    payload.get("doc_a").and_then(|m| m.as_str()),
                    payload.get("doc_b").and_then(|m| m.as_str()),
                ) {
                    println!("    де: {a} ↔ {b}");
                }
            }
        }
    }
    Ok(())
}
