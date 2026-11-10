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
//!
//! FQA16 (машинно-читабельні коди): кожен warning і verdict має стабільне
//! поле `code` + структуровані `params` (числа/імена) для локалізації UI;
//! український human-текст (`message`/`verdict`/`issues`) зберігається для CLI.
//! Стабільні коди валідатора епізоду (інтегратор локалізує UI за ними):
//!
//! | code                     | де                  | params                                            |
//! |--------------------------|---------------------|---------------------------------------------------|
//! | `no_cold_open`           | EpisodeCheck.issues | `{}`                                              |
//! | `too_few_turns`          | EpisodeCheck.issues | `{turns, min_turns}`                              |
//! | `no_hook`                | EpisodeCheck.issues | `{}`                                              |
//! | `dialogue_scenes_in_row` | EpisodeCheck.issues | `{scene, streak, max_dialogue_scenes_in_row}`     |
//! | `empty_scene`            | MuteBeat            | `{scene, action_lines, dialogue_lines}`           |
//! | `visual_pacing_ok`       | MuteBeat            | `{scene, visual_ratio}`                           |
//! | `dialogue_heavy_scene`   | MuteBeat            | `{scene, visual_ratio}`                           |
//! | `mute_violation`         | MuteBeat            | `{scene, visual_ratio}`                           |
//! | `bible_rule_hit`         | BibleRuleHit        | `{rule, keywords, hits}`                          |

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
    /// людський укр. текст issues (CLI, сумісність) — коди в [`EpisodeCheck::issues_structured`]
    pub issues: Vec<String>,
    /// FQA16: ті самі issues з машинно-читабельним code + params (для локалізації UI)
    pub issues_structured: Vec<EpisodeIssue>,
}

/// Один issue валідатора: стабільний машинний код + параметри + людський текст.
#[derive(Debug, Clone, Serialize)]
pub struct EpisodeIssue {
    /// стабільний код (див. таблицю кодів у шапці модуля)
    pub code: String,
    /// структуровані параметри (числа/імена) для підстановки в локалізований шаблон
    pub params: serde_json::Value,
    /// існуючий український human-текст (CLI не ламається)
    pub message: String,
}

