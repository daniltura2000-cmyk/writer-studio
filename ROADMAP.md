# Roadmap — Writer Studio (Скрипторій)

Джерело вимог: `SKRIPTORIY-IMPLEMENTATION-PROMPT.md` (специфікація, зібрана з дослідження
~75 невирішених потреб письменників). Пріоритезація — сила болю × частота × реалізовність.

## M0 — Скелет ✅
- [x] Rust workspace: `skri-core` (lib) + `skri-cli` (bin)
- [x] Project Store: markdown-файли + SQLite (WAL) — файлова структура як єдине джерело правди
- [x] Frontmatter-парсер (YAML-lite), скан папки проєкту
- [x] SQLite FTS5-індекс тексту (швидкий пошук, tantivy — пізніше)
- [x] Preset Engine: JSON-маніфести, валідація, 2 вбудовані пресети (`novelist-uk`, `note-taker-uk`)
- [x] CLI: `init` / `index` / `bible` / `drift` / `preset list`

## M1 — Index Engine + Story Bible ✅
- [x] Токенізація з offsets (Unicode, кирилиця)
- [x] Кандидати у власні назви: правила (капіталізація, частотність, стоп-словники UA/EN)
- [x] Mentions з char-offsets → SQLite
- [x] Name-drift: нормалізований Levenshtein, групування варіантів → алерти
- [x] Story Bible: картки сутностей = агрегат mentions + assertions
- [x] Assertions з `bible/*.md` + конфлікт-детектор атрибутів
- [x] Acceptance-тести (fixtures укр.)

## M1.5 — Model Router ✅ (запит замовника в процесі збірки)
- [x] Каталог задач «по суті» (6 кнопок): генерація укр / переклад / транскрипція / граматика / рекапи / семантичний пошук
- [x] Автоматичний вибір: перша (рекомендована ЛОКАЛЬНА) модель задачі; хмара ніколи не авто
- [x] Кнопкове перевизначення: CLI-меню + `set_model`/`model_for` у settings.json
- [x] `catalog_json()` — те саме джерело для кнопок Tauri UI (M2); мокап `docs/ui/model-picker.html`

## M2 — Пресети повні + мультимовність (ядро ✅, UI ⬜)
- [ ] LanguageTool server локально (grammar UA+multi)
- [x] Серійний глосарій (series_wide) + narrative memory
- [x] Side-by-side переклад: NLLB-200 / Lapa LLM, стани draft→mt→human→approved
- [x] T-V реєстр (ти/ви) checker
- [x] Локалізаційні ліміти per-language (de ×1.30, uk ×1.15, CJK ×0.60) — алерт ДО публікації
- [x] PO-експорт із msgctxt (контекст подорожує з рядком)
- [x] preset compose: власний пресет з ассетів кількох джерел (kind:preset)

## M3 — Гейм-наратив Studio (ядро ✅, UI ⬜)
- [x] Text-first діалог (Ink/Yarn-дружний синтаксис), граф як похідна візуалізація
- [x] Barks як першокласний об'єкт (тригер/умови/пріоритет/cooldown/варіанти) + CSV експорт, дубль-детекція
- [x] Auditor змінних: матриця set/check/unset, мертві прапорці, сироти
- [x] Досяжність нод: недосяжні, dangling goto
- [ ] Round-trip імпорт/експорт .ink/.yarn
- [x] Локалізаційні ліміти per-language (дані в loc.rs, M2)

## M3.5 — Animation Writer (ядро ✅, UI ⬜)
- [x] Валідатор 20-хв епізоду: cold open + ≥2 narrative turns + hook, діалогові сцени підряд (red flag)
- [x] Мют-тест: visual_ratio по сценах («сцена гасне без звуку»)
- [x] Show Bible правила: `bible/rules.md` (правило :: ключові слова) → «retcon чи помилка?»
- [ ] Side-by-side сторіборд-лист (Flashboards-стиль)

## M2.5/M3.5 — Tauri UI (кнопкові пресети-напрямки)
- [ ] Онбордінг-вибір напрямку з описом «якою буде кінцева програма»
- [ ] Кнопковий каталог моделей (catalog_json), bark-таблиця, vars-матриця, episode-check дашборд
- [ ] Side-by-side переклад UI

## M4 — Серійний командний центр
- [ ] Kanban глав + прогноз вичерпання буфера (слів/день)
- [ ] Crosspost Orchestrator (RoyalRoad, ScribbleHub, Wattpad, AO3, Patreon API v2)
- [ ] Recap Generator з Index Engine
- [ ] Роялті-дашборд (локальний імпорт CSV KDP/D2D/Patreon)

## M5 — Решта модулів
- [ ] LitRPG event-sourcing stats engine (ретроактивні правки + діф-звіт)
- [ ] Retcon Manager (draft/published гілки, черга ревізій)
- [ ] Timeline Engine (часові маркери, календар світу, локаційний календар персонажів)
- [ ] Docs Studio (регресія тексту, docs↔код links, freshness)
- [ ] Notes→Manuscript Composer (двонаправлений зв'язок нотатка↔абзац)
- [ ] Журналістика: Source CRM, Quote Verifier (whisper.cpp), Pitch Lifecycle, Fact Ledger
- [ ] Поезія (скансія, рими), Comics Scripter (PANEL), IF Playtest Runner
- [ ] Pen-name Manager + Metadata Scrubber (EPUB/PDF/DOCX)

## M6 — Синк і шліфування
- [ ] CRDT (Automerge) поверх markdown/SQLite, офлайн-first
- [ ] Comments/Suggestions колаб
- [ ] Прогін acceptance-тестів §9 специфікації

## M2 — Пресети повні + мультимовність (ядро ✅, UI ⬜)
- [x] Серійний глосарій (series_wide, rule keep/translate/transliterate, міжтомний експорт)
- [x] Глосарій-валідація перекладу („Alice→Elise" ловиться; unresolved-терміни)
- [x] Промпт-блок глосарію для AI-перекладу (авто замість ручної вставки)
- [x] Перекладний пайплайн: сегменти, статуси draft→mt→human→approved, side-by-side дані
- [x] Локальна Ollama-інтеграція (raw TCP, std-only) + dry-run без моделі
- [x] Локалізаційні ліміти per-language (de ×1.30, uk ×1.15, CJK ×0.60) — алерт ДО публікації
- [x] T-V реєстр (ти/ви) — mixed-детекція з прикладами
- [x] PO-експорт із msgctxt (контекст подорожує з рядком)
- [x] preset compose: власний пресет з ассетів кількох джерел (kind:preset)
- [ ] LanguageTool server клієнт (grammar) — M2.5
- [ ] Tauri UI для всього вище — M2.5
