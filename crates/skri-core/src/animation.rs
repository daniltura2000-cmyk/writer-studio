//! Animation Writer (M3): Show Bible як база знань, структурний валідатор
//! анімаційного епізоду, мют-тест (біти-кадри з action lines).
//!
//! Лікує досліджені болі:
//! - *"Any particular piece of software better? Or do you just use something
//!    basic like MS Word or Apple Pages?"* — біблія серіалу не пов'язана з епізодами
//! - *"a 20-minute episode includes a brief cold open, two narrative turns,
//!    and concludes with a compelling hook"* — структура не перевіряється ніде
//! - *"watch it with the sound off: wherever the story goes blank..."* —
//!    для dialogue-free сцен немає ані аналітики бітів, ані зв'язки зі сторібордом

use crate::store::ProjectStore;
use crate::Result;
use rusqlite::params;
use serde::Serialize;

/// Валідатор структури анімаційного епізоду (20-хв формат).
#[derive(Debug, Clone, Serialize)]
pub struct EpisodeCheck {
    pub passed: bool,
    /// знайдений cold open (сцена до першого title/time-skip)
    pub cold_open: Option<String>,
    /// кількість narrative turns (паузи/зсуви між сценами з конфліктом)
    pub turns: usize,
    pub hook: Option<String>,
    pub issues: Vec<String>,
}

/// Пресет структури 20-хв анімаційного епізоду.
#[derive(Debug, Clone, Serialize)]
pub struct EpisodeFormat {
    pub cold_open_required: bool,
    pub min_turns: usize,
    pub max_dialogue_scenes_in_row: usize,
    pub hook_required: bool,
}

impl Default for EpisodeFormat {
    fn default() -> Self {
        EpisodeFormat {
            cold_open_required: true,
            min_turns: 2,
            max_dialogue_scenes_in_row: 2,
            hook_required: true,
        }
    }
}

/// Сцена епізоду (розмітка у документі: `## СЦЕНА: Назва` + опційно `COLD OPEN` / `HOOK`).
#[derive(Debug, Clone)]
pub struct Scene {
    pub name: String,
    /// діалогових рядків (та персона мовить)
    pub dialogue_lines: usize,
    /// action lines (опис дії, без реплік)
    pub action_lines: usize,
    pub cold_open: bool,
    pub hook: bool,
}

/// Розпарсити сцени з тіла епізоду.
pub fn parse_scenes(body: &str) -> Vec<Scene> {
    let mut scenes: Vec<Scene> = Vec::new();
    let mut current: Option<Scene> = None;
    for line in body.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("## СЦЕНА:") {
            if let Some(c) = current.take() {
                scenes.push(c);
            }
            let name = rest.trim().to_string();
            let cold_open = name.to_uppercase().contains("COLD OPEN")
                || name.to_uppercase().contains("ХОЛОДНИЙ ОТКРИТТЯ")
                || name.to_uppercase().contains("COLD");
            let hook = name.to_uppercase().contains("HOOK") || name.to_uppercase().contains("ФІНАЛ");
            current = Some(Scene {
                name,
                dialogue_lines: 0,
                action_lines: 0,
                cold_open,
                hook,
            });
            continue;
        }
        if t.is_empty() || t.starts_with("##") || t.starts_with("---") {
            continue;
        }
        if let Some(c) = current.as_mut() {
            // діалог: ПЕРСОНА\nрядок або ПЕРСОНА: репліка
            if t.contains(':') && !t.ends_with(':') && t.split(':').next().map(|w| w.chars().all(|c| c.is_uppercase() || c == ' ' || c == '(')).unwrap_or(false) {
                c.dialogue_lines += 1;
            } else {
                c.action_lines += 1;
            }
        }
    }
    if let Some(c) = current.take() {
        scenes.push(c);
    }
    scenes
}

