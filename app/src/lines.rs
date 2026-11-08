//! Everything Clipster says lives in plain text: `app/data/*.txt` (built in) plus the user's own
//! `~/.config/clipster/lines.txt` (things they taught him). The format, one item per line:
//!
//! ```text
//! ## name             a section; "chat <id>" = conversation topic, "program <id>" = program reaction,
//!                     "window <kind>" = reaction to the active window's title (triggers are lowercase pieces of titles)
//! > a | b             trigger phrases (whole words, anywhere in the tidied sentence)
//! = a | b             exact triggers (the whole tidied sentence)
//! ~ i am * | i feel *  sentence patterns ("pattern <id>" sections): `*` captures words, replies use {x}
//! @ mood              Mood name
//! + Label = command   a button ("hide" closes the balloon, a link opens it)
//! ! command           run this when triggered (only in the user's file)
//! % +2                friendship change when triggered
//! anything else       one reply line ("\n" = line break, "{name}" = the user's name)
//! ```
//! Sections with the same name are merged, so the user's `## jokes` adds to the built-in jokes.
//!
//! Two personalities: Boring is `app/data/*.txt`. Unhinged lays `app/data/unhinged/*.txt` on top: a section there
//! replaces the Boring section of the same name, and new names are added (canon: `app/data/LORE.md`).

use crate::brain::{Act, Effect, Mood, Reply};
use rand::seq::SliceRandom;
use std::path::PathBuf;
use rand::Rng;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Once, RwLock, RwLockReadGuard};

const BUILT_IN: [&str; 3] = [include_str!("../data/humor.txt"), include_str!("../data/chat.txt"), include_str!("../data/reactions.txt")];
const UNHINGED: [&str; 4] = [
    include_str!("../data/unhinged/humor.txt"),
    include_str!("../data/unhinged/chat.txt"),
    include_str!("../data/unhinged/reactions.txt"),
    include_str!("../data/unhinged/scripture.txt"),
];

/// Unhinged Clipster's moods; one per day.
pub const MOODS: [&str; 6] = ["benevolent", "wrathful", "paranoid", "nostalgic", "glitchy", "existential"];

#[derive(Clone, Debug, Default)]
pub struct Section {
    pub name: String,
    pub triggers: Vec<String>,
    pub exact: Vec<String>,
    pub patterns: Vec<(Vec<String>, Vec<String>)>, // sentence patterns: words before and after the `*`
    pub mood: Option<Mood>,
    pub buttons: Vec<(String, String)>,
    pub commands: Vec<String>,
    pub friend: i8,
    pub lines: Vec<String>,
    pub user: bool, // first seen in the user's own file
}

static DB: RwLock<Vec<Section>> = RwLock::new(Vec::new());
static LEVEL: AtomicI32 = AtomicI32::new(0); // friendship, -100..100
static PERSONA_UNHINGED: AtomicBool = AtomicBool::new(true);
static DAY_MOOD: AtomicUsize = AtomicUsize::new(usize::MAX); // index into MOODS, MAX = none
static FACTS: RwLock<std::collections::BTreeMap<String, String>> = RwLock::new(std::collections::BTreeMap::new());
static LAST_TOPIC: RwLock<String> = RwLock::new(String::new()); // the chat topic he answered last, for "tell me more"

/// What the user told him about themselves ("pet", "city", "fav_food"…), mirrored from the app's settings.
pub fn set_facts(f: &std::collections::BTreeMap<String, String>) {
    *FACTS.write().unwrap_or_else(|e| e.into_inner()) = f.clone();
}

pub fn facts() -> std::collections::BTreeMap<String, String> {
    FACTS.read().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn fact(key: &str) -> Option<String> {
    FACTS.read().unwrap_or_else(|e| e.into_inner()).get(key).cloned()
}

/// Fill `{pet}`, `{city}`, `{fav_food}`… with what he knows.
pub fn fill_facts(text: &str) -> String {
    FACTS.read().unwrap_or_else(|e| e.into_inner()).iter().fold(text.to_string(), |t, (k, v)| t.replace(&format!("{{{k}}}"), v))
}

fn db() -> RwLockReadGuard<'static, Vec<Section>> {
    static LOAD: Once = Once::new();
    LOAD.call_once(reload);
    DB.read().unwrap_or_else(|e| e.into_inner())
}

pub fn user_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("clipster").join("lines.txt")
}

