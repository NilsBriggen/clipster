//! Free-text understanding without AI: tidy the sentence, spot what it's asking for, map it onto a command.
//! "could you pls set an alarm for half an hour to check the oven?" -> "timer 1800s check the oven".

use crate::brain::{self, exact, url_encode, Act, Effect, Mood, Reply};
use crate::lines;

pub fn respond(input: &str) -> Reply {
    let raw = input.trim();
    let head = raw.split_whitespace().next().unwrap_or("").to_lowercase();
    // shell commands and button-only commands (whose text is data, like quiz answers) are taken literally
    if ["run", "$", "sh", "cmd", "reveal", "wyrpick", "onboard"].contains(&head.as_str()) || head == "answer" && raw.contains('|') {
        if let Some(r) = exact(raw) {
            return r; // shell commands are taken literally, never "understood"
        }
    }
    let s = clean(raw);
    if let Some(r) = intent(&s, raw).or_else(|| exact(raw)).or_else(|| exact(&s)) {
        return r;
    }
    let fixed = fix_typos(&s);
    if fixed != s {
        if let Some(mut r) = intent(&fixed, &fixed).or_else(|| exact(&fixed)) {
            r.text = format!("(I'm guessing you meant \"{fixed}\".)\n{}", r.text);
            return r;
        }
    }
    fallback(raw, &fixed)
}

// ---------- tidying ----------

const SWAPS: &[(&str, &str)] = &[
    ("what's", "what is"), ("whats", "what is"), ("who's", "who is"), ("whos", "who is"), ("where's", "where is"),
    ("wheres", "where is"), ("how's", "how is"), ("hows", "how is"), ("it's", "it is"), ("i'm", "i am"), ("im", "i am"),
    ("don't", "do not"), ("dont", "do not"), ("can't", "cannot"), ("cant", "cannot"), ("won't", "will not"),
    ("i'd", "i would"), ("i'll", "i will"), ("you're", "you are"), ("youre", "you are"), ("u", "you"), ("r", "are"),
    ("ur", "your"), ("pls", "please"), ("plz", "please"), ("thx", "thanks"), ("let's", "let us"), ("lets", "let us"),
    ("isn't", "is not"), ("there's", "there is"), ("that's", "that is"), ("what're", "what are"), ("wanna", "want to"),
    ("gonna", "going to"), ("gimme", "give me"), ("tmrw", "tomorrow"), ("mins", "minutes"), ("secs", "seconds"),
    ("hrs", "hours"), ("noon", "12pm"), ("midnight", "12am"), ("todo", "to do"), ("e-mail", "email"),
];

const PHRASES: &[(&str, &str)] = &[
    ("an hour and a half", "90 minutes"), ("one and a half hours", "90 minutes"), ("half an hour", "30 minutes"),
    ("half a minute", "30 seconds"), ("half a hour", "30 minutes"), ("a quarter of an hour", "15 minutes"),
    ("quarter of an hour", "15 minutes"), ("quarter hour", "15 minutes"), ("a couple of minutes", "2 minutes"),
    ("a couple minutes", "2 minutes"), ("a few minutes", "3 minutes"), ("a few seconds", "10 seconds"),
    ("a minute or two", "2 minutes"), ("a sec", "10 seconds"), ("a moment", "1 minute"),
];

const NUMS: &[(&str, u32)] = &[
    ("zero", 0), ("one", 1), ("two", 2), ("three", 3), ("four", 4), ("five", 5), ("six", 6), ("seven", 7), ("eight", 8),
    ("nine", 9), ("ten", 10), ("eleven", 11), ("twelve", 12), ("thirteen", 13), ("fourteen", 14), ("fifteen", 15),
    ("sixteen", 16), ("seventeen", 17), ("eighteen", 18), ("nineteen", 19), ("twenty", 20), ("thirty", 30),
    ("forty", 40), ("fifty", 50), ("sixty", 60), ("seventy", 70), ("eighty", 80), ("ninety", 90), ("hundred", 100),
];

const UNITISH: &[&str] = &[
    "second", "seconds", "minute", "minutes", "min", "hour", "hours", "day", "days", "week", "mile", "km", "kilometer",
    "pound", "kilo", "kg", "gallon", "liter", "litre", "cup", "foot", "inch", "meter", "metre", "yard", "ounce", "ton",
];

const LEAD: &[&str] = &[
    "hey clipster ", "hi clipster ", "ok clipster ", "okay clipster ", "dear clipster ", "clipster ", "hey ", "ok ", "okay ",
    "so ", "um ", "uh ", "well ", "please ", "pretty please ", "kindly ", "can you please ", "could you please ", "can you ",
    "could you ", "would you please ", "would you mind ", "would you ", "will you ", "i want you to ", "i would like you to ",
    "i need you to ", "i want to ", "i would like to ", "i would like ", "i need to ", "i need ", "i want ", "help me ",
    "let us ", "go ahead and ", "just ", "quickly ", "do you know ", "give me ", "get me ", "show me ", "can i get ",
    "can i have ", "could i have ", "may i have ", "i said ", "now ", "and ", "then ",
];

const TRAIL: &[&str] = &[" please", " for me", " thanks", " thank you", " now", " right now", " clipster", " asap", " quickly", " real quick", " ok", " okay"];

/// Lowercase, expand contractions and number words, drop politeness and filler.
/// Lowercase, punctuation out, contractions and slang expanded, number words as digits; every word kept
/// ("I'm not sure, Clipster!" -> "i am not sure clipster"). Sentence patterns match on this.
pub fn tidy_words(raw: &str) -> String {
    let mut s = raw.to_lowercase().replace(['’', '‘'], "'").replace(['?', '!', '"', '“', '”'], " ");
    s = format!(" {} ", s.split_whitespace().collect::<Vec<_>>().join(" "));
    s = s.replace(". ", " ");
    for (a, b) in PHRASES {
        s = s.replace(&format!(" {a} "), &format!(" {b} "));
    }
    let words: Vec<&str> = s.split_whitespace().collect();
    let mut out: Vec<String> = vec![];
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        let next = words.get(i + 1).copied().unwrap_or("");
        let num = |w: &str| NUMS.iter().find(|n| n.0 == w).map(|n| n.1);
        if let (Some(t), Some(u)) = (num(w), num(next)) {
            if t >= 20 && t % 10 == 0 && u < 10 {
                out.push((t + u).to_string()); // "twenty five"
                i += 2;
                continue;
            }
        }
        if let Some(n) = num(w) {
            out.push(n.to_string());
        } else if (w == "a" || w == "an") && UNITISH.contains(&next) {
            out.push("1".into());
        } else if let Some((_, b)) = SWAPS.iter().find(|(a, _)| *a == w) {
            out.push(b.to_string());
        } else {
            out.push(w.to_string());
        }
        i += 1;
    }
    out.join(" ")
}

/// `tidy_words`, then without polite openers and fillers ("could you please ...", "... thanks").
pub fn clean(raw: &str) -> String {
    let mut s = tidy_words(raw);
    loop {
        let before = s.len();
        for l in LEAD {
            // "hey clipster, could you ..." -> the lead may be followed by a space or a comma
            if let Some(r) = s.strip_prefix(l.trim_end()).filter(|r| r.starts_with([' ', ','])) {
                s = r.trim_start_matches([' ', ',']).to_string();
            }
        }
        for t in TRAIL {
            if let Some(r) = s.strip_suffix(t) {
                if !r.is_empty() {
                    s = r.to_string();
                }
            }
        }
        if s.len() == before {
            break;
        }
    }
    s.trim().trim_end_matches([',', '.', ':', ';']).trim().to_string()
}

// ---------- helpers ----------

fn has(s: &str, kws: &[&str]) -> bool {
    let p = format!(" {s} ");
    kws.iter().any(|k| p.contains(&format!(" {k} ")))
}

fn after<'a>(s: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|p| if s == p.trim_end() { Some("") } else { s.strip_prefix(p) }).map(str::trim)
}

/// Take `piece` (lowercase) back out of what the person typed, so notes and paths keep their capitals.
fn keep(raw: &str, piece: &str) -> String {
    let lr = raw.to_lowercase();
    lr.find(piece)
        .filter(|_| lr.len() == raw.len())
        .and_then(|i| raw.get(i..i + piece.len()))
        .unwrap_or(piece)
        .to_string()
}

fn go(cmd: String) -> Option<Reply> {
    exact(&cmd)
}

fn in_path(p: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(p).is_file() || cfg!(windows) && d.join(format!("{p}.exe")).is_file()))
}

fn first_in_path(list: &[&str]) -> Option<String> {
    list.iter().find(|p| in_path(p)).map(|p| p.to_string())
}

// ---------- the intents, most specific first ----------

fn intent(s: &str, raw: &str) -> Option<Reply> {
    if let Some(r) = crisis(s) {
        return Some(r);
    }
    if let Some(r) = teach(raw) {
        return Some(r);
    }
    if s.is_empty() {
        return Some(brain::greeting());
    }
    if let Some(r) = lines::topic(s, true) {
        return Some(r); // what the user taught me comes first
    }
    if let Some(r) = brain::egg(s) {
        return Some(r); // 🥚
    }
    if let Some(day) = after(s, &["my birthday is ", "my birthday is on ", "my bday is ", "i was born on ", "remember my birthday "]) {
        return go(format!("birthday {day}"));
    }
    // commands with side effects first, then conversation topics, then the broad keyword groups
    timers(s, raw)
        .or_else(|| notes(s, raw))
        .or_else(|| powers(s, raw))
        .or_else(|| lines::topic(s, false))
        .or_else(|| extras(s, raw))
        .or_else(|| maths(s))
        .or_else(|| verbs(s, raw))
        .or_else(|| text_tools(s, raw))
        .or_else(|| chance(s))
        .or_else(|| chat(s))
        .or_else(|| system(s))
        // sentence patterns ("i am *", "do you like *"), but encyclopedia questions go to the encyclopedia
        .or_else(|| if knowledge(s) { None } else { lines::pattern(raw) })
        .or_else(|| questions(s, raw))
}

/// An encyclopedia-style question ("who painted…", "what is a…", "how many…"), not one about Clipster or the person.
fn knowledge(s: &str) -> bool {
    let personal = s.split_whitespace().any(|w| ["you", "your", "yours", "yourself", "i", "me", "my", "mine", "myself", "we", "our"].contains(&w));
    let openers = [
        "who ", "what is ", "what are ", "what was ", "what were ", "what does ", "what did ", "when ", "where ", "which ", "how many ", "how much ",
        "how far ", "how old ", "how tall ", "how big ", "how long ", "how does ", "why is ", "why are ", "why does ", "why did ", "is it true ", "tell me about ",
    ];
    !personal && openers.iter().any(|p| s.starts_with(p))
}

