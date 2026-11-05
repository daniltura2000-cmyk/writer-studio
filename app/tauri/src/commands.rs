use skri_core::models;
use skri_core::narrative;
use skri_core::{animation, bible, drift, indexer, presets, store::ProjectStore};

// --- DTO-структури для фронту (core-структури без Serialize не тягнемо) ---

#[derive(serde::Serialize)]
pub struct Direction {
    pub id: &'static str,
    pub title: &'static str,
    /// «якою буде кінцева програма»
    pub outcome: &'static str,
    /// опис із presets::builtin_list (мова + сутності), якщо пресет вбудований
    pub preset_info: Option<String>,
}

#[derive(serde::Serialize)]
pub struct EntityCardDto {
    pub entity_id: i64,
    pub entity_type: String,
    pub canonical: String,
    pub aliases: Vec<String>,
    pub mention_total: usize,
    pub docs: Vec<DocHits>,
    pub assertions: Vec<AssertionDto>,
}

#[derive(serde::Serialize)]
pub struct DocHits {
    pub path: String,
    pub mentions: i64,
}

#[derive(serde::Serialize)]
pub struct AssertionDto {
    pub attribute: String,
    pub value: String,
    pub doc: String,
}

#[derive(serde::Serialize)]
pub struct AlertDto {
    pub id: i64,
    pub kind: String,
    pub entity_id: Option<i64>,
    pub payload: serde_json::Value,
}

#[derive(serde::Serialize)]
pub struct DriftReport {
    pub groups: Vec<drift::DriftGroup>,
    pub alerts: Vec<AlertDto>,
}

#[derive(serde::Serialize)]
pub struct IndexReport {
    pub docs: usize,
    pub words: usize,
    pub reindexed: usize,
    pub candidates: usize,
    pub mentions: usize,
    pub drift_groups: usize,
    pub entities: usize,
    pub assertions: usize,
    pub conflicts: usize,
    pub merged: usize,
}

#[derive(serde::Serialize)]
pub struct EpisodeReport {
    pub check: animation::EpisodeCheck,
    pub mute: Vec<animation::MuteBeat>,
    pub rule_hits: Vec<String>,
}

type CmdResult<T> = Result<T, String>;

fn open(root: &str) -> CmdResult<ProjectStore> {
    ProjectStore::open(std::path::Path::new(root)).map_err(|e| format!("не вдалося відкрити проєкт: {e}"))
}

/// Повний конвеєр індексування (те саме, що `skri index`).
fn full_pipeline(store: &ProjectStore) -> CmdResult<()> {
    store.reindex().map_err(|e| e.to_string())?;
    indexer::scan_project(store).map_err(|e| e.to_string())?;
    bible::sync_bible(store).map_err(|e| e.to_string())?;
    bible::merge_auto_into_manual(store).map_err(|e| e.to_string())?;
    narrative::scan_dialogue(store).map_err(|e| e.to_string())?;
    Ok(())
}

// --- Команди (invoke-інтерфейс) ---

/// 4 напрямки для онбордінгу: як CLI `print_directions` у main.rs,
/// описи збагачені даними presets::builtin_list().
#[tauri::command]
pub fn list_directions() -> Vec<Direction> {
    let builtins: Vec<(String, String)> = presets::builtin_list()
        .into_iter()
        .collect();
    let find = |id: &str| {
        builtins
            .iter()
            .find(|(n, _)| n == id)
            .map(|(_, d)| d.clone())
    };
    vec![
        Direction {
            id: "novelist-uk",
            title: "Романіст / вебновел",
            outcome: "Manuscript + живий Story Bible + name-drift + канон-конфлікти. Кінцева програма: редактор глав, що сам читає рукопис.",
            preset_info: find("novelist-uk"),
        },
        Direction {
            id: "game-narrative-uk",
            title: "Гейм-наратив",
            outcome: "dialogue/ з text-first нодами, bark-таблиці замість Excel, аудитор змінних, ліміти локалізації. Кінцева програма: студія діалогів із валідатором.",
            preset_info: find("game-narrative-uk"),
        },
        Direction {
            id: "animation-uk",
            title: "Сценарист анімації",
            outcome: "Show Bible з правилами світу, валідатор 20-хв епізоду (cold open + 2 turns + hook), мют-тест сцен. Кінцева програма: рев'юер епізодів.",
            preset_info: find("animation-uk"),
        },
        Direction {
            id: "note-taker-uk",
            title: "Нотатник / Zettelkasten",
            outcome: "Атомарні нотатки + композитор рукопису. Кінцева програма: база знань, що зшивається в тексти.",
            preset_info: find("note-taker-uk"),
        },
    ]
}

/// Кнопковий каталог моделей — те саме джерело, що й CLI `skri model menu`.
#[tauri::command]
pub fn model_catalog() -> CmdResult<serde_json::Value> {
    models::catalog_json()
        .map_err(|e| e.to_string())?
        .parse()
        .map_err(|e| format!("каталог моделей: {e}"))
}

