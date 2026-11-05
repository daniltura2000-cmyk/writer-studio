# Дослідження невирішених потреб авторів художньої прози та вебновел
(субагент-звіт, листопад 2026; джерела — Reddit r/writing, r/fantasywriters, r/royalroad, r/litrpg, r/ProgressionFantasy, r/FanFiction, r/AO3, r/worldbuilding, r/Webnovel, RoyalRoad форуми)

## ТОП-потреби (відсортовано за частотою/силою болю)

### 1. Автоматичний story bible / continuity-трекінг із тексту (найсильніша потреба)
- Біль: за 50k+ слів автори забувають колір очей, місце персонажа, хто живий; story bible "застаріває" і розходиться з рукописом. Оновлення вручну — друга робота, яку кидають.
- Чому не закрито: Scrivener/Novelcrafter Codex/World Anvil = ручне заповнення. Жоден з мейнстрім-інструментів сам не витягує факти з тексту й не прапорить конфлікти.
- Джерела:
  - https://www.reddit.com/r/writing/comments/1rk2t3n/how_do_you_actually_track_continuity_across_a/ — "Spreadsheets — effective, but cumbersome... I often forget to revise them during the drafting process. Story bible... tends to become misaligned with the actual manuscript."
  - https://www.reddit.com/r/writing/comments/132z765/take_notes_on_your_characters/ — "found myself having to scroll through 11 chapters just to remember what hand someone writes with"
  - https://www.royalroad.com/forums/thread/178150 — автори на RR-форумі прямо кажуть: "most tools give you a place to keep your notes... but you still have to keep those notes up to date yourself"

### 2. LitRPG/progression: движок статів і лічби прогресії
- Біль: трекінг статів/рівнів/XP багатьох персонажів по главах; ретроактивна зміна рівня в гл. 5 ламає числа у всіх наступних главах. В handcrafted Excel — 70 колонок; ведення книжності їсть час нарівні з письменством. Автори пишуть власні Python-скрипти й навіть Git для character sheet.
- Чому не закрито: жоден writing-інструмент не має моделі "state machine статів з подіями по главах" і не вміє перерахунок при retcon.
- Джерела:
  - https://www.reddit.com/r/litrpg/comments/e567jr/keeping_track_of_stats_as_an_author/ — "this bookkeeping often consumes as much time, if not more, than the actual writing... chart with 70 columns"
  - https://www.reddit.com/r/litrpg/comments/1hmb5ia/litrpg_authors_how_do_you_keep_track_of_your/ — автори пишуть власні Python-трекери
  - https://www.reddit.com/r/ProgressionFantasy/comments/1g4w09m/and_then_by_ch_30_a_reader_points_out_a_critical/ — "reader points out a critical stat error from ch 5"
  - https://www.reddit.com/r/litrpg/comments/1uv61cz/litrpg_stat_tracking_tool_for_writers/ — спроба такого тулза; "having to adjust a level-up from earlier... a genuine concern in serialized writing"

### 3. Крос-постинг і синхронізація глав між платформами (RR/ScribbleHub/Wattpad/AO3/Patreon)
- Біль: 5 вкладок, копі-паст заголовка і тіла глави вручну, різні редактори/форматування, правки глави = повторний аплоад всюди.
- Чому не закрито: жодного офіційного/стороннього пайплайну "один манускрипт → N платформ із розкладом".
- Джерела:
  - https://www.reddit.com/r/royalroad/comments/1ritiyu/do_you_crosspost_to_other_sites_does_it_actually/ — "I open five tabs, copy the chapter title once, paste it into all five... setting each release date individually"
  - https://www.royalroad.com/forums/thread/140823 — "it's also twice the work to upload on twice as many sites, especially if you ever have to go back and edit"

### 4. Патреон-логістика (tiers, advance chapters, черга публікацій)
- Біль: розкидання глав по тірах (ch 160 → tier 3, ch 130 → tier 2, ch 123 → всі) вручну щоразу; Patreon не призначений для серіалу; автори пишуть власні Chrome-розширення.
- Джерела:
  - https://www.reddit.com/r/litrpg/comments/1ra8toh/patreon_sucks_for_authors_posting_chapters_is_a/ — "What a disaster... I spent around an hour and a half developing the extension" (автор сам написав екстеншн, бо інструменту нема)
  - https://www.reddit.com/r/royalroad/comments/1du4lwz/can_you_only_schedule_five_chapters_in_advance/ — "It's definitely a lot of maintenance to do the 'advanced chapter' thing... It gets very tedious."
  - https://www.reddit.com/r/litrpg/comments/1so9e6q/authors_this_is_probably_a_stupid_question_but/ — "high potential for confusion or mistakenly sharing the incorrect chapter"