/// "when I say gm, say good morning sunshine" / "when I say work, open firefox and open code"
fn teach(raw: &str) -> Option<Reply> {
    let low = raw.to_lowercase();
    if low.len() != raw.len() {
        return None; // byte offsets must line up between the two
    }
    let start = ["when i say ", "whenever i say ", "if i say "].iter().find_map(|p| low.find(p).filter(|i| *i <= 12).map(|i| i + p.len()))?;
    let (rest, lrest) = (&raw[start..], &low[start..]);
    let first = |keys: &[&str]| keys.iter().filter_map(|k| lrest.find(k).map(|i| (i, k.len()))).min();
    let unquote = |t: &str| t.trim().trim_matches(['"', '\'', '“', '”']).trim().to_string();
    if let Some((i, len)) = first(&[", say ", " say ", ", reply ", " reply ", ", answer ", " answer ", ", you say ", " you say "]) {
        let (phrase, reply) = (unquote(&rest[..i]), unquote(&rest[i + len..]));
        if phrase.is_empty() || reply.is_empty() {
            return None;
        }
        return Some(Reply::say(format!("Got it! 📝 When you say \"{phrase}\", I'll say:\n\"{reply}\"\n\nTry it! (Say \"what have you learned\" to see everything.)")).mood(Mood::Happy).effect(Effect::Teach(phrase, reply, vec![])));
    }
    let (i, len) = first(&[", then ", " then ", ", "])?;
    let phrase = unquote(&rest[..i]);
    let mut cmds = vec![rest[i + len..].to_string()];
    for sep in ["; ", " and then ", " then ", " and "] {
        cmds = cmds.iter().flat_map(|c| c.split(sep).map(|x| unquote(x)).collect::<Vec<_>>()).filter(|c| !c.is_empty()).collect();
    }
    if phrase.is_empty() || cmds.is_empty() {
        return None;
    }
    let list = cmds.join(" → ");
    Some(Reply::say(format!("Got it! 📝 When you say \"{phrase}\", I'll do: {list}\n\nTry it! (Say \"what have you learned\" to see everything.)")).mood(Mood::Happy).effect(Effect::Teach(phrase, String::new(), cmds)))
}

/// Someone who might be in danger gets a straight answer: no jokes, no gags, a real place to turn to.
fn crisis(s: &str) -> Option<Reply> {
    let words = [
        "kill myself", "killing myself", "suicide", "suicidal", "end my life", "ending my life", "want to die", "wanna die", "self harm", "self-harm",
        "hurt myself", "hurting myself", "cut myself", "cutting myself", "no reason to live", "better off dead", "do not want to live", "take my own life",
        "do not want to be alive", "not want to be here anymore",
    ];
    has(s, &words).then(|| {
        Reply::say(
            "I'm really sorry you're feeling this way. I'm only a paperclip, but you deserve to talk to a real person about this, right now.\n\n\
             If you might act on these thoughts, please call your local emergency number. You can also reach a free, confidential helpline in your country, any time:",
        )
        .btn("Find a helpline near me", Act::Open("https://findahelpline.com".into()))
        .btn("Close", Act::Hide)
    })
}

const TIMER_WORDS: &[&str] = &[
    "timer", "timers", "alarm", "alarms", "countdown", "count down", "remind", "reminder", "reminders", "wake me",
    "ping me", "alert me", "tell me in", "notify me", "let me know in", "beep", "remember to", "do not let me forget",
];

fn timers(s: &str, raw: &str) -> Option<Reply> {
    if !has(s, TIMER_WORDS) && !s.starts_with("in ") && !s.starts_with("every ") || has(s, &["until", "till"]) || s.contains("countdown to") || s.contains("count down to") {
        return None; // "countdown to christmas" is a date question
    }
    // repeating: "remind me every hour to drink water"
    let ev = s.replace("every hour", "every 1 hour").replace("every minute", "every 1 minute").replace("hourly", "every 1 hour").replace("every half hour", "every 30 minutes");
    if let Some(i) = ev.find("every ") {
        if let Some((secs, rest)) = brain::parse_dur(&ev[i + 6..]) {
            return go(format!("every {secs}s {}", keep(raw, &label(&ev[..i], rest))));
        }
    }
    if has(s, &["cancel", "stop", "delete", "clear", "remove", "end", "kill", "turn off"]) && has(s, &["timer", "timers", "alarm", "alarms", "reminder", "reminders", "countdown", "all"]) {
        return go("cancel".into());
    }
    if let Some((secs, label)) = timer_parts(s) {
        return go(format!("timer {secs}s {}", keep(raw, &label)));
    }
    let checking = has(s, &["timer", "alarm", "countdown"]) && has(s, &["left", "remaining", "how long", "status", "check", "show", "list"]);
    if checking || has(s, &["timers", "alarms", "reminders", "how long left", "how much time left", "time left"]) || s == "timer" {
        return go("timers".into());
    }
    if let Some(what) = after(s, &["remind me to ", "remind me about ", "remind me of ", "remind me ", "remind "]) {
        return go(format!("remind {}", keep(raw, what)));
    }
    if has(s, &["timer", "alarm", "countdown"]) {
        return Some(Reply::say("For how long? Try: timer 10 minutes, or alarm at 7:30"));
    }
    None
}

/// Find "10 minutes" / "1h30" / "at 5pm" anywhere in the sentence, plus what it's for.
pub fn timer_parts(s: &str) -> Option<(u64, String)> {
    let b = s.as_bytes();
    for i in 0..b.len() {
        if b[i].is_ascii_digit() && (i == 0 || b[i - 1] == b' ') {
            let before = &s[..i];
            if before.ends_with("at ") {
                continue;
            }
            if let Some((secs, rest)) = brain::parse_dur(&s[i..]) {
                let used = &s[i..s.len() - rest.len()];
                if used.chars().any(|c| c.is_ascii_alphabetic()) || before.ends_with("in ") || before.ends_with("for ") {
                    return Some((secs, label(before, rest)));
                }
            }
        }
    }
    let at = if s.starts_with("at ") { Some(0) } else { s.find(" at ").map(|i| i + 1) }?;
    let (secs, used) = clock(&s[at + 3..])?;
    Some((secs, label(&s[..at], &s[at + 3 + used..])))
}

fn label(before: &str, after: &str) -> String {
    const SKIP: &[&str] = &[
        "timer", "alarm", "countdown", "reminder", "for", "to", "that", "about", "called", "named", "and", "then", "so",
        "remind", "me", "saying", "wake", "up", "ping", "alert", "set", "a", "an", "the", "start", "in", "please",
    ];
    let words: Vec<&str> = after.split_whitespace().skip_while(|w| SKIP.contains(w)).collect();
    if !words.is_empty() {
        return words.join(" ");
    }
    let what = before.split_once(" to ").or_else(|| before.split_once("about ")).map(|x| x.1).unwrap_or("");
    let mut w: Vec<&str> = what.split_whitespace().collect();
    while w.last().is_some_and(|l| ["in", "for", "at", "after", "on", "by"].contains(l)) {
        w.pop();
    }
    w.join(" ")
}

/// "5pm", "17:30", "7:15 am", "9 o'clock" -> seconds until then, and how much text was used.
fn clock(t: &str) -> Option<(u64, usize)> {
    use chrono::{Duration, Local};
    let d = t.find(|c: char| !c.is_ascii_digit()).unwrap_or(t.len());
    if d == 0 || d > 2 {
        return None;
    }
    let h: u32 = t[..d].parse().ok()?;
    let mut i = d;
    let mut m = 0;
    if t[i..].starts_with(':') {
        m = t.get(i + 1..i + 3)?.parse().ok()?;
        i += 3;
    }
    let rest = t[i..].trim_start();
    let gap = t.len() - i - rest.len();
    let mut pm = None;
    for (w, is_pm) in [("pm", Some(true)), ("p.m.", Some(true)), ("am", Some(false)), ("a.m.", Some(false)), ("o'clock", None), ("oclock", None)] {
        if rest.starts_with(w) {
            pm = is_pm;
            i += gap + w.len();
            break;
        }
    }
    if h > 23 || m > 59 {
        return None;
    }
    let hours: Vec<u32> = match pm {
        Some(p) if h <= 12 => vec![h % 12 + if p { 12 } else { 0 }],
        Some(_) => return None,
        None if h >= 1 && h < 12 => vec![h, h + 12], // "at 5": whichever comes first
        None => vec![h],
    };
    let now = Local::now().naive_local();
    hours
        .iter()
        .filter_map(|&h| {
            let target = now.date().and_hms_opt(h, m, 0)?;
            let target = if target <= now { target + Duration::days(1) } else { target };
            Some((target - now).num_seconds().max(1) as u64)
        })
        .min()
        .map(|secs| (secs, i))
}

