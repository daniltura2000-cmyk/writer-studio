//! Мінімальний frontmatter-парсер (YAML-lite: `key: value` у блоці `---`).
//! Для M0 достатньо плоских ключів; повний YAML — пізніше.

use std::collections::BTreeMap;

/// Розібраний документ: метадані + тіло (без frontmatter) + байтовий offset тіла.
#[derive(Debug, Clone)]
pub struct Parsed {
    pub meta: BTreeMap<String, String>,
    pub body: String,
    /// Байтовий offset тіла відносно початку файлу
    pub body_start: usize,
}

/// Розібрати markdown-текст з опційним frontmatter.
pub fn parse(text: &str) -> Parsed {
    if let Some(rest) = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n")) {
        let header_start = 4usize.min(text.len()); // після "---\n"
        if let Some(end_rel) = find_header_end(&text[header_start..]) {
            let header = &text[header_start..header_start + end_rel.0];
            let mut meta = BTreeMap::new();
            for line in header.lines() {
                if let Some((k, v)) = split_kv(line) {
                    meta.insert(k, v);
                }
            }
            return Parsed {
                meta,
                body: rest[end_rel.1..].to_string(),
                body_start: header_start + end_rel.1,
            };
        }
    }
    Parsed {
        meta: BTreeMap::new(),
        body: text.to_string(),
        body_start: 0,
    }
}

/// Знайти кінець header-блоку: (offset закриваючого `---`, offset тіла одразу після нього)
fn find_header_end(s: &str) -> Option<(usize, usize)> {
    for (line_start, line) in line_spans(s) {
        let t = line.trim();
        if t == "---" {
            // тіло починається одразу після закриваючого "---" (разом з його \n, якщо є)
            let body = line_start + line.len();
            return Some((line_start, body));
        }
    }
    None
}

/// (offset початку, рядок) для всіх рядків
fn line_spans(s: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    for line in s.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').map(|l| l.strip_suffix('\r').map(|l2| l2).unwrap_or(l)).unwrap_or(line);
        out.push((start, trimmed));
        start += line.len();
    }
    out
}

fn split_kv(line: &str) -> Option<(String, String)> {
    let (k, v) = line.split_once(':')?;
    let k = k.trim();
    if k.is_empty() || k.contains(' ') {
        return None;
    }
    Some((k.to_string(), v.trim().trim_matches('"').to_string()))
}

// --- QA-01: захист від пошкодження з'єднання frontmatter ↔ тіло ---

/// Хірургічний сплайсинг тіла: header-блок (frontmatter) байт-в-байт,
/// тіло — з ГАРАНТОВАНИМ переносом рядка після закриваючого `---`.
///
/// QA-01: тіло без початкового порожнього рядка («# Заголовок») НЕ має
/// зливатися з роздільником — `---# Заголовок` робить файл непарсуваним
/// (frontmatter «протікає» в текст, word_count рахує метадані).
/// Файл без frontmatter повертає тіло як є.
pub fn splice_body(raw: &str, body: &str) -> String {
    let parsed = parse(raw);
    if parsed.body_start == 0 {
        // frontmatter немає — захищати з'єднання нічого
        return body.to_string();
    }
    // body_start вказує на перенос після закриваючого `---` (або на кінець
    // файла, якщо переносу не було) — head закінчується самим `---`.
    let head = &raw[..parsed.body_start];
    let mut out = String::with_capacity(head.len() + body.len() + 1);
    out.push_str(head);
    // Якщо тіло не починається з нового рядка — додаемо один: перенос
    // закриваючого `---` зобов'язаний бути перед першим символом тіла.
    if !(body.starts_with('\n') || body.starts_with("\r\n")) {
        out.push('\n');
    }
    out.push_str(body);
    out
}

/// Захисний інваріант QA-01: після сплайсинга frontmatter має парситися
/// так само, як до нього (мінімум — title не загублено). `false` означає:
/// такий вміст записувати НЕ можна (з'єднання пошкоджене).
pub fn title_preserved(before: &str, after: &str) -> bool {
    let was = parse(before);
    if was.meta.is_empty() {
        return true; // frontmatter не було — захищати нічого
    }
    let now = parse(after);
    if now.meta.is_empty() {
        return false; // frontmatter перестав парситися
    }
    // усі ключі, що були, мають зберегтися з тим самим значенням
    was.meta.iter().all(|(k, v)| now.meta.get(k) == Some(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let doc = "---\ntitle: Розділ 1\nlang: uk\n---\n\nТіло тексту.";
        let p = parse(doc);
        assert_eq!(p.meta.get("title").unwrap(), "Розділ 1");
        assert_eq!(p.meta.get("lang").unwrap(), "uk");
        assert!(p.body.contains("Тіло тексту"));
    }

    #[test]
    fn no_frontmatter() {
        let p = parse("Просто текст\n---\nтеж текст");
        assert!(p.meta.is_empty());
        assert_eq!(p.body_start, 0);
    }

    #[test]
    fn splice_body_adds_separator_before_hash_body() {
        let raw = "---\ntitle: QA Канон\nlang: uk\n---\n\nСтаре.\n";
        // тіло від редактора БЕЗ початкового порожнього рядка
        let out = splice_body(raw, "# QA Канон\n\nНовий текст.");
        let p = parse(&out);
        assert_eq!(p.meta.get("title").unwrap(), "QA Канон");
        assert_eq!(p.meta.get("lang").unwrap(), "uk");
        assert!(p.body.contains("# QA Канон"));
        assert!(!out.contains("---#"));
        // round-trip: повторний сплайсинг не нарощує порожні рядки
        let out2 = splice_body(&out, "# QA Канон\n\nЩе новіший текст.");
        assert_eq!(parse(&out2).meta.get("title").unwrap(), "QA Канон");
    }

    #[test]
    fn splice_body_keeps_leading_newline_body() {
        let raw = "---\ntitle: T\n---\n";
        let out = splice_body(raw, "\n\nТіло.");
        assert_eq!(out, "---\ntitle: T\n---\n\nТіло.");
    }

    #[test]
    fn merged_separator_is_broken_and_caught_by_invariant() {
        let broken = "---\ntitle: QA Канон\n---# тіло, змите з роздільником";
        // на цьому й ловиться баг QA-01: злитий роздільник = frontmatter зник
        assert!(parse(broken).meta.is_empty());
        assert!(!title_preserved(
            "---\ntitle: QA Канон\n---\n\nок",
            broken
        ));
        assert!(title_preserved(
            "---\ntitle: QA Канон\n---\n\nок",
            "---\ntitle: QA Канон\n---\n\nінше ок"
        ));
        // без frontmatter інваріант нічого не вимагає
        assert!(title_preserved("просто текст", "інший текст"));
    }
}
