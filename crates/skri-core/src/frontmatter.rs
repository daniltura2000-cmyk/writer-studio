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
}