/// Валідувати епізод проти формату.
pub fn validate_episode(body: &str, fmt: &EpisodeFormat) -> EpisodeCheck {
    let scenes = parse_scenes(body);
    let mut issues = Vec::new();

    let cold_open = scenes.iter().find(|s| s.cold_open).map(|s| s.name.clone());
    if fmt.cold_open_required && cold_open.is_none() {
        issues.push("немає cold open — анімаційний формат починається з холодного відкриття (тізер до титрів)".into());
    }

    // narrative turns = зміни напрямку між сценами: спрощено — кількість переходів
    // сцена→сцена (кожен перехід у 20-хв форматі обов'язково тримає конфлікт)
    let turns = scenes.len().saturating_sub(1);
    if turns < fmt.min_turns {
        issues.push(format!(
            "narrative turns: {} < {} мінімуму для 20-хв епізоду",
            turns, fmt.min_turns
        ));
    }

    let hook = scenes.iter().find(|s| s.hook).map(|s| s.name.clone());
    if fmt.hook_required && hook.is_none() {
        issues.push("немає hook — фінальна сцена-гачок для наступного епізоду обов'язкова".into());
    }

    // діалогові сцени підряд — червоний прапор анімації
    let mut streak = 0usize;
    for s in &scenes {
        if s.dialogue_lines > s.action_lines {
            streak += 1;
            if streak > fmt.max_dialogue_scenes_in_row {
                issues.push(format!(
                    "сцена «{}»: {} діалогових сцен підряд (> {}); «characters standing around engaging in lengthy conversations» — для анімації це червоний прапор",
                    s.name, streak, fmt.max_dialogue_scenes_in_row
                ));
                streak = 0;
            }
        } else {
            streak = 0;
        }
    }

    EpisodeCheck {
        passed: issues.is_empty(),
        cold_open,
        turns,
        hook,
        issues,
    }
}

/// Мют-тест: сцена «з вимкненим звуком» — біти-кадри з action lines.
///
/// Сцена, де action-ліній мало, а діалогу багато, «гасне» без звуку:
/// графік бітів показує, де картинка робить роботу, а де діалог виконує її за неї.
#[derive(Debug, Clone, Serialize)]
pub struct MuteBeat {
    pub scene: String,
    pub action_lines: usize,
    pub dialogue_lines: usize,
    /// action / (action + dialogue)
    pub visual_ratio: f64,
    pub verdict: String,
}

pub fn mute_test(body: &str) -> Vec<MuteBeat> {
    parse_scenes(body)
        .into_iter()
        .map(|s| {
            let total = s.action_lines + s.dialogue_lines;
            let ratio = if total == 0 {
                0.0
            } else {
                s.action_lines as f64 / total as f64
            };
            let verdict = if total == 0 {
                "порожня сцена".into()
            } else if ratio >= 0.6 {
                "гра читається без звуку".into()
            } else if ratio >= 0.3 {
                "половина картини — діалог; перевірте mute-тестом".into()
            } else {
                "сцена «гасне» без звуку: діалог виконує роботу картинки".into()
            };
            MuteBeat {
                scene: s.name,
                action_lines: s.action_lines,
                dialogue_lines: s.dialogue_lines,
                visual_ratio: (ratio * 100.0).round() / 100.0,
                verdict,
            }
        })
        .collect()
}

/// Show Bible: правила світу («що НІКОЛИ не станеться») з `bible/rules.md`
/// перевіряються проти нових епізодів: правило-слова ключі, що з'явились у сцені.
#[derive(Debug, Clone, Serialize)]
pub struct BibleRule {
    pub rule: String,
    /// слова-індикатори (напр., «мертвець», «осквернення»)
    pub keywords: Vec<String>,
}

/// Завантажити правила з `bible/rules.md` (кожен рядок `- правило :: слово1, слово2`).
pub fn bible_rules(store: &ProjectStore) -> Result<Vec<BibleRule>> {
    let path = store.root.join("bible").join("rules.md");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path)?;
    let body = crate::frontmatter::parse(&raw).body;
    let mut out = Vec::new();
    for line in body.lines() {
        let t = line.trim().trim_start_matches('-').trim();
        if t.is_empty() {
            continue;
        }
        let (rule, kws) = match t.split_once("::") {
            Some((r, k)) => (r.trim().to_string(), k.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect()),
            None => (t.to_string(), Vec::new()),
        };
        out.push(BibleRule { rule, keywords: kws });
    }
    Ok(out)
}

/// Перевірити епізод проти правил біблії: keywords правила з'явились у тексті.
/// Це ПОВІДОМЛЕННЯ автору («retcon чи помилка?»), не автоматична правка.
pub fn check_bible_rules(store: &ProjectStore, episode_body: &str) -> Result<Vec<String>> {
    let rules = bible_rules(store)?;
    let lower = episode_body.to_lowercase();
    let mut warnings = Vec::new();
    for r in rules {
        if r.keywords.is_empty() {
            continue;
        }
        let hits: Vec<&String> = r.keywords.iter().filter(|k| lower.contains(k.as_str())).collect();
        if !hits.is_empty() {
            warnings.push(format!(
                "правило біблії: «{}» — у епізоді згадані слова-індикатори ({}). Це retcon чи помилка?",
                r.rule,
                hits.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            ));
        }
    }
    Ok(warnings)
}

