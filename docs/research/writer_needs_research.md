# Нереалізовані потреби нон-фікшн авторів (дослідження, листопад 2026)

Джерела: Reddit (r/Journalism, r/freelanceWriters, r/Blogging, r/ObsidianMD, r/GradSchool, r/AskAcademia, r/technicalwriting, r/Substack, r/books), Hacker News, Zotero forums, Overleaf GitHub, GIJN, Zotero KB. Цитати — дослівні з гілок (англ. оригінал).

## 1. Крос-сторі трекінг джерел і цитат ("хто що сказав")
**Біль:** фріланс-журналісти жонглюють кількома історіями; нотатки в Google Docs + email + зошит; втрачають інформацію і повторно ставлять те саме питання джерелу.
**URL:** https://www.reddit.com/r/Journalism/comments/1sc2o2i/freelance_journalists_how_do_you_manage_your/
**Чому не закривають існуючі:** Notion/Airtable — генеричні CRM-подібні бази без інтерв'ю-центричної моделі; транскрайбери (Otter/Trint) не пов'язують цитату зі стороною і не пам'ятають контекст попередніх інтерв'ю з тим самим джерелом.
**Цитата:** "Initially, my organizational approach was quite chaotic, relying on Google Docs, email threads, and a notebook… This often led to lost information and instances where I inadvertently asked the same question to a source in different interviews—definitely not ideal."