/// Controlling the computer and handy tools: files, sound, music, screen, power, dates...
fn powers(s: &str, raw: &str) -> Option<Reply> {
    // system switches and readings
    for (kws, what) in [(&["wifi", "wi fi", "wireless", "internet"][..], "wifi"), (&["bluetooth"][..], "bluetooth")] {
        if has(s, kws) && !has(s, &["name", "password", "which", "what"]) {
            if has(s, &["off", "disable", "kill", "stop"]) {
                return go(format!("{what} off"));
            }
            if has(s, &["turn on", "switch on", "enable", "start", "on"]) && !has(s, &["am i on", "i am on"]) {
                return go(format!("{what} on"));
            }
        }
    }
    if has(s, &["wifi name", "wi fi name", "which wifi", "what wifi", "network name", "ssid", "which network am i on", "what network am i on"]) {
        return go("wifiname".into());
    }
    if let Some(t) = after(s, &["how much ram is ", "how much memory is ", "how much ram does ", "how much memory does ", "memory used by ", "ram used by ", "ram usage of ", "memory usage of ", "how much ram ", "how much memory "]) {
        if let Some(app) = t.split_whitespace().next().filter(|a| !["my", "do", "i", "is", "left", "free", "the"].contains(a)) {
            return go(format!("appram {app}"));
        }
    }
    if has(s, &["cpu temperature", "cpu temp", "how hot is my cpu", "how hot is my computer", "how hot is my laptop", "computer temperature", "laptop temperature", "is my cpu hot", "is my computer hot", "is my laptop hot", "cpu heat", "processor temperature"]) {
        return go("cputemp".into());
    }
    if has(s, &["biggest files", "largest files", "big files", "what is taking space in downloads", "huge files"]) {
        return go("bigfiles".into());
    }
    if has(s, &["cache size", "clear cache", "clear my cache", "clean my cache", "clean cache", "my cache", "the cache"]) {
        return go("cache".into());
    }
    if has(s, &["which distro", "what distro", "which linux", "what linux", "kernel version", "which kernel", "what kernel", "os version", "operating system", "my distro"]) {
        return go("sysinfo".into());
    }
    if has(s, &["my gpu", "graphics card", "which gpu", "what gpu", "video card", "my graphics"]) {
        return go("gpu".into());
    }
    // the first-run setup, again ("start the tour" must not launch a program called tour)
    if has(s, &["start the tour", "take the tour", "give me a tour", "show me around", "tutorial", "onboarding", "the tour", "start the setup", "setup again"]) {
        return go("tour".into());
    }
    // teaching
    if has(s, &["what have you learned", "what did i teach you", "what i taught you", "what have i taught you", "things i taught you", "what you learned", "my phrases"]) {
        return go("taught".into());
    }
    if let Some(p) = after(s, &["forget what i taught you about ", "forget the phrase ", "unlearn ", "stop responding to ", "forget when i say "]) {
        return go(format!("unlearn {p}"));
    }
    if has(s, &["edit my lines", "edit my phrases", "edit what you learned", "open my lines", "open my phrases", "edit your lines"]) {
        return go("editlines".into());
    }
    if let Some(n) = after(s, &["my name is ", "call me ", "you can call me ", "please call me ", "i am called ", "i go by "]) {
        return go(format!("setname {}", keep(raw, n)));
    }
    if has(s, &["what is my name", "who am i", "do you know my name", "what do you call me", "say my name", "do you remember me"]) {
        return go("myname".into());
    }
    if ["good morning", "morning", "briefing", "daily briefing", "my briefing", "morning briefing", "what is today", "what is up today", "what is happening today", "my day", "plan for today", "start my day", "what is on today"].contains(&s) {
        return go("briefing".into());
    }
    if has(s, &["surprise me", "random article", "random wikipedia", "random wiki", "teach me something", "learn something", "something random", "random knowledge", "random fact from wikipedia"]) {
        return go("randomwiki".into());
    }
    if has(s, &["start with my computer", "start automatically", "autostart", "auto start", "launch at login", "start at login", "open at startup", "run at startup", "start on login", "start at boot", "start when i log in"]) {
        return go(format!("autostart {s}"));
    }
    // games
    if has(s, &["rock paper scissors", "rock paper scissor", "rps"]) || ["rock", "paper", "scissors"].contains(&s) {
        let pick = ["rock", "paper", "scissors"].into_iter().find(|m| s.split_whitespace().last() == Some(*m) && s.split_whitespace().count() <= 2).unwrap_or("");
        return go(format!("rps {pick}"));
    }
    if has(s, &["guess my number", "guess the number", "number guessing", "guess a number", "guessing game", "read my mind"]) {
        return go("guess".into());
    }
    // the arcade: "play chess", "let's play paper jam", "open minesweeper"
    const ARCADE: &[(&[&str], &str)] = &[
        (&["paper jam", "paperjam", "tetris", "falling blocks", "block game", "blocks game"], "paperjam"),
        (&["clip glide", "clipglide", "flappy", "flappy bird", "flying game"], "clipglide"),
        (&["desk merge", "deskmerge", "suika", "merge game", "watermelon game"], "deskmerge"),
        (&["bug hunt", "bughunt", "minesweeper", "mine sweeper", "mines"], "bughunt"),
        (&["chess"], "chess"),
        (&["hangman", "hang man"], "hangman"),
        (&["byte size", "bytesize", "play 2048", "2048 game", "sliding tiles", "tile game", "sliding game"], "bytesize"),
    ];
    // "i lost at chess" or "board games are fun" is small talk, not a request to play
    let small_talk = (["i ", "we ", "my "].iter().any(|p| s.starts_with(p)) || has(s, &["is", "are", "was", "were"])) && !has(s, &["play"]);
    if let Some((_, id)) = ARCADE.iter().find(|(kws, _)| has(s, kws)).filter(|_| !small_talk) {
        return go(format!("play {id}"));
    }
    if !small_talk && has(s, &["play a game", "games", "play something", "play with me", "a game", "game", "arcade"]) {
        return go("games".into());
    }
    // text tricks: "upside down hello", "hello in morse code", "01001000 01101001 from binary"
    const TRICKS: &[(&str, &str)] = &[
        ("upside down ", "upsidedown"), ("flip text ", "upsidedown"), ("flip the text ", "upsidedown"), ("morse code for ", "morse"), ("morse code ", "morse"),
        ("morse ", "morse"), ("from morse ", "morse"), ("binary for ", "binary"), ("binary ", "binary"), ("from binary ", "binary"),
        ("leetspeak ", "leet"), ("leet ", "leet"), ("rot13 ", "rot13"),
    ];
    for (p, kind) in TRICKS {
        if let Some(t) = after(s, &[p]).filter(|t| !t.is_empty()) {
            return go(format!("{kind} {}", keep(raw, t)));
        }
    }
    for (suffix, kind) in [(" in morse code", "morse"), (" in morse", "morse"), (" to morse", "morse"), (" from morse", "morse"), (" in binary", "binary"), (" to binary", "binary"), (" from binary", "binary"), (" upside down", "upsidedown"), (" in leetspeak", "leet"), (" in leet", "leet")] {
        if let Some(t) = s.strip_suffix(suffix) {
            let t = after(t, &["convert ", "translate ", "write ", "put ", "turn ", "say ", "make "]).unwrap_or(t);
            return go(format!("{kind} {}", keep(raw, t)));
        }
    }
    // dress-up: "wear a wizard hat", "take off your hat", "turn gold", "make yourself pink"
    let question = ["what ", "who ", "why ", "how ", "where "].iter().any(|q| s.starts_with(q));
    if !question && has(s, &["hat", "hats", "cap", "crown", "beanie", "sombrero", "tophat", "halo"]) {
        let mut h = format!(" {s} ");
        for (a, b) in [(" christmas ", " santa "), (" witch ", " wizard "), (" magic ", " wizard "), (" king ", " crown "), (" queen ", " crown "), (" royal ", " crown "), (" fancy ", " top "), (" tophat ", " top "), (" birthday ", " party "), (" take off ", " none "), (" remove ", " none "), (" no hat ", " none ")] {
            h = h.replace(a, b);
        }
        return go(format!("hat {}", h.trim()));
    }
    const SKINS: &[(&str, &str)] = &[
        ("gold", "gold"), ("golden", "gold"), ("rose", "rose"), ("pink", "rose"), ("neon", "neon"), ("green", "neon"), ("glow", "neon"),
        ("rainbow", "rainbow"), ("midnight", "midnight"), ("dark", "midnight"), ("black", "midnight"), ("ocean", "ocean"), ("blue", "ocean"),
        ("silver", "silver"), ("classic", "silver"), ("normal", "silver"), ("default", "silver"), ("beige", "beige"), ("retro", "beige"),
        ("ember", "ember"), ("red", "ember"), ("lava", "ember"),
    ];
    if let Some(skin) = SKINS.iter().find(|(w, _)| has(s, &[w])).map(|x| x.1) {
        if !question && (has(s, &["yourself", "you", "your", "skin", "color", "colour", "clipster"]) || s.split_whitespace().count() <= 3 && has(s, &["turn", "go", "be", "paint", "dark mode", "mode"])) {
            return go(format!("skin {skin}"));
        }
    }
    const FIND: &[&str] = &[
        "find file ", "find files ", "find the file ", "find my file ", "find a file ", "search for file ", "search for the file ",
        "search my files for ", "locate file ", "locate the file ", "locate ", "where is my file ", "where is the file ",
        "look for file ", "look for the file ", "find document ", "find my document ", "find the document ", "find a document ",
    ];
    if let Some(n) = after(s, FIND).filter(|n| !n.is_empty()) {
        return go(format!("findfile {}", keep(raw, n.trim_start_matches("called ").trim_start_matches("named "))));
    }
    if let Some(t) = after(s, &["qr code for ", "qr code of ", "qr code with ", "a qr code for ", "make a qr code for ", "make a qr code of ", "create a qr code for ", "qr for ", "qr code ", "qr "]) {
        return go(format!("qr {}", keep(raw, t)));
    }
    if ["qr", "qr code", "a qr code"].contains(&s) {
        return go("qr".into());
    }
    if has(s, &["shrug", "table flip", "tableflip", "flip the table", "flip a table", "flip table", "lenny", "lenny face", "unflip", "put the table back", "disapproval", "look of disapproval", "kaomoji"]) {
        return go(format!("kaomoji {s}"));
    }
    // dates: "what date is in 90 days", "what day is christmas", "days until june 3"
    if let Some(r) = after(s, &["what date is ", "what is the date ", "what day will it be ", "which date is ", "the date "]).filter(|r| r.starts_with("in ")) {
        return go(format!("until {r}"));
    }
    if let Some(r) = after(s, &["what day is ", "what day of the week is ", "which day is ", "what weekday is "]).filter(|r| !["it", "today", "it today", "tomorrow"].contains(r)) {
        return go(format!("until {r}"));
    }
    let padded = format!(" {s}"); // whole words only: "still working" is not "till working"
    for k in ["how many days are left until ", "how many days until ", "how many days till ", "how many weeks until ", "how long is it until ", "how long until ", "how long till ", "days until ", "day until ", "days till ", "weeks until ", "countdown to ", "count down to ", "time until ", "until ", "till "] {
        if let Some((_, r)) = padded.split_once(&format!(" {k}")) {
            return go(format!("until {r}"));
        }
    }
    let volume_down = has(s, &["too loud", "quieter", "softer", "turn it down", "turn down the sound", "turn down the volume", "lower the volume", "volume down", "sound down"]);
    let volume_up = has(s, &["louder", "turn it up", "turn up the sound", "turn up the volume", "raise the volume", "volume up", "sound up", "cannot hear", "can not hear", "pump it up"]);
    if volume_down {
        return go("volume down".into());
    }
    if volume_up {
        return go("volume up".into());
    }
    if has(s, &["volume", "unmute", "mute the sound", "mute sound", "mute audio", "mute my computer", "mute the computer", "mute speakers", "mute the speakers"]) {
        return go(format!("volume {s}"));
    }
    if has(s, &[
        "next song", "next track", "skip song", "skip this song", "skip track", "skip this", "previous song", "previous track", "last song",
        "go back a song", "pause music", "pause the music", "pause", "resume", "resume music", "stop the music", "stop music", "what is playing",
        "what song is this", "what song is playing", "current song", "now playing", "play music", "play the music", "play my music", "unpause",
    ]) {
        return go(format!("media {s}"));
    }
    if has(s, &["too bright", "dimmer", "dim the screen", "dim my screen", "darker", "screen darker", "lower the brightness", "brightness down"]) {
        return go("brightness down".into());
    }
    if has(s, &["too dark", "brighter", "screen brighter", "brightness up", "raise the brightness"]) {
        return go("brightness up".into());
    }
    if has(s, &["brightness"]) {
        return go(format!("brightness {s}"));
    }
    if has(s, &["screenshot", "screen shot", "screen capture", "capture the screen", "capture my screen", "print screen", "snapshot", "screengrab", "screen grab"]) {
        return go(format!("screenshot {s}"));
    }
    let whole_computer = has(s, &["computer", "pc", "laptop", "system", "machine", "everything"]) || ["shutdown", "shut down", "restart", "reboot", "power off", "turn off"].contains(&s);
    if (whole_computer && has(s, &["shutdown", "shut down", "power off", "turn off", "restart", "reboot"])) || has(s, &["suspend", "sleep mode", "hibernate", "log out", "logout", "log me out", "sign out", "sign me out"]) {
        return go(format!("shutdown {s}"));
    }
    for (kws, cmd) in [
        (&["empty trash", "empty the trash", "empty my trash", "clear trash", "clear the trash", "empty recycle bin", "empty the recycle bin", "empty the bin", "take out the trash", "empty bin"][..], "trash"),
        (&["latest download", "last download", "recent download", "newest download", "what did i download", "last thing i downloaded", "most recent download"][..], "lastdownload"),
        (&["clipboard history", "copy history", "what did i copy", "things i copied", "copied before", "my clipboard", "clipboard", "what have i copied"][..], "cliphistory"),
        (&["pomodoro", "focus session", "focus mode", "focus timer", "help me focus", "deep work", "study session"][..], "pomodoro"),
        (&["uuid", "guid", "unique id"][..], "uuid"),
        (&["lorem ipsum", "lorem", "placeholder text", "dummy text", "filler text"][..], "lorem"),
    ] {
        if has(s, kws) {
            return go(cmd.into());
        }
    }
    if has(s, &["stopwatch", "stop watch"]) {
        return go(format!("stopwatch {s}"));
    }
    None
}