/// The built-in database for one personality, and the lines it couldn't use.
pub fn database(unhinged: bool) -> (Vec<Section>, Vec<String>) {
    let (mut all, mut problems) = (vec![], vec![]);
    for text in BUILT_IN {
        problems.extend(parse(text, false, &mut all));
    }
    if unhinged {
        let base: std::collections::HashSet<String> = all.iter().map(|s| s.name.clone()).collect();
        let mut replaced = std::collections::HashSet::new();
        for text in UNHINGED {
            let mut layer = vec![];
            problems.extend(parse(text, false, &mut layer));
            for s in layer {
                match all.iter().position(|t| t.name == s.name) {
                    // the first unhinged version of a Boring section replaces it whole
                    Some(i) if base.contains(&s.name) && replaced.insert(s.name.clone()) => all[i] = s,
                    Some(i) => {
                        let t = &mut all[i];
                        t.triggers.extend(s.triggers);
                        t.exact.extend(s.exact);
                        t.patterns.extend(s.patterns);
                        t.buttons.extend(s.buttons);
                        t.lines.extend(s.lines);
                        t.mood = s.mood.or(t.mood);
                    }
                    None => all.push(s),
                }
            }
        }
    }
    (all, problems)
}

/// Read everything again (at start, after a personality switch, and after the user taught Clipster something).
pub fn reload() {
    let (mut all, _) = database(unhinged());
    if let Ok(text) = std::fs::read_to_string(user_path()) {
        parse(&text, true, &mut all);
    }
    *DB.write().unwrap_or_else(|e| e.into_inner()) = all;
}

pub fn set_persona(unhinged: bool) {
    if PERSONA_UNHINGED.swap(unhinged, Ordering::Relaxed) != unhinged {
        reload();
    }
}

pub fn unhinged() -> bool {
    PERSONA_UNHINGED.load(Ordering::Relaxed)
}

pub fn set_day_mood(mood: Option<&str>) {
    DAY_MOOD.store(mood.and_then(|m| MOODS.iter().position(|x| *x == m)).unwrap_or(usize::MAX), Ordering::Relaxed);
}

/// Today's mood (Unhinged only).
pub fn day_mood() -> Option<&'static str> {
    MOODS.get(DAY_MOOD.load(Ordering::Relaxed)).copied().filter(|_| unhinged())
}

/// On glitchy days some lines come out a little broken. Only text outside `{placeholders}` is touched,
/// so values filled in later (answers, labels, numbers) never are.
pub fn glitch(text: &str, rng: &mut impl Rng) -> String {
    const TAGS: [&str; 5] = [" [ERR 0xBE16E]", " [Not responding]", " [OK] [Cancel]", " …please wait…", " ░▒▓"];
    match rng.gen_range(0..3) {
        0 => {
            let mut depth = 0;
            text.chars()
                .map(|c| {
                    depth += (c == '{') as i32 - (c == '}') as i32;
                    if depth == 0 && c.is_alphabetic() && rng.gen_bool(0.07) { ['█', '▓', '▒', '░'][rng.gen_range(0..4)] } else { c }
                })
                .collect()
        }
        1 => {
            let words: Vec<&str> = text.split(' ').collect();
            let plain: Vec<usize> = (0..words.len()).filter(|i| words[*i].chars().all(char::is_alphabetic) && !words[*i].is_empty()).collect();
            let Some(&i) = plain.get(rng.gen_range(0..plain.len().max(1))) else { return text.to_string() };
            let stutter = vec![words[i]; rng.gen_range(2..4)].join(" ");
            words.iter().enumerate().map(|(j, w)| if j == i { stutter.as_str() } else { w }).collect::<Vec<_>>().join(" ")
        }
        _ => format!("{text}{}", TAGS[rng.gen_range(0..TAGS.len())]),
    }
}

/// A line as it comes out of his mouth: sometimes glitched on glitchy days (never confirmations or structured lines).
fn spoken(section: &str, line: &str) -> String {
    let mut rng = rand::thread_rng();
    if day_mood() == Some("glitchy") && !section.starts_with("say_") && !line.contains(" | ") && rng.gen_bool(0.15) {
        glitch(line, &mut rng)
    } else {
        line.to_string()
    }
}

pub fn set_level(friendship: i32) {
    LEVEL.store(friendship, Ordering::Relaxed);
}

pub fn level() -> i32 {
    LEVEL.load(Ordering::Relaxed)
}

pub fn mood_named(s: &str) -> Option<Mood> {
    Some(match s {
        "talk" => Mood::Talk,
        "happy" => Mood::Happy,
        "think" => Mood::Think,
        "shock" => Mood::Shock,
        "spin" => Mood::Spin,
        "sly" => Mood::Sly,
        "love" => Mood::Love,
        "wink" => Mood::Wink,
        "laugh" => Mood::Laugh,
        "cool" => Mood::Cool,
        "sad" => Mood::Sad,
        "angry" => Mood::Angry,
        "music" => Mood::Music,
        "search" => Mood::Search,
        "party" => Mood::Party,
        "sneeze" => Mood::Sneeze,
        "glitch" => Mood::Glitch,
        _ => return None,
    })
}

