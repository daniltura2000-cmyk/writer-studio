# DEVLOG — журнал процесу розробки

## 2026-11-05 — вертикальний слайс M0+M1 + Model Router

### Що зроблено

**Дослідження (до коду).** 4+3 дослідницькі агенти прошерстили Reddit (r/writing,
r/litrpg, r/royalroad, r/Journalism, r/AO3, r/Screenwriting, r/TranslationStudies,
r/selfpublish), форуми RoyalRoad/Zotero/Stage 32, GitHub issues, Trustpilot —
~75 невирішених потреб письменників із цитатами та URL. З них народилася
специфікація (`SKRIPTORIY-IMPLEMENTATION-PROMPT.md`) і цей репозиторій.

**M0 — скелет.**
- Rust workspace: `skri-core` (lib) + `skri-cli` (bin `skri`).
- Project Store: markdown + SQLite (WAL) + FTS5; `.skri/` в .gitignore —
  індекс перебудовується з файлів будь-коли (залізне правило №1).
- Frontmatter-парсер (YAML-lite) з байтовими offset тіла — критично для
  переходів «алерт → місце в тексті».
- Preset Engine: JSON-маніфести, валідація, 2 вбудовані пресети укр.

**M1 — Index Engine + Story Bible (серце продукту).**
- Токенізатор з Unicode-офсетами (кирилиця+латиниця, апострофи всередині слова).
- Кандидати у власні назви: пара «Ім'я Прізвище», non-initial capitalized,
  частотність ≥ 2. Слабкі кандидати (одинична згадка на початку речення)
  реєструються як `auto-weak` — у біблію не потрапляють, але беруть участь
  у drift-детекції: **опечатка при першій згадці імені теж ловиться**.
- Name-drift: Levenshtein ≥ 0.82, довжина ≥ 5; канон = форма з максимумом
  згадок; варіанти перешиваються в aliases, згадки перелінковуються.
- Story Bible: `bible/*.md` → manual-сутності + assertions;
  **крос-файлові конфлікти канону** («очі: зелені» в одному файлі vs
  «очі: карі» в іншому) → алерт з обома значеннями.
- Manual-картка біблії поглинає auto-сутність тієї ж назви (згадки перешиваються).

**Model Router (запит замовника в процесі).**
- Каталог задач «по суті»: генерація укр / переклад / транскрипція / граматика /
  рекапи / семантичний пошук. Перша опція кожної задачі = автоматичний вибір;
  «кнопки» = перевизначення. Хмара (BYOK) в каталозі є, але ніколи не авто.
- `skri model menu|auto|set|status|tasks`; збереження в `.skri/settings.json`.
- `models::catalog_json()` — те саме джерело для кнопок майбутнього Tauri UI;
  мокап екрану: `docs/ui/model-picker.html`.

### Помилки, знайдені тестами (і виправлені)
1. **Char-boundary panic у frontmatter**: подвійний відлік offset →
   `start byte index 39 is not a char boundary` на кирилиці. Виправлено:
   тіло починається одразу після закриваючого `---`.
2. **FTS5 contentless-таблиця не підтримує DELETE** → звичайна fts5-таблиця.
3. **walkdir::Error не конвертується в Error** → явний мапінг через into_io_error.
4. **Drift-фільтр з'їдав опечатку при першій згадці** (total=1, initial-only
   відфільтровувався до порівняння) → пере-дизайн: слабкі кандидати живуть
   у БД до drift-фази, потім шум прибирається.
5. **Конфлікт канону всередині одного файлу не ловив крос-файловий**:
   delete-then-insert стирав історію → per-doc delete + крос-файлова перевірка.
6. **Порядок синхронізації**: `merge_auto_into_manual` вимагає, щоб bible-синк
   відбувся ПЕРШИМ (manual-картка поглинає auto напряму) — виправлено і в CLI,
   і в тестах.

### Відомі обмеження M1 (свідомо, для M2)
- Відмінки українською («Велесом») — окрема сутність; потрібна лематизація (M2).
- Пара «Старий Велес» (прикметник+імена) хибно кандидат; потрібен словник
  прикметників (M2).
- «Ім'я» не автолінкується на «Ім'я Прізвище» без підтвердження автора —
  свідомо: правила без AI дають мало хибних злиттів, AI-підсилювач у M2.

### Прогін
```
cargo test → 14 passed (4 unit + 3 models + 7... зведення: 0 failed)
skri init/index/drift/bible/model/search — вручну на демо-проєкті: OK
```

## 2026-11-05 (продовження) — M2: мультимовність і глосарій

### Що зроблено
- **glossary.rs** — серійний глосарій: upsert за (term, src, tgt), rule
  (keep/translate/transliterate), series_wide переноситься між томами
  (`glossary series` → .skri/glossary-series.json). Пост-валідатор ловить
  „Alice becoming Elise" (перекладач перейменував персонажа) і невизначені
  терміни (потрібне рішення автора). `prompt_block()` — авто-генерація
  глосарій-блоку для промпта (замість ручної вставки в кожен запит).
- **loc.rs** — ліміти символів per-language ЗАЗДАЛЕГІДЬ (фактори: de/fr/ru
  ×1.30, uk ×1.15, CJK ×0.60); `worst_limit` для UI-підсвітки. T-V реєстр
  (ти/ви): займенники + дієслівні закінчення, mixed-детекція з прикладами.
  PO-експорт з msgctxt: контекст подорожує з рядком (speaker/tone).
- **translate.rs** — пайплайн «локальна чернетка + людина»: paragraphs →
  prepare (ідемпотентні сегменти) → build_prompt (глосарій-примус) →
  run_mt через ЛОКАЛЬНУ Ollama (raw TCP HTTP/1.1 клієнт, std-only, chunked
  розбір) → глосарій-валідація (порушення тримають статус draft) →
  side-by-side зі статусами draft→mt→human→approved. Dry-run працює без
  Ollama.
- **presets::compose** — збирання ВЛАСНОГО пресету з ассетів різних джерел:
  `entities:novelist-uk checklists:note-taker-uk validators:...` →
  валідований маніфест → `.preset.json` (імпорт/експорт файлом).

### CLI (нові)
`glossary add|list|series`, `translate dry|segments|set`, `limits`,
`tv`, `preset compose`.

### Знайдені тестами/збіркою
1. `std::http` не існує в stable → raw TCP HTTP/1.1 клієнт (std-only, без нових залежностей).
2. `UNIQUE(lower(term),...)` в TABLE-констрейнті невалідний у SQLite →
   `CREATE UNIQUE INDEX ... ON glossary(lower(term), ...)`.
3. Run_mt: плейсхолдери `?2/?3` змістилися після правки → mismatched params.
4. `unwrap_or_else` з `?` всередині — не компілюється → match.

### Прогін
```
cargo test → 34 passed (15 unit + 11 acceptance M0/M1 + 8 acceptance M2), 0 failed
CLI вручну: glossary/limits/tv/translate dry/preset compose — OK
```
