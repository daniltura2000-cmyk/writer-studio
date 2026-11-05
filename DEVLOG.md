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

## 2026-11-05 (продовження) — M3: round-trip Ink/Yarn (inkbridge)

### Що зроблено
- **inkbridge.rs** — round-trip міст .ink/.yarn ↔ text-first markdown:
  - імпорт: Ink-ноди (`== knot`), `VAR`, `~ set` (з операторами =/+=/-=), standalone-диверти
    `-> X`, диверти в кінці рядка, вибори `* [текст] -> X`, whole-line умови `{ x }`;
    Yarn: `title:`/`---`/`===`, `[[Target]]`/`[[Текст|Target]]`, `<<set $x = v>>` (і форма `to`),
    `<<if $x>>`. Все → markdown з `-> нода` / `[goto нода]` / `%SET/%CHECK` маркерами
    (сумісно з narrative::scan_dialogue/graph_audit — `skri vars` і `skri graph` бачать імпортовані документи).
  - експорт назад у .ink/.yarn; round-trip імпорт→експорт→імпорт дає семантично
    еквівалентний документ (тест на фікстурах укр./англ.).
  - VAR-декларації Ink → frontmatter `ink-vars: x=5; y=...` → назад у `VAR`.
- **Детект зовнішніх змін**: SHA-256 (std-only, без нових залежностей) імпортованого
  файлу в таблиці `ink_imports`; якщо файл змінився з останнього імпорту (правки
  з боку рушія/git diff інших) — алерт kind='ink-external-change' з payload
  (src, old_hash, new_hash). Повторний імпорт без змін нових алертів не дає.
- **CLI**: `skri ink import <папка> <файл.ink|.yarn>`, `skri ink export <папка> <док.md> <файл.ink|.yarn>`.