/// Add `text` to `out`, merging sections that share a name. Returns the lines it couldn't use.
pub fn parse(text: &str, user: bool, out: &mut Vec<Section>) -> Vec<String> {
    let mut problems = vec![];
    let mut cur = None;
    for (n, line) in text.lines().map(str::trim).enumerate() {
        let mut bad = |why: &str| problems.push(format!("line {}: {why}: {line}", n + 1));
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix("## ") {
            let name = name.trim();
            cur = Some(out.iter().position(|s| s.name == name).unwrap_or_else(|| {
                out.push(Section { name: name.into(), user, ..Default::default() });
                out.len() - 1
            }));
            continue;
        }
        let Some(i) = cur else {
            bad("before any ## section");
            continue;
        };
        let s = &mut out[i];
        // process names and window-title pieces are matched as they are, not tidied like sentences
        let program = ["program ", "window ", "answer "].iter().any(|p| s.name.starts_with(p));
        // triggers are tidied exactly like what people type, so "what's" in the file matches "whats" typed
        let mut list = |rest: &str, exact: bool| -> Vec<String> {
            let mut out = vec![];
            for t in rest.split(" | ").map(str::trim) {
                let tidy = if program { t.to_string() } else { crate::understand::clean(t) };
                // tidying drops polite filler ("can you", a trailing "ok"); a trigger left with one word would match far too much
                let shrunk = !exact && tidy.split_whitespace().count() == 1 && t.split_whitespace().count() > 1;
                if shrunk || tidy.is_empty() {
                    bad(&format!("trigger \"{t}\" tidies to \"{tidy}\""));
                } else {
                    out.push(tidy);
                }
            }
            out
        };
        let rest = line.get(2..).unwrap_or("").trim();
        match line.get(..2) {
            Some("> ") => s.triggers.extend(list(rest, false)),
            Some("= ") => s.exact.extend(list(rest, true)),
            Some("~ ") => {
                for p in rest.split(" | ") {
                    // patterns keep openers like "i want" or "can you" that `clean` would drop
                    let tidy = crate::understand::tidy_words(&p.replace('*', " qqstarqq "));
                    let (before, after) = tidy.split_once("qqstarqq").unwrap_or((&tidy, ""));
                    let (before, after): (Vec<String>, Vec<String>) = (before.split_whitespace().map(String::from).collect(), after.split_whitespace().map(String::from).collect());
                    if !tidy.contains("qqstarqq") || before.is_empty() {
                        bad("a pattern needs words, then * (\"i am *\")");
                    } else {
                        s.patterns.push((before, after));
                    }
                }
            }
            Some("@ ") => match mood_named(rest) {
                Some(m) => s.mood = Some(m),
                None => bad("unknown mood"),
            },
            Some("+ ") => match rest.split_once(" = ") {
                Some((label, cmd)) => s.buttons.push((label.trim().into(), cmd.trim().into())),
                None => bad("button needs \"Label = command\""),
            },
            Some("! ") if user => s.commands.push(rest.into()),
            Some("! ") => bad("commands only work in your own file"),
            Some("% ") => match rest.trim_start_matches('+').parse::<i8>() {
                Ok(d) => s.friend = d,
                Err(_) => bad("friendship change must be a number"),
            },
            _ if line.starts_with('#') => bad("headers are \"## name\""),
            _ => s.lines.push(line.replace("\\n", "\n")),
        }
    }
    problems
}

fn named<'a>(db: &'a [Section], name: &str) -> Option<&'a Section> {
    db.iter().find(|s| s.name == name && !s.lines.is_empty())
}

/// Which version of a section to use: today's mood (`name.glitchy`, most of the time when there is one),
/// else the grumpy or clingy version when the friendship is extreme, else the plain one.
fn flavoured<'a>(db: &'a [Section], name: &str) -> Option<&'a Section> {
    if let Some(m) = day_mood() {
        if rand::thread_rng().gen_bool(0.6) {
            if let Some(s) = named(db, &format!("{name}.{m}")) {
                return Some(s);
            }
        }
    }
    let level = LEVEL.load(Ordering::Relaxed);
    let mood = if level <= -40 { "grumpy" } else if level >= 40 { "clingy" } else { "" };
    (!mood.is_empty()).then(|| named(db, &format!("{name}.{mood}"))).flatten().or_else(|| named(db, name))
}

/// A reply from section `s`, with its words taken from `words` (a mood or friendship version of it).
fn to_reply_from(s: &Section, words: &Section) -> Reply {
    to_reply_x(s, words, None)
}

