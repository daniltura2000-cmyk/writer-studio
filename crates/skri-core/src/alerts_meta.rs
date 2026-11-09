//! Глобальна мапа видів алертів з людськими поясненнями (uk/en/ru).
//!
//! Лікує скаргу «ніхуя не зрозуміло що за алерти»: кожен kind у системі
//! має людську назву, пояснення простими словами (1-2 речення) і пораду,
//! що з цим робити. Джерело правди для UI (Tauri/CLI) і довідки.
//!
//! Рядки kind звірені з фактичними рядками в коді:
//! - алерти в БД: drift.rs (name-drift), bible.rs (assertion-conflict),
//!   narrative.rs (bark-duplicate), crosspost.rs (crosspost-stale),
//!   timeline.rs (timeline-backwards, location-conflict), sync.rs
//!   (sync-conflict), stats.rs (stat-change, stat-error), animation.rs
//!   (episode-check), inkbridge.rs (ink-external-change);
//! - попередження/звіти валідаторів: timeline.rs (timeline-missing —
//!   попередження у звіті, не алерт у БД), pitch.rs (fact-unverified —
//!   звіт `skri fact unverified`), narrative.rs (graph-unreachable,
//!   graph-dangling-goto, var-orphan-check, var-dead-flag), animation.rs
//!   (episode-structure, dialogue-streak, mute-test), notes.rs
//!   (broken-links, orphan-notes, stale-notes), пресет journalist-uk
//!   (pitch-followup, fact-unverified-check).

use serde::Serialize;

/// Людський опис одного виду алерту трьома мовами.
#[derive(Debug, Clone, Serialize)]
pub struct AlertKindMeta {
    /// Точний рядок kind, як він живе в БД/звітах/пресетах.
    pub kind: String,
    /// Серйозність: "problem" (треба виправити) або "tip" (порада).
    pub severity: &'static str,
    pub title_uk: String,
    pub title_en: String,
    pub title_ru: String,
    /// 1-2 речення простими словами: що це означає.
    pub explain_uk: String,
    pub explain_en: String,
    pub explain_ru: String,
    /// Що з цим робити.
    pub action_uk: String,
    pub action_en: String,
    pub action_ru: String,
}

/// Усі види алертів/попереджень системи з поясненнями.
pub fn catalog() -> Vec<AlertKindMeta> {
    raw()
        .into_iter()
        .map(
            |(kind, tu, te, tr, xu, xe, xr, au, ae, ar)| AlertKindMeta {
                kind: kind.to_string(),
                severity: severity_of(kind),
                title_uk: tu.to_string(),
                title_en: te.to_string(),
                title_ru: tr.to_string(),
                explain_uk: xu.to_string(),
                explain_en: xe.to_string(),
                explain_ru: xr.to_string(),
                action_uk: au.to_string(),
                action_en: ae.to_string(),
                action_ru: ar.to_string(),
            },
        )
        .collect()
}

/// Каталог як JSON (масив об'єктів) — джерело для UI/довідки.
pub fn catalog_json() -> serde_json::Value {
    serde_json::to_value(catalog()).unwrap_or(serde_json::Value::Array(vec![]))
}

/// Серйозність виду: "problem" = треба виправити, "tip" = порада, на яку
/// варто глянути (попередження/стеження, а не зламане).
fn severity_of(kind: &str) -> &'static str {
    match kind {
        "timeline-missing" | "mute-test" | "stale-notes" | "pitch-followup" => "tip",
        _ => "problem",
    }
}

type Meta = (
    &'static str, // kind
    &'static str, &'static str, &'static str, // titles uk/en/ru
    &'static str, &'static str, &'static str, // explain uk/en/ru
    &'static str, &'static str, &'static str, // action uk/en/ru
);

