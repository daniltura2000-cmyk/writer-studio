//! Timeline Engine — часові маркери, календар світу, локаційний календар (§5.5).
//!
//! Принципи (залізне правило №1 дотримане): усе перебудовується з manuscript/
//! будь-коли — таблиці `timeline_points`/`timeline_locs` похідні від файлів.
//!
//! ## Календар світу
//! Джерело календарю (перше знайдене перемагає):
//! 1. `.skri/calendar.json` — `{"months":[{"name":"Тарвен","days":30},...],
//!    "days_of_week":["День Попелу",...]}` (місяці можна і рядками "Тарвен=30");
//! 2. `bible/calendar.md` з frontmatter `months: Тарвен=30, Вален=40` і
//!    опційним `days_of_week:`;
//! 3. григоріанський календар за замовчуванням (укр. назви місяців).
//!
//! «Абсолютна доба» — номер доби від початку оповіді (доба 1 = перша доба).
//! Явні дати («12 жовтня», «[time: день 12]») дають день-у-році календаря і
//! відображаються в РІК, у якому перебуває поточна доба (детерміновано).
//!
//! ## Маркери часу
//! - frontmatter глави `day_start: N` — абсолютна доба початку глави;
//!   маркери між главами накопичуються послідовно (без `day_start` глава
//!   продовжує з кінцевої доби попередньої);
//! - у тексті: `[time: +3 днів]`, `[time: день 12]`, `[time: наступний ранок]`,
//!   `[time: flashback]` — детерміновано, з байтовими offsetами;
//! - природні фрази (укр.+англ.): «наступного ранку», «через три дні»,
//!   «ввечері того ж дня», «за тиждень», «пізніше», «на світанку», «12 жовтня»,
//!   "next morning", "three days later", "at dawn" тощо.
//!
//! ## Валідація
//! - стрибок назад у добах БЕЗ активного флешбек-маркера (`[time: flashback]`)
//!   → алерт `kind='timeline-backwards'`;
//! - глава без жодного time-маркера → ПОПЕРЕДЖЕННЯ `timeline-missing` у звіті
//!   validate (НЕ алерт у БД);
//! - локації: `[loc: Хтось = Місце]` / `[loc: Хтось = Місце (шлях ...)]` —
//!   маркер переміщення (дужка) знімає конфлікт; сутність у ДВОХ місцях в ОДНУ
//!   добу без маркера переміщення → алерт `kind='location-conflict'`.
//!
//! Свідоме обмеження: регресія часу доби (ранок→вечір в ОДНУ добу) алертом не
//! вважається — алерт лише на стрибок назад у ДОБАХ (занадто багато хибних
//! спрацювань на звичайній прозі, де «вранці» пишеться довільно).

use crate::frontmatter;
use crate::store::ProjectStore;
use crate::Result;
use rusqlite::params;
use std::collections::BTreeMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Календар світу
// ---------------------------------------------------------------------------

/// Місяць календаря світу.
#[derive(Debug, Clone)]
pub struct Month {
    pub name: String,
    pub days: i64,
    /// форма для «12 <genitive>» (григоріанські: «жовтня»); None = як name
    pub genitive: Option<String>,
}

/// Календар світу: місяці/довжини + назви днів тижня.
#[derive(Debug, Clone)]
pub struct Calendar {
    pub months: Vec<Month>,
    pub days_of_week: Vec<String>,
    /// звідки взято: ".skri/calendar.json" | "bible/calendar.md" | "default-gregorian"
    pub source: String,
    /// варіанти написання кожного місяця (нижній регістр) для парсинга дат
    aliases: Vec<Vec<String>>,
}

