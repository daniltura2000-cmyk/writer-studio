//! EPUB-експорт з metadata scrubber (pen name management) — потреба №15
//! дослідження `docs/research/unmet_needs_niche_authors.md`:
//! *«you don't want to go through all this effort only to discover your
//! software has automatically inserted your government name into a
//! watermark field»*.
//!
//! - **Збірка EPUB 3** з `manuscript/*.md` (порядок як у release.rs: ведучі
//!   цифри імені файла, інакше сорт за шляхом). ZIP — методом STORED
//!   (compression method 0; для EPUB це валідно), std-only: власний
//!   CRC-32 + записувач ZIP. `mimetype` — ПЕРШИМ записом, без extra-поля.
//! - **Scrub full (за замовчуванням)**: з метаданих прибирається все, що
//!   ідентифікує реального автора — `dc:creator` = тільки pen name,
//!   `dc:publisher` — геть, у `dc:description` реальні імена замінюються
//!   pen name'ом, `meta generator` прибирається з кожного XHTML, реальне
//!   ім'я замінюється і в тілі глав (copyright-рядок теж витік би).
//!   `dc:date` = тільки дата публікації, яку вказав користувач.
//! - **Scrub light**: звичайні метадані лишаються (generator, publisher,
//!   реальний автор — як є).
//! - **Scrub-звіт** — чесний список того, що саме видалено/замінено
//!   («dc:creator: Вялков В. → Ash Thornwood», «generator: _rm»);
//!   зберігається в `.skri/scrub-report.json` (останній звіт) + таблиця
//!   `epub_exports` (pen_name, scrub_mode, sha256).
//!
//! UUID ідентифікатора детермінований (похідний від вмісту, sha256),
//! тож дві збірки того самого рукопису дають байт-в-байт однаковий EPUB.
//!
//! Публічні API інших модулів не змінюються; markdown→HTML — реюз
//! `crosspost::inline_html` (escape + bold/italic) + свій блоковий рівень
//! (заголовки #/##/###, --- → <hr/>, параграфи).

use crate::frontmatter;
use crate::store::ProjectStore;
use crate::{Error, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Режим scrub метаданих.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrubMode {
    /// Видалити все, що ідентифікує реального автора (за замовчуванням).
    Full,
    /// Лиши звичайні метадані (generator, publisher, реальний автор).
    Light,
}

impl ScrubMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScrubMode::Full => "full",
            ScrubMode::Light => "light",
        }
    }
    pub fn parse(s: &str) -> Option<ScrubMode> {
        match s {
            "full" => Some(ScrubMode::Full),
            "light" => Some(ScrubMode::Light),
            _ => None,
        }
    }
}

/// Опції збірки EPUB.
#[derive(Debug, Clone)]
pub struct ScrubOptions {
    pub mode: ScrubMode,
    /// pen name; None → frontmatter `pen_name` → налаштування проєкту.
    pub pen_name: Option<String>,
    /// дата публікації "YYYY-MM-DD"; None → frontmatter `pub_date` → сьогодні.
    pub date: Option<String>,
}

impl Default for ScrubOptions {
    fn default() -> Self {
        ScrubOptions {
            mode: ScrubMode::Full,
            pen_name: None,
            date: None,
        }
    }
}

/// Scrub-звіт: що саме було видалено/замінено (чесність перед автором).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrubReport {
    pub mode: String,
    pub pen_name: String,
    pub title: String,
    pub date: String,
    pub language: String,
    /// doc_path глав, що увійшли в EPUB
    pub chapters: Vec<String>,
    /// список замін/видалень у форматі «поле: було → стало» / «поле: _rm»
    pub changes: Vec<String>,
    pub output: String,
    pub sha256: String,
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS epub_exports (
  id INTEGER PRIMARY KEY,
  doc_paths_json TEXT NOT NULL DEFAULT '[]',
  pen_name TEXT NOT NULL DEFAULT '',
  scrub_mode TEXT NOT NULL DEFAULT 'full',
  created_at INTEGER NOT NULL,
  sha256 TEXT NOT NULL DEFAULT ''
);
"#;

const CONTAINER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

const MIMETYPE: &str = "application/epub+zip";

