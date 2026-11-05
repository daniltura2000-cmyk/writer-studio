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