const GREG_NOM: [&str; 12] = [
    "січень", "лютий", "березень", "квітень", "травень", "червень", "липень", "серпень",
    "вересень", "жовтень", "листопад", "грудень",
];
const GREG_GEN: [&str; 12] = [
    "січня", "лютого", "березня", "квітня", "травня", "червня", "липня", "серпня",
    "вересня", "жовтня", "листопада", "грудня",
];
const GREG_EN: [&str; 12] = [
    "january", "february", "march", "april", "may", "june", "july", "august", "september",
    "october", "november", "december",
];
const GREG_DAYS: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Календар за замовчуванням: григоріанський (укр. назви).
pub fn default_gregorian() -> Calendar {
    let months: Vec<Month> = GREG_NOM
        .iter()
        .zip(GREG_GEN.iter().zip(GREG_DAYS))
        .map(|(n, (g, d))| Month {
            name: n.to_string(),
            days: d,
            genitive: Some(g.to_string()),
        })
        .collect();
    Calendar {
        aliases: months
            .iter()
            .enumerate()
            .map(|(i, m)| {
                vec![
                    m.name.to_lowercase(),
                    GREG_NOM[i].to_string(),
                    GREG_GEN[i].to_string(),
                    GREG_EN[i].to_string(),
                    GREG_EN[i][..3].to_string(),
                ]
            })
            .collect(),
        days_of_week: vec![
            "понеділок", "вівторок", "середа", "четвер", "п'ятниця", "субота", "неділя",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        source: "default-gregorian".into(),
        months,
    }
}

fn months_from_pairs(pairs: &[(String, i64)]) -> Result<Calendar> {
    if pairs.is_empty() {
        return Err(crate::Error::Invalid("календар без місяців".into()));
    }
    for (name, days) in pairs {
        if name.trim().is_empty() || *days < 1 {
            return Err(crate::Error::Invalid(format!(
                "некоректний місяць календаря: {name:?} = {days}"
            )));
        }
    }
    let months: Vec<Month> = pairs
        .iter()
        .map(|(n, d)| Month { name: n.clone(), days: *d, genitive: None })
        .collect();
    // для кастомних назв додаємо відмінкові форми («Тарвеня», «Тарвені»),
    // щоб явні дати «12 Тарвеня» парсились
    let mut aliases: Vec<Vec<String>> = Vec::new();
    for m in &months {
        let base = m.name.to_lowercase();
        let mut forms = vec![base.clone()];
        for suf in ["а", "я", "и", "у", "і", "їв", "ів", "ом"] {
            forms.push(format!("{base}{suf}"));
        }
        aliases.push(forms);
    }
    Ok(Calendar {
        aliases,
        days_of_week: Vec::new(),
        source: String::new(),
        months,
    })
}

/// Завантажити календар світу з папки проєкту.
pub fn load_calendar(root: &Path) -> Result<Calendar> {
    let json_path = root.join(".skri").join("calendar.json");
    if json_path.exists() {
        let raw = std::fs::read_to_string(&json_path)?;
        let v: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| crate::Error::Invalid(format!(".skri/calendar.json: {e}")))?;
        let mut pairs: Vec<(String, i64)> = Vec::new();
        if let Some(arr) = v.get("months").and_then(|m| m.as_array()) {
            for item in arr {
                if let Some(o) = item.as_object() {
                    let name = o.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                    let days = o.get("days").and_then(|d| d.as_i64()).unwrap_or(0);
                    pairs.push((name, days));
                } else if let Some(s) = item.as_str() {
                    pairs.push(parse_month_pair(s).ok_or_else(|| {
                        crate::Error::Invalid(format!("calendar.json: місяць «{s}» не в форматі «Назва=Днів»"))
                    })?);
                }
            }
        }
        let mut cal = months_from_pairs(&pairs)
            .map_err(|e| crate::Error::Invalid(format!(".skri/calendar.json: {e}")))?;
        cal.source = ".skri/calendar.json".into();
        if let Some(dows) = v.get("days_of_week").and_then(|d| d.as_array()) {
            cal.days_of_week = dows
                .iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect();
        }
        return Ok(cal);
    }
    let md_path = root.join("bible").join("calendar.md");
    if md_path.exists() {
        let raw = std::fs::read_to_string(&md_path)?;
        let parsed = frontmatter::parse(&raw);
        let mut pairs: Vec<(String, i64)> = Vec::new();
        if let Some(spec) = parsed.meta.get("months") {
            for part in spec.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                pairs.push(parse_month_pair(part).ok_or_else(|| {
                    crate::Error::Invalid(format!("bible/calendar.md: місяць «{part}» не в форматі «Назва=Днів»"))
                })?);
            }
        }
        let mut cal = months_from_pairs(&pairs)
            .map_err(|e| crate::Error::Invalid(format!("bible/calendar.md: {e}")))?;
        cal.source = "bible/calendar.md".into();
        if let Some(spec) = parsed.meta.get("days_of_week") {
            cal.days_of_week = spec
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        return Ok(cal);
    }
    Ok(default_gregorian())
}

/// «Назва=Днів» або «Назва:Днів» або «Назва Днів» → (назва, днів).
fn parse_month_pair(s: &str) -> Option<(String, i64)> {
    let (name, days) = if let Some((n, d)) = s.split_once('=') {
        (n.to_string(), d)
    } else if let Some((n, d)) = s.split_once(':') {
        (n.to_string(), d)
    } else {
        // «Тарвен 30» — останній токен = число
        let mut it = s.split_whitespace();
        let days = it.next_back()?;
        (it.collect::<Vec<_>>().join(" "), days)
    };
    let days: i64 = days.trim().parse().ok()?;
    Some((name.trim().to_string(), days))
}

impl Calendar {
    /// Днів у році світу.
    pub fn year_length(&self) -> i64 {
        self.months.iter().map(|m| m.days).sum()
    }

    /// Індекс місяця за токеном назви (нижній регістр).
    pub fn month_index(&self, tok: &str) -> Option<usize> {
        for (i, al) in self.aliases.iter().enumerate() {
            if al.iter().any(|a| a == tok) {
                return Some(i);
            }
        }
        None
    }

    /// День-у-році (1-based) для місяця й дня місяця.
    pub fn day_of_year(&self, month_idx: usize, dom: i64) -> Option<i64> {
        let m = self.months.get(month_idx)?;
        if dom < 1 || dom > m.days {
            return None;
        }
        Some(self.months[..month_idx].iter().map(|m| m.days).sum::<i64>() + dom)
    }

    /// Людський вигляд абсолютної доби: «12 жовтня», «5 Вален — День Вогню, рік 2».
    pub fn format_day(&self, day: i64) -> String {
        if day < 1 {
            return format!("доба {day}");
        }
        let yl = self.year_length().max(1);
        let year = (day - 1) / yl + 1;
        let doy = (day - 1) % yl + 1;
        let mut acc = 0i64;
        let mut label = format!("доба {day}");
        for m in &self.months {
            if doy <= acc + m.days {
                let disp = m.genitive.as_deref().unwrap_or(&m.name);
                label = format!("{} {}", doy - acc, disp);
                break;
            }
            acc += m.days;
        }
        if year > 1 {
            label.push_str(&format!(", рік {year}"));
        }
        if !self.days_of_week.is_empty() {
            let dow = &self.days_of_week[((day - 1) % self.days_of_week.len() as i64) as usize];
            label.push_str(&format!(" — {dow}"));
        }
        label
    }
}

