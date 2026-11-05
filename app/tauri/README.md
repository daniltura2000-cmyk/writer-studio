# app/tauri — десктопний UI Writer Studio (Tauri 2)

Каркас M2.5/M3.5: Tauri 2 + vanilla JS frontend (без build-степу, `frontendDist: ui`).

## Структура

- `src/commands.rs` — invoke-команди, що викликають публічні API `skri-core`:
  - `list_directions()` — 4 напрямки онбордінгу (опис «якою буде кінцева програма»,
    збагачений `presets::builtin_list()`;
    дзеркало CLI `print_directions` у `crates/skri-cli/src/main.rs`);
  - `model_catalog()` — `models::catalog_json()` (джерело кнопок = те саме, що CLI `skri model menu`);
  - `project_info(root)` — перевірка папки (`.skri/settings.json`) + пресет/мова;
  - `run_index(root)` — `store::reindex` + `indexer::scan_project` + `bible::sync_bible` +
    `merge_auto_into_manual` (зведення як `skri index`);
  - `run_bible(root)` — картки сутностей `bible::cards`;
  - `run_drift(root)` — `drift::detect` + усі невирішені алерти (name-drift + assertion-conflict);
  - `var_audit(root)` — `narrative::scan_dialogue` + `var_audit`;
  - `barks(root)` — `narrative::barks`;
  - `episode_validate(root, rel)` — `animation::validate_episode` + `mute_test` + `check_bible_rules`
    (формат 20-хв епізоду за замовчуванням);
  - `set_model(root, task, model, auto)` — `models::set_model` (кнопка каталогу → settings.json).
- `ui/` — статичний фронт (index.html + app.js + style.css), екрани:
  Напрямки / Моделі / Біблія / Алерти. Стиль — з мокапу `docs/ui/model-picker.html`.
  Фронт звертається до бекенда через `window.__TAURI__.core.invoke`.

## Запуск (повна збірка, коли потрібна)

```bash
# разово: Node 18+ і Rust; фронт статичний, npm-залежностей нема
cd app/tauri
cargo tauri dev    # або: cargo run з цього крейта (режим dev підхоплює ui/)
cargo tauri build  # пакування (bundle поки вимкнений у tauri.conf.json)
```

Якщо `tauri-cli` не встановлений: `cargo install tauri-cli --version "^2"` або
`npm i -g @tauri-apps/cli`. Але для розробки достатньо `cargo run` —
`tauri::generate_context!` вбудовує `ui/` як frontendDist.

## Примітки

- Шлях до проєкту вводиться вручну (папка з `.skri/`); діалог вибору папки —
  `tauri-plugin-dialog` (додати, коли знадобиться: `cargo add tauri-plugin-dialog`).
- Іконка `icons/icon.ico` — заглушка (згенерована); замінити перед релізом.
- `bundle.active = false` — пакування вимкнене до M2.5 завершення.