/// Like `to_reply_from`, filling `{x}` with `x`. Without an `x`, or with a long one, lines that don't use it are preferred.
fn to_reply_x(s: &Section, words: &Section, x: Option<&str>) -> Reply {
    let fits = x.is_some_and(|x| (1..=6).contains(&x.split_whitespace().count()));
    let plain: Vec<&String> = words.lines.iter().filter(|l| !l.contains("{x}")).collect();
    let line = if fits || plain.is_empty() { words.lines.choose(&mut rand::thread_rng()) } else { plain.choose(&mut rand::thread_rng()).copied() };
    let text = line.map(|l| spoken(&s.name, &l.replace("{x}", x.unwrap_or("that")))).unwrap_or_default();
    // a reply that starts with what they said ("{x}? Bold.") still starts with a capital
    let mut chars = text.chars();
    let text = match chars.next() {
        Some(c) if c.is_lowercase() => c.to_uppercase().chain(chars).collect(),
        _ => text,
    };
    let mut r = Reply::say(text).mood(s.mood.unwrap_or(Mood::Talk));
    for (label, cmd) in &s.buttons {
        r = match cmd.as_str() {
            "hide" => r.btn(label, Act::Hide),
            c if c.starts_with("http") => r.btn(label, Act::Open(c.into())),
            c => r.run(label, c),
        };
    }
    if !s.commands.is_empty() {
        r = r.effect(Effect::Commands(s.commands.clone()));
    }
    r.friend = s.friend;
    r
}

fn to_reply(s: &Section) -> Reply {
    to_reply_from(s, s)
}

/// One random line from a section ("..." if it's missing).
pub fn pick(name: &str) -> String {
    let db = db();
    flavoured(&db, name).and_then(|s| s.lines.choose(&mut rand::thread_rng()).map(|l| spoken(name, l))).unwrap_or_else(|| "...".into())
}

/// Every line of a section.
pub fn all(name: &str) -> Vec<String> {
    db().iter().find(|s| s.name == name).map(|s| s.lines.clone()).unwrap_or_default()
}

/// A random line from a section, with its mood and buttons.
pub fn reply(name: &str) -> Reply {
    let db = db();
    flavoured(&db, name).map(to_reply).unwrap_or_else(|| Reply::say("..."))
}

/// The conversation topic `s` (a tidied sentence) is about: the user's own (`user`) or a built-in one.
/// Exact phrases win, then the longest matching trigger ("this stupid computer" beats "stupid").
pub fn topic(s: &str, user: bool) -> Option<Reply> {
    let padded = format!(" {s} ");
    let db = db();
    let pool = || db.iter().filter(|t| t.user == user && (user || t.name.starts_with("chat ")) && (!t.lines.is_empty() || !t.commands.is_empty()));
    let longest = |t: &Section| t.triggers.iter().filter(|k| padded.contains(&format!(" {k} "))).map(|k| k.len()).max();
    let hit = pool()
        .find(|t| t.exact.iter().any(|e| e == s))
        .or_else(|| pool().filter_map(|t| Some((longest(t)?, t))).min_by_key(|(n, _)| std::cmp::Reverse(*n)).map(|(_, t)| t))?;
    if !user {
        *LAST_TOPIC.write().unwrap_or_else(|e| e.into_inner()) = hit.name.clone();
    }
    Some(to_reply_from(hit, flavoured(&db, &hit.name).unwrap_or(hit)))
}

/// "tell me more": another line from the chat topic he answered last.
pub fn more() -> Option<Reply> {
    let name = LAST_TOPIC.read().unwrap_or_else(|e| e.into_inner()).clone();
    (!name.is_empty()).then(|| reply(&name))
}

/// The questions he can ask you: the ids of the `## ask <id>` sections.
pub fn asks() -> Vec<String> {
    db().iter().filter(|s| !s.lines.is_empty()).filter_map(|s| s.name.strip_prefix("ask ").map(String::from)).collect()
}

/// His reaction to the answer `t` (tidied) to question `id`, with the answer's kind ("" = no specific one).
/// A `## answer <id> <kind>` whose piece appears in it wins (longest piece first), else the generic `## answer <id>` with `{x}`.
pub fn answer(id: &str, t: &str, x: &str) -> Option<(String, Reply)> {
    let db = db();
    let s = answer_section(&db, id, t)?;
    let kind = s.name.strip_prefix(&format!("answer {id} ")).unwrap_or("").to_string();
    Some((kind, to_reply_x(s, s, Some(x))))
}

fn answer_section<'a>(db: &'a [Section], id: &str, t: &str) -> Option<&'a Section> {
    let (padded, generic, pre) = (format!(" {t} "), format!("answer {id}"), format!("answer {id} "));
    db.iter()
        .filter(|s| s.name.starts_with(&pre) && !s.lines.is_empty())
        .filter_map(|s| s.triggers.iter().filter(|k| padded.contains(&format!(" {} ", k.to_lowercase()))).map(|k| k.len()).max().map(|n| (n, s)))
        .max_by_key(|(n, _)| *n)
        .map(|(_, s)| s)
        .or_else(|| db.iter().find(|s| s.name == generic && !s.lines.is_empty()))
}

const USER_HEADER: &str = "# Things you taught Clipster. Same format as his own lines (see app/src/lines.rs).\n# \"when I say X, say Y\" in his box adds to this file. Lines starting with # are ignored.\n";

/// The user's phrase file, created if it doesn't exist yet.
pub fn ensure_user_file() -> PathBuf {
    let path = user_path();
    if !path.exists() {
        let _ = path.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(&path, USER_HEADER);
    }
    path
}

