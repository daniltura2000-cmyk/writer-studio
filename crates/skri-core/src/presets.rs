//! Preset Engine (M0): композабельні пресети жанрів як JSON-маніфести.
//!
//! Ядро порожнє; кожен жанр = пресет. Ассети (структури, чеклісти, валідатори,
//! шаблони, AI-рецепти) — першокласні об'єкти, з яких користувач збирає свій пресет.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Тип сутності з кастомними полями.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityType {
    pub name: String,
    #[serde(default)]
    pub fields: Vec<String>,
}

/// Чекліст.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checklist {
    pub name: String,
    pub items: Vec<String>,
}

/// AI-рецепт (локальна модель за замовчуванням).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRecipe {
    pub name: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub prompt: String,
}

fn default_model() -> String {
    "local".to_string()
}

/// Маніфест пресету.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetManifest {
    pub preset: String,
    #[serde(default = "default_lang")]
    pub language: String,
    #[serde(default)]
    pub entities: Vec<EntityType>,
    #[serde(default)]
    pub structure: serde_json::Value,
    #[serde(default)]
    pub checklists: Vec<Checklist>,
    #[serde(default)]
    pub validators: BTreeMap<String, bool>,
    #[serde(default)]
    pub templates: BTreeMap<String, String>,
    #[serde(default)]
    pub ai_recipes: Vec<AiRecipe>,
}

fn default_lang() -> String {
    "uk".to_string()
}

impl PresetManifest {
    /// Розпарсити маніфест з JSON-рядка і провалідувати.
    pub fn from_json(json: &str) -> Result<Self> {
        let p: PresetManifest = serde_json::from_str(json)?;
        p.validate()?;
        Ok(p)
    }

    /// Валідація: назва, мова, хоча б один тип сутності.
    pub fn validate(&self) -> Result<()> {
        if self.preset.trim().is_empty() {
            return Err(Error::Invalid("порожня назва пресету".into()));
        }
        let l = self.language.len();
        if !(2..=8).contains(&l) || !self.language.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(Error::Invalid(format!(
                "некоректна мова «{}» (очікується код типу uk/en)",
                self.language
            )));
        }
        if self.entities.is_empty() {
            return Err(Error::Invalid("пресет без типів сутностей".into()));
        }
        for e in &self.entities {
            if e.name.trim().is_empty() {
                return Err(Error::Invalid("порожня назва типу сутності".into()));
            }
        }
        Ok(())
    }
}

/// Вбудовані пресети (M0+M1): романіст (укр) і нотатник (укр).
pub const BUILTIN: &[(&str, &str)] = &[
    ("novelist-uk", include_str!("../../../presets/novelist-uk.json")),
    ("note-taker-uk", include_str!("../../../presets/note-taker-uk.json")),
    ("game-narrative-uk", include_str!("../../../presets/game-narrative-uk.json")),
    ("animation-uk", include_str!("../../../presets/animation-uk.json")),
    ("journalist-uk", include_str!("../../../presets/journalist-uk.json")),
];

/// Завантажити вбудований пресет за назвою.
pub fn builtin(name: &str) -> Option<PresetManifest> {
    BUILTIN
        .iter()
        .find(|(n, _)| *n == name)
        .and_then(|(_, json)| PresetManifest::from_json(json).ok())
}

/// Завантажити пресет: вбудований або файл.
pub fn load(name: &str) -> Result<PresetManifest> {
    if let Some(p) = builtin(name) {
        return Ok(p);
    }
    let path = std::path::Path::new(name);
    if path.exists() {
        let raw = std::fs::read_to_string(path)?;
        return PresetManifest::from_json(&raw);
    }
    Err(Error::Invalid(format!(
        "пресет «{}» не знайдено (немає вбудованого і файлу)",
        name
    )))
}

