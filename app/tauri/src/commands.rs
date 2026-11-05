use skri_core::models;
use skri_core::narrative;
use skri_core::{
    animation, bible, crosspost, drift, frontmatter, indexer, presets, release, stats, timeline,
    store::ProjectStore,
};

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

// --- Редактор: документи manuscript/ ---

#[derive(serde::Serialize)]
pub struct DocDto {
    pub path: String,
    pub title: String,
    pub word_count: i64,
    pub mtime: i64,
}

/// Список глав manuscript/ (зі свіжою реіндексацією — індекс похідний від файлів).
#[tauri::command]
pub fn list_docs(root: String) -> CmdResult<Vec<DocDto>> {
    let store = open(&root)?;
    store.reindex().map_err(|e| e.to_string())?;
    let mut stmt = store
        .conn()
        .prepare(
            "SELECT path, COALESCE(title,''), COALESCE(word_count,0), COALESCE(mtime,0)
             FROM docs WHERE kind='manuscript' AND path LIKE 'manuscript/%' ORDER BY path",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(DocDto {
                path: r.get(0)?,
                title: {
                    let t: String = r.get(1)?;
                    if t.is_empty() {
                        r.get::<_, String>(0)?
                            .rsplit('/')
                            .next()
                            .unwrap_or("")
                            .trim_end_matches(".md")
                            .to_string()
                    } else {
                        t
                    }
                },
                word_count: r.get(2)?,
                mtime: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

#[derive(serde::Serialize)]
pub struct DocBody {
    pub path: String,
    pub title: String,
    /// тіло БЕЗ frontmatter
    pub body: String,
    pub has_frontmatter: bool,
    /// пари ключ=значення з frontmatter (тільки для показу)
    pub meta: Vec<(String, String)>,
}

/// Прочитати главу: тіло без frontmatter + метадані для заголовка.
#[tauri::command]
pub fn read_doc(root: String, rel: String) -> CmdResult<DocBody> {
    let store = open(&root)?;
    let rel = rel.replace('\\', "/");
    let raw = std::fs::read_to_string(store.root.join(&rel))
        .map_err(|e| format!("не вдалося прочитати {rel}: {e}"))?;
    let parsed = frontmatter::parse(&raw);
    let title = parsed
        .meta
        .get("title")
        .cloned()
        .unwrap_or_else(|| {
            rel.rsplit('/')
                .next()
                .unwrap_or(&rel)
                .trim_end_matches(".md")
                .to_string()
        });
    Ok(DocBody {
        path: rel,
        title,
        body: parsed.body,
        has_frontmatter: !parsed.meta.is_empty(),
        meta: parsed.meta.into_iter().collect(),
    })
}

/// Зберегти тіло глави: frontmatter ПЕРЕЗАПИСУЄТЬСЯ БЕЗ ЗМІН (хірургічний
/// сплайсинг — перечитуємо файл на момент збереження і не чіпаємо header-блок).
#[tauri::command]
pub fn write_doc(root: String, rel: String, body: String) -> CmdResult<()> {
    let store = open(&root)?;
    let rel = rel.replace('\\', "/");
    let path = store.root.join(&rel);
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("не вдалося прочитати {rel}: {e}"))?;
    let parsed = frontmatter::parse(&raw);
    let mut out = String::with_capacity(parsed.body_start + body.len());
    out.push_str(&raw[..parsed.body_start]);
    out.push_str(&body);
    std::fs::write(&path, out).map_err(|e| format!("не вдалося записати {rel}: {e}"))?;
    // індекс = відображення файлів: оновлюємо docs/FTS одразу після збереження
    store.reindex().map_err(|e| e.to_string())?;
    Ok(())
}

// --- Онбордінг: пресети і створення проєкту ---

#[derive(serde::Serialize)]
pub struct PresetDto {
    pub id: String,
    pub desc: String,
}

/// Вбудовані пресети (для вибору при створенні проєкту).
#[tauri::command]
pub fn list_presets() -> Vec<PresetDto> {
    presets::builtin_list()
        .into_iter()
        .map(|(id, desc)| PresetDto { id, desc })
        .collect()
}

/// Створити новий проєкт у папці (пресет + мова uk).
#[tauri::command]
pub fn init_project(root: String, preset: String) -> CmdResult<()> {
    ProjectStore::init(std::path::Path::new(&root), &preset, "uk")
        .map(|_| ())
        .map_err(|e| format!("не вдалося створити проєкт: {e}"))
}

// --- Реліз: kanban + прогноз буфера ---

#[derive(serde::Serialize)]
pub struct ChapterDto {
    pub doc_path: String,
    pub title: String,
    pub order: i64,
    pub status: String,
    pub pub_date: Option<i64>,
    pub platforms: Vec<String>,
    pub word_count: i64,
}

#[derive(serde::Serialize)]
pub struct ReleaseColumnDto {
    pub status: String,
    pub chapters: Vec<ChapterDto>,
}

#[derive(serde::Serialize)]
pub struct ForecastDto {
    pub window: usize,
    pub words_per_day: f64,
    pub samples: usize,
    pub buffer_chapters: usize,
    pub buffer_words: i64,
    pub days_per_chapter: f64,
    pub days_left: Option<f64>,
    pub exhaustion_date: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ReleaseBoard {
    pub columns: Vec<ReleaseColumnDto>,
    pub forecast: ForecastDto,
}

/// Kanban-колонки + прогноз буфера (release::kanban + release::forecast).
#[tauri::command]
pub fn release_board(root: String) -> CmdResult<ReleaseBoard> {
    let store = open(&root)?;
    let kan = release::kanban(&store).map_err(|e| e.to_string())?;
    let fc = release::forecast(&store, 7).map_err(|e| e.to_string())?;
    Ok(ReleaseBoard {
        columns: kan
            .columns
            .into_iter()
            .map(|(status, chapters)| ReleaseColumnDto {
                status,
                chapters: chapters
                    .into_iter()
                    .map(|c| ChapterDto {
                        doc_path: c.doc_path,
                        title: c.title,
                        order: c.order,
                        status: c.status,
                        pub_date: c.pub_date,
                        platforms: c.platforms,
                        word_count: c.word_count,
                    })
                    .collect(),
            })
            .collect(),
        forecast: ForecastDto {
            window: fc.window,
            words_per_day: fc.words_per_day,
            samples: fc.samples,
            buffer_chapters: fc.buffer_chapters,
            buffer_words: fc.buffer_words,
            days_per_chapter: fc.days_per_chapter,
            days_left: fc.days_left,
            exhaustion_date: fc.exhaustion_date,
        },
    })
}

/// Перехід статусу глави (пише pub_status прямо у frontmatter файлу).
#[tauri::command]
pub fn release_set_status(
    root: String,
    rel: String,
    status: String,
    pub_date: Option<String>,
) -> CmdResult<()> {
    let store = open(&root)?;
    release::set_status(&store, &rel, &status, pub_date.as_deref(), &[])
        .map_err(|e| e.to_string())
}

// --- «Перевірити»: конвеєр indexer+bible+drift+валідатори ---

#[derive(serde::Serialize)]
pub struct CheckReport {
    pub docs: usize,
    pub words: usize,
    pub candidates: usize,
    pub mentions: usize,
    pub entities: usize,
    pub assertions: usize,
    pub conflicts: usize,
    pub merged: usize,
    pub drift_groups: usize,
    /// глави без жодного time-маркера (попередження, не алерт)
    pub timeline_missing: Vec<(String, i64)>,
    /// УСІ невирішені алерти з усіх валідаторів (drift, канон, stats, timeline, crosspost, ...)
    pub alerts: Vec<AlertDto>,
}

/// Повний прогін перевірки: реіндекс → скан → біблія → merge → діалоги →
/// drift → stats → timeline → crosspost; повертає зведення і всі алерти.
#[tauri::command]
pub fn run_checks(root: String) -> CmdResult<CheckReport> {
    let store = open(&root)?;
    let st = store.reindex().map_err(|e| e.to_string())?;
    let scan = indexer::scan_project(&store).map_err(|e| e.to_string())?;
    let bib = bible::sync_bible(&store).map_err(|e| e.to_string())?;
    let merged = bible::merge_auto_into_manual(&store).map_err(|e| e.to_string())?;
    narrative::scan_dialogue(&store).map_err(|e| e.to_string())?;
    let groups = drift::detect(store.conn()).map_err(|e| e.to_string())?;
    stats::rebuild(&store).map_err(|e| e.to_string())?;
    let tv = timeline::validate(&store).map_err(|e| e.to_string())?;
    let _stale = crosspost::check_stale(&store).map_err(|e| e.to_string())?;

    let mut alerts = Vec::new();
    let mut stmt = store
        .conn()
        .prepare("SELECT id, kind, entity_id, payload_json FROM alerts WHERE resolved=0 ORDER BY kind, id")
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
    Ok(CheckReport {
        docs: st.docs,
        words: st.words,
        candidates: scan.candidates,
        mentions: scan.mentions,
        entities: bib.entities,
        assertions: bib.assertions,
        conflicts: bib.conflicts,
        merged,
        drift_groups: groups.len(),
        timeline_missing: tv.missing,
        alerts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_root(tag: &str) -> String {
        let dir = std::env::temp_dir().join(format!("skri-tauri-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.to_string_lossy().to_string()
    }

    /// write_doc зберігає тіло і НЕ чіпає frontmatter (байт-в-байт).
    #[test]
    fn write_doc_preserves_frontmatter() {
        let root = tmp_root("write");
        let _store = ProjectStore::init(std::path::Path::new(&root), "novelist-uk", "uk").expect("init");
        let raw = "---\ntitle: Розділ 1\npub_status: draft\n---\n\nСтарий текст.";
        std::fs::create_dir_all(std::path::Path::new(&root).join("manuscript")).unwrap();
        std::fs::write(std::path::Path::new(&root).join("manuscript/01.md"), raw).unwrap();

        write_doc(root.clone(), "manuscript/01.md".into(), "\nНовий текст глави.\n".into()).unwrap();

        let after = std::fs::read_to_string(std::path::Path::new(&root).join("manuscript/01.md")).unwrap();
        assert!(after.starts_with("---\ntitle: Розділ 1\npub_status: draft\n---\n"), "frontmatter змінено: {after:?}");
        assert!(after.contains("Новий текст глави."));
        assert!(!after.contains("Старий текст."));

        // read_doc віддає тіло без frontmatter і метадані
        let d = read_doc(root, "manuscript/01.md".into()).unwrap();
        assert_eq!(d.title, "Розділ 1");
        assert!(d.body.contains("Новий текст глави."));
        assert!(!d.body.contains("pub_status"));
        assert!(d.has_frontmatter);
    }

    /// list_docs бачить глави manuscript/ після реіндексації.
    #[test]
    fn list_docs_finds_manuscript() {
        let root = tmp_root("list");
        let _store = ProjectStore::init(std::path::Path::new(&root), "novelist-uk", "uk").expect("init");
        std::fs::create_dir_all(std::path::Path::new(&root).join("manuscript")).unwrap();
        std::fs::write(
            std::path::Path::new(&root).join("manuscript/01.md"),
            "---\ntitle: Глава один\n---\n\nСлово слово слово.",
        )
        .unwrap();
        let docs = list_docs(root).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].path, "manuscript/01.md");
        assert_eq!(docs[0].title, "Глава один");
        assert_eq!(docs[0].word_count, 3);
    }
}