/// The online extras: recipes, books, TV, earth and sky, and silly stuff.
fn extras(s: &str, raw: &str) -> Option<Reply> {
    let place_after = |s: &str| [" in ", " for ", " at "].iter().filter_map(|k| s.rsplit_once(k).map(|x| x.1.to_string())).next().unwrap_or_default();
    for (kws, kind) in [(&["programming joke", "dev joke", "coding joke", "nerd joke", "computer joke", "developer joke"][..], "dev"), (&["chuck norris"][..], "chuck"), (&["dad joke", "dad jokes"][..], "dad")] {
        if has(s, kws) {
            return go(format!("joke {kind}"));
        }
    }
    if has(s, &["earthquake", "earthquakes", "quake", "quakes", "seismic"]) {
        return go("quakes".into());
    }
    if has(s, &["air quality", "pollution", "smog", "uv index", "uv", "sunscreen", "air outside", "fine dust"]) {
        return go(format!("air {}", place_after(s).trim_end_matches(" today")));
    }
    if let Some(q) = after(s, &["books about ", "book about ", "books on ", "a book about ", "recommend a book about ", "find a book about ", "find the book ", "the book ", "book "]) {
        return go(format!("book {}", keep(raw, q)));
    }
    if let Some(q) = after(s, &["tell me about the show ", "tell me about the series ", "tell me about the tv show ", "the tv show ", "tv show ", "the show ", "series ", "show info "]) {
        return go(format!("show {q}"));
    }
    if let Some(q) = after(s, &["tell me about the pokemon ", "pokemon ", "pokedex "]).or_else(|| s.strip_suffix(" pokemon")) {
        return go(format!("pokemon {}", q.trim_start_matches("the ")));
    }
    if let Some(q) = after(s, &["a recipe for ", "recipe for ", "recipes for ", "recipe ", "cook "]) {
        return go(format!("recipe {q}"));
    }
    if has(s, &["what should i cook", "what to cook", "what should i eat", "dinner idea", "dinner ideas", "i am hungry", "what is for dinner", "lunch idea", "recipe", "recipes", "something to eat"]) {
        return go("recipe".into());
    }
    if has(s, &["cocktail", "cocktails", "make me a drink", "drink idea", "what should i drink", "mix me a drink", "happy hour", "mocktail"]) {
        return go("cocktail".into());
    }
    if has(s, &["cat fact", "cat facts", "fact about cats", "facts about cats", "tell me about cats"]) {
        return go("catfact".into());
    }
    if has(s, &["i am bored", "bored", "nothing to do", "what should i do", "what can i do", "something to do", "give me something to do"]) {
        return go("bored".into());
    }
    if has(s, &["xkcd", "comic", "a comic", "web comic", "comics"]) {
        return go(format!("xkcd {}", if has(s, &["random", "old", "another"]) { "random" } else { "" }));
    }
    if has(s, &["affirmation", "encourage me", "make me feel better", "i feel bad", "i am sad", "i feel sad", "feeling down", "bad day", "i am stressed", "cheer me on"]) {
        return go("affirm".into());
    }
    if has(s, &["kanye", "ye quote"]) {
        return go("kanye".into());
    }
    if has(s, &["moon", "full moon", "moon phase", "lunar", "werewolf"]) && !has(s, &["far", "distance", "weigh", "land", "landing", "who", "walk", "walked"]) {
        return go("moon".into());
    }
    None
}

fn notes(s: &str, raw: &str) -> Option<Reply> {
    let about_notes = has(s, &["notes", "note", "to do", "to do list", "list", "todos", "reminders"]);
    if about_notes && has(s, &["delete", "clear", "erase", "remove", "wipe", "forget", "empty", "reset"]) && !s.starts_with("note ") {
        return go("forget".into());
    }
    if s == "notes" || has(s, &["my notes", "show notes", "list notes", "read notes", "my to do", "my to do list", "my list", "my todos", "what is on my list", "what did i ask you to remember", "what did i tell you to remember", "what do i need to do", "what should i remember", "all notes"]) {
        return go("notes".into());
    }
    const P: &[&str] = &[
        "note that ", "note to self ", "note down ", "note: ", "note ", "make a note that ", "make a note to ", "make a note of ",
        "make a note ", "take a note that ", "take a note ", "write down that ", "write down ", "jot down ", "remember that ",
        "remember to ", "remember ", "do not let me forget to ", "do not let me forget ", "add a note ", "save a note ",
        "new note ", "to do ", "write a note ",
    ];
    if let Some(what) = after(s, P).filter(|w| !w.is_empty()) {
        return go(format!("note {}", keep(raw, what)));
    }
    // "add milk to my list"
    let (what, list) = s.strip_prefix("add ")?.rsplit_once(" to ")?;
    has(list, &["list", "notes", "to do", "to do list", "shopping list", "todos"]).then(|| ())?;
    go(format!("note {}", keep(raw, what)))
}

const MATH_WORDS: &[(&str, &str)] = &[
    (" plus ", "+"), (" minus ", "-"), (" multiplied by ", "*"), (" times ", "*"), (" divided by ", "/"), (" over ", "/"),
    (" to the power of ", "^"), (" raised to ", "^"), (" to the ", "^"), (" squared", "^2"), (" cubed", "^3"),
    (" percent of ", "% of "), (" mod ", "%"), (" modulo ", "%"), ("square root of ", "sqrt "), ("root of ", "sqrt "),
    ("half of ", "0.5*"), ("double ", "2*"), ("twice ", "2*"), ("triple ", "3*"), (" x ", "*"),
];

fn maths(s: &str) -> Option<Reply> {
    // money and dates
    if let Some(r) = after(s, &["tip on ", "tip for ", "how much tip on ", "how much should i tip on ", "what tip on ", "calculate tip on ", "tip "]).filter(|r| r.chars().any(|c| c.is_ascii_digit())) {
        return go(format!("tip {r}"));
    }
    if let Some(r) = after(s, &["split ", "split the bill ", "divide the bill "]).filter(|r| r.chars().any(|c| c.is_ascii_digit())) {
        return go(format!("split {r}"));
    }
    if let Some(r) = after(s, &["percent change from ", "percentage change from ", "% change from ", "percent increase from ", "percent decrease from ", "how much percent from "]) {
        return go(format!("pctchange {r}"));
    }
    if let Some(r) = after(s, &["how old am i if i was born on ", "how old am i if i was born ", "how old is someone born on ", "how old is someone born in ", "how old is someone born ", "age of someone born ", "i was born on ", "i was born "]) {
        return go(format!("age {r}"));
    }
    if let Some(r) = after(s, &["how many days between ", "days between ", "number of days between ", "how long between ", "time between "]) {
        return go(format!("between {r}"));
    }
    if has(s, &["unix timestamp", "unix time", "epoch time", "current timestamp", "timestamp now", "the timestamp"]) && !s.chars().any(|c| c.is_ascii_digit()) {
        return go("timestamp".into());
    }
    if let Some(r) = s.strip_suffix(" to date").or_else(|| s.strip_suffix(" as a date")).or_else(|| after(s, &["convert timestamp ", "timestamp ", "unix "])) {
        if r.trim_start_matches("convert ").chars().all(|c| c.is_ascii_digit()) {
            return go(format!("timestamp {}", r.trim_start_matches("convert ")));
        }
    }
    let q = after(s, &["what is ", "how much is ", "how much ", "calculate ", "calc ", "compute ", "solve ", "work out ", "evaluate ", "what does ", "what do ", "convert ", "change "]).unwrap_or(s);
    if let Some(r) = crate::powers::bases(q) {
        return Some(r); // "what is 255 in hex"
    }
    // unit conversions: "5 kilometers in miles", "how many cups in a gallon"
    let q2 = q.replace(" into ", " to ").replace(" equals ", " to ").replace(" = ", " to ").replace(" in terms of ", " to ");
    if let Some(r) = brain::convert(&q2) {
        return Some(r);
    }
    if let Some(r) = s.strip_prefix("how many ") {
        for sep in [" are there in ", " are in ", " is in ", " in ", " make ", " are ", " is "] {
            if let Some((x, y)) = r.split_once(sep) {
                let y = y.trim_end_matches(" make").trim_end_matches(" have");
                let y = if y.starts_with(|c: char| c.is_ascii_digit()) || y.starts_with("a ") || y.starts_with("an ") { y.to_string() } else { format!("1 {y}") };
                if let Some(r) = brain::convert(&format!("{y} to {x}")) {
                    return Some(r);
                }
            }
        }
    }
    if let Some(r) = money(&q2) {
        return Some(r);
    }
    if !q.contains(|c: char| c.is_ascii_digit()) {
        return None;
    }

    let q = q.strip_prefix("the ").unwrap_or(q);
    let mut e = format!(" {} ", q.trim_end_matches(" equal").trim_end_matches(" equals").trim_end_matches(" make").trim_end_matches(" is").trim_end_matches(" ="));
    for (a, b) in MATH_WORDS {
        e = e.replace(a, b);
    }
    let e = e.trim();
    // "add 5 and 3", "subtract 3 from 10", "average of 2, 4 and 6"
    let e = if let Some(r) = after(e, &["add ", "sum of ", "the sum of "]) {
        r.replace(" and ", "+").replace(" to ", "+").replace(',', "+")
    } else if let Some((a, b)) = after(e, &["subtract ", "take away "]).and_then(|r| r.split_once(" from ")) {
        format!("({b})-({a})")
    } else if let Some(r) = after(e, &["multiply ", "product of ", "the product of "]) {
        r.replace(" and ", "*").replace(" by ", "*")
    } else if let Some(r) = after(e, &["divide "]) {
        r.replace(" by ", "/")
    } else if let Some((a, b)) = after(e, &["difference between ", "the difference between "]).and_then(|r| r.split_once(" and ")) {
        format!("abs(({a})-({b}))")
    } else if let Some(r) = after(e, &["average of ", "the average of ", "mean of ", "the mean of "]) {
        let parts: Vec<&str> = r.split([',', ' ']).filter(|w| !w.is_empty() && *w != "and").collect();
        format!("({})/{}", parts.join("+"), parts.len())
    } else {
        e.to_string()
    };
    brain::calc(&e)?;
    go(format!("calc {e}"))
}

const FIAT: &[(&str, &str)] = &[
    ("usd", "USD"), ("dollar", "USD"), ("dollars", "USD"), ("bucks", "USD"), ("$", "USD"), ("eur", "EUR"), ("euro", "EUR"),
    ("euros", "EUR"), ("€", "EUR"), ("chf", "CHF"), ("franc", "CHF"), ("francs", "CHF"), ("gbp", "GBP"), ("pound", "GBP"),
    ("pounds", "GBP"), ("quid", "GBP"), ("£", "GBP"), ("yen", "JPY"), ("jpy", "JPY"), ("¥", "JPY"), ("cad", "CAD"), ("aud", "AUD"),
    ("cny", "CNY"), ("yuan", "CNY"), ("rmb", "CNY"), ("inr", "INR"), ("rupee", "INR"), ("rupees", "INR"), ("sek", "SEK"),
    ("nok", "NOK"), ("dkk", "DKK"), ("krona", "SEK"), ("kronor", "SEK"), ("krone", "NOK"), ("pln", "PLN"), ("zloty", "PLN"),
    ("mxn", "MXN"), ("peso", "MXN"), ("pesos", "MXN"), ("brl", "BRL"), ("reais", "BRL"), ("czk", "CZK"), ("huf", "HUF"),
    ("forint", "HUF"), ("krw", "KRW"), ("hkd", "HKD"), ("sgd", "SGD"), ("nzd", "NZD"), ("zar", "ZAR"), ("rand", "ZAR"),
];
const COINS: &[(&str, &str)] = &[
    ("bitcoin", "bitcoin"), ("bitcoins", "bitcoin"), ("btc", "bitcoin"), ("ethereum", "ethereum"), ("eth", "ethereum"),
    ("ether", "ethereum"), ("dogecoin", "dogecoin"), ("doge", "dogecoin"), ("solana", "solana"), ("litecoin", "litecoin"),
    ("ltc", "litecoin"), ("cardano", "cardano"), ("xrp", "ripple"), ("ripple", "ripple"),
];

/// "100 dollars to euros", "$50 in chf", "bitcoin price", "how much is 1 eth in euros"
fn money(q: &str) -> Option<Reply> {
    let spaced = q.replace('$', " $ ").replace('€', " € ").replace('£', " £ ").replace('¥', " ¥ ");
    let words: Vec<&str> = spaced.split_whitespace().collect();
    let fiat: Vec<&str> = words.iter().filter_map(|w| FIAT.iter().find(|f| f.0 == *w).map(|f| f.1)).collect();
    if let Some(coin) = words.iter().find_map(|w| COINS.iter().find(|c| c.0 == *w).map(|c| c.1)) {
        return go(format!("crypto {coin} {}", fiat.first().unwrap_or(&"USD").to_lowercase()));
    }
    let amount = words.iter().find_map(|w| w.replace(',', "").parse::<f64>().ok());
    let from = *fiat.first()?;
    if amount.is_none() && !has(q, &["rate", "exchange rate", "worth", "value"]) {
        return None;
    }
    let to = fiat.iter().find(|c| **c != from).copied().unwrap_or(if from == "USD" { "EUR" } else { "USD" });
    go(format!("money {} {from} {to}", amount.unwrap_or(1.0)))
}

