# Writer Studio (Скрипторій)

Локальне, безкоштовне, офлайн-first десктоп writing-студіо, яке **читає рукопис сам**:
будує живий Story Bible з тексту, ловить дрейф імен/фактів, керує пресетами жанрів.

> Народжено з дослідження ≈75 невирішених потреб письменників усіх жанрів
> (вебновелі, романи, гейм-наратив, анімація, журналістика, академія, поезія, переклад).
> Див. `docs/research-summary.md`.

## Чому це існує

Усі існуючі інструменти (Scrivener, Novelcrafter, Sudowrite, Obsidian, Articy…) зберігають
те, що автор **ввів вручну**, але жоден не **читає сам рукопис**:

> *"Whether a name is spelled one way in chapter three and another in chapter
> nineteen... no tool catches it."* — огляд Novelcrafter

Writer Studio індексує markdown-рукопис при кожному скануванні (правила + локальні LLM
як підсилювач) і звідти **виводить** канон, а не просить його обслуговувати.

## Залізні правила

1. **Markdown + SQLite на диску — єдине джерело правди.** Проєкт відкривається блокнотом.
2. **Локально і безкоштовно назавжди.** Хмара/AI-хмара — тільки опційно (BYOK).
3. **Кожен жанр = пресет.** Ядро порожнє; композабельні ассети (структури, чеклісти,
   валідатори, AI-рецепти) збираються у власний пресет.

## Статус

| Milestone | Опис | Статус |
|---|---|---|
| M0 | Скелет: Project Store (markdown+SQLite), пресети, CLI | ✅ |
| M1 | Index Engine + Story Bible + name-drift | ✅ |
| M1.5 | Model Router: автовибір моделі по суті задачі (кнопковий каталог) | ✅ |
| M2 | Пресети повні + мультимовність (глосарій, ліміти, T-V, перекладний пайплайн) | ✅ ядро |
| M3 | Гейм-наратив (barks, vars-аудитор, граф) + Анімація (епізод, мют-тест) | ✅ ядро |
| M2.5/M3.5 | Tauri UI (кнопкові пресети-напрямки) | ⬜ |
| M4 | Серійний центр: crosspost, Patreon tiers, recap | ⬜ |
| M5 | LitRPG, retcon, timeline, дока, composer, журналістика | ⬜ |
| M6 | CRDT-синк, колаб | ⬜ |

## Чотири напрямки (пресети) — користувач обирає свій шлях при init

| Пресет | Асети | Кінцева програма |
|---|---|---|
| `novelist-uk` | Story Bible, name-drift, канон-конфлікти | редактор глав, що сам читає рукопис |
| `game-narrative-uk` | dialogue/ text-first, barks, vars-аудитор, ліміти | студія діалогів з валідатором |
| `animation-uk` | Show Bible правила, валідатор епізоду, мют-тест | рев'юер епізодів |
| `note-taker-uk` | атомарні нотатки, композитор | база знань → тексти |

Комбінування: `skri preset compose <назва> entities:<пресет> checklists:<пресет> validators:<пресет> ...`

Детальний план: `ROADMAP.md`. Журнал процесу розробки: `DEVLOG.md`.

## Швидкий старт (CLI)

```bash
git clone https://github.com/daniltura2000-cmyk/writer-studio
cd writer-studio
cargo run -p skri-cli -- init my-novel --preset novelist-uk
cargo run -p skri-cli -- index my-novel
cargo run -p skri-cli -- drift my-novel
cargo run -p skri-cli -- bible my-novel
```

### Вибір AI-моделей — по суті задачі, без вивчення назв моделей

Користувач тисне кнопку задачі — модель підбирається автоматично (перша,
рекомендована локальна); будь-коли перевизначається кнопкою:

```bash
skri model tasks            # список кнопок-задач
skri model menu my-novel    # інтерактивне меню (Enter = автоматично)
skri model auto my-novel translate   # авто: NLLB-200
skri model set  my-novel translate lapa-v0.1.2  # перевизначити
skri model status my-novel
```

Каталог задач і рекомендації з дослідження локального укр.-стеку:
Lapa LLM (генерація укр), NLLB-200 (MT), whisper-uk (STT), LanguageTool
(граматика). Хмара — опція «byok-cloud» в каталозі, але НІКОЛИ не авто.
Мокап майбутнього кнопкового UI: `docs/ui/model-picker.html`.

## Архітектура (M0+M1)

```
crates/skri-core   — бібліотека: store (markdown+SQLite), indexer, drift, bible, presets
crates/skri-cli    — CLI: init / index / bible / drift / preset
app/tauri          — десктопний UI (M1+, окремий milestone)
```

Документи: `docs/data-model.md`, `docs/architecture.md`.

## Ліцензія

MIT. Безкоштовний локальний режим — повнофункціональний назавжди.
