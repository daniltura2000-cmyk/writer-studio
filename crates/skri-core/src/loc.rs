//! Локалізаційний рушій (M2): ліміти символів per-language ЗАЗДАЛЕГІДЬ
//! + T-V реєстр (ти/ви) + PO-експорт із контекстом.
//!
//! Лікує досліджені болі:
//! - ліміт «bites in one language only» — німецький +30% ловиться У РАЙТЕРА,
//!   а не після локалізації (Trails of Cold Steel 3 кейс)
//! - *"the reverse model picks a register at random"* — T-VChecker

use crate::{Error, Result};
use serde::Serialize;
use std::collections::BTreeMap;

/// Фактори розширення тексту при перекладі (дослідження локалізації ігор):
/// en→de ≈ +30%, en→uk ≈ +15%, en→ja/zh (CJK) ≈ ×0.6 (стискається).
fn expansion_factor(target: &str) -> f64 {
    match target {
        "de" | "fr" | "es" | "it" | "pt" | "nl" | "ru" | "pl" | "cs" => 1.30,
        "uk" | "be" | "bg" | "sr" | "hr" => 1.15,
        "ja" | "zh" | "ko" => 0.60,
        "en" => 1.0,
        _ => 1.20, // консервативно для невідомих
    }
}

/// Рядок з лімітами по мовах.
#[derive(Debug, Clone, Serialize)]
pub struct LocalizedString {
    /// ідентифікатор рядка (в договорі до джерела діалогу)
    pub key: String,
    /// текст джерельною мовою
    pub source_text: String,
    pub source_lang: String,
    /// ліміти по цільових мовах: {de: 42} — у СИМВОЛАХ
    pub limits: BTreeMap<String, usize>,
    /// контекст рядка: хто/кому/тон/скріншот-слот
    pub context: BTreeMap<String, String>,
}

/// Результат перевірки ліміту для однієї цільової мови.
#[derive(Debug, Clone, Serialize)]
pub struct LimitCheck {
    pub lang: String,
    pub factor: f64,
    /// очікувана довжина перекладу в символах
    pub projected_len: usize,
    pub limit: usize,
    /// projected > limit
    pub over: bool,
    /// скільки символів надлишку (0 якщо ок)
    pub overflow: usize,
    pub advice: String,
}

/// Перевірити рядок ПРОТИ всіх лімітів (викликається в редакторі, ДО публікації).
pub fn check_limits(s: &LocalizedString) -> Vec<LimitCheck> {
    let src_len = s.source_text.chars().count();
    s.limits
        .iter()
        .map(|(lang, &limit)| {
            let factor = expansion_factor(lang);
            let projected = ((src_len as f64) * factor).ceil() as usize;
            let over = projected > limit;
            let overflow = projected.saturating_sub(limit);
            let advice = if over {
                format!(
                    "скорочено ~{} симв.: змініть конструкцію, приберіть повтори; \
                     (фактор {lang} ×{factor:.2} → очікується {projected} > ліміт {limit})",
                    overflow
                )
            } else {
                format!("ок: {projected} ≤ {limit}")
            };
            LimitCheck {
                lang: lang.clone(),
                factor,
                projected_len: projected,
                limit,
                over,
                overflow,
                advice,
            }
        })
        .collect()
}

/// Найбільш «небезпечна» мова рядка (максимальний перевищення; None = все ок).
pub fn worst_limit(s: &LocalizedString) -> Option<LimitCheck> {
    check_limits(s)
        .into_iter()
        .filter(|c| c.over)
        .max_by_key(|c| c.overflow)
}

// --- T-V реєстр (ти/ви) ---

/// Форми «ти» та «ви» укр. мовою (займенники + дієслівні закінчення).
const TV_TY: &[&str] = &["ти", "тобі", "тобою", "твій", "твоя", "твоє", "твої", "твого", "твою"];
const TV_VY: &[&str] = &[
    "ви", "вас", "вами", "ваш", "ваша", "ваше", "ваші", "вашого", "вашу",
];

/// Сигнали дієслів 2-ї особи однини (ти) і множини (ви).
const TV_TY_VERB_ENDINGS: &[&str] = &["єш", "иш", "еш", "сь", "тимся"];
const TV_VY_VERB_ENDINGS: &[&str] = &["єте", "ите", "ете"];

/// Аналіз реєстру T-V у тексті.
#[derive(Debug, Clone, Serialize)]
pub struct TvReport {
    pub ty_hits: usize,
    pub vy_hits: usize,
    /// домінантний реєстр
    pub dominant: String, // "ty" | "vy" | "none" | "mixed"
    /// змішування в межах одного тексту
    pub mixed: bool,
    /// приклади змішування (речення)
    pub examples: Vec<String>,
}