/// Зібрати EPUB з `manuscript/*.md` і повернути scrub-звіт.
/// Звіт також зберігається в `.skri/scrub-report.json` (останній звіт)
/// і рядком у таблиці `epub_exports`.
pub fn build(store: &ProjectStore, out_path: &Path, opts: &ScrubOptions) -> Result<ScrubReport> {
    // файли — єдине джерело правди (залізне правило №1)
    store.reindex()?;
    let conn = store.conn();
    conn.execute_batch(SCHEMA)?;

    let paths: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT path FROM docs WHERE kind='manuscript' AND path LIKE 'manuscript/%' ORDER BY path",
        )?;
        let rows = stmt
            .query_map([], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };
    // порядок як у release.rs: ведучі цифри імені файла, інакше сорт за шляхом
    let mut chapters: Vec<(i64, String)> = paths
        .into_iter()
        .map(|p| (chapter_number(&p).unwrap_or(i64::MAX), p))
        .collect();
    chapters.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    if chapters.is_empty() {
        return Err(Error::Invalid(
            "у manuscript/ немає жодної глави — збирати нічого".into(),
        ));
    }

    // --- читаємо глави і збираємо метадані з frontmatter ---
    struct ChapterSrc {
        doc_path: String,
        meta: std::collections::BTreeMap<String, String>,
        body: String,
    }
    let mut srcs: Vec<ChapterSrc> = Vec::new();
    for (_, rel) in &chapters {
        let raw = std::fs::read_to_string(store.root.join(rel))?;
        let parsed = frontmatter::parse(&raw);
        srcs.push(ChapterSrc {
            doc_path: rel.clone(),
            meta: parsed.meta,
            body: parsed.body,
        });
    }

    let real_author: Option<String> = srcs
        .iter()
        .find_map(|c| {
            c.meta
                .get("author")
                .or_else(|| c.meta.get("автор"))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    let publisher: Option<String> = srcs
        .iter()
        .find_map(|c| {
            c.meta
                .get("publisher")
                .or_else(|| c.meta.get("видавець"))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    let description: Option<String> = srcs
        .iter()
        .find_map(|c| {
            c.meta
                .get("description")
                .or_else(|| c.meta.get("опис"))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });

    let pen_name = opts
        .pen_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            srcs.iter().find_map(|c| {
                c.meta
                    .get("pen_name")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            })
        })
        .or_else(|| {
            store
                .settings()
                .ok()
                .and_then(|s| s.get("pen_name").and_then(|v| v.as_str()).map(String::from))
        })
        .unwrap_or_else(|| "Автор".to_string());

    let language = srcs
        .first()
        .and_then(|c| c.meta.get("lang"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            store
                .settings()
                .ok()
                .and_then(|s| s.get("language").and_then(|v| v.as_str()).map(String::from))
        })
        .unwrap_or_else(|| "uk".to_string());

    // дата публікації: вказана користувачем → frontmatter першої глави → сьогодні
    let date = match &opts.date {
        Some(d) => {
            if crate::release::parse_ymd(d).is_none() {
                return Err(Error::Invalid(format!(
                    "дата публікації: очікується YYYY-MM-DD, є «{d}»"
                )));
            }
            d.trim().to_string()
        }
        None => srcs
            .first()
            .and_then(|c| c.meta.get("pub_date").cloned())
            .filter(|d| crate::release::parse_ymd(d).is_some())
            .unwrap_or_else(|| crate::release::fmt_date(now_epoch())),
    };

    // --- scrub full: заміни в тілі та description ---
    let full = opts.mode == ScrubMode::Full;
    let mut changes: Vec<String> = Vec::new();
    let mut body_replacements = 0usize;
    if full {
        if let Some(real) = &real_author {
            for c in &mut srcs {
                let n = c.body.matches(real.as_str()).count();
                if n > 0 {
                    c.body = c.body.replace(real.as_str(), &pen_name);
                    body_replacements += n;
                }
            }
        }
    }

    // назва книги: frontmatter title першої глави → перший # заголовок → ім'я файла
    let book_title = srcs
        .first()
        .and_then(|c| c.meta.get("title").map(|s| s.trim()))
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| first_h1(&srcs[0].body))
        .unwrap_or_else(|| {
            srcs[0]
                .doc_path
                .rsplit('/')
                .next()
                .unwrap_or("book")
                .trim_end_matches(".md")
                .to_string()
        });

    // опис: у full — реальні імена → pen name
    let description_final = match &description {
        Some(d) if full => {
            if let Some(real) = &real_author {
                if d.contains(real.as_str()) {
                    changes.push(format!("dc:description: {real} → {pen_name}"));
                    d.replace(real.as_str(), &pen_name)
                } else {
                    d.clone()
                }
            } else {
                d.clone()
            }
        }
        Some(d) => d.clone(),
        None => String::new(),
    };

    // --- звіт замін ---
    if full {
        match &real_author {
            Some(real) => changes.push(format!("dc:creator: {real} → {pen_name}")),
            None => changes.push(format!("dc:creator: {pen_name} (pen name)")),
        }
        if let Some(publ) = &publisher {
            changes.push(format!("dc:publisher: {publ} → _rm"));
        }
        changes.push(format!(
            "generator: _rm (meta generator прибрано з {} XHTML)",
            srcs.len()
        ));
        if body_replacements > 0 {
            let real = real_author.clone().unwrap_or_default();
            changes.push(format!(
                "тіло глав: «{real}» → «{pen_name}» ({body_replacements} замін)"
            ));
        }
        // frontmatter-поля, що ідентифікують автора, в EPUB не потрапляють
        let mut seen_keys: Vec<&str> = Vec::new();
        for c in &srcs {
            for k in ["email", "url", "www"] {
                if c.meta.contains_key(k) && !seen_keys.contains(&k) {
                    seen_keys.push(k);
                    changes.push(format!("frontmatter.{k}: _rm (не потрапляє в EPUB)"));
                }
            }
        }
    } else {
        changes.push("режим light: звичайні метадані збережено".to_string());
        if let Some(real) = &real_author {
            changes.push(format!("dc:creator: {real} (реальний автор, режим light)"));
        }
        changes.push("generator: залишено (light)".to_string());
    }

    // --- XHTML глав ---
    let mut zip_entries: Vec<(&str, Vec<u8>)> = Vec::new();
    zip_entries.push(("mimetype", MIMETYPE.as_bytes().to_vec()));
    zip_entries.push(("META-INF/container.xml", CONTAINER_XML.as_bytes().to_vec()));

    let mut manifest_items = String::new();
    let mut spine_refs = String::new();
    let mut nav_items = String::new();
    let mut xhtml_names: Vec<(String, String)> = Vec::new(); // (ім'я в ZIP, xhtml)

    for (i, c) in srcs.iter().enumerate() {
        let id = format!("ch{}", i + 1);
        let file = format!("{id}.xhtml");
        let chap_title = c
            .meta
            .get("title")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .or_else(|| first_h1(&c.body))
            .unwrap_or_else(|| {
                c.doc_path
                    .rsplit('/')
                    .next()
                    .unwrap_or("глава")
                    .trim_end_matches(".md")
                    .to_string()
            });
        let body_html = md_to_xhtml_body(&c.body);
        let xhtml = chapter_xhtml(&chap_title, &language, &body_html, opts.mode);
        xhtml_names.push((file.clone(), xhtml));

        manifest_items.push_str(&format!(
            "    <item id=\"{id}\" href=\"{file}\" media-type=\"application/xhtml+xml\"/>\n"
        ));
        spine_refs.push_str(&format!("    <itemref idref=\"{id}\"/>\n"));
        nav_items.push_str(&format!(
            "      <li><a href=\"{file}\">{}</a></li>\n",
            xml_escape(&chap_title)
        ));
    }

    // --- content.opf ---
    let uuid = deterministic_uuid(&format!("{book_title}|{pen_name}|{}", srcs[0].doc_path));
    let mut opf = String::new();
    opf.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    opf.push_str(&format!(
        "<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"bookid\" xml:lang=\"{}\">\n",
        xml_attr(&language)
    ));
    opf.push_str("  <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n");
    opf.push_str(&format!(
        "    <dc:identifier id=\"bookid\">urn:uuid:{uuid}</dc:identifier>\n"
    ));
    opf.push_str(&format!(
        "    <dc:title>{}</dc:title>\n",
        xml_escape(&book_title)
    ));
    opf.push_str(&format!(
        "    <dc:language>{}</dc:language>\n",
        xml_escape(&language)
    ));
    let creator = if full {
        pen_name.clone()
    } else {
        real_author.clone().unwrap_or_else(|| pen_name.clone())
    };
    opf.push_str(&format!(
        "    <dc:creator>{}</dc:creator>\n",
        xml_escape(&creator)
    ));
    opf.push_str(&format!("    <dc:date>{}</dc:date>\n", xml_escape(&date)));
    if !description_final.is_empty() {
        opf.push_str(&format!(
            "    <dc:description>{}</dc:description>\n",
            xml_escape(&description_final)
        ));
    }
    // publisher: тільки в light (у full — scrub: реального видавця немає)
    if !full {
        if let Some(publ) = &publisher {
            opf.push_str(&format!(
                "    <dc:publisher>{}</dc:publisher>\n",
                xml_escape(publ)
            ));
        }
    }
    opf.push_str(&format!(
        "    <meta property=\"dcterms:modified\">{}T00:00:00Z</meta>\n",
        xml_escape(&date)
    ));
    opf.push_str("  </metadata>\n");
    opf.push_str("  <manifest>\n");
    opf.push_str(
        "    <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n",
    );
    opf.push_str(&manifest_items);
    opf.push_str("  </manifest>\n");
    opf.push_str("  <spine>\n    <itemref idref=\"nav\"/>\n");
    opf.push_str(&spine_refs);
    opf.push_str("  </spine>\n");
    opf.push_str("</package>\n");

    // --- nav.xhtml (EPUB3 toc) ---
    let nav = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <!DOCTYPE html>\n\
         <html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{}\">\n\
         <head><title>Зміст</title><meta charset=\"utf-8\"/></head>\n\
         <body>\n\
         <nav epub:type=\"toc\" id=\"toc\">\n\
         <ol>\n{}    </ol>\n\
         </nav>\n\
         </body>\n\
         </html>\n",
        xml_attr(&language),
        nav_items
    );

    zip_entries.push(("OEBPS/content.opf", opf.into_bytes()));
    zip_entries.push(("OEBPS/nav.xhtml", nav.into_bytes()));
    for (name, xhtml) in xhtml_names {
        let zname: &'static str = Box::leak(name.clone().into_boxed_str());
        zip_entries.push((zname, xhtml.into_bytes()));
    }

    // --- ZIP (STORED) + sha256 ---
    let borrowed: Vec<(&str, &[u8])> = zip_entries
        .iter()
        .map(|(n, d)| (*n, d.as_slice()))
        .collect();
    let zip = zip_stored(&borrowed);
    let sha = crate::inkbridge::sha256_hex(&zip);
    std::fs::write(out_path, &zip)?;

    let output = out_path.to_string_lossy().replace('\\', "/");
    let report = ScrubReport {
        mode: opts.mode.as_str().to_string(),
        pen_name: pen_name.clone(),
        title: book_title,
        date,
        language,
        chapters: chapters.iter().map(|(_, p)| p.clone()).collect(),
        changes,
        output: output.clone(),
        sha256: sha.clone(),
    };

    // останній звіт + журнал експортів
    let rep_dir = store.root.join(".skri");
    std::fs::create_dir_all(&rep_dir)?;
    std::fs::write(
        rep_dir.join("scrub-report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    conn.execute(
        "INSERT INTO epub_exports(doc_paths_json, pen_name, scrub_mode, created_at, sha256)
         VALUES (?1, ?2, ?3, strftime('%s','now'), ?4)",
        params![
            serde_json::to_string(&report.chapters)?,
            report.pen_name,
            report.mode,
            sha
        ],
    )?;

    Ok(report)
}