fn issue(code: &str, params: serde_json::Value, message: String) -> EpisodeIssue {
    EpisodeIssue {
        code: code.to_string(),
        params,
        message,
    }
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
    let mut issues: Vec<EpisodeIssue> = Vec::new();

    let cold_open = scenes.iter().find(|s| s.cold_open).map(|s| s.name.clone());
    if fmt.cold_open_required && cold_open.is_none() {
        issues.push(issue(
            "no_cold_open",
            serde_json::json!({}),
            "немає cold open — анімаційний формат починається з холодного відкриття (тізер до титрів)".into(),
        ));
    }

    // narrative turns = зміни напрямку між сценами: спрощено — кількість переходів
    // сцена→сцена (кожен перехід у 20-хв форматі обов'язково тримає конфлікт)
    let turns = scenes.len().saturating_sub(1);
    if turns < fmt.min_turns {
        issues.push(issue(
            "too_few_turns",
            serde_json::json!({ "turns": turns, "min_turns": fmt.min_turns }),
            format!(
                "narrative turns: {} < {} мінімуму для 20-хв епізоду",
                turns, fmt.min_turns
            ),
        ));
    }

    let hook = scenes.iter().find(|s| s.hook).map(|s| s.name.clone());
    if fmt.hook_required && hook.is_none() {
        issues.push(issue(
            "no_hook",
            serde_json::json!({}),
            "немає hook — фінальна сцена-гачок для наступного епізоду обов'язкова".into(),
        ));
    }

    // діалогові сцени підряд — червоний прапор анімації
    let mut streak = 0usize;
    for s in &scenes {
        if s.dialogue_lines > s.action_lines {
            streak += 1;
            if streak > fmt.max_dialogue_scenes_in_row {
                issues.push(issue(
                    "dialogue_scenes_in_row",
                    serde_json::json!({
                        "scene": s.name,
                        "streak": streak,
                        "max_dialogue_scenes_in_row": fmt.max_dialogue_scenes_in_row,
                    }),
                    format!(
                        "сцена «{}»: {} діалогових сцен підряд (> {}); «characters standing around engaging in lengthy conversations» — для анімації це червоний прапор",
                        s.name, streak, fmt.max_dialogue_scenes_in_row
                    ),
                ));
                streak = 0;
            }
        } else {
            streak = 0;
        }
    }

    let issues_structured = issues;
    let human: Vec<String> = issues_structured.iter().map(|i| i.message.clone()).collect();
    EpisodeCheck {
        passed: issues_structured.is_empty(),
        cold_open,
        turns,
        hook,
        issues: human,
        issues_structured,
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
    /// людський укр. verdict (CLI, сумісність); машинний код — у [`MuteBeat::code`]
    pub verdict: String,
    /// FQA16: стабільний код verdict (див. таблицю кодів у шапці модуля):
    /// empty_scene | visual_pacing_ok | dialogue_heavy_scene | mute_violation
    pub code: String,
    /// FQA16: структуровані параметри для локалізації UI
    pub params: serde_json::Value,
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
            let (code, verdict) = if total == 0 {
                ("empty_scene", "порожня сцена".to_string())
            } else if ratio >= 0.6 {
                ("visual_pacing_ok", "гра читається без звуку".to_string())
            } else if ratio >= 0.3 {
                (
                    "dialogue_heavy_scene",
                    "половина картини — діалог; перевірте mute-тестом".to_string(),
                )
            } else {
                (
                    "mute_violation",
                    "сцена «гасне» без звуку: діалог виконує роботу картинки".to_string(),
                )
            };
            let rounded = (ratio * 100.0).round() / 100.0;
            MuteBeat {
                scene: s.name.clone(),
                action_lines: s.action_lines,
                dialogue_lines: s.dialogue_lines,
                visual_ratio: rounded,
                verdict,
                code: code.to_string(),
                params: serde_json::json!({
                    "scene": s.name,
                    "visual_ratio": rounded,
                    "action_lines": s.action_lines,
                    "dialogue_lines": s.dialogue_lines,
                }),
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
///
/// FQA16: структурний варіант — [`check_bible_rules_structured`] (code + params);
/// ця функція лишається людським uk-текстом для CLI.
pub fn check_bible_rules(store: &ProjectStore, episode_body: &str) -> Result<Vec<String>> {
    Ok(check_bible_rules_structured(store, episode_body)?
        .into_iter()
        .map(|w| w.message)
        .collect())
}

/// Структуроване попередження про потрапляння правила біблії в епізод.
#[derive(Debug, Clone, Serialize)]
pub struct BibleRuleHit {
    /// стабільний код: `bible_rule_hit`
    pub code: String,
    pub rule: String,
    /// всі слова-індикатори правила
    pub keywords: Vec<String>,
    /// слова-індикатори, знайдені в епізоді
    pub hits: Vec<String>,
    /// існуючий український human-текст (CLI не ламається)
    pub message: String,
}

/// Перевірити епізод проти правил біблії — машинно-читабельний варіант (FQA16).
pub fn check_bible_rules_structured(
    store: &ProjectStore,
    episode_body: &str,
) -> Result<Vec<BibleRuleHit>> {
    let rules = bible_rules(store)?;
    let lower = episode_body.to_lowercase();
    let mut warnings = Vec::new();
    for r in rules {
        if r.keywords.is_empty() {
            continue;
        }
        let hits: Vec<String> = r
            .keywords
            .iter()
            .filter(|k| lower.contains(k.as_str()))
            .cloned()
            .collect();
        if !hits.is_empty() {
            warnings.push(BibleRuleHit {
                code: "bible_rule_hit".to_string(),
                rule: r.rule.clone(),
                keywords: r.keywords.clone(),
                hits: hits.clone(),
                message: format!(
                    "правило біблії: «{}» — у епізоді згадані слова-індикатори ({}). Це retcon чи помилка?",
                    r.rule,
                    hits.join(", ")
                ),
            });
        }
    }
    Ok(warnings)
}

/// Зберегти episode-check у БД для дашиборда (можливо пізніше).
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
                // FQA16: машинно-читабельні коди + params для локалізації UI
                "issues_structured": check.issues_structured,
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

    // --- FQA16: машинно-читабельні коди ---

    #[test]
    fn fqa16_episode_issues_have_stable_codes() {
        let bad = "## СЦЕНА: Початок\nВсе спокійно.\n";
        let check = validate_episode(bad, &EpisodeFormat::default());
        let codes: Vec<&str> = check
            .issues_structured
            .iter()
            .map(|i| i.code.as_str())
            .collect();
        assert!(codes.contains(&"no_cold_open"), "codes: {codes:?}");
        assert!(codes.contains(&"too_few_turns"), "codes: {codes:?}");
        assert!(codes.contains(&"no_hook"), "codes: {codes:?}");
        // структуровані params (числа)
        let turns = check
            .issues_structured
            .iter()
            .find(|i| i.code == "too_few_turns")
            .unwrap();
        assert_eq!(turns.params.get("turns"), Some(&serde_json::json!(0)));
        assert_eq!(turns.params.get("min_turns"), Some(&serde_json::json!(2)));
        // людський uk-текст не зник (CLI не ламається)
        assert!(check.issues.iter().any(|i| i.contains("cold open")));
        assert!(check.issues.iter().any(|i| i.contains("hook")));
        // JSON-серіалізація включає code + params
        let json = serde_json::to_string(&check).unwrap();
        assert!(json.contains("\"code\":\"no_cold_open\""), "json: {json}");
        assert!(json.contains("\"params\""), "json: {json}");
    }

    #[test]
    fn fqa16_dialogue_streak_has_code() {
        let bad = "\
## СЦЕНА: А\nА: мов.\nА: ще мов.\n
дія.\n## СЦЕНА: Б\nБ: мов.\nБ: ще мов.\n
дія.\n## СЦЕНА: В\nВ: мов.\nВ: ще мов.\n
дія.\n## СЦЕНА: Г\nГ: мов.\nГ: ще мов.\n
дія.\n";
        let check = validate_episode(bad, &EpisodeFormat::default());
        let hit = check
            .issues_structured
            .iter()
            .find(|i| i.code == "dialogue_scenes_in_row")
            .expect("код dialogue_scenes_in_row");
        assert_eq!(hit.params.get("streak"), Some(&serde_json::json!(3)));
        assert_eq!(
            hit.params.get("max_dialogue_scenes_in_row"),
            Some(&serde_json::json!(2))
        );
        assert!(hit.message.contains("підряд"), "human-текст збережено");
    }

    #[test]
    fn fqa16_mute_beats_have_codes() {
        let talky = "## СЦЕНА: Крамниця\nА: мов.\nБ: мов.\nВ: мов.\nГ: мов.\nдія.\n";
        let beats = mute_test(talky);
        assert_eq!(beats[0].code, "mute_violation", "діалогова сцена гасне");
        assert_eq!(beats[0].verdict, "сцена «гасне» без звуку: діалог виконує роботу картинки");
        assert_eq!(
            beats[0].params.get("scene"),
            Some(&serde_json::json!("Крамниця"))
        );
        // валідна сцена: свій код, не violation
        let good = mute_test("## СЦЕНА: Дія\nМеч впав у пилюку.\nВітер стих.\nПростішало.\n");
        assert_eq!(good[0].code, "visual_pacing_ok", "ratio={}", good[0].visual_ratio);
        // JSON-серіалізація включає code + params
        let json = serde_json::to_string(&beats).unwrap();
        assert!(json.contains("\"code\":\"mute_violation\""), "json: {json}");
        assert!(json.contains("\"params\""), "json: {json}");
    }

    #[test]
    fn fqa16_bible_rule_hits_structured() {
        let (root, store) = crate::tests_common::tmp_project("anim-fqa16");
        std::fs::write(
            root.join("bible").join("rules.md"),
            "- Мертві не оживають у цьому світі :: мертвяк, оживання, некромантія\n",
        ).unwrap();
        store.reindex().unwrap();
        let ep = "## СЦЕНА: Бій\nНекромантія сплеснула в темряві.\n";
        let hits = check_bible_rules_structured(&store, ep).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].code, "bible_rule_hit");
        assert_eq!(hits[0].rule, "Мертві не оживають у цьому світі");
        assert!(hits[0].hits.contains(&"некромантія".to_string()));
        // human-варіант для CLI не змінюється
        let human = check_bible_rules(&store, ep).unwrap();
        assert_eq!(human.len(), 1);
        assert_eq!(human[0], hits[0].message);
        // record_check зберігає issues_structured у payload
        let check = validate_episode(ep, &EpisodeFormat::default());
        record_check(&store, "manuscript/ep1.md", &check).unwrap();
        let payload: String = store
            .conn()
            .query_row(
                "SELECT payload_json FROM alerts WHERE kind='episode-check' LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            payload.contains("issues_structured") && payload.contains("\"code\""),
            "payload: {payload}"
        );
    }
}