### 5. Планування release-розкладу серії та беклогу
- Біль: беклог ≠ просто файли: треба тримати буфер (1-3 міс), розклад днів тижня, перемикання між серіями при burnout, розкриття швидкості для RS-алгоритму RR. Все ведеться в Google Sheet.
- Джерела:
  - https://www.reddit.com/r/royalroad/comments/1hsxfmn/help_with_schedule_slow_writer/ — "I have more than 650 chapters spread across nine series... if I start to feel burnt out I can switch"
  - https://www.reddit.com/r/royalroad/comments/1ni4e92/the_downside_of_a_substantial_backlog/ — беклог + страх інконсистентності між написаним і опублікованим
  - https://www.reddit.com/r/royalroad/comments/1os3y6p/to_the_authors/ — "unrealistic output level that readers and the algorithm expect"

### 6. Редагування вже опублікованих глав / retcon-менеджмент
- Біль: після публікації глава "застигає"; виправлення в гл. 7 вимагає перевірки мільйона слів далі; два "бранчі" (draft vs published) підтримувати вручну в RR+Patreon+...
- Джерела:
  - https://www.reddit.com/r/ProgressionFantasy/comments/105oqt6/quickly_debunking_the_most_common_misconception/ — "Does fixing one chapter 1000000 words ago mean I have to review the rest of those million words?... The fix never happened."
  - https://www.reddit.com/r/litrpg/comments/1e8lm3t/the_sideeffects_of_web_serial_publishing_thoughts/ — "you're now having to copy paste 7 chapters x2 to both RR and Patreon"

### 7. Таймлайн-чекер (дати, віки, подорожі, "хто де коли")
- Біль: персонажі знають те, чого ще не могли дізнатися; подорожі 3 дні vs 2; вік не сходиться. Рішення — вручну календар у документі.
- Джерела:
  - https://www.reddit.com/r/AO3/comments/1ta7ja1/what_are_your_methods_for_keeping_track_of/ — "avoid the issue of characters having knowledge they shouldn't possess until much later, like Chapter 20"; "I can't imagine managing travel times without it [spreadsheet]"
  - https://www.reddit.com/r/FanFiction/comments/1mzcmcg/no_one_talks_about_how_hard_keeping_track_of_a/ — "No one talks about how hard keeping track of a timeline... is"

### 8. Трекер відкритих сюжетних ліній / setup-payoff
- Біль: автори ведуть "chapter post-mortem" документи вручну, копіюючи стан після кожної глави (established rules, open threads, questions characters have).
- Джерела:
  - https://www.reddit.com/r/AO3/comments/1ta7ja1/ — "chapter post-mortem... a living document that outlines all the universal rules I've set, plot threads ready to be resolved"
  - https://www.reddit.com/r/fantasywriters/comments/1dtq465/how_do_you_keep_track_of_the_numerous_plot/ — "Obsidian... merely mitigates complexity rather than solving it"

### 9. Легкий worldbuilding-інструмент саме для романіста (не wiki-D&D)
- Біль: World Anvil — надлишковий, відволікає від письма, платні фічі, "shaping world to fit the platform". Google Docs не виношує зв'язки.
- Джерела:
  - https://www.reddit.com/r/worldbuilding/comments/1ioi11t/what_are_your_thoughts_about_worldanvil/ — "overstuffed, distracting, and overwhelming"; "my world wasn't ready for it... I ended up shaping my world to fit World Anvil's templates"
  - https://www.reddit.com/r/worldbuilding/comments/9tq8pv/is_world_anvil_as_good_as_it_sounds/ — "EXTREMELY complicated... nothing to show for it in the way of useable content"

### 10. POV/tense consistency checker
- Біль: змішування часів/особи всередині глави; Google Docs ловить частково; контекстно-залежні випадки (флешбеки, діалоги) ніхто не вміє розрізняти.
- Джерела:
  - https://www.reddit.com/r/AO3/comments/1vnu99r/we_have_to_do_better_at_tense_consistency/ — "many word processing programs may not catch these issues, as the appropriateness of a tense shift can depend heavily on context"
  - https://www.reddit.com/r/writing/comments/186yowv/selfpublished_authors_you_need_to_maintain/ — self-pub автори системно ламають POV