/// Останній scrub-звіт (`.skri/scrub-report.json`), якщо був.
pub fn last_report(store: &ProjectStore) -> Result<Option<ScrubReport>> {
    let p = store.root.join(".skri").join("scrub-report.json");
    if !p.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&p)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

// --- markdown → XHTML (блоковий рівень; інлайн — реюз crosspost::inline_html) ---

/// Блоки markdown → XHTML-фрагмент: `#`/`##`/`###` заголовки, `---` → `<hr/>`,
/// параграфи (розділені порожнім рядком). Інлайн (escape, **bold**, *italic*)
/// — `crosspost::inline_html`; `&<>` екранується завжди.
pub fn md_to_xhtml_body(md: &str) -> String {
    let mut out = String::with_capacity(md.len() + 128);
    let mut para: Vec<String> = Vec::new();
    let flush = |para: &mut Vec<String>, out: &mut String| {
        if !para.is_empty() {
            out.push_str("<p>");
            out.push_str(&para.join("\n"));
            out.push_str("</p>\n");
            para.clear();
        }
    };
    for line in md.lines() {
        let t = line.trim();
        if t.is_empty() {
            flush(&mut para, &mut out);
            continue;
        }
        if t == "---" {
            flush(&mut para, &mut out);
            out.push_str("<hr/>\n");
            continue;
        }
        for (prefix, tag) in [("### ", "h3"), ("## ", "h2"), ("# ", "h1")] {
            if let Some(h) = t.strip_prefix(prefix) {
                flush(&mut para, &mut out);
                out.push_str(&format!("<{tag}>{}</{tag}>\n", crate::crosspost::inline_html(h)));
                break;
            }
        }
        // рядок не був заголовком (strip_prefix не спрацював для жодного)
        if !(t.starts_with("# ") || t.starts_with("## ") || t.starts_with("### ")) {
            para.push(crate::crosspost::inline_html(t));
        }
    }
    flush(&mut para, &mut out);
    out
}

