//! Перекладний пайплайн прози (M2): «локальна чернетка + людина фінально».
//!
//! Пайплайн зі специфікації §7.2:
//! 1) серійний глосарій → 2) промпт з глосарієм-примусом → 3) локальна модель
//! (Ollama на localhost:11434) → 4) пост-валідація термінів → 5) side-by-side
//! proofread зі статусами draft→mt→human→approved.
//!
//! Модель обирається Model Router'ом (`models::recommend("translate")`).
//! Все локально; без Ollama доступний dry-run (промпт без виклику).

use crate::glossary;
use crate::models;
use crate::store::ProjectStore;
use crate::{Error, Result};
use rusqlite::{params, OptionalExtension};

/// Статус сегмента перекладу.
pub const STATUS_DRAFT: &str = "draft";
pub const STATUS_MT: &str = "mt";
pub const STATUS_HUMAN: &str = "human";
pub const STATUS_APPROVED: &str = "approved";

/// Сегмент side-by-side.
#[derive(Debug, Clone)]
pub struct Segment {
    pub id: i64,
    pub doc_path: String,
    pub para_idx: i64,
    pub source_text: String,
    pub translated_text: String,
    pub status: String,
    /// порушення глосарію (заповнюється валідатором)
    pub violations: Vec<String>,
}

fn ensure_tables(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS trans_segments (
           id INTEGER PRIMARY KEY,
           doc_path TEXT NOT NULL,
           para_idx INTEGER NOT NULL,
           source_text TEXT NOT NULL,
           translated_text TEXT NOT NULL DEFAULT '',
           status TEXT NOT NULL DEFAULT 'draft',
           target_lang TEXT NOT NULL,
           UNIQUE(doc_path, para_idx, target_lang)
         );",
    )?;
    Ok(())
}

