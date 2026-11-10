//! Перевірки стилю і грамотності (M10): rules-based, без AI.
//!
//! Лікує дефект «Знайдені проблеми: 0» — до M10 жодних перевірок стилю
//! не існувало. Чотири правила:
//!
//! 1. `style-tautology` (problem) — те саме слово (нормалізоване, ≥3 символи)
//!    повторене 2+ рази ПІДРЯД або через одне слово в межах одного речення:
//!    «масло масляне», «він він», «клацнув клацнув».
//! 2. `style-repeat-word` (tip) — те саме значуще слово (не службове —
//!    фільтр через стоп-лист) ≥3 рази в ОДНОМУ реченні (настирливий повтор).
//! 3. `style-double-space` (tip) — подвійні пробіли в тілі (не відступ
//!    на початку рядка).
//! 4. `style-dash-orphan` (tip) — дефіс з пробілами замість тире в діалозі
//!    укр/ру («- Привіт» замість «— Привіт»); tip, бо це може бути
//!    свідомий стилістичний вибір.
//!
//! Усе через `store::add_alert` (kind `style-*`), sentence/char_start у
//! payload. `run_style_checks` ідемпотентна: стиль-алерти попередніх
//! прогонів зносяться перед новим (як інші rebuild).

use crate::indexer::{self, sentences, tokenize};
use crate::store::{self, ProjectStore};
use crate::Result;
use std::collections::HashSet;

/// Один знахідки перевірки стилю (до запису в БД).
#[derive(Debug, Clone)]
pub struct StyleFinding {
    /// style-tautology | style-repeat-word | style-double-space | style-dash-orphan
    pub kind: String,
    /// речення або рядок, у якому знайдено
    pub sentence: String,
    /// байтовий offset у ТІЛІ документа (без frontmatter)
    pub char_start: usize,
    /// слово або фрагмент-деталь
    pub word: String,
    pub message: String,
}

/// Нормалізація слова для порівняння «того самого слова»:
/// нижній регістр; слова ≥5 символів — за першими 4 (стем-префікс),
/// тому «масло»/«масляне» і «сказав»/«сказал» збігаються, а «він»/«вона» — ні.
fn norm(word: &str) -> String {
    let lower = word.to_lowercase();
    if lower.chars().count() >= 5 {
        lower.chars().take(4).collect()
    } else {
        lower
    }
}

fn char_len(word: &str) -> usize {
    word.chars().count()
}

/// Перевірити текст (тіло документа без frontmatter) — чиста функція.
pub fn check_text(body: &str) -> Vec<StyleFinding> {
    let mut out = Vec::new();

    for sent in sentences(body) {
        let toks = tokenize(&sent.text);

        // 1) style-tautology: те саме слово підряд або через одне слово
        let mut consumed: HashSet<usize> = HashSet::new();
        for i in 0..toks.len() {
            if consumed.contains(&i) {
                continue; // вже другий елемент попереднього повтору
            }
            let a = &toks[i];
            if char_len(&a.text) < 3 {
                continue;
            }
            let na = norm(&a.text);
            for gap in 1..=2 {
                let j = i + gap;
                if j >= toks.len() {
                    break;
                }
                let b = &toks[j];
                if char_len(&b.text) < 3 || norm(&b.text) != na {
                    continue;
                }
                consumed.insert(j);
                // FQA-11: char_start + word мають вказувати ТОЧНО на знахідку
                // в тілі: беремо початок ПЕРШОГО слова пари, а word — реальний
                // фрагмент тіла від першого до кінця другого слова (substring
                // тіла за [char_start, char_start+word.len()) = word).
                let cs = sent.start + a.start;
                let ce = sent.start + b.end;
                out.push(StyleFinding {
                    kind: "style-tautology".into(),
                    sentence: sent.text.clone(),
                    char_start: cs,
                    word: body[cs..ce].to_string(),
                    message: format!(
                        "Тавтологія: «{} {}» — те саме слово підряд (або через одне слово).",
                        a.text, b.text
                    ),
                });
                break;
            }
        }

        // 2) style-repeat-word: значуще слово ≥3 рази в реченні
        let mut counts: std::collections::HashMap<String, (usize, usize, String)> =
            std::collections::HashMap::new(); // lower -> (count, first_start, surface)
        for tok in &toks {
            if char_len(&tok.text) < 3 {
                continue;
            }
            let lower = tok.text.to_lowercase();
            if indexer::is_stopword_word(&lower) {
                continue; // службові не рахуємо — інакше «він ... він ... він» шумітиме
            }
            let e = counts
                .entry(lower)
                .or_insert((0usize, tok.start, tok.text.clone()));
            e.0 += 1;
        }
        for (_, (count, first_start, surface)) in counts {
            if count >= 3 {
                out.push(StyleFinding {
                    kind: "style-repeat-word".into(),
                    sentence: sent.text.clone(),
                    char_start: sent.start + first_start,
                    word: surface.clone(),
                    message: format!(
                        "Слово «{surface}» повторюється {count} рази в одному реченні — настирливий повтор."
                    ),
                });
            }
        }
    }

    // 3) style-double-space: подвійні пробіли в тілі (не відступ рядка)
    let chars: Vec<(usize, char)> = body.char_indices().collect();
    for k in 0..chars.len().saturating_sub(1) {
        if chars[k].1 != ' ' || chars[k + 1].1 != ' ' {
            continue;
        }
        if k > 0 {
            let prev = chars[k - 1].1;
            if prev == ' ' {
                continue; // не перший пробіл у серії — алерт уже є на початку серії
            }
            if prev == '\n' || prev == '\t' {
                continue; // відступ/табуляція на початку рядка — це форматування
            }
        }
        out.push(StyleFinding {
            kind: "style-double-space".into(),
            sentence: line_at(body, chars[k].0),
            char_start: chars[k].0,
            word: "  ".into(),
            message: "Подвійний пробіл у тексті.".into(),
        });
    }

    // 4) style-dash-orphan: дефіс з пробілами замість тире (діалог укр/ру)
    let bytes = body.as_bytes();
    for (i, c) in body.char_indices() {
        if c != '-' {
            continue;
        }
        let before_ok = i == 0
            || bytes
                .get(i.wrapping_sub(1))
                .map(|b| *b == b' ' || *b == b'\n' || *b == b'\t')
                .unwrap_or(false);
        let after_ok = bytes
            .get(i + 1)
            .map(|b| *b == b' ' || *b == b'\n')
            .unwrap_or(false);
        if before_ok && after_ok {
            out.push(StyleFinding {
                kind: "style-dash-orphan".into(),
                sentence: line_at(body, i),
                char_start: i,
                word: "-".into(),
                message: "Дефіс із пробілами там, де в діалозі укр/ру очікується тире «—». Це порада: може бути свідомим стилістичним вибором.".into(),
            });
        }
    }

    out
}