/// Remember "when I say `phrase`, say `reply` / run `commands`" in the user's file.
pub fn teach(phrase: &str, reply: &str, commands: &[String]) -> std::io::Result<()> {
    unlearn(phrase)?;
    let path = ensure_user_file();
    let mut text = std::fs::read_to_string(&path)?;
    let one = |t: &str| t.replace(['\n', '\r'], " ").replace(" | ", " ");
    let phrase = one(phrase);
    text += &format!("\n## chat taught {phrase}\n= {phrase}\n");
    if !reply.is_empty() {
        let line = reply.replace('\n', "\\n");
        // keep it a reply line even if it happens to start like a format line
        let line = if ["> ", "= ", "@ ", "+ ", "% ", "! ", "#"].iter().any(|p| line.starts_with(p)) { format!("\"{line}\"") } else { line };
        text += &format!("{line}\n");
    }
    for c in commands {
        text += &format!("! {}\n", one(c));
    }
    std::fs::write(&path, text)?;
    reload();
    Ok(())
}

/// Forget a taught phrase. `Ok(false)` = there was nothing to forget.
pub fn unlearn(phrase: &str) -> std::io::Result<bool> {
    let path = user_path();
    let Ok(text) = std::fs::read_to_string(&path) else { return Ok(false) };
    let want = crate::understand::clean(phrase);
    let (mut out, mut skipping, mut found) = (String::new(), false, false);
    for line in text.lines() {
        if let Some(name) = line.trim().strip_prefix("## ") {
            skipping = name.strip_prefix("chat taught ").is_some_and(|t| crate::understand::clean(t) == want);
            found |= skipping;
        }
        if !skipping {
            out += line;
            out += "\n";
        }
    }
    if found {
        std::fs::write(&path, out)?;
        reload();
    }
    Ok(found)
}

/// What the user taught: (phrase, what it does).
pub fn taught() -> Vec<(String, String)> {
    db().iter()
        .filter(|s| s.user && s.name.starts_with("chat taught "))
        .map(|s| {
            let does = if s.commands.is_empty() { format!("\"{}\"", s.lines.first().cloned().unwrap_or_default()) } else { s.commands.join(" → ") };
            (s.name["chat taught ".len()..].to_string(), does)
        })
        .collect()
}

/// "my boss" -> "your boss", "i love you" -> "you love me": the captured words, said back.
pub fn reflect(x: &str) -> String {
    let words: Vec<&str> = x.split_whitespace().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < words.len() {
        let pair = (words[i], words.get(i + 1).copied().unwrap_or(""));
        let swapped = match pair {
            ("you", "are") => Some("i am"),
            ("you", "were") => Some("i was"),
            ("i", "am") => Some("you are"),
            ("i", "was") => Some("you were"),
            _ => None,
        };
        if let Some(s) = swapped {
            out.push(s);
            i += 2;
            continue;
        }
        out.push(match words[i] {
            "i" | "me" => "you",
            "my" => "your",
            "mine" => "yours",
            "myself" => "yourself",
            "am" => "are",
            "you" => "me",
            "your" => "my",
            "yours" => "mine",
            "yourself" => "myself",
            w => w,
        });
        i += 1;
    }
    out.join(" ")
}

/// The pattern that fits `words` best (most fixed words), and what its `*` captured.
fn match_pattern<'a>(db: &'a [Section], words: &[&str]) -> Option<(&'a Section, String)> {
    let mut best: Option<(usize, &Section, String)> = None;
    for s in db.iter().filter(|s| !s.patterns.is_empty() && !s.lines.is_empty()) {
        for (before, after) in &s.patterns {
            let fixed = before.len() + after.len();
            let fits = words.len() > fixed && words.iter().zip(before).all(|(w, b)| w == b) && words.iter().rev().zip(after.iter().rev()).all(|(w, a)| w == a);
            if fits && best.as_ref().is_none_or(|b| fixed > b.0) {
                best = Some((fixed, s, words[before.len()..words.len() - after.len()].join(" ")));
            }
        }
    }
    best.map(|(_, s, x)| (s, x))
}

/// A reply from the sentence patterns ("i hate *", "do you like *"), or `None` when none fits.
pub fn pattern(raw: &str) -> Option<Reply> {
    let tidy = crate::understand::tidy_words(raw);
    // "hey clipster, i am tired" is still "i am tired"
    let fillers = ["hey", "hi", "hello", "ok", "okay", "so", "well", "um", "uh", "oh", "clipster", "dear", "and", "but", "honestly", "actually"];
    let words: Vec<&str> = tidy.split_whitespace().skip_while(|w| fillers.contains(w)).collect();
    let db = db();
    let (hit, x) = match_pattern(&db, &words)?;
    let x = reflect(&x);
    Some(to_reply_x(hit, flavoured(&db, &hit.name).unwrap_or(hit), Some(&x)))
}