/// Повний XHTML однієї глави. У full-режимі `meta generator` НЕ пишеться.
pub fn chapter_xhtml(title: &str, lang: &str, body_html: &str, mode: ScrubMode) -> String {
    let generator = match mode {
        ScrubMode::Light => "  <meta name=\"generator\" content=\"Writer Studio (skri)\"/>\n",
        ScrubMode::Full => "",
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <!DOCTYPE html>\n\
         <html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{lang}\">\n\
         <head>\n\
         <title>{title}</title>\n\
         <meta charset=\"utf-8\"/>\n\
         {generator}\
         </head>\n\
         <body>\n\
         {body_html}\
         </body>\n\
         </html>\n",
        lang = xml_attr(lang),
        title = xml_escape(title),
        generator = generator,
        body_html = body_html,
    )
}

// --- XML екранування ---

/// Екранувати текстовий вузол XML/HTML: & < >.
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

/// Екранувати значення атрибута: + лапки.
pub fn xml_attr(s: &str) -> String {
    xml_escape(s).replace('"', "&quot;")
}

// --- детермінований UUID (v5-подібний, з sha256 вмісту) ---

/// UUID із sha256 seed: версія 5, RFC-4122 варіант. Детермінований —
/// дві збірки того самого рукопису → той самий ідентифікатор (і той самий файл).
pub fn deterministic_uuid(seed: &str) -> String {
    let h = crate::inkbridge::sha256_hex(seed.as_bytes());
    let mut b = [0u8; 16];
    for (i, slot) in b.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap_or(0);
    }
    b[6] = (b[6] & 0x0f) | 0x50; // version 5
    b[8] = (b[8] & 0x3f) | 0x80; // variant RFC 4122
    let hx: Vec<String> = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        hx[0..4].concat(),
        hx[4..6].concat(),
        hx[6..8].concat(),
        hx[8..10].concat(),
        hx[10..16].concat()
    )
}