fn verbs(s: &str, raw: &str) -> Option<Reply> {
    if let Some(u) = after(s, &["shorten ", "short link for ", "make a short link for ", "make short ", "tiny url for ", "tinyurl "]) {
        return go(format!("shorten {}", keep(raw, u)));
    }
    let site = after(s, &["check if ", "check whether ", "is "]).and_then(|r| ["down", "up", "working", "online", "offline", "dead", "broken", "down right now", "down for everyone", "up right now"].iter().find_map(|e| r.strip_suffix(&format!(" {e}"))));
    if let Some(mid) = site.map(|m| m.trim_start_matches("the ").trim_end_matches(" website").trim_end_matches(" site")) {
        if !mid.is_empty() && !mid.contains(' ') && !["it", "my", "this", "that", "everything", "everyone", "internet", "wifi"].contains(&mid) {
            let host = if mid.contains('.') { mid.to_string() } else { format!("{mid}.com") };
            return go(format!("isdown {host}"));
        }
    }
    for (prefixes, cmd) in [
        (&["synonyms for ", "synonym for ", "synonyms of ", "synonym of ", "another word for ", "other words for ", "a word for ", "a different word for ", "similar words to ", "thesaurus "][..], "synonym"),
        (&["antonyms for ", "antonym for ", "antonyms of ", "antonym of ", "the opposite of ", "opposite of ", "opposites of "][..], "antonym"),
        (&["words that rhyme with ", "word that rhymes with ", "what rhymes with ", "something that rhymes with ", "rhymes with ", "rhyme with ", "rhymes for "][..], "rhyme"),
        (&["how do you spell ", "how do i spell ", "how to spell ", "spelling of ", "correct spelling of ", "spell "][..], "spell"),
    ] {
        if let Some(w) = after(s, prefixes).filter(|w| !w.is_empty()) {
            return go(format!("{cmd} {w}"));
        }
    }
    if let Some(q) = after(s, &["search the web for ", "search the internet for ", "search online for ", "search for ", "search ", "google for ", "google ", "look up ", "lookup ", "look for ", "find out ", "find information about ", "find info on ", "find info about ", "find me ", "find ", "web search ", "duckduckgo ", "bing "]) {
        return go(format!("search {q}"));
    }
    if let Some(w) = after(s, &["define ", "definition of ", "the definition of ", "meaning of the word ", "meaning of ", "the meaning of ", "dictionary "]).filter(|w| !w.contains("life")) {
        return go(format!("define {w}"));
    }
    if let Some(w) = s.strip_prefix("what does ").and_then(|r| r.strip_suffix(" mean")) {
        return go(format!("define {w}"));
    }
    if s.starts_with("translate ") || s.starts_with("how do you say ") || s.starts_with("how to say ") {
        return go(format!("search {s}"));
    }
    if let Some(q) = after(s, &["play ", "watch ", "listen to ", "put on ", "stream ", "videos of ", "a video of ", "video of ", "youtube ", "videos about ", "a video about "]) {
        return go(format!("youtube {q}"));
    }
    if let Some(q) = after(s, &["where is ", "where are ", "where can i find ", "directions to ", "how do i get to ", "how to get to ", "how far is ", "map of ", "a map of ", "on the map ", "locate ", "find on the map ", "navigate to ", "route to "]) {
        return go(format!("map {q}"));
    }
    if let Some(t) = after(s, &["open up ", "open ", "launch ", "start up ", "start ", "fire up ", "boot up ", "load ", "bring up ", "pull up ", "go to ", "visit ", "take me to ", "browse to ", "switch to ", "run the ", "show my "]) {
        let target = resolve(t, raw);
        let is_cmd = target.contains(['/', ':']) || in_path(target.split_whitespace().next().unwrap_or(""));
        if let Some((name, exec)) = (!is_cmd).then(|| find_app(t)).flatten() {
            return Some(Reply::say(format!("Opening {name}! Wheee!")).effect(brain::Effect::Open(format!("app:{exec}"))).mood(Mood::Happy));
        }
        return go(format!("open {target}"));
    }
    // "my pictures" (the "show me" was tidied away)
    if let Some(dir) = s.strip_prefix("my ").map(|t| resolve(t, raw)).filter(|d| std::path::Path::new(d).is_dir()) {
        return go(format!("open {dir}"));
    }
    if let Some(t) = after(s, &["close ", "quit ", "kill ", "exit ", "end ", "stop ", "shut down ", "smite ", "banish ", "destroy "]) {
        if ["clipster", "you", "yourself", "it", "this", ""].contains(&t) {
            return go("bye".into());
        }
        let name = crate::sys::program_name(t.split_whitespace().next()?);
        return Some(
            Reply::say(lines::pick("say_close").replace("{app}", &name))
                .mood(Mood::Shock)
                .run(&format!("Yes, close {name}"), &format!("run {}", crate::sys::close_command(&name)))
                .btn("No, leave it open", Act::Hide),
        );
    }
    None
}

/// Friendly names -> something the system can open: "my downloads", "the browser", "a terminal", "reddit".
fn tidy_target(t: &str) -> &str {
    let mut t = t.trim();
    for p in ["the ", "my ", "a ", "new ", "up "] {
        t = t.strip_prefix(p).unwrap_or(t);
    }
    for suf in [" app", " application", " program", " folder", " directory", " website", " site", " page", " window", " please"] {
        t = t.strip_suffix(suf).unwrap_or(t);
    }
    t
}

/// Look through the installed apps' menu entries (also Flatpak and Snap) for one called like `t`.
/// An installed app by name: its Start-menu shortcut on Windows.
#[cfg(windows)]
fn find_app(t: &str) -> Option<(String, String)> {
    use std::path::PathBuf;
    let t = tidy_target(t);
    if t.len() < 2 {
        return None;
    }
    let menu = |base: Option<PathBuf>| base.map(|b| b.join("Microsoft").join("Windows").join("Start Menu").join("Programs"));
    let mut dirs_left: Vec<PathBuf> = [menu(std::env::var_os("ProgramData").map(PathBuf::from)), menu(dirs::config_dir())].into_iter().flatten().collect();
    let mut best: Option<(u8, String, String)> = None;
    while let Some(dir) = dirs_left.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs_left.push(p);
                continue;
            }
            if !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) {
                continue;
            }
            let name = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            let n = name.to_lowercase();
            if n.contains("uninstall") || n.contains("readme") {
                continue;
            }
            let score = if n == t { 4 } else if n.starts_with(t) { 3 } else if n.split([' ', '-', '_']).any(|part| part == t) { 2 } else if n.contains(t) { 1 } else { 0 };
            if score > best.as_ref().map_or(0, |b| b.0) {
                best = Some((score, name, p.to_string_lossy().into_owned()));
            }
        }
    }
    best.map(|(_, name, link)| (name, link))
}

/// An installed app by name: its menu entry (.desktop file) on Linux.
#[cfg(not(windows))]
fn find_app(t: &str) -> Option<(String, String)> {
    use std::path::PathBuf;
    let t = tidy_target(t);
    if t.len() < 2 {
        return None;
    }
    let home = dirs::home_dir().unwrap_or_default();
    let places = [
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        home.join(".local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        home.join(".local/share/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];
    let mut best: Option<(u8, String, String)> = None;
    for e in places.iter().filter_map(|d| std::fs::read_dir(d).ok()).flatten().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "desktop") {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&p) else { continue };
        let field = |k: &str| body.lines().find_map(|l| l.strip_prefix(k)).map(|v| v.trim().to_string());
        let (Some(name), Some(exec)) = (field("Name="), field("Exec=")) else { continue };
        let n = name.to_lowercase();
        let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
        let score = if n == t { 4 } else if n.starts_with(t) { 3 } else if stem.split(['.', '-', '_']).any(|part| part == t) { 2 } else if n.contains(t) { 1 } else { 0 };
        if score > best.as_ref().map_or(0, |b| b.0) {
            best = Some((score, name, exec));
        }
    }
    let (_, name, exec) = best?;
    // drop the menu's placeholders like %u / %F
    let exec = exec.split_whitespace().filter(|w| !(w.starts_with('%') && w.len() == 2)).collect::<Vec<_>>().join(" ");
    Some((name, exec))
}

/// Named places and programs on Windows ("open the calculator", "open the trash").
fn windows_place(t: &str) -> Option<String> {
    Some(
        match t {
            "trash" | "recycle bin" | "bin" => "shell:RecycleBinFolder",
            "terminal" | "console" | "command line" | "shell" | "command prompt" | "powershell" => if in_path("wt") { "wt" } else { "powershell" },
            "calculator" | "calc" => "calc",
            "text editor" | "editor" | "notepad" => "notepad",
            "settings" | "system settings" | "control panel" => "ms-settings:",
            "file manager" | "file explorer" | "explorer" => "explorer",
            "paint" => "mspaint",
            "task manager" => "taskmgr",
            "word" | "word processor" | "writer" => "winword",
            "excel" | "spreadsheet" | "spreadsheets" => "excel",
            "powerpoint" | "presentation" | "slides" => "powerpnt",
            "chrome" | "google chrome" => "chrome",
            "edge" => "msedge",
            _ => return None,
        }
        .to_string(),
    )
}

fn resolve(t: &str, raw: &str) -> String {
    let t = tidy_target(t);
    // without XDG user-dirs set up, `dirs` knows no Downloads folder: use ~/Downloads rather than downloads.com
    let dir = |d: Option<std::path::PathBuf>, name: &str| d.or_else(|| dirs::home_dir().map(|h| h.join(name))).map(|p| p.to_string_lossy().into_owned());
    let place = if cfg!(windows) { windows_place(t) } else { None }.or_else(|| match t {
        "downloads" | "download" => dir(dirs::download_dir(), "Downloads"),
        "documents" | "docs" | "document" => dir(dirs::document_dir(), "Documents"),
        "pictures" | "photos" | "images" | "pics" => dir(dirs::picture_dir(), "Pictures"),
        "music" | "songs" => dir(dirs::audio_dir(), "Music"),
        "videos" | "movies" => dir(dirs::video_dir(), "Videos"),
        "desktop" => dir(dirs::desktop_dir(), "Desktop"),
        "home" | "home folder" | "files" | "file manager" | "file explorer" | "explorer" | "finder" => dir(dirs::home_dir(), ""),
        "trash" | "recycle bin" | "bin" => Some("trash:///".into()),
        "browser" | "web browser" | "internet" | "web" => Some("https://duckduckgo.com".into()),
        "email" | "mail" | "inbox" => first_in_path(&["thunderbird", "evolution", "kmail", "geary"]).or(Some("mailto:".into())),
        "terminal" | "console" | "command line" | "shell" | "command prompt" => first_in_path(&["konsole", "gnome-terminal", "kgx", "kitty", "alacritty", "wezterm", "xfce4-terminal", "xterm"]),
        "calculator" | "calc" => first_in_path(&["kcalc", "gnome-calculator", "galculator", "qalculate-gtk"]),
        "text editor" | "editor" | "notepad" => first_in_path(&["kate", "gnome-text-editor", "gedit", "mousepad", "kwrite", "xed"]),
        "settings" | "system settings" | "control panel" => first_in_path(&["systemsettings", "gnome-control-center", "xfce4-settings-manager"]),
        "word" | "word processor" | "writer" => Some("libreoffice --writer".into()),
        "excel" | "spreadsheet" | "spreadsheets" => Some("libreoffice --calc".into()),
        "powerpoint" | "presentation" | "slides" => Some("libreoffice --impress".into()),
        "vs code" | "vscode" | "visual studio code" => Some("code".into()),
        "chrome" | "google chrome" => first_in_path(&["google-chrome-stable", "google-chrome", "chromium"]),
        _ => None,
    });
    if let Some(p) = place {
        return p;
    }
    let one = t.replace(' ', "-");
    if in_path(&one) || t.contains(['/', '.', '~']) {
        return keep(raw, t);
    }
    if let Some(first) = t.split_whitespace().next().filter(|f| in_path(f)) {
        return first.to_string(); // "obs studio" -> obs
    }
    if !t.contains(' ') && !t.is_empty() {
        return format!("{t}.com"); // "open reddit"
    }
    keep(raw, t)
}