/// Рядок, що містить offset (для показу в payload).
fn line_at(body: &str, offset: usize) -> String {
    let start = body[..offset].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let end = body[offset..]
        .find('\n')
        .map(|p| offset + p)
        .unwrap_or(body.len());
    body[start..end].trim().to_string()
}

/// Прогнати style-перевірки по всьому рукопису (manuscript/*.md).
///
/// Ідемпотентна: усі алерти kind `style-*` зносяться перед прогоном.
/// Повертає усі невирішені style-алерти після запису.
pub fn run_style_checks(root: &str) -> Result<Vec<crate::store::Alert>> {
    let store = ProjectStore::open(std::path::Path::new(root))?;
    let conn = store.conn();
    conn.execute("DELETE FROM alerts WHERE kind LIKE 'style-%'", [])?;

    let mut stmt = conn.prepare("SELECT id, path FROM docs WHERE kind='manuscript'")?;
    let docs: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let mut inserted = 0usize;
    for (_, rel) in &docs {
        let body = indexer::read_body(&store, rel)?;
        for f in check_text(&body) {
            let payload = serde_json::json!({
                "doc": rel,
                "sentence": f.sentence,
                "char_start": f.char_start,
                "word": f.word,
                "message": f.message,
            });
            store::add_alert(conn, &f.kind, None, &payload)?;
            inserted += 1;
        }
    }

    let _ = inserted;
    let mut stmt = conn.prepare(
        "SELECT id, kind, entity_id, payload_json, resolved FROM alerts
         WHERE kind LIKE 'style-%' ORDER BY id",
    )?;
    let alerts: Vec<crate::store::Alert> = stmt
        .query_map([], |r| {
            Ok(crate::store::Alert {
                id: r.get(0)?,
                kind: r.get(1)?,
                entity_id: r.get(2)?,
                payload: serde_json::from_str(&r.get::<_, String>(3)?)
                    .unwrap_or(serde_json::json!({})),
                resolved: r.get::<_, i64>(4)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(alerts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tautology_next_to_each_other_and_through_one_word() {
        let f = check_text("Масло масляне.");
        assert!(f.iter().any(|x| x.kind == "style-tautology"));
        // через одне слово
        let f = check_text("Він казав казав нічого.");
        assert!(f.iter().any(|x| x.kind == "style-tautology"));
        // однакове підряд, кирилиця
        let f = check_text("Клацнув клацнув замок і відчинився.");
        assert!(f.iter().any(|x| x.kind == "style-tautology"));
        // різні слова не тавтологія
        assert!(!check_text("Дві differing слова підряд.").iter().any(|x| x
            .kind
            == "style-tautology"));
    }

    #[test]
    fn pronoun_repeat_is_tautology() {
        let f = check_text("Він він нічого не сказав.");
        assert!(f.iter().any(|x| x.kind == "style-tautology" && x.word.contains("Він")));
    }

    #[test]
    fn short_words_not_tautology() {
        // слова < 3 символів не розглядаються
        assert!(check_text("На на межі он і де де.")
            .iter()
            .all(|x| x.kind != "style-tautology"));
    }

    #[test]
    fn repeat_word_three_times_caught_service_word_not() {
        // значуще слово 3 рази
        let f = check_text("Тіні повзли, тіні росли, і тіні зникали.");
        assert!(f.iter().any(|x| x.kind == "style-repeat-word" && x.word.to_lowercase().contains("тіні")));
        // службове «сказав» 3 рази — стоп-фільтр, НЕ ловиться
        assert!(check_text("Сказав він, сказав вона, сказав я.")
            .iter()
            .all(|x| x.kind != "style-repeat-word"));
    }

    #[test]
    fn double_space_caught_line_indent_not() {
        let f = check_text("Перше речення.  Друге речення починається тут.");
        assert!(f.iter().any(|x| x.kind == "style-double-space"));
        // відступ рядка пробілами — не знахідка
        assert!(check_text("Заголовок\n    відступ з чотирьох пробілів.")
            .iter()
            .all(|x| x.kind != "style-double-space"));
    }

    #[test]
    fn dash_orphan_caught_as_tip() {
        let f = check_text("- Привіт, сказав він.");
        let hit = f.iter().find(|x| x.kind == "style-dash-orphan");
        assert!(hit.is_some());
        // тире (em dash) не дефіс — чисто
        assert!(check_text("— Привіт, сказав він.")
            .iter()
            .all(|x| x.kind != "style-dash-orphan"));
    }
}