/// Швидка перевірка папки: це проєкт skri? Який пресет/мова?
#[tauri::command]
pub fn project_info(root: String) -> CmdResult<serde_json::Value> {
    let store = open(&root)?;
    let settings = store.settings().map_err(|e| e.to_string())?;
    Ok(settings)
}

/// Реіндексація + скан + біблія: зведення як `skri index`.
#[tauri::command]
pub fn run_index(root: String) -> CmdResult<IndexReport> {
    let store = open(&root)?;
    let stats = store.reindex().map_err(|e| e.to_string())?;
    let scan = indexer::scan_project(&store).map_err(|e| e.to_string())?;
    let bib = bible::sync_bible(&store).map_err(|e| e.to_string())?;
    let merged = bible::merge_auto_into_manual(&store).map_err(|e| e.to_string())?;
    Ok(IndexReport {
        docs: stats.docs,
        words: stats.words,
        reindexed: stats.reindexed,
        candidates: scan.candidates,
        mentions: scan.mentions,
        drift_groups: scan.drift_groups,
        entities: bib.entities,
        assertions: bib.assertions,
        conflicts: bib.conflicts,
        merged,
    })
}

/// Картки сутностей Story Bible (після конвеєра індексування).
#[tauri::command]
pub fn run_bible(root: String) -> CmdResult<Vec<EntityCardDto>> {
    let store = open(&root)?;
    full_pipeline(&store)?;
    let cards = bible::cards(&store).map_err(|e| e.to_string())?;
    Ok(cards
        .into_iter()
        .map(|c| EntityCardDto {
            entity_id: c.entity_id,
            entity_type: c.entity_type,
            canonical: c.canonical,
            aliases: c.aliases,
            mention_total: c.mention_total,
            docs: c
                .docs
                .into_iter()
                .map(|(path, mentions)| DocHits { path, mentions })
                .collect(),
            assertions: c
                .assertions
                .into_iter()
                .map(|(attribute, value, doc)| AssertionDto { attribute, value, doc })
                .collect(),
        })
        .collect())
}

/// Drift-групи імен + усі невирішені алерти (drift + канон-конфлікти).
#[tauri::command]
pub fn run_drift(root: String) -> CmdResult<DriftReport> {
    let store = open(&root)?;
    store.reindex().map_err(|e| e.to_string())?;
    indexer::scan_project(&store).map_err(|e| e.to_string())?;
    bible::sync_bible(&store).map_err(|e| e.to_string())?;
    let groups = drift::detect(store.conn()).map_err(|e| e.to_string())?;
    let mut alerts = Vec::new();
    let mut stmt = store
        .conn()
        .prepare("SELECT id, kind, entity_id, payload_json FROM alerts WHERE resolved=0 ORDER BY id")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows.flatten() {
        let (id, kind, entity_id, payload_json) = row;
        let payload = serde_json::from_str(&payload_json).unwrap_or(serde_json::Value::Null);
        alerts.push(AlertDto { id, kind, entity_id, payload });
    }
    Ok(DriftReport { groups, alerts })
}

/// Матриця set/check/unset змінних діалогу (аудитор).
#[tauri::command]
pub fn var_audit(root: String) -> CmdResult<Vec<narrative::VarAudit>> {
    let store = open(&root)?;
    narrative::scan_dialogue(&store).map_err(|e| e.to_string())?;
    narrative::var_audit(&store).map_err(|e| e.to_string())
}

/// Bark-таблиця (тригер/умови/пріоритет/cooldown/варіанти).
#[tauri::command]
pub fn barks(root: String) -> CmdResult<Vec<narrative::Bark>> {
    let store = open(&root)?;
    narrative::scan_dialogue(&store).map_err(|e| e.to_string())?;
    narrative::barks(&store).map_err(|e| e.to_string())
}

/// Валідатор 20-хв епізоду + мют-тест + правила Show Bible.
/// `rel` — шлях епізоду відносно кореня проєкту (напр. "manuscript/ep01.md").
#[tauri::command]
pub fn episode_validate(root: String, rel: String) -> CmdResult<EpisodeReport> {
    let store = open(&root)?;
    let body = indexer::read_body(&store, &rel).map_err(|e| e.to_string())?;
    let fmt = animation::EpisodeFormat::default();
    let check = animation::validate_episode(&body, &fmt);
    let mute = animation::mute_test(&body);
    let rule_hits = animation::check_bible_rules(&store, &body).map_err(|e| e.to_string())?;
    Ok(EpisodeReport { check, mute, rule_hits })
}

/// Зберегти вибір моделі задачі (кнопка в каталозі) → .skri/settings.json.
#[tauri::command]
pub fn set_model(root: String, task: String, model: String, auto: bool) -> CmdResult<()> {
    let store = open(&root)?;
    store.set_model(&task, &model, auto).map_err(|e| e.to_string())
}