// ---------------------------------------------------------------------------
// Маркери часу
// ---------------------------------------------------------------------------

/// Канонічні часи доби.
pub const TOD_MORNING: &str = "ранок";
pub const TOD_DAY: &str = "день";
pub const TOD_EVENING: &str = "вечір";
pub const TOD_NIGHT: &str = "ніч";

/// Вираз часу, спарсений з маркера.
#[derive(Debug, Clone, PartialEq)]
pub enum TimeExpr {
    /// зсув на N діб («+3 днів», «через три дні», «за тиждень»)
    OffsetDays(i64),
    /// абсолютний день-у-році календаря («день 12», «12 жовтня»)
    AbsoluteDay(i64),
    /// та сама доба, можливо уточнений час доби («ввечері того ж дня», «на світанку», «пізніше»)
    SameDay { tod: Option<&'static str> },
    /// наступна доба («наступного ранку», "next morning")
    NextDay { tod: Option<&'static str> },
    /// маркер флешбеку — знімає алерт «стрибок назад»
    Flashback,
}

/// Маркер часу в тілі документа (байтові offsets тіла).
#[derive(Debug, Clone)]
pub struct TimeTag {
    pub start: usize,
    pub end: usize,
    /// оригінальний фрагмент тексту
    pub text: String,
    pub expr: TimeExpr,
}

/// Маркер локації `[loc: Хтось = Місце]` / `[loc: Хтось = Місце (шлях)]`.
#[derive(Debug, Clone)]
pub struct LocTag {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub entity: String,
    pub location: String,
    /// є анотація переміщення «(шлях ...)»
    pub moved: bool,
    pub path: Option<String>,
}

// --- нормалізація токенів: lower + уніфікація апострофів ---
fn norm(t: &str) -> String {
    t.to_lowercase().replace('\u{2019}', "'")
}

fn num_word(t: &str) -> Option<i64> {
    Some(match t {
        "один" | "одну" | "одна" | "одного" | "одному" | "одним" | "one" => 1,
        "два" | "дві" | "двох" | "двом" | "двома" | "two" => 2,
        "три" | "трьох" | "трьом" | "трьома" | "three" => 3,
        "чотири" | "чотирьох" | "чотирьом" | "four" => 4,
        "п'ять" | "п'яти" | "п'ятьома" | "five" => 5,
        "шість" | "шести" | "шістьма" | "six" => 6,
        "сім" | "семи" | "сімома" | "seven" => 7,
        "вісім" | "восьми" | "вісьмома" | "eight" => 8,
        "дев'ять" | "дев'яти" | "nine" => 9,
        "десять" | "десяти" | "ten" => 10,
        "одинадцять" | "eleven" => 11,
        "дванадцять" | "дванадцяти" | "twelve" => 12,
        _ => return t.parse::<i64>().ok(),
    })
}

/// Одиниця часу в добах.
fn unit_days(t: &str) -> Option<i64> {
    Some(match t {
        "день" | "дня" | "дні" | "днів" | "дню" | "day" | "days" => 1,
        "тиждень" | "тижня" | "тижнів" | "тижні" | "week" | "weeks" => 7,
        "місяць" | "місяця" | "місяці" | "місяців" | "month" | "months" => 30,
        "рік" | "року" | "роки" | "років" | "год" | "year" | "years" => 365,
        _ => return None,
    })
}

/// Час доби для складених фраз («наступного РАНКУ», «того ж ДНЯ»).
fn tod_word(t: &str) -> Option<&'static str> {
    Some(match t {
        "ранку" | "ранок" | "рання" | "раннім" | "ранній" | "morning" => TOD_MORNING,
        "дня" | "день" | "полудні" | "обіді" | "noon" | "midday" => TOD_DAY,
        "вечора" | "вечір" | "вечері" | "evening" | "dusk" => TOD_EVENING,
        "ночі" | "ніч" | "night" | "midnight" => TOD_NIGHT,
        _ => return None,
    })
}

/// Наріччя часу доби, яке в прозі однозначне (самостійний маркер).
fn tod_adverb(t: &str) -> Option<&'static str> {
    Some(match t {
        "вранці" | "уранці" => TOD_MORNING,
        "вдень" => TOD_DAY,
        "ввечері" | "увечері" | "надвечір" => TOD_EVENING,
        "уночі" | "вночі" | "опівночі" => TOD_NIGHT,
        _ => return None,
    })
}

/// Службовий час: «через годину», «через мить» — доба не змінюється.
fn short_time(t: &str) -> bool {
    matches!(
        t,
        "годину" | "год" | "години" | "хвилину" | "хвилі" | "мить" | "миту" | "секунду"
            | "мгновення" | "миттю" | "hour" | "minute" | "moment"
    )
}