/// Зберегти episode-check у БД для дашихборда (можливо пізніше).
pub fn record_check(store: &ProjectStore, episode_path: &str, check: &EpisodeCheck) -> Result<()> {
    store.conn().execute(
        "INSERT INTO alerts(kind, entity_id, payload_json, created_at)
         SELECT 'episode-check', NULL, ?1, strftime('%s','now')
         WHERE NOT EXISTS (SELECT 1 FROM alerts WHERE kind='episode-check' AND payload_json LIKE ?2 AND resolved=0)",
        params![
            serde_json::to_string(&serde_json::json!({
                "episode": episode_path,
                "passed": check.passed,
                "issues": check.issues,
            }))?,
            format!("%\"episode\":\"{episode_path}\"%"),
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EP: &str = "\
## СЦЕНА: COLD OPEN — Дах башти
КАЛЬДРІН: Знову ти?
ГРАФ: Небо не пам'ятає імен.
Вітер рвав плащ. Граф підвився з парапету.

## СЦЕНА: Дорога на захід
Вцілілі йшли мовчки. Попіл ложився на плечі.
Кінь хрипнув і впав.

## СЦЕНА: Крамниця мудреця
МАЙСТЕР: Довго ж ти.
КАЛЬДРІН: Дорога була довга.
МАЙСТЕР: Дороги не бувають довгими. Бувають — порожніми.
КАЛЬДРІН: То що мені робити?
МАЙСТЕР: Питання — вже половина відповіді.
КАЛЬДРІН: А друга половина?
МАЙСТЕР: Знайти того, хто запитає інакше.

## СЦЕНА: ФІНАЛ — Гачок
КАЛЬДРІН: Небо ж бо... воно пам'ятає все.
";

    #[test]
    fn episode_structure_validated() {
        let fmt = EpisodeFormat::default();
        let check = validate_episode(EP, &fmt);
        assert!(check.cold_open.is_some(), "cold open знайдено");
        assert!(check.hook.is_some(), "hook знайдено");
        assert!(check.turns >= 2);
        // діалогова сцена «Крамниця мудреця» підряд з іншою діалоговою? тут лише одна діалогова streak
        // 3 діалогові сцени підряд не мають бути (тут «Крамниця» і «Фінал» підряд — 2, дозволено)
        assert!(check.passed, "issues: {:?}", check.issues);
    }

    #[test]
    fn missing_cold_open_and_hook_flagged() {
        let bad = "## СЦЕНА: Початок\nВсе спокійно.\n";
        let check = validate_episode(bad, &EpisodeFormat::default());
        assert!(!check.passed);
        assert!(check.issues.iter().any(|i| i.contains("cold open")));
        assert!(check.issues.iter().any(|i| i.contains("hook")));
    }

    #[test]
    fn dialogue_streak_red_flag() {
        // 3 діалогові сцени підряд (> max 2)
        let bad = "\
## СЦЕНА: А\nА: мов.\nА: ще мов.\n
дія.
## СЦЕНА: Б\nБ: мов.\nБ: ще мов.\n
дія.
## СЦЕНА: В\nВ: мов.\nВ: ще мов.\n
дія.
## СЦЕНА: Г\nГ: мов.\nГ: ще мов.\n
дія.
";
        let check = validate_episode(bad, &EpisodeFormat::default());
        assert!(
            check.issues.iter().any(|i| i.contains("підряд")),
            "стрик діалогових сцен не виявлено; issues: {:?}",
            check.issues
        );
    }

    #[test]
    fn mute_test_flags_dialogue_heavy_scene() {
        let beats = mute_test(EP);
        let shop = beats.iter().find(|b| b.scene.contains("Крамниця")).unwrap();
        assert!(shop.visual_ratio < 0.3, "діалогова сцена гасне без звуку: ratio={}", shop.visual_ratio);
        assert!(shop.verdict.contains("гасне"));
        let roof = beats.iter().find(|b| b.scene.contains("COLD OPEN")).unwrap();
        assert!(roof.visual_ratio >= 0.3, "cold open має дію: ratio={}", roof.visual_ratio);
    }

    #[test]
    fn bible_rules_warn_on_violation_keywords() {
        let (root, store) = crate::tests_common::tmp_project("anim");
        std::fs::write(
            root.join("bible").join("rules.md"),
            "- Мертві не оживають у цьому світі :: мертвяк, оживання, некромантія\n",
        ).unwrap();
        store.reindex().unwrap();
        let ep = "## СЦЕНА: Бій\nНекромантія сплеснула в темряві.\n";
        let w = check_bible_rules(&store, ep).unwrap();
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("retcon чи помилка"));
    }
}
