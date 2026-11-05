//! Автовибір локальної моделі «по суті задачі» (кнопковий каталог).
//!
//! Користувач не мусить знати назви моделей: він тисне кнопку задачі
//! («переклад», «транскрипція»…), і система сама підбирає найкращу ЛОКАЛЬНУ
//! модель. Перший варіант у списку = автоматичний вибір; решта — опції для
//! перевизначення. У десктопному UI (Tauri, M2) це рендериться як кнопки —
//! каталог уже віддється машиночитним `catalog_json()`; у CLI — нумероване
//! меню `skri model`.
//!
//! Джерело рекомендацій: дослідження локального укр.-стеку (специфікація §7.3).
//! Залізні правила: усе нижче — локально і безкоштовно; хмара (byok-cloud) —
//! завжди Остання опція і ніколи не авто.

use crate::{Error, Result};
use serde::Serialize;
use std::path::Path;

/// Одна опція моделі в каталозі задачі.
#[derive(Debug, Clone, Serialize)]
pub struct ModelOption {
    /// ідентифікатор для settings.json
    pub id: String,
    /// людською, для кнопки
    pub label: String,
    /// особливість/джерело; показується під кнопкою
    pub note: String,
    /// чи працює повністю локально (byok-cloud = false)
    pub local: bool,
}

/// Задача (кнопка в UI).
#[derive(Debug, Clone, Serialize)]
pub struct TaskSpec {
    pub id: String,
    /// назва кнопки
    pub label: String,
    /// коли цю кнопку тиснути
    pub hint: String,
    /// options[0] = автоматичний вибір
    pub options: Vec<ModelOption>,
}

/// Каталог задач (порядок = порядок кнопок в UI).
pub fn catalog() -> Vec<TaskSpec> {
    vec![
        TaskSpec {
            id: "generate-uk".into(),
            label: "Генерація тексту українською".into(),
            hint: "чернетки сцен, продовження речення, стилізація".into(),
            options: vec![
                ModelOption {
                    id: "lapa-v0.1.2".into(),
                    label: "Lapa LLM v0.1.2 — Ollama `uamarchuan/lapa-v0.1.2-instruct`".into(),
                    note: "найкраща відкрита укр. модель (Gemma3 12B, 128K ctx). Моделі <7B ламають укр. орфографію — тримайте 7B+".into(),
                    local: true,
                },
                ModelOption {
                    id: "llama-3.1-8b".into(),
                    label: "Llama 3.1 8B Instruct — Ollama".into(),
                    note: "загальна модель; укр. вихід слабший за Lapa".into(),
                    local: true,
                },
                ModelOption {
                    id: "byok-cloud".into(),
                    label: "Хмара (свій API-ключ)".into(),
                    note: "опційно, з явним дозволом; ніколи не обирається автоматично".into(),
                    local: false,
                },
            ],
        },
        TaskSpec {
            id: "translate".into(),
            label: "Переклад прози".into(),
            hint: "укр↔en і далі; глосарій серії застосовується примусово".into(),
            options: vec![
                ModelOption {
                    id: "nllb-200".into(),
                    label: "NLLB-200 (локально)".into(),
                    note: "baseline укр. MT на 200 мов; реєстр ти/ви перевіряти окремим проходом".into(),
                    local: true,
                },
                ModelOption {
                    id: "lapa-v0.1.2".into(),
                    label: "Lapa LLM v0.1.2 — стильовий переклад".into(),
                    note: "глосарій серії в промпті; для художнього ритму".into(),
                    local: true,
                },
                ModelOption {
                    id: "byok-cloud".into(),
                    label: "Хмара (свій API-ключ)".into(),
                    note: "опційно".into(),
                    local: false,
                },
            ],
        },
        TaskSpec {
            id: "transcribe-uk".into(),
            label: "Транскрипція аудіо (інтерв'ю)".into(),
            hint: "записи інтерв'ю → текст; усе on-device".into(),
            options: vec![
                ModelOption {
                    id: "whisper-large-v3-turbo-uk".into(),
                    label: "whisper large-v3-turbo (укр. fine-tune Yehor, WER 12.75%)".into(),
                    note: "github.com/egorsmkv/speech-recognition-uk".into(),
                    local: true,
                },
                ModelOption {
                    id: "whisper-large-v3".into(),
                    label: "whisper large-v3 (базовий)".into(),
                    note: "плутає укр/рос на кириличному міксі → мову задавати вручну".into(),
                    local: true,
                },
            ],
        },
        TaskSpec {
            id: "grammar".into(),
            label: "Граматика і стиль".into(),
            hint: "spell/grammar під час письма; кілька мов одночасно".into(),
            options: vec![
                ModelOption {
                    id: "languagetool-local".into(),
                    label: "LanguageTool server (локально)".into(),
                    note: "укр + 25+ мов, без хмари; вимкнений Live-Pattern конфлікт не стосується".into(),
                    local: true,
                },
                ModelOption {
                    id: "goroh-dictionary".into(),
                    label: "Goroh + dmklinger укр. словник".into(),
                    note: "офлайн-перевірка написання без сервера".into(),
                    local: true,
                },
            ],
        },
        TaskSpec {
            id: "summarize".into(),
            label: "Рекапи і резюме (пам'ять серії)".into(),
            hint: "recap між книгами, резюме арок, «де я зупинився»".into(),
            options: vec![
                ModelOption {
                    id: "lapa-v0.1.2".into(),
                    label: "Lapa LLM v0.1.2 — резюме з індексу сутностей".into(),
                    note: "вхід — не повний текст, а агрегат Index Engine".into(),
                    local: true,
                },
                ModelOption {
                    id: "llama-3.1-8b".into(),
                    label: "Llama 3.1 8B Instruct — Ollama".into(),
                    note: "якщо Lapa не встановлена".into(),
                    local: true,
                },
            ],
        },
        TaskSpec {
            id: "semantic-search".into(),
            label: "Семантичний пошук і порівняння описів".into(),
            hint: "кластеризація описів, «схожі сцени»".into(),
            options: vec![
                ModelOption {
                    id: "rules-plus-fts".into(),
                    label: "Правила + FTS5 (без моделей)".into(),
                    note: "детерміновано; embedding-моделі для укр. поки слабкі".into(),
                    local: true,
                },
                ModelOption {
                    id: "multilingual-e5".into(),
                    label: "multilingual-e5-base (експеримент)".into(),
                    note: "коли з'явиться сильніша укр. embedding-модель".into(),
                    local: true,
                },
            ],
        },
    ]
}

