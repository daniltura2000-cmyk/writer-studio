// Writer Studio UI — vanilla JS поверх Tauri invoke-команд.
// Tauri-вікно: window.__TAURI__.core.invoke; звичайний браузер — попередження.
//
// Принципи цього UI:
// 1) УСІ рядки інтерфейсу — через t() (I18N: uk/en/ru), мова в localStorage.
// 2) Жодного жаргону без пояснення: кожна панель має 1-рядкове пояснення.
// 3) Тексти видів алертів приходять з бекенду (get_alerts_meta) — у js лише
//    emergency-фолбек, якщо команда недоступна.

const invoke = window.__TAURI__?.core?.invoke;
const $ = (id) => document.getElementById(id);
let root = localStorage.getItem("skri.root") || "";
let projGen = 0; // покоління проєкту: +1 при кожному відкритті (QA-10 гонка)
let gotoRegistry = []; // алерт → кнопка «Перейти» (для цитати при переході)

/* ============================================================
   I18N — словник інтерфейсу
   ============================================================ */
const I18N = {
  uk: {
    no_invoke: "Це інтерфейс програми: запускайте через Tauri (cargo run), інакше команди недоступні.",
    // шапка
    open: "Відкрити",
    proj_ph: "Шлях до папки проєкту",
    proj_ok: "Проєкт відкрито",
    proj_bad: "Ця папка — не проєкт Writer Studio",
    check: "Перевірити",
    check_hint: "«Перевірити» — програма прочитає рукопис і знайде розбіжності: різні написання імен, суперечливі факти, стрибки в часі.",
    // вкладки
    tab_studio: "Студія",
    tab_start: "Початок роботи",
    tab_models: "Моделі",
    // сайдбар
    t_chapters: "Глави",
    chapters_sub: "Твої файли рукопису з папки manuscript/. Натисни на главу, щоб відкрити її в редакторі.",
    btn_new_chapter: "✚ Створити главу",
    btn_refresh: "Оновити",
    new_chapter_prompt: "Назва нової глави:", 
    new_chapter_default: "Нова глава",
    no_docs: "Тут поки порожньо. Натисни «Створити главу», щоб почати писати.",
    words: ["слово", "слова", "слів"],
    chars: ["символ", "символи", "символів"],
    untitled: "Без назви",
    // біблія
    t_bible: "Персонажі та світ",
    bible_sub: "Програма сама збирає відомості про персонажів і світ з твого тексту: імена, варіанти написання, згадані факти.",
    btn_rescan: "Перечитати рукопис",
    no_entities: "Тут поки порожньо. Натисни «＋ Додати досьє», щоб створити картку вручну, або поклади глави в manuscript/ і натисни «Перечитати рукопис».",
    mentions: ["згадка", "згадки", "згадок"],
    also_known: "також зустрічається як",
    found_in: "зустрічається у",
    facts: "факти з тексту:",
    // досьє автора
    dossier_hint_main: "Створи досьє на персонажа — програма стежитиме, щоб його описи в усіх главах не суперечили один одному.",
    dossier_hint_auto: "Нижче — те, що програма знайшла в тексті сама.",
    btn_add_dossier: "＋ Додати досьє",
    dos_new: "Нове досьє",
    dos_edit: "Редагувати досьє",
    dos_name: "Ім'я:",
    dos_name_ph: "Наприклад: Міра Велан",
    dos_aliases: "Псевдоніми (через кому):",
    dos_aliases_ph: "Наприклад: Міра, Мірочка",
    dos_kind: "Тип:",
    kind_character: "Персонаж",
    kind_location: "Локація",
    kind_item: "Предмет",
    kind_other: "Інше",
    dos_notes: "Що важливо стежити:",
    dos_notes_ph: "Зріст, шрами, характер, як говорить…",
    dos_save: "Зберегти досьє",
    dos_cancel: "Скасувати",
    dos_need_name: "Вкажи ім'я.",
    src_manual: "Досьє автора",
    src_auto: "Знайдено в тексті",
    btn_edit_card: "Редагувати",
    // алерти
    t_alerts: "Знайдені проблеми",
    alerts_sub: "Ось що програма помітила у рукописі. Синім — поради, червоним — те, що варто виправити.",
    filter_all: "Всі",
    chapters_w: ["глава", "глави", "глав"],
    no_alerts: "Проблем не знайдено — рукопис узгоджений. Натисни «Перевірити» вгорі, щоб перевірити знову.",
    what_means: "Що це означає:",
    what_do: "Що з цим робити:",
    show_n: "Показати",

    problem_w: ["проблему", "проблеми", "проблем"],
    tip_w: ["пораду", "поради", "порад"],
    missing_time_title: "Глави без позначок часу",
    missing_time_explain: "У цих главах програма не знайшла жодної позначки часу («наступного ранку», «через три дні», дата).",
    missing_time_action: "Додай хоч одну позначку часу в кожну главу — тоді програма стежитиме за хронологією історії.",
    sum_checks: "Перевірено {d} {dw} ({w} {ww}).",
    sum_found: "Знайдено: {p} {pw} і {tp} {tw}.",
    sum_drift_only: "Знайдено {g} {gw} з різними написаннями імен. Натисни «Перевірити» вгорі, щоб перевірити весь рукопис.",
    drift_gw: ["групу", "групи", "груп"],
    checking: "Читаю рукопис…",
    // реліз
    t_release: "Публікації",
    release_sub: "Дошка публікацій: рухай главу ліворуч-праворуч по шляху від чернетки до опублікованої.",
    fc_title: "Скільки вистачить запасу",
    fc_speed: "Ти пишеш приблизно {n} слів на день",
    fc_buffer: "У запасі: {c} {cw} ({w} {ww})",
    fc_buffer_w: ["глава", "глави", "глав"],
    fc_days_ch: "Одна глава займає близько {n} днів",
    fc_left: "За такої швидкості запас вичерпається приблизно через {n} днів",
    fc_exhaust: "Запас закінчиться приблизно {d}",
    fc_unknown: "Поки не вдається порахувати: потрібно щонайменше дві глави з датами збереження.",
    st_draft: "Чернетка",
    st_edit: "Редагування",
    st_buffer: "Запас",
    st_scheduled: "Заплановано",
    st_published: "Опубліковано",
    move_to: "Перенести в «{s}»",
    done_pub: "✔ Опубліковано",
    no_release: "Тут поки порожньо — створи главу на вкладці «Глави».",
    // редактор
    save: "Зберегти (Ctrl+S)",
    saved_at: "Збережено о {t}",
    no_doc_save: "Спершу відкрий главу ліворуч.",
    save_suspicious: "⚠ Збереження виглядає підозріло — перевір файл",
    save_wrong_project: "Ця глава з іншого проєкту — збереження скасовано. Відкрий главу в поточному проєкті.",
    goto: "Перейти",
    accept_variant: "Прийняти варіант",
    ignore: "Ігнорувати",
    accepted_alias: "Записано в досьє як псевдонім",
    sum_entities: "Досьє автора: {m} · Знайдено в тексті: {a}",
    models_settings_note: "Це налаштування. Щоб виконати задачу — скоро: редактор отримає кнопки",
    ollama_ready: "Готово (локально)",
    ollama_missing: "Не встановлено",
    ed_empty_t: "Почніть писати",
    ed_empty_d: "Відкрий главу ліворуч або натисни «Створити главу». Frontmatter (службовий блок на початку файлу) тут не редагується — лише текст.",
    ed_ph: "Текст глави…",
    ed_meta_fm: "службові поля:",
    ed_meta_nofm: "без службового блоку",
    confirm_unsaved: "Є незбережені зміни. Перейти без збереження?",
    fqa09_save_first: "Зберегти незбережені зміни перед відкриттям іншого проєкту?",
    fqa09_save_failed: "Не вдалося зберегти — проєкт НЕ змінено.",
    fqa09_discard_confirm: "Скинути незбережені зміни?",
    fqa04_save_first: "Зберегти зміни перед перевіркою?",
    fqa04_stale_draft: "Перевірено збережену версію — твої правки ще не збережені",
    fqa03_age: "вік",
    fqa03_eye: "колір очей",
    fqa03_vs: "{e} · {kind}: у главі «{b}» ({db}), у досьє «{a}» ({da})",
    fqa03_conflict: "{e} · {kind}: у главі «{a}» ({da}) і «{b}» ({db}) — суперечність",
    fqa06_date_invalid: "Дату не збережено: потрібен реальний день у форматі РРРР-ММ-ДД (наприклад 2026-11-17). Статус не змінено.",
    opt_gen_lapa_l: "Lapa LLM v0.1.2 — Ollama `uamarchuan/lapa-v0.1.2-instruct`",
    opt_gen_lapa_n: "найкраща відкрита укр. модель (Gemma3 12B, 128K ctx). Моделі <7B ламають укр. орфографію — тримайте 7B+",
    opt_gen_llama_l: "Llama 3.1 8B Instruct — Ollama",
    opt_gen_llama_n: "загальна модель; укр. вихід слабший за Lapa",
    opt_byok_l: "Хмара (свій API-ключ)",
    opt_byok_n_full: "опційно, з явним дозволом; ніколи не обирається автоматично",
    opt_byok_n_short: "опційно",
    opt_tr_nllb_l: "NLLB-200 (локально)",
    opt_tr_nllb_n: "baseline укр. MT на 200 мов; реєстр ти/ви перевіряти окремим проходом",
    opt_tr_lapa_l: "Lapa LLM v0.1.2 — стильовий переклад",
    opt_tr_lapa_n: "глосарій серії в промпті; для художнього ритму",
    opt_tr_wturbo_l: "whisper large-v3-turbo (укр. fine-tune Yehor, WER 12.75%)",
    opt_tr_wturbo_n: "github.com/egorsmkv/speech-recognition-uk",
    opt_tr_wv3_l: "whisper large-v3 (базовий)",
    opt_tr_wv3_n: "плутає укр/рос на кириличному міксі → мову задавати вручну",
    opt_gr_lt_l: "LanguageTool server (локально)",
    opt_gr_lt_n: "укр + 25+ мов, без хмари; вимкнений Live-Pattern конфлікт не стосується",
    opt_gr_goroh_l: "Goroh + dmklinger укр. словник",
    opt_gr_goroh_n: "офлайн-перевірка написання без сервера",
    opt_sum_lapa_l: "Lapa LLM v0.1.2 — резюме з індексу сутностей",
    opt_sum_lapa_n: "вхід — не повний текст, а агрегат Index Engine",
    opt_sum_llama_l: "Llama 3.1 8B Instruct — Ollama",
    opt_sum_llama_n: "якщо Lapa не встановлена",
    opt_ss_rules_l: "Правила + FTS5 (без моделей)",
    opt_ss_rules_n: "детерміновано; embedding-моделі для укр. поки слабкі",
    opt_ss_e5_l: "multilingual-e5-base (експеримент)",
    opt_ss_e5_n: "коли з'явиться сильніша укр. embedding-модель",
    fqa06_date_prompt: "Дата публікації для «Заплановано» (РРРР-ММ-ДД):",
    fqa11_loc: "Сторінка: глава {n}",
    fqa13_preset_kept: "Напрям застосовується до НОВОГО проєкту. Поточний проєкт залишається без змін (пресет: {x})",
    // моделі (QA-08: чесний каталог)
    models_t: "Налаштування моделей",
    models_sub: "Для різних справ програма використовує різні «мозки». Вибери справу — покажемо, хто її виконує. Перша модель завжди рекомендована і працює на твоєму комп'ютері.",
    pick_task: "Оберіть справу",
    pick_task_hint: "Натисни кнопку зі справою зверху — покажемо доступні моделі.",
    mt_generate_uk_t: "Зможе дописати текст",
    mt_generate_uk_d: "Чернетки сцен, продовження речення, підправити стиль — українською.",
    mt_translate_t: "Перекладе главу",
    mt_translate_d: "Переклад на іншу мову, імена персонажів лишаться правильними.",
    mt_transcribe_uk_t: "Розшифрує запис інтерв'ю",
    mt_transcribe_uk_d: "Запис розмови перетворить на текст — усе на твоєму комп'ютері.",
    mt_grammar_t: "Перевірить орфографію і стиль",
    mt_grammar_d: "Підказки під час письма: українська і ще понад 25 мов.",
    mt_summarize_t: "Нагадає, що було раніше",
    mt_summarize_d: "Короткий переказ попередніх глав, щоб не заплутатися в сюжеті.",
    mt_semantic_search_t: "Знайде схоже у тексті",
    mt_semantic_search_d: "Схожі сцени й описи — звичайним пошуком, без штучного інтелекту.",
    badge_rec: "Рекомендовано",
    badge_local: "Працює локально",
    badge_cloud: "Потрібен інтернет і свій ключ",
    ollama_t: "Що таке Ollama?",
    ollama_d: "Ollama — це невелика програма, яка запускає штучний інтелект прямо на твоєму комп'ютері: без інтернету, без підписки і без оплати за кожне запитання. Якщо Ollama встановлена, Writer Studio пише й перекладає текст локально — жоден твій текст не відправляється в мережу. Встановити можна безкоштовно з ollama.com.",
    tech_t: "Технічні подробиці (для розробників)",
    tech_note: "Вибір зберігається в .skri/settings.json → розділ «models». Те саме доступне з командного рядка: skri model menu|auto|set|status.",
    // онбордінг
    wiz_q: "Що ти пишеш?",
    wiz_sub: "Відповідь налаштує програму під твою справу. Пізніше можна змінити.",
    w_novel_t: "Роман або повість",
    w_novel_d: "Програма читатиме твій рукопис: збиратиме персонажів, стежитиме за іменами і фактами, щоб у 12-й главі не з'явилися очі іншого кольору.",
    w_web_t: "Вебновел-серія (публікація по главах)",
    w_web_d: "Все, що для роману, плюс дошка публікацій: яка глава чернетка, яка в запасі, і на скільки днів вистачить запасу.",
    w_game_t: "Гра або сценарій",
    w_game_d: "Програма стежитиме за діалогами і змінними, збиратиме короткі репліки персонажів і перевірятиме структуру епізодів.",
    w_notes_t: "Нотатки та статті",
    w_notes_d: "Програма зв'язуватиме нотатки посиланнями, підкаже, які факти ще не перевірені, і нагадає надіслати статтю.",
    wiz_lang_q: "Якою мовою показувати програму?",
    wiz_lang_sub: "Можна змінити в будь-який момент — перемикач у верхньому правому куті.",
    wiz_done_t: "Готово!",
    wiz_done_d: "Тепер відкрий папку проєкту або створи нову — і можна писати.",
    wiz_finish: "Почати",
    btn_back: "← Назад",
    start_t: "Початок роботи",
    start_sub: "Два швидкі питання — і програма налаштується під те, що ти пишеш: роман, вебновел, сценарій гри чи нотатки.",
    start_btn: "Пройти швидке налаштування",
    // вибір проєкту
    setup_t: "Відкрий або створи проєкт",
    setup_sub: "Проєкт — це папка з твоїми текстами (у ній програма тримає службові дані .skri/). Вкажи шлях до наявної папки або створи нову.",
    setup_ph: "Наприклад: C:\\Мої тексти\\Мій роман",
    setup_preset: "Набір під твій напрямок:",
    setup_open: "Відкрити папку",
    setup_create: "Створити новий проєкт",
    setup_need_path: "Спершу вкажи шлях до папки.",
    setup_notproj: "Ця папка поки що не проєкт. Можеш виправити шлях або створити проєкт тут.",
    setup_create_fail: "Не вдалося створити:",
    // діалоги-інструменти
    dlg_tools: "Інструменти для ігор і сценаріїв",
    btn_vars: "Аудит змінних",
    btn_barks: "Таблиця реплік",
    btn_episode: "Перевірити епізод",
    ep_ph: "manuscript/ep01.md",
    need_ep: "Вкажи файл епізоду, наприклад manuscript/ep01.md",
    th_var: "змінна", th_set: "встановлюється", th_check: "перевіряється", th_unset: "скидається", th_status: "стан",
    v_orphan: "перевіряється, але ніде не встановлюється",
    v_dead: "встановлюється, але ніде не використовується",
    v_ok: "✔ усе гаразд",
    th_trigger: "тригер", th_priority: "пріоритет", th_cooldown: "пауза", th_conditions: "умови", th_variants: "варіанти",
    ep_passed: "Структура епізоду в порядку",
    ep_issues: "Є зауваження",
    ep_cold_open: "вступний кадр",
    ep_turns: "повороти сюжету",
    ep_hook: "гачок у кінці",
    none: "немає",
    th_scene: "сцена", th_action: "дії", th_dialogue: "діалоги", th_ratio: "частка дій", th_verdict: "висновок",
    ep_rule: "можливий реткон:",
    // інше
    err_cmd: "Сталася помилка",
    theme_btn: "Змінити тему",
    sec: "с",
  },

  en: {
    no_invoke: "This is the app UI: run it through Tauri (cargo run), otherwise the commands are unavailable.",
    open: "Open",
    proj_ph: "Path to the project folder",
    proj_ok: "Project opened",
    proj_bad: "This folder is not a Writer Studio project",
    check: "Check",
    check_hint: "“Check” — the app reads your manuscript and finds inconsistencies: name spellings, contradictory facts, timeline jumps.",
    tab_studio: "Studio",
    tab_start: "Get started",
    tab_models: "Models",
    t_chapters: "Chapters",
    chapters_sub: "Your manuscript files from the manuscript/ folder. Click a chapter to open it in the editor.",
    btn_new_chapter: "✚ New chapter",
    btn_refresh: "Refresh",
    new_chapter_prompt: "Title of the new chapter:",
    new_chapter_default: "New chapter",
    no_docs: "Nothing here yet. Click “New chapter” to start writing.",
    words: ["word", "words", "words"],
    chars: ["character", "characters", "characters"],
    untitled: "Untitled",
    t_bible: "Characters & world",
    bible_sub: "The app collects facts about your characters and world from the text by itself: names, spellings, mentioned details.",
    btn_rescan: "Re-read manuscript",
    no_entities: "Nothing here yet. Click “＋ Add dossier” to create a card by hand, or put chapters into manuscript/ and click “Re-read manuscript”.",
    mentions: ["mention", "mentions", "mentions"],
    also_known: "also appears as",
    found_in: "appears in",
    facts: "facts from the text:",
    dossier_hint_main: "Create a character dossier — the app will make sure its descriptions never contradict each other across chapters.",
    dossier_hint_auto: "Below — what the app found in the text by itself.",
    btn_add_dossier: "＋ Add dossier",
    dos_new: "New dossier",
    dos_edit: "Edit dossier",
    dos_name: "Name:",
    dos_name_ph: "For example: Mira Velan",
    dos_aliases: "Aliases (comma-separated):",
    dos_aliases_ph: "For example: Mira, Mirka",
    dos_kind: "Type:",
    kind_character: "Character",
    kind_location: "Location",
    kind_item: "Item",
    kind_other: "Other",
    dos_notes: "What to keep track of:",
    dos_notes_ph: "Height, scars, temperament, how they speak…",
    dos_save: "Save dossier",
    dos_cancel: "Cancel",
    dos_need_name: "Enter a name.",
    src_manual: "Author's dossier",
    src_auto: "Found in the text",
    btn_edit_card: "Edit",
    t_alerts: "Found issues",
    alerts_sub: "What the app noticed in your manuscript. Blue — advice, red — worth fixing.",
    filter_all: "All",
    chapters_w: ["chapter", "chapters", "chapters"],
    no_alerts: "No issues found — the manuscript is consistent. Click “Check” above to run the check again.",
    what_means: "What this means:",
    what_do: "What to do:",
    show_n: "Show",
    problem_w: ["issue", "issues", "issues"],
    tip_w: ["tip", "tips", "tips"],
    missing_time_title: "Chapters without time markers",
    missing_time_explain: "The app found no time markers (“next morning”, “three days later”, a date) in these chapters.",
    missing_time_action: "Add at least one time marker per chapter — then the app can keep track of your story's chronology.",
    sum_checks: "Checked {d} {dw} ({w} {ww}).",
    sum_found: "Found: {p} {pw} and {tp} {tw}.",
    sum_drift_only: "Found {g} {gw} of names with different spellings. Click “Check” above to run the full check.",
    drift_gw: ["group", "groups", "groups"],
    checking: "Reading the manuscript…",
    t_release: "Publishing",
    release_sub: "Your publishing board: move chapters from draft to published.",
    fc_title: "How long the buffer lasts",
    fc_speed: "You write about {n} words per day",
    fc_buffer: "In the buffer: {c} {cw} ({w} {ww})",
    fc_buffer_w: ["chapter", "chapters", "chapters"],
    fc_days_ch: "One chapter takes about {n} days",
    fc_left: "At this pace the buffer runs out in about {n} days",
    fc_exhaust: "The buffer will run out around {d}",
    fc_unknown: "Can't calculate yet: at least two chapters with dates are needed.",
    st_draft: "Draft",
    st_edit: "Editing",
    st_buffer: "Buffer",
    st_scheduled: "Scheduled",
    st_published: "Published",
    move_to: "Move to “{s}”",
    done_pub: "✔ Published",
    no_release: "Nothing here yet — create a chapter on the “Chapters” tab.",
    save: "Save (Ctrl+S)",
    saved_at: "Saved at {t}",
    no_doc_save: "Open a chapter on the left first.",
    save_suspicious: "⚠ Saving looks suspicious — check the file",
    save_wrong_project: "This chapter belongs to another project — save cancelled. Open a chapter in the current project.",
    goto: "Go to",
    accept_variant: "Accept spelling",
    ignore: "Ignore",
    accepted_alias: "Saved to the dossier as an alias",
    sum_entities: "Author's dossiers: {m} · Found in text: {a}",
    models_settings_note: "This is a setting. To actually run the job — soon: the editor will get buttons",
    ollama_ready: "Ready (local)",
    ollama_missing: "Not installed",
    ed_empty_t: "Start writing",
    ed_empty_d: "Open a chapter on the left or click “New chapter”. Frontmatter (the service block at the top of the file) is not edited here — only the text.",
    ed_ph: "Chapter text…",
    ed_meta_fm: "frontmatter:",
    ed_meta_nofm: "no frontmatter",
    confirm_unsaved: "There are unsaved changes. Leave without saving?",
    fqa09_save_first: "Save unsaved changes before opening another project?",
    fqa09_save_failed: "Could not save — project NOT switched.",
    fqa09_discard_confirm: "Discard unsaved changes?",
    fqa04_save_first: "Save changes before checking?",
    fqa04_stale_draft: "The saved version was checked — your edits are not saved yet",
    fqa03_age: "age",
    fqa03_eye: "eye color",
    fqa03_vs: "{e} · {kind}: chapter says “{b}” ({db}), dossier says “{a}” ({da})",
    fqa03_conflict: "{e} · {kind}: “{a}” ({da}) and “{b}” ({db}) — contradiction",
    fqa06_date_invalid: "Date not saved: a real calendar day in YYYY-MM-DD format is required (e.g. 2026-11-17). Status unchanged.",
    opt_gen_lapa_l: "Lapa LLM v0.1.2 — Ollama `uamarchuan/lapa-v0.1.2-instruct`",
    opt_gen_lapa_n: "the best open Ukrainian model (Gemma3 12B, 128K ctx). Models under 7B break Ukrainian spelling — keep 7B+",
    opt_gen_llama_l: "Llama 3.1 8B Instruct — Ollama",
    opt_gen_llama_n: "general model; Ukrainian output weaker than Lapa",
    opt_byok_l: "Cloud (your own API key)",
    opt_byok_n_full: "optional, with explicit permission; never selected automatically",
    opt_byok_n_short: "optional",
    opt_tr_nllb_l: "NLLB-200 (local)",
    opt_tr_nllb_n: "baseline Ukrainian MT across 200 languages; check the tu/vy register in a separate pass",
    opt_tr_lapa_l: "Lapa LLM v0.1.2 — stylistic translation",
    opt_tr_lapa_n: "series glossary in the prompt; for literary rhythm",
    opt_tr_wturbo_l: "whisper large-v3-turbo (Ukrainian fine-tune by Yehor, WER 12.75%)",
    opt_tr_wturbo_n: "github.com/egorsmkv/speech-recognition-uk",
    opt_tr_wv3_l: "whisper large-v3 (base)",
    opt_tr_wv3_n: "confuses Ukrainian/Russian on a Cyrillic mix → set the language manually",
    opt_gr_lt_l: "LanguageTool server (local)",
    opt_gr_lt_n: "Ukrainian + 25+ languages, no cloud; the disabled Live-Pattern conflict does not apply",
    opt_gr_goroh_l: "Goroh + dmklinger Ukrainian dictionary",
    opt_gr_goroh_n: "offline spelling check without a server",
    opt_sum_lapa_l: "Lapa LLM v0.1.2 — summaries from the entity index",
    opt_sum_lapa_n: "the input is not the full text but an Index Engine aggregate",
    opt_sum_llama_l: "Llama 3.1 8B Instruct — Ollama",
    opt_sum_llama_n: "if Lapa is not installed",
    opt_ss_rules_l: "Rules + FTS5 (no models)",
    opt_ss_rules_n: "deterministic; Ukrainian embedding models are still weak",
    opt_ss_e5_l: "multilingual-e5-base (experimental)",
    opt_ss_e5_n: "until a stronger Ukrainian embedding model appears",
    epi_no_cold_open: "No cold open — the animation format starts with a cold open (a teaser before the titles)",
    epi_too_few_turns: "Narrative turns: {turns} < {min_turns} minimum for a 20-minute episode",
    epi_no_hook: "No hook — a final hook scene for the next episode is required",
    epi_dialogue_scenes_in_row: "Scene “{scene}”: {streak} dialogue scenes in a row (> {max_dialogue_scenes_in_row}); “characters standing around engaging in lengthy conversations” is a red flag for animation",
    epi_mute_empty_scene: "Empty scene",
    epi_mute_visual_pacing_ok: "The story reads without sound",
    epi_mute_dialogue_heavy_scene: "Half the picture is dialogue; check with the mute test",
    epi_mute_mute_violation: "The scene “goes dark” without sound: dialogue does the picture’s job",
    epi_bible_rule_hit: "Bible rule: “{rule}” — indicator words appear in the episode ({hits}). Is this a retcon or a mistake?",
    fqa06_date_prompt: "Publication date for “Scheduled” (YYYY-MM-DD):",
    fqa11_loc: "Page: chapter {n}",
    fqa13_preset_kept: "The direction applies to a NEW project. The current project stays unchanged (preset: {x})",
    models_t: "Model settings",
    models_sub: "For different jobs the app uses different “brains”. Pick a job and we'll show who does it. The first model is always recommended and runs on your computer.",
    pick_task: "Pick a job",
    pick_task_hint: "Click a job button above — we'll show the available models.",
    mt_generate_uk_t: "Will be able to draft text",
    mt_generate_uk_d: "Scene drafts, sentence continuation, style fixes — in Ukrainian.",
    mt_translate_t: "Translates a chapter",
    mt_translate_d: "Translation into another language while character names stay correct.",
    mt_transcribe_uk_t: "Transcribes an interview",
    mt_transcribe_uk_d: "Turns a conversation recording into text — entirely on your computer.",
    mt_grammar_t: "Checks spelling and style",
    mt_grammar_d: "Hints while you write: Ukrainian and 25+ more languages.",
    mt_summarize_t: "Reminds you what happened",
    mt_summarize_d: "A short recap of previous chapters so the plot stays clear.",
    mt_semantic_search_t: "Finds similar passages",
    mt_semantic_search_d: "Similar scenes and descriptions — plain search, no AI.",
    badge_rec: "Recommended",
    badge_local: "Runs locally",
    badge_cloud: "Needs internet and your API key",
    ollama_t: "What is Ollama?",
    ollama_d: "Ollama is a small program that runs AI right on your computer: no internet, no subscription, no per-request fees. If Ollama is installed, Writer Studio writes and translates locally — none of your text ever leaves your machine. Install it for free from ollama.com.",
    tech_t: "Technical details (for developers)",
    tech_note: "The choice is stored in .skri/settings.json → the “models” section. Also available from the command line: skri model menu|auto|set|status.",
    wiz_q: "What are you writing?",
    wiz_sub: "Your answer sets the app up for your kind of work. You can change it later.",
    w_novel_t: "A novel or novella",
    w_novel_d: "The app reads your manuscript: collects characters, tracks names and facts, so chapter 12 doesn't suddenly have eyes of a different color.",
    w_web_t: "A web-novel series (published chapter by chapter)",
    w_web_d: "Everything the novel setup does, plus a publishing board: which chapter is a draft, which is buffered, and how long the buffer will last.",
    w_game_t: "A game or screenplay",
    w_game_d: "The app tracks dialogue and variables, collects short character lines (barks) and checks episode structure.",
    w_notes_t: "Notes and articles",
    w_notes_d: "The app links notes together, shows which facts are still unverified and reminds you to send the article out.",
    wiz_lang_q: "Which language for the app?",
    wiz_lang_sub: "You can change it any time — the switch is in the top-right corner.",
    wiz_done_t: "All set!",
    wiz_done_d: "Now open a project folder or create a new one — and start writing.",
    wiz_finish: "Get started",
    btn_back: "← Back",
    start_t: "Get started",
    start_sub: "Two quick questions and the app adapts to what you write: a novel, a web-novel, a game script or notes.",
    start_btn: "Run the quick setup",
    setup_t: "Open or create a project",
    setup_sub: "A project is a folder with your texts (the app keeps its service data in .skri/ inside). Point to an existing folder or create a new one.",
    setup_ph: "For example: C:\\My texts\\My novel",
    setup_preset: "Kit for your kind of work:",
    setup_open: "Open folder",
    setup_create: "Create a new project",
    setup_need_path: "First enter the folder path.",
    setup_notproj: "This folder is not a project yet. Fix the path, or create a project right here.",
    setup_create_fail: "Could not create:",
    dlg_tools: "Tools for games and screenplays",
    btn_vars: "Variable audit",
    btn_barks: "Line table",
    btn_episode: "Check episode",
    ep_ph: "manuscript/ep01.md",
    need_ep: "Enter the episode file, e.g. manuscript/ep01.md",
    th_var: "variable", th_set: "is set", th_check: "is checked", th_unset: "is unset", th_status: "status",
    v_orphan: "checked but never set",
    v_dead: "set but never used",
    v_ok: "✔ all good",
    th_trigger: "trigger", th_priority: "priority", th_cooldown: "cooldown", th_conditions: "conditions", th_variants: "variants",
    ep_passed: "Episode structure looks good",
    ep_issues: "There are notes",
    ep_cold_open: "cold open",
    ep_turns: "story turns",
    ep_hook: "end hook",
    none: "none",
    th_scene: "scene", th_action: "action", th_dialogue: "dialogue", th_ratio: "action ratio", th_verdict: "verdict",
    ep_rule: "possible retcon:",
    err_cmd: "An error occurred",
    theme_btn: "Switch theme",
    sec: "s",
  },

  ru: {
    no_invoke: "Это интерфейс программы: запускайте через Tauri (cargo run), иначе команды недоступны.",
    open: "Открыть",
    proj_ph: "Путь к папке проекта",
    proj_ok: "Проект открыт",
    proj_bad: "Эта папка — не проект Writer Studio",
    check: "Проверить",
    check_hint: "«Проверить» — программа прочитает рукопись и найдёт расхождения: разные написания имён, противоречивые факты, скачки во времени.",
    tab_studio: "Студия",
    tab_start: "Начало работы",
    tab_models: "Модели",
    t_chapters: "Главы",
    chapters_sub: "Ваши файлы рукописи из папки manuscript/. Нажмите на главу, чтобы открыть её в редакторе.",
    btn_new_chapter: "✚ Создать главу",
    btn_refresh: "Обновить",
    new_chapter_prompt: "Название новой главы:",
    new_chapter_default: "Новая глава",
    no_docs: "Здесь пока пусто. Нажмите «Создать главу», чтобы начать писать.",
    words: ["слово", "слова", "слов"],
    chars: ["символ", "символа", "символов"],
    untitled: "Без названия",
    t_bible: "Персонажи и мир",
    bible_sub: "Программа сама собирает сведения о персонажах и мире из вашего текста: имена, варианты написания, упомянутые факты.",
    btn_rescan: "Перечитать рукопись",
    no_entities: "Здесь пока пусто. Нажмите «＋ Добавить досье», чтобы создать карточку вручную, или положите главы в manuscript/ и нажмите «Перечитать рукопись».",
    mentions: ["упоминание", "упоминания", "упоминаний"],
    also_known: "также встречается как",
    found_in: "встречается в",
    facts: "факты из текста:",
    dossier_hint_main: "Создайте досье на персонажа — программа будет следить, чтобы его описания во всех главах не противоречили друг другу.",
    dossier_hint_auto: "Ниже — то, что программа нашла в тексте сама.",
    btn_add_dossier: "＋ Добавить досье",
    dos_new: "Новое досье",
    dos_edit: "Редактировать досье",
    dos_name: "Имя:",
    dos_name_ph: "Например: Мира Велан",
    dos_aliases: "Псевдонимы (через запятую):",
    dos_aliases_ph: "Например: Мира, Мирочка",
    dos_kind: "Тип:",
    kind_character: "Персонаж",
    kind_location: "Локация",
    kind_item: "Предмет",
    kind_other: "Другое",
    dos_notes: "За чем важно следить:",
    dos_notes_ph: "Рост, шрамы, характер, как говорит…",
    dos_save: "Сохранить досье",
    dos_cancel: "Отмена",
    dos_need_name: "Укажите имя.",
    src_manual: "Досье автора",
    src_auto: "Найдено в тексте",
    btn_edit_card: "Редактировать",
    t_alerts: "Найденные проблемы",
    alerts_sub: "Вот что программа заметила в рукописи. Синим — советы, красным — то, что стоит исправить.",
    filter_all: "Все",
    chapters_w: ["глава", "главы", "глав"],
    no_alerts: "Проблем не найдено — рукопис согласован. Нажмите «Проверить» сверху, чтобы проверить снова.",
    what_means: "Что это значит:",
    what_do: "Что с этим делать:",
    show_n: "Показать",
    problem_w: ["проблему", "проблемы", "проблем"],
    tip_w: ["совет", "совета", "советов"],
    missing_time_title: "Главы без отметок времени",
    missing_time_explain: "В этих главах программа не нашла ни одной отметки времени («на следующее утро», «через три дня», дата).",
    missing_time_action: "Добавьте хотя бы одну отметку времени в каждую главу — тогда программа будет следить за хронологией истории.",
    sum_checks: "Проверено {d} {dw} ({w} {ww}).",
    sum_found: "Найдено: {p} {pw} и {tp} {tw}.",
    sum_drift_only: "Найдено {g} {gw} с разными написаниями имён. Нажмите «Проверить» сверху, чтобы проверить всю рукопись.",
    drift_gw: ["группу", "группы", "групп"],
    checking: "Читаю рукопись…",
    t_release: "Публикации",
    release_sub: "Доска публикаций: двигайте главу по пути от черновика до опубликованной.",
    fc_title: "На сколько хватит запаса",
    fc_speed: "Вы пишете примерно {n} слов в день",
    fc_buffer: "В запасе: {c} {cw} ({w} {ww})",
    fc_buffer_w: ["глава", "главы", "глав"],
    fc_days_ch: "Одна глава занимает около {n} дней",
    fc_left: "При таком темпе запас исчерпается примерно через {n} дней",
    fc_exhaust: "Запас закончится примерно {d}",
    fc_unknown: "Пока не удаётся посчитать: нужно минимум две главы с датами сохранения.",
    st_draft: "Черновик",
    st_edit: "Редактирование",
    st_buffer: "Запас",
    st_scheduled: "Запланировано",
    st_published: "Опубликовано",
    move_to: "Перенести в «{s}»",
    done_pub: "✔ Опубликовано",
    no_release: "Здесь пока пусто — создайте главу на вкладке «Главы».",
    save: "Сохранить (Ctrl+S)",
    saved_at: "Сохранено в {t}",
    no_doc_save: "Сначала откройте главу слева.",
    save_suspicious: "⚠ Сохранение выглядит подозрительно — проверьте файл",
    save_wrong_project: "Эта глава из другого проекта — сохранение отменено. Откройте главу в текущем проекте.",
    goto: "Перейти",
    accept_variant: "Принять вариант",
    ignore: "Игнорировать",
    accepted_alias: "Записано в досье как псевдоним",
    sum_entities: "Досье автора: {m} · Найдено в тексте: {a}",
    models_settings_note: "Это настройка. Чтобы выполнить задачу — скоро: редактор получит кнопки",
    ollama_ready: "Готово (локально)",
    ollama_missing: "Не установлено",
    ed_empty_t: "Начните писать",
    ed_empty_d: "Откройте главу слева или нажмите «Создать главу». Frontmatter (служебный блок в начале файла) здесь не редактируется — только текст.",
    ed_ph: "Текст главы…",
    ed_meta_fm: "служебные поля:",
    ed_meta_nofm: "без служебного блока",
    confirm_unsaved: "Есть несохранённые изменения. Перейти без сохранения?",
    fqa09_save_first: "Сохранить несохранённые изменения перед открытием другого проекта?",
    fqa09_save_failed: "Не удалось сохранить — проект НЕ переключён.",
    fqa09_discard_confirm: "Сбросить несохранённые изменения?",
    fqa04_save_first: "Сохранить изменения перед проверкой?",
    fqa04_stale_draft: "Проверена сохранённая версия — ваши правки ещё не сохранены",
    fqa03_age: "возраст",
    fqa03_eye: "цвет глаз",
    fqa03_vs: "{e} · {kind}: в главе «{b}» ({db}), в досье «{a}» ({da})",
    fqa03_conflict: "{e} · {kind}: «{a}» ({da}) и «{b}» ({db}) — противоречие",
    fqa06_date_invalid: "Дата не сохранена: нужен реальный день в формате ГГГГ-ММ-ДД (например 2026-11-17). Статус не изменён.",
    opt_gen_lapa_l: "Lapa LLM v0.1.2 — Ollama `uamarchuan/lapa-v0.1.2-instruct`",
    opt_gen_lapa_n: "лучшая открытая укр. модель (Gemma3 12B, 128K ctx). Модели <7B ломают укр. орфографию — держите 7B+",
    opt_gen_llama_l: "Llama 3.1 8B Instruct — Ollama",
    opt_gen_llama_n: "общая модель; укр. вывод слабже, чем у Lapa",
    opt_byok_l: "Облако (свой API-ключ)",
    opt_byok_n_full: "опционально, с явным разрешением; никогда не выбирается автоматически",
    opt_byok_n_short: "опционально",
    opt_tr_nllb_l: "NLLB-200 (локально)",
    opt_tr_nllb_n: "базовый укр. MT на 200 языков; реестр ты/вы проверять отдельным проходом",
    opt_tr_lapa_l: "Lapa LLM v0.1.2 — стилевой перевод",
    opt_tr_lapa_n: "глоссарий серии в промпте; для художественного ритма",
    opt_tr_wturbo_l: "whisper large-v3-turbo (укр. fine-tune Yehor, WER 12.75%)",
    opt_tr_wturbo_n: "github.com/egorsmkv/speech-recognition-uk",
    opt_tr_wv3_l: "whisper large-v3 (базовый)",
    opt_tr_wv3_n: "путает укр/рос на кириллическом миксе → язык задавать вручную",
    opt_gr_lt_l: "LanguageTool server (локально)",
    opt_gr_lt_n: "укр + 25+ языков, без облака; выключенный Live-Pattern конфликт не касается",
    opt_gr_goroh_l: "Goroh + dmklinger укр. словарь",
    opt_gr_goroh_n: "офлайн-проверка написания без сервера",
    opt_sum_lapa_l: "Lapa LLM v0.1.2 — резюме из индекса сущностей",
    opt_sum_lapa_n: "вход — не полный текст, а агрегат Index Engine",
    opt_sum_llama_l: "Llama 3.1 8B Instruct — Ollama",
    opt_sum_llama_n: "если Lapa не установлена",
    opt_ss_rules_l: "Правила + FTS5 (без моделей)",
    opt_ss_rules_n: "детерминированно; embedding-модели для укр. пока слабкие",
    opt_ss_e5_l: "multilingual-e5-base (эксперимент)",
    opt_ss_e5_n: "когда появится более сильная укр. embedding-модель",
    epi_no_cold_open: "Нет cold open — анимационный формат начинается с холодного открытия (тизер до титров)",
    epi_too_few_turns: "Narrative turns: {turns} < {min_turns} минимума для 20-минутного эпизода",
    epi_no_hook: "Нет hook — финальная сцена-крючок для следующего эпизода обязательна",
    epi_dialogue_scenes_in_row: "Сцена «{scene}»: {streak} диалоговых сцен подряд (> {max_dialogue_scenes_in_row}); «персонажи стоят и долго разговаривают» — для анимации это красный флаг",
    epi_mute_empty_scene: "Пустая сцена",
    epi_mute_visual_pacing_ok: "История читается без звука",
    epi_mute_dialogue_heavy_scene: "Половина картины — диалог; проверьте mute-тестом",
    epi_mute_mute_violation: "Сцена «гаснет» без звука: диалог выполняет работу картинки",
    epi_bible_rule_hit: "Правило библии: «{rule}» — в эпизоде упомянуты слова-индикаторы ({hits}). Это retcon или ошибка?",
    fqa06_date_prompt: "Дата публикации для «Запланировано» (ГГГГ-ММ-ДД):",
    fqa11_loc: "Страница: глава {n}",
    fqa13_preset_kept: "Направление применяется к НОВОМУ проекту. Текущий проект остаётся без изменений (пресет: {x})",
    models_t: "Настройки моделей",
    models_sub: "Для разных дел программа использует разные «мозги». Выберите дело — покажем, кто его выполняет. Первая модель всегда рекомендована и работает на вашем компьютере.",
    pick_task: "Выберите дело",
    pick_task_hint: "Нажмите кнопку с делом сверху — покажем доступные модели.",
    mt_generate_uk_t: "Сможет дописать текст",
    mt_generate_uk_d: "Черновики сцен, продолжение фраз, правка стиля — на украинском.",
    mt_translate_t: "Переведёт главу",
    mt_translate_d: "Перевод на другой язык, имена персонажей останутся правильными.",
    mt_transcribe_uk_t: "Расшифрует запись интервью",
    mt_transcribe_uk_d: "Превратит запись разговора в текст — всё на вашем компьютере.",
    mt_grammar_t: "Проверит орфографию и стиль",
    mt_grammar_d: "Подсказки во время письма: украинский и ещё 25+ языков.",
    mt_summarize_t: "Напомнит, что было раньше",
    mt_summarize_d: "Краткий пересказ предыдущих глав, чтобы не запутаться в сюжете.",
    mt_semantic_search_t: "Найдёт похожее в тексте",
    mt_semantic_search_d: "Похожие сцены и описания — обычным поиском, без искусственного интеллекта.",
    badge_rec: "Рекомендовано",
    badge_local: "Работает локально",
    badge_cloud: "Нужен интернет и свой ключ",
    ollama_t: "Что такое Ollama?",
    ollama_d: "Ollama — это небольшая программа, которая запускает искусственный интеллект прямо на вашем компьютере: без интернета, без подписки и без оплаты за каждый запрос. Если Ollama установлена, Writer Studio пишет и переводит текст локально — ни один ваш текст не уходит в сеть. Установить можно бесплатно с ollama.com.",
    tech_t: "Технические подробности (для разработчиков)",
    tech_note: "Выбор сохраняется в .skri/settings.json → раздел «models». То же доступно из командной строки: skri model menu|auto|set|status.",
    wiz_q: "Что вы пишете?",
    wiz_sub: "Ответ настроит программу под ваше дело. Позже можно изменить.",
    w_novel_t: "Роман или повесть",
    w_novel_d: "Программа будет читать вашу рукопись: собирать персонажей, следить за именами и фактами, чтобы в 12-й главе не появились глаза другого цвета.",
    w_web_t: "Вебновел-серия (публикация по главам)",
    w_web_d: "Всё то же, что для романа, плюс доска публикаций: какая глава черновик, какая в запасе, и на сколько дней хватит запаса.",
    w_game_t: "Игра или сценарий",
    w_game_d: "Программа будет следить за диалогами и переменными, собирать короткие реплики персонажей и проверять структуру эпизодов.",
    w_notes_t: "Заметки и статьи",
    w_notes_d: "Программа свяжет заметки ссылками, подскажет, какие факты ещё не проверены, и напомнит отправить статью.",
    wiz_lang_q: "На каком языке показывать программу?",
    wiz_lang_sub: "Можно изменить в любой момент — переключатель в правом верхнем углу.",
    wiz_done_t: "Готово!",
    wiz_done_d: "Теперь откройте папку проекта или создайте новую — и можно писать.",
    wiz_finish: "Начать",
    btn_back: "← Назад",
    start_t: "Начало работы",
    start_sub: "Два быстрых вопроса — и программа настроится под то, что вы пишете: роман, вебновел, сценарий игры или заметки.",
    start_btn: "Пройти быструю настройку",
    setup_t: "Откройте или создайте проект",
    setup_sub: "Проект — это папка с вашими текстами (программа хранит служебные данные в .skri/ внутри). Укажите путь к существующей папке или создайте новую.",
    setup_ph: "Например: C:\\Мои тексты\\Мой роман",
    setup_preset: "Набор под ваше направление:",
    setup_open: "Открыть папку",
    setup_create: "Создать новый проект",
    setup_need_path: "Сначала укажите путь к папке.",
    setup_notproj: "Эта папка пока не проект. Можно исправить путь или создать проект прямо здесь.",
    setup_create_fail: "Не удалось создать:",
    dlg_tools: "Инструменты для игр и сценариев",
    btn_vars: "Аудит переменных",
    btn_barks: "Таблица реплик",
    btn_episode: "Проверить эпизод",
    ep_ph: "manuscript/ep01.md",
    need_ep: "Укажите файл эпизода, например manuscript/ep01.md",
    th_var: "переменная", th_set: "устанавливается", th_check: "проверяется", th_unset: "сбрасывается", th_status: "состояние",
    v_orphan: "проверяется, но нигде не устанавливается",
    v_dead: "устанавливается, но нигде не используется",
    v_ok: "✔ всё в порядке",
    th_trigger: "триггер", th_priority: "приоритет", th_cooldown: "пауза", th_conditions: "условия", th_variants: "варианты",
    ep_passed: "Структура эпизода в порядке",
    ep_issues: "Есть замечания",
    ep_cold_open: "вступительный кадр",
    ep_turns: "повороты сюжета",
    ep_hook: "крючок в конце",
    none: "нет",
    th_scene: "сцена", th_action: "действия", th_dialogue: "диалоги", th_ratio: "доля действий", th_verdict: "вывод",
    ep_rule: "возможный реткон:",
    err_cmd: "Произошла ошибка",
    theme_btn: "Сменить тему",
    sec: "с",
  },
};

