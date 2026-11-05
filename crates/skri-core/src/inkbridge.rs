//! Round-trip міст Ink/Yarn ↔ text-first markdown (M3, §5.7 «Гейм-наратив»).
//!
//! Лікує досліджений біль:
//! - *"I'd need to retype everything by hand... soul-crushing"* — імпорт
//!   .ink/.yarn у робочі документи (dialogue/*.md) і експорт назад;
//! - зміни, зроблені в коді гри (git diff імпортованого файлу),
//!   підсвічуються як «вхідні правки» — алерт `ink-external-change`.
//!
//! ## Нормалізований text-first формат (markdown)
//! Сумісний із [`crate::narrative`] (scan_dialogue / graph_audit / var_audit):
//! ```text
//! ## назва_ноди
//!
//! -> назва_ноди
//! Проза репліки.
//! %SET gold = 5
//! %CHECK метка
//! [goto інша_нода]
//! * текст опції [goto ціль]
//! ```
//!
//! ## Підмножина синтаксису (чесно обмежена)
//! **Ink:** ноди `== knot` (декор `===` навколо ігнорується), `VAR x = v`,
//! `~ x = v` / `~ x += v` / `~ x -= v` (та `~ x`), standalone-диверти
//! `-> target`, диверти в кінці рядка (проза й вибори `* [текст] -> target`),
//! whole-line умови `{ x }`. Інлайн-умови `{...}` всередині речення лишаються
//! прозою як є (round-trip зберігає їх дослівно).
//!
//! **Yarn:** ноди `title: ...` / `---` / `===`, посилання `[[Target]]` та
//! `[[Текст|Target]]`, команди `<<set $x = v>>` (і форма `to`),
//! `<<if $x>>`. `<<endif>>` / `<<else>>` лишаються прозою як є.
//!
//! Змінні: Ink `VAR` → frontmatter `ink-vars:`; set/check-операції в обох
//! форматах → маркери `%SET` / `%CHECK` (значення зберігаються).

use crate::store::ProjectStore;
use crate::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

// --- Формат і документ ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Ink,
    Yarn,
}

impl Format {
    pub fn from_extension(ext: &str) -> Result<Format> {
        match ext {
            "ink" => Ok(Format::Ink),
            "yarn" => Ok(Format::Yarn),
            other => Err(Error::Invalid(format!(
                "невідомий формат діалогу «{other}» (очікую .ink або .yarn)"
            ))),
        }
    }
}

/// Нода діалогу в нормалізованому text-first markdown.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogueNode {
    pub name: String,
    /// рядки тіла: проза, `[goto X]`, `%SET/%CHECK`, опції `* ...`
    pub lines: Vec<String>,
}

/// Розібраний діалоговий документ.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DialogueDoc {
    pub nodes: Vec<DialogueNode>,
    /// початкові значення змінних (Ink `VAR x = 5`)
    pub vars: Vec<(String, String)>,
}