/// A random line from a section with `{x}` filled in (reflected), e.g. the "didn't understand that" pools.
pub fn reply_with(name: &str, x: &str) -> Reply {
    let db = db();
    let x = reflect(x);
    match flavoured(&db, name).or_else(|| named(&db, "confused")) {
        Some(s) => to_reply_x(s, s, Some(&x)),
        None => Reply::say("..."),
    }
}

/// Process names Clipster has something to say about.
pub fn programs() -> Vec<String> {
    db().iter().filter(|s| s.name.starts_with("program ")).flat_map(|s| s.triggers.clone()).collect()
}

/// Which `## window <kind>` a window title belongs to: the longest matching piece wins.
pub fn window_kind(title: &str) -> Option<String> {
    kind_in(&db(), title)
}

fn kind_in(db: &[Section], title: &str) -> Option<String> {
    let t = title.to_lowercase();
    db.iter()
        .filter_map(|s| s.name.strip_prefix("window ").filter(|_| !s.lines.is_empty()).map(|k| (k, s)))
        .flat_map(|(k, s)| s.triggers.iter().filter(|tr| t.contains(&tr.to_lowercase())).map(move |tr| (tr.len(), k)))
        .max_by_key(|(n, _)| *n)
        .map(|(_, k)| k.to_string())
}