## 2. Pitch-трекінг і ре-сабмішини
**Біль:** пітчі трекають жирним шрифтом у Word-документі; спредшіети «стають хаотичними»; повторний пітч того ж матеріалу в кілька видань не моделюється (спредшіет відсортований за виданням, а не за п'єсою).
**URL:** https://www.reddit.com/r/Journalism/comments/1roz3es/staying_organised_and_tracking/ ; https://www.reddit.com/r/freelanceWriters/comments/1imgnpp/does_anyone_have_a_decent_system_for_tracking/
**Чому не закривають:** Trello/Notion — генеричні канборди без статусів пітчу (pitched → follow-up date → resubmit elsewhere), без норм етикету фолоу-апів.
**Цитата:** "I maintain a Word document filled with story ideas… Whenever I pitch an idea, I highlight in bold the name of the publication…"; "I'm struggling to devise an effective method… especially when it comes to pitching the same piece to various magazines, as my spreadsheet is primarily sorted by publication name."; "I utilize the notes application on my smartphone along with Google Calendar."

## 3. Очікування відповіді редактора / емаіл-центричність пітчингу
**Біль:** невідомо, коли редактор відповість; усі взаємодії — розсип email-ів; помилки типу «прикріпив не ту статтю».
**URL:** https://www.reddit.com/r/Journalism/comments/17b2ofz/submission_and_pitch_pain_points/
**Чому не закривають:** жоден інструмент (Submittable — для літературних журналів, Notion) не моделює емаіл-воркфлоу пітчів редакторам журналів зі слідами фолоу-апів і версіями чернеток.
**Цитата:** "Another pain point is the waiting, and the uncertainty around that waiting. You pitch, you have no idea when the commissioning editor will get back to you."; "Just this morning I finished a story at 730am… I'd attached the wrong story."

## 4. Верифікація цитат проти аудіо (видимий шар перевірки)
**Біль:** транскрипт знайде цитату, але перед публікацією кожну цитату треба звірити з аудіо; статус «перевірено» ніде фіксувати і тягнути в драфт.
**URL:** https://otio.ai/blog/best-ai-tools-journalists-research-interviews-fact-checking ; https://speecho.app/blog/interview-transcription-for-journalists/ ; https://www.merlin.computer/ai-for/journalists
**Чому не закривають:** Otter/Descript/Trint дають пошуковий транскрипт, але не інтегровані з редактором чернетки; немає інструменту, де цитата в драфті лінкується на таймкод зі статусом "verified".
**Цитата:** "the transcript locates the quote, the recording confirms it"; "nothing goes inside quotation marks until you've heard it."

## 5. Безпека джерел при роботі з записами
**Біль:** завантаження запису в хмару = третя сторона має матеріал, який обіцяно захистити; оф-рекорд фрагменти треба різати до аплоаду; ретеншн після публікації — ручні рішення.
**URL:** https://www.merlin.computer/ai-for/journalists
**Чому не закривають:** жоден популярний транскрайбер не пропонує локальний-only pipeline з політиками ретеншну і маркуванням off-record.
**Цитата:** "uploading a recording to a cloud service gives a third party material you may have promised to shield. So set one rule: nothing goes inside quotation marks until you've heard it."

## 6. Response-to-reviewers як першокласний об'єкт
**Біль:** ревізії після peer-review — ручний Excel-трекер: копіюєш коментар рецензента, пишеш відповідь, підсвічуєш у манускрипті кольором, потім вручну збираєш response letter. Десятки коментарів від кількох рецензентів, дублікати між рецензентами треба дедуплікувати вручну.
**URL:** https://www.reddit.com/r/AskAcademia/comments/1lsi5nw/strategy_for_paper_revisions/ ; https://www.reddit.com/r/GradSchool/comments/7hq2fl/feeling_hopeless_and_overwhelmed_after_receiving/
**Чому не закривають:** Word/Zotero/Overleaf не знають про зв'язок «коментар рецензента → зміна в рукописі → відповідь у листі»; Latex/Overleaf без track-changes-еквівалента.
**Цитата:** "It's a nightmare… I like to use an Excel spreadsheet and re-word criticisms into less harsh/grating phrases."; "Make an excel sheet with a list of the revisions and space to write how you addressed them… I highlight any page numbers in yellow… Once I've addressed a comment… I mark both the comment and response in green."

## 7. Zotero + Obsidian + Scrivener «трійця»: немає одного інструменту для лонгформу з цитатами
**Біль:** нон-фікшн книга з інтерв'ю і сотнями цитат вимагає трьох аплікейшн-ів; експорт з Obsidian у docx з footnote — величезна ручна праця; Scrivener не інтегрує Zotero.
**URL:** https://www.reddit.com/r/ObsidianMD/comments/vc35ey/limitations_in_using_obsidianmarkdown_for/ ; https://forums.zotero.org/discussion/108575/using-zotero-obsidian-and-scrivener-together-as-a-system
**Чому не закривають:** Scrivener — без reference manager; Obsidian — лонгформ і експорт хромають; Zotero — нотатки ізольовані від редактора.
**Цитата:** "Getting the material out of Obsidian in a usable form, with footnotes and the like was an enormous about of labour."; "It still bugs some part of me that I have to use three tools… Scrivener for writing, Zotero for references, Obsidian for notes."; "the problem with Word is… not designed for long projects like books… but it doesn't give you integration with any reference manager."

## 8. Overleaf + Zotero group libraries: refresh тільки для «власника» імпорту
**Біль:** співавтори не можуть оновити zotero.bib — «Access Denied»; workaround — окремі bib-файли на автора і дублікати.
**URL:** https://github.com/overleaf/overleaf/issues/1065
**Чому не закривають:** Overleaf impose arbitrary limitation; Zotero API це дозволяє.
**Цитата:** "Right now it doesn't really — only the user who originally added the group library is allowed to refresh it, which defeats the point of using group libraries in the first place."

## 9. Zotero повільні вставки цитат у довгих документах
**Біль:** вставка цитати сканує весь документ — секунди/хвилини на великій статті/книзі; Word для Mac 2008 і Google Docs особливо страждають.
**URL:** https://www.zotero.org/support/kb/citations_slow
**Чому не закривають:** це офіційний KB від Zotero — тобто відома і невирішена (тільки обхідні шляхи: розділяти документи, вимикати auto-update).
**Цитата:** "In longer documents, scanning the entire document can take multiple seconds or even minutes, and these updates can become disruptive to the writing process."

## 10. Content decay / оновлення опублікованих статей
**Біль:** блогер зі 1000+ сторінок не може вручну відстежувати застарілі факти/статистику/лінки в старих постах; спредшіет стає time burden; контент у tech/finance «застаріває і вводить в оману через кілька місяців».
**URL:** https://www.reddit.com/r/Blogging/comments/1g5x7g9/keeping_blog_content_up_to_date/ ; https://www.reddit.com/r/seogrowth/comments/1raxdex/content_decay_is_killing_our_traffic_how_are_you/
**Чому не закривають:** редактор Substack/WordPress/Notion не знає, які факти-твердження в яких постах; SEO-інструменти бачать трафік, але не зв'язок «стаття → конкретна цифра → джерело, що застаріло».
**Цитата:** "going through my old articles started to become very tedious when my blog had 1000+ pages"; "In my space (tech/finance) content can become outdated and misleading after a few months — which apart from SEO isn't good for the brand"; "the core issue wasn't understanding the need to update our content, but rather the absence of a consistent process."

## 11. Research overload: конфліктні джерела → структурований outline
**Біль:** вичитувати десятки джерел, розв'язувати суперечності між ними і перетворити розсипані нотатки на цитат-реді outline — саме тут біль, а не в читанні.
**URL:** https://www.reddit.com/r/Blogging/comments/1n8kf5z/how_do_you_handle_research_overload_when_writing/
**Чому не закривають:** Obsidian зберігає нотатки, але не розв'язує суперечності і не будує outline з готовими для цитування блоками; Mural/міндмепи — ручні.
**Цитата:** "The real difficulty lies not only in reading all that material but also in: Efficiently pulling out the essential information; Resolving conflicting statements from different sources; Transforming my scattered notes into a coherent outline that's ready for citations"; "When I begin developing my ideas, I typically start by opening around 100 tabs for research."

## 12. Дослідження як вузьке горло (5x довше за написання)
**Біль:** research займає в рази більше часу, ніж письмо; до кінця research автор «втомлений і менш натхненний» писати.
**URL:** https://www.reddit.com/r/Blogging/comments/ezg0g0/how_much_referencecitation_in_your_writing/
**Чому не закривають:** інструменти нотаток зберігають, але не скорочують час між «знайшов» і «можу процитувати з точним твердженням».
**Цитата:** "Research can take easily 5x longer than writing. I find that by the time I've gone through all the research I'm worn down and less excited to write about the topic."

## 13. Fact-checking нон-фікшн книги: видавці не перевіряють, все на авторі
**Біль:** видавці не мають фактчекерів; автор має сам (або за свій рахунок) верифікувати кожне твердження; з AI-чернетками задача стала системною — кожна цифра/цитата/дата має лінк на джерело.
**URL:** https://www.reddit.com/r/books/comments/ih014q/fact_checking_is_the_core_of_nonfiction_writing/ ; https://www.builtwritten.com/blog/write-nonfiction-book-with-ai-without-making-up-facts
**Чому не закривають:** Scrivener/Word — без моделі «твердження → джерело → статус верифікації»; Zotero керує бібліографією, але не мапою фактів по рукопису.
**Цитата:** "Publishers expect authors to do fact checks themselves (or to pay for someone else to do it using money from the royalty advance)"; "The Reddit author's solution was to track the source of each claim and separate AI drafting from factual verification."; "When your workflow is fragmented across half a dozen platforms, verification becomes its own full-time job."

## 14. Техрайтери: трекінг-спредшіет дорожчий за саму правку; знання в голові однієї людини
**Біль:** Excel-трекінг змін у доках забирає більше часу, ніж реальні правки; після звільнення ветерана команда не знає, які версії документів актуальні; Confluence «як старий Word», Notion — «продуктивність, не документація».
**URL:** https://www.reddit.com/r/technicalwriting/comments/65hxoa/documentation_tracking_toolsystem/ ; https://www.reddit.com/r/technicalwriting/comments/1wobeq5/looking_for_a_docs_tool_that_doesnt_make_one/ ; https://www.reddit.com/r/technicalwriting/comments/1lps5jy/what_documentation_tool_is_actually_working_for/
**Чому не закривають:** Confluence/GitHub-README не зв'язують доки з кодом/релізами; немає інструменту, який прапорить ймовірні розбіжності док↔код для рев'ю.
**Цитата:** "Often, the time spent updating the tracking documents exceeds the time taken to implement the actual changes… there's currently no effective way to get a comprehensive view of ongoing tasks"; "a long-time employee departed, revealing how heavily our documentation relied on their knowledge of where everything was located"; "Confluence, but it often feels like I'm battling with an outdated version of Microsoft Word."

## 15. Техрайтери: інконсистентні формати цитат під вимоги клієнтів
**Біль:** клієнти постійно змінюють вимоги до форматів цитування (IEEE/APA/Chicago/Vancouver); ретрофіт пізніше — «дуже складно»; імена/дати/URL/доступ-дати — джерело помилок.
**URL:** https://www.reddit.com/r/technicalwriting/comments/1ml3o4a/how_do_you_handle_citation_format_inconsistencies/
**Чому не закривають:** Zotero/BibTeX вирішує академічний кейс, але «брудні» реальні цитати з PDF і клієнтські перемикання стилів вимагають ручних спредшіетів з колонками на кожен елемент.
**Цитата:** "Retrofitting citations later can be quite challenging… I maintain a spreadsheet with dedicated columns for each citation element. This approach simplifies reformatting when clients alter their requirements (which occurs more often than I'd prefer)."

## 16. Бонус: ньюслеттери — фрагментований стек і «заметки → випуск»
**Біль:** Notion (пайплайн) + Google Docs (чернетки) + Substack (публікація) не інтегровані; головний біль — перетворити хаос нотаток на один когерентний випуск.
**URL:** https://www.reddit.com/r/Substack/comments/1v7qfmn/how_many_of_you_are_on_substack/ ; https://www.reddit.com/r/Substack/comments/1p8w33e/newsletter_writers_whats_your_biggest_workflow/
**Чому не закривають:** Substack editor — тільки публікація; Notion не підключений до публікації.
**Цитата:** "Unfortunately, these tools don't integrate, and often my Notion board ends up being an unused resource"; "What frustrates me the most is the disconnect between having '50 ideas about coding' and transforming that into 'a clear, actionable topic for my audience.'"; "My notes app is overflowing with screenshots, git diffs, inquiries from junior developers… it resembles a detective's crime board!"