/// Композитор: зібрати ВЛАСНИЙ пресет з ассетів кількох джерел.
///
/// `spec` — рядок виду `"structure:novelist-uk checklists:academic-uk entities:novelist-uk"`,
/// тобто `<asset_kind>:<preset_name>` через пробіл. Останній виграє при конфлікті.
/// Результат валідується і повертається як маніфест (зберігайте `save_to_json`).
pub fn compose(spec: &str, name: &str, language: Option<&str>) -> Result<PresetManifest> {
    if name.trim().is_empty() {
        return Err(Error::Invalid("порожня назва композитного пресету".into()));
    }
    let mut entities: Vec<EntityType> = Vec::new();
    let mut structure: Option<serde_json::Value> = None;
    let mut checklists: Vec<Checklist> = Vec::new();
    let mut validators: BTreeMap<String, bool> = BTreeMap::new();
    let mut templates: BTreeMap<String, String> = BTreeMap::new();
    let mut ai_recipes: Vec<AiRecipe> = Vec::new();
    let mut lang = language.unwrap_or("uk").to_string();

    for part in spec.split_whitespace() {
        let (kind, preset_name) = part.split_once(':').ok_or_else(|| {
            Error::Invalid(format!(
                "ассет «{part}» без ':пресет' (формат: kind:preset, напр. checklists:novelist-uk)"
            ))
        })?;
        let src = load(preset_name)?;
        match kind {
            "entities" => {
                for e in &src.entities {
                    if !entities.iter().any(|x| x.name == e.name) {
                        entities.push(e.clone());
                    }
                }
            }
            "structure" => structure = Some(src.structure.clone()),
            "checklists" => {
                for c in &src.checklists {
                    if !checklists.iter().any(|x| x.name == c.name) {
                        checklists.push(c.clone());
                    }
                }
            }
            "validators" => validators.extend(src.validators.clone()),
            "templates" => templates.extend(src.templates.clone()),
            "ai_recipes" => {
                for r in &src.ai_recipes {
                    if !ai_recipes.iter().any(|x| x.name == r.name) {
                        ai_recipes.push(r.clone());
                    }
                }
            }
            "language" => lang = src.language.clone(),
            other => {
                return Err(Error::Invalid(format!(
                    "невідомий вид ассета «{other}» (доступні: entities, structure, checklists, validators, templates, ai_recipes, language)"
                )))
            }
        }
    }

    if entities.is_empty() {
        return Err(Error::Invalid(
            "композит не містить entities — додайте хоча б `entities:<пресет>`".into(),
        ));
    }
    let manifest = PresetManifest {
        preset: name.to_string(),
        language: lang,
        entities,
        structure: structure.unwrap_or(serde_json::json!({})),
        checklists,
        validators,
        templates,
        ai_recipes,
    };
    manifest.validate()?;
    Ok(manifest)
}

/// Зберегти маніфест у JSON-файл (для імпорту в іншому проєкті).
pub fn save_to_json(p: &PresetManifest, path: &std::path::Path) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(p)?)?;
    Ok(())
}

/// Список вбудованих пресетів: (назва, опис).
pub fn builtin_list() -> Vec<(String, String)> {
    BUILTIN
        .iter()
        .filter_map(|(name, json)| {
            PresetManifest::from_json(json)
                .ok()
                .map(|p| (name.to_string(), describe(&p)))
        })
        .collect()
}

fn describe(p: &PresetManifest) -> String {
    let types: Vec<&str> = p.entities.iter().map(|e| e.name.as_str()).collect();
    format!(
        "мова: {}; сутності: {}",
        p.language,
        types.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_valid() {
        for (name, json) in BUILTIN {
            let p = PresetManifest::from_json(json).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(p.language, "uk");
        }
    }

    #[test]
    fn rejects_bad_language() {
        let raw = r#"{"preset":"x","language":"українська (UA)","entities":[{"name":"a"}]}"#;
        assert!(PresetManifest::from_json(raw).is_err());
    }
}
