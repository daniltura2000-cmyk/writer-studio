# Data Model (M0+M1)

Принцип: **проєкт = папка**, документи = `.md` файли з frontmatter, індекс = SQLite
(відновлюваний з файлів у будь-який момент — індекс завжди можна стерти й перебудувати).

## Структура папки проєкту

```
my-novel/
├── .skri/
│   └── settings.json        # { "preset": "novelist-uk", "language": "uk" }
├── manuscript/              # рукопис: глави/сцени
│   ├── 01-розділ.md
│   └── 02-розділ.md
├── bible/                   # канон: картки сутностей
│   └── kaldrin.md
└── notes/                   # нотатки (Zettelkasten-стиль)
```

## SQLite схема (M0+M1)

```sql
-- Документи (рукопис, біблія, нотатки)
CREATE TABLE docs (
  id INTEGER PRIMARY KEY,
  path TEXT UNIQUE,             -- відносний шлях у проєкті
  kind TEXT,                    -- manuscript | bible | notes
  title TEXT,
  lang TEXT,
  mtime INTEGER,                -- остання індексація
  word_count INTEGER
);

-- Повнотекстовий пошук
CREATE VIRTUAL TABLE docs_fts USING fts5(path, title, body);

-- Сутності (персонажі, місця, предмети; типи задаються пресетом)
CREATE TABLE entities (
  id INTEGER PRIMARY KEY,
  type TEXT,                    -- з пресету: персонаж, локація...
  canonical_form TEXT UNIQUE,   -- канонічне написання
  aliases_json TEXT,            -- варіанти (в т.ч. drift-варіанти)
  source TEXT,                  -- auto | manual
  created_at INTEGER
);

-- Згадки сутностей у документах (char-offsets для переходів)
CREATE TABLE mentions (
  id INTEGER PRIMARY KEY,
  entity_id INTEGER REFERENCES entities(id),
  doc_id INTEGER REFERENCES docs(id),
  char_start INTEGER,
  char_end INTEGER,
  surface_form TEXT,
  sentence TEXT
);

-- Твердження канону («очі зелені», «рівень 12»)
CREATE TABLE assertions (
  id INTEGER PRIMARY KEY,
  entity_id INTEGER REFERENCES entities(id),
  attribute TEXT,
  value TEXT,
  doc_id INTEGER REFERENCES docs(id),
  status TEXT                   -- canonical | conflict | superseded
);

-- Алерти консистентності (name-drift, конфлікти атрибутів)
CREATE TABLE alerts (
  id INTEGER PRIMARY KEY,
  kind TEXT,                    -- name-drift | assertion-conflict
  entity_id INTEGER,
  payload_json TEXT,            -- варіанти, місця, цитати
  created_at INTEGER,
  resolved INTEGER DEFAULT 0
);
```

## Пізніші таблиці (з специфікації, реалізуються по мілстоунах)

`doc_versions` (retcon, M5), `events` (timeline, M5), `vars`/`var_ops` (гейм-наратив, M3),
`barks` (M3), `strings`/`string_translations` (локалізація, M2), `glossary` (M2),
`preset_assets` (M2), `links` (дока↔таски, M5), `notes` (composer, M5).

## Пресет-маніфест (JSON)

```json
{
  "preset": "novelist-uk",
  "language": "uk",
  "entities": [
    {"name": "персонаж", "fields": ["вік", "зовнішність", "арка"]},
    {"name": "локація", "fields": []}
  ],
  "structure": ["твори": ["книга": ["розділ": ["сцена"]]]],
  "checklists": [
    {"name": "перед-публікація", "items": ["consistency-scan", "name-drift"]}
  ],
  "validators": {"name-drift": "on", "assertion-conflict": "on"}
}
```