fn raw() -> Vec<Meta> {
    vec![
        (
            "name-drift",
            "Дрейф імен",
            "Name drift",
            "Дрейф имён",
            "Одне й те саме ім'я пишеться по-різному в різних місцях (наприклад, «Кальдрін» і «Калдрін»). Програма вважає, що це той самий персонаж, і просить обрати одне написання.",
            "The same name is spelled differently in different places (e.g. \"Kaldryn\" vs \"Kaldrin\"). The program treats them as one character and asks you to pick a single spelling.",
            "Одно и то же имя пишется по-разному в разных местах (например, «Кальдрин» и «Калдрин»). Программа считает это одним персонажем и просит выбрать одно написание.",
            "Перегляньте алерт, оберіть правильне написання, а решту варіантів додайте в alias-и або виправте в тексті.",
            "Open the alert, pick the correct spelling and either fix the rest in the text or add them as aliases.",
            "Откройте алерт, выберите правильное написание, а остальные варианты добавьте в alias-ы или исправьте в тексте.",
        ),
        (
            "assertion-conflict",
            "Конфлікт канону",
            "Canon conflict",
            "Конфликт канона",
            "Два місця в рукописі чи біблії стверджують різне про одну й ту саму річ — наприклад, «очі: зелені» в одному файлі і «очі: карі» в іншому.",
            "Two places in the manuscript or bible claim different things about the same fact — e.g. \"eyes: green\" in one file and \"eyes: brown\" in another.",
            "Два места в рукописи или библии утверждают разное об одном и том же — например, «глаза: зелёные» в одном файле и «глаза: карие» в другом.",
            "Вирішіть, яка версія правильна: виправте текст або картку в біблії — після наступної перевірки алерт зникне.",
            "Decide which version is correct: fix the text or the bible card — the alert will clear on the next check.",
            "Решите, какая версия правильная: исправьте текст или карточку в библии — после следующей проверки алерт исчезнет.",
        ),
        (
            "bark-duplicate",
            "Дублікат bark-репліки",
            "Duplicate bark line",
            "Дубликат bark-реплики",
            "У одного bark-тригера знайдено дві однакові репліки — у грі вони звучатимуть поспіль як явний повтор.",
            "One bark trigger contains two identical lines — in the game they would play back to back as an obvious repeat.",
            "У одного bark-триггера найдены две одинаковые реплики — в игре они прозвучат подряд как явный повтор.",
            "Приберіть повтор або замініть його новим варіантом репліки (норма — 10-15 різних варіантів на тригер).",
            "Remove the duplicate or replace it with a new variant (the norm is 10-15 distinct lines per trigger).",
            "Уберите повтор или замените его новым вариантом реплики (норма — 10-15 разных вариантов на триггер).",
        ),
        (
            "crosspost-stale",
            "Змінено після публікації",
            "Changed after publishing",
            "Изменено после публикации",
            "Главу відредагували після того, як її вже опубліковано на платформі — опублікована версія більше не збігається з текстом.",
            "The chapter was edited after it was already published on a platform — the published version no longer matches the text.",
            "Главу отредактировали после того, как она уже опубликована на платформе — опубликованная версия больше не совпадает с текстом.",
            "Перевідправте оновлену главу на платформу і позначте її опублікованою знову (skri crosspost mark ... posted) — це зафіксує нову версію.",
            "Re-publish the updated chapter and mark it posted again (skri crosspost mark ... posted) — that records the new version.",
            "Переопубликуйте обновлённую главу на платформе и отметьте её опубликованной снова (skri crosspost mark ... posted) — это зафиксирует новую версию.",
        ),
        (
            "timeline-backwards",
            "Стрибок назад у часі",
            "Timeline jump backwards",
            "Скачок назад во времени",
            "Події нової глави починаються раніше, ніж закінчилась попередня, і це не позначено як флешбек.",
            "The new chapter starts earlier than the previous one ended, and it is not marked as a flashback.",
            "События новой главы начинаются раньше, чем закончилась предыдущая, и это не помечено как флешбек.",
            "Або виправте часові маркери або день початку глави (day_start), або додайте маркер флешбеку, якщо це свідомий прийом.",
            "Either fix the time markers or the chapter's day_start, or add a flashback marker if that is intentional.",
            "Либо исправьте временные маркеры или день начала главы (day_start), либо добавьте маркер флешбека, если это сознательный приём.",
        ),
        (
            "timeline-missing",
            "Глава без часових маркерів",
            "Chapter without time markers",
            "Глава без временных маркеров",
            "У главі не знайдено жодного часового орієнтира, тому програма не може розмістити її на часовій лінії.",
            "The chapter has no time references, so the program cannot place it on the timeline.",
            "В главе не найдено ни одного временного ориентира, поэтому программа не может разместить её на временной линии.",
            "Додайте хоча б один маркер: [time: +3 днів], [time: день 12] або природну фразу на кшталт «наступного ранку».",
            "Add at least one marker: [time: +3 days], [time: day 12], or a natural phrase like \"next morning\".",
            "Добавьте хотя бы один маркер: [time: +3 дня], [time: день 12] или естественную фразу вроде «на следующее утро».",
        ),
        (
            "location-conflict",
            "Конфлікт локацій",
            "Location conflict",
            "Конфликт локаций",
            "Персонаж в одну й ту саму добу перебуває у двох різних місцях без позначеного переміщення між ними.",
            "A character is in two different places on the same day with no marked travel between them.",
            "Персонаж в один и тот же день находится в двух разных местах без отмеченного перемещения между ними.",
            "Або розведіть події в часі, або додайте маркер переміщення виду [loc: X = Y (шлях …)].",
            "Either space the events out in time or add a travel marker like [loc: X = Y (route …)].",
            "Либо разведите события во времени, либо добавьте маркер перемещения вида [loc: X = Y (путь …)].",
        ),
        (
            "sync-conflict",
            "Конфлікт синхронізації",
            "Sync conflict",
            "Конфликт синхронизации",
            "Дві машини редагували те саме місце документа, і автоматичне злиття не змогло вирішити, чия версія правильна.",
            "Two machines edited the same part of a document and automatic merge could not decide whose version is right.",
            "Две машины редактировали одно и то же место документа, и автоматическое слияние не смогло решить, чья версия правильная.",
            "Відкрийте файл, знайдіть маркери <<<<<<< / ======= / >>>>>>>, залиште потрібний варіант і збережіть документ.",
            "Open the file, find the <<<<<<< / ======= / >>>>>>> markers, keep the version you want and save the document.",
            "Откройте файл, найдите маркеры <<<<<<< / ======= / >>>>>>>, оставьте нужный вариант и сохраните документ.",
        ),
        (
            "fact-unverified",
            "Неперевірене твердження",
            "Unverified claim",
            "Непроверенное утверждение",
            "У факт-леджері є твердження без підтвердженого джерела, а матеріал готується до здачі.",
            "The fact ledger contains claims without a verified source while the piece is being prepared for submission.",
            "В факт-леджере есть утверждения без подтверждённого источника, а материал готовится к сдаче.",
            "Перевірте твердження за джерелом і позначте його перевіреним (skri fact verify) або приберіть його з тексту.",
            "Check the claim against its source and mark it verified (skri fact verify), or remove it from the text.",
            "Проверьте утверждение по источнику и отметьте его проверенным (skri fact verify) или уберите его из текста.",
        ),
        (
            "episode-structure",
            "Структура епізоду",
            "Episode structure",
            "Структура эпизода",
            "Епізод не відповідає формату 20-хвилинного анімаційного епізоду: бракує cold open, мінімуму поворотів або фінального гачка.",
            "The episode does not match the 20-minute animation format: it is missing a cold open, the minimum number of turns, or the final hook.",
            "Эпизод не соответствует формату 20-минутного анимационного эпизода: не хватает cold open, минимума поворотов или финального крючка.",
            "Додайте відсутні структурні елементи (cold open, ≥2 повороти, hook) або свідомо перебудуйте епізод.",
            "Add the missing structural elements (cold open, ≥2 turns, hook) or deliberately restructure the episode.",
            "Добавьте недостающие структурные элементы (cold open, ≥2 поворота, hook) или сознательно перестройте эпизод.",
        ),
        (
            "dialogue-streak",
            "Занадто багато діалогових сцен підряд",
            "Too many dialogue scenes in a row",
            "Слишком много диалоговых сцен подряд",
            "Кілька діалогових сцен ідуть підряд без перерв на дію — «персонажі просто стоять і розмовляють», для анімації це червоний прапор.",
            "Several dialogue-heavy scenes run in a row without action breaks — \"characters standing around talking\", a red flag for animation.",
            "Несколько диалоговых сцен идут подряд без перерывов на действие — «персонажи просто стоят и разговаривают», для анимации это красный флаг.",
            "Розбавте серію сценами з дією або переробіть частину діалогів в екшн.",
            "Break the streak with action scenes or rework part of the dialogue into action.",
            "Разбавьте серию сценами с действием или переработайте часть диалогов в экшн.",
        ),
        (
            "mute-test",
            "Сцена гасне без звуку",
            "Scene fails the mute test",
            "Сцена гаснет без звука",
            "У сцені майже немає дії, лише діалог: якщо вимкнути звук, буде незрозуміло, що відбувається.",
            "The scene has almost no action, only dialogue: with the sound off you cannot tell what is happening.",
            "В сцене почти нет действия, только диалог: если выключить звук, будет непонятно, что происходит.",
            "Додайте action-лінії — біти, які рухають картину, поки персонажі мовчать.",
            "Add action lines — beats that move the picture while the characters stay silent.",
            "Добавьте action-строки — биты, которые двигают картинку, пока персонажи молчат.",
        ),
        (
            "graph-unreachable",
            "Недосяжна нода",
            "Unreachable node",
            "Недостижимая нода",
            "Нода діалогового графа існує, але на неї ніхто не переходить — гравець туди ніколи не потрапить.",
            "A dialogue graph node exists but nothing leads to it — the player will never reach it.",
            "Нода диалогового графа существует, но на неё никто не переходит — игрок туда никогда не попадёт.",
            "Приберіть ноду або додайте перехід [goto ...] з місця, яке доступне гравцеві.",
            "Remove the node or add a [goto ...] transition from a place the player can actually reach.",
            "Уберите ноду или добавьте переход [goto ...] из места, доступного игроку.",
        ),
        (
            "graph-dangling-goto",
            "Перехід у нікуди",
            "Dangling goto",
            "Переход в никуда",
            "[goto X] веде на ноду, якої не існує — гравець би побачив зламану діалогову гілку.",
            "[goto X] points to a node that does not exist — the player would hit a broken dialogue branch.",
            "[goto X] ведёт на ноду, которой не существует — игрок попал бы на сломанную диалоговую ветку.",
            "Виправте назву ноди в переході або створіть відсутню ноду.",
            "Fix the node name in the goto or create the missing node.",
            "Исправьте название ноды в переходе или создайте недостающую ноду.",
        ),
        (
            "var-orphan-check",
            "Змінна перевіряється, але не встановлюється",
            "Variable checked but never set",
            "Переменная проверяется, но не устанавливается",
            "Діалог перевіряє змінну (%CHECK), але ніде її не встановлює (%SET) — умова або ніколи не спрацює, або спрацює випадково.",
            "Dialogue checks a variable (%CHECK) but never sets it (%SET) — the condition either never fires or fires by accident.",
            "Диалог проверяет переменную (%CHECK), но нигде её не устанавливает (%SET) — условие либо никогда не сработает, либо сработает случайно.",
            "Додайте %SET у потрібному місці або приберіть зайву перевірку.",
            "Add a %SET where it belongs or remove the stray check.",
            "Добавьте %SET в нужном месте или уберите лишнюю проверку.",
        ),
        (
            "var-dead-flag",
            "Мертвий прапорець",
            "Dead flag",
            "Мёртвый флаг",
            "Змінна встановлюється (%SET), але ніде не перевіряється — ця гілка сюжету ні на що не впливає.",
            "A variable is set (%SET) but never checked — that story branch affects nothing.",
            "Переменная устанавливается (%SET), но нигде не проверяется — эта сюжетная ветка ни на что не влияет.",
            "Або використайте прапорець у перевірці (%CHECK), або приберіть його встановлення.",
            "Either use the flag in a check (%CHECK) or remove its set.",
            "Либо используйте флаг в проверке (%CHECK), либо уберите его установку.",
        ),
        (
            "stat-change",
            "Ретроактивна правка статів",
            "Retroactive stat change",
            "Ретроактивная правка статов",
            "Правка минулого тексту змінила вже зафіксовані значення статів — пізніші глави тепер перераховуються від нового числа.",
            "An edit to earlier text changed already recorded stat values — later chapters now recalculate from the new number.",
            "Правка прошлого текста изменила уже зафиксированные значения статов — поздние главы теперь пересчитываются от нового числа.",
            "Перегляньте каскад по пізніх главах у payload і вирішіть: це свідомий реткон чи помилка в дельті.",
            "Review the cascade across later chapters in the payload and decide: intentional retcon or a delta mistake.",
            "Просмотрите каскад по поздним главам в payload и решите: сознательный реткон или ошибка в дельте.",
        ),
        (
            "stat-error",
            "Помилка статів",
            "Stat error",
            "Ошибка статов",
            "Атрибут пішов у мінус (наприклад, hp нижче нуля) — у прогресії це майже завжди помилка арифметики дельт.",
            "An attribute went negative (e.g. hp below zero) — in progression that is almost always a delta arithmetic mistake.",
            "Атрибут ушёл в минус (например, hp ниже нуля) — в прогрессии это почти всегда ошибка арифметики дельт.",
            "Знайдіть главу, де значення пішло вниз (перелік у payload), і виправте дельту або баланс.",
            "Find the chapter where the value went down (listed in the payload) and fix the delta or the balance.",
            "Найдите главу, где значение ушло вниз (список в payload), и исправьте дельту или баланс.",
        ),
        (
            "episode-check",
            "Зауваження валідатора епізоду",
            "Episode validator remark",
            "Замечание валидатора эпизода",
            "Валідатор епізоду знайшов структурні зауваження — звіт збережено в алертах для дашборда.",
            "The episode validator found structural remarks — the report is stored as an alert for the dashboard.",
            "Валидатор эпизода нашёл структурные замечания — отчёт сохранён в алертах для дашборда.",
            "Розкрийте алерт: у payload перелічені конкретні зауваження по сценах епізоду.",
            "Open the alert: the payload lists the specific remarks per episode scene.",
            "Раскройте алерт: в payload перечислены конкретные замечания по сценам эпизода.",
        ),
        (
            "ink-external-change",
            "Зовнішня зміна ink/yarn",
            "External ink/yarn change",
            "Внешнее изменение ink/yarn",
            "Файл .ink/.yarn, який раніше імпортували, змінився поза програмою — імпортований markdown вже не відповідає джерелу.",
            "An .ink/.yarn file that was imported earlier changed outside the program — the imported markdown no longer matches the source.",
            "Файл .ink/.yarn, который раньше импортировали, изменился вне программы — импортированный markdown больше не соответствует источнику.",
            "Повторно імпортуйте файл, щоб підтягнути зміни, або свідомо ігноруйте різницю.",
            "Re-import the file to pick up the changes, or deliberately ignore the difference.",
            "Повторно импортируйте файл, чтобы подтянуть изменения, или сознательно игнорируйте разницу.",
        ),
        (
            "pitch-followup",
            "Час follow-up пітчу",
            "Pitch follow-up due",
            "Пора сделать follow-up питча",
            "Пітч відправлено, строк очікування відповіді минув, а відповіді немає — час нагадати редакції про себе.",
            "The pitch was sent, the wait window has passed and there is no reply — time to remind the editor.",
            "Питч отправлен, срок ожидания ответа прошёл, а ответа нет — пора напомнить редакции о себе.",
            "Напишіть слідкуюче повідомлення й оновіть статус пітча на followed_up (skri pitch set ...).",
            "Send a follow-up message and move the pitch to followed_up (skri pitch set ...).",
            "Напишите следящее сообщение и переведите питч в followed_up (skri pitch set ...).",
        ),
        (
            "broken-links",
            "Поламане вікі-посилання",
            "Broken wiki link",
            "Сломанная вики-ссылка",
            "Нотатка посилається на [[нотатку]], якої не існує в проєкті.",
            "A note links to [[a note]] that does not exist in the project.",
            "Заметка ссылается на [[заметку]], которой нет в проекте.",
            "Або створіть відсутню нотатку, або виправте назву посилання.",
            "Either create the missing note or fix the link's target name.",
            "Либо создайте недостающую заметку, либо исправьте название ссылки.",
        ),
        (
            "orphan-notes",
            "Нотатка-сирота",
            "Orphan note",
            "Заметка-сирота",
            "На нотатку не веде жодне посилання з інших нотаток — вона випала з графа знань.",
            "No other note links to this note — it has fallen out of the knowledge graph.",
            "На заметку не ведёт ни одна ссылка из других заметок — она выпала из графа знаний.",
            "Додайте посилання на неї з відповідних нотаток або вирішіть, чи вона взагалі потрібна.",
            "Link to it from related notes, or decide whether it is needed at all.",
            "Добавьте ссылки на неё из соответствующих заметок или решите, нужна ли она вообще.",
        ),
        (
            "stale-notes",
            "Застаріла нотатка",
            "Stale note",
            "Устаревшая заметка",
            "Нотатку не змінювали вже N днів — можливо, її час перечитати чи оновити.",
            "The note has not been touched for N days — maybe it is time to revisit or update it.",
            "Заметку не меняли уже N дней — возможно, пора её перечитать или обновить.",
            "Перечитайте нотатку, оновіть її або приберіть, якщо вона більше не актуальна.",
            "Re-read the note, update it, or drop it if it is no longer relevant.",
            "Перечитайте заметку, обновите её или уберите, если она больше не актуальна.",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_json_clean_and_complete() {
        let a = catalog_json();
        let b = catalog_json();
        assert_eq!(a, b, "catalog_json має бути чистою функцією");
        let arr = a.as_array().expect("масив");
        assert!(arr.len() >= 20);
        for item in arr {
            for f in [
                "kind",
                "severity",
                "title_uk",
                "title_en",
                "title_ru",
                "explain_uk",
                "explain_en",
                "explain_ru",
                "action_uk",
                "action_en",
                "action_ru",
            ] {
                let v = item.get(f).and_then(|x| x.as_str()).unwrap_or_default();
                assert!(!v.trim().is_empty(), "порожнє поле {f} у {}", item["kind"]);
            }
        }
    }
}