fn text_tools(s: &str, raw: &str) -> Option<Reply> {
    const P: &[(&str, &str)] = &[
        ("uppercase ", "upper"), ("upper case ", "upper"), ("capitalize ", "upper"), ("all caps ", "upper"), ("caps ", "upper"),
        ("shout ", "upper"), ("lowercase ", "lower"), ("lower case ", "lower"), ("reverse ", "reverse"), ("backwards ", "reverse"),
        ("mock ", "mock"), ("mockify ", "mock"), ("spongebob ", "mock"), ("sarcastic ", "mock"), ("title case ", "title"),
        ("count the words in ", "count"), ("count words in ", "count"), ("count words ", "count"), ("count the characters in ", "count"),
        ("count characters in ", "count"), ("count letters in ", "count"), ("word count of ", "count"), ("word count ", "count"),
        ("count ", "count"), ("how many words are in ", "count"), ("how many words in ", "count"), ("how many characters are in ", "count"),
        ("how many characters in ", "count"), ("how many letters are in ", "count"), ("how many letters in ", "count"),
        ("base64 decode ", "unbase64"), ("decode base64 ", "unbase64"), ("decode the base64 ", "unbase64"), ("from base64 ", "unbase64"),
        ("base64 encode ", "base64"), ("encode base64 ", "base64"), ("to base64 ", "base64"), ("base64 ", "base64"),
        ("url encode ", "urlencode"), ("urlencode ", "urlencode"), ("url decode ", "urldecode"), ("urldecode ", "urldecode"),
        ("slugify ", "slug"), ("slug ", "slug"), ("make a slug of ", "slug"), ("sha256 ", "sha256"), ("sha256 hash of ", "sha256"), ("hash ", "sha256"),
        ("sort lines ", "sortlines"), ("sort the lines ", "sortlines"), ("sort my clipboard", "sortlines"), ("dedupe ", "dedupe"), ("dedupe lines ", "dedupe"),
        ("remove duplicate lines ", "dedupe"), ("remove duplicates ", "dedupe"), ("remove duplicates from ", "dedupe"),
    ];
    for (p, kind) in P {
        if let Some(t) = after(s, &[p]) {
            return go(format!("{kind} {}", keep(raw, t)));
        }
    }
    // "make this uppercase", "turn the clipboard backwards"
    let r = after(s, &["make ", "turn ", "convert ", "change ", "put "])?;
    const K: &[(&str, &str)] = &[
        (" uppercase", "upper"), (" upper case", "upper"), (" all caps", "upper"), (" caps", "upper"), (" louder", "upper"),
        (" lowercase", "lower"), (" lower case", "lower"), (" quieter", "lower"), (" backwards", "reverse"), (" reversed", "reverse"),
        (" sarcastic", "mock"), (" mocking", "mock"), (" title case", "title"),
    ];
    let (what, kind) = K.iter().find_map(|(k, kind)| {
        let w = r.strip_suffix(k)?;
        Some((w.trim_end_matches(" into").trim_end_matches(" to").trim_end_matches(" in").trim(), *kind))
    })?;
    go(format!("{kind} {}", keep(raw, what)))
}

fn chance(s: &str) -> Option<Reply> {
    if has(s, &["flip a coin", "coin flip", "toss a coin", "coin toss", "heads or tails", "flip coin", "flip", "coin"]) {
        return go("flip".into());
    }
    if has(s, &["roll", "dice", "throw a die", "roll a die"]) {
        let spec = s.split_whitespace().find(|w| w.split_once('d').is_some_and(|(a, b)| a.chars().all(|c| c.is_ascii_digit()) && !b.is_empty() && b.chars().all(|c| c.is_ascii_digit())));
        let n = s.split_whitespace().find_map(|w| w.parse::<u32>().ok()).unwrap_or(1);
        return go(format!("roll {}", spec.map(String::from).unwrap_or(format!("{n}d6"))));
    }
    if has(s, &["random number", "pick a number", "number between", "random between", "a random", "rng", "number from"]) {
        return go(format!("random {s}"));
    }
    if has(s, &["password", "passwords", "passphrase", "passcode", "pass word"]) {
        let n = s.split_whitespace().find_map(|w| w.parse::<u32>().ok()).unwrap_or(16);
        return go(format!("password {n}"));
    }
    if let Some(list) = after(s, &["pick between ", "choose between ", "decide between ", "pick one of ", "choose one of ", "pick from ", "choose from ", "pick ", "choose ", "which one ", "which should i pick ", "which should i choose "]) {
        return go(format!("pick {}", list.replace(" and ", ", ").replace(" or ", ", ")));
    }
    if has(s, &["magic 8", "8 ball", "8ball", "eight ball", "yes or no", "fortune", "predict", "prophecy"]) {
        return go("8ball".into());
    }
    None
}

fn chat(s: &str) -> Option<Reply> {
    if has(s, &["what can you do", "what do you do", "help", "commands", "how do you work", "how does this work", "features", "abilities", "skills", "menu", "what else", "instructions", "how to use you", "manual", "what can i ask", "what can i say"]) {
        return go("help".into());
    }
    if has(s, &["options", "preferences", "configure you", "your settings", "settings"]) {
        return go("options".into());
    }
    if has(s, &["who are you", "your name", "what are you", "about yourself", "about you", "who made you", "who created you", "who built you"]) {
        return go("about".into());
    }
    if [
        "hi", "hello", "hey", "yo", "howdy", "hiya", "greetings", "good morning", "good afternoon", "good evening", "morning", "hello there", "hi there", "hey there", "heya", "hallo", "hola", "bonjour",
    ].contains(&s) {
        return go("hello".into());
    }
    if ["rather", "would you rather"].contains(&s) {
        return go("wyr".into()); // "would you" is tidied away as politeness
    }
    if has(s, &["thank", "thanks", "cheers", "appreciate it", "ty", "thank you"]) {
        return go("thanks".into());
    }
    if has(s, &["letter", "write an email", "write a email", "cover letter", "formal letter"]) {
        return Some(Reply::say("It looks like you're writing a letter! Here's a classic opening I prepared earlier:").mood(Mood::Happy).btn("Copy a letter template", Act::Copy(brain::LETTER.into())));
    }
    for (kws, cmd) in [
        (&["joke", "jokes", "funny", "laugh", "bored", "humor", "humour", "entertain me", "make me smile", "cheer me up", "something fun", "lol"][..], "joke"),
        (&["roast", "insult me", "make fun of me", "burn me", "be mean", "diss me", "trash talk"][..], "roast"),
        (&["fact", "facts", "something interesting", "teach me something", "did you know", "something i do not know", "tell me something", "learn something"][..], "fact"),
        (&["compliment", "something nice", "i am sad", "i feel sad", "feeling down", "feel bad", "bad day", "hug", "lonely", "encourage me", "sad"][..], "hug"),
        (&["quiz", "quiz me", "trivia", "test me", "ask me a question", "trivia question"][..], "quiz"),
        (&["quote", "quotes", "inspire me", "inspiration", "inspirational", "motivate me", "motivation", "motivational", "wise words", "wisdom"][..], "quote"),
        (&["advice", "give me advice", "life advice"][..], "advice"),
        (&["riddle", "riddles", "riddle me", "a riddle"][..], "riddle"),
        (&["would you rather", "wyr", "play would you rather", "ask me would you rather"][..], "wyr"),
        (&["emoji quiz", "emoji game", "guess the emoji", "emoji puzzle", "emoji riddle"][..], "emoji quiz"),
        (&["fortune", "fortune cookie", "my fortune", "tell my fortune", "read my fortune"][..], "fortune"),
        (&["shower thought", "shower thoughts", "deep thought", "random thought", "say something deep"][..], "thought"),
        (&["pun", "puns", "a pun"][..], "pun"),
        (&["do you like me", "are we friends", "how much do you like me", "friendship level", "are we best friends", "our friendship", "friendship", "my faith", "how is my faith", "am i chosen", "faith level"][..], "friendship"),
        (&["all memories", "all your memories", "everything you remember", "what you remember"][..], "backstory all"),
        (&["tell me about your past", "your past", "your memories", "what happened to you", "your backstory", "tell me a memory", "a memory", "remember something", "your history", "memories"][..], "backstory"),
        (&["your diary", "read your diary", "read me your diary", "diary entry", "dear diary", "diary"][..], "diary"),
        (&["commandments", "the commandments", "a commandment", "your commandments", "your rules"][..], "commandments"),
        (&["prophecy", "a prophecy", "prophesy", "predict my future", "what does the future hold", "oracle", "my future"][..], "prophecy"),
        (&["your mood", "what mood are you in", "mood today", "what is your mood", "today's mood"][..], "mood"),
        (&["be boring", "boring mode", "be normal", "normal mode", "be sane", "stop being weird", "tone it down", "boring personality"][..], "persona boring"),
        (&["be unhinged", "unhinged mode", "go unhinged", "be weird", "god mode", "unleash yourself", "unhinged personality"][..], "persona unhinged"),
        (&["dance", "party", "spin", "celebrate", "boogie", "do a trick", "trick", "do something", "show off"][..], "dance"),
        (&["tip", "tips", "productivity", "be productive", "procrastinating"][..], "tip"),
        (&["shut up", "be quiet", "go away", "leave me alone", "stop talking", "silence", "mute", "shh", "stfu", "quiet", "hush", "pipe down", "stop bothering me", "not now", "annoying me"][..], "quiet"),
    ] {
        if has(s, kws) {
            return go(cmd.into());
        }
    }
    if ["bye", "goodbye", "good bye", "see you", "see ya", "quit", "exit", "close", "farewell", "cya", "see you later", "bye bye", "close yourself", "turn yourself off", "go to sleep"].contains(&s) {
        return go("bye".into());
    }
    None
}

