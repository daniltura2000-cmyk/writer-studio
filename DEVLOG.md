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

## 2026-11-05 (продовження) — M4: Recap Generator (recap.rs)

### Що зроблено
- **recap.rs** — рекап по book-boundary з Index Engine (специфікація §5.6):
  - rules-first (детерміновано, без AI): топ-сутності по частотності згадок
    у діапазоні, «нові персонажі» = сутності, чия ПЕРША згадка в усьому
    рукописі потрапляє в діапазон, кількість глав/слів; вивід —
    markdown-список у файл `recap-<від>-<до>.md` у папці проєкту.
  - Глава = порядок `manuscript/*.md` (сорт за іменем файлу), номер 1-based.
  - AI-рекап через ЛОКАЛЬНУ Ollama: модель задачі `summarize` з Model Router
    (авто = lapa-v0.1.2); промпт = rules-дані + текст діапазону (ОСТАННІ
    3 глави, хвіст до 12000 символів на главу, зріз по char boundaries —
    кирилиця). `build_prompt` публічна і чиста — будується без виклику моделі.
  - Ollama недоступна/порожня відповідь → чесне повідомлення і fallback на
    rules-only; файл рекапу зберігається в будь-якому разі (не падає).
  - `translate::ollama_generate` тепер `pub(crate)` — публічний API не змінено.
- **CLI**: `skri recap <папка> <від-глави> <до-глави> [--ai]`
  (сам робить reindex + scan_project перед рекапом).

### Попутні виправлення (спільні файли)
- store.rs `reindex()`: рядки docs для ВИДАЛЕНИХ файлів тепер зникають
  (разом з FTS і каскадом згадок) — індекс = відображення файлів (залізне
  правило №1); ловилося тестом m4_release.
- release.rs / m4_release.rs: дрібні синтаксичні/типові фікси (E0597 у
  query_map-блоках, зайва дужка, Option/i64 mismatch) — код того ж дня.

### Прогін
```
cargo test → 78 passed, 0 failed (28 unit + acceptance M0–M5, вкл. 6 нових m4_recap)
CLI вручну: skri recap demo 1 2 → recap-1-2.md (rules-only, числа/сутності коректні);
skri recap demo 1 2 --ai без Ollama → «⚠ Ollama недоступна…» + rules-only файл, exit 0
```

## 2026-11-05 (продовження) — M4: Release Planner (Kanban глав + прогноз буфера)