/// ASCII-незалежний пошук тега `[key:` (шаблон чистий ASCII — порівняння по
/// байтах безпечне і не ламає кириличні offset-и; як у stats.rs).
fn find_tag(line: &str, key: &str, from: usize) -> Option<usize> {
    let pat = format!("[{key}:");
    let bytes = line.as_bytes();
    let pb = pat.as_bytes();
    let mut i = from;
    while i + pb.len() <= bytes.len() {
        if bytes[i..i + pb.len()]
            .iter()
            .zip(pb)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Спарсити специфікацію `[time: ...]` (григоріанський календар).
pub fn parse_time_spec(spec: &str) -> Option<TimeExpr> {
    parse_time_spec_in(spec, &default_gregorian())
}

/// Спарсити специфікацію `[time: ...]` у календарі світу `cal`.
pub fn parse_time_spec_in(spec: &str, cal: &Calendar) -> Option<TimeExpr> {
    let s = norm(spec.trim());
    if s == "flashback" || s == "флешбек" || s == "флешбеком" {
        return Some(TimeExpr::Flashback);
    }
    let toks: Vec<String> = s.split_whitespace().map(String::from).collect();
    if toks.is_empty() {
        return None;
    }
    // «+3 днів» / «+3» / «-2 днів»
    if let Some(rest) = toks[0].strip_prefix('+').or_else(|| toks[0].strip_prefix('-')) {
        let sign = if toks[0].starts_with('-') { -1 } else { 1 };
        if let Some(n) = num_word(rest) {
            let mult = toks.get(1).and_then(|u| unit_days(u)).unwrap_or(1);
            return Some(TimeExpr::OffsetDays(sign * n * mult));
        }
        return None;
    }
    // «день 12» / «day 12»
    if (toks[0] == "день" || toks[0] == "day") && toks.len() >= 2 {
        if let Some(n) = num_word(&toks[1]) {
            return Some(TimeExpr::AbsoluteDay(n));
        }
        return None;
    }
    // «12 жовтня» / «12 Тарвеня» / «жовтень 12» — явна дата в календарі світу
    let refs: Vec<&str> = toks.iter().map(String::as_str).collect();
    if let Some(mi) = refs.iter().position(|t| cal.month_index(t).is_some()) {
        if let Some(n) = refs
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != mi)
            .find_map(|(_, t)| num_word(t))
        {
            if let Some(doy) = cal.day_of_year(mi, n) {
                return Some(TimeExpr::AbsoluteDay(doy));
            }
        }
    }
    // природна фраза всередині тега
    natural_expr(&refs, 0, cal).map(|(_, e)| e)
}

struct Tok {
    norm: String,
    start: usize,
    end: usize,
}

/// Спроба зматчити природну фразу в токенах з позиції `i`:
/// (скільки токенів з'їдено, вираз). Явні дати парсяться у календарі `cal`.
fn natural_expr(toks: &[&str], i: usize, cal: &Calendar) -> Option<(usize, TimeExpr)> {
    let t0 = toks.get(i)?;
    // «наступного ранку» / «наступний ранок» / "next morning" / "the next day"
    if t0.starts_with("наступн") || *t0 == "next" || (*t0 == "the" && toks.get(i + 1).map(|t| *t == "next").unwrap_or(false)) {
        let mut j = i + if *t0 == "the" { 2 } else { 1 };
        if toks.get(j).map(|t| *t == "ж").unwrap_or(false) {
            j += 1; // «наступного ж ранку»
        }
        if let Some(t1) = toks.get(j) {
            // «наступного дня» — доба відома, час доби ні
            if *t1 == "день" || *t1 == "дня" || *t1 == "day" {
                return Some((j - i + 1, TimeExpr::NextDay { tod: None }));
            }
            if let Some(tod) = tod_word(t1) {
                return Some((j - i + 1, TimeExpr::NextDay { tod: Some(tod) }));
            }
        }
    }
    // «через три дні» / «за тиждень» / «через годину»
    if *t0 == "через" || *t0 == "за" {
        if let Some(t1) = toks.get(i + 1) {
            if let Some(n) = num_word(t1) {
                if let Some(t2) = toks.get(i + 2) {
                    if let Some(u) = unit_days(t2) {
                        return Some((3, TimeExpr::OffsetDays(n * u)));
                    }
                }
            } else if let Some(u) = unit_days(t1) {
                return Some((2, TimeExpr::OffsetDays(u)));
            } else if short_time(t1) {
                return Some((2, TimeExpr::SameDay { tod: None }));
            }
        }
    }
    // «три дні потому» / "three days later"
    if let Some(n) = num_word(t0) {
        if let Some(t1) = toks.get(i + 1) {
            if let Some(u) = unit_days(t1) {
                let next = toks.get(i + 2).copied();
                if matches!(next, Some("потому") | Some("згодом") | Some("later") | Some("пізніше")) {
                    return Some((3, TimeExpr::OffsetDays(n * u)));
                }
            }
            // явна дата «12 жовтня» (у календарі світу)
            if let Some(idx) = cal.month_index(t1) {
                if let Some(doy) = cal.day_of_year(idx, n) {
                    return Some((2, TimeExpr::AbsoluteDay(doy)));
                }
            }
        }
    }
    // "October 12" / «Тарвен 12»
    if cal.month_index(t0).is_some() {
        if let Some(t1) = toks.get(i + 1) {
            if let Some(d) = num_word(t1) {
                if let Some(idx) = cal.month_index(t0) {
                    if let Some(doy) = cal.day_of_year(idx, d) {
                        return Some((2, TimeExpr::AbsoluteDay(doy)));
                    }
                }
            }
        }
    }
    // «того ж дня» / «того самого дня» / "that same day"
    if *t0 == "того" {
        if let (Some(t1), Some(t2)) = (toks.get(i + 1), toks.get(i + 2)) {
            if (*t1 == "ж" || *t1 == "самого") && *t2 == "дня" {
                return Some((3, TimeExpr::SameDay { tod: None }));
            }
        }
    }
    if *t0 == "that" {
        if let (Some(t1), Some(t2)) = (toks.get(i + 1), toks.get(i + 2)) {
            if *t1 == "same" && *t2 == "day" {
                return Some((3, TimeExpr::SameDay { tod: None }));
            }
        }
        // "that evening" / "that night"
        if let Some(t1) = toks.get(i + 1) {
            if let Some(tod) = tod_word(t1) {
                return Some((2, TimeExpr::SameDay { tod: Some(tod) }));
            }
        }
    }
    // «ввечері того ж дня» / «ввечері того самого дня» / "that evening"
    if let Some(tod) = tod_adverb(t0) {
        if let (Some(t1), Some(t2), Some(t3)) = (toks.get(i + 1), toks.get(i + 2), toks.get(i + 3)) {
            if *t1 == "того" && (*t2 == "ж" || *t2 == "самого") && *t3 == "дня" {
                return Some((4, TimeExpr::SameDay { tod: Some(tod) }));
            }
        }
        return Some((1, TimeExpr::SameDay { tod: Some(tod) }));
    }
    // «на світанку» / «на розсвітку»
    if *t0 == "на" {
        if let Some(t1) = toks.get(i + 1) {
            if matches!(*t1, "світанку" | "розсвітку" | "світанні") {
                return Some((2, TimeExpr::SameDay { tod: Some(TOD_MORNING) }));
            }
        }
    }
    if *t0 == "по" {
        if let Some(t1) = toks.get(i + 1) {
            if *t1 == "обіді" {
                return Some((2, TimeExpr::SameDay { tod: Some(TOD_DAY) }));
            }
        }
    }
    if *t0 == "у" {
        if let Some(t1) = toks.get(i + 1) {
            if *t1 == "надвечір" {
                return Some((2, TimeExpr::SameDay { tod: Some(TOD_EVENING) }));
            }
        }
    }
    // "at dawn" / "at night" / "in the morning"
    if *t0 == "at" {
        if let Some(t1) = toks.get(i + 1) {
            let tod = match *t1 {
                "dawn" | "sunrise" => Some(TOD_MORNING),
                "night" | "midnight" => Some(TOD_NIGHT),
                "noon" | "midday" => Some(TOD_DAY),
                "dusk" | "evening" => Some(TOD_EVENING),
                _ => None,
            };
            if let Some(tod) = tod {
                return Some((2, TimeExpr::SameDay { tod: Some(tod) }));
            }
        }
    }
    if *t0 == "in" {
        if let (Some(t1), Some(t2)) = (toks.get(i + 1), toks.get(i + 2)) {
            if *t1 == "the" && *t2 == "morning" {
                return Some((3, TimeExpr::SameDay { tod: Some(TOD_MORNING) }));
            }
        }
    }
    // «пізніше» / "later"
    if *t0 == "пізніше" || *t0 == "later" || *t0 == "згодом" {
        return Some((1, TimeExpr::SameDay { tod: None }));
    }
    None
}

/// Спарсити всі часові маркери з тіла документа (inline `[time: ...]`
/// мають пріоритет; природні фрази шукаються на токенах поза тегами).
/// Явні дати — у григоріанському календарі.
pub fn parse_time_markers(body: &str) -> Vec<TimeTag> {
    parse_time_markers_in(body, &default_gregorian())
}

/// Те саме, але явні дати («12 жовтня», «12 Тарвеня») парсяться
/// у календарі світу `cal`.
pub fn parse_time_markers_in(body: &str, cal: &Calendar) -> Vec<TimeTag> {
    let mut tags: Vec<TimeTag> = Vec::new();
    let mut used: Vec<(usize, usize)> = Vec::new();

    // 1) inline теги
    for line_start in line_starts(body) {
        let line = &body[line_start..line_end(body, line_start)];
        let mut from = 0usize;
        while let Some(tag) = find_tag(line, "time", from) {
            let start = line_start + tag + "[time:".len();
            let Some(end_rel) = line[tag + "[time:".len()..].find(']') else {
                break;
            };
            let end = start + end_rel;
            from = tag + "[time:".len() + end_rel + 1;
            used.push((line_start + tag, end + 1));
            if let Some(expr) =
                parse_time_spec_in(&line[start - line_start..start - line_start + end_rel], cal)
            {
                tags.push(TimeTag {
                    start: line_start + tag,
                    end: end + 1,
                    text: line[tag..tag + "[time:".len() + end_rel + 1].to_string(),
                    expr,
                });
            }
        }
    }

    // 2) природні фрази (поза inline-тегами)
    let toks: Vec<Tok> = crate::indexer::tokenize(body)
        .into_iter()
        .map(|t| Tok {
            norm: norm(&t.text),
            start: t.start,
            end: t.end,
        })
        .collect();
    let refs: Vec<&str> = toks.iter().map(|t| t.norm.as_str()).collect();
    let mut i = 0usize;
    while i < toks.len() {
        let overlaps = |s: usize, e: usize| used.iter().any(|(us, ue)| s < *ue && *us < e);
        if overlaps(toks[i].start, toks[i].end) {
            i += 1;
            continue;
        }
        if let Some((len, expr)) = natural_expr(&refs, i, cal) {
            let last = &toks[i + len - 1];
            if !overlaps(toks[i].start, last.end) {
                tags.push(TimeTag {
                    start: toks[i].start,
                    end: last.end,
                    text: body[toks[i].start..last.end].to_string(),
                    expr,
                });
                i += len;
                continue;
            }
        }
        i += 1;
    }
    tags.sort_by_key(|t| t.start);
    tags
}

fn line_starts(body: &str) -> Vec<usize> {
    let mut out = vec![0usize];
    for (i, b) in body.bytes().enumerate() {
        if b == b'\n' {
            out.push(i + 1);
        }
    }
    out
}

fn line_end(body: &str, start: usize) -> usize {
    body[start..]
        .find('\n')
        .map(|i| start + i)
        .unwrap_or(body.len())
}

/// Спарсити локаційні маркери `[loc: Хтось = Місце]` / `...(шлях)`.
pub fn parse_loc_markers(body: &str) -> Vec<LocTag> {
    let mut out = Vec::new();
    for line_start in line_starts(body) {
        let line = &body[line_start..line_end(body, line_start)];
        let mut from = 0usize;
        while let Some(tag) = find_tag(line, "loc", from) {
            let inner_start = tag + "[loc:".len();
            let Some(end_rel) = line[inner_start..].find(']') else {
                break;
            };
            let end = inner_start + end_rel;
            from = end + 1;
            let full = &line[tag..end + 1];
            let content = line[inner_start..end].trim();
            let Some((ent_raw, loc_raw)) = content.split_once('=') else {
                continue;
            };
            let entity = ent_raw.trim();
            if entity.is_empty() {
                continue;
            }
            let loc_raw = loc_raw.trim();
            let (location, path, moved) = match loc_raw.find('(') {
                Some(pi) => {
                    let close = loc_raw[pi..].find(')').map(|c| pi + c).unwrap_or(loc_raw.len());
                    let path = loc_raw[pi + 1..close].trim();
                    let loc = loc_raw[..pi].trim();
                    if loc.is_empty() {
                        continue;
                    }
                    (
                        loc.to_string(),
                        if path.is_empty() { None } else { Some(path.to_string()) },
                        true,
                    )
                }
                None => {
                    if loc_raw.is_empty() {
                        continue;
                    }
                    (loc_raw.to_string(), None, false)
                }
            };
            out.push(LocTag {
                start: line_start + tag,
                end: line_start + end + 1,
                text: full.to_string(),
                entity: entity.to_string(),
                location,
                moved,
                path,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Перебудова таймлайну (ідемпотентна)
// ---------------------------------------------------------------------------

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS timeline_points (
  doc_path TEXT NOT NULL,
  ord INTEGER NOT NULL,
  day INTEGER NOT NULL,
  time_of_day TEXT NOT NULL DEFAULT '',
  marker_text TEXT NOT NULL DEFAULT '',
  char_start INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_timeline_points_doc ON timeline_points(doc_path);
CREATE TABLE IF NOT EXISTS timeline_locs (
  doc_path TEXT NOT NULL,
  ord INTEGER NOT NULL,
  day INTEGER NOT NULL,
  entity TEXT NOT NULL,
  location TEXT NOT NULL,
  moved INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_timeline_locs_entity ON timeline_locs(entity);
";

/// Точка таймлайну (рядок таблиці).
#[derive(Debug, Clone)]
pub struct TimelinePoint {
    pub doc_path: String,
    pub ord: i64,
    pub day: i64,
    pub time_of_day: String,
    pub marker_text: String,
    pub char_start: i64,
}

/// Рядок локаційного календаря.
#[derive(Debug, Clone)]
pub struct LocRow {
    pub doc_path: String,
    pub ord: i64,
    pub day: i64,
    pub entity: String,
    pub location: String,
    pub moved: bool,
}

/// Звіт перебудови.
#[derive(Debug, Default)]
pub struct RebuildReport {
    pub chapters: usize,
    pub points: usize,
    pub locs: usize,
    pub backwards: usize,
    pub loc_conflicts: usize,
    pub missing: usize,
}

/// Номер глави з імені файла: ведучі цифри («03-розділ.md» → 3).
fn chapter_from_path(rel: &str) -> Option<i64> {
    let stem = rel.rsplit('/').next()?;
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

struct St {
    day: i64,
    tod: Option<String>,
    flash: bool,
}

/// Перебудувати timeline_points/timeline_locs з manuscript/ і прогнати
/// валідатори (алерти `timeline-backwards`, `location-conflict`). Ідемпотентно:
/// повторний прогін без правок тексту алертів не плодить.
pub fn rebuild(store: &ProjectStore) -> Result<RebuildReport> {
    let conn = store.conn();
    conn.execute_batch(SCHEMA)?;
    let cal = load_calendar(&store.root)?;
    let yl = cal.year_length().max(1);
    let mut report = RebuildReport::default();

    conn.execute("DELETE FROM timeline_points", [])?;
    conn.execute("DELETE FROM timeline_locs", [])?;
    conn.execute(
        "DELETE FROM alerts WHERE kind IN ('timeline-backwards','location-conflict') AND resolved=0",
        [],
    )?;

    let mut stmt = conn.prepare("SELECT path FROM docs WHERE kind='manuscript' ORDER BY path")?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut state = St { day: 1, tod: None, flash: false };
    let mut missing: Vec<(String, i64)> = Vec::new();
    let mut all_locs: Vec<LocRow> = Vec::new();

    for (i, rel) in paths.iter().enumerate() {
        let ord = chapter_from_path(rel).unwrap_or((i + 1) as i64);
        let raw = std::fs::read_to_string(store.root.join(rel))?;
        let parsed = frontmatter::parse(&raw);
        let body = parsed.body;
        let day_start = parsed
            .meta
            .get("day_start")
            .and_then(|v| v.trim().parse::<i64>().ok());
        let start_day = day_start.unwrap_or(state.day);
        report.chapters += 1;
        // явний day_start нижче кінцевої доби попередньої глави без флешбеку — теж стрибок назад
        if let Some(ds) = day_start {
            if ds < state.day && !state.flash {
                insert_alert(
                    conn,
                    "timeline-backwards",
                    &serde_json::json!({
                        "doc": rel,
                        "ord": ord,
                        "from": state.day,
                        "to": ds,
                        "marker": format!("day_start: {ds}"),
                        "char_start": 0,
                        "message": format!(
                            "Стрибок назад у часі: доба {} → {ds} у «{rel}» (frontmatter day_start) без маркера флешбеку",
                            state.day
                        ),
                    }),
                )?;
                report.backwards += 1;
            }
        }

        let mut rows: Vec<(i64, String, String, usize)> =
            vec![(start_day, String::new(), String::new(), 0)];
        let mut prev = start_day;
        let mut flash = state.flash;
        let mut tod: Option<String> = state.tod.clone();
        let mut marked = false;

        // об'єднаний список подій: час + локації, за offsetами
        let mut time_tags = parse_time_markers_in(&body, &cal);
        let loc_tags = parse_loc_markers(&body);
        time_tags.sort_by_key(|t| t.start);
        let mut events: Vec<(usize, bool, usize)> = time_tags
            .iter()
            .enumerate()
            .map(|(j, t)| (t.start, true, j))
            .chain(loc_tags.iter().enumerate().map(|(j, l)| (l.start, false, j)))
            .collect();
        events.sort_by_key(|(s, is_time, j)| (*s, !*is_time, *j));

        for &(_start, is_time, j) in &events {
            if is_time {
                let tag = &time_tags[j];
                let day_before = prev;
                match &tag.expr {
                    TimeExpr::OffsetDays(d) => prev += d,
                    TimeExpr::AbsoluteDay(doy) => {
                        let year_idx = (prev - 1).div_euclid(yl);
                        prev = year_idx * yl + doy;
                    }
                    TimeExpr::SameDay { tod: t } => {
                        if let Some(t) = t {
                            tod = Some(t.to_string());
                        }
                    }
                    TimeExpr::NextDay { tod: t } => {
                        prev += 1;
                        tod = t.map(|t| t.to_string());
                    }
                    TimeExpr::Flashback => flash = true,
                }
                if prev < day_before && !flash {
                    insert_alert(
                        conn,
                        "timeline-backwards",
                        &serde_json::json!({
                            "doc": rel,
                            "ord": ord,
                            "from": day_before,
                            "to": prev,
                            "marker": tag.text,
                            "char_start": tag.start,
                            "message": format!(
                                "Стрибок назад у часі: доба {day_before} → {prev} у «{rel}» ({}) без маркера флешбеку",
                                tag.text
                            ),
                        }),
                    )?;
                    report.backwards += 1;
                }
                if prev > day_before {
                    flash = false;
                }
                rows.push((prev, tod.clone().unwrap_or_default(), tag.text.clone(), tag.start));
                marked = true;
            } else {
                let tag = &loc_tags[j];
                all_locs.push(LocRow {
                    doc_path: rel.clone(),
                    ord,
                    day: prev,
                    entity: tag.entity.clone(),
                    location: tag.location.clone(),
                    moved: tag.moved,
                });
            }
        }

        for (day, t, marker, cs) in &rows {
            conn.execute(
                "INSERT INTO timeline_points(doc_path, ord, day, time_of_day, marker_text, char_start)
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![rel, ord, day, t, marker, *cs as i64],
            )?;
        }
        report.points += rows.len();

        if !marked {
            missing.push((rel.clone(), ord));
        }
        state = St { day: prev, tod, flash };
    }

    // локаційні рядки
    for l in &all_locs {
        conn.execute(
            "INSERT INTO timeline_locs(doc_path, ord, day, entity, location, moved)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![l.doc_path, l.ord, l.day, l.entity, l.location, l.moved as i64],
        )?;
    }
    report.locs = all_locs.len();

    // конфлікти локацій: сутність у двох місцях в ОДНУ добу без переміщення
    let mut groups: BTreeMap<(String, i64), Vec<&LocRow>> = BTreeMap::new();
    let mut display: BTreeMap<String, String> = BTreeMap::new();
    for l in &all_locs {
        groups
            .entry((l.entity.to_lowercase(), l.day))
            .or_default()
            .push(l);
        display
            .entry(l.entity.to_lowercase())
            .or_insert_with(|| l.entity.clone());
    }
    for ((ek, day), group) in groups {
        let mut locs: Vec<String> = group.iter().map(|l| l.location.clone()).collect();
        locs.sort();
        locs.dedup();
        if locs.len() < 2 || group.iter().any(|l| l.moved) {
            continue;
        }
        let e_disp = display.get(&ek).cloned().unwrap_or(ek);
        let docs: Vec<String> = {
            let mut d: Vec<String> = group.iter().map(|l| l.doc_path.clone()).collect();
            d.sort();
            d.dedup();
            d
        };
        insert_alert(
            conn,
            "location-conflict",
            &serde_json::json!({
                "entity": e_disp,
                "day": day,
                "locations": locs,
                "docs": docs,
                "message": format!(
                    "Конфлікт локації: «{}» у добу {} водночас у: {} (без маркера переміщення)",
                    e_disp,
                    day,
                    locs.join(", ")
                ),
            }),
        )?;
        report.loc_conflicts += 1;
    }

    report.missing = missing.len();
    Ok(report)
}

fn insert_alert(conn: &rusqlite::Connection, kind: &str, payload: &serde_json::Value) -> Result<()> {
    conn.execute(
        "INSERT INTO alerts(kind, entity_id, payload_json, created_at)
         VALUES (?1, NULL, ?2, strftime('%s','now'))",
        params![kind, serde_json::to_string(payload)?],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Читання / валідація
// ---------------------------------------------------------------------------

/// Усі точки таймлайну (по главах, в межах глави — за offsetом).
pub fn points(store: &ProjectStore) -> Result<Vec<TimelinePoint>> {
    store.conn().execute_batch(SCHEMA)?;
    let mut stmt = store.conn().prepare(
        "SELECT doc_path, ord, day, time_of_day, marker_text, char_start
         FROM timeline_points ORDER BY ord, char_start, rowid",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(TimelinePoint {
                doc_path: r.get(0)?,
                ord: r.get(1)?,
                day: r.get(2)?,
                time_of_day: r.get(3)?,
                marker_text: r.get(4)?,
                char_start: r.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Локаційний календар: хто де по главах.
pub fn locs(store: &ProjectStore) -> Result<Vec<LocRow>> {
    store.conn().execute_batch(SCHEMA)?;
    let mut stmt = store.conn().prepare(
        "SELECT doc_path, ord, day, entity, location, moved
         FROM timeline_locs ORDER BY ord, rowid",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(LocRow {
                doc_path: r.get(0)?,
                ord: r.get(1)?,
                day: r.get(2)?,
                entity: r.get(3)?,
                location: r.get(4)?,
                moved: r.get::<_, i64>(5)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Алерти стрибків назад: (alert_id, payload_json).
pub fn backwards_alerts(store: &ProjectStore) -> Result<Vec<(i64, String)>> {
    alerts_of(store, "timeline-backwards")
}

/// Алерти конфліктів локацій: (alert_id, payload_json).
pub fn conflict_alerts(store: &ProjectStore) -> Result<Vec<(i64, String)>> {
    alerts_of(store, "location-conflict")
}

fn alerts_of(store: &ProjectStore, kind: &str) -> Result<Vec<(i64, String)>> {
    let mut stmt = store.conn().prepare(
        "SELECT id, payload_json FROM alerts WHERE kind=?1 AND resolved=0 ORDER BY id",
    )?;
    let rows = stmt
        .query_map(params![kind], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Звіт валідації.
#[derive(Debug, Default)]
pub struct ValidationReport {
    /// алерти «стрибок назад» (id, payload)
    pub backwards: Vec<(i64, serde_json::Value)>,
    /// конфлікти локацій (id, payload)
    pub conflicts: Vec<(i64, serde_json::Value)>,
    /// ПОПЕРЕДЖЕННЯ: глави без жодного time-маркера (path, ord) — не алерт
    pub missing: Vec<(String, i64)>,
    pub chapters: usize,
}

impl ValidationReport {
    pub fn is_clean(&self) -> bool {
        self.backwards.is_empty() && self.conflicts.is_empty() && self.missing.is_empty()
    }
}

/// Валідація таймлайну: перебудовує індекс і повертає звіт
/// (алерти назад/конфлікти — з БД; пропущені маркери — попередження).
pub fn validate(store: &ProjectStore) -> Result<ValidationReport> {
    let rep = rebuild(store)?;
    let mut out = ValidationReport {
        missing: Vec::new(),
        chapters: rep.chapters,
        ..Default::default()
    };
    // missing із звіту: перечитуємо з таблиці (глави без маркерів)
    let mut stmt = store.conn().prepare(
        "SELECT doc_path, MIN(ord) FROM timeline_points
         GROUP BY doc_path
         HAVING COUNT(*) = 1 AND MAX(CASE WHEN marker_text = '' THEN 0 ELSE 1 END) = 0",
    )?;
    out.missing = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    for (_, payload) in backwards_alerts(store)? {
        out.backwards.push((0, serde_json::from_str(&payload)?));
    }
    for (_, payload) in conflict_alerts(store)? {
        out.conflicts.push((0, serde_json::from_str(&payload)?));
    }
    Ok(out)
}