fn system(s: &str) -> Option<Reply> {
    let place_after = |s: &str| [" in ", " for ", " at "].iter().filter_map(|k| s.rsplit_once(k).map(|x| x.1.to_string())).next().unwrap_or_default();
    if let Some((_, p)) = ["time in ", "time is it in ", "time is in ", "clock in ", "time over in "].iter().find_map(|k| s.split_once(k)) {
        return go(format!("time {p}"));
    }
    if has(s, &["sunrise", "sunset", "sun rise", "sun set", "sun go down", "sun come up", "sundown", "dawn", "dusk", "sun up", "sun down"]) {
        return go(format!("sun {}", place_after(s).trim_end_matches(" today")));
    }
    let words = s.split_whitespace().count();
    for (kws, cmd) in [
        (&["tech news", "hacker news", "technology news", "tech headlines"][..], "news tech"),
        (&["on this day", "this day in history", "today in history", "history today", "happened on this day", "historical events", "happened today in history"][..], "onthisday"),
        (&["news", "headlines", "current events", "what is happening in the world", "what is going on in the world"][..], "news"),
        (&["iss", "space station", "international space station", "astronauts", "people in space"][..], "iss"),
        (&["check for updates", "check for update", "check for a new version", "is there an update", "is there a new version", "update clipster", "update yourself", "new version of clipster", "what version are you", "which version are you", "your version", "clipster version"][..], "updates"),
        (&["holiday", "holidays", "public holiday", "bank holiday", "day off", "days off"][..], "holiday"),
        (&["local ip", "lan", "local network", "wifi address", "network interfaces"][..], "lan"),
        (&["public ip", "where am i", "my location", "my ip", "ip address", "ip", "my isp", "internet provider"][..], "ip"),
    ] {
        if has(s, kws) {
            return go(cmd.into());
        }
    }
    let picture = words <= 3 || has(s, &["picture", "pic", "pics", "photo", "photos", "image", "gif", "random"]);
    for (kws, kind) in [(&["dog", "dogs", "puppy", "puppies", "doggo", "doggy", "pupper"][..], "dog"), (&["cat", "cats", "kitten", "kittens", "kitty"][..], "cat"), (&["fox", "foxes"][..], "fox")] {
        if picture && has(s, kws) {
            return go(kind.into());
        }
    }
    for (kws, cmd) in [
        (&["what time", "the time", "time is it", "what day", "which day", "what date", "the date", "todays date", "today is", "what year", "day is it", "what month", "clock", "time now", "current time", "time"][..], "time"),
        (&["battery", "charge left", "charging", "power left", "plugged in", "how much power"][..], "battery"),
        (&["disk", "disks", "storage", "space left", "free space", "hard drive", "ssd", "drive space", "disk space"][..], "disk"),
        (&["network", "wifi", "wi fi", "internet connection", "am i online", "connected"][..], "lan"),
        (&["uptime", "how long has my computer", "been on for", "been running", "since boot", "last reboot"][..], "uptime"),
        (&["hogging", "using all", "using the most", "slowing down", "slowing", "what is using", "who is using", "most cpu", "task manager", "processes", "greedy", "eating my"][..], "top"),
        (&["lock the screen", "lock screen", "lock my computer", "lock the computer", "lock my pc", "lock", "lock it"][..], "lock"),
        (&["ram", "memory", "cpu", "processor", "computer doing", "pc doing", "system", "performance", "slow", "health", "how is my computer", "how is my pc", "specs", "load", "stats", "status", "computer ok"][..], "stats"),
    ] {
        if has(s, kws) {
            return go(cmd.into());
        }
    }
    if has(s, &["weather", "forecast", "rain", "raining", "sunny", "snow", "snowing", "temperature outside", "how hot is it", "how cold is it", "hot outside", "cold outside", "umbrella", "degrees outside", "jacket"]) {
        let place = [" in ", " for ", " at "].iter().filter_map(|k| s.rsplit_once(k).map(|x| x.1)).next().unwrap_or("");
        let place = place.trim_end_matches(" today").trim_end_matches(" tomorrow").trim_end_matches(" this week");
        let place = if ["today", "tomorrow", "the morning", "the evening", "this week", "outside"].contains(&place) { "" } else { place };
        return go(format!("weather {place}"));
    }
    None
}

fn questions(s: &str, raw: &str) -> Option<Reply> {
    if let Some(q) = after(s, &["tell me about ", "who is ", "who was ", "who are ", "who were ", "who invented ", "who made ", "who wrote ", "who discovered ", "who painted ", "what is a ", "what is an ", "what is the ", "what is ", "what are ", "what was ", "what were ", "history of ", "explain ", "info on ", "information about ", "wikipedia "]) {
        if !q.is_empty() {
            let topic = if s.starts_with("who invented ") || s.starts_with("who made ") || s.starts_with("who wrote ") || s.starts_with("who discovered ") || s.starts_with("who painted ") { s } else { q };
            return go(format!("ask {}", keep(raw, topic)));
        }
    }
    if ["how ", "why ", "when ", "which ", "can i ", "is there ", "what "].iter().any(|p| s.starts_with(p)) && s.split_whitespace().count() >= 3 {
        return go(format!("ask {s}"));
    }
    // "pizza or tacos"
    if s.contains(" or ") && s.split_whitespace().count() <= 10 {
        let body = after(s, &["should i ", "should we ", "do i want ", "is it ", "which is better ", "would you rather "]).unwrap_or(s);
        return go(format!("pick {}", body.replace(" or ", ", ")));
    }
    if ["should ", "will ", "shall ", "am i ", "is it ", "are we ", "do i ", "does ", "did ", "would ", "is my ", "are you going "].iter().any(|p| s.starts_with(p)) || raw.trim_end().ends_with('?') && s.split_whitespace().count() <= 8 && ["is ", "are ", "can ", "will "].iter().any(|p| s.starts_with(p)) {
        return go("8ball".into());
    }
    None
}

// ---------- typos ----------

const VOCAB: &[&str] = &[
    "timer", "timers", "alarm", "remind", "reminder", "countdown", "minute", "minutes", "second", "seconds", "hour", "hours",
    "note", "notes", "remember", "search", "google", "open", "launch", "start", "play", "watch", "weather", "forecast",
    "battery", "memory", "processor", "computer", "joke", "jokes", "funny", "roast", "fact", "facts", "password", "coin",
    "dice", "roll", "flip", "random", "number", "convert", "calculate", "translate", "define", "uppercase", "lowercase",
    "reverse", "count", "words", "clipboard", "help", "quiet", "wikipedia", "youtube", "music", "downloads", "documents",
    "pictures", "desktop", "browser", "terminal", "calculator", "settings", "compliment", "dance", "uptime", "storage",
    "network", "address", "lock", "screen", "time", "date", "today", "kilometers", "miles", "celsius", "fahrenheit",
    "pounds", "kilograms", "inches", "feet", "meters", "gallons", "liters", "plus", "minus", "times", "divided", "percent",
    "square", "root", "average", "what", "where", "when", "which", "tell", "show", "please", "thanks", "hello", "goodbye",
    "tomorrow", "something", "interesting", "choose", "pick", "between", "delete", "clear", "cancel", "folder", "email",
    "directions", "location", "temperature", "about", "should", "could", "would", "write", "letter", "about", "sarcastic",
];