### Що зроблено
- **release.rs** — серійний командний центр (§5.6, частина 1):
  - реєстр глав: auto-scan manuscript/*.md → `chapters_registry(doc_path, order,
    pub_status, pub_date, platforms_json)`. **Статус живе у frontmatter глави
    (`pub_status:`)** — файли лишаються єдиним джерелом правди (залізне правило №1),
    таблиця — похідний індекс; порядок = ведучі цифри імені файла, інакше позиція
    серед docs; `pub_date: YYYY-MM-DD` → epoch-секунди UTC, `pub_platforms:` → JSON.
  - Kanban-звіт: колонки draft → edit → buffer → scheduled → published (+ будь-які
    нестандартні статуси окремими колонками) + журнал переходів `chapters_log`
    (зміна статусу між прогонами пишеться автоматично, повторний прогін без правок
    алертів/записів не плодить — ідемпотентність).
  - `release::set_status` — статус/дата/платформи пишуться ПРЯМО у frontmatter
    файлу (хірургічно: решта header-блоку і тіло не чіпаються; файл без
    frontmatter отримує новий блок).
  - Прогноз буфера: фактична швидкість = слів/день з mtime-дельт (rate кожної
    глави = слів/днів від попередньої за mtime; ковзне середнє по останніх N
    главах, `--window`); буфер = глави buffer/scheduled; «запас вичерпається
    через X днів» = слів буфера / слів-на-день; «закінчиться Y-го числа» — дата
    YYYY-MM-DD (стеля, консервативно). Чиста функція `forecast_with(history,
    buffer, now, window)` — тестована без файлів.
- **Std-only дати**: epoch-арифметика днів від епохи (civil_from_days /
  days_from_civil, Howard Hinnant) — chrono НЕ додавався; `fmt_date`/`parse_ymd`
  публічні (стануть кнопками Tauri UI).
- **CLI**: `skri release kanban <папка>`, `skri release forecast <папка>
  [--window 7]`, `skri release set <папка> <док.md> <статус> [YYYY-MM-DD] [платформи]`.

### Знайдені тестами (і виправлені)
1. reindex не чистить docs від зниклих файлів → rebuild впав на read_to_string
   зниклої глави → rebuild пропускає неіснуючі шляхи, рядок зникає зі реєстру
   через stale-чистку (правило «індекс похідний» дотримане і тут).

### Прогін
```
cargo test → усі зелені (28 unit + 11 acceptance + 8 M2 + 6 M3 + 4 inkbridge +
6 m5_stats + 5 m4_release + 4 m3_inkbridge + інші модулі паралельних агентів), 0 failed
CLI вручну: release kanban (3 глави у 3 колонках, дати/платформи), release
forecast --window 5 (5 слів/день, буфер 2 глави, «через ~3.3 дні — 2026-11-09»),
release set (pub_status записано у frontmatter, file = джерело правди) — OK
```

## 2026-11-05 (продовження) — M4: Crosspost Orchestrator + роялті-дашборд

### Що зроблено
- **crosspost.rs** — пакувальник + журнал публікацій (§5.6):
  - канонічний markdown глави → пакунки для 5 платформ (royalroad/
    scribblehub/wattpad/ao3/patreon): `.skri/outbound/<глава>/<платформа>/`
    з `package.json` {platform, title, html_body, notes, tags_json} і
    готовим до копіювання `chapter.html`; tags/notes — з frontmatter;
  - простий markdown→html (std-only): параграфи, `---`→`<hr>`,
    `**bold**`→`<strong>`, `*italic*`→`<em>`, escape HTML;
  - журнал `crosspost_log(doc_path, platform, version_hash, status
    pending|posted|failed, packed_at, published_at)`; статус оновлюється
    ВРУЧНУ `skri crosspost mark <папка> <док> <платформа> posted`;
  - **чесно задокументовано**: у платформ немає відкритих API для постингу
    вебновел (AO3 закритий, RR/SH/Wattpad — лише скрейпери з ризиком бану,
    Patreon — OAuth) → реальна відправка ручна; майбутні адаптери з OAuth
    перевикористають той самий журнал і формат пакунків;
  - правка канону після posted: sha256 тіла ≠ version_hash → алерт
    `kind='crosspost-stale'` («главу змінено після публікації на X — потрібна
    перевідправка»), ідемпотентно (1 відкритий алерт на пару док×платформа);
    повторний `mark ... posted` фіксує хеш поточної версії і розв'язує алерт.
- **royalty.rs** — роялті-дашборд з локальних CSV:
  - гнучкий CSV-парсер (лапки, `""`, CRLF): колонки розпізнаються за
    заголовками-синонімами (title/book/product, date/order date/period,
    marketplace/platform/channel, royalty/earnings/revenue, units/quantity);
    невідомі колонки ігноруються, рядки без суми пропускаються;
  - суми: `$1,234.56`, `1.234,56`, валютні коди → центи; тисячні vs
    десяткові коми розрізняються за патерном груп по 3;
  - таблиця `royalty_rows(source_file, date, platform, book, units,
    revenue_cents)`; імпорт ідемпотентний (перезапис рядків свого файла);
  - агрегати по платформах / книгах / місяцях (нормалізація дат ISO,
    M/D/YYYY, D.M.Y, «January 2026»).
- **CLI**: `skri crosspost pack|status|mark`, `skri royalty import|summary
  [--by platform|book|month]`; status заодно виконує stale-перевірку.

### Знайдені тестами (і виправлені)
1. Повторний `mark posted` після правки перезаписував СТАРИЙ запакований
   хеш → stale-алерт не гаснув. Виправлено: posted фіксує хеш ПОТОЧНОГО
   канону (перевідправлена версія).
2. `By::label` був приватним → CLI не компілився (E0624).

### Прогін
```
cargo test → 78 passed, 0 failed (28 unit + acceptance, у т.ч. 6 нових m4_crosspost)
CLI вручну: crosspost pack (5 пакунків, версія d186cbdb), mark wattpad posted,
status (таблиця журналу), правка глави → stale «d186cbdb → c9eb5668»,
royalty import ($1,234.56 → 123456 центів) + summary --by month — OK
```

## 2026-11-05 (продовження) — M5: Retcon Manager (retcon.rs)

### Що зроблено
- **retcon.rs** — draft/published гілки (§5.4):
  - публікація глави = фіксація снапшоту тіла (без frontmatter) у
    `published_snapshots(doc_path, version, body_sha256, published_at, body)`;
    хеш — той самий публічний `inkbridge::sha256_hex` (std-only); версії
    ростуть (MAX+1), старі снапшоти лишаються в БД;
  - diff чернетки проти останнього снапшоту: по-рядковий LCS (std-only,
    без crates); «змінений» рядок = пара removed+added; захист від
    квадратичного вибуху: добуток рядків > 4 млн → ВСІ рядки вважаються
    зміненими (консервативно — більше зобов'язань, ніж менше);
  - retcon-зобов'язання: якщо дельта (обидва боки, substr без регістру)
    містить назву сутності з entities або її alias — усі згадки сутності
    в ПІЗНІШИХ главах (order > поточної, порядок із chapters_registry
    release-модуля) → черга `retcon_queue(entity_name, source_doc,
    affected_doc, affected_pos, sentence, status open|reviewed)`;
  - ідемпотентність: перебудова черги стирає open-рядки і генерує заново;
    позиція, вже позначена reviewed (той самий ключ), назад у open НЕ
    повертається — reviewed не плодяться і не зникають;
  - `retcon::rebuild` сам робить reindex + scan_project + release::rebuild
    (залізне правило №1: черга похідна від файлів, базу можна стерти).
- **CLI**: `skri retcon publish|diff|queue|review`; diff показує `-/+`
  рядки з номерами (published/draft), queue — таблицю із sentence і
  char-offset згадки.

### Прогін
```
cargo test → 85 passed, 0 failed (29 unit, вкл. 1 новий diff_lines +
6 нових m5_retcon: publish/diff/queue/ідемпотентність/review/без-сутностей)
CLI вручну: publish 01+02 → версії 1; правка гл.1 («заплющив» → «затулив»)
→ diff «-4/+4», queue: Кальдрін → manuscript/02.md @2 (речення збережене);
review #1 → reviewed; повторний queue — reviewed лишився, open не повернувся;
diff неіснуючого снапшоту → чесна помилка «спершу skri retcon publish»
```

## 2026-11-05 (продовження) — M5: Timeline Engine (timeline.rs)

### Що зроблено
- **timeline.rs** — часові маркери, календар світу, локаційний календар (§5.5):
  - календар світу: `.skri/calendar.json` (місяці об'єктами {name,days} або
    рядками «Тарвен=30») АБО `bible/calendar.md` з frontmatter
    `months:`/`days_of_week:`; немає обох — григоріанський за замовчуванням
    (укр. назви, родовий відмінок для показу). `format_day(day)` — людський
    вигляд абсолютної доби («12 жовтня», «5 Вален — День Попелу, рік 2»);
  - часові маркери: inline `[time: +3 днів]`, `[time: день 12]`,
    `[time: наступний ранок]`, `[time: flashback]` + природні фрази укр./англ.
    («наступного ранку», «через три дні», «ввечері того ж дня», «за тиждень»,
    «пізніше», «на світанку», «12 жовтня», "next morning", "three days later",
    "at dawn") — токен-матчинг на indexer::tokenize (байтові offsets, кирилиця);
    числівники словом і цифрою; явні дати парсяться у КАЛЕНДАРІ СВІТУ
    (кастомні назви + відмінкові суфікси «Тарвеня»);
  - накопичення по главах: frontmatter `day_start: N` (абсолютна доба початку
    глави), без нього — продовження з кінцевої доби попередньої; маркери між
    главами накопичуються послідовно; явна дата відображається в РІК поточної
    доби календаря;
  - таблиці `timeline_points(doc_path, ord, day, time_of_day, marker_text,
    char_start)` і `timeline_locs(doc_path, ord, day, entity, location,
    moved)` — перебудовуються ідемпотентно (DELETE + INSERT з manuscript/);
    `ord` замість `order` — зарезервоване слово SQL;
  - валідатори: стрибок назад у добах без активного флешбеку → алерт
    `kind='timeline-backwards'` (у т.ч. day_start нижче кінця попередньої
    глави); глава без жодного time-маркера → ПОПЕРЕДЖЕННЯ `timeline-missing`
    у звіті validate (НЕ алерт у БД); конфлікт локацій: сутність у двох
    місцях в ОДНУ добу без маркера переміщення `[loc: X = Y (шлях)]` → алерт
    `kind='location-conflict'`; дужка «(шлях ...)» = маркер переміщення;
  - свідоме обмеження: регресія часу доби (вечір→ранок в ОДНУ добу) алертом
    не рахується — надто багато хибних спрацювань на звичайній прозі;
  - loc-маркери інтерполюються з time-маркерами за offsetами — доба локації
    = стан годинника в місці маркера.
- **CLI**: `skri timeline show <папка>` (лінійка діб по главах з форматуванням
  календаря), `skri timeline validate <папка>`, `skri timeline locs <папка>`.

### Знайдені тестами (і виправлені)
1. Явна дата «12 жовтня» у світі з кастомним календарем (рік 70 днів)
   проєціювалась на день-у-році чужого календаря → дати тепер парсяться
   через `parse_time_markers_in(body, &cal)` у календарі світу.
2. «12 жовтень» (називний) у показі — додано родовий відмінок для
   григоріанських місяців (Month.genitive).
3. «three days later» не матчився — числівники розширені англійськими.

### Прогін
```
cargo test → 92 passed, 0 failed (29 unit + acceptance M0–M5, вкл. 7 нових
m5_timeline: парсинг форм, накопичення по главах, стрибок назад/флешбек,
глава-без-маркерів = попередження, локаційний зведення, конфлікт локації,
календар json/md/default)
CLI вручну: timeline show (3 глави, кастомний календар: «12 Тарвеня» →
доба 12), validate (чистий; конфлікт без (шлях) → алерт), locs (хто-де по
главах, [переміщення]) — OK
```

## 2026-11-05 (продовження) — Tauri UI: повноцінний редактор (M2.6)

### Що зроблено
- **Головний робочий екран «Студія»**: лівий сайдбар (340px) із чотирма
  табами + центральна панель редактора. Темна тема (палітра в дусі
  старого `.json`-блока: bg #0f1218, card #171b23, acc #818cf8).
- **Таб «Глави»**: список `manuscript/*.md` (нова команда `list_docs` —
  свіжий reindex + запит docs WHERE kind='manuscript', title з frontmatter
  або з імені файла). Клік по главі → `read_doc`.
- **Редактор**: textarea ТІЛА БЕЗ frontmatter; `write_doc` хірургічно
  перечитує файл на момент збереження і підставляє нове тіло після
  незміненого header-блока (frontmatter байт-в-байт, включно з
  pub_status/pub_date для release) + одразу store.reindex() (індекс =
  відображення файлів). Статус-бар: слів/символів лічильник on-input,
  «Зберегти» + Ctrl+S (window keydown, preventDefault), час збереження.
  Перехід на іншу главу з незбереженими змінами — confirm.
- **Таб «Біблія»**: картки EntityCard (канон, тип, варіанти, згадки,
  assertions) + кнопка «Реіндексувати»; ліниве завантаження при першому
  відкритті таба, оновлення після кожного «Перевірити».
- **Таб «Алерти»**: УСІ невирішені алери з БД (drift, assertion-conflict,
  stat-change/stat-error, timeline-backwards/location-conflict,
  crosspost-stale, episode-check, ink-external-change) з фільтром по kind
  (укр. підписи KIND_LABELS), лічильник-бейдж на табі; collapsible
  «Інструменти діалогів» (аудит змінних, barks, валідація епізоду) —
  перенесені зі старого екрана.
- **Таб «Реліз»**: kanban-колонки draft→edit→buffer→scheduled→published
  (кожна глава має кнопку «→ наступний статус» через release::set_status;
  published — без кнопки) + прогноз буфера (швидкість слів/день, розмір
  буфера, днів до вичерпання, дата) через release::kanban + forecast(7).
- **Кнопка «Перевірити»** (шапка): нова команда `run_checks` — повний
  конвеєр reindex → scan → sync_bible → merge → scan_dialogue →
  drift::detect → stats::rebuild → timeline::validate →
  crosspost::check_stale; повертає зведення + усі алерти; UI перемикається
  на таб «Алерти» і показує підсумок (плюс окремий warnbox для глав без
  time-маркерів — попередження, не алерт).
- **Вибір проєкту/пресету при запуску**: оверлей-екран, якщо шлях не
  збережений або `.skri/settings.json` не знаходиться: text-input шляху +
  select пресета (нова команда `list_presets` = presets::builtin_list()) +
  «Відкрити» / «Створити новий проєкт» (нова команда `init_project` →
  ProjectStore::init(root, preset, "uk")). Tauri-plugin-dialog НЕ додавався
  (нульова залежність) — діалог вибору папки залишається вдосконаленням.
  Фоновий probe проєкту при старті — тихий (без alert-спаму).
- Структура: старі екрани «Напрямки»/«Моделі» збережені як верхні таби;
  старі повноекранні «Біблія»/«Алерти» замінені сайдбар-табами.
  `tauri.conf.json`: вікно 1280×820 (min 960×620).

### Нові invoke-команди (app/tauri/src/commands.rs; core не змінено)
`list_docs`, `read_doc`, `write_doc`, `list_presets`, `init_project`,
`release_board`, `release_set_status`, `run_checks`.

### Тести
- 2 нові unit-тести в commands.rs: `write_doc_preserves_frontmatter`
  (байт-в-байт header + тіло оновлене) і `list_docs_finds_manuscript`.
```
cargo test --workspace → 94 passed, 0 failed (92 попередні + 2 нові skri-tauri)
cargo check --workspace → Finished (єдине наявне warning ядра: translate.rs unused import)
node --check app.js → OK
```

## 2026-11-05 (продовження) — M6: Журналістика (pitch CRM + факт-леджер) і Нотатник

### Що зроблено
- **pitch.rs** — Pitch CRM + Fact Ledger (§5.11):
  - pitch lifecycle: `pitches(id, title, outlet, status, due_date, sent_date,
    followup_after_days, last_touch, note_doc)` + журнал переходів `pitch_log`
    (перший запис — створення як idea; для статистики часу). Статуси
    idea → pitched → followed_up → assigned → drafting → submitted →
    published|rejected; опційний документ-заметка (--note). CRM-дані живуть
    у БД (як glossary) — перебудувати з файлів їх неможливо, це свідомо.
  - follow-up: due = sent_date + followup_after_days; кандидат = due вже
    настала, а last_touch давніша за due; термінальні статуси не нагорять.
    Backdated sent_date при `set` фіксує last_touch = дата події (запис
    історії заднім числом не гасить нагадування сам собою).
  - `pitch stats`: конверсія idea→published (%), розклад по статусах,
    середній час у статусах з pitch_log (чиста `avg_time_in_status`,
    останній інтервал тягнеться до `now`).
  - факт-леджер: `facts(claim, source_ref, verified 0|1, doc_path,
    char_start, verified_at)`; `fact unverified` — твердження без verified=1
    перед здачею (прив'язані до рукопису — першими); `fact decay [--days]` —
    content decay: перевірене, але старше N днів повертається на
    пере-перевірку. Дати — epoch UTC через публічні release::parse_ymd/
    fmt_date; функції, що залежать від «зараз», приймають `now` параметром
    (чиста логіка тестується без системного часу).
- **notes.rs** — нотатник (§5.10):
  - беклінки: скан notes/*.md на `[[вікі-лінки]]` (укр+латиниця,
    `[[target|alias]]`, `#розділ`/`^блок` відрізаються) з байтовими offset у
    тілі; таблиця `links(from_doc, to_title, target_doc, char_start,
    char_end)` перебудовується ідемпотентно; резолв по імені файла без
    розширення (без регістру) серед УСІХ документів проєкту; звіт
    `notes links` — граф + broken + сироти.
  - daily notes: `notes/YYYY-MM-DD.md` із шаблону пресету
    (`templates.daily-note`, плейсхолдери {{date}}/{{yesterday}}/{{tomorrow}},
    фолбек — вбудований) з auto-insert лінками вчора/завтра; існуючий файл
    НЕ перезаписується. `notes new` — тематична нотатка з topic-note.
  - `notes orphans` (без вхідних), `notes stale [--days 30]` (без змін N
    днів по mtime з docs; manuscript не потрапляє — фільтр kind='notes').
- **Пресети**: новий `presets/journalist-uk.json` (validators:
  fact-unverified-check, pitch-followup; templates: pitch-card, fact-entry;
  сутності джерело/інтерв'ю/твердження/пітч) + `note-taker-uk.json`
  розширено сумісно (validators: broken-links, orphan-notes, stale-notes;
  templates: daily-note, topic-note) — структура маніфесту вже підтримувала
  обидві секції, presets.rs не змінено (крім реєстрації BUILTIN).
- **CLI**: `skri pitch add|list|set|followup|stats`,
  `skri fact add|list|unverified|verify|decay`,
  `skri notes links|orphans|stale|daily|new`; journalist-uk додано в
  огляд напрямків після init. Публічні API інших модулів не змінено.

### Знайдені тестами (і виправлені)
1. Синтетичні epoch у тестах vs `now_epoch()` при створенні: запис створення
   в pitch_log із реальним часом ламав хронологію інтервалів → `add` приймає
   `at` параметром (CLI передає now_epoch()).
2. `avg_time_in_status` повертав середнє без p4 (пітч-ідея без переходів) —
   модульний тест спершу не мав пітча, що залишився в idea; +запис.
3. Тест stale оновлював mtime у docs, а наступний reindex (файл «змінився»
   відносно синтетичного mtime) скидав його назад → реіндекс у тесті
   відпрацьовує ДО підстановки mtime; рукопис із старим mtime перевіряється
   фільтром kind='notes' в одному запиті.
4. Borrow-of-moved у avg_time_in_status (map зі зсувом) → обчислення до move.

### Прогін
```
cargo test --workspace → 106 passed, 0 failed (32 unit, вкл. 3 нові чисті;
  acceptance M0–M5 + 9 нових m6_journal_notes: lifecycle+log, followup-кандидати,
  stats конверсія/час, facts unverified/verify/decay, вікі-лінки укр+лат offsetи,
  резолв+broken+сироти+ідемпотентність, stale по mtime, daily з лінком на вчора,
  обидва пресети; + 2 skri-tauri)
cargo build --workspace → warnings лише наявні (translate.rs)
CLI вручну (журналіст-проєкт): pitch add/list/set (sent 2026-10-01 →
  followup-кандидат «прострочено ~21д»)/stats; fact add→unverified (@30)→verify;
  notes daily (лінк на вчора) → new → links (граф, broken 3) → orphans/stale — OK
```

## 2026-11-05 (продовження) — M7: EPUB-експорт з metadata scrubber (scrub.rs)

### Що зроблено
- **scrub.rs** — EPUB 3 з `manuscript/*.md` (порядок як у release.rs: ведучі
  цифри імені файла, інакше сорт за шляхом) — потреба №15 дослідження
  (pen name management, витік реального імені в метадані самвидаву):
  - ZIP власним записувачем, методом STORED (compression method 0 — валідний
    EPUB; std-only: власний CRC-32, local/central headers, EOCD);
    `mimetype` — ПЕРШИМ записом, без extra-поля; далі META-INF/container.xml,
    OEBPS/content.opf, OEBPS/nav.xhtml (EPUB3 toc), глава = окремий XHTML
    (ch1.xhtml…). markdown→HTML: параграфи, `---`→`<hr/>`, `#`/`##`/`###`,
    інлайн — **реюз публічного `crosspost::inline_html`** (escape+bold+italic);
    `&<>` екрануються завжди; frontmatter у XHTML не потрапляє.
  - метадані: dc:title = frontmatter title першої глави → перший `# ` заголовок
    → ім'я файла; dc:language = frontmatter `lang` → settings `language` → uk;
    dc:identifier = детермінований UUID (v5-подібний з sha256 вмісту) —
    дві збірки → байт-в-байт той самий файл; dc:date = дата публікації
    користувача (`--date` → frontmatter `pub_date` → сьогодні).
  - **Scrub full (за замовчуванням)**: dc:creator = тільки pen name
    (`--pen` → frontmatter `pen_name` → settings `pen_name` → «Автор»),
    dc:publisher геть, у dc:description реальні імена замінюються pen name'ом,
    meta generator прибраний з кожного XHTML, реальне ім'я з frontmatter
    (`author`/`автор`) замінюється pen name'ом І В ТІЛІ глав (copyright-рядок
    теж витікав би), email/url/www з frontmatter не потрапляють у EPUB.
  - **Scrub light (`--light`)**: звичайні метадані лишаються (реальний автор,
    publisher, generator meta).
  - **Scrub-звіт**: чесний список замін/видалень («dc:creator: Володимир
    Вялков → Ash Thornwood», «dc:publisher: Самвидав+ → _rm», «generator: _rm
    (… з N XHTML)», «тіло глав: … → … (N замін)»); зберігається в
    `.skri/scrub-report.json` (останній звіт) + таблиця `epub_exports
    (doc_paths_json, pen_name, scrub_mode, created_at, sha256)`.
- **CLI**: `skri scrub build <папка> <вихід.epub> [--pen «Ash Thornwood»]
  [--light] [--date YYYY-MM-DD]`, `skri scrub report <папка>`.

### Знайдені тестами (і виправлені)
1. Тест EOCD зчитував лічильник записів зі зміщенням сигнатури (n-20 замість
   n-14) — «entries 1541»; офсети EOCD виправлені.

### Тести
7 acceptance m7_scrub: структура EPUB (mimetype перший/STORED/без extra,
container, opf, nav), XHTML-екранування &<> + frontmatter не витікає,
full: dc:creator = pen name і реальне ім'я відсутнє В ЦІЛОМУ файлі (байти),
generator _rm у full / наявний у light (+ реальний автор у light-opf),
звіт перелічує всі заміни і збігається з last_report, sha256 детермінований
+ запис у epub_exports, light-звіт без _rm + валідація дати.
```
cargo test --workspace → 118 passed, 0 failed (37 unit, вкл. 5 нових чистих
  scrub.rs: crc32/uuid/escape/zip-layout/markdown; + 7 m7_scrub; + 2 skri-tauri)
CLI вручну: scrub build (2 глави, full: 5 замін у звіті) → scrub report — OK
EPUB перевірено СТОРОННІМ валідатором (python zipfile+ElementTree):
  testzip чистий, mimetype перший STORED, opf/container/nav XML-валідні,
  creator = Ash Thornwood, publisher відсутній, generator немає,
  реального імені немає в жодному байті файлу
```

## 2026-11-05 (продовження) — M8: легкий CRDT-синк (векторні годинники, merge, конфлікти)

### Що зроблено
- **sync.rs** — легкий ВЛАСНИЙ синк для типової ситуації письменника: один
  автор, 2–3 машини, обмін через спільну папку (Syncthing/USB/хмара) або
  флешку. Повноцінний Automerge/Yjs НЕ додавався (велика залежність
  проти правила «без нових crates») — це однокористувацький multi-device
  merge, не multi-user real-time:
  - векторні годинники: `device_id` (uuid 8-4-4-4-12 з std-only SHA-256)
    у `.skri/device.json`; на кожному доку годинник `device → counter`
    зберігається у frontmatter (`vclock: dev=3;dev2=1`) — файл є джерелом
    правди, годинник подорожує разом із документом;
  - **sync::save_doc** — точка інтеграції для CLI/Tauri: пише тіло
    (frontmatter байт-в-байт, крім рядка `vclock:`), інкрементує СВІЙ
    лічильник, за першим збереженням фіксує базу для diff3 (стан до
    правки), реіндексує; імпорт чужого пакета лічильник НЕ збільшує
    (тільки max по компонентах);
  - merge: один вектор строго домінує інший → fast-forward (звичайний
    випадок); непорівнювані → по-рядковий three-way merge відносно
    `sync_base` (блоки правок обох сторін відносно бази через власний
    LCS): незалежні правки різних рядків застосовуються обидва, правка
    того самого місця з обох боків → маркери `<<<<<<< device-a` /
    `=======` / `>>>>>>> device-b` прямо у файлі + рядки в
    `sync_conflicts` (device, рядок, текст обох варіантів) + алерт
    `kind='sync-conflict'` (ідемпотентний: один відкритий на док);
  - пакет обміну: `skri sync export <папка> <вихід.json>` (JSON:
    вектори + тіла доків, чий хеш тіла ≠ хеш бази) →
    `skri sync import <папка> <пакет.json>` (merge; формат
    `skri-sync-packet` валідується, шляхи з `..` відкидаються);
  - журнал `sync_log(ts, device, doc_path, action merged|fast-forward|
    conflict)` (device = пристрій-джерело правки); `skri sync status`
    показує device_id, годинники всіх доків і останні 20 подій журналу.
- Свідомі обмеження (документовані в док-коментарі): база diff3 живе в БД
  (`sync_base`) — стертої БД деградує до «base = локальне тіло»; маркери
  конфліктів розв'язує автор вручну, наступний цикл export/import
  зшиває стани (після конфлікту база = віддалена версія).

### Знайдені тестами (і виправлені)
1. `compare` векторних годинників мав ПЕРЕПЛУТАНІ напрямки домінування
   (a_ge перевіряв b >= a) — юніт-тест спіймав одразу; домінування тепер
   по ОБ'ЄДНАННЮ ключів (відсутній компонент = 0).
2. Тіло за `frontmatter::parse` завжди починається з `\n` (перенос
   закриваючого `---` входить у тіло) — наївний compose при export/import
   нарощував порожні рядки; `compose_raw` тепер пише тіло без ведучого
   `\n` → `parse(compose(c)) == c`, round-trip байт-стабільний.

### Тести
6 acceptance m8_sync: інкремент годинника при save (+ device.json
стабільний, frontmatter переживає), fast-forward обидва напрямки
(+skip для локально-новішого), конфліктний рядок → маркери з device_id
обох пристроїв + sync_conflicts + алерт, непорівнювані без конфлікту →
автозлиття обох правок, export→import раунд-трип на другому device_id
(2 раунди, без конфліктів), журнал + status. +2 чисті юніти в sync.rs
(годинники/compare, merge3).
```
cargo test --workspace → 126 passed, 0 failed (118 попередніх + 6 m8_sync + 2 unit)
CLI вручну: init→sync status (∅)→sync export (1 док)→init b→sync import
(fast-forward 1)→sync status b (журнал: fast-forward від пристрою A) — OK;
round-trip тіла байт-в-байт, vclock у frontmatter
```

## 2026-11-09 — M9: alerts_meta (людські пояснення алертів) + webnovelist-uk + людські описи пресетів

### Що зроблено
- **alerts_meta.rs** — глобальна мапа видів алертів з людськими поясненнями
  uk/en/ru (title/explain/action) для 24 kind системи; рядки kind звірені
  з фактичними рядками в коді (БД-алерти drift/bible/narrative/crosspost/
  timeline/sync/stats/animation/inkbridge + попередження валідаторів
  timeline-missing, fact-unverified, episode-structure, dialogue-streak,
  mute-test, graph-unreachable, graph-dangling-goto, var-orphan-check,
  var-dead-flag, pitch-followup, broken-links, orphan-notes, stale-notes).
  Поле severity (problem|tip) — щоб UI міг одразу сортувати. Точка входу
  для UI: `alerts_meta::catalog_json()` (масив об'єктів, чиста функція).
- **Пресет webnovelist-uk** — вебновел-серіаліст (RoyalRoad/Wattpad):
  серійні сутності (сюжетна лінія), валідатори name-drift/timeline/
  crosspost-stale/release; зареєстрований у BUILTIN.
- **Людські описи всіх пресетів**: у маніфестах `description` (якого взагалі
  не було в структурі) → `desc` («що програма буде робити для тебе») +
  нове поле `best_for` («Для кого: …») у PresetManifest (serde default —
  зворотна сумісність збережена); `builtin_list()` тепер повертає
  desc+best_for замість технічного «мова: …; сутності: …» — CLI і PresetDto
  в UI оновлюються автоматично.

### Тести
4 acceptance m9_meta (каталог покриває всі kind трьома мовами без порожніх
полів; webnovelist-uk валідний і в BUILTIN; усі пресети мають людський
desc/best_for; ідемпотентність catalog_json) + 1 unit у alerts_meta.
```
```
cargo test -p skri-core -p skri-cli → 129 passed, 0 failed (124 попередні + 5 нових)
cargo test --workspace → skri-tauri на момент прогону не компілюється через
незакомічені правки commands.rs іншого агента (const A: lifetime) — не
стосовується skri-core; його ж коментар каже переключити get_alerts_meta
на alerts_meta::catalog_json() після приземлення модуля
```

## 2026-11-09 — M10: стоп-слова в Index Engine, style-перевірки, ручні досьє

### Контекст
Тест на рукописі російською показав два реальні дефекти:
1. «Казалось» і «Почему» витягнулись як персонажі — стоп-слова не відфільтровані;
2. «Знайдені проблеми: 0» — жодних перевірок граматики/стилю.

### Що зроблено
- **stopwords.rs** — окремий модуль стоп-слів: три списки uk/ru/en по ≥100
  унікальних слів (вводні, займенники, прислівники, частки, дієслова
  мовлення: казалось/почему/когда/нельзя; казалося/чому/отже/звісно;
  seemed/perhaps/however/yesterday; форми з дефісом «что-то», «все-таки»,
  «всё-таки» — токенайзер тримає дефіс всередині слова). Юніт-тест фіксує
  ≥100 унікальних слів у кожному списку і відсутність справжніх імен у списках.
- **indexer.rs — розумніша евристика кандидатів** (двопрохідний скан):
  - стоп-слова uk/ru/en не кандидати (порівняння в нижньому регістрі);
  - ГОЛОВНИЙ детектор «Казалось»: слово з великої ЛИШЕ на початку речення,
    яке нижнім регістром зустрічається деінсь у тексті, — не кандидат
    (pre-pass `lower_seen` по всіх токенах з offsetами);
  - поріг «сильності» одиночного слова підвищено з total ≥ 2 до ≥ 3;
    пара «Ім'я Прізвище» лишається завжди сильною;
  - речення з offsetами реюзнуті (sentences+tokenize), згадки як і раніше
    з байтовими offsetами в тілі.
- **style.rs** — новий модуль правил-перевірок (без AI):
  - `style-tautology` (problem): те саме слово (нормалізоване, ≥3 символи)
    підряд або через одне слово в реченні. Нормалізація = lower + стем-
    префікс (≥5 символів → перші 4), тому «масло масляне» і «сказав сказал»
    ловляться, а «він/вона» — не плутаються;
  - `style-repeat-word` (tip): значуще слово (стоп-фільтр службових) ≥3
    рази в ОДНОМУ реченні;
  - `style-double-space` (tip): подвійні пробіли в тілі (відступи рядків
    і табуляції не рахуються); `char_start` — offset у payload;
  - `style-dash-orphan` (tip): дефіс з пробілами замість тире в діалозі
    («- Привіт»);
  - `run_style_checks(root)` — ідемпотентна (DELETE kind LIKE 'style-%'
    перед прогоном), manuskript/*.md, алерти через новий спільний
    `store::add_alert` (kind, entity_id, payload_json).
- **alerts_meta.rs**: +4 kind (style-tautology = problem; repeat-word,
  double-space, dash-orphan = tip), title/explain/action × uk/en/ru.
- **bible.rs**: `ManualCard { name, aliases, kind, notes }` +
  `add_manual_card(root, card)` — пише `bible/<slug>.md` (frontmatter
  name/aliases/type + notes у тілі); існуючий файл з тим самим name
  перезаписується (оновлення досьє, дублікатів не плодить). sync_bible
  тепер також підтягує `aliases:` з frontmatter в entities.aliases_json —
  карта одразу з'являється в EntityCard з варіантами імені.
- **CLI**: `skri style <папка>` (звіт з severity і offsetами),
  `skri dossier add <папка> <ім'я> [--aliases а,б] [--kind персонаж]
  [--notes файл]` (пише картку + одразу sync_bible), `skri dossier list`.

### Тести
11 acceptance `m10_style.rs`: «Казалось» на початку речення / в середині
з великої — не кандидат; ПІБ пара лишається кандидатом; поріг total≥3
(2 згадки на початках речень = слабкий для drift); тавтологія «масло
масляне» / повтор 3× / службове «сказав» 3× НЕ ловиться / подвійний
пробіл / дефіс-сирота; ідемпотентність run_style_checks; add_manual_card →
sync_bible → EntityCard (aliases+assertions) без алертів і без дубля файла;
повний scan_project російського рукопису без «Казалось/Почему/Дальше».
+3 юніти stopwords (≥100 слів ×3, дефектні слова фільтровані, імена — ні),
+6 юнітів у style.rs.
```
cargo test -p skri-core -p skri-cli → 151 passed, 0 failed (132 попередніх + 19 нових)
```

## 2026-11-09 — Tauri UI: UX-редизайн «людською мовою» (M9-ui)

### Що зроблено (app/tauri/ui + commands.rs; crates/ не чіпано)
- **Онбордінг-майстер при першому запуску** (`skri.onboarded` у localStorage):
  крок 1 «Що ти пишеш?» — 4 великі картки людською мовою (Роман або повість /
  Вебновел-серія (публікація по главах) / Гра або сценарій / Нотатки та статті)
  з одним реченням «що програма буде робити для тебе»; вибір → авто-вибір
  пресету (novelist-uk ×2, game-narrative-uk, note-taker-uk); крок 2 — мова
  інтерфейсу (3 великі кнопки); крок 3 — готово → екран вибору папки з уже
  підставленим пресетом. Майстер перевикликається з вкладки «Початок роботи».
- **Перемикач мови uk/en/ru**: словник `I18N = {uk,en,ru}` в app.js, ВСІ рядки
  через `t()` (+ `plural()` з правилами 1/2-4/5+ для uk/ru, 1/other для en);
  мова в `skri.lang` (дефолт uk), видимий селект у шапці; при зміні мови
  перезаписуються і динамічні списки (картки проблем, kanban, біблія, моделі).
  Ключі трьох мов звірені скриптом: 125 ключів, розбіжностей немає.
- **Людські назви + пояснення**: «Біблія» → «Персонажі та світ» (підпис
  «програма сама збирає відомості про персонажів з твого тексту»), «Алерти» →
  «Знайдені проблеми», «Реліз» → «Публікації»; кожна панель має 1-рядкове
  пояснення; kanban-статуси («Чернетка/Редагування/Запас/Заплановано/
  Опубліковано») і прогноз буфера — людськими фразами; жодного жаргону без
  пояснення (drift → «одне і те саме ім'я пишеться по-різному»).
- **Алерти як людські картки**: команда `get_alerts_meta` — ТОНКА обгортка над
  `skri_core::alerts_meta::catalog_json()` (тексти живуть ТІЛЬКИ в ядрі, у js
  лише emergency-фолбек на 12 kind, якщо команда недоступна). Картка виду =
  людська назва + «Що це означає» + «Що з цим робити» + лічильник; групування
  problem (червоний корінець) → tip (синій); payload рендериться парами
  ключ-значення з перекладеними ключами (variants → «варіанти написання»,
  old_value → «було»…), сирих JSON більше немає; кнопка «Перевірити» отримала
  постійну підказку в шапці («програма прочитає рукопис і знайде розбіжності»);
  глави без time-маркерів — окрема tip-картка, а не warnbox.
- **Екран «Моделі» людською**: кожна задача описана як дія («Напише текст за
  тебе», «Перекладе главу», «Розшифрує запис інтерв'ю»…); бейджі
  «Рекомендовано» / «Працює локально» / «Потрібен інтернет і свій ключ»;
  блок «Що таке Ollama?» простим текстом; технічний JSON сховано в details.
- **Дизайн-система (новий style.css)**: токени темної/світлої теми
  (`[data-theme]`, темна за замовчуванням, перемикач 🌙/☀️ у шапці,
  `skri.theme`), системний шрифт, тіло 16px+, ієрархія 24/20/16, шкала
  відступів 8px, кнопки 44px+ (compact 40px), контрасти WCAG AA,
  focus-visible, порожні стани з інструкціями («Натисни “Створити главу”
  щоб почати») у редакторі, списку глав, біблії, публікаціях і проблемах.
- **Кнопка «✚ Створити главу»** в сайдбарі → нова команда `new_doc(root, title)`:
  manuscript/NNN-назва.md (номер = max ведучих цифр + 1; санітизація
  Windows-заборонених символів і переносів), шаблон frontmatter
  (title + pub_status: draft) + H1-заголовок; нова глава одразу відкривається.
- Усі попередні функції збережено: глава/редактор/Ctrl+S/біблія/канбан/моделі/
  напрямки; generate_handler поповнено `get_alerts_meta` і `new_doc`.

### Тести
1 новий unit у commands.rs: `new_doc_numbers_and_templates` (нумерація NNN,
шаблон frontmatter, санітизація `a/b:c?d` → `a-b-c-d`, реіндекс бачить глави).
```
node --check app/tauri/ui/app.js → OK
cargo check -p skri-tauri → Finished (warnings лише наявні в skri-core)
cargo test -p skri-tauri → 3 passed, 0 failed
E2E у headless-браузері (CDP, мок invoke): онбордінг 4 картки → мова → фініш;
перемикач uk/en переписує весь інтерфейс; тема light/dark; «Перевірити» малює
людські картки (назва/пояснення/дія/лічильник, problem→tip), фільтр і бейдж;
kanban + прогноз людськими фразами; моделі з бейджами; 0 JS-помилок.
```

## 2026-11-10 — QA-фікси P1/P2: frontmatter-захист, рушій фактів досьє, відмінки не drift

### Що зроблено
- **QA-01 (P1) — збереження не пошкоджує frontmatter.** Баг: `write_doc` писав
  тіло без початкового порожнього рядка → на диску `---# Заголовок` (роздільник
  злитий з тілом, файл непарсуваний, frontmatter «протікає» в текст,
  word_count рахує метадані). Фікс: у `frontmatter.rs` нові публічні
  `splice_body(raw, body)` (гарантований перенос після закриваючого `---`,
  header байт-в-байт, round-trip без нарощування порожніх рядків) і
  `title_preserved(before, after)` (інваріант: frontmatter парситься так само
  як до запису). `write_doc` (commands.rs) переписано на splice_body + захисний
  інваріант: після запису файл перечитується; якщо frontmatter не парситься —
  ВІДКАТ до попереднього вмісту і Err з людською укр. відповіддю (захист від
  будь-яких майбутніх регресій з'єднання). Ізоляція шляхів (QA-10) збережена.
- **QA-02 (P1) — рушій фактів досьє.** Новий `facts.rs`:
  - витяг фактів з нотаток досьє (`bible/*.md`) і з глав: вік «число+одиниця»
    (років/рік/лет/год, uk+ru), колір очей «очі <колір>»/«<колір> очі»
    (рос. форми нормалізуються до укр. канону — «зеленые» ≠ хибний конфлікт);
  - факти глави — лише з речень, де сутність/alias згадується (токен-матчинг
    з нормалізацією відмінків); таблиця `extracted_facts(doc_path, char_start,
    entity, kind, value, sentence)`, перебудова ідемпотентна;
  - алерти: `fact-conflict` (≥2 різних значення одного факту в різних главах)
    і `fact-vs-dossier` (глава суперечить канону досьє), severity problem,
    payload {entity, kind, kind_label, value_a/doc_a, value_b/doc_b, message};
  - звірка сутностей через aliases + стаб-нормалізацію відмінків
    (Олександр→Олександра→одна сутність);
  - CLI `skri facts <папка>`: конвеєр індексування → звіт фактів (канон/глава,
    offsetи) + факт-конфлікти з людськими поясненнями.
- **QA-03 (P2) — відмінки не drift.** `drift.rs`: `strip_case_suffix` (стаб
  без лематизації: суфіксний стрип -ою/-ом/-ов/-а/-у/-и/-і/-е, основа ≥4
  символів на приголосній — «чоловічі імена»; «Міра»/«Оля» не чіпаються) +
  попередній прохід у `detect`: відмінкові форми зливаються ТИХО (згадки
  перешиті, форма в aliases) ДО жадібного similarity-групування —
  «Дэвид»/«Дэвида» = 0 drift-алертів, одна сутність.
- **alerts_meta.rs**: +fact-conflict, +fact-vs-dossier (title/explain/action
  uk/en/ru, severity problem).

### Тести
10 acceptance `m11_qa.rs`: тіло з '#' зберігається → файл парситься,
frontmatter не в тілі, word_count без метаданих; тіло з порожнім першим
рядком стабільне; злитий роздільник ловиться інваріантом; header байт-в-байт;
досьє 25 + глави 25/40 → рівно 2 алерти (fact-vs-dossier на 40, fact-conflict
25↔40) + ідемпотентність; очі зелені/блакитні → алерт; без досьє, але конфлікт
у главах → fact-conflict; Олександр/Олександра — одна сутність (alias),
без drift; рос. кольори нормалізуються; Дэвид/Дэвида → 0 drift, одна сутність,
обидві згадки перешиті. +4 юніти frontmatter (splice/інваріант).
```
cargo test -p skri-core -p skri-cli → усі зелені (159 попередніх + 14 нових), 0 failed
CLI вручну (демо-проєкт): facts → 5 фактів (канон+глави), 3 алерти
(2 fact-vs-dossier: вік 40/очі блакитні; 1 fact-conflict 25↔40) з укр. повідомленнями
```
### Відомі обмеження (свідомо)
- Стаб-нормалізація відмінків груба: жіночі імена на -а з довгою основою
  («Марина»→«марин») можуть зливатися з чоловічою формою — повна лематизація
  лишається в M2.
- Факти беруться лише з речень із прямою згадкою сутності (без резолву
  займенників «йому/вона» без імені в реченні).