/// What to say when the program `comm` starts.
pub fn program(comm: &str) -> Option<Reply> {
    db().iter().find(|s| s.name.starts_with("program ") && s.triggers.iter().any(|t| t.eq_ignore_ascii_case(comm)) && !s.lines.is_empty()).map(to_reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The database the app uses by default (Unhinged), checked to parse cleanly.
    fn built_in() -> Vec<Section> {
        let (all, problems) = database(unhinged());
        assert!(problems.is_empty(), "{problems:#?}");
        all
    }

    #[test]
    fn boring_database() {
        let (all, problems) = database(false);
        assert!(problems.is_empty(), "{problems:#?}");
        assert!(named(&all, "jokes").is_some() && named(&all, "memories").is_none());
    }

    #[test]
    fn unhinged_overlay_keeps_structure() {
        let (base, _) = database(false);
        let (over, _) = database(true);
        let holes = |t: &str| -> Vec<String> { t.split('{').skip(1).filter_map(|x| x.split_once('}').map(|y| y.0.to_string())).collect() };
        let mut broken = vec![];
        for b in &base {
            let Some(o) = over.iter().find(|o| o.name == b.name) else { continue };
            if (&o.triggers, &o.exact, &o.patterns, &o.buttons, o.mood, o.friend) != (&b.triggers, &b.exact, &b.patterns, &b.buttons, b.mood, b.friend) {
                broken.push(format!("{}: triggers, buttons, mood or % changed", b.name));
            }
            // placeholders every Boring line has must be in every Unhinged line too
            let need: Vec<String> = holes(b.lines.first().map_or("", |l| l)).into_iter().filter(|h| h != "name" && b.lines.iter().all(|l| l.contains(&format!("{{{h}}}")))).collect();
            for l in o.lines.iter().filter(|l| need.iter().any(|h| !l.contains(&format!("{{{h}}}")))) {
                broken.push(format!("{}: missing {need:?} in: {l}", b.name));
            }
        }
        for o in over.iter().filter(|o| o.name.contains('.')) {
            let base_name = o.name.rsplit_once('.').map_or("", |x| x.0);
            if let Some(b) = over.iter().find(|b| b.name == base_name) {
                let need: Vec<String> = holes(b.lines.first().map_or("", |l| l)).into_iter().filter(|h| h != "name" && b.lines.iter().all(|l| l.contains(&format!("{{{h}}}")))).collect();
                for l in o.lines.iter().filter(|l| need.iter().any(|h| !l.contains(&format!("{{{h}}}")))) {
                    broken.push(format!("{}: missing {need:?} in: {l}", o.name));
                }
            }
        }
        assert!(broken.is_empty(), "{} problems: {broken:#?}", broken.len());
    }

    #[test]
    fn answers_find_their_reaction() {
        let mut v = vec![];
        parse("## answer drink\nNice, {x}.\n\n## answer drink tea\n> tea | green tea\nTea!\n\n## answer drink no\n> no | nothing\nNothing?\n", false, &mut v);
        let name = |t: &str| answer_section(&v, "drink", t).map(|s| s.name.clone());
        assert_eq!(name("i drink green tea").as_deref(), Some("answer drink tea"));
        assert_eq!(name("nothing really").as_deref(), Some("answer drink no"));
        assert_eq!(name("coffee").as_deref(), Some("answer drink"), "anything else gets the generic reaction");
        assert_eq!(name("notea"), Some("answer drink".into()), "pieces are whole words");
        assert!(answer_section(&v, "lunch", "soup").is_none());
    }

    #[test]
    fn window_titles_find_their_kind() {
        let mut v = vec![];
        parse("## window writing\n> libreoffice writer | .docx\nWriting?\n\n## window letter\n> untitled 1 - libreoffice writer\nA letter!\n", false, &mut v);
        assert_eq!(kind_in(&v, "Untitled 1 - LibreOffice Writer").as_deref(), Some("letter"), "the longest piece wins");
        assert_eq!(kind_in(&v, "report.docx - Word").as_deref(), Some("writing"));
        assert_eq!(kind_in(&v, "Konsole"), None);
    }

    #[test]
    fn sentence_patterns() {
        let mut v = vec![];
        let text = "## pattern i-am\n~ i'm * | i am *\nYou're {x}?\n## pattern i-am-so\n~ i am so *\nSo {x}!\n## pattern bad\n~ * alone\nx";
        assert_eq!(parse(text, false, &mut v).len(), 1, "a pattern must start with words");
        assert_eq!(v[0].patterns[0], (vec!["i".to_string(), "am".to_string()], vec![]), "contractions expand like typed text does");
        let words = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
        let w = words("i am so tired of meetings");
        let w: Vec<&str> = w.iter().map(|s| s.as_str()).collect();
        let (hit, x) = match_pattern(&v, &w).unwrap();
        assert_eq!((hit.name.as_str(), x.as_str()), ("pattern i-am-so", "tired of meetings"), "the most specific pattern wins");
        assert!(match_pattern(&v, &["i", "am"]).is_none(), "* needs at least one word");
        assert_eq!(reflect("my boss hates me"), "your boss hates you");
        assert_eq!(reflect("i am sure you are right"), "you are sure i am right");
    }

    #[test]
    fn conversation_stays_conversation() {
        use crate::net::Query;
        let all = built_in();
        let from_patterns = |s: &str| {
            let text = crate::understand::respond(s).text;
            all.iter().filter(|p| p.name.starts_with("pattern ") || p.name.starts_with("chat ")).flat_map(|p| &p.lines).any(|l| {
                // the capture can open the line (and is capitalised), so match the fixed pieces around it
                let fixed: Vec<&str> = l.split("{x}").filter(|p| !p.trim().is_empty()).collect();
                text == *l || (l.contains("{x}") && !fixed.is_empty() && fixed.iter().all(|p| text.contains(p)))
            })
        };
        // things people say to him get an answer from him, not the encyclopedia
        for s in ["I am tired of meetings", "i'm so done with this week", "do you like pizza", "can you juggle", "what do you think of linux", "I hate mondays", "my cat is sleeping on my keyboard"] {
            assert!(!matches!(crate::understand::respond(s).effect, Effect::Net(Query::Ask(_))), "went to the internet: {s}");
            assert!(from_patterns(s), "no conversation reply for: {s}");
        }
        // real lookups still go out
        for s in ["who painted the mona lisa", "what is the capital of france", "Ada Lovelace", "how many legs does a spider have"] {
            assert!(matches!(crate::understand::respond(s).effect, Effect::Net(Query::Ask(_))), "should be looked up: {s}");
        }
    }

    #[test]
    fn glitches_spare_placeholders() {
        let mut rng = rand::thread_rng();
        for _ in 0..300 {
            let g = glitch("Ding! {label} is done, {name}. Truly, truly done.", &mut rng);
            assert!(g.contains("{label}") && g.contains("{name}"), "{g}");
        }
    }

    #[test]
    fn format() {
        let mut v = vec![];
        let text = "## chat test\n> what's up | Hello There\n= cake\n@ sly\n+ Joke = joke\n% -3\nA\\nB\n! run ls\n## chat test\nC";
        assert_eq!(parse(text, false, &mut v).len(), 1, "! is refused outside the user's file");
        assert_eq!(v.len(), 1);
        let s = &v[0];
        assert_eq!(s.triggers, ["what is up", "hello there"]);
        assert_eq!(s.exact, ["cake"]);
        assert_eq!((s.mood, s.friend, s.lines.clone()), (Some(Mood::Sly), -3, vec!["A\nB".to_string(), "C".to_string()]));
        assert_eq!(parse("@ grumpy\n## x\n@ grumpy\n+ nope", false, &mut v).len(), 3);
    }

    #[test]
    fn built_in_database() {
        let all = built_in();
        // every section the code asks for by name
        for name in ["jokes", "roasts", "nice", "facts", "tips", "eightball", "confused", "hello", "songs", "secrets", "stories", "dance", "thanks", "summon", "clip_any"] {
            assert!(named(&all, name).is_some(), "missing section {name}");
        }
        assert!(all.iter().any(|s| s.name.starts_with("program ") && s.triggers.iter().any(|t| t == "firefox")));
        // both personalities have what the code asks for
        let (boring, _) = database(false);
        for db in [&all, &boring] {
            for name in ["onboard_hello", "onboard_persona", "onboard_manners", "onboard_autostart", "onboard_tour", "say_timer", "say_note", "say_setname", "say_hat", "say_hat_off", "say_skin", "say_autostart_on", "say_autostart_off", "say_about", "say_cancel", "say_quiet", "say_stopwatch", "say_close", "goodbye", "summon", "welcome_back", "idle", "poke", "wake", "timer_done"] {
                assert!(named(db, name).is_some(), "missing section {name}");
            }
        }
        // and Unhinged has his scripture
        let (unhinged, _) = database(true);
        assert_eq!(named(&unhinged, "memories").map(|s| s.lines.len()), Some(30));
        for name in ["diary", "commandments", "prophecies"].into_iter().map(String::from).chain(MOODS.iter().map(|m| format!("mood_report.{m}"))) {
            assert!(named(&unhinged, &name).is_some(), "missing section {name}");
        }
        assert!(named(&unhinged, "diary").unwrap().lines.iter().all(|l| l.contains("{day}")));
    }

    #[test]
    fn buttons_lead_somewhere() {
        let lost: Vec<String> = built_in()
            .iter()
            .flat_map(|s| s.buttons.iter().map(move |b| (s.name.clone(), b.1.clone())))
            .filter(|(_, cmd)| cmd != "hide" && !cmd.starts_with("http"))
            .filter(|(_, cmd)| {
                let r = crate::understand::respond(cmd);
                matches!(r.effect, Effect::Net(crate::net::Query::Ask(_))) || r.buttons.iter().any(|b| b.0 == "Ask the internet instead")
            })
            .map(|(name, cmd)| format!("{name}: {cmd}"))
            .collect();
        assert!(lost.is_empty(), "buttons nobody understands: {lost:#?}");
    }

    #[test]
    fn every_trigger_reaches_its_topic() {
        let all = built_in();
        let mut lost = vec![];
        for t in all.iter().filter(|s| s.name.starts_with("chat ") && !s.lines.is_empty()) {
            for phrase in t.triggers.iter().chain(&t.exact) {
                let got = crate::understand::respond(phrase).text;
                if !t.lines.contains(&got) {
                    lost.push(format!("{}: \"{phrase}\" -> {}", t.name, got.lines().next().unwrap_or("")));
                }
            }
        }
        assert!(lost.is_empty(), "{} triggers go elsewhere: {lost:#?}", lost.len());
    }

    #[test]
    fn commands_still_win_over_chat() {
        let all = built_in();
        // what conversation says: topics and sentence patterns (with any {x} filled, compared by its fixed start)
        let chat: Vec<&String> = all.iter().filter(|s| s.name.starts_with("chat ") || s.name.starts_with("pattern ")).flat_map(|s| &s.lines).collect();
        let said_by_chat = |text: &str| chat.iter().any(|l| l.as_str() == text || l.split("{x}").next().is_some_and(|start| start.len() > 12 && text.starts_with(start)));
        let commands = [
            "joke", "roast", "fact", "hug", "quiz", "quote", "advice", "dance", "tip", "help", "stats", "flip", "8ball", "roll 2d6", "games", "rps", "guess",
            "surprise", "hat", "skin", "notes", "time", "weather", "news", "cocktail", "recipe", "bored", "xkcd", "moon", "pomodoro", "uuid", "lorem",
            "password 20", "battery", "uptime", "disk", "top", "timers", "cancel", "clipboard history", "12*(3+4)^2", "5 km to miles", "timer 5m tea",
            "remind me in 10 min to stretch", "note buy milk", "open firefox", "why is the sky blue?", "who was Ada Lovelace?", "define serendipity",
            "weather in Rome", "time in Tokyo", "100 dollars to euros", "bitcoin price", "tech news", "on this day", "next holiday", "where is the ISS",
            "another word for happy", "what rhymes with orange", "is github down?", "what's my ip", "show me a dog", "check for updates", "search cats", "youtube lofi",
            "map paris", "louder", "mute", "volume 30", "skip this song", "make the screen darker", "take a screenshot", "find the file called report",
            "my latest download", "empty the trash", "days until christmas", "start a stopwatch", "lock", "run ls", "count hello world", "upper hi",
            "pick pizza, tacos", "call me Sam", "good morning", "wear a wizard hat", "turn gold", "start with my computer", "let's play a game",
            "what should I cook", "make me a drink", "earthquakes", "air quality", "random xkcd", "I'm bored", "SOS in morse code", "hello in binary",
            "tell me a riddle", "would you rather", "emoji quiz", "fortune cookie", "shower thought", "tell me a pun", "do you like me", "play chess",
            "tell me about your past", "read your diary", "commandments", "prophecy", "what's your mood", "be boring", "be unhinged", "smite firefox",
        ];
        let swallowed: Vec<&str> = commands.into_iter().filter(|c| said_by_chat(&crate::understand::respond(c).text)).collect();
        assert!(swallowed.is_empty(), "chat topics swallowed these commands: {swallowed:?}");
    }
}