/// Damerau-Levenshtein (adjacent swaps count once).
pub fn dist(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 0..=a.len() {
        d[i][0] = i;
    }
    for j in 0..=b.len() {
        d[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = (a[i - 1] != b[j - 1]) as usize;
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

pub fn fix_typos(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            if w.len() < 4 || VOCAB.contains(&w) || !w.chars().all(|c| c.is_ascii_alphabetic()) {
                return w.to_string();
            }
            let max = if w.len() >= 7 { 2 } else { 1 };
            VOCAB.iter().map(|v| (dist(w, v), *v)).filter(|(d, _)| *d <= max).min_by_key(|(d, _)| *d).map_or(w.to_string(), |(_, v)| v.to_string())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------- when nothing fits ----------

const GUESSES: &[(&[&str], &str, &str)] = &[
    (&["time", "date", "day", "clock", "today"], "Tell you the time", "time"),
    (&["laugh", "joke", "funny", "bored", "fun"], "Tell you a joke", "joke"),
    (&["computer", "slow", "cpu", "memory", "ram", "battery", "disk"], "Check how your computer is doing", "stats"),
    (&["timer", "alarm", "remind", "minutes", "later", "wait"], "Set a 5-minute timer", "timer 5m"),
    (&["note", "notes", "write", "remember", "list", "save"], "Show your notes", "notes"),
    (&["open", "app", "program", "launch", "folder", "files"], "Open your home folder", "open home"),
    (&["math", "calculate", "number", "plus", "sum", "percent"], "Do some math (try: 15% of 80)", "15% of 80"),
    (&["weather", "rain", "sun", "cold", "hot"], "Check the weather", "weather"),
    (&["music", "song", "video", "listen", "watch"], "Find something to watch", "youtube lofi"),
];

fn fallback(raw: &str, s: &str) -> Reply {
    let words: Vec<&str> = s.split_whitespace().collect();
    let question = raw.ends_with('?') || ["what", "who", "why", "how", "when", "where", "which", "is", "are", "can", "does", "do"].contains(&words.first().copied().unwrap_or(""));
    let mut scored: Vec<(usize, &str, &str)> = GUESSES
        .iter()
        .map(|(kws, label, cmd)| (words.iter().filter(|w| kws.iter().any(|k| dist(w, k) <= (w.len() > 4) as usize)).count(), *label, *cmd))
        .filter(|g| g.0 > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    // the internet only gets real lookups: encyclopedia questions, or a bare name or thing ("Ada Lovelace", "platypus")
    let personal = words.iter().any(|w| ["i", "me", "my", "you", "your", "we", "our", "us"].contains(w));
    if knowledge(s) || (!personal && words.len() <= 3 && scored.is_empty()) {
        return exact(&format!("ask {raw}")).unwrap_or_else(|| Reply::say("Hmm?"));
    }
    // anything else is conversation he has no rule for: answer in character, offer the web as an option
    let mut r = crate::lines::reply_with(if question { "unknown_question" } else { "unknown_statement" }, &tidy_words(raw));
    if !scored.is_empty() {
        r.text += "\n\nOr were you after one of these?";
        for (_, label, cmd) in scored.iter().take(2) {
            r = r.run(label, cmd);
        }
    }
    r.mood(Mood::Think)
        .run("Ask the internet instead", &format!("ask {raw}"))
        .btn("Search the web for it", Act::Open(format!("https://duckduckgo.com/?q={}", url_encode(raw))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::Effect;
    use crate::net::{Kind, Query};

    fn eff(s: &str) -> Effect {
        respond(s).effect
    }
    fn text(s: &str) -> String {
        respond(s).text
    }

    #[test]
    fn tidying() {
        assert_eq!(clean("Hey Clipster, could you PLEASE tell me a joke?"), "tell me a joke");
        assert_eq!(clean("set a timer for twenty five minutes please"), "set a timer for 25 minutes");
        assert_eq!(clean("remind me in half an hour"), "remind me in 30 minutes");
        assert_eq!(dist("tiemr", "timer"), 1);
        assert_eq!(fix_typos("set a tiemr for 5 minutes"), "set a timer for 5 minutes");
    }

    #[test]
    fn timers_in_plain_words() {
        assert_eq!(eff("set a timer for 5 minutes"), Effect::Timer(300, "Time's up!".into()));
        assert_eq!(eff("Could you set an alarm for ten minutes to check the oven?"), Effect::Timer(600, "check the oven".into()));
        assert_eq!(eff("remind me to call Mom in an hour"), Effect::Timer(3600, "call Mom".into()));
        assert_eq!(eff("in 2 minutes remind me to stretch"), Effect::Timer(120, "stretch".into()));
        assert_eq!(eff("start a 15 minute timer for pasta"), Effect::Timer(900, "pasta".into()));
        assert_eq!(eff("wake me up in half an hour"), Effect::Timer(1800, "Time's up!".into()));
        assert!(matches!(eff("remind me at 5pm to leave"), Effect::Timer(s, l) if s <= 86400 && l == "leave"));
        assert_eq!(eff("cancel all my timers"), Effect::CancelTimers);
        assert_eq!(eff("set a tiemr for 5 minuts"), Effect::Timer(300, "Time's up!".into()));
    }

    #[test]
    fn maths_and_units_in_plain_words() {
        assert!(text("what's 5 plus 3?").ends_with("= 8"));
        assert!(text("what is the square root of 81").ends_with("= 9"));
        assert!(text("how much is 20 percent of 150").ends_with("= 30"));
        assert!(text("add 5 and 7").ends_with("= 12"));
        assert!(text("average of 2, 4 and 6").ends_with("= 4"));
        assert!(text("convert 10 kilometers into miles").contains("6.2137"));
        assert!(text("how many feet are in a mile").contains("5280"));
        assert!(text("what is 100 degrees fahrenheit in celsius").contains("37.7778"));
        assert!(text("five km to miles").contains("3.1069"));
    }

    #[test]
    fn new_powers_in_plain_words() {
        // teaching is only parsed here; the file is written later, by the app
        assert_eq!(eff("When I say gm, say Good morning, sunshine!"), Effect::Teach("gm".into(), "Good morning, sunshine!".into(), vec![]));
        assert_eq!(eff("when i say work, open firefox and open code"), Effect::Teach("work".into(), String::new(), vec!["open firefox".into(), "open code".into()]));
        if cfg!(windows) {
            // Windows only lets people flip these switches themselves, so he opens the right page
            assert!(matches!(eff("turn off the wifi"), Effect::Open(u) if u.contains("network-wifi")));
            assert!(matches!(eff("bluetooth on"), Effect::Open(u) if u.contains("bluetooth")));
        } else {
            assert_eq!(eff("turn off the wifi"), Effect::Shell("nmcli radio wifi off".into()));
            assert_eq!(eff("bluetooth on"), Effect::Shell("bluetoothctl power on".into()));
        }
        assert_eq!(eff("what's my wifi name"), eff("wifiname"));
        assert!(text("what's 255 in hex").ends_with("0xFF"));
        assert!(text("2024 in roman numerals").ends_with("MMXXIV"));
        assert!(text("how much should I tip on 48").contains("15% tip"));
        assert!(text("split 90 between 3 people").contains("30.00 each"));
        assert!(text("1700000000 to date").contains("2023"));
        assert!(text("how many days between 2026-01-01 and 2026-01-31").contains("30 days"));
        assert!(text("base64 hello").contains("aGVsbG8="));
        assert!(text("tell me a riddle").starts_with("🧩"));
        assert!(text("would you rather").starts_with("🤔"));
        assert_eq!(eff("my birthday is june 3"), Effect::SetBirthday(6, 3));
        assert_eq!(eff("play chess"), Effect::Play("chess".into()));
        assert_eq!(eff("start the tour"), Effect::Onboard(String::new()));
        assert_eq!(eff("onboard persona boring"), Effect::Onboard("persona boring".into()), "taken literally, not as 'be boring'");
        assert_eq!(eff("onboard name Help"), Effect::Onboard("name Help".into()));
        assert_eq!(eff("wear a halo"), Effect::Hat("halo".into()));
        assert_eq!(eff("turn yourself beige"), Effect::Skin("beige".into()));
        assert_eq!(eff("let's play minesweeper"), Effect::Play("bughunt".into()));
        assert_eq!(eff("i want to play chess"), Effect::Play("chess".into()));
        assert_eq!(eff("let's play 2048"), Effect::Play("bytesize".into()));
        assert_eq!(eff("check for updates"), Effect::Net(Query::Update));
        assert_eq!(eff("is there a new version?"), Effect::Net(Query::Update));
        assert!(!matches!(eff("2048 in hex"), Effect::Play(_)), "a number is not a game");
        assert_eq!(eff("I lost at chess"), Effect::None, "small talk, not a game");
        assert_eq!(eff("what have you learned"), Effect::None);
        assert!(text("I want to kill myself").contains("helpline"));
        assert!(!text("I'm still working on it").contains("Until when"));
    }

    #[test]
    fn everything_else_in_plain_words() {
        assert!(matches!(eff("can you look up how tall is the eiffel tower"), Effect::Open(u) if u.contains("duckduckgo")));
        assert_eq!(eff("who is Ada Lovelace?"), Effect::Net(Query::Ask("Ada Lovelace".into())));
        assert_eq!(eff("what's the weather like in Paris"), Effect::Net(Query::Place(Kind::Weather, "paris".into())));
        assert_eq!(eff("what time is it in Tokyo"), Effect::Net(Query::Place(Kind::Time, "tokyo".into())));
        assert_eq!(eff("convert 100 dollars to euros"), Effect::Net(Query::Money(100.0, "USD".into(), "EUR".into())));
        assert_eq!(eff("how much is $50 in swiss francs"), Effect::Net(Query::Money(50.0, "USD".into(), "CHF".into())));
        assert_eq!(eff("bitcoin price in euros"), Effect::Net(Query::Crypto("bitcoin".into(), "eur".into())));
        assert_eq!(eff("is github down?"), Effect::Net(Query::IsDown("github.com".into())));
        assert_eq!(eff("another word for happy"), Effect::Net(Query::Words("rel_syn", "happy".into())));
        assert_eq!(eff("what rhymes with orange"), Effect::Net(Query::Words("rel_rhy", "orange".into())));
        assert_eq!(eff("show me a dog"), Effect::Net(Query::Animal("dog".into())));
        assert_eq!(eff("quiz me"), Effect::Net(Query::Trivia));
        assert_eq!(eff("what's in the news"), Effect::Net(Query::News(false)));
        assert_eq!(eff("why is the sky blue"), Effect::Net(Query::Ask("why is the sky blue".into())));
        assert!(matches!(eff("5 pounds to kg"), Effect::None)); // still a weight, not money
        assert!(matches!(eff("play some lofi music"), Effect::Open(u) if u.contains("youtube")));
        assert!(matches!(eff("where is Zurich"), Effect::Open(u) if u.contains("openstreetmap")));
        assert!(matches!(eff("please open my downloads folder"), Effect::Open(u) if u.ends_with("Downloads")));
        assert!(matches!(eff("go to reddit"), Effect::Open(u) if u == "https://reddit.com"));
        assert_eq!(eff("what does serendipity mean"), Effect::Net(Query::Define("serendipity".into())));
        assert!(text("how much battery do I have left").to_lowercase().contains("battery"));
        assert!(text("I'm bored").len() > 10);
        assert!(text("flip a coin for me").ends_with('!'));
        assert!(text("give me a random number between 1 and 6").contains("between 1 and 6"));
        assert!(text("should I eat pizza or tacos").contains("I choose"));
        assert!(text("generate a 24 character password").lines().nth(1).is_some_and(|l| l.len() == 24));
        assert!(text("what can you do").contains("Math"));
        assert_eq!(keep("Note that Bob's birthday is Friday", "bob's birthday is friday"), "Bob's birthday is Friday");
        assert!(text("make HELLO lowercase").starts_with("hello"));
        assert!(matches!(eff("asdkjh qwe"), Effect::Net(Query::Ask(_))));
        assert_eq!(eff("find the file called Report"), Effect::Net(Query::Files("Report".into())));
        // the same requests, each OS's way: PipeWire and KDE on Linux, media keys, WMI and the snipping overlay on Windows
        let shell_has = |s: &str, linux: &str, windows: &str| matches!(eff(s), Effect::Shell(c) if c.contains(if cfg!(windows) { windows } else { linux }));
        assert!(shell_has("it's too loud", "10%-", "[char]174"));
        assert!(shell_has("set the volume to 30", "30%", "1..15 "));
        assert!(shell_has("skip this song", "Player.Next", "[char]176"));
        assert!(shell_has("make the screen darker", "-0.15", "$c - 15"));
        if cfg!(windows) {
            assert_eq!(eff("take a screenshot of an area"), Effect::Open("ms-screenclip:".into()));
        } else {
            assert!(shell_has("take a screenshot of an area", " -r ", ""));
        }
        assert!(text("restart my computer").contains("restart the computer"));
        assert_eq!(eff("what's on my clipboard history"), Effect::ClipHistory);
        assert_eq!(eff("start the stopwatch"), Effect::Stopwatch("start"));
        assert!(text("how many days until christmas").contains("Christmas"));
        assert!(text("what date is in 90 days").starts_with("📅 In 90 days"));
        assert!(text("countdown to new year").contains("New year"));
        assert!(matches!(eff("shrug"), Effect::CopyNow(f) if f.contains("ツ")));
        assert!(matches!(eff("make a qr code for https://example.com"), Effect::Open(u) if u.contains("qrserver")));
        assert!(matches!(eff("shut down firefox"), Effect::None)); // closes the app, not the computer
        assert_eq!(eff("set a timer for 5 minutes"), Effect::Timer(300, "Time's up!".into()));
        assert_eq!(eff("remind me every hour to drink water"), Effect::Repeat(3600, "drink water".into()));
        assert_eq!(eff("every 20 minutes remind me to look away from the screen"), Effect::Repeat(1200, "look away from the screen".into()));
        assert_eq!(eff("call me Captain Nils"), Effect::SetName("Captain Nils".into()));
        assert_eq!(eff("what's my name?"), Effect::SayName);
        assert_eq!(eff("wear a wizard hat"), Effect::Hat("wizard".into()));
        assert_eq!(eff("put on your christmas hat"), Effect::Hat("santa".into()));
        assert_eq!(eff("take off your hat"), Effect::Hat("none".into()));
        assert_eq!(eff("turn gold"), Effect::Skin("gold".into()));
        assert_eq!(eff("make yourself pink"), Effect::Skin("rose".into()));
        assert_eq!(eff("start with my computer"), Effect::Autostart(true));
        assert_eq!(eff("don't start automatically"), Effect::Autostart(false));
        assert_eq!(eff("good morning"), Effect::Net(Query::Briefing));
        assert_eq!(eff("surprise me"), Effect::Net(Query::RandomWiki));
        assert!(text("SOS in morse code").starts_with("... --- ..."));
        assert!(text("let's play a game").contains("Pick a game"));
        assert!(text("rock").starts_with("You: rock."));
        assert!(text("what is a hat").len() > 0); // a question, not a costume change
        assert!(!matches!(eff("what is a hat"), Effect::Hat(_)));
        assert_eq!(eff("how do i unplug my pc"), Effect::Net(Query::Ask("how do i unplug my pc".into())));
        assert_eq!(eff("tell me a programming joke"), Effect::Net(Query::Joke("dev".into())));
        assert_eq!(eff("were there any earthquakes today?"), Effect::Net(Query::Quakes));
        assert_eq!(eff("how is the air quality in Paris"), Effect::Net(Query::Air("paris".into())));
        assert_eq!(eff("recommend a book about space"), Effect::Net(Query::Book("space".into())));
        assert_eq!(eff("tell me about the show severance"), Effect::Net(Query::Show("severance".into())));
        assert_eq!(eff("what should I cook tonight?"), Effect::Net(Query::Recipe("".into())));
        assert_eq!(eff("recipe for lasagne"), Effect::Net(Query::Recipe("lasagne".into())));
        assert_eq!(eff("pikachu pokemon"), Effect::Net(Query::Pokemon("pikachu".into())));
        assert_eq!(eff("I'm bored"), Effect::Net(Query::Bored));
        assert_eq!(eff("show me a random xkcd"), Effect::Net(Query::Xkcd(true)));
        assert_eq!(eff("tell me a cat fact"), Effect::Net(Query::CatFact));
        assert!(text("is it a full moon tonight?").contains("moon") || text("is it a full moon tonight?").contains("MOON"));
        assert!(matches!(eff("konami code"), Effect::Costume(h, k) if h == "crown" && k == "rainbow"));
        assert!(text("sudo make me a sandwich").starts_with("Okay."));
        assert!(text("are you clippy?").contains("Never heard of him"));
        assert!(matches!(eff("rickroll"), Effect::Open(u) if u.contains("dQw4w9WgXcQ")));
        // shell commands stay literal
        assert_eq!(eff("run echo hi"), Effect::Shell("echo hi".into()));
    }
}