/// Проаналізувати текст на змішування ти/ви.
///
/// Обмеження чесно: евристика по займенниках і типових закінченнях;
/// «ви» в значенні множини (до групи) не відрізнити без контексту —
/// тому інструмент ПОПЕРЕДЖАЄ, а не виправляє сам.
pub fn analyze_tv(text: &str) -> TvReport {
    let lower = text.to_lowercase();
    let ty_hits = TV_TY.iter().filter(|w| has_word(&lower, w)).count()
        + ends_with_any(&lower, TV_TY_VERB_ENDINGS);
    let vy_hits = TV_VY.iter().filter(|w| has_word(&lower, w)).count()
        + ends_with_any(&lower, TV_VY_VERB_ENDINGS);

    let dominant = if ty_hits == 0 && vy_hits == 0 {
        "none"
    } else if ty_hits > 0 && vy_hits == 0 {
        "ty"
    } else if vy_hits > 0 && ty_hits == 0 {
        "vy"
    } else {
        "mixed"
    };

    let examples = if dominant == "mixed" {
        text.lines()
            .filter(|l| {
                let ll = l.to_lowercase();
                let t = TV_TY.iter().any(|w| has_word(&ll, w));
                let v = TV_VY.iter().any(|w| has_word(&ll, w));
                t || v
            })
            .take(5)
            .map(|s| s.trim().to_string())
            .collect()
    } else {
        Vec::new()
    };

    TvReport {
        ty_hits,
        vy_hits,
        dominant: dominant.to_string(),
        mixed: dominant == "mixed",
        examples,
    }
}

fn has_word(lower: &str, word: &str) -> bool {
    lower.split(|c: char| !c.is_alphanumeric() && c != '\'').any(|w| w == word)
}

fn ends_with_any(lower: &str, endings: &[&str]) -> usize {
    lower
        .split_whitespace()
        .filter(|w| endings.iter().any(|e| w.ends_with(e)))
        .count()
}

// --- PO-експорт (GNU gettext) з контекстом ---

/// Екранування PO.
fn po_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

/// Згенерувати .po-файл (msgctxt = контекст, msgid = джерело).
/// Контекст подорожує з рядком — саме те, що втрачає Articy-експорт.
pub fn export_po(strings: &[LocalizedString], target_lang: &str) -> Result<String> {
    if target_lang.is_empty() {
        return Err(Error::Invalid("порожня цільова мова".into()));
    }
    let mut out = String::new();
    out.push_str(&format!(
        "# Writer Studio PO export (context travels with the line)\n\
         msgid \"\"\n\
         msgstr \"\"\n\
         \"Language: {target_lang}\\n\"\n\n"
    ));
    for s in strings {
        if !s.context.is_empty() {
            let ctx: Vec<String> = s
                .context
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect();
            out.push_str(&format!("#. {}\n", po_escape(&ctx.join("; "))));
        }
        out.push_str(&format!("msgctxt \"{}\"\n", po_escape(&s.key)));
        out.push_str(&format!("msgid \"{}\"\n", po_escape(&s.source_text)));
        out.push_str(&format!("msgstr \"\"\n\n"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_flag_overflow_before_publication() {
        let s = LocalizedString {
            key: "d01_l03".into(),
            source_text: "Ти не пройдеш повз цю браму, мандрівнику!".into(), // 42 chars
            source_lang: "uk".into(),
            limits: BTreeMap::from([("de".into(), 40), ("uk".into(), 60)]),
            context: BTreeMap::from([("speaker".into(), "вартовий".into())]),
        };
        let checks = check_limits(&s);
        let de = checks.iter().find(|c| c.lang == "de").unwrap();
        assert!(de.over, "de: 42×1.30 ≈ 55 > 40 — має бути перевищення ДО публікації");
        assert!(de.overflow > 0);
        let uk = checks.iter().find(|c| c.lang == "uk").unwrap();
        assert!(!uk.over);
        let worst = worst_limit(&s).unwrap();
        assert_eq!(worst.lang, "de");
    }

    #[test]
    fn cjk_shrinks_not_expands() {
        let s = LocalizedString {
            key: "k".into(),
            source_text: "Привіт, світ!".into(),
            source_lang: "uk".into(),
            limits: BTreeMap::from([("ja".into(), 10)]),
            context: BTreeMap::new(),
        };
        let ja = check_limits(&s).remove(0);
        assert!(!ja.over, "ja ×0.6 → 13×0.6 ≈ 8 ≤ 10");
        assert_eq!(ja.factor, 0.60);
    }

    #[test]
    fn tv_detects_mixed_register() {
        let mixed = "Ти чого тут забрів? Ну й ну, вам же казали не ходити. Ваш кінець.";
        let r = analyze_tv(mixed);
        assert!(r.mixed, "ти+ваш у одному монолозі = mixed");
        assert_eq!(r.dominant, "mixed");
        assert!(!r.examples.is_empty());

        let pure_ty = "Куди ти йдеш, ти ж обіцяв? Твій шлях — твій вибір.";
        assert_eq!(analyze_tv(pure_ty).dominant, "ty");
        let none = "Дощ іде. Море шумить.";
        assert_eq!(analyze_tv(none).dominant, "none");
    }

    #[test]
    fn po_export_keeps_context() {
        let s = LocalizedString {
            key: "d01_l01".into(),
            source_text: "Скажи їй \"ні\"".into(),
            source_lang: "uk".into(),
            limits: BTreeMap::new(),
            context: [
                ("speaker".to_string(), "Кальдрін".to_string()),
                ("tone".to_string(), "ironic".to_string()),
            ]
            .into_iter()
            .collect(),
        };
        let po = export_po(&[s], "en").unwrap();
        assert!(po.contains("Language: en"));
        assert!(po.contains("msgctxt \"d01_l01\""));
        assert!(po.contains("#. speaker: Кальдрін; tone: ironic"), "контекст подорожує з рядком");
        assert!(po.contains("msgid \"Скажи їй \\\"ні\\\"\""), "лапки екрановані");
    }
}