/// Розбити тіло документа на абзаци (порожні рядки = розділювачі).
pub fn paragraphs(body: &str) -> Vec<String> {
    body.split("\n\n")
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Підготувати сегменти перекладу для документа (ідемпотентно).
pub fn prepare(store: &ProjectStore, doc_path: &str, target_lang: &str) -> Result<Vec<Segment>> {
    ensure_tables(store.conn())?;
    let body = crate::indexer::read_body(store, doc_path)?;
    let paras = paragraphs(&body);
    let conn = store.conn();
    for (idx, text) in paras.iter().enumerate() {
        conn.execute(
            "INSERT INTO trans_segments(doc_path, para_idx, source_text, target_lang)
             VALUES (?1,?2,?3,?4)
             ON CONFLICT(doc_path, para_idx, target_lang)
             DO UPDATE SET source_text=excluded.source_text",
            params![doc_path, idx as i64, text, target_lang],
        )?;
    }
    segments(store, doc_path, target_lang)
}

/// Сегменти документа.
pub fn segments(store: &ProjectStore, doc_path: &str, target_lang: &str) -> Result<Vec<Segment>> {
    ensure_tables(store.conn())?;
    let conn = store.conn();
    let mut stmt = conn.prepare(
        "SELECT id, doc_path, para_idx, source_text, translated_text, status
         FROM trans_segments WHERE doc_path=?1 AND target_lang=?2 ORDER BY para_idx",
    )?;
    let rows = stmt
        .query_map(params![doc_path, target_lang], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .map(|(id, doc_path, para_idx, source_text, translated_text, status)| Segment {
            id,
            doc_path,
            para_idx,
            source_text,
            translated_text,
            status,
            violations: Vec::new(),
        })
        .collect();
    Ok(rows)
}

/// Зібрати промпт для сегмента: глосарій + narrative memory (коротко) + текст.
pub fn build_prompt(
    store: &ProjectStore,
    source_lang: &str,
    target_lang: &str,
    source_text: &str,
) -> Result<String> {
    let gloss = glossary::prompt_block(store, source_lang, target_lang)?;
    let mut out = String::new();
    if !gloss.is_empty() {
        out.push_str(&gloss);
        out.push_str("\n\n");
    }
    out.push_str(&format!(
        "Переклади текст з {source_lang} на {target_lang}. \
         Збережи стиль, ритм і реєстр мовлення; звертайся до персонажів СТРОГО за глосарієм. \
         Поверни ТІЛЬКИ переклад без коментарів.\n\n---\n{source_text}\n---"
    ));
    Ok(out)
}

///Dry-run: промпти всіх сегментів без виклику моделі (перевірка підготовки).
pub fn dry_run(
    store: &ProjectStore,
    doc_path: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<Vec<String>> {
    let segs = prepare(store, doc_path, target_lang)?;
    segs.iter()
        .map(|s| build_prompt(store, source_lang, target_lang, &s.source_text))
        .collect()
}

/// Запустити машинний переклад через ЛОКАЛЬНУ Ollama (localhost:11434).
/// Модель — з Model Router (перевизначена або рекомендована для "translate").
/// Після кожного сегмента — глосарій-валідація; порушення зберігаються в сегменті.
pub fn run_mt(
    store: &ProjectStore,
    doc_path: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<MtReport> {
    ensure_tables(store.conn())?;
    let segs = prepare(store, doc_path, target_lang)?;
    let (model_id, _auto) = match store.model_for("translate")? {
        Some(pair) => pair,
        None => {
            // ще не обрано → автоматичний вибір рекомендованої
            let rec = models::recommend("translate")
                .ok_or_else(|| Error::Invalid("каталог перекладу порожній".into()))?;
            (rec.id.clone(), true)
        }
    };
    let ollama_model = match model_id.as_str() {
        "nllb-200" => "nllb-200", // див. примітку: справжній MT через окремий сервіс
        other => other,
    };

    let mut report = MtReport::default();
    for s in &segs {
        let prompt = build_prompt(store, source_lang, target_lang, &s.source_text)?;
        let translated = match ollama_generate(ollama_model, &prompt) {
            Ok(t) => t,
            Err(e) => {
                report.errors.push(format!("сегмент {}: {e}", s.para_idx));
                continue;
            }
        };
        // пост-валідація глосарію
        let violations = glossary::validate_translation(
            store, &s.source_text, &translated, source_lang, target_lang,
        )?;
        let viols: Vec<String> = violations.iter().map(|v| v.problem.clone()).collect();
        let status = if viols.is_empty() { STATUS_MT } else { STATUS_DRAFT };
        store.conn().execute(
            "UPDATE trans_segments
             SET translated_text=?2, status=?3 WHERE id=?1",
            params![s.id, translated, status],
        )?;
        report.translated += 1;
        report.violations += viols.len();
    }
    Ok(report)
}

/// Мінімальний HTTP-клієнт Ollama (localhost:11434 /api/generate), std-only.
/// pub(crate): використовується також Recap Generator'ом (recap.rs).
pub(crate) fn ollama_generate(model: &str, prompt: &str) -> Result<String> {
    let payload = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "options": { "temperature": 0.4 }
    })
    .to_string()
    .into_bytes();

    use std::io::{Read, Write};
    let addr: std::net::SocketAddr = "127.0.0.1:11434"
        .parse()
        .map_err(|e| Error::Invalid(format!("адреса: {e}")))?;
    let mut stream = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5))
        .map_err(|_| {
            Error::Invalid(
                "Ollama недоступна на 127.0.0.1:11434 — встановіть Ollama і оберіть модель (`skri model auto <папка> translate`)".into(),
            )
        })?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(300)))?;
    let req_text = format!(
        "POST /api/generate HTTP/1.1\r\nHost: 127.0.0.1:11434\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    stream.write_all(req_text.as_bytes())?;
    stream.write_all(&payload)?;

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf)?;
    let raw = String::from_utf8_lossy(&buf);
    // тіло після порожнього рядка; chunked не чіпаємо (Connection: close → Ollama шле довжину)
    let body_start = raw.find("\r\n\r\n").map(|i| i + 4).ok_or_else(|| {
        Error::Invalid("Ollama: некоректна відповідь".into())
    })?;
    let resp_body = &raw[body_start..];
    // у разі chunked — склеюємо чанки
    let body_json = if raw.to_lowercase().contains("transfer-encoding: chunked") {
        dechunk(resp_body)
    } else {
        resp_body.to_string()
    };
    let v: serde_json::Value = serde_json::from_str(body_json.trim())
        .map_err(|e| Error::Invalid(format!("Ollama: не JSON ({e})")))?;
    v.get("response")
        .and_then(|r| r.as_str())
        .map(|s| s.trim().to_string())
        .ok_or_else(|| Error::Invalid("Ollama: немає поля response".into()))
}