let lang = localStorage.getItem("skri.lang") || "uk";
if (!I18N[lang]) lang = "uk";

function t(key, params) {
  let s = (I18N[lang] && I18N[lang][key]) ?? I18N.uk[key] ?? key;
  if (params) for (const [k, v] of Object.entries(params)) s = s.replaceAll("{" + k + "}", String(v));
  return s;
}

function plural(n, forms) {
  if (lang === "en") return n === 1 ? forms[0] : forms[1];
  const a = Math.abs(n) % 100, b = a % 10;
  if (a > 10 && a < 20) return forms[2];
  if (b > 1 && b < 5) return forms[1];
  if (b === 1) return forms[0];
  return forms[2];
}

function applyI18n() {
  document.documentElement.lang = lang;
  document.querySelectorAll("[data-i18n]").forEach((el) => { el.textContent = t(el.dataset.i18n); });
  document.querySelectorAll("[data-i18n-ph]").forEach((el) => { el.placeholder = t(el.dataset.i18nPh); });
  $("langSel").value = lang;
  $("themeBtn").title = t("theme_btn");
  $("themeBtn").textContent = document.documentElement.dataset.theme === "light" ? "🌙" : "☀️";
  // динамічні частини, якщо вже завантажені
  if (lastProjOk !== null) setProjState(lastProjOk, lastProjPreset); // FQA-05: статус у шапці
  // FQA-05 (повторно відкрито): існуючі результати перемальовуємо в новій
  // мові БЕЗ повторного прогону — loadAlerts() замінював повний підсумок
  // Check-а на drift-only «Found 0 groups…», хоч fact-картки лишались.
  if (lastAlerts) loadMeta().then(renderAlerts);
  if (currentDoc) renderDocMeta(currentDoc); // FQA-05: «службові поля» теж мовою UI
  if (lastBoard) renderRelease(lastBoard);
  if (lastCards) renderBible(lastCards);
  if (catalog.length) {
    // FQA-05: каталог моделей перемальовується людськими назвами; поточний
    // вибір задачі відновлюємо за id (після перебудови кнопок об'єкт новий)
    loadCatalog().then(() => {
      if (currentTask) {
        const cur = catalog.find((x) => x.id === currentTask.id);
        if (cur) selectTask(cur, true);
      }
    });
  }
  if (currentTask && !catalog.length) selectTask(currentTask, true);
  // FQA-05: результати інструментів перемальовуємо зі збережених даних
  if (lastVars) renderVars(lastVars);
  if (lastBarks) renderBarks(lastBarks);
  if (lastEpisode) renderEpisode(lastEpisode.rel, lastEpisode.r);
  if (root) loadDocs();
  updateCounts();
  renderWizardCards();
}