### 11. Автогенерація рекапів для читачів
- Біль: на 100+ главах читачі губляться; автори пишуть рекапи кожної книги ВРУЧНУ (Mage Tank — 4 рекапи вручну).
- Джерела:
  - https://www.royalroad.com/fiction/76463/mage-tank/chapter/4252586/book-1-recap — "With the lack of recaps on RR... you all may have been neglected"
  - (низка фіктів мають окремі recap-глави — пошук "recap" на RR дає десятки)

### 12. Менеджер імен/назв (колізії, плейсхолдери, забуті назви)
- Біль: "Sam becomes Stanislaw halfway thru the novel"; два імені для Бога Сонця; [STREET where Maggie lives] плейсхолдери губляться.
- Джерела:
  - https://www.reddit.com/r/writing/comments/txevaf/is_it_normal_to_sometimes_forget_character_or/ — "there are two names for my Sun God and I can't remember when I switch it and why"
  - https://www.reddit.com/r/writing/comments/u3j5se/when_in_your_writing_process_do_you_start_naming/ — плейсхолдери в квадратних дужках як звичайна практика

### 13. Чесна аналітика читачів по серії (reads vs views, воронка)
- Біль: RR рахує перегляди (боти/refresh), "views ≠ readers"; детальна аналітика — лише платний Author Premium; дані по крос-постингу розкидані по 5 дашбордах.
- Джерела:
  - https://www.reddit.com/r/royalroad/comments/1k63v1r/are_views_on_my_novel_are_real/ — "views don't equate to reads... the exact number of genuine reads remains uncertain"
  - https://www.royalroad.com/forums/thread/139888?page=2 — сумніви авторів у походженні аналітики RR (GA-фільтрація)

### 14. Портфель WIP-ів для фанфік-авторів (10-100 проєктів)
- Біль: 20-100 незавершених фіків; handmade-системи "Fic of the Day", слоти, Trello. Жоден writing-ап не заточений під багатопроєктне fanfic-портфоліо з фандомами/дедлайнами обмінів.
- Джерела:
  - https://www.reddit.com/r/FanFiction/comments/13uu19k/who_else_has_too_many_wips/ — "I have more than a hundred works in progress"
  - https://www.reddit.com/r/AO3/comments/1j06wi3/how_many_fics_are_you_working_on_at_the_same_time/ — "32 incomplete pieces in that particular fandom alone... I lack a system"

### 15. Зведений дашборд роялті/статистики по всіх платформах
- Біль: KDP + D2D + Apple/Kobo + RR + Patreon = ручний щомісячний spreadsheet; "no single tool provides a fully automated, accurate multi-platform dashboard... without manual data input" (стан на 2026).
- Джерела:
  - https://liberscript.com/guides/ebook-sales-reporting-guide
  - https://kdp.amazon.com/en_US/help/topic/G201493250 (постійні затримки/різні звіти)

### 16. Референс фандом-канону vs fanon
- Біль: автори шоковані, що "канон" виявився фаноном; проверка деталей канону — відкриті вікі в 20 вкладках.
- Джерела:
  - https://www.reddit.com/r/FanFiction/comments/oygq3k/do_you_ever_refer_back_to_source_material_when/ — "I'm sometimes shocked at what I thought was canon that turned out to be fanon"

### 17. Пайплайн "ідея з телефону → рукопис"
- Біль: нотатки в телефоні "only to never look at it again"; немає мосту нотатки→сцена→глава.
- Джерела:
  - https://www.reddit.com/r/writing/comments/18g500i/where_how_do_you_keep_random_ideas_for_a_story/
  - https://www.reddit.com/r/writing/comments/1852c49/how_do_you_capture_your_fleeting_ideas/

### 18. "Де хто зараз знаходиться" — локаційний трекер акторів
- Біль: "keeping track of where everyone went last (and who they went with) is a struggle"; частково входить в #1, але має власний UX (мапа/календар персонажів).
- Джерела:
  - https://www.reddit.com/r/writing/comments/132z765/take_notes_on_your_characters/ — порада "treat it like a group calendar for your characters" (Google Calendar як костиль!)

## Примітки
- Потреби 1, 2, 6 — найбільш часто згадувані і з найгострішим болем (втрата даних/перезапис глав).
- Потреби 3-5 — суто серіально-платформні, майже не торкаються існуючих writing-апів; саме тут нішові гравці пишуть власні скрипти/екстеншни (найчіткіший сигнал невирішеності).
- Ігноровано як вирішене: форматування/компайл (Vellum/Atticus), планування сюжету (Plottr), worldbuilding-вікі як клас (World Anvil/Kanka), AI-чернетки (Sudowrite), grammar (ProWritingAid).