fn dechunk(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    loop {
        let Some(line_end) = rest.find("\r\n") else { break };
        let size = usize::from_str_radix(rest[..line_end].trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        let start = line_end + 2;
        let end = (start + size).min(rest.len());
        out.push_str(&rest[start..end]);
        rest = &rest[(end + 2).min(rest.len())..];
    }
    out
}

/// Застосувати ручний переклад сегмента і позначити human/approved.
pub fn set_translation(store: &ProjectStore, segment_id: i64, text: &str, status: &str) -> Result<()> {
    if !matches!(status, STATUS_HUMAN | STATUS_APPROVED | STATUS_MT | STATUS_DRAFT) {
        return Err(Error::Invalid(format!("невідомий статус «{status}»")));
    }
    store.conn().execute(
        "UPDATE trans_segments SET translated_text=?2, status=?3 WHERE id=?1",
        params![segment_id, text, status],
    )?;
    Ok(())
}

/// Звіт машинного перекладу.
#[derive(Debug, Default)]
pub struct MtReport {
    pub translated: usize,
    pub violations: usize,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmpstore(name: &str) -> (PathBuf, ProjectStore) {
        let root = std::env::temp_dir().join(format!("skri-tr-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let store = ProjectStore::init(&root, "novelist-uk", "uk").unwrap();
        (root, store)
    }

    const DOC: &str = "---\ntitle: Гл 1\nlang: uk\n---\n\nКальдрін вийшов на стіни.\n\nМеч «Тиша» висів на спині.\n";

    #[test]
    fn prepare_segments_idempotent_side_by_side() {
        let (root, store) = tmpstore("prep");
        std::fs::write(root.join("manuscript").join("01.md"), DOC).unwrap();
        store.reindex().unwrap();

        let s1 = prepare(&store, "manuscript/01.md", "en").unwrap();
        assert_eq!(s1.len(), 2, "2 абзаци → 2 сегменти");
        assert_eq!(s1[0].status, STATUS_DRAFT);
        // повторно — ідемпотентно, ті самі рядки
        let s2 = prepare(&store, "manuscript/01.md", "en").unwrap();
        assert_eq!(s1.len(), s2.len());

        // human-статус зберігається
        set_translation(&store, s1[0].id, "Kaldrin walked the walls.", STATUS_HUMAN).unwrap();
        let s3 = segments(&store, "manuscript/01.md", "en").unwrap();
        assert_eq!(s3[0].status, STATUS_HUMAN);
        assert_eq!(s3[0].translated_text, "Kaldrin walked the walls.");
    }

    #[test]
    fn prompt_includes_glossary_block() {
        let (root, store) = tmpstore("prompt");
        glossary::upsert(&store, &glossary::GlossaryEntry {
            id: 0, term: "Тиша".into(), source_lang: "uk".into(), target_lang: "en".into(),
            canonical_translation: "the Hush".into(), rule: "translate".into(),
            notes: "меч".into(), series_wide: true,
        }).unwrap();
        let p = build_prompt(&store, "uk", "en", "Меч «Тиша» висів.").unwrap();
        assert!(p.contains("ГЛОСАРІЙ СЕРІЇ"));
        assert!(p.contains("Тиша → the Hush"));
        assert!(p.contains("[меч]"));
    }

    #[test]
    fn dry_run_works_without_ollama() {
        let (root, store) = tmpstore("dry");
        std::fs::write(root.join("manuscript").join("01.md"), DOC).unwrap();
        store.reindex().unwrap();
        let prompts = dry_run(&store, "manuscript/01.md", "uk", "en").unwrap();
        assert_eq!(prompts.len(), 2);
        assert!(prompts[0].contains("Переклади текст з uk на en"));
    }

    #[test]
    fn glossary_violation_blocks_approval_status() {
        let (root, store) = tmpstore("viol");
        glossary::upsert(&store, &glossary::GlossaryEntry {
            id: 0, term: "Кальдрін".into(), source_lang: "uk".into(), target_lang: "en".into(),
            canonical_translation: "Kaldrin".into(), rule: "translate".into(),
            notes: String::new(), series_wide: true,
        }).unwrap();
        // переклад без канонічної форми імені → порушення
        let v = glossary::validate_translation(&store, "Кальдрін пішов.", "Someone walked.", "uk", "en").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].expected, "Kaldrin");
        // з канонічною формою — чисто
        let ok = glossary::validate_translation(&store, "Кальдрін пішов.", "Kaldrin walked.", "uk", "en").unwrap();
        assert!(ok.is_empty());
        // rule=keep: термін має залишитися як є
        glossary::upsert(&store, &glossary::GlossaryEntry {
            id: 0, term: "Тиша".into(), source_lang: "uk".into(), target_lang: "en".into(),
            canonical_translation: String::new(), rule: "keep".into(),
            notes: "назва меча".into(), series_wide: true,
        }).unwrap();
        let kept = glossary::validate_translation(&store, "Меч «Тиша».", "The sword «Тиша» shone.", "uk", "en").unwrap();
        assert!(kept.is_empty());
        let lost = glossary::validate_translation(&store, "Меч «Тиша».", "The Hush shone.", "uk", "en").unwrap();
        assert_eq!(lost.len(), 1, "keep-термін перекладено — порушення");
    }
}