/// Семантичний підпис документа — для round-trip-порівнянь у тестах.
pub fn semantic_signature(doc: &DialogueDoc) -> String {
    let mut out = String::new();
    for v in &doc.vars {
        out.push_str(&format!("VAR {v:?}\n"));
    }
    for n in &doc.nodes {
        out.push_str(&format!("## {}\n", n.name));
        for l in &n.lines {
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

// --- Імпорт: Ink ---

/// Розібрати .ink-джерело в нормалізований документ.
pub fn parse_ink(src: &str) -> Result<DialogueDoc> {
    let mut doc = DialogueDoc::default();
    let mut cur: Option<DialogueNode> = None;
    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.chars().all(|c| c == '=') {
            continue; // декоративні роздільники
        }
        if let Some(name) = knot_header(line) {
            if let Some(prev) = cur.take() {
                doc.nodes.push(prev);
            }
            cur = Some(DialogueNode { name, lines: Vec::new() });
            continue;
        }
        if let Some(rest) = line.strip_prefix("VAR ") {
            if let Some((n, _, v)) = split_assign(rest) {
                doc.vars.push((n, v));
            }
            continue;
        }
        let Some(node) = cur.as_mut() else {
            // проза до першої ноди — поза підмножиною
            continue;
        };
        if let Some(rest) = line.strip_prefix('~') {
            let rest = rest.trim().strip_prefix("temp ").unwrap_or(rest.trim()).trim();
            if rest.is_empty() {
                continue;
            }
            node.lines.push(match split_assign(rest) {
                Some((n, _, v)) => set_line(&n, Some(&v)),
                None => set_line(rest, None),
            });
            continue;
        }
        if let Some(name) = bare_check(line) {
            node.lines.push(format!("%CHECK {name}"));
            continue;
        }
        node.lines.push(convert_ink_line(line));
    }
    if let Some(prev) = cur.take() {
        doc.nodes.push(prev);
    }
    if doc.nodes.is_empty() {
        return Err(Error::Invalid("Ink: не знайдено жодної ноди (== knot)".into()));
    }
    Ok(doc)
}

/// Заголовок ноди: `== knot`, `=== knot ===`. Ім'я — без пробілів.
fn knot_header(line: &str) -> Option<String> {
    if !line.starts_with("==") {
        return None;
    }
    let name = line.trim_start_matches('=').trim();
    let name = name.trim_end_matches('=').trim();
    if name.is_empty() || name.split_whitespace().count() != 1 {
        return None;
    }
    Some(name.to_string())
}

/// `{ x }` (весь рядок) → перевірка змінної.
fn bare_check(line: &str) -> Option<String> {
    let inner = line.strip_prefix('{')?.strip_suffix('}')?.trim();
    if inner.is_empty()
        || inner.contains('=')
        || inner.contains(':')
        || inner.split_whitespace().count() != 1
    {
        return None;
    }
    Some(inner.to_string())
}

/// `x = 5` / `x += 5` / `x -= 5` → (ім'я, оператор, значення).
fn split_assign(s: &str) -> Option<(String, String, String)> {
    let eq = s.find('=')?;
    let head = &s[..eq];
    let (name, op) = if let Some(stripped) = head.strip_suffix('+') {
        (stripped.trim(), "+=")
    } else if let Some(stripped) = head.strip_suffix('-') {
        (stripped.trim(), "-=")
    } else {
        (head.trim(), "=")
    };
    if name.is_empty() || name.split_whitespace().count() != 1 {
        return None;
    }
    let value = s[eq + 1..].trim();
    if value.is_empty() {
        return None;
    }
    Some((name.to_string(), op.to_string(), value.to_string()))
}

/// Маркер %SET: голе ім'я для `true`/порожнього, з = для значень.
fn set_line(name: &str, value: Option<&str>) -> String {
    match value.map(str::trim) {
        None | Some("") | Some("true") => format!("%SET {name}"),
        Some(v) => format!("%SET {name} = {v}"),
    }
}

fn is_simple_target(s: &str) -> bool {
    !s.is_empty() && !s.contains(' ') && !s.contains(']') && !s.contains('[')
}

/// `проза -> ціль` у кінці рядка → (проза, ціль).
fn split_trailing_divert(line: &str) -> Option<(String, String)> {
    let idx = line.rfind(" -> ")?;
    let target = line[idx + 4..].trim();
    if !is_simple_target(target) {
        return None;
    }
    Some((line[..idx].trim().to_string(), target.to_string()))
}

/// Заголовок опції: `* [текст]` / `* текст` → "* текст".
fn choice_head(head: &str) -> String {
    let star = &head[..1];
    let rest = head[1..].trim_start();
    if let Some(end) = rest.strip_prefix('[').and_then(|_| rest.find(']')) {
        format!("{star} {}", rest[1..end].trim())
    } else {
        format!("{star} {rest}")
    }
}

fn convert_ink_line(line: &str) -> String {
    // standalone-диверт
    if let Some(target) = line.strip_prefix("-> ").map(str::trim).filter(|t| is_simple_target(t)) {
        return format!("[goto {target}]");
    }
    // диверт наприкінці рядка (проза або вибір)
    if let Some((head, target)) = split_trailing_divert(line) {
        if head.starts_with('*') || head.starts_with('+') {
            return format!("{} [goto {target}]", choice_head(&head));
        }
        return format!("{head} [goto {target}]");
    }
    // вибір без диверту
    if line.starts_with('*') || line.starts_with('+') {
        return choice_head(line);
    }
    line.to_string()
}

// --- Імпорт: Yarn ---

/// Розібрати .yarn-джерело в нормалізований документ.
pub fn parse_yarn(src: &str) -> Result<DialogueDoc> {
    let mut doc = DialogueDoc::default();
    let mut cur: Option<DialogueNode> = None;
    let mut in_header = true;
    for raw in src.lines() {
        let t = raw.trim();
        if in_header {
            if let Some(name) = t.strip_prefix("title:").map(str::trim) {
                if let Some(prev) = cur.take() {
                    doc.nodes.push(prev);
                }
                cur = Some(DialogueNode { name: name.to_string(), lines: Vec::new() });
            } else if t == "---" {
                in_header = false;
            }
            // інші заголовки (position, color, tags) — поза підмножиною
            continue;
        }
        if t == "===" {
            in_header = true;
            if let Some(prev) = cur.take() {
                doc.nodes.push(prev);
            }
            continue;
        }
        if t.is_empty() {
            continue;
        }
        let Some(node) = cur.as_mut() else { continue };
        node.lines.push(convert_yarn_line(t));
    }
    if let Some(prev) = cur.take() {
        doc.nodes.push(prev);
    }
    if doc.nodes.is_empty() {
        return Err(Error::Invalid("Yarn: не знайдено жодної ноди (title:)".into()));
    }
    Ok(doc)
}

fn convert_yarn_line(t: &str) -> String {
    if let Some(rest) = t.strip_prefix("<<set ") {
        let rest = rest.trim_end_matches(">>").trim();
        let rest = rest.strip_prefix('$').unwrap_or(rest).trim();
        // форми `x = v` та `x to v`
        let pair = split_assign(rest).or_else(|| {
            rest.find(" to ").and_then(|i| {
                let (n, v) = (rest[..i].trim(), rest[i + 4..].trim());
                if n.split_whitespace().count() == 1 && !v.is_empty() {
                    Some((n.to_string(), "=".to_string(), v.to_string()))
                } else {
                    None
                }
            })
        });
        return match pair {
            Some((n, _, v)) => set_line(&n, Some(&v)),
            None if !rest.is_empty() => set_line(rest, None),
            None => t.to_string(),
        };
    }
    if let Some(rest) = t.strip_prefix("<<if ") {
        let inner = rest.trim_end_matches(">>").trim();
        let name = inner.strip_prefix('$').unwrap_or(inner).trim();
        if is_simple_target(name) {
            return format!("%CHECK {name}");
        }
        return t.to_string(); // складна умова — лишається прозою
    }
    replace_yarn_links(t)
}

/// `[[Target]]` → `[goto Target]`; `[[Текст|Target]]` → `Текст [goto Target]`.
fn replace_yarn_links(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(start) = rest.find("[[") {
        let Some(end_rel) = rest[start + 2..].find("]]") else { break };
        let inner = &rest[start + 2..start + 2 + end_rel];
        out.push_str(&rest[..start]);
        match inner.split_once('|') {
            Some((text, target)) => {
                let target = target.trim();
                if is_simple_target(target) {
                    out.push_str(&format!("{} [goto {target}]", text.trim()));
                } else {
                    out.push_str(&format!("[[{inner}]]"));
                }
            }
            None => {
                if is_simple_target(inner.trim()) {
                    out.push_str(&format!("[goto {}]", inner.trim()));
                } else {
                    out.push_str(&format!("[[{inner}]]"));
                }
            }
        }
        rest = &rest[start + 2 + end_rel + 2..];
    }
    out.push_str(rest);
    out
}

// --- Markdown (нормалізований text-first) ---

/// Документ → markdown (з frontmatter `ink-vars:` за потреби).
pub fn to_markdown(doc: &DialogueDoc) -> String {
    let mut out = String::new();
    if !doc.vars.is_empty() {
        let joined: Vec<String> = doc.vars.iter().map(|(n, v)| format!("{n}={v}")).collect();
        out.push_str(&format!("---\ntitle: діалог\nink-vars: {}\n---\n\n", joined.join("; ")));
    }
    for (i, n) in doc.nodes.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("## {}\n\n", n.name));
        out.push_str(&format!("-> {}\n", n.name));
        for l in &n.lines {
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

/// Розібрати markdown-ТІЛО (без frontmatter) назад у документ.
pub fn parse_markdown(body: &str) -> Result<DialogueDoc> {
    let mut nodes: Vec<DialogueNode> = Vec::new();
    let mut cur: Option<DialogueNode> = None;
    for raw in body.lines() {
        let t = raw.trim();
        if let Some(name) = t.strip_prefix("## ") {
            if let Some(prev) = cur.take() {
                nodes.push(prev);
            }
            cur = Some(DialogueNode { name: name.trim().to_string(), lines: Vec::new() });
            continue;
        }
        let Some(node) = cur.as_mut() else { continue };
        if t.is_empty() {
            continue;
        }
        if let Some(name) = t.strip_prefix("-> ").map(str::trim) {
            // маркер входу ноди, згенерований to_markdown
            if node.name == name && node.lines.is_empty() {
                continue;
            }
            node.lines.push(format!("[goto {name}]"));
            continue;
        }
        node.lines.push(t.to_string());
    }
    if let Some(prev) = cur.take() {
        nodes.push(prev);
    }
    if nodes.is_empty() {
        return Err(Error::Invalid("markdown: не знайдено жодної ноди (## назва)".into()));
    }
    Ok(DialogueDoc { nodes, vars: Vec::new() })
}

/// Розібрати ПОВНИЙ markdown-файл (frontmatter `ink-vars:` + тіло).
pub fn parse_markdown_file(raw: &str) -> Result<DialogueDoc> {
    let parsed = crate::frontmatter::parse(raw);
    let mut doc = parse_markdown(&parsed.body)?;
    if let Some(vars_raw) = parsed.meta.get("ink-vars") {
        doc.vars = vars_raw
            .split(';')
            .filter_map(|piece| {
                let (n, v) = piece.trim().split_once('=')?;
                let n = n.trim();
                if n.is_empty() {
                    None
                } else {
                    Some((n.to_string(), v.trim().to_string()))
                }
            })
            .collect();
    }
    Ok(doc)
}

// --- Експорт: markdown → Ink / Yarn ---

/// Документ → вихідний файл обраного формату.
pub fn export_source(doc: &DialogueDoc, fmt: Format) -> Result<String> {
    match fmt {
        Format::Ink => Ok(export_ink(doc)),
        Format::Yarn => Ok(export_yarn(doc)),
    }
}

fn export_ink(doc: &DialogueDoc) -> String {
    let mut out = String::new();
    for (n, v) in &doc.vars {
        out.push_str(&format!("VAR {n} = {v}\n"));
    }
    if !doc.vars.is_empty() {
        out.push('\n');
    }
    for node in &doc.nodes {
        out.push_str(&format!("== {} ===\n", node.name));
        for line in &node.lines {
            out.push_str(&md_line_to_ink(line));
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

fn export_yarn(doc: &DialogueDoc) -> String {
    let mut out = String::new();
    for (i, node) in doc.nodes.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("title: {}\n---\n", node.name));
        for line in &node.lines {
            out.push_str(&md_line_to_yarn(line));
            out.push('\n');
        }
        out.push_str("===\n");
    }
    out
}

/// Усі `[goto X]` у рядку → список цілей.
fn extract_gotos(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(rel) = line[search..].find("[goto ") {
        let abs = search + rel + 6;
        if let Some(end) = line[abs..].find(']') {
            let target = line[abs..abs + end].trim().to_string();
            if !target.is_empty() {
                out.push(target);
            }
            search = abs + end;
        } else {
            break;
        }
    }
    out
}

fn md_line_to_ink(line: &str) -> String {
    if let Some(rest) = line.strip_prefix("%SET ") {
        let rest = rest.trim();
        return match split_assign(rest) {
            Some((n, op, v)) => format!("~ {n} {op} {v}"),
            None => format!("~ {} = true", rest),
        };
    }
    if let Some(rest) = line.strip_prefix("%CHECK ") {
        return format!("{{ {} }}", rest.trim());
    }
    if let Some(rest) = line.strip_prefix("%UNSET ") {
        return format!("~ {} = false", rest.trim());
    }
    let gotos = extract_gotos(line);
    if gotos.is_empty() {
        return line.to_string();
    }
    let head = {
        let mut h = line.to_string();
        for g in &gotos {
            h = h.replace(&format!("[goto {g}]"), "");
        }
        h.trim_end().to_string()
    };
    let target = gotos.last().unwrap();
    if line.starts_with("* ") || line.starts_with("+ ") {
        return format!("* [{}] -> {}", head[1..].trim(), target);
    }
    if head.is_empty() {
        return format!("-> {target}");
    }
    format!("{head} -> {target}")
}

fn md_line_to_yarn(line: &str) -> String {
    if let Some(rest) = line.strip_prefix("%SET ") {
        let rest = rest.trim();
        return match split_assign(rest) {
            Some((n, op, v)) => format!("<<set ${n} {op} {v}>>"),
            None => format!("<<set ${} = true>>", rest),
        };
    }
    if let Some(rest) = line.strip_prefix("%CHECK ") {
        return format!("<<if ${}>>", rest.trim());
    }
    if let Some(rest) = line.strip_prefix("%UNSET ") {
        return format!("<<set ${} = false>>", rest.trim());
    }
    let gotos = extract_gotos(line);
    if gotos.is_empty() {
        return line.to_string();
    }
    let mut out = line.to_string();
    for g in &gotos {
        out = out.replace(&format!("[goto {g}]"), &format!("[[{g}]]"));
    }
    out
}

// --- Хеш (SHA-256, std-only: без нових залежностей) ---

const SHA_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
    0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
    0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
    0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
    0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
    0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
    0xc67178f2,
];

/// SHA-256 від байтів ( hex-рядок ). std-only — без нових залежностей.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

// --- Імпорт/експорт у проєкт + детект зовнішніх змін ---

/// Результат імпорту файлу діалогу.
#[derive(Debug)]
pub struct ImportReport {
    pub doc_path: String,
    pub nodes: usize,
    pub vars: usize,
    /// файл змінився з останнього імпорту → алерт «вхідні правки»
    pub external_change: bool,
}

fn ensure_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ink_imports (
           src_path TEXT PRIMARY KEY,
           hash TEXT NOT NULL,
           doc_path TEXT NOT NULL,
           imported_at INTEGER NOT NULL
         );",
    )?;
    Ok(())
}

/// Імпортувати .ink/.yarn у `dialogue/<стем>.md`.
///
/// Хеш файлу зберігається в `ink_imports`; якщо файл змінився з останнього
/// імпорту (правки з боку рушія/інших розробників) — пишеться алерт
/// kind='ink-external-change' з payload (src, old_hash, new_hash).
pub fn import_file(store: &ProjectStore, src: &Path) -> Result<ImportReport> {
    let bytes = std::fs::read(src)?;
    let fmt = Format::from_extension(
        src.extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default(),
    )?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let doc = match fmt {
        Format::Ink => parse_ink(&text)?,
        Format::Yarn => parse_yarn(&text)?,
    };
    let md = to_markdown(&doc);

    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::Invalid("некоректне ім'я файлу діалогу".into()))?;
    let dialogue_dir = store.root.join("dialogue");
    std::fs::create_dir_all(&dialogue_dir)?;
    let doc_fs = dialogue_dir.join(format!("{stem}.md"));
    std::fs::write(&doc_fs, &md)?;
    let doc_path = format!("dialogue/{stem}.md");

    let hash = sha256_hex(&bytes);
    let conn = store.conn();
    ensure_tables(conn)?;
    let src_key = src.to_string_lossy().replace('\\', "/");

    let old: Option<String> = conn
        .query_row(
            "SELECT hash FROM ink_imports WHERE src_path=?1",
            params![&src_key],
            |r| r.get(0),
        )
        .optional()?;
    let mut external_change = false;
    if let Some(old_hash) = old {
        if old_hash != hash {
            external_change = true;
            let payload = serde_json::json!({
                "src": src_key,
                "doc": doc_path,
                "old_hash": old_hash,
                "new_hash": hash,
                "message": "Імпортований діалог змінено зовні (напр., git diff інших) — звірте «вхідні правки» перед прийняттям"
            });
            conn.execute(
                "INSERT INTO alerts(kind, entity_id, payload_json, created_at)
                 VALUES ('ink-external-change', NULL, ?1, strftime('%s','now'))",
                params![serde_json::to_string(&payload)?],
            )?;
        }
    }
    conn.execute(
        "INSERT INTO ink_imports(src_path, hash, doc_path, imported_at)
         VALUES (?1,?2,?3,strftime('%s','now'))
         ON CONFLICT(src_path) DO UPDATE SET
           hash=excluded.hash, doc_path=excluded.doc_path, imported_at=excluded.imported_at",
        params![src_key, hash, doc_path],
    )?;

    Ok(ImportReport {
        doc_path,
        nodes: doc.nodes.len(),
        vars: doc.vars.len(),
        external_change,
    })
}

/// Експортувати markdown-документ проєкту назад у .ink/.yarn.
pub fn export_file(store: &ProjectStore, doc_rel: &str, out: &Path) -> Result<()> {
    let fmt = Format::from_extension(
        out.extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default(),
    )?;
    let raw = std::fs::read_to_string(store.root.join(doc_rel))?;
    let doc = parse_markdown_file(&raw)?;
    std::fs::write(out, export_source(&doc, fmt)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