/// Задача за id.
pub fn task(id: &str) -> Option<TaskSpec> {
    catalog().into_iter().find(|t| t.id == id)
}

/// Автоматичний вибір: перша (рекомендована) опція задачі.
pub fn recommend(task_id: &str) -> Option<ModelOption> {
    task(task_id).and_then(|t| t.options.into_iter().next())
}

/// Каталог як JSON — для кнопок майбутнього UI (Tauri) і для тестів.
pub fn catalog_json() -> serde_json::Result<String> {
    serde_json::to_string_pretty(&catalog())
}

// --- Збереження вибору в settings.json проєкту ---

fn settings_path(root: &Path) -> std::path::PathBuf {
    root.join(".skri").join("settings.json")
}

fn read_settings(root: &Path) -> Result<serde_json::Value> {
    let raw = std::fs::read_to_string(settings_path(root))?;
    Ok(serde_json::from_str(&raw)?)
}

fn write_settings(root: &Path, v: &serde_json::Value) -> Result<()> {
    std::fs::write(settings_path(root), serde_json::to_string_pretty(v)?)?;
    Ok(())
}

/// Зберегти вибір моделі для задачі.
/// `auto=true` — автоматичний вибір (рекомендована модель).
/// `auto=false` — ручний вибір кнопкою.
pub fn set_model(root: &Path, task_id: &str, model: &str, auto: bool) -> Result<()> {
    if task(task_id).is_none() {
        return Err(Error::Invalid(format!("невідома задача «{task_id}»")));
    }
    let mut v = read_settings(root)?;
    let obj = v
        .as_object_mut()
        .ok_or_else(|| Error::Invalid("settings.json — не JSON-об'єкт".into()))?;
    let models = obj
        .entry("models".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let m = models
        .as_object_mut()
        .ok_or_else(|| Error::Invalid("розділ models пошкоджено".into()))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    m.insert(
        task_id.to_string(),
        serde_json::json!({ "model": model, "auto": auto, "set_at": now }),
    );
    write_settings(root, &v)
}

/// Поточне призначення: Some((model, auto)).
pub fn model_for(root: &Path, task_id: &str) -> Result<Option<(String, bool)>> {
    let v = read_settings(root)?;
    let m = v.get("models").and_then(|m| m.get(task_id));
    Ok(m.and_then(|m| {
        let model = m.get("model")?.as_str()?.to_string();
        let auto = m.get("auto").and_then(|a| a.as_bool()).unwrap_or(true);
        Some((model, auto))
    }))
}

/// Призначення для всіх задач каталогу (для `skri model status` і UI).
pub fn assignments(root: &Path) -> Result<Vec<(TaskSpec, Option<(String, bool)>)>> {
    Ok(catalog()
        .into_iter()
        .map(|t| {
            let cur = model_for(root, &t.id).unwrap_or(None);
            (t, cur)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_complete_and_unique() {
        let cat = catalog();
        assert!(cat.len() >= 5);
        let mut ids: Vec<&str> = cat.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), cat.len(), "id задач унікальні");
        for t in &cat {
            assert!(!t.options.is_empty(), "{} без опцій", t.id);
            assert!(!t.label.is_empty());
            // кожна задача має локальну рекомендовану опцію
            assert!(t.options[0].local, "{}: авто-вибір має бути локальним", t.id);
        }
    }

    #[test]
    fn recommend_is_first_and_never_cloud() {
        let rec = recommend("translate").expect("translate у каталозі");
        assert!(rec.local, "автовибір ніколи не хмара");
        assert_eq!(rec.id, "nllb-200");
        assert!(recommend("нема-такої").is_none());
    }

    #[test]
    fn unknown_task_rejected_on_set() {
        let dir = std::env::temp_dir().join(format!("skri-model-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".skri")).unwrap();
        std::fs::write(
            dir.join(".skri").join("settings.json"),
            r#"{"preset":"novelist-uk","language":"uk"}"#,
        )
        .unwrap();
        assert!(set_model(&dir, "неіснуюча", "x", true).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