// --- перемикач мови (шапка) ---
$("langSel").onchange = () => {
  lang = $("langSel").value;
  localStorage.setItem("skri.lang", lang);
  applyI18n();
};

// --- перемикач теми (темна за замовчуванням) ---
function setTheme(th) {
  document.documentElement.dataset.theme = th;
  localStorage.setItem("skri.theme", th);
  $("themeBtn").textContent = th === "light" ? "🌙" : "☀️";
}
$("themeBtn").onclick = () =>
  setTheme(document.documentElement.dataset.theme === "light" ? "dark" : "light");

/* ============================================================
   інфраструктура
   ============================================================ */
function esc(s) {
  return String(s).replace(/[&<>\\\"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

async function cmd(name, args = {}) {
  const r = await cmdq(name, args);
  if (r.err != null) {
    alert(t("err_cmd") + " (" + name + "):\n" + r.err);
    throw new Error(r.err);
  }
  return r.ok;
}

// тихий варіант: не показує alert (для фонових перевірок при запуску)
async function cmdq(name, args = {}) {
  if (!invoke) return { err: "no invoke" };
  try {
    return { ok: await invoke(name, { root: root || undefined, ...args }) };
  } catch (e) {
    return { err: String(e) };
  }
}

// --- верхні вкладки ---
document.querySelectorAll("nav .tab").forEach((tb) => {
  tb.onclick = () => {
    document.querySelectorAll("nav .tab").forEach((x) => x.classList.remove("active"));
    document.querySelectorAll(".screen").forEach((x) => x.classList.remove("active"));
    tb.classList.add("active");
    $(tb.dataset.tab).classList.add("active");
  };
});

/* ============================================================
   ОНБОРДІНГ-МАЙСТЕР
   ============================================================ */
const WIZ_PRESETS = {
  novel: "novelist-uk",
  webnovel: "novelist-uk",
  game: "game-narrative-uk",
  notes: "note-taker-uk",
};
let wizPreset = null;

function renderWizardCards() {
  $("wizCards").innerHTML = [
    ["novel", "w_novel_t", "w_novel_d", "📖"],
    ["webnovel", "w_web_t", "w_web_d", "📰"],
    ["game", "w_game_t", "w_game_d", "🎮"],
    ["notes", "w_notes_t", "w_notes_d", "🗂️"],
  ]
    .map(
      ([id, tk, dk, ico]) =>
        `<button class="wiz-card" data-id="${id}"><span class="t">${ico} ${esc(t(tk))}</span><span class="d">${esc(t(dk))}</span></button>`
    )
    .join("");
  $("wizCards").querySelectorAll("button").forEach((b) => {
    b.onclick = () => {
      wizPreset = WIZ_PRESETS[b.dataset.id];
      wizGo(2);
    };
  });
  $("wizLangs").innerHTML = [
    ["uk", "Українська"],
    ["en", "English"],
    ["ru", "Русский"],
  ]
    .map(([l, nm]) => `<button class="wiz-lang-btn" data-lang="${l}">${nm}</button>`)
    .join("");
  $("wizLangs").querySelectorAll("button").forEach((b) => {
    b.onclick = () => {
      lang = b.dataset.lang;
      localStorage.setItem("skri.lang", lang);
      applyI18n();
      wizGo(3);
    };
  });
}

function wizGo(step) {
  document.querySelectorAll(".wiz-step").forEach((s) => s.classList.toggle("active", s.dataset.step === String(step)));
}

function showWizard() {
  renderWizardCards();
  wizGo(1);
  $("wizard").classList.remove("hidden");
}

$("wizBack").onclick = () => wizGo(1);
$("wizFinish").onclick = () => {
  $("wizard").classList.add("hidden");
  localStorage.setItem("skri.onboarded", "1");
  // після майстра: проєкт уже відкритий → працюємо; ні → екран вибору папки
  if (root) {
    // FQA-13: напрям, обраний у майстрі, стосується ЛИШЕ нового проєкту.
    // Відкритий проєкт не перенастроюємо — чесно попереджаємо.
    if (wizPreset && lastProjOk && lastProjPreset && wizPreset !== lastProjPreset) {
      alert(t("fqa13_preset_kept", { x: lastProjPreset }));
    }
    openProject(root).then((ok) => { if (ok === false) showSetup(false); }); // "cancelled" → нічого не робимо
  }
  else { if (wizPreset) selectSetupPreset(wizPreset); showSetup(false); }
};
$("startWizard").onclick = showWizard;

/* ============================================================
   ПРОЄКТ (шапка + оверлей вибору)
   ============================================================ */
// FQA-05: останній стан шапки зберігаємо, щоб перемикання мови могло
// перезмалювати «Проєкт відкрито» з t() без повторного project_info.
let lastProjOk = null;
let lastProjPreset = "";
function setProjState(ok, preset) {
  lastProjOk = ok;
  lastProjPreset = preset || "";
  const el = $("projState");
  el.textContent = ok
    ? `${t("proj_ok")}${lastProjPreset ? " — " + lastProjPreset : ""}`
    : t("proj_bad");
  el.className = "proj-state " + (ok ? "ok" : "bad");
}

// QA-10 (ізоляція проєктів): при зміні проєкту відкрита глава старого
// не має права перезаписати файл нового. Скидаємо ВСЕ, що тримало старий
// проєкт у пам'яті/DOM: главу, редактор, біблію, алерти, канбан, фільтри.
function resetProjectState() {
  currentDoc = null;
  dirty = false;
  editor.value = "";
  updateCounts(); // QA-10: лічильник «6 слів» попередньої глави не може залишатись
  $("docTitle").textContent = "";
  $("docMeta").textContent = "";
  $("saveState").textContent = "";
  $("saveState").className = "note";
  $("editorEmpty").classList.remove("hidden");
  $("docList").innerHTML = "";
  // біблія
  lastCards = null;
  delete $("cards").dataset.loaded;
  $("cards").innerHTML = "";
  $("dossierForm").classList.add("hidden");
  dosEditing = null;
  // алерти + фільтр
  lastAlerts = null;
  alertSummaryBase = "";
  $("alertList").innerHTML = "";
  $("alertsSummary").textContent = "";
  $("alertFilter").innerHTML = `<option value="">${esc(t("filter_all"))}</option>`;
  // FQA-10: бейдж ховається ГАРАНТОВАНО — і атрибут hidden, і нульовий текст
  // (CSS .badge-n більше не перебиває display [hidden])
  const ab = $("alertBadge");
  ab.hidden = true;
  ab.textContent = "0";
  // канбан
  lastBoard = null;
  delete $("kanban").dataset.loaded;
  $("kanban").innerHTML = "";
  $("forecastBox").innerHTML = "";
  // інструменти діалогів
  $("varsOut").innerHTML = "";
  $("barksOut").innerHTML = "";
  $("epOut").innerHTML = "";
}

async function openProject(path, quiet = false) {
  // FQA-09 (P1): незбережений текст НЕ може зникати при зміні проєкту.
  // Три стани двома confirm: «Зберегти» → save і продовжити;
  // «Скинути» → продовжити; «Скасувати» → взагалі не чіпати проєкт.
  // Результат: true = відкрито, false = не проєкт skri, "cancelled" =
  // користувач відмовився (жодних змін стану, callers НЕ показують помилку).
  if (!quiet && currentDoc && dirty) {
    if (confirm(t("fqa09_save_first"))) {
      const r = await cmdq("write_doc", { rel: currentDoc.path, body: editor.value });
      if (r.err != null) { alert(t("fqa09_save_failed") + "\n" + r.err); return "cancelled"; }
      dirty = false;
    } else if (!confirm(t("fqa09_discard_confirm"))) {
      return "cancelled"; // скасовано — залишаємось у поточному проєкті з текстом
    }
  }
  root = path;
  projGen++; // QA-10: stale-результати перевірок попереднього проєкту ігноруються
  localStorage.setItem("skri.root", root);
  // спершу скидаємо стан СТАРОГО проєкту — до будь-яких запитів
  resetProjectState();
  $("projRoot").value = root;
  $("setupRoot").value = root;
  const r = await cmdq("project_info");
  if (r.err != null) {
    setProjState(false, "");
    return false;
  }
  const s = r.ok;
  setProjState(true, s?.preset ?? "");
  $("setup").classList.add("hidden");
  await loadDocs();
  return true;
}

$("openProj").onclick = async () => {
  const ok = await openProject($("projRoot").value.trim());
  if (ok === "cancelled") return; // користувач відмовився — НЕ «не проєкт»
  if (!ok) {
    $("setupMsg").textContent = t("setup_notproj");
    $("setupMsg").className = "note err";
    showSetup(false);
  }
};

function showSetup(fillRoot = true) {
  if (fillRoot && !$("setupRoot").value) $("setupRoot").value = root;
  $("setup").classList.remove("hidden");
}

function selectSetupPreset(id) {
  const sel = $("setupPreset");
  if ([...sel.options].some((o) => o.value === id)) sel.value = id;
}

async function fillPresets() {
  try {
    const presets = await cmd("list_presets");
    $("setupPreset").innerHTML = presets
      .map((p) => `<option value="${esc(p.id)}">${esc(p.id)} — ${esc(p.desc)}</option>`)
      .join("");
  } catch { /* без invoke — поле просто порожнє */ }
}

$("setupOpen").onclick = async () => {
  const p = $("setupRoot").value.trim();
  if (!p) { $("setupMsg").textContent = t("setup_need_path"); $("setupMsg").className = "note err"; return; }
  const ok = await openProject(p);
  if (ok === "cancelled") return; // відмова — без «папка не проєкт»
  if (!ok) { $("setupMsg").textContent = t("setup_notproj"); $("setupMsg").className = "note err"; }
};

$("setupCreate").onclick = async () => {
  const p = $("setupRoot").value.trim();
  if (!p) { $("setupMsg").textContent = t("setup_need_path"); $("setupMsg").className = "note err"; return; }
  try {
    root = p; // cmd() підставляє root сам — до створення він ще не був виставлений
    localStorage.setItem("skri.root", root);
    const r = await cmdq("init_project", { preset: $("setupPreset").value });
    if (r.err != null) { $("setupMsg").textContent = t("setup_create_fail") + " " + r.err; $("setupMsg").className = "note err"; return; }
    $("setupMsg").textContent = "";
    await openProject(p);
  } catch (e) {
    $("setupMsg").textContent = t("setup_create_fail") + " " + e;
  }
};

/* ============================================================
   СТУДІЯ: сайдбар-таби
   ============================================================ */
document.querySelectorAll(".stab").forEach((tb) => {
  tb.onclick = () => {
    document.querySelectorAll(".stab").forEach((x) => x.classList.remove("active"));
    document.querySelectorAll(".stabpane").forEach((x) => x.classList.remove("active"));
    tb.classList.add("active");
    $("stab-" + tb.dataset.stab).classList.add("active");
    if (tb.dataset.stab === "bible" && !$("cards").dataset.loaded) loadBible();
    if (tb.dataset.stab === "alerts" && !lastAlerts) loadAlerts();
    if (tb.dataset.stab === "release" && !$("kanban").dataset.loaded) loadRelease();
  };
});

/* ============================================================
   ГЛАВИ + РЕДАКТОР
   ============================================================ */
let currentDoc = null;
let dirty = false;

async function loadDocs() {
  if (!root) return;
  const docs = await cmdq("list_docs");
  if (docs.err != null) return;
  $("docList").innerHTML =
    docs.ok
      .map(
        (d) => `<button class="doc ${currentDoc && currentDoc.path === d.path ? "sel" : ""}" data-path="${esc(d.path)}">
        <span class="t">${esc(d.title)}</span>
        <span class="m">${d.word_count} ${plural(d.word_count, I18N[lang].words)}</span>
      </button>`
      )
      .join("") ||
    `<div class="empty"><span class="ico">📝</span>${esc(t("no_docs"))}</div>`;
  $("docList").querySelectorAll(".doc").forEach((b) => {
    b.onclick = () => openDoc(b.dataset.path);
  });
}
$("btnReloadDocs").onclick = loadDocs;

$("btnNewDoc").onclick = async () => {
  if (!root) { showSetup(false); return; }
  const title = prompt(t("new_chapter_prompt"), t("new_chapter_default"));
  if (title === null) return;
  const rel = await cmd("new_doc", { title: title.trim() || t("new_chapter_default") });
  await loadDocs();
  await openDoc(rel);
};

// шапка метаданих редактора (pub_status тощо) — спільна для openDoc і
// FQA-08 (оновлення після release_set_status)
function renderDocMeta(d) {
  $("docMeta").textContent = d.has_frontmatter
    ? t("ed_meta_fm") + " " + d.meta.map(([k, v]) => `${k}=${v}`).join(", ")
    : t("ed_meta_nofm");
}

async function openDoc(path) {
  if (dirty && !confirm(t("confirm_unsaved"))) return null; // FQA-11: скасовано — нічого не завантажено
  const d = await cmd("read_doc", { rel: path });
  currentDoc = d;
  dirty = false;
  $("editor").value = d.body;
  $("docTitle").textContent = d.title;
  renderDocMeta(d);
  $("saveState").textContent = "";
  $("editorEmpty").classList.add("hidden");
  updateCounts();
  document.querySelectorAll(".doc").forEach((b) => b.classList.toggle("sel", b.dataset.path === path));
  return d; // FQA-11: викликати може перевірити, що редактор справді завантажено
}

const editor = $("editor");
editor.addEventListener("input", () => { dirty = true; updateCounts(); });

function updateCounts() {
  const body = editor.value;
  const words = (body.match(/\S+/g) || []).length;
  $("cntWords").textContent = `${words} ${plural(words, I18N[lang].words)} · ${body.length} ${plural(body.length, I18N[lang].chars)}`;
}

async function saveDoc() {
  if (!currentDoc) { $("saveState").textContent = t("no_doc_save"); return; }
  // QA-10: глава має належати ПОТОЧНОМУ проєкту. read_doc віддає
  // project_root — якщо він не збігається з відкритим root, збереження
  // скасовуємо ще ДО виклику write_doc (бекенд додатково відмовляє шляхи
  // поза коренем — подвійний захист).
  if (currentDoc.project_root && !sameRoot(currentDoc.project_root, root)) {
    dirty = false;
    currentDoc = null;
    $("saveState").className = "note err";
    $("saveState").textContent = t("save_wrong_project");
    return;
  }
  try {
    await cmd("write_doc", { rel: currentDoc.path, body: editor.value });
    // QA-01: тиха повторна перевірка — прочитали файл назад і звірили з тим,
    // що мали зберегти. Порожній/без тексту при непорожньому редакторі =
    // підозріло: показуємо попередження і НЕ ховаємо статус (не пишемо
    // «збережено», не скидаємо dirty — користувач має перевірити файл).
    const r = await cmdq("read_doc", { rel: currentDoc.path });
    const body = r.ok && typeof r.ok.body === "string" ? r.ok.body : null;
    if (r.err != null || (body !== null && !body.trim() && editor.value.trim())) {
      $("saveState").className = "note err";
      $("saveState").textContent = t("save_suspicious");
      return;
    }
    dirty = false;
    const now = new Date();
    $("saveState").className = "note";
    $("saveState").textContent = t("saved_at", {
      t: String(now.getHours()).padStart(2, "0") + ":" + String(now.getMinutes()).padStart(2, "0"),
    });
    await loadDocs(); // свіжі word_count у списку
  } catch { /* помилку вже показано в cmd() */ }
}
// порівняння коренів: роздільники/регістр/хвостові слеші не мають значення
function sameRoot(a, b) {
  const norm = (p) => String(p || "").replace(/\\+/g, "/").replace(/\/+$/, "").toLowerCase();
  return norm(a) === norm(b);
}
$("btnSave").onclick = saveDoc;

// Ctrl+S — збереження (глобально, перехоплюємо браузерний діалог)
window.addEventListener("keydown", (e) => {
  if ((e.ctrlKey || e.metaKey) && (e.key === "s" || e.key === "S")) {
    e.preventDefault();
    if (document.activeElement === editor || currentDoc) saveDoc();
  }
});

/* ============================================================
   ПЕРСОНАЖІ ТА СВІТ (біблія)
   ============================================================ */
let lastCards = null;

// канонічні типи досьє → ключі i18n (невідомі типи показуються як є)
const KIND_I18N = { character: "kind_character", location: "kind_location", item: "kind_item", other: "kind_other" };
// застарілі/авто-типи українською → канонічні (для префіллу форми редагування)
const KIND_MAP = { "персонаж": "character", "локація": "location", "предмет": "item", "інше": "other" };

function kindLabel(s) {
  return KIND_I18N[s] ? t(KIND_I18N[s]) : s;
}

function renderBible(cards) {
  lastCards = cards;
  $("cards").innerHTML =
    cards
      .map(
        (c) => {
          const manual = c.source === "manual";
          return `<div class="card ${manual ? "manual" : ""}">
      <span class="name">${esc(c.canonical)}</span><span class="type">${esc(kindLabel(c.entity_type))}</span><span class="src-badge ${manual ? "src-manual" : "src-auto"}">${esc(t(manual ? "src_manual" : "src_auto"))}</span>
      <div class="meta">${c.mention_total} ${plural(c.mention_total, I18N[lang].mentions)}${c.aliases.length ? " · " + esc(t("also_known")) + ": " + esc(c.aliases.join(", ")) : ""}</div>
      ${c.docs.length ? `<div class="meta">${esc(t("found_in"))}: ${c.docs.map((d) => `${esc(d.path)}×${d.mentions}`).join(", ")}</div>` : ""}
      ${
        c.assertions.length
          ? `<div class="meta">${esc(t("facts"))}</div><ul>` +
            c.assertions.map((a) => `<li>${esc(a.attribute)}: ${esc(a.value)} <span class="al">(${esc(a.doc)})</span></li>`).join("") +
            "</ul>"
          : ""
      }
      ${manual ? `<div class="card-actions"><button class="compact" data-edit="${c.entity_id}">✎ ${esc(t("btn_edit_card"))}</button></div>` : ""}
    </div>`;
        }
      )
      .join("") ||
    `<div class="empty"><span class="ico">👥</span>${esc(t("no_entities"))}</div>`;
  $("cards").querySelectorAll("button[data-edit]").forEach((b) => {
    b.onclick = () => {
      const card = (lastCards || []).find((c) => c.entity_id === Number(b.dataset.edit));
      if (card) openDossierForm(card);
    };
  });
}

async function loadBible() {
  const cards = await cmd("run_bible");
  $("cards").dataset.loaded = "1";
  renderBible(cards);
}

/* --- досьє автора: форма створення/редагування --- */
let dosEditing = null; // canonical імені картки при редагуванні, null при створенні

function openDossierForm(card) {
  if (!root) { showSetup(false); return; }
  dosEditing = card ? card.canonical : null;
  $("dosTitle").textContent = card ? t("dos_edit") : t("dos_new");
  $("dosName").value = card ? card.canonical : "";
  $("dosAliases").value = card ? card.aliases.join(", ") : "";
  const sel = $("dosKind");
  const kind = card ? KIND_MAP[card.entity_type] || card.entity_type : "character";
  if ([...sel.options].some((o) => o.value === kind)) sel.value = kind;
  else if (KIND_I18N[kind]) sel.value = kind;
  else sel.value = "other";
  $("dosNotes").value = "";
  $("dosMsg").textContent = "";
  $("dosMsg").className = "note";
  $("dossierForm").classList.remove("hidden");
  $("dosName").focus();
}

$("btnAddDossier").onclick = () => openDossierForm(null);

$("dosSave").onclick = async () => {
  if (!root) { showSetup(false); return; }
  const name = $("dosName").value.trim();
  if (!name) { $("dosMsg").textContent = t("dos_need_name"); $("dosMsg").className = "note err"; return; }
  const aliases = $("dosAliases").value.split(",").map((s) => s.trim()).filter(Boolean);
  const kind = $("dosKind").value;
  const notes = $("dosNotes").value;
  $("dosSave").disabled = true;
  try {
    await cmd("add_dossier", { name, aliases, kind, notes });
    $("dossierForm").classList.add("hidden");
    dosEditing = null;
    await loadBible(); // список карток після reindex+sync_bible
  } catch { /* помилку вже показано в cmd() */ }
  finally { $("dosSave").disabled = false; }
};

$("dosCancel").onclick = () => {
  $("dossierForm").classList.add("hidden");
  dosEditing = null;
};

$("btnIndex").onclick = async () => {
  await cmd("run_index");
  await loadBible();
};

/* ============================================================
   ЗНАЙДЕНІ ПРОБЛЕМИ (алерти як людські картки)
   ============================================================ */

// Emergency-фолбек: якщо get_alerts_meta недоступна (старий бінарі).
// Головне джерело текстів — бекенд; тут лише uk-мінімум.
const FALLBACK_META = [
  ["name-drift", "problem", "Одне ім'я пишеться по-різному", "Одне і те саме ім'я зустрічається у кількох написаннях.", "Виберіть одне написання і виправте інші."],
  ["assertion-conflict", "problem", "Суперечливі факти", "Одну властивість записано по-різному в різних місцях.", "Вирішіть, яка версія правильна, і виправте іншу."],
  ["bark-duplicate", "tip", "Репліка повторюється", "Та сама репліка зустрічається кілька разів.", "Залиште один варіант або додайте умови."],
  ["crosspost-stale", "problem", "Главу змінено після публікації", "На платформі досі стара версія глави.", "Оновіть текст на платформі."],
  ["timeline-backwards", "problem", "Час пішов назад", "Події наступної глави відбуваються раніше попередньої.", "Перевірте послідовність або позначте флешбек."],
  ["location-conflict", "problem", "Персонаж у двох місцях одночасно", "Дві локації в один день без позначки переміщення.", "Додайте [loc: ...] або виправте локацію."],
  ["sync-conflict", "problem", "Конфлікт синхронізації", "Два пристрої змінили те саме місце по-різному.", "Виберіть варіант і видаліть маркери <<<<<<<."],
  ["fact-unverified", "tip", "Факт не підтверджено", "Твердження не має позначки «перевірено».", "Перевірте за джерелом і позначте."],
  ["stat-change", "tip", "Статистику змінено заднім числом", "Правка в ранній главі змінила статистику.", "Гляньте, чи пізніші глави узгоджуються."],
  ["stat-error", "problem", "Статистика стала від'ємною", "У персонажа hp/ресурс нижче нуля.", "Виправте тег [stat: ...]."],
  ["episode-check", "tip", "Зауваження до структури епізоду", "Відхилення від схеми 20-хвилинного епізоду.", "Прочитайте зауваження."],
  ["ink-external-change", "tip", "Файл діалогів змінено ззовні", "Файл .ink/.yarn змінився після імпорту.", "Імпортуйте файл знову."],
  ["style-tautology", "tip", "Тавтологія: слово підряд", "Одне й те саме слово стоїть підряд або через одне слово в межах речення.", "Приберіть один із повторів або перепишіть речення."],
  ["style-repeat-word", "tip", "Настирливий повтор у реченні", "Одне й те саме значуще слово з'являється три і більше разів в одному реченні.", "Замініть частину повторів синонімами або розбийте речення."],
  ["style-double-space", "tip", "Подвійні пробіли", "У тексті трапляються подвійні пробіли — у верстці вони виглядають як розрив.", "Знайдіть подвійний пробіл за offset у payload і приберіть зайвий пробіл."],
  ["style-dash-orphan", "tip", "Дефіс замість тире", "У діалозі стоїть дефіс із пробілами там, де типографіка вимагає тире.", "Замініть на тире «—», якщо це не свідомий стилістичний вибір."],
].map(([kind, severity, title, explain, action]) => ({ kind, severity, title_uk: title, explain_uk: explain, action_uk: action }));

let alertMeta = {};   // kind -> {severity, title, explain, action}
let lastAlerts = null; // масив {kind, payload, id} (drift-групи — віртуальні записи)
let alertSummaryBase = ""; // статична частина підсумку («що перевірено») — QA-07

// переклад ключів payload людською мовою (QA-09)
const PK = {
  uk: { canonical: "правильне написання", variants: "варіанти написання", doc: "файл", doc_path: "файл", chapter: "глава", attribute: "властивість", value: "значення", old_value: "було", new_value: "стало", old: "було", new: "стало", entity: "персонаж", platform: "платформа", version_hash: "опублікована версія", new_hash: "нова версія", day: "день", location: "локація", locations: "локації", sentence: "речення", line: "рядок", attr: "характеристика", device: "пристрій", src: "файл", claim: "твердження", source: "джерело", entity_name: "персонаж", affected_doc: "файл", docs: "глави", marker: "мітка часу", from: "була доба", to: "стала доба", values: "значення", duplicate: "дубль", trigger: "тригер", changes: "зміни", entries: "записи" },
  en: { canonical: "correct spelling", variants: "spellings found", doc: "file", doc_path: "file", chapter: "chapter", attribute: "attribute", value: "value", old_value: "was", new_value: "now", old: "was", new: "now", entity: "character", platform: "platform", version_hash: "published version", new_hash: "new version", day: "day", location: "location", locations: "locations", sentence: "sentence", line: "line", attr: "attribute", device: "device", src: "file", claim: "claim", source: "source", entity_name: "character", affected_doc: "file", docs: "chapters", marker: "time marker", from: "was day", to: "became day", values: "values", duplicate: "duplicate", trigger: "trigger", changes: "changes", entries: "entries" },
  ru: { canonical: "правильное написание", variants: "варианты написания", doc: "файл", doc_path: "файл", chapter: "глава", attribute: "свойство", value: "значение", old_value: "было", new_value: "стало", old: "было", new: "стало", entity: "персонаж", platform: "платформа", version_hash: "опубликованная версия", new_hash: "новая версия", day: "день", location: "локация", locations: "локации", sentence: "предложение", line: "строка", attr: "свойство", device: "устройство", src: "файл", claim: "утверждение", source: "источник", entity_name: "персонаж", affected_doc: "файл", docs: "главы", marker: "отметка времени", from: "был день", to: "стал день", values: "значения", duplicate: "дубль", trigger: "триггер", changes: "изменения", entries: "записи" },
};
// службові ключі payload — НЕ показуємо (QA-09): message — це core-текст
// валідатора (окрема робота з перекладу), offset-и/id живлять кнопку «Перейти»
const PK_HIDDEN = new Set(["kind", "message", "char_start", "ord", "bark_id"]);

function pkLabel(k) { return (PK[lang] && PK[lang][k]) || k; }

function fmtVal(k, v) {
  if (v == null) return "—";
  // QA-09: складені payload-структури — людською, а не JSON-стенограмою
  if (k === "changes" && Array.isArray(v))
    return v.map((c) => `${fmtVal("chapter", c.chapter)}: ${fmtVal("old", c.old)} → ${fmtVal("new", c.new)}`).join(" | ");
  if (k === "entries" && Array.isArray(v))
    return v.map((e) => `${fmtVal("chapter", e.chapter)}: ${fmtVal("value", e.value)}`).join(" | ");
  if (Array.isArray(v)) return v.map((x) => fmtVal("", x)).join(" | ");
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}

// --- QA-05: дієві алерти — глава + цитата + кнопка «Перейти» ---

// глава й offset алерту (offset-и з ядра — байтові, у тілі без frontmatter)
function alertDoc(a) {
  const p = a.payload || {};
  return p.doc || p.doc_path || p.affected_doc || null;
}
function alertOffset(a) {
  const n = Number((a.payload || {}).char_start);
  return Number.isFinite(n) ? n : null;
}
function quoteOf(a) {
  const p = a.payload || {};
  let q = String(p.sentence || p.marker || p.word || p.canonical || "").trim();
  if (q.length > 60) q = q.slice(0, 57) + "…";
  return q;
}

// UTF-8 байтовий offset → індекс у JS-рядку (бінарний пошук по префіксу)
function byteIndexToCharIndex(str, byteOffset) {
  const enc = new TextEncoder();
  let lo = 0, hi = str.length;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (enc.encode(str.slice(0, mid)).length <= byteOffset) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

// Підсвітити рядок textarea (0-based) на 2 секунди зі скролом до нього.
// Геометрія (FQA-11, уточнення Codex): overlay живе в editorPane
// (position:relative), але textarea стоїть ПІД editorHead — тому top
// смуги = ta.offsetTop (позиція textarea всередині pane) + зміщення
// рядка всередині textarea з урахуванням scroll і паддінгів.
// lineNo*lineHeight ігнорує soft-wrap — для обгортаних рядків рахуємо
// висоту префікса тексту до цільового рядка через measure-спані.
function flashLineNumber(lineNo) {
  const ta = $("editor");
  const text = ta.value;
  if (!text || lineNo < 0) return;
  const pane = $("editorPane");
  const cs = getComputedStyle(ta);
  const lineHeight = parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.7 || 27;
  const padTop = parseFloat(cs.paddingTop) || 0;
  // 1) висота тексту ДО цільового рядка з урахуванням soft-wrap:
  //    div-дзеркало без паддінгів/бордерів, ширина = контент textarea.
  //    FQA-11 (нативний QA): БЕЗ хвостового ZWJ — префікс і так закінчується
  //    "\n", а ZWJ малював ЗАЙВИЙ рядок після нього → смуга на 1 рядок нижче
  //    (старий CDP-тест не ловив це, бо будував дзеркало так само).
  const meas = document.createElement("div");
  const mcs = meas.style;
  mcs.position = "absolute"; mcs.visibility = "hidden"; mcs.top = "-9999px";
  mcs.whiteSpace = "pre-wrap"; mcs.wordWrap = "break-word";
  mcs.width = (ta.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight)) + "px";
  mcs.fontSize = cs.fontSize; mcs.fontFamily = cs.fontFamily;
  mcs.fontWeight = cs.fontWeight; mcs.fontStyle = cs.fontStyle;
  mcs.letterSpacing = cs.letterSpacing; mcs.lineHeight = cs.lineHeight;
  mcs.textTransform = cs.textTransform;
  const prefix = lineNo > 0 ? text.split("\n").slice(0, lineNo).join("\n") + "\n" : "";
  meas.textContent = prefix;
  document.body.appendChild(meas);
  const prefixH = meas.offsetHeight;
  document.body.removeChild(meas);
  // 2) top overlay = позиція textarea в pane + паддінг + висота префікса − скрол.
  //    ВАЖЛИВО (FQA-11): scrollTop спочатку виставляємо, і лише ПОТІМ читаємо
  //    його для top смуги — інакше смуга їде на величину зміни скролу.
  const taTopInPane = ta.getBoundingClientRect().top - pane.getBoundingClientRect().top;
  // скрол так, щоб рядок опинився приблизно посередині видимої області
  const lineTopInTa = padTop + prefixH;
  const target = Math.max(0, Math.min(lineTopInTa - ta.clientHeight / 2, ta.scrollHeight - ta.clientHeight));
  ta.scrollTop = target;
  const top = taTopInPane + padTop + prefixH - ta.scrollTop;
  const hl = document.createElement("div");
  hl.className = "line-flash";
  hl.style.top = Math.max(top, taTopInPane) + "px";
  hl.style.height = Math.ceil(lineHeight) + "px";
  hl.style.left = (ta.getBoundingClientRect().left - pane.getBoundingClientRect().left) + "px";
  hl.style.right = (pane.getBoundingClientRect().right - ta.getBoundingClientRect().right) + "px";
  $("editorPane").appendChild(hl);
  setTimeout(() => hl.remove(), 2000);
}

// FQA-11: якщо перейти не вдалося (цитата не знайдена, offset некоректний
// або редагатор не завантажився) — чесний fallback: під цитатою в картці
// алерту з'являється рядок «Сторінка: глава N», БЕЗ жовтої смуги.
function gotoAlertFallback(btn, doc) {
  if (!btn) return;
  const agoto = btn.closest(".agoto");
  if (!agoto || agoto.querySelector(".loc-fallback")) return;
  const base = String(doc || "").split(/[\\/]/).pop() || "";
  const m = base.match(/(\d+)/);
  const n = m ? String(parseInt(m[1], 10)) : base.replace(/\.md$/, "");
  if (!n) return;
  const span = document.createElement("span");
  span.className = "where loc-fallback";
  span.textContent = t("fqa11_loc", { n });
  agoto.appendChild(span);
}

async function gotoAlert(doc, charStart, btn) {
  if (!doc) return;
  try {
    const a = gotoAlert.alert; // цитата для надійного пошуку рядка
    const loaded = await openDoc(doc);
    // FQA-11: редагатор має бути ПОВНІСТЮ завантажений ДО пошуку цитати —
    // openDoc міг бути скасований confirm-ом (тоді editor містить ІНШУ главу)
    const body = loaded && typeof loaded.body === "string" ? loaded.body : "";
    const ready = !!loaded && editor.value === body && body.trim().length > 0;
    let line = -1;
    const q = ready && a
      ? String(a.payload?.sentence || a.payload?.marker || a.payload?.word || a.payload?.canonical || "").trim()
      : "";
    if (ready && q.length >= 4) {
      const idx = editor.value.indexOf(q);
      if (idx >= 0) {
        const lineStart = editor.value.lastIndexOf("\n", Math.max(idx - 1, 0)) + 1;
        line = (editor.value.slice(0, lineStart).match(/\n/g) || []).length;
      }
    }
    if (ready && line < 0 && charStart != null) {
      // FQA-11: offset — байтовий; довіряємо ЛИШЕ коректному діапазону
      // (нульовий/від'ємний = підозріло малий, більший за тіло = некоректний)
      const byteLen = new TextEncoder().encode(body).length;
      if (charStart > 0 && charStart <= byteLen) {
        const ci = Math.min(byteIndexToCharIndex(editor.value, charStart), Math.max(editor.value.length - 1, 0));
        const lineStart = editor.value.lastIndexOf("\n", Math.max(ci - 1, 0)) + 1;
        line = (editor.value.slice(0, lineStart).match(/\n/g) || []).length;
      }
    }
    if (line >= 0) flashLineNumber(line);
    else gotoAlertFallback(btn, doc); // чесний fallback без жовтої смуги
  } catch { /* помилку вже показано в cmd() */ }
}

// --- ігнорування карток: localStorage за (root, kind, doc, key) ---
function alertKey(a) {
  const p = a.payload || {};
  return [a.kind, p.doc || p.doc_path || p.affected_doc || "", p.canonical || p.entity || p.entity_name || p.trigger || p.attribute || ""].join("|");
}
function dismissedMap() {
  try { return JSON.parse(localStorage.getItem("skri.dismissed") || "{}"); } catch { return {}; }
}
function isDismissed(a) { return !!dismissedMap()[`${root}|${alertKey(a)}`]; }
function dismissAlerts(arr) {
  const m = dismissedMap();
  for (const a of arr) m[`${root}|${alertKey(a)}`] = 1;
  localStorage.setItem("skri.dismissed", JSON.stringify(m));
}

// «Прийняти варіант» у name-drift: записати форму як псевдонім досьє
// (add_dossier робить union псевдонімів, нотатки наявного досьє зберігаються).
async function acceptVariant(canonical, variant) {
  if (!root) { showSetup(false); return; }
  try {
    let aliases = [variant];
    let kind = "other";
    try {
      const cards = await cmd("run_bible");
      const c = (cards || []).find((x) => (x.canonical || "").toLowerCase() === String(canonical).toLowerCase());
      if (c) {
        aliases = [...new Set([...(c.aliases || []), variant])];
        kind = KIND_MAP[c.entity_type] || c.entity_type || "other";
      }
    } catch { /* картки ще не завантажені — записуємо мінімум */ }
    await cmd("add_dossier", { name: canonical, aliases, kind, notes: "" });
    try { await loadBible(); } catch { /* оновимо наступного разу */ }
    // drift-групу, для якої прийняли варіант, прибираємо з екрана
    lastAlerts = (lastAlerts || []).filter(
      (a) => !(a.kind === "name-drift" && (a.payload?.canonical || "").toLowerCase() === String(canonical).toLowerCase())
    );
    renderAlerts();
  } catch { /* помилку вже показано в cmd() */ }
}

// FQA-03: людський рядок для fact-алертів замість сирих ключів
// doc_a/doc_b/kind_label/value_a/value_b. kind_label з ядра — укр.,
// тому вид факту перекладаємо власним i18n (age/eye_color → t()).
function factKind(p) {
  if (p.kind === "age") return t("fqa03_age");
  if (p.kind === "eye_color") return t("fqa03_eye");
  return p.kind_label || p.kind || "";
}
function factHumanHtml(a) {
  const p = a.payload || {};
  const e = String(p.entity || p.entity_name || "");
  const kind = factKind(p);
  const va = String(p.value_a ?? ""), vb = String(p.value_b ?? "");
  const da = String(p.doc_a || p.doc || ""), db = String(p.doc_b || "");
  // зрізати шлях до імені глави («manuscript/02.md» → «02.md»)
  const base = (s) => s.split(/[\\/]/).pop();
  const key = a.kind === "fact-conflict" ? "fqa03_conflict" : "fqa03_vs";
  return `<div class="v fline">${esc(t(key, {
    e, kind, a: va, b: vb, da: base(da), db: base(db),
  }))}</div>`;
}

function instanceHtml(a) {
  const p = a.payload && typeof a.payload === "object" && !Array.isArray(a.payload) ? a.payload : null;
  let rows;
  if (a.kind === "fact-vs-dossier" || a.kind === "fact-conflict") {
    // FQA-03: сирі ключі не показуємо взагалі — лише людське порівняння
    rows = p ? factHumanHtml(a) : "—";
  } else if (p) {
    // QA-09: службові ключі (message з ядра, технічні offset-и) не показуємо
    rows = Object.entries(p)
      .filter(([k]) => !PK_HIDDEN.has(k))
      .map(([k, v]) => `<div class="kv"><span class="k">${esc(pkLabel(k))}:</span><span class="v">${esc(fmtVal(k, v))}</span></div>`)
      .join("");
  } else {
    rows = `<div class="v">${esc(fmtVal("", a.payload))}</div>`;
  }
  // Кнопка «Перейти» — лише якщо у payload є і глава, і offset (QA-05);
  // індекс у gotoRegistry дозволяє дістати ЦІЛИЙ алерт (цитату) в onclick
  const doc = alertDoc(a);
  const off = alertOffset(a);
  let gotoHtml = "";
  if (doc && off != null) {
    const q = quoteOf(a);
    gotoRegistry.push(a);
    const gi = gotoRegistry.length - 1;
    gotoHtml =
      `<div class="agoto">` +
      (q ? `<span class="q">«${esc(q)}»</span>` : "") +
      `<span class="where">${esc(doc)}</span>` +
      `<button class="compact" data-goto-doc="${esc(doc)}" data-goto-off="${off}" data-gi="${gi}">${esc(t("goto"))} ↦</button>` +
      `</div>`;
  }
  return `<div class="ainstance">${rows || "—"}${gotoHtml}</div>`;
}

function metaFor(kind) {
  if (kind === "__missing_time")
    return { severity: "tip", title: t("missing_time_title"), explain: t("missing_time_explain"), action: t("missing_time_action") };
  // FQA-04: чесна порада «перевірено збережену версію» — окрема tip-картка
  if (kind === "__stale_draft")
    return { severity: "tip", title: t("fqa04_stale_draft"), explain: "", action: "" };
  const m = alertMeta[kind];
  if (m) return m;
  const f = FALLBACK_META.find((x) => x.kind === kind);
  return f
    ? { severity: f.severity, title: f.title_uk, explain: f.explain_uk, action: f.action_uk }
    : { severity: "problem", title: kind, explain: "", action: "" };
}

function renderAlerts() {
  if (!lastAlerts) return;
  gotoRegistry = []; // реєстр кнопок «Перейти» перебудовується щоразу
  // FQA-12 (повторно відкрито): ПАДІННЯ фільтра ДО підрахунків. Якщо вибраний
  // вид зник із видимих (усі його картки проігноровано) — повертаємо фільтр
  // на «Всі» ПЕРЕД обчисленням shown/бейджа/підсумку, інакше shown=[] дає
  // брехливе зелене «все добре» при живих знахідках інших видів.
  const visible = lastAlerts.filter((a) => !isDismissed(a));
  const visByKind = new Map();
  for (const a of visible) {
    if (!visByKind.has(a.kind)) visByKind.set(a.kind, []);
    visByKind.get(a.kind).push(a);
  }
  let f = $("alertFilter").value;
  if (f && !visByKind.has(f)) {
    f = "";
    $("alertFilter").value = "";
  }
  const shown = f ? visible.filter((a) => a.kind === f) : visible;
  // групування по kind; спершу problem, потім tip
  const byKind = new Map();
  for (const a of shown) {
    if (!byKind.has(a.kind)) byKind.set(a.kind, []);
    byKind.get(a.kind).push(a);
  }
  const sevRank = (k) => (metaFor(k).severity === "problem" ? 0 : 1);
  const kinds = [...byKind.keys()].sort((x, y) => sevRank(x) - sevRank(y) || byKind.get(y).length - byKind.get(x).length);
  // підсумок: статична частина (що перевірено) + чесний «знайдено» по фільтру
  const problems = shown.filter((a) => sevRank(a.kind) === 0).length;
  const tips = shown.length - problems;
  // FQA-05: підсумок зберігається СТРУКТУРОВАНО ({key, params} або {raw}) —
  // зміна мови перемальовує його з t() без повторного прогону перевірки
  let sum = buildSummary(alertSummaryBase);
  if (shown.length) {
    sum = (sum ? sum + " " : "") + t("sum_found", { p: problems, pw: plural(problems, I18N[lang].problem_w), tp: tips, tw: plural(tips, I18N[lang].tip_w) });
  }
  $("alertsSummary").textContent = sum;
  $("alertList").innerHTML =
    kinds
      .map((k) => {
        const m = metaFor(k);
        const arr = byKind.get(k);
        const tip = m.severity !== "problem";
        // QA-05: у name-drift — «Прийняти варіант» для кожної форми;
        // на кожній картці — «Ігнорувати» (ховає до перезапуску перевірки)
        const acceptBtns =
          k === "name-drift"
            ? arr
                .map((a) =>
                  (a.payload?.variants || [])
                    .map(
                      (v) =>
                        `<button class="compact" data-accept="${esc(a.payload?.canonical || "")}" data-variant="${esc(v)}">✓ ${esc(t("accept_variant"))} «${esc(v)}»</button>`
                    )
                    .join("")
                )
                .join("")
            : "";
        const ignoreBtn = `<button class="compact ghost" data-ignore-kind="${esc(k)}">✕ ${esc(t("ignore"))}</button>`;
        return `<div class="acard ${tip ? "tip" : ""}">
        <div class="ahead"><span class="atitle">${esc(m.title)}</span><span class="acount">${arr.length}</span></div>
        ${m.explain ? `<div class="aexp">${esc(t("what_means"))} ${esc(m.explain)}</div>` : ""}
        ${m.action ? `<div class="ado"><b>${esc(t("what_do"))}</b> ${esc(m.action)}</div>` : ""}
        <div class="acard-actions">${acceptBtns}${ignoreBtn}</div>
        <details><summary>${esc(t("show_n"))} (${arr.length})</summary>${arr.map(instanceHtml).join("")}</details>
      </div>`;
      })
      .join("") ||
    `<div class="empty"><span class="ico">✅</span>${esc(t("no_alerts"))}</div>`;
  // бейдж на вкладці = кількість видимих у поточному фільтрі
  const badge = $("alertBadge");
  badge.hidden = shown.length === 0;
  badge.textContent = shown.length;
  // FQA-12: опції фільтра теж з масиву visible (мінус ігноровані!) —
  // список видів НЕ залежить від активного фільтра, щоб дропдаун не «згортався»
  // (visByKind обчислено на початку renderAlerts — разом із падінням фільтра)
  const allKinds = [...visByKind.keys()].sort(
    (x, y) => sevRank(x) - sevRank(y) || visByKind.get(y).length - visByKind.get(x).length
  );
  $("alertFilter").innerHTML =
    `<option value="">${esc(t("filter_all"))} (${visible.length})</option>` +
    allKinds.map((k) => `<option value="${esc(k)}" ${k === f ? "selected" : ""}>${esc(metaFor(k).title)} (${visByKind.get(k).length})</option>`).join("");
  // кнопки «Перейти» (глава + offset), «Прийняти варіант», «Ігнорувати»
  $("alertList").querySelectorAll("button[data-goto-doc]").forEach((b) => {
    b.onclick = () => {
      gotoAlert.alert = gotoRegistry[Number(b.dataset.gi)] || null;
      gotoAlert(b.dataset.gotoDoc, Number(b.dataset.gotoOff), b); // FQA-11: btn — для чесного fallback
    };
  });
  $("alertList").querySelectorAll("button[data-accept]").forEach((b) => {
    b.onclick = () => acceptVariant(b.dataset.accept, b.dataset.variant);
  });
  $("alertList").querySelectorAll("button[data-ignore-kind]").forEach((b) => {
    b.onclick = () => {
      dismissAlerts((lastAlerts || []).filter((a) => a.kind === b.dataset.ignoreKind));
      renderAlerts();
    };
  });
}

// FQA-05: підсумок перевірки у структурній формі — переклад формується при
// кожному рендері; {raw} — для сторонніх/тестових рядків без перекладу
function buildSummary(s) {
  if (!s) return "";
  if (s.raw != null) return String(s.raw);
  if (s.key === "sum_drift_only") {
    return t("sum_drift_only", { g: s.g, gw: plural(s.g, I18N[lang].drift_gw) });
  }
  if (s.key === "sum_checks") {
    return t("sum_checks", { d: s.d, dw: plural(s.d, I18N[lang].chapters_w), w: s.w, ww: plural(s.w, I18N[lang].words) }) +
      " " + t("sum_entities", { m: s.m, a: s.a });
  }
  return "";
}

function fillAlerts(alerts, base) {
  lastAlerts = alerts;
  alertSummaryBase = typeof base === "string" ? { raw: base } : (base || "");
  renderAlerts();
}

async function loadMeta() {
  const r = await cmdq("get_alerts_meta");
  if (r.err != null) return; // фолбек уже на місці
  alertMeta = {};
  for (const m of r.ok) {
    alertMeta[m.kind] = {
      severity: m.severity,
      title: m["title_" + lang] ?? m.title_uk,
      explain: m["explain_" + lang] ?? m.explain_uk,
      action: m["action_" + lang] ?? m.action_uk,
    };
  }
}

async function loadAlerts() {
  await loadMeta();
  const r = await cmd("run_drift");
  // drift-групи → віртуальні алерти kind=name-drift, щоб мати ті самі картки
  const driftAlerts = (r.groups || []).map((g, i) => ({
    id: "d" + i,
    kind: "name-drift",
    payload: { canonical: g.canonical, variants: g.variants },
  }));
  fillAlerts([...driftAlerts, ...(r.alerts || [])], { key: "sum_drift_only", g: r.groups.length });
}
$("alertFilter").onchange = renderAlerts;

// --- кнопка «Перевірити»: повний конвеєр валідаторів ---
$("btnCheck").onclick = async () => {
  if (!root) { showSetup(false); return; }
  const gen = projGen; // QA-10: якщо проєкт змінили під час перевірки — результат ігноруємо
  $("btnCheck").disabled = true;
  $("btnCheck").textContent = "⏳ " + t("checking");
  // FQA-04 (чесна перевірка): при незбережених правках пропонуємо зберегти.
  // OK → saveDoc перед прогоном; Cancel → продовжуємо, але prepend'ом додаємо
  // чесну пораду-картку (kind='__stale_draft'), що перевірено збережену версію.
  let staleDraft = false;
  if (currentDoc && dirty) {
    if (confirm(t("fqa04_save_first"))) await saveDoc();
    else staleDraft = true;
  }
  try {
    await loadMeta();
    const r = await cmd("run_checks");
    // FQA-01: style-перевірки виконує САМ run_checks (інтервально, до
    // SELECT-а алертів) — style-алерти приходять у r.alerts; окремий виклик
    // run_style з фронтової кнопки прибрано (він і давав дублікати карток).
    const alerts = r.alerts || [];
    // QA-07: чесні підсумки — «глав» = лише manuscript; сутності окремо:
    // досьє автора vs знайдено в тексті. problem/tip рахує renderAlerts
    // ОДНАКОВО з фільтром (бейдж = фільтр = підсумок).
    let sum = { key: "sum_checks", d: r.manuscript_docs ?? r.docs, w: r.words, m: r.entities_manual ?? 0, a: r.entities_auto ?? 0 };
    // глави без time-маркерів — порада (не алерт у БД)
    if (r.timeline_missing.length) {
      alerts.unshift({
        id: "tm",
        kind: "__missing_time",
        payload: { docs: r.timeline_missing.map(([p, n]) => `${p} — #${n}`) },
      });
    }
    // FQA-04: чесна порада — найпершою карткою
    if (staleDraft) {
      alerts.unshift({ id: "stale", kind: "__stale_draft", payload: {} });
    }
    if (gen !== projGen) return; // проєкт змінили — stale результат не малюємо
    fillAlerts(alerts, sum);
    await loadBible(); // картки оновилися після конвеєра
    document.querySelector('.stab[data-stab="alerts"]').click();
  } finally {
    $("btnCheck").disabled = false;
    $("btnCheck").textContent = t("check");
  }
};


/* ============================================================
   ІНСТРУМЕНТИ ДІАЛОГІВ (ігри/сценарії)
   FQA-05: останні результати зберігаємо, щоб зміна мови
   перемальовувала таблиці з t() без повторного прогону.
   ============================================================ */
let lastVars = null;
let lastBarks = null;
let lastEpisode = null; // { rel, r }

function renderVars(rows) {
  lastVars = rows;
  $("varsOut").innerHTML =
    `<div class="tablewrap"><table class="t"><tr><th>${esc(t("th_var"))}</th><th>${esc(t("th_set"))}</th><th>${esc(t("th_check"))}</th><th>${esc(t("th_unset"))}</th><th>${esc(t("th_status"))}</th></tr>` +
    rows
      .map(
        (v) =>
          `<tr><td>${esc(v.var_name)}</td><td>${v.sets.map(esc).join(", ") || "—"}</td>` +
          `<td>${v.checks.map(esc).join(", ") || "—"}</td><td>${v.unsets.map(esc).join(", ") || "—"}</td>` +
          `<td>${v.orphan_check ? `<span class="warn">${esc(t("v_orphan"))}</span>` : v.dead_flag ? `<span class="warn">${esc(t("v_dead"))}</span>` : esc(t("v_ok"))}</td></tr>`
      )
      .join("") +
    "</table></div>";
}

function renderBarks(rows) {
  lastBarks = rows;
  $("barksOut").innerHTML =
    `<div class="tablewrap"><table class="t"><tr><th>${esc(t("th_trigger"))}</th><th>${esc(t("th_priority"))}</th><th>${esc(t("th_cooldown"))}</th><th>${esc(t("th_conditions"))}</th><th>${esc(t("th_variants"))}</th></tr>` +
    rows
      .map(
        (b) =>
          `<tr><td>${esc(b.trigger)}</td><td>${b.priority}</td><td>${b.cooldown}${esc(t("sec"))}</td>` +
          `<td>${esc(b.conditions)}</td><td>${esc(b.variants.join(" / "))}</td></tr>`
      )
      .join("") +
    "</table></div>";
}

function renderEpisode(rel, r) {
  lastEpisode = { rel, r };
  const c = r.check;
  // FQA-05/FQA16: warnings і verdict'и локалізуємо за машинними кодами
  // (epi_*), uk і далі бере людський текст з ядра. Немає коду/ключа — fallback.
  const isUk = lang === "uk";
  const issueLine = (i) => {
    const key = "epi_" + (i.code || "");
    const txt = !isUk && I18N[lang][key] ? t(key, i.params || {}) : i.message;
    return `<div class="warn">• ${esc(txt)}</div>`;
  };
  const issues = c.issues_structured && c.issues_structured.length
    ? c.issues_structured.map(issueLine).join("")
    : c.issues.map((i) => `<div class="warn">• ${esc(i)}</div>`).join("");
  const verdictOf = (m) => {
    const key = "epi_mute_" + (m.code || "");
    return !isUk && I18N[lang][key] ? t(key, m.params || {}) : m.verdict;
  };
  const ruleHit = (h) => {
    const key = "epi_" + (h.code || "bible_rule_hit");
    const txt = !isUk && I18N[lang][key]
      ? t(key, { rule: h.rule, hits: (h.hits || []).join(", ") })
      : h.message;
    return `<div class="warn">${esc(t("ep_rule"))} ${esc(txt)}</div>`;
  };
  const ruleLines = r.rule_hits_structured && r.rule_hits_structured.length
    ? r.rule_hits_structured.map(ruleHit).join("")
    : (r.rule_hits.length ? r.rule_hits.map((i) => `<div class="warn">${esc(t("ep_rule"))} ${esc(i)}</div>`).join("") : "");
  $("epOut").innerHTML =
    `<div class="card"><b>${esc(t("btn_episode"))}: ${esc(rel)}</b>
      <div class="meta">${c.passed ? "✅ " + esc(t("ep_passed")) : "⚠ " + esc(t("ep_issues"))}</div>
      <div class="meta">${esc(t("ep_cold_open"))}: ${esc(c.cold_open ?? t("none"))} · ${esc(t("ep_turns"))}: ${c.turns} · ${esc(t("ep_hook"))}: ${esc(c.hook ?? t("none"))}</div>
      ${issues}
      ${ruleLines}
    </div>
    <div class="tablewrap"><table class="t"><tr><th>${esc(t("th_scene"))}</th><th>${esc(t("th_action"))}</th><th>${esc(t("th_dialogue"))}</th><th>${esc(t("th_ratio"))}</th><th>${esc(t("th_verdict"))}</th></tr>` +
    r.mute
      .map(
        (m) =>
          `<tr><td>${esc(m.scene)}</td><td>${m.action_lines}</td><td>${m.dialogue_lines}</td>` +
          `<td>${m.visual_ratio}</td><td>${esc(verdictOf(m))}</td></tr>`
      )
      .join("") +
    "</table></div>";
}

$("btnVars").onclick = async () => renderVars(await cmd("var_audit"));

$("btnBarks").onclick = async () => renderBarks(await cmd("barks"));

$("btnEpisode").onclick = async () => {
  const rel = $("epPath").value.trim();
  if (!rel) { alert(t("need_ep")); return; }
  renderEpisode(rel, await cmd("episode_validate", { rel }));
};

/* ============================================================
   ПУБЛІКАЦІЇ (kanban + прогноз)
   ============================================================ */
const STATUSES = ["draft", "edit", "buffer", "scheduled", "published"];
// FQA-06: календарна валідація YYYY-MM-DD (збігається з release::parse_ymd —
// реальні дні місяця з урахуванням високосного року, без зайвих форматів)
function isValidYmd(s) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s || "");
  if (!m) return false;
  const y = +m[1], mo = +m[2], d = +m[3];
  if (mo < 1 || mo > 12) return false;
  const leap = (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;
  const dim = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][mo - 1];
  return d >= 1 && d <= dim;
}
let lastBoard = null;

function renderRelease(b) {
  lastBoard = b;
  const fc = b.forecast;
  let fcHtml = `<b>${esc(t("fc_title"))}</b>`;
  if (fc.words_per_day > 0) {
    fcHtml += `<div class="fline">${esc(t("fc_speed", { n: fc.words_per_day.toFixed(1) }))}</div>`;
    fcHtml += `<div class="fline">${esc(t("fc_buffer", { c: fc.buffer_chapters, cw: plural(fc.buffer_chapters, I18N[lang].fc_buffer_w), w: fc.buffer_words, ww: plural(fc.buffer_words, I18N[lang].words) }))}</div>`;
    fcHtml += `<div class="fline">${esc(t("fc_days_ch", { n: fc.days_per_chapter.toFixed(1) }))}</div>`;
    if (fc.days_left != null) fcHtml += `<div class="fline">${esc(t("fc_left", { n: fc.days_left.toFixed(1) }))}</div>`;
    if (fc.exhaustion_date) fcHtml += `<div class="fline">${esc(t("fc_exhaust", { d: fc.exhaustion_date }))}</div>`;
  } else {
    fcHtml += `<div class="fline">${esc(t("fc_unknown"))}</div>`;
  }
  $("forecastBox").innerHTML = fcHtml;

  const anyChapters = b.columns.some((c) => c.chapters.length);
  $("kanban").innerHTML = anyChapters
    ? b.columns
        .map((col) => {
          return `<div class="kcol">
        <div class="khead">${esc(t("st_" + col.status))} <span class="kn">${col.chapters.length}</span></div>
        ${col.chapters
          .map(
            (c) => `<div class="kcard">
          <span class="t" title="${esc(c.doc_path)}">${esc(c.title)}</span>
          <span class="m">${c.word_count} ${plural(c.word_count, I18N[lang].words)}${c.pub_date ? " · " + esc(new Date(c.pub_date * 1000).toISOString().slice(0, 10)) : ""}</span>
          ${/* FQA-07: select УСІХ 5 статусів — перехід на будь-який,
                включно з поверненням published → draft */
            `<label class="kstatus-line"><select class="kstatus" data-doc="${esc(c.doc_path)}" data-status="${esc(c.status)}" aria-label="${esc(t("move_to", { s: t("st_" + c.status) }))}">${STATUSES.map(
              (s) => `<option value="${s}"${s === c.status ? " selected" : ""}>${esc(t("st_" + s))}</option>`
            ).join("")}</select></label>`}
        </div>`
          )
          .join("")}
      </div>`;
        })
        .join("")
    : `<div class="empty"><span class="ico">🗓️</span>${esc(t("no_release"))}</div>`;

  $("kanban").querySelectorAll("select.kstatus").forEach((sel) => {
    sel.onchange = async () => {
      const rel = sel.dataset.doc;
      const status = sel.value;
      let pubDate; // FQA-06: перехід у «Заплановано» питає дату (за замовчуванням сьогодні+7)
      if (status === "scheduled") {
        const def = new Date(Date.now() + 7 * 86400000).toISOString().slice(0, 10);
        const d = prompt(t("fqa06_date_prompt"), def);
        if (d === null) { sel.value = sel.dataset.status; return; } // скасовано — статус не змінюємо
        pubDate = d.trim();
        // FQA-06 (нативний QA): порожня/нереальна дата НЕ переводить главу в
        // «Заплановано» — чесна помилка, статус лишається попереднім
        if (!isValidYmd(pubDate)) {
          sel.value = sel.dataset.status;
          alert(t("fqa06_date_invalid"));
          return;
        }
      }
      try {
        await cmd("release_set_status", { rel, status, pub_date: pubDate || undefined });
        // FQA-08: якщо ця глава відкрита в редакторі — оновлюємо docMeta
        // (pub_status у шапці редактора мусить показати новий статус)
        if (currentDoc && sameRoot(currentDoc.path, rel)) {
          try {
            const d = await cmd("read_doc", { rel });
            currentDoc = d;
            renderDocMeta(d);
          } catch { /* помилку вже показано в cmd() */ }
        }
        await loadRelease();
      } catch {
        // помилку вже показано в cmd(); FQA-06: канбан перечитується —
        // селект повертається до РЕАЛЬНОГО статусу на диску
        try { await loadRelease(); } catch { /* ігноруємо */ }
      }
    };
  });
}

async function loadRelease() {
  const b = await cmd("release_board");
  $("kanban").dataset.loaded = "1";
  renderRelease(b);
}

/* ============================================================
   МОДЕЛІ: кожна задача = дія людською мовою
   ============================================================ */
const TASK_HUMAN = {
  "generate-uk": ["mt_generate_uk_t", "mt_generate_uk_d"],
  translate: ["mt_translate_t", "mt_translate_d"],
  "transcribe-uk": ["mt_transcribe_uk_t", "mt_transcribe_uk_d"],
  grammar: ["mt_grammar_t", "mt_grammar_d"],
  summarize: ["mt_summarize_t", "mt_summarize_d"],
  "semantic-search": ["mt_semantic_search_t", "mt_semantic_search_d"],
};

// FQA-05: людські назви/описи ОПЦІЙ каталогу моделей ×3 мови (дані з ядра —
// лише uk). Ключ = "<задача>/<опція>"; byok-cloud має спільну назву.
// Відсутній ключ = fallback на uk-текст з ядра (нові опції не губляться).
const OPT_HUMAN = {
  "generate-uk/lapa-v0.1.2": ["opt_gen_lapa_l", "opt_gen_lapa_n"],
  "generate-uk/llama-3.1-8b": ["opt_gen_llama_l", "opt_gen_llama_n"],
  "generate-uk/byok-cloud": ["opt_byok_l", "opt_byok_n_full"],
  "translate/nllb-200": ["opt_tr_nllb_l", "opt_tr_nllb_n"],
  "translate/lapa-v0.1.2": ["opt_tr_lapa_l", "opt_tr_lapa_n"],
  "translate/byok-cloud": ["opt_byok_l", "opt_byok_n_short"],
  "transcribe-uk/whisper-large-v3-turbo-uk": ["opt_tr_wturbo_l", "opt_tr_wturbo_n"],
  "transcribe-uk/whisper-large-v3": ["opt_tr_wv3_l", "opt_tr_wv3_n"],
  "grammar/languagetool-local": ["opt_gr_lt_l", "opt_gr_lt_n"],
  "grammar/goroh-dictionary": ["opt_gr_goroh_l", "opt_gr_goroh_n"],
  "summarize/lapa-v0.1.2": ["opt_sum_lapa_l", "opt_sum_lapa_n"],
  "summarize/llama-3.1-8b": ["opt_sum_llama_l", "opt_sum_llama_n"],
  "semantic-search/rules-plus-fts": ["opt_ss_rules_l", "opt_ss_rules_n"],
  "semantic-search/multilingual-e5": ["opt_ss_e5_l", "opt_ss_e5_n"],
};
function optHuman(taskId, o) {
  const pair = OPT_HUMAN[taskId + "/" + o.id] || (o.id === "byok-cloud" ? ["opt_byok_l", null] : null);
  return pair
    ? [pair[0] ? t(pair[0]) : o.label, pair[1] ? t(pair[1]) : o.note]
    : [o.label, o.note];
}

let catalog = [];
let currentTask = null;
// QA-08: статус Ollama (null = ще не перевіряли; true/false — результат)
let ollamaReady = null;

async function loadOllamaStatus() {
  const r = await cmdq("ollama_status");
  if (r.err == null && r.ok && typeof r.ok.installed === "boolean") ollamaReady = r.ok.installed;
}

async function loadCatalog() {
  const r = await cmdq("model_catalog");
  if (r.err != null) return;
  catalog = r.ok;
  $("tasks").innerHTML = "";
  catalog.forEach((task) => {
    const [tk, dk] = TASK_HUMAN[task.id] || [task.label, task.hint];
    const b = document.createElement("button");
    b.className = "task";
    // QA-08: чесний підпис — це екран налаштувань, виконання задач скоро
    b.innerHTML = `<span class="t">${esc(t(tk))}</span><span class="h">${esc(t(dk))}</span><span class="soon">${esc(t("models_settings_note"))}</span>`;
    b.onclick = () => selectTask(task);
    task._btn = b;
    $("tasks").appendChild(b);
  });
}

function selectTask(task, silent = false) {
  currentTask = task;
  catalog.forEach((x) => x._btn && x._btn.classList.remove("active"));
  if (task._btn) task._btn.classList.add("active");
  const [tk, dk] = TASK_HUMAN[task.id] || [task.label, task.hint];
  $("panel").innerHTML =
    `<h3>${esc(t(tk))}</h3><div class="hint">${esc(t(dk))}</div><div class="hint soon">${esc(t("models_settings_note"))}</div>` +
    task.options
      .map(
        (o, j) => {
          // FQA-05: назви/описи опцій локалізуємо на фронті (дані ядра — uk)
          const [ol, on] = optHuman(task.id, o);
          return `<label class="opt">
        <input type="radio" name="m" ${j === 0 ? "checked" : ""}>
        <span><span class="l">${esc(ol)}</span>
        <span class="badges">
          ${j === 0 ? `<span class="badge b-rec">${esc(t("badge_rec"))}</span>` : ""}
          <span class="badge ${o.local ? "b-local" : "b-cloud"}">${o.local ? esc(t("badge_local")) : esc(t("badge_cloud"))}</span>
          ${o.local && ollamaReady === true ? `<span class="badge b-ollama-ok">${esc(t("ollama_ready"))}</span>` : ""}
          ${o.local && ollamaReady === false ? `<span class="badge b-ollama-no">${esc(t("ollama_missing"))}</span>` : ""}
        </span>
        <div class="n">${esc(on)}</div></span>
      </label>`;
        }
      )
      .join("");
  $("panel").querySelectorAll("input").forEach((i) => (i.onchange = applyModel));
  if (!silent) applyModel();
}

async function applyModel() {
  const inputs = [...$("panel").querySelectorAll("input")];
  const idx = inputs.findIndex((i) => i.checked);
  if (idx < 0 || !currentTask) return;
  const o = currentTask.options[idx];
  $("modelOut").textContent = JSON.stringify(
    { models: { [currentTask.id]: { model: o.id, auto: idx === 0 } } },
    null, 2
  );
  if (root) {
    const r = await cmdq("set_model", { task: currentTask.id, model: o.id, auto: idx === 0 });
    if (r.err == null) {
      $("modelOut").textContent += "\n✓ " + t("proj_ok");
    }
  }
}

/* ============================================================
   ЗАПУСК
   ============================================================ */
(async () => {
  document.documentElement.dataset.theme = localStorage.getItem("skri.theme") || "dark";
  $("themeBtn").textContent = document.documentElement.dataset.theme === "light" ? "🌙" : "☀️";
  if (!invoke) {
    $("noInvoke").classList.remove("hidden");
    $("noInvoke").textContent = t("no_invoke");
  }
  await fillPresets();
  await loadCatalog();
  await loadMeta();          // тексти карток проблем з бекенду
  loadOllamaStatus().then(() => { if (currentTask) selectTask(currentTask, true); }); // статус Ollama — QA-08
  renderWizardCards();
  applyI18n();               // переклад усіх статичних рядків

  // перший запуск → майстер; інакше — одразу до проєкту
  if (!localStorage.getItem("skri.onboarded")) {
    showWizard();
  } else if (root) {
    const ok = await openProject(root);
    if (ok === false) showSetup(false); // "cancelled" → залишаємось як є
  } else {
    showSetup(false);
  }
})();