// --- ZIP STORED (std-only) ---

/// CRC-32 (IEEE 802.3), std-only.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Записати ZIP методом STORED (compression method 0 — валідно для EPUB).
/// Порядок записів = порядок `entries` (mimetype мусить бути першим);
/// extra-полів немає, names — ASCII.
pub fn zip_stored(entries: &[(&str, &[u8])]) -> Vec<u8> {
    struct Central {
        name: Vec<u8>,
        crc: u32,
        size: u32,
        offset: u32,
    }
    let mut out: Vec<u8> = Vec::new();
    let mut centrals: Vec<Central> = Vec::new();
    for (name, data) in entries {
        let nb = name.as_bytes();
        let crc = crc32(data);
        let size = data.len() as u32;
        let offset = out.len() as u32;
        // local file header
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method = STORED
        out.extend_from_slice(&0u16.to_le_bytes()); // mod time
        out.extend_from_slice(&0u16.to_le_bytes()); // mod date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes()); // compressed
        out.extend_from_slice(&size.to_le_bytes()); // uncompressed
        out.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len = 0
        out.extend_from_slice(nb);
        out.extend_from_slice(data);
        centrals.push(Central {
            name: nb.to_vec(),
            crc,
            size,
            offset,
        });
    }
    let cd_offset = out.len() as u32;
    for c in &centrals {
        out.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version made by
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method
        out.extend_from_slice(&0u16.to_le_bytes()); // time
        out.extend_from_slice(&0u16.to_le_bytes()); // date
        out.extend_from_slice(&c.crc.to_le_bytes());
        out.extend_from_slice(&c.size.to_le_bytes());
        out.extend_from_slice(&c.size.to_le_bytes());
        out.extend_from_slice(&(c.name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra
        out.extend_from_slice(&0u16.to_le_bytes()); // comment
        out.extend_from_slice(&0u16.to_le_bytes()); // disk start
        out.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        out.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        out.extend_from_slice(&c.offset.to_le_bytes());
        out.extend_from_slice(&c.name);
    }
    let cd_size = out.len() as u32 - cd_offset;
    let count = centrals.len() as u16;
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // disk
    out.extend_from_slice(&0u16.to_le_bytes()); // cd disk
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len
    out
}

// --- внутрішні хелпери ---

/// Номер глави з імені файла: ведучі цифри («03-розділ.md» → 3) — як у release.rs.
fn chapter_number(rel: &str) -> Option<i64> {
    let stem = rel.rsplit('/').next()?;
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Перший `# ` заголовок тіла (текст без `#`).
fn first_h1(body: &str) -> Option<String> {
    body.lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("# ").map(|h| h.trim().to_string()))
        .filter(|h| !h.is_empty())
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn uuid_deterministic_and_well_formed() {
        let a = deterministic_uuid("seed-a");
        let b = deterministic_uuid("seed-a");
        let c = deterministic_uuid("seed-b");
        assert_eq!(a, b, "той самий seed → той самий UUID");
        assert_ne!(a, c);
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "5", "version nibble");
        assert!(matches!(&a[19..20], "8" | "9" | "a" | "b"), "variant");
    }

    #[test]
    fn xml_escaping_and_attr() {
        assert_eq!(xml_escape("a<b>&c"), "a&lt;b&gt;&amp;c");
        assert_eq!(xml_attr("x\"y&z"), "x&quot;y&amp;z");
    }

    #[test]
    fn zip_stored_layout() {
        let zip = zip_stored(&[("a.txt", b"hello".as_slice()), ("b/c.txt", b"x")]);
        assert_eq!(&zip[0..4], &[0x50, 0x4b, 0x03, 0x04], "local header sig");
        assert_eq!(&zip[8..10], &[0, 0], "compression method = STORED");
        // EOCD наприкінці
        let n = zip.len();
        assert_eq!(&zip[n - 22..n - 18], &[0x50, 0x4b, 0x05, 0x06]);
        assert_eq!(u16::from_le_bytes([zip[n - 14], zip[n - 13]]), 2, "entries");
    }

    #[test]
    fn markdown_blocks() {
        let html = md_to_xhtml_body("# Титул\n\nАбзац з **жирним** і *курсивом*.\n\n---\n\n## Підзаголовок\n\n& <>");
        assert!(html.contains("<h1>Титул</h1>"));
        assert!(html.contains("<h2>Підзаголовок</h2>"));
        assert!(html.contains("<strong>жирним</strong>"));
        assert!(html.contains("<em>курсивом</em>"));
        assert!(html.contains("<hr/>"));
        assert!(html.contains("<p>&amp; &lt;&gt;</p>"));
        assert!(!html.contains("<p><h1>"), "заголовок не всередині параграфа");
    }
}