### Знайдені тестами
1. Подвійний `=`: split_assign повертав "op value" одним рядком, а set_line додавав
   `= ` повторно → `%SET x = = 5`. Виправлено: (ім'я, оператор, значення) окремо.
2. `VAR` до першої ноди з'їдався guard'ом «проза до першої ноди» → VAR-парсинг
   перенесено до guard'а.
3. Порядок кортежу parse_var_ops — (op, name, pos), а не (name, op, ...) — тест спочатку
   читав його навпаки.

### Свідомі обмеження підмножини (документовані в док-коментарі)
- Інлайн-умови Ink `{...}` всередині речення лишаються прозою як є (round-trip їх зберігає,
  але аудитор змінних їх не бачить); складні yarn-умови `<<if $a and $b>>` — так само.
- Ігноруються: position/color/tags-заголовки Yarn, glue, stitches Inka.

### Прогін
```
cargo test → 54 passed (у т.ч. 4 нових m3_inkbridge), 0 failed
CLI вручну: ink import (укр. .ink) → dialogue/quest.md → ink export → back.ink байт-в-байт
```

## 2026-11-05 (продовження) — M2.5/M3.5: каркас Tauri UI (app/tauri)

### Що зроблено
- **`app/tauri`** — третій член workspace: Tauri 2.12.5 + tauri-build 2.7.4
  (останні стабільні 2.x; залежності лише tauri + tauri-build + serde/serde_json + skri-core).
  Фронт — статичні файли `app/tauri/ui/` (vanilla JS, БЕЗ build-степу і npm-залежностей):
  `frontendDist: "ui"` у tauri.conf.json, `withGlobalTauri: true` → `window.__TAURI__.core.invoke`.
- **Invoke-команди** (`app/tauri/src/commands.rs`, усі викликають існуючі публічні API
  skri-core — сам core не змінено):
  `list_directions` (4 напрямки, дзеркало `print_directions` з main.rs, збагачене
  `presets::builtin_list()`), `model_catalog` (= `models::catalog_json()`),
  `project_info`, `run_index` (reindex+scan+sync_bible+merge+scan_dialogue — як `skri index`),
  `run_bible` (`bible::cards`), `run_drift` (`drift::detect` + усі невирішені алерти),
  `var_audit`, `barks`, `episode_validate` (validate_episode + mute_test + check_bible_rules),
  `set_model` (кнопка каталогу → `.skri/settings.json`).
- **Екрани** (style-референс `docs/ui/model-picker.html`): Напрямки (4 кнопки-напрямки
  з описом кінцевої програми), Моделі (кнопковий каталог 1:1 з мокапом, вибір зберігається
  через set_model), Біблія (картки сутностей + зведення реіндексації), Алерти
  (drift/конфлікти, аудит змінних, bark-таблиця, валідатор епізоду + мют-тест + правила Show Bible).
- Структура: `src/main.rs` (Builder + generate_handler), `src/commands.rs` (команди + DTO,
  бо EntityCard/Alert у core без Serialize), `ui/index.html|app.js|style.css`,
  `icons/icon.ico` (згенерована заглушка 16×16), `bundle.active=false` до завершення M2.5.
  Докладніше — `app/tauri/README.md`.

### Прогін
```
cargo check --workspace → Finished dev profile in 3m02s (skri-tauri + tauri 2.12.5 скомпільовано)
```
Warnings лише наявні в skri-core (translate.rs unused import, stats.rs unused mut) — не з UI.

### Як добудувати далі
- Повний запуск: `cd app/tauri && cargo tauri dev` (tauri-cli: `cargo install tauri-cli --version "^2"`),
  або просто `cargo run` — фронт статичний, generate_context! вбудовує `ui/`.
- Діалог вибору папки проєкту — додати `tauri-plugin-dialog` (зараз шлях вводиться вручну,
  запам'ятовується в localStorage).
- Редагування (маскир alias-ів, resolve алертів, upsert bark) — нові команди поверх
  існуючих мутаторів core; поки каркас read-only + set_model.

## 2026-11-05 (продовження) — M5: LitRPG Progression Engine (event-sourcing статів)

### Що зроблено
- **stats.rs** — дельти статів парсяться прямо з рукопису: `[stat: Кальдрін рівень +2]`,
  `[stat: Кальдрін hp -30]`, `[stat: Кальдрін xp += 150]` (голе число = приріст;
  оператор працює і впритул `+=150`, і окремим токеном `+= 150`; сутність може
  містити пробіл — атрибут і значення завжди два останні токени). Теги шукаються
  ASCII-незалежно від регістру по байтах (шаблон чистий ASCII → кириличні
  offset-и не ламаються), кілька тегів на рядок підтримано.
- **Snapshot-модель**: `stat_snapshots(entity_name, attr, chapter, value)` —
  кумулятивний стан ПІСЛЯ глави; перебудова з manuscript/ ідемпотентна
  (номер глави = ведучі цифри імені файла, інакше порядок у docs). Стан на
  гл. N = рядок з MAX(chapter) ≤ N.
- **Ретроактивна правка**: rebuild порівнює старі vs нові снапшоти → алерт
  `kind='stat-change'` з payload (глава, старе → нове, з каскадом по пізніх
  главах); невирішені stat-change перезаписуються щоразу — повторний прогін
  без правок алертів не плодить.
- **Stat sheet preview**: `skri stats sheet <папка> <глава>` — стан усіх
  персонажів, як читач бачить у гл. N. **Stat-error radar**: від'ємні hp/mp →
  алерт `kind='stat-error'` (один на атрибут, із переліком глав-порушень).

### Знайдені тестами (і виправлені)
1. `-= 30` з пробілом (оператор окремим токеном) парсився як attr="-=", value=+30
   → парсер розпізнає обидві форми оператора.
2. Радар рахував алерти по рядках снапшотів (по одному на главу) → згруповано
   по (сутність, атрибут).

### Прогін
```
cargo test → 54 passed (21 unit + 11 M0/M1 + 8 M2 + 6 M3 + 4 inkbridge + 4 m5_stats), 0 failed
CLI вручну: stats sheet (кумулятив по главах), stats diff (ретроактивна правка
hp 100→200: «глава 1: 100 → 200; глава 2: -30 → 70», повторний прогін чистий), stats radar — OK
```
