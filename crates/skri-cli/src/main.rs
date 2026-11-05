//! Writer Studio CLI (`skri`): init / index / bible / drift / preset / search.
//! Локально, безкоштовно, українською.

use skri_core::{bible, drift, indexer, models, presets, ProjectStore};
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
         \x20 skri model tasks                                     список задач (кнопок)"
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
