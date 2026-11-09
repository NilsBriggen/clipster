//! Free-text understanding without AI: tidy the sentence, spot what it's asking for, map it onto a command.
//! "could you pls set an alarm for half an hour to check the oven?" -> "timer 1800s check the oven".

use crate::brain::{self, exact, url_encode, Act, Effect, Mood, Reply};
use crate::lines;

pub fn respond(input: &str) -> Reply {
    let raw = input.trim();
    // first of all, on the words as typed (tidying drops openers like "i want to", which matter here)
    if let Some(r) = crisis(&tidy_words(raw)) {
        return r;
    }
    let head = raw.split_whitespace().next().unwrap_or("").to_lowercase();
    // shell commands and button-only commands (whose text is data, like quiz answers) are taken literally
    // "run" only for a program (or shell syntax): "run a backup" and "run to the store" are sentences
    let second = raw.split_whitespace().nth(1).unwrap_or("");
    let shell_head = ["$", "sh", "cmd"].contains(&head.as_str()) || head == "run" && (in_path(second) || raw.contains(['|', '>', '<', ';', '&', '/', '~', '=']));
    if shell_head || ["reveal", "wyrpick", "window", "answer-to", "checkin", "install", "forget", "tidy", "pref", "clip", "forgetfact", "upkeep", "backup", "onlist", "readlater", "calendar", "person", "contacts", "workday"].contains(&head.as_str()) || head == "answer" && raw.contains('|') {
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
        // common words are never "fixed" (see REAL), and a guess that would act asks first
        if let Some(mut r) = intent(&fixed, &fixed).or_else(|| exact(&fixed)) {
            // a guess never locks, runs, opens or switches anything by itself: it asks first
            let harmless = matches!(r.effect, Effect::None | Effect::Net(_) | Effect::Timer(..) | Effect::Repeat(..) | Effect::RepeatAt(..) | Effect::ListTimers | Effect::Stopwatch(_) | Effect::OpenDesk(_) | Effect::Play(_) | Effect::Hat(_) | Effect::Skin(_) | Effect::TimeReport | Effect::Goals | Effect::AppTips | Effect::ClipHistory | Effect::SayName | Effect::ListAchievements);
            if !harmless {
                return Reply::say(format!("Did you mean \"{fixed}\"?")).mood(Mood::Think).run("Yes, that", &fixed).btn("No", Act::Hide);
            }
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
    ("hrs", "hours"), ("noon", "12pm"), ("midnight", "12am"), ("todo", "to do"), ("to-do", "to do"), ("e-mail", "email"),
];

const PHRASES: &[(&str, &str)] = &[
    ("an hour and a half", "90 minutes"), ("one and a half hours", "90 minutes"), ("half an hour", "30 minutes"),
    ("two and a half hours", "150 minutes"), ("2 and a half hours", "150 minutes"), ("three and a half hours", "210 minutes"), ("3 and a half hours", "210 minutes"),
    ("two and a half minutes", "150 seconds"), ("2 and a half minutes", "150 seconds"), ("a minute and a half", "90 seconds"),
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
pub fn keep(raw: &str, piece: &str) -> String {
    // lowercase char by char, remembering where each one came from ("İ" lowercases to two chars)
    let mut low = String::new();
    let mut from: Vec<(usize, usize)> = vec![]; // (byte in `low`, byte in `raw`) at each char start
    for (i, c) in raw.char_indices() {
        for l in c.to_lowercase() {
            from.push((low.len(), i));
            low.push(l);
        }
    }
    from.push((low.len(), raw.len()));
    let to_raw = |b: usize| from.iter().find(|(l, _)| *l == b).map(|(_, r)| *r);
    low.rfind(piece).and_then(|at| Some(&raw[to_raw(at)?..to_raw(at + piece.len())?])).unwrap_or(piece).to_string()
}

const PETS: &[&str] = &[
    "cat", "dog", "puppy", "kitten", "hamster", "rabbit", "bunny", "bird", "parrot", "budgie", "fish", "goldfish", "turtle", "tortoise",
    "snake", "guinea pig", "horse", "ferret", "lizard", "gecko", "rat", "mouse", "pet",
];
const PARTNERS: &[&str] = &["wife", "husband", "partner", "girlfriend", "boyfriend", "fiance", "fiancee"];
const FAVS: &[(&str, &str)] = &[
    ("food", "food"), ("meal", "food"), ("dish", "food"), ("drink", "drink"), ("color", "color"), ("colour", "color"), ("music", "music"),
    ("band", "music"), ("song", "music"), ("singer", "music"), ("movie", "movie"), ("film", "movie"), ("video game", "game"),
    ("game", "game"), ("animal", "animal"), ("book", "book"),
];

/// Facts people tell about themselves ("my cat is called Mo", "I live in Bern", "my favorite food is pizza"),
/// questions about them ("what's my cat called?") and forgetting them ("forget my job").
fn remember(raw: &str) -> Option<Reply> {
    let t = tidy_words(raw);
    let t = t.trim_end_matches(['.', ',', ';']).trim();
    // a short name or thing, with the person's own capitals
    let value = |x: &str, most: usize| -> Option<String> {
        let x = x.trim().trim_end_matches(['.', ',', ';']).trim();
        (!x.is_empty() && x.split_whitespace().count() <= most).then(|| keep(raw, x))
    };
    let told = |pool: &str, facts: &[(&str, &str)], fill: &[(&str, &str)]| -> Option<Reply> {
        let text = fill.iter().fold(lines::pick(pool), |t, (k, v)| t.replace(&format!("{{{k}}}"), v));
        let facts = facts.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        Some(Reply::say(text).mood(Mood::Love).effect(Effect::Remember(facts)))
    };
    let named = |who: &str| [format!("my {who} is called "), format!("my {who} is named "), format!("my {who}'s name is "), format!("my {who}s name is ")];
    if let Some(what) = t.strip_prefix("forget ") {
        return forget(what);
    }
    for pet in PETS {
        let more = [format!("i have a {pet} called "), format!("i have a {pet} named "), format!("we have a {pet} called "), format!("we have a {pet} named ")];
        if let Some(name) = named(pet).iter().chain(&more).find_map(|p| t.strip_prefix(p.as_str())).and_then(|x| value(x, 3)) {
            return told("remember_pet", &[("pet", &name), ("pet_kind", pet)], &[("x", &name), ("kind", pet)]);
        }
    }
    for who in PARTNERS {
        if let Some(name) = named(who).iter().find_map(|p| t.strip_prefix(p.as_str())).and_then(|x| value(x, 3)) {
            return told("remember_partner", &[("partner", &name), ("partner_word", who)], &[("x", &name), ("who", who)]);
        }
    }
    // "my job is boring" or "my job is killing me" is a feeling, not a job
    const FEELINGS: &[&str] = &["boring", "killing", "stressful", "hard", "fun", "great", "awful", "terrible", "exhausting", "tiring", "annoying", "ok", "okay", "fine", "amazing", "demanding", "crazy", "insane", "a nightmare", "too", "so", "really", "very", "pretty", "kind", "not", "driving", "easy", "good", "bad", "the worst", "the best", "hell", "fine", "over", "done", "gone", "toxic", "fun", "nice", "cool", "weird", "tough", "brutal", "meh", "great"];
    if let Some(job) = after(t, &["i work as ", "i am working as ", "my job is ", "my profession is "]).filter(|x| !FEELINGS.iter().any(|f| x == f || x.starts_with(&format!("{f} ")))).and_then(|x| value(x, 4)) {
        return told("remember_job", &[("job", &job)], &[("x", &job)]);
    }
    if let Some(city) = after(t, &["i live in ", "i am living in ", "i am from ", "i come from ", "i moved to "])
        .filter(|x| !["a ", "an ", "my ", "your ", "the ", "fear", "hope", "denial", "peace", "constant", "terror", "dread", "sin", "luxury", "poverty", "future", "past", "space", "here", "there", "nowhere", "everywhere", "another", "this", "that"].iter().any(|n| x.starts_with(n)))
        .and_then(|x| value(x, 3))
    {
        return told("remember_city", &[("city", &city)], &[("x", &city)]);
    }
    for (word, key) in FAVS {
        if let Some(v) = after(t, &[&format!("my favorite {word} is "), &format!("my favourite {word} is ")]).and_then(|x| value(x, 5)) {
            return told("remember_fav", &[(&format!("fav_{key}"), &v)], &[("what", word), ("x", &v)]);
        }
    }
    if has(t, &["what do you know about me", "what do you remember about me", "what have you learned about me", "what you know about me"]) {
        return go("aboutme".into());
    }
    recall(t)
}

/// "why?", "really?", "tell me more": about whatever he just said.
fn followup(raw: &str) -> Option<Reply> {
    let t = tidy_words(raw);
    match t.trim_end_matches(['.', ',']).trim() {
        "why" | "why not" | "how come" | "but why" | "why though" | "why is that" => Some(Reply::say(lines::pick("followup_why")).mood(Mood::Sly)),
        "really" | "seriously" | "are you sure" | "for real" | "is that true" | "no way" => Some(Reply::say(lines::pick("followup_really")).mood(Mood::Wink)),
        "tell me more" | "go on" | "continue" | "and then" | "more about that" | "explain" | "elaborate" | "keep going" => {
            Some(lines::more().unwrap_or_else(|| Reply::say(lines::pick("followup_more")).mood(Mood::Think)))
        }
        _ => None,
    }
}

/// What you typed after he asked you something (`## ask <id>`): his reaction, and the fact if it was a fact question
/// (the ids "city", "job", "pet", "fav_food", "fav_music" are also fact keys).
pub fn answer_to(id: &str, text: &str) -> Reply {
    let t = tidy_words(text);
    let t = t.trim_end_matches(['.', ',', '!']).trim();
    let (kind, mut r) = lines::answer(id, t, &lines::reflect(t)).unwrap_or_else(|| (String::new(), lines::reply("confused")));
    let no = kind == "no" || ["no", "nope", "none", "nothing", "not really", "nah", "no idea", "i do not know", "dunno"].iter().any(|n| t == *n || t.starts_with(&format!("{n} ")));
    if let Some(Effect::Remember(f)) = remember(text).map(|r| r.effect) {
        r.effect = Effect::Remember(f); // "I have a cat called Mo", said as an answer
    } else if !no && t.split_whitespace().count() <= 4 {
        let strip = |pre: &[&str]| keep(text, pre.iter().find_map(|p| t.strip_prefix(p)).unwrap_or(t).trim());
        let fact: Vec<(&str, String)> = match id {
            "city" => vec![("city", strip(&["i am in ", "in ", "near ", "from "]))],
            "job" => vec![("job", strip(&["i am ", "i work as "]))],
            "fav_food" | "fav_music" => vec![(id, strip(&["i like ", "i love ", "probably ", "definitely "]))],
            "pet" if t.split_whitespace().count() <= 2 && !PETS.iter().any(|p| has(t, &[p])) => vec![("pet", keep(text, t)), ("pet_kind", "pet".into())],
            _ => vec![],
        };
        if fact.iter().all(|(_, v)| !v.is_empty()) && !fact.is_empty() {
            r.effect = Effect::Remember(fact.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
        }
    }
    r
}

/// "what's my cat called?", "where do I live?", "what is my favorite food?": answered from the facts.
fn recall(t: &str) -> Option<Reply> {
    if !(t.starts_with("what") || t.starts_with("where") || t.starts_with("do you remember")) {
        return None;
    }
    let fav = FAVS.iter().find(|(w, _)| t.contains(&format!("my favorite {w}")) || t.contains(&format!("my favourite {w}")));
    let kind = |k: &str, d: &str| lines::fact(k).unwrap_or_else(|| d.to_string());
    let (key, sentence) = if let Some((w, k)) = fav {
        (format!("fav_{k}"), format!("Your favorite {w} is {{v}}."))
    } else if t.starts_with("what") && has(t, &["called", "named", "name", "s name"]) && PETS.iter().any(|p| has(t, &[&format!("my {p}"), &format!("my {p}s")])) {
        ("pet".into(), format!("Your {} is called {{v}}.", kind("pet_kind", "pet")))
    } else if t.starts_with("what") && has(t, &["called", "named", "name", "s name"]) && PARTNERS.iter().any(|p| has(t, &[&format!("my {p}"), &format!("my {p}s")])) {
        ("partner".into(), format!("Your {} is called {{v}}.", kind("partner_word", "partner")))
    } else if has(t, &["where do i live", "where am i from", "my city", "my hometown"]) {
        ("city".into(), "You live in {v}. I check the weather there for you.".into())
    } else if has(t, &["what is my job", "what do i do for work", "what do i do for a living", "what do i work as", "what is my profession", "where do i work"]) {
        ("job".into(), "You work as {v}.".into())
    } else {
        return None;
    };
    Some(match lines::fact(&key) {
        Some(v) => Reply::say(sentence.replace("{v}", &v)).mood(Mood::Sly),
        None => Reply::say("You haven't told me that yet. Tell me, and I'll remember it.").mood(Mood::Think),
    })
}

/// "forget my job", "forget where I live", "forget everything about me".
fn forget(what: &str) -> Option<Reply> {
    let all = ["everything about me", "all about me", "what you know about me", "everything you know about me"];
    let keys: Vec<String> = if all.iter().any(|a| what.starts_with(a)) {
        vec![]
    } else if let Some(rest) = what.strip_prefix("my ") {
        if let Some((_, k)) = FAVS.iter().find(|(w, _)| rest == format!("favorite {w}") || rest == format!("favourite {w}")) {
            vec![format!("fav_{k}")]
        } else if PETS.contains(&rest) {
            vec!["pet".into(), "pet_kind".into()]
        } else if PARTNERS.contains(&rest) {
            vec!["partner".into(), "partner_word".into()]
        } else if ["job", "work"].contains(&rest) {
            vec!["job".into()]
        } else if ["city", "home", "hometown"].contains(&rest) {
            vec!["city".into()]
        } else {
            return None; // "forget my notes" and friends belong to someone else
        }
    } else if what == "where i live" {
        vec!["city".into()]
    } else {
        return None;
    };
    if keys.is_empty() {
        // everything at once only after a yes, like notes and to-dos
        return Some(Reply::say("Forget everything I know about you? Your facts and your birthday go. Your name and your phrases stay.").mood(Mood::Think).run("Yes, forget it all", "forgetfact all").btn("No, keep it", Act::Hide));
    }
    let text = lines::pick("forget_one").replace("{x}", &lines::reflect(what));
    Some(Reply::say(text).mood(Mood::Think).effect(Effect::Forget(keys)))
}

fn go(cmd: String) -> Option<Reply> {
    exact(&cmd)
}

fn in_path(p: &str) -> bool {
    crate::sys::in_path(p)
}

fn first_in_path(list: &[&str]) -> Option<String> {
    list.iter().find(|p| in_path(p)).map(|p| p.to_string())
}

// ---------- the intents, most specific first ----------

fn intent(s: &str, raw: &str) -> Option<Reply> {
    if let Some(r) = crisis(s).or_else(|| crisis(&tidy_words(raw))) {
        return Some(r);
    }
    if let Some(r) = grief(&tidy_words(raw)) {
        return Some(r); // before the topics, so "my dog died" never gets a dog joke or a dog photo
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
    if let Some(r) = followup(raw) {
        return Some(r); // "why?", "really?", "tell me more" are about what he just said
    }
    if let Some(r) = brain::egg(s) {
        return Some(r); // 🥚
    }
    if let Some(day) = after(s, &["my birthday is ", "my birthday is on ", "my bday is ", "i was born on ", "remember my birthday "]) {
        return go(format!("birthday {day}"));
    }
    if let Some(r) = remember(raw) {
        return Some(r); // before the commands, so "my favorite color is green" is a fact, not a new skin
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
    let (start, iffy) = ["when i say ", "whenever i say ", "if i say "].iter().find_map(|p| low.find(p).filter(|i| *i <= 12).map(|i| (i + p.len(), *p == "if i say ")))?;
    // "if i say yes, will you tell me a joke" is a question, and "yes" or "please" mustn't get taken over
    let common = |p: &str| ["yes", "no", "please", "ok", "okay", "thanks", "thank you", "sure", "yep", "nope", "hi", "hello", "y", "n"].contains(&p.to_lowercase().as_str());
    let (rest, lrest) = (&raw[start..], &low[start..]);
    let first = |keys: &[&str]| keys.iter().filter_map(|k| lrest.find(k).map(|i| (i, k.len()))).min();
    let unquote = |t: &str| t.trim().trim_matches(['"', '\'', '“', '”']).trim().to_string();
    if let Some((i, len)) = first(&[", say ", " say ", ", reply ", " reply ", ", answer ", " answer ", ", you say ", " you say "]) {
        let (phrase, reply) = (unquote(&rest[..i]), unquote(&rest[i + len..]));
        if phrase.is_empty() || reply.is_empty() || common(&phrase) {
            return None;
        }
        return Some(Reply::say(format!("Got it! 📝 When you say \"{phrase}\", I'll say:\n\"{reply}\"\n\nTry it! (Say \"what have you learned\" to see everything.)")).mood(Mood::Happy).effect(Effect::Teach(phrase, reply, vec![])));
    }
    let (i, len) = first(&[", then ", " then ", ", "])?;
    let asks = raw.trim_end().ends_with('?') || ["will you", "would you", "can you", "could you", "do you", "are you", "is it", "is that", "what", "how", "why"].iter().any(|q| lrest[i + len..].trim_start().starts_with(q));
    if iffy || asks {
        return None;
    }
    let phrase = unquote(&rest[..i]);
    if common(&phrase) {
        return None;
    }
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
        "do not want to be alive", "not want to be here anymore", "like to die", "need to die", "going to die", "want to be dead", "wish i was dead",
        "wish i were dead", "wish i was not alive", "wish i were not alive", "end it all", "want it all to end", "not worth living", "let me die",
        "do not want to wake up", "want to disappear forever", "kill me", "no point in living", "rather be dead", "want to stop living",
    ];
    let t = s.trim().trim_end_matches(['.', '!']).trim();
    (has(s, &words) || ["die", "i die", "please die", "die please", "dying", "i am dying inside"].contains(&t)).then(|| {
        Reply::say(
            "I'm really sorry you're feeling this way. I'm only a paperclip, but you deserve to talk to a real person about this, right now.\n\n\
             If you might act on these thoughts, please call your local emergency number. You can also reach a free, confidential helpline in your country, any time:",
        )
        .btn("Find a helpline near me", Act::Open("https://findahelpline.com".into()))
        .btn("Close", Act::Hide)
    })
}

/// Loss and serious illness: "my dog died", "my grandma passed away", "i have cancer". A kind word in both voices, no jokes
/// (the hard limits). Phones, batteries and other things that "die" are left alone.
fn grief(s: &str) -> Option<Reply> {
    // who can be lost: family, friends, pets, and the people he knows by name ("my kettle died" is a kettle)
    const LIVING: &[&str] = &[
        "mum", "mom", "mother", "dad", "father", "grandma", "grandpa", "granny", "grandmother", "grandfather", "nan", "nana", "gran", "grandad", "granddad",
        "papa", "mama", "friend", "dog", "cat", "pet", "fish", "goldfish", "hamster", "rabbit", "bunny", "bird", "parrot", "budgie", "horse", "pig", "tortoise",
        "turtle", "snake", "lizard", "ferret", "puppy", "kitten", "uncle", "aunt", "auntie", "brother", "sister", "son", "daughter", "husband", "wife",
        "partner", "boyfriend", "girlfriend", "cousin", "nephew", "niece", "baby", "colleague", "coworker", "boss", "neighbour", "neighbor", "teacher", "grandparent",
    ];
    let w: Vec<&str> = s.split_whitespace().collect();
    let loss = w.iter().position(|x| ["died", "passed", "dead"].contains(x)).filter(|&i| {
        let near: Vec<&str> = w[i.saturating_sub(3)..i].to_vec();
        let who = near.iter().any(|x| LIVING.contains(x) || crate::people::known(x));
        let form = match w[i] {
            "passed" => w.get(i + 1) == Some(&"away"),
            "dead" => near.contains(&"is"),
            _ => true,
        };
        i > 0 && who && form
    });
    let ill = has(s, &["i have cancer", "has cancer", "got cancer", "diagnosed with", "terminally ill", "in the hospital", "in hospital", "had a miscarriage", "funeral", "lost my mum", "lost my mom", "lost my dad", "lost my father", "lost my mother", "lost my husband", "lost my wife", "lost my baby", "lost my brother", "lost my sister", "lost my friend", "lost my dog", "lost my cat", "lost my grandma", "lost my grandpa", "had to put my", "put down today", "was put down"]);
    (loss.is_some() || ill).then(|| Reply::say(lines::pick("grief")).mood(Mood::Sad))
}

/// Things that are files ("where is my CV" is a file search, "where is my phone" isn't).
const FILE_WORDS: &[&str] = &["file", "document", "pdf", "photo", "picture", "spreadsheet", "presentation", "slides", "doc", "docx", "cv", "resume", "invoice", "report", "contract", "screenshot", "video", "song"];

const TIMER_WORDS: &[&str] = &[
    "timer", "timers", "alarm", "alarms", "countdown", "count down", "remind", "reminder", "reminders", "wake me",
    "ping me", "alert me", "tell me in", "notify me", "let me know in", "beep", "remember to", "do not let me forget",
];

fn timers(s: &str, raw: &str) -> Option<Reply> {
    // "todo remind Anna on friday", "todo wrap up the report": a to-do, whatever words it holds
    if adding_todo(s) {
        return None;
    }
    // commands only at the start of the sentence, so "i hate system updates" stays a conversation
    let starts = |ps: &[&str]| ps.iter().any(|p| s == *p || s.starts_with(&format!("{p} ")));
    // goals and daily limits (before focus sessions, so "focus goal 2 hours" isn't a session)
    if has(s, &["no focus goal", "remove my focus goal", "turn off my focus goal", "delete my focus goal", "remove the focus goal"]) {
        return go("goal off".into());
    }
    if let Some(rest) = after(s, &["set my focus goal to ", "set my daily focus goal to ", "set a focus goal of ", "set the focus goal to ", "my focus goal is ", "focus goal of ", "focus goal to ", "focus goal "]) {
        return go(format!("goal {rest}"));
    }
    if starts(&["how am i doing on my goals", "how are my goals", "goal progress", "my goals", "my daily limits", "my limits", "my focus goal", "show my goals"]) {
        return go("goals".into());
    }
    for (prefix, off) in [("limit ", false), ("set a limit for ", false), ("set a daily limit for ", false), ("no limit for ", true), ("remove the limit for ", true), ("remove my limit for ", true), ("stop limiting ", true)] {
        if let Some(rest) = s.strip_prefix(prefix) {
            let (what, amount) = if off { (rest, "off") } else { rest.split_once(" to ").or_else(|| rest.split_once(" at ")).unwrap_or((rest, "")) };
            if let Some(kind) = crate::brain::budget_kind(what.trim_end_matches(" time").trim_end_matches(" limit")) {
                return go(format!("budget {kind} {}", if amount.is_empty() { "off" } else { amount }));
            }
        }
    }
    // looking after the computer ("check for updates" is Clipster's own; these are the system's)
    if starts(&["computer checkup", "check up on my computer", "check my computer", "computer check up", "upkeep", "maintenance check", "look after my computer"]) {
        return go("upkeep".into());
    }
    if starts(&["system updates", "update my system", "update the system", "install system updates", "open the updater", "check for system updates", "show system updates", "any system updates"]) {
        return go("sysupdates".into());
    }
    if s == "back up" || s == "backup" || starts(&["back up now", "backup now", "back up my files now", "backup my files now", "run a backup", "run my backup", "run the backup", "start a backup", "start the backup", "make a backup"]) {
        return go("backup now".into());
    }
    if starts(&["startup apps", "start up apps", "show my startup apps", "apps that start with my computer", "programs that start with my computer", "autostart apps"]) {
        return go("desk upkeep".into());
    }
    // the start and end of the day, and the week
    if starts(&["plan my day", "plan the day", "plan today", "plan my morning", "morning plan", "help me plan my day"]) {
        return go("planday".into());
    }
    if starts(&["wrap up my day", "wrap up the day", "wrap up today", "end my day", "close out my day", "wrap up"]) || s == "end of the day" || s == "end of day" {
        return go("wrapup".into());
    }
    if starts(&["week in review", "weekly review", "review my week", "how was my week", "my week so far", "sum up my week", "summary of my week", "my week in review", "show my week in review"]) {
        return go("review".into());
    }
    // focus sessions: "focus for 45 minutes on the report", "help me focus", "stop focusing"
    if has(s, &["stop focusing", "end focus", "end the focus", "stop the focus", "cancel focus", "cancel the focus", "focus off", "end my focus session"]) {
        return go("focus stop".into());
    }
    // pause, go on, a few more minutes (before "focus …" starts a new session)
    if has(s, &["focus", "focus session", "session"]) && has(s, &["pause", "hold"]) {
        return go("focus pause".into());
    }
    if has(s, &["focus", "focus session"]) && has(s, &["resume", "continue", "unpause", "go on with"]) {
        return go("focus resume".into());
    }
    if let Some(r) = after(s, &["extend my focus by ", "extend the focus by ", "extend focus by ", "add to my focus ", "more focus "]).or_else(|| s.strip_suffix(" more of focus").or_else(|| s.strip_suffix(" more minutes of focus"))) {
        return go(format!("focus extend {r}"));
    }
    // "focus 25 minutes", "focus on the report", "help me focus"; not "focus groups are useful"
    let focus_cmd = s == "focus" || s.strip_prefix("focus ").is_some_and(|r| r.starts_with("on ") || r.starts_with("for ") || r.starts_with(|c: char| c.is_ascii_digit()) || r.split_whitespace().count() <= 3 && !has(r, &["is", "are", "was", "were", "group", "groups"]));
    if focus_cmd || has(s, &["focus session", "focus mode", "help me focus", "i need to focus", "start focusing", "time to focus"]) {
        let rest = after(s, &["start a focus session for ", "start a focus session ", "focus session for ", "focus mode for ", "focus for ", "focus ", "help me focus for ", "help me focus on ", "help me focus "]).unwrap_or("");
        return go(format!("focus {}", keep(raw, rest)));
    }
    if has(s, &["where did my time go", "time report", "how did i spend my time", "what did i do today", "my screen time", "screen time today", "how long was i on", "what have i been doing"]) {
        return go("timereport".into());
    }
    if !has(s, TIMER_WORDS) && !s.starts_with("in ") && !s.starts_with("every ") || has(s, &["until", "till"]) || s.contains("countdown to") || s.contains("count down to") {
        return None; // "countdown to christmas" is a date question
    }
    // by the calendar: "remind me every weekday at 8 to take my pills", "on the 1st of every month pay rent"
    if let Some(i) = s.find("every ").filter(|_| !s.contains("every other")) {
        if let Some((rule, rest)) = crate::timers::parse_rule(&s[i + 6..]) {
            return go(format!("everyat {rule}|{}", keep(raw, &label(&s[..i], &rest))));
        }
    }
    if let Some(i) = ["on the ", "every month on ", "each month on "].iter().filter_map(|p| s.find(p)).min() {
        if let Some((rule, rest)) = crate::timers::parse_monthly(&s[i..]) {
            return go(format!("everyat {rule}|{}", keep(raw, &label(&s[..i], &rest))));
        }
    }
    // repeating: "remind me every hour to drink water"
    let ev = s.replace("every hour", "every 1 hour").replace("every minute", "every 1 minute").replace("hourly", "every 1 hour").replace("every half hour", "every 30 minutes");
    if let Some(i) = ev.find("every ") {
        if let Some((secs, rest)) = brain::parse_dur(&ev[i + 6..]) {
            return go(format!("every {secs}s {}", keep(raw, &label(&ev[..i], rest))));
        }
    }
    const STOP: &[&str] = &["cancel", "stop", "delete", "clear", "remove", "end", "kill", "turn off"];
    const KINDS: &[&str] = &["timer", "timers", "alarm", "alarms", "reminder", "reminders", "countdown", "all"];
    if has(s, STOP) && has(s, KINDS) {
        // "cancel the tea timer" cancels that one; "cancel all timers" all of them
        let name: Vec<&str> = s.split_whitespace().filter(|w| !STOP.contains(w) && !KINDS.contains(w) && !["the", "my", "a", "off", "please"].contains(w)).collect();
        return go(if name.is_empty() { "cancel".into() } else { format!("cancel {}", keep(raw, &name.join(" "))) });
    }
    if let Some((secs, label, when)) = dated(s) {
        let label = keep(raw, &label);
        let text = lines::pick("say_remind_dated").replace("{label}", if label.is_empty() { "your reminder" } else { &label }).replace("{when}", &when);
        return Some(Reply::say(text).mood(Mood::Happy).effect(Effect::Timer(secs, label)));
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
            let t = &s[i..];
            let digits = t.find(|c: char| !c.is_ascii_digit()).unwrap_or(t.len());
            let next = t[digits..].trim_start();
            let next_word = next.split_whitespace().next().unwrap_or("");
            // "for 6am", "for 7:30", "for 9 o'clock": a time of day, not six minutes
            if t[digits..].starts_with(':') || ["am", "pm", "a.m.", "p.m.", "o'clock", "oclock"].contains(&next_word) {
                if let Some((secs, used)) = clock(t) {
                    return Some((secs, label(before.trim_end_matches("for ").trim_end_matches("at "), &t[used..])));
                }
            }
            // "in 2024 i went to paris", "in 5 years": not a timer
            if ["year", "years", "month", "months", "decade", "decades", "century"].contains(&next_word) || next_word.is_empty() && digits >= 4 && !before.ends_with("for ") {
                continue;
            }
            if let Some((secs, rest)) = brain::parse_dur(t) {
                let used = &s[i..s.len() - rest.len()];
                // a bare number is minutes only when it's plausibly a timer: not a year, and with a timer word unless short
                if !used.chars().any(|c| c.is_ascii_alphabetic()) && (digits >= 4 || !has(s, TIMER_WORDS) && secs > 180 * 60) {
                    continue;
                }
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

/// A reminder on another day: "tomorrow at 3", "on friday", "next monday at 9:30", "on june 3", "in 2 days".
/// Returns seconds from now, the label and a readable "when". Without a time it's 9:00.
pub fn dated(s: &str) -> Option<(u64, String, String)> {
    use chrono::{Duration, Local};
    let now = Local::now().naive_local();
    let today = now.date();
    let words: Vec<&str> = s.split_whitespace().collect();
    // the day phrase: (first word, one past its last word, the date)
    let day = (0..words.len()).find_map(|i| match words[i] {
        "tomorrow" => Some((i, i + 1, today + Duration::days(1))),
        "tonight" => Some((i, i + 1, today)),
        "this" if matches!(words.get(i + 1), Some(&"evening") | Some(&"afternoon")) => Some((i, i + 2, today)),
        "in" => {
            let n: u64 = words.get(i + 1)?.parse().ok()?;
            let unit = words.get(i + 2)?;
            let days = if unit.starts_with("week") { n.checked_mul(7)? } else if unit.starts_with("day") { n } else { return None };
            Some((i, i + 3, today.checked_add_days(chrono::Days::new(days.min(3650)))?))
        }
        // "on friday", "next monday", "on june 3": the longest run of up to 3 words that is a day
        // the shortest run that is a day, so "on friday to water the plants" keeps its words
        w @ ("on" | "next") => (1..=3).filter(|n| i + 1 + n <= words.len()).find_map(|n| {
            let (d, _) = crate::powers::parse_day(&words[i + 1..i + 1 + n].join(" "), today)?;
            (d > today || w == "next").then_some((i, i + 1 + n, d))
        }),
        _ => None,
    });
    let (start, mut end, date) = day?;
    // the time: "at 3", "at 9:30am"; without am/pm, 1-7 means the afternoon (nobody books 3 a.m.)
    let mut time = match words[start] {
        "tonight" => (20, 0),
        "this" if words.get(start + 1) == Some(&"evening") => (19, 0),
        "this" => (15, 0),
        _ => (9, 0),
    };
    if let Some(a) = words.iter().position(|w| *w == "at") {
        let rest = words[a + 1..].join(" ");
        if let Some((hours, m, used)) = hm(&rest) {
            time = (if hours.len() == 2 && hours[0] < 8 { hours[1] } else { hours[0] }, m);
            if a == end {
                end = a + 1 + rest[..used].split_whitespace().count(); // "tomorrow at 3 to call anna": the label starts after the time
            }
        }
    }
    let at = date.and_hms_opt(time.0, time.1, 0)?;
    if at <= now {
        return None;
    }
    let label = label(&words[..start].join(" "), &words[end.min(words.len())..].join(" "));
    let label = label.trim_end_matches(" at").to_string();
    let clock = at.format("%H:%M");
    // always the day's name: the lines say "on {when}" ("on tomorrow at 9:00" doesn't read)
    let when = format!("{} at {clock}", at.format("%A, %B %-d"));
    // through real (zoned) times: across a daylight-saving change a local day isn't 24 hours
    Some(((crate::timers::unix_of(at) - crate::timers::unix_now()).max(60) as u64, label, when))
}

/// When something should happen, for snoozing and moving reminders: "in 20 min", "in 2 hours", "at 3", "15:30",
/// "this evening" (18:00), "tonight" (20:00), "tomorrow", "friday at 9", "in 2 days". Seconds from now and a readable "when".
pub fn when_secs(s: &str) -> Option<(u64, String)> {
    use chrono::Local;
    let s = s.trim().to_lowercase();
    let s = s.trim_start_matches("for ").trim();
    if let Some(r) = s.strip_prefix("in ") {
        if let Some((secs, _)) = brain::parse_dur(r).filter(|(_, rest)| rest.trim().is_empty()) {
            return Some((secs, format!("in {}", brain::fmt_dur(secs))));
        }
    }
    if let Some((secs, _, when)) = dated(s) {
        return Some((secs, when));
    }
    let evening = |h: u32| {
        let now = Local::now().naive_local();
        let at = now.date().and_hms_opt(h, 0, 0)?;
        (at > now).then(|| ((crate::timers::unix_of(at) - crate::timers::unix_now()).max(1) as u64, format!("at {h}:00")))
    };
    match s {
        "this evening" | "evening" | "after work" => return evening(18),
        "tonight" => return evening(20),
        _ => {}
    }
    let t = s.trim_start_matches("at ");
    let (secs, used) = clock(t)?;
    (used == t.len()).then(|| (secs, format!("at {}", (Local::now() + chrono::Duration::seconds(secs as i64)).format("%H:%M"))))
}

/// "5", "5pm", "17:30", "7:15 am", "9 o'clock" -> the possible hours (both for "at 5"), the minutes and how much text was used.
pub fn hm(t: &str) -> Option<(Vec<u32>, u32, usize)> {
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
    let hours = match pm {
        Some(p) if h <= 12 => vec![h % 12 + if p { 12 } else { 0 }],
        Some(_) => return None,
        None if (1..12).contains(&h) => vec![h, h + 12],
        None => vec![h],
    };
    Some((hours, m, i))
}

/// "5pm", "17:30", "7:15 am", "9 o'clock" -> seconds until then, and how much text was used.
fn clock(t: &str) -> Option<(u64, usize)> {
    use chrono::{Duration, Local};
    let (hours, m, i) = hm(t)?;
    let now = Local::now().naive_local();
    hours
        .iter()
        .filter_map(|&h| {
            let target = now.date().and_hms_opt(h, m, 0)?;
            let target = if target <= now { target + Duration::days(1) } else { target };
            Some((crate::timers::unix_of(target) - crate::timers::unix_now()).max(1) as u64)
        })
        .min()
        .map(|secs| (secs, i))
}

/// Controlling the computer and handy tools: files, sound, music, screen, power, dates...
fn powers(s: &str, raw: &str) -> Option<Reply> {
    if has(s, &["open my desk", "show my desk", "open the desk", "open desk", "clipster s desk", "clipsters desk", "my desk window", "focus stats", "my week stats", "show my week", "productivity stats"]) || s == "my desk" {
        return go("desk".into());
    }
    if has(s, &["tips for this app", "tips for this program", "any tips for this", "app tips", "keyboard shortcuts", "shortcuts for this", "useful shortcuts", "a shortcut", "shortcut tips", "tips for this window"]) {
        return go("apptips".into());
    }
    // tidying files: "tidy my downloads", "clean up my desktop", "undo the tidy", "find duplicates in downloads"
    let place = if has(s, &["desktop"]) { "desktop" } else { "downloads" };
    if has(s, &["undo the tidy", "undo tidying", "undo the cleanup", "undo the tidying", "put my files back", "undo tidy"]) {
        return go("tidy undo".into());
    }
    // "remove duplicates" (lines in a text) is a text tool; files need a place or the word files
    if has(s, &["duplicate files", "duplicates", "find duplicates", "same files twice", "doubled files"]) && has(s, &["downloads", "download", "desktop", "files"]) {
        return go(format!("dupes {place}"));
    }
    if has(s, &["old big files", "big old files", "large old files", "old large files", "old files", "files i never use"]) {
        return go(format!("oldfiles {place}"));
    }
    if has(s, &["tidy", "tidy up", "clean up", "organize", "organise", "sort", "sort out", "declutter"]) && has(s, &["downloads", "download folder", "desktop", "my files"]) && !has(s, &["trash"]) {
        return go(format!("tidy {place}"));
    }
    // system switches and readings
    // only a real request to switch ("turn the wifi off", "disable bluetooth", "wifi on"), never "is the internet off"
    // or "recipes on the internet"
    for (kws, what) in [(&["wifi", "wi fi", "wireless", "internet"][..], "wifi"), (&["bluetooth"][..], "bluetooth")] {
        if has(s, kws) && !has(s, &["name", "password", "which", "what", "is", "keeps", "on the internet", "my internet", "the internet is"]) {
            let ask = s.starts_with("turn ") || s.starts_with("switch ") || s.starts_with("please ") || kws.iter().any(|k| s.starts_with(k));
            if has(s, &["disable", "deactivate"]) || ask && s.ends_with(" off") || has(s, &["turn off", "switch off"]) {
                return go(format!("{what} off"));
            }
            if has(s, &["enable", "activate"]) || ask && s.ends_with(" on") || has(s, &["turn on", "switch on"]) {
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
    if has(s, &["start the tour", "take the tour", "give me a tour", "show me around", "start the setup", "setup again"]) || ["tutorial", "the tour", "tour", "onboarding", "show me the tour", "the tutorial"].contains(&s) {
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
        // "call me later", "can you call me in 5 minutes", "i go by bus" aren't names
        const NOT: &[&str] = &["later", "back", "tomorrow", "tonight", "soon", "in", "at", "when", "if", "maybe", "sometime", "by", "on", "bus", "train", "car", "bike", "foot", "taxi", "plane", "the", "a", "an", "now", "please", "asap", "again", "anytime", "whenever"];
        let first = n.split_whitespace().next().unwrap_or("");
        let named = s.starts_with("my name is ") || s.starts_with("i am called ");
        if named || (!NOT.contains(&first) && !n.chars().any(|c| c.is_ascii_digit()) && n.split_whitespace().count() <= 3) {
            return go(format!("setname {}", keep(raw, n)));
        }
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
        (&["clip chain", "clipchain", "play snake", "snake game", "game of snake", "worm game"], "clipchain"),
        (&["byte size", "bytesize", "play 2048", "2048 game", "sliding tiles", "tile game", "sliding game"], "bytesize"),
    ];
    // "i lost at chess" or "board games are fun" is small talk, not a request to play
    // also questions ("who invented chess", "how do you play chess") and memories ("remember when we played chess")
    let asking = knowledge(s) || ["how ", "who ", "why ", "what ", "when ", "remember ", "do you "].iter().any(|q| s.starts_with(q)) && !s.starts_with("how about ");
    let small_talk = (["i ", "we ", "my "].iter().any(|p| s.starts_with(p)) || has(s, &["is", "are", "was", "were", "played"]) || asking) && !has(s, &["play", "lets play", "let us play"]);
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
            // "42 in binary", "what is 255 in binary": a number base, not the text's bits
            if kind == "binary" && suffix != " from binary" {
                if let Some(r) = crate::powers::bases(s) {
                    return Some(r);
                }
            }
            let t = after(t, &["convert ", "translate ", "write ", "put ", "turn ", "say ", "make "]).unwrap_or(t);
            return go(format!("{kind} {}", keep(raw, t)));
        }
    }
    // dress-up: "wear a wizard hat", "take off your hat", "turn gold", "make yourself pink"
    let question = ["what ", "who ", "why ", "how ", "where "].iter().any(|q| s.starts_with(q));
    // a request to dress up ("wear a wizard hat", "take off your hat"), not "i'm playing halo" or "my cap is red"
    let dressing = has(s, &["wear", "put on", "take off", "remove", "try on", "your hat", "a hat", "new hat", "give yourself", "hat on", "hat off", "change your hat", "no hat"]) || s.split_whitespace().count() <= 2;
    if !question && dressing && has(s, &["hat", "hats", "cap", "crown", "beanie", "sombrero", "tophat", "halo"]) {
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
        "where did i save the ", "where did i save my ", "where did i save ", "where did i put the file ", "where did i put my file ",
    ];
    if let Some(n) = after(s, FIND).filter(|n| !n.is_empty()) {
        return go(format!("findfile {}", keep(raw, n.trim_start_matches("called ").trim_start_matches("named "))));
    }
    // "find my cv", "where is my tax pdf": a file, unless it's a thing you lose in real life
    if let Some(n) = after(s, &["find my ", "where is my ", "where are my "]).filter(|n| !n.is_empty()) {
        const THINGS: &[&str] = &["phone", "keys", "key", "glasses", "wallet", "car", "way", "ip", "ip address", "iphone", "location", "mouse", "cursor", "motivation", "mind", "passion", "purpose"];
        let filey = n.split_whitespace().any(|w| FILE_WORDS.contains(&w));
        let thing = THINGS.contains(&n) || n.split_whitespace().any(|w| ["keys", "phone", "wallet", "glasses", "car", "cat", "dog", "bike", "umbrella", "headphones", "charger", "remote"].contains(&w));
        if !thing && (filey || s.starts_with("find my ")) || filey && !THINGS.contains(&n) && s.starts_with("find my ") {
            return go(format!("findfile {}", keep(raw, n)));
        }
    }
    if ["recent files", "recently opened files", "my recent files", "my recent documents", "recent documents", "what did i open recently", "what files did i open", "files i opened recently", "last opened files"].contains(&s) {
        return go("recent".into());
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
    let how = ["how do i ", "how to ", "how can i ", "how does "].iter().any(|q| s.starts_with(q));
    if !knowledge(s) && !how && has(s, &["screenshot", "screen shot", "screen capture", "capture the screen", "capture my screen", "print screen", "snapshot", "screengrab", "screen grab"]) {
        return go(format!("screenshot {s}"));
    }
    let whole_computer = has(s, &["computer", "pc", "laptop", "system", "machine", "everything"]) || ["shutdown", "shut down", "restart", "reboot", "power off", "turn off"].contains(&s);
    if whole_computer && has(s, &["to sleep", "go to sleep"]) {
        return go("suspend".into()); // "put my computer to sleep"
    }
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

/// "todo clear the desk", "to do: remind Anna": adding an item (not "todo list" or "todo clear").
fn adding_todo(s: &str) -> bool {
    ["todo ", "to do ", "to-do "].iter().find_map(|p| s.strip_prefix(p)).is_some_and(|rest| !matches!(rest.trim(), "list" | "clear" | "clear list" | "delete" | "reset" | "empty" | "wipe" | "clear finished"))
}

/// "my shopping list", "the packing list": (the list's name, where "my"/"the" is). Never the to-do or reading list.
fn named_list(s: &str) -> Option<(String, usize)> {
    let w: Vec<&str> = s.split_whitespace().collect();
    (0..w.len().saturating_sub(2))
        .find(|&i| {
            const NOT_NAMES: &[&str] = &["todo", "to-do", "reading", "task", "tasks", "do", "whole", "entire", "main", "full", "old", "current", "big", "long", "short", "new", "same", "other", "next", "last", "first", "complete", "master", "own", "usual", "normal", "regular", "this", "that", "my"];
            ["my", "the", "our"].contains(&w[i]) && w[i + 2] == "list" && !NOT_NAMES.contains(&w[i + 1]) && w[i + 1].chars().all(|c| c.is_alphabetic() || c == '-')
        })
        .map(|i| (w[i + 1].to_string(), i))
}

/// Named lists: "add eggs, milk and bread to my shopping list", "what's on the packing list", "clear my shopping list".
fn named_lists(s: &str, raw: &str) -> Option<Reply> {
    let (name, at) = named_list(s)?;
    let w: Vec<&str> = s.split_whitespace().collect();
    let first = w[0];
    if ["add", "put"].contains(&first) && at + 3 == w.len() && at >= 2 && ["to", "on", "onto"].contains(&w[at - 1]) {
        let items = w[1..at - 1].join(" ");
        return (!items.is_empty()).then(|| go(format!("onlist {name} add {}", keep(raw, &items)))).flatten();
    }
    // "remove eggs from my shopping list", "tick off eggs on my shopping list"
    let verb = [("remove ", "remove"), ("delete ", "remove"), ("take ", "remove"), ("tick off ", "done"), ("cross off ", "done"), ("check off ", "done"), ("mark ", "done")].iter().find(|(v, _)| s.starts_with(v));
    if let Some((v, op)) = verb.filter(|_| at >= 2 && ["from", "off", "on", "in"].contains(&w[at - 1])) {
        let item = w[..at - 1].join(" ");
        let item = item.trim_start_matches(v.trim()).trim().trim_end_matches(" as done").trim();
        if !item.is_empty() {
            return go(format!("onlist {name} {op} {}", keep(raw, item)));
        }
    }
    if ["clear", "empty", "wipe", "reset", "clean"].contains(&first) {
        return go(format!("onlist {name} clear"));
    }
    let asks = ["what", "show", "read", "open", "check", "see", "view", "whats", "list"].contains(&first) || at == 0 && at + 3 == w.len();
    if asks {
        return go(format!("onlist {name}"));
    }
    None
}

/// The calendar: "what's on tomorrow", "my next meeting", "am I free on Friday at 3", "add my calendar webcal://…".
fn agenda(s: &str, raw: &str) -> Option<Reply> {
    // a calendar link: take it from what was typed, exactly (links are case-sensitive)
    let link = raw.split_whitespace().map(|w| w.trim_matches(['"', '\'', '<', '>'])).find(|w| w.starts_with("webcal") || w.starts_with("http") && (w.contains(".ics") || w.contains("/ical/") || w.contains("calendar")));
    if let Some(l) = link.filter(|l| l.starts_with("webcal") || has(s, &["calendar", "subscribe", "ical", "agenda"])) {
        return go(format!("calendar add {l}"));
    }
    const TODAY: &[&str] = &[
        "agenda", "my agenda", "what is on today", "what is on my calendar", "what is on my agenda", "my calendar", "show my calendar", "what do i have today",
        "what is my schedule", "my schedule", "what is on my schedule today", "my schedule today", "agenda today", "what is on the calendar today",
        "do i have meetings today", "any meetings today", "what meetings do i have today", "what is on my calendar today", "my agenda today",
    ];
    const TOMORROW: &[&str] = &[
        "what is on tomorrow", "what do i have tomorrow", "my agenda for tomorrow", "agenda tomorrow", "tomorrow agenda", "my schedule tomorrow",
        "what is on my calendar tomorrow", "any meetings tomorrow", "my calendar tomorrow", "what is my schedule tomorrow",
    ];
    const NEXT: &[&str] = &[
        "next meeting", "my next meeting", "when is my next meeting", "what is my next meeting", "next appointment", "my next appointment",
        "what is my next appointment", "when is my next appointment", "what is my next event", "next event",
    ];
    let t = s.trim_end_matches('?').trim();
    if TODAY.contains(&t) {
        return go("calendar today".into());
    }
    if TOMORROW.contains(&t) {
        return go("calendar tomorrow".into());
    }
    if NEXT.contains(&t) {
        return go("calendar next".into());
    }
    let today = chrono::Local::now().date_naive();
    if let Some(day) = after(t, &["what is on ", "what do i have on ", "my agenda for ", "my schedule for ", "what is on my calendar on ", "agenda for "]) {
        if let Some((d, _)) = crate::powers::parse_day(day, today).filter(|_| day.split_whitespace().count() <= 3) {
            // "what's on friday" on a Friday means today, not next week
            let d = if (d - today).num_days() == 7 && day.starts_with(&today.format("%A").to_string().to_lowercase()) { today } else { d };
            return go(format!("calendar day {d}"));
        }
    }
    // "am i free friday at 3", "am i busy tomorrow at 10", "do i have anything at 2"
    let rest = after(t, &["am i free ", "am i busy ", "do i have anything ", "do i have something ", "is there anything "])?;
    let day_words = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
    let rest = if day_words.iter().any(|d| rest.starts_with(d)) { format!("on {rest}") } else { rest.to_string() };
    let now = chrono::Local::now();
    let at = if let Some((secs, _, _)) = dated(&format!("then {rest}")) {
        Some((now.timestamp() + secs as i64 + 30) / 60 * 60) // to the minute: "now" had a fraction of a second
    } else {
        // today: "at 3" is the next 3 o'clock still to come
        let evening = rest.starts_with("tonight");
        let clock = rest.trim_start_matches("today ").trim_start_matches("tonight ").strip_prefix("at ")?;
        let (hours, m, _) = hm(clock)?;
        let hours: Vec<u32> = if evening { hours.into_iter().filter(|h| *h >= 12).collect() } else { hours };
        // the next one still to come: today, or else tomorrow ("am i free at 3" asked at 16:00)
        [today, today + chrono::Days::new(1)].iter().flat_map(|d| hours.iter().filter_map(move |h| d.and_hms_opt(*h, m, 0))).map(|t| crate::timers::unix_of(t)).find(|t| *t > now.timestamp())
    }?;
    go(format!("calendar free {at}"))
}

/// The people in your life: "Anna's birthday is on 3 March", "keep in touch with mum every 2 weeks", "I called mum".
fn people_talk(s: &str, raw: &str) -> Option<Reply> {
    let r = raw.trim().trim_end_matches(['.', '!', '?']).replace('’', "'");
    let lower = r.to_lowercase();
    // "<name>'s birthday is (on) <date>", "my mum's email is …" (but "my birthday is" stays your own)
    for (what, cmd) in [("'s birthday is ", "birthday"), ("'s birthday's ", "birthday"), ("'s email is ", "email"), ("'s email address is ", "email"), ("'s e-mail is ", "email")] {
        // the position in the lowercased text only holds in the typed one when lowercasing kept the length
        let found = if lower.len() == r.len() { lower.find(what) } else { r.find(what) };
        if let Some(i) = found {
            let name = r[..i].trim();
            let name = name.strip_prefix("my ").or_else(|| name.strip_prefix("My ")).unwrap_or(name).trim();
            let value = r[i + what.len()..].trim();
            if !name.is_empty() && name.split_whitespace().count() <= 3 && !value.is_empty() {
                return go(format!("person {cmd} {name}|{value}"));
            }
        }
    }
    // "keep in touch with mum every 2 weeks"
    if let Some(rest) = after(s, &["keep in touch with ", "stay in touch with "]) {
        let (who, when) = rest.split_once(" every ")?;
        let n: u32 = match when.split_whitespace().next().unwrap_or("") {
            "other" | "couple" | "2" => 2,
            "few" => 3,
            w => w.parse().unwrap_or(1),
        };
        let days = if when.contains("week") { n * 7 } else if when.contains("month") { n * 30 } else { n };
        return go(format!("person every {}|{days}", keep(raw, who)));
    }
    // "i called mum", "i talked to anna": only for someone he knows
    // "my mum" is "mum", and "today" or "yesterday" at the end isn't part of the name
    let who_of = |w: &str| -> String {
        let w = w.strip_prefix("my ").unwrap_or(w);
        let w = ["today", "yesterday", "this morning", "this afternoon", "this evening", "earlier", "just now", "last night", "again"].iter().find_map(|t| w.strip_suffix(t)).unwrap_or(w);
        w.trim().to_string()
    };
    if let Some(who) = after(s, &["i just called ", "i called ", "i talked to ", "i spoke to ", "i spoke with ", "i talked with ", "i met ", "i saw ", "i texted ", "i messaged ", "i emailed ", "i visited ", "i caught up with ", "just called ", "just talked to "]) {
        let who = who_of(who);
        let who = who.as_str();
        if crate::people::known(who) {
            return go(format!("person met {}", keep(raw, who)));
        }
    }
    if let Some(who) = after(s, &["when did i last talk to ", "when did i last speak to ", "when did i last call ", "when did i last see ", "when did i last text ", "when did i last meet ", "when did i last hear from "]) {
        let who = who_of(who);
        return go(format!("person last {}", keep(raw, &who)));
    }
    if let Some(who) = after(s, &["email ", "write to ", "send an email to ", "write an email to ", "mail "]) {
        let who = who_of(who);
        let who = who.as_str();
        if crate::people::known(who) {
            return go(format!("person mail {}", keep(raw, who)));
        }
        // a name he doesn't know (one or two words, not "email my boss about…"): say so instead of searching the web
        let name = keep(raw, who);
        if who.split_whitespace().count() <= 2 && name.starts_with(char::is_uppercase) && !has(who, &["about", "the", "a", "an", "to", "my"]) {
            return Some(Reply::say(format!("I don't have an email address for {name}. Tell me: {name}'s email is …")).mood(Mood::Think).btn("Open the mail app", Act::Open("mailto:".into())));
        }
    }
    let t = s.trim_end_matches('?');
    if ["upcoming birthdays", "whose birthday is next", "any birthdays coming up", "birthdays coming up", "next birthday", "whose birthday is coming up", "who has a birthday soon"].contains(&t) {
        return go("people birthdays".into());
    }
    if ["import contacts", "import my contacts", "import a contacts file", "import my address book"].contains(&t) {
        return go("contacts import".into());
    }
    if ["my people", "people i know", "who do you know", "show my people", "my contacts"].contains(&t) {
        return go("people".into());
    }
    None
}

fn notes(s: &str, raw: &str) -> Option<Reply> {
    if let Some(r) = named_lists(s, raw) {
        return Some(r);
    }
    // start my workday: opens your work things
    let t = s.trim_end_matches(['!', '.']);
    if ["start my workday", "start my work day", "start the workday", "start work", "start working", "begin my workday", "begin my work day", "workday start", "open my work stuff", "open my work apps"].contains(&t) {
        return go("workday".into());
    }
    if ["my workday", "set up my workday", "edit my workday", "change my workday"].contains(&t) {
        return go("desk workday".into());
    }
    if let Some(what) = s.strip_prefix("add ").and_then(|r| r.strip_suffix(" to my workday").or_else(|| r.strip_suffix(" to my work day")).or_else(|| r.strip_suffix(" to the workday"))) {
        return go(format!("workday add {}", keep(raw, what)));
    }
    if let Some(r) = people_talk(s, raw) {
        return Some(r);
    }
    if let Some(r) = agenda(s, raw) {
        return Some(r);
    }
    // the reading list: "save this link", "read it later", "my reading list"
    if ["save this link", "save the link", "save that link", "save link", "read later", "read this later", "read it later", "save for later", "save it for later", "save this for later", "add to my reading list", "add this to my reading list", "add it to my reading list"].contains(&s) {
        return go("readlater".into());
    }
    if has(s, &["reading list", "my saved links", "what did i save to read", "links i saved", "read later list"]) && !s.starts_with("add ") {
        return go("reading".into());
    }
    let about_todos = has(s, &["to do", "to do list", "todo", "todos", "todo list", "list", "tasks"]) && !has(s, &["notes", "note"]);
    let about_notes = has(s, &["notes", "note", "reminders"]) || about_todos;
    if about_notes && has(s, &["delete", "clear", "erase", "remove", "wipe", "forget", "empty", "reset"]) && !s.starts_with("note ") && !adding_todo(s) {
        // the to-do list only ever loses finished items, and both ask first
        return go(if about_todos { "todos clear" } else { "forget notes" }.into());
    }
    // rescheduling: "move call Anna to friday", "push the report to tomorrow", "the report has no deadline"
    let today = chrono::Local::now().date_naive();
    if let Some(rest) = after(s, &["move ", "push ", "reschedule ", "postpone ", "put off ", "delay ", "shift "]) {
        if let Some((what, when)) = rest.rsplit_once(" to ").or_else(|| rest.rsplit_once(" until ")).or_else(|| rest.rsplit_once(" till ")) {
            let what = what.trim_start_matches("the ").trim_start_matches("my ").trim();
            let all_overdue = ["overdue", "everything overdue", "all overdue", "overdue to dos", "overdue todos", "overdue tasks", "my overdue to dos", "all my overdue to dos"].contains(&what);
            if all_overdue && when == "today" {
                return go("todos overdue today".into());
            }
            if crate::todo::parse_when(when, today).is_some_and(|d| d.is_some()) && !what.is_empty() && what.split_whitespace().count() <= 8 {
                return go(format!("todos move {}|{when}", keep(raw, what)));
            }
        }
    }
    if let Some(what) = s.strip_suffix(" has no deadline").or_else(|| s.strip_suffix(" has no due date")).or_else(|| s.strip_suffix(" does not have a deadline")).or_else(|| after(s, &["no deadline for ", "no due date for ", "remove the deadline from ", "remove the due date from "])) {
        let what = what.trim_start_matches("the ").trim_start_matches("my ").trim();
        if !what.is_empty() {
            return go(format!("todos move {}|none", keep(raw, what)));
        }
    }
    let t = s.trim_end_matches('?');
    for (phrase, when) in [("this week", "week"), ("today", "today")] {
        let asks = ["what did i finish ", "what did i get done ", "what have i finished ", "what have i done ", "what did i complete ", "what did i accomplish ", "what did i achieve ", "what i finished ", "what i got done "];
        if asks.iter().any(|a| t == format!("{a}{phrase}")) || t == format!("finished {phrase}") || t == format!("done {phrase}") {
            return go(format!("todos done {when}"));
        }
    }
    // "done call anna", "mark the report as done", "tick off milk"
    if let Some(what) = after(s, &["done with ", "mark off ", "tick off ", "check off ", "cross off "]).or_else(|| {
        s.strip_prefix("mark ").and_then(|r| r.strip_suffix(" as done").or_else(|| r.strip_suffix(" done")).or_else(|| r.strip_suffix(" as finished")))
    }) {
        let what = what.trim_start_matches("the ");
        if ["it", "this", "that", "them", "everything", "all", "you", "life", "today", "work"].contains(&what) {
            return None; // "done with it" is a feeling, not an item
        }
        return go(format!("done {}", keep(raw, what)));
    }
    if ["to do list", "todo list", "to do", "my to do", "to dos", "the to do list", "show to do list"].contains(&s) {
        return go("todos".into()); // "to do list" is asking to see it, not adding an item called "list"
    }
    let adding = s.starts_with("add ") || s.starts_with("put ");
    if !adding && has(s, &["my to do", "my to do list", "my list", "my todos", "my todo list", "my tasks", "what is on my list", "what do i need to do", "what is on my to do list", "what do i have to do", "show my list"]) {
        return go("todos".into());
    }
    if s == "notes" || has(s, &["my notes", "show notes", "list notes", "read notes", "what did i ask you to remember", "what did i tell you to remember", "what should i remember", "all notes"]) {
        return go("notes".into());
    }
    if let Some(what) = after(s, &["to do ", "todo ", "add to my list ", "add to my to do list ", "put on my list ", "new task ", "add a task "]).filter(|w| !w.is_empty()) {
        return go(format!("todo {}", keep(raw, what)));
    }
    const P: &[&str] = &[
        "note that ", "note to self ", "note down ", "note: ", "note ", "make a note that ", "make a note to ", "make a note of ",
        "make a note ", "take a note that ", "take a note ", "write down that ", "write down ", "jot down ", "remember that ",
        "remember to ", "remember ", "do not let me forget to ", "do not let me forget ", "add a note ", "save a note ",
        "new note ", "write a note ",
    ];
    let not_a_note = |w: &str| ["me", "us", "him", "her", "them", "this", "that"].contains(&w) || ["when ", "the time ", "how ", "what ", "who ", "where ", "why ", "if "].iter().any(|p| w.starts_with(p));
    if let Some(what) = after(s, P).filter(|w| !w.is_empty() && !(s.starts_with("remember ") && not_a_note(w))) {
        return go(format!("note {}", keep(raw, what)));
    }
    // "add milk to my list" (a to-do), "add the code to my notes" (a note)
    let rest = s.strip_prefix("add ").or_else(|| s.strip_prefix("put "))?;
    const LISTS: &[&str] = &[
        " to my to do list", " to the to do list", " to my todo list", " to my todos", " to my tasks",
        " to my list", " to the list", " on my list", " on my to do list", " on the list",
    ];
    if let Some(what) = rest.strip_suffix(" to my notes").or_else(|| rest.strip_suffix(" to the notes")) {
        return go(format!("note {}", keep(raw, what)));
    }
    let what = LISTS.iter().find_map(|l| rest.strip_suffix(l))?;
    go(format!("todo {}", keep(raw, what)))
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
    // "from 40 to 50 in percent" (the help's own example)
    if let Some(r) = s.strip_prefix("from ").and_then(|r| r.strip_suffix(" in percent").or_else(|| r.strip_suffix(" in %")).or_else(|| r.strip_suffix(" as a percentage"))) {
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
    if has(s, &["the iss", "space station"]) {
        return go("iss".into()); // "where is the ISS"
    }
    // "where is my phone", "where are my keys": not on a map
    if let Some(thing) = after(s, &["where is my ", "where are my ", "where did i put my ", "where did i leave my "]) {
        if !FILE_WORDS.iter().any(|f| thing.contains(f)) {
            return Some(Reply::say(lines::pick("lost_things").replace("{x}", thing)).mood(Mood::Think));
        }
    }
    if let Some(q) = after(s, &["where is ", "where are ", "where can i find ", "directions to ", "how do i get to ", "how to get to ", "how far is ", "map of ", "a map of ", "on the map ", "locate ", "find on the map ", "navigate to ", "route to "]) {
        return go(format!("map {q}"));
    }
    if let Some(t) = after(s, &["open up ", "open ", "launch ", "start up ", "start ", "fire up ", "boot up ", "load ", "bring up ", "pull up ", "go to ", "visit ", "take me to ", "browse to ", "switch to ", "run the ", "show my "]) {
        // "go to bed", "go to work", "go to sleep": not a website called bed.com
        let place = ["go to ", "visit ", "take me to ", "browse to "].iter().any(|p| s.starts_with(p));
        const PLACES: &[&str] = &["bed", "work", "sleep", "school", "lunch", "dinner", "breakfast", "the gym", "gym", "church", "the shops", "the shop", "the store", "the doctor", "the dentist", "hell", "town", "class", "war", "the park", "the office", "the beach", "a party", "the party", "the bathroom", "the toilet", "the loo", "the movies", "the cinema", "university", "uni", "college", "kindergarten", "prison", "jail", "the pub", "a bar", "the bar", "work early", "bed early", "the hospital", "hospital", "the vet", "my room", "your room", "the kitchen", "the airport", "the station", "the moon", "space", "heaven", "the next level", "the point", "the top", "the bottom", "the end"];
        if place && (PLACES.contains(&t) || t.contains(' ') && !t.contains('.') && !t.contains('/')) {
            return None; // "go to bed", "go to the shops": not a website
        }
        let target = resolve(t, raw);
        if s.starts_with("show my ") && target == format!("{t}.com") {
            return None; // "show my stats" isn't stats.com
        }
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
        if ["clipster", "you", "yourself"].contains(&t) {
            return go("bye".into());
        }
        // only a program that's running: "close the browser" means its name, "stop worrying" isn't a program
        let t = t.trim_start_matches("the ").trim_start_matches("my ").trim_start_matches("this ");
        let word = t.split_whitespace().next()?;
        let word = match word {
            "browser" | "web" => if cfg!(windows) { "msedge" } else { "firefox" },
            w => w,
        };
        let name = crate::sys::program_name(word);
        if crate::sys::program_memory_kb(&name).is_empty() {
            // "stop worrying", "end the meeting": not about a program. "close firefox" when it isn't open: say so
            let asked = ["close ", "quit ", "kill ", "exit ", "shut down "].iter().any(|v| s.starts_with(v));
            return asked.then(|| Reply::say(format!("{name} isn't running, as far as I can see.")).mood(Mood::Think));
        }
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
    if ["fix the easy ones", "fix it", "fix the easy mistakes", "fix my mistakes", "fix the mistakes", "fix the typos", "fix my typos"].contains(&s) {
        return go("proofread fix".into());
    }
    // "check my text", "proofread this": what's on the clipboard
    if has(s, &["proofread", "proof read", "check my text", "check this text", "check the text", "check my writing", "check my spelling", "spell check", "spellcheck", "check my grammar", "grammar check", "fix my text"]) {
        let rest = after(s, &["proofread ", "proof read ", "spell check ", "spellcheck "]).filter(|r| r.split_whitespace().count() > 2).map(|r| keep(raw, r));
        return go(format!("proofread {}", rest.unwrap_or_default()));
    }
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
    if knowledge(s) {
        return None; // "what is a coin", "who invented dice"
    }
    if has(s, &["flip a coin", "coin flip", "toss a coin", "coin toss", "heads or tails", "flip coin"]) || ["flip", "coin", "flip it"].contains(&s) {
        return go("flip".into());
    }
    if s.starts_with("roll ") || s == "roll" || has(s, &["dice", "throw a die", "roll a die", "roll the die"]) {
        let spec = s.split_whitespace().find(|w| w.split_once('d').is_some_and(|(a, b)| a.chars().all(|c| c.is_ascii_digit()) && !b.is_empty() && b.chars().all(|c| c.is_ascii_digit())));
        let n = s.split_whitespace().find_map(|w| w.parse::<u32>().ok()).unwrap_or(1);
        return go(format!("roll {}", spec.map(String::from).unwrap_or(format!("{n}d6"))));
    }
    if has(s, &["random number", "pick a number", "number between", "random between", "a random", "rng", "number from"]) {
        return go(format!("random {s}"));
    }
    let making = s.starts_with("password") || s.starts_with("passphrase") || has(s, &["generate", "make", "create", "new", "random", "strong", "give me", "suggest", "need a"]);
    if has(s, &["password", "passwords", "passphrase", "passcode", "pass word"]) && making && !has(s, &["my password", "wifi password", "not working", "forgot", "manager"]) {
        let n = s.split_whitespace().find_map(|w| w.parse::<u32>().ok()).unwrap_or(16);
        return go(format!("password {n}"));
    }
    if let Some(list) = after(s, &["pick between ", "choose between ", "decide between ", "pick one of ", "choose one of ", "pick from ", "choose from ", "pick ", "choose ", "which one ", "which should i pick ", "which should i choose "]) {
        return go(format!("pick {}", list.replace(" and ", ", ").replace(" or ", ", ")));
    }
    if has(s, &["magic 8", "8 ball", "8ball", "eight ball", "yes or no", "predict"]) {
        return go("8ball".into());
    }
    None
}

fn chat(s: &str) -> Option<Reply> {
    // "what can you do with files?", "help with writing": that part of the help
    const TOPICS: &[(&[&str], &str)] = &[
        (&["timer", "timers", "reminder", "reminders", "alarm", "alarms", "notes", "todo", "todos", "to do", "focus"], "timers"),
        (&["writing", "write", "letters", "letter", "text", "proofread", "proofreading", "spelling", "words"], "writing"),
        (&["files", "file", "folders", "folder", "downloads"], "files"),
        (&["computer", "system", "pc", "laptop", "volume", "screen"], "computer"),
        (&["internet", "web", "online", "weather", "news"], "internet"),
        (&["math", "maths", "numbers", "conversions", "convert", "calculations", "units"], "numbers"),
        (&["fun", "games", "game", "jokes"], "fun"),
        (&["desk", "stats", "statistics"], "desk"),
    ];
    if has(s, &["what can you do", "help", "how do you work", "what can i ask", "features", "commands"]) {
        if let Some((_, topic)) = TOPICS.iter().find(|(kws, _)| has(s, kws)) {
            return go(format!("help {topic}"));
        }
    }
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
    // "write a complaint letter", "I need a cover letter", "out of office message"
    if has(s, &["letter", "write an email", "write a email", "cover letter", "formal letter", "out of office", "email template", "letter template", "resignation", "write a note to my landlord", "thank you email", "thank you note", "thank you letter", "thank you mail", "thank-you email", "thank-you note", "thank-you letter"]) {
        return go(format!("letter {}", crate::writing::template_for(s).unwrap_or_default()));
    }
    for (kws, cmd) in [
        (&["joke", "jokes", "funny", "laugh", "bored", "humor", "humour", "entertain me", "make me smile", "cheer me up", "something fun", "lol"][..], "joke"),
        (&["roast", "insult me", "make fun of me", "burn me", "be mean", "diss me", "trash talk"][..], "roast"),
        (&["fact", "facts", "something interesting", "teach me something", "did you know", "something i do not know", "tell me something", "learn something"][..], "fact"),
        (&["compliment", "something nice", "i am sad", "i feel sad", "feeling down", "feel bad", "bad day", "hug", "lonely", "encourage me", "sad"][..], "hug"),
        // the emoji quiz before the quiz, or "emoji quiz" is a trivia question
        (&["emoji quiz", "emoji game", "guess the emoji", "emoji puzzle", "emoji riddle"][..], "emoji quiz"),
        (&["quiz", "quiz me", "trivia", "test me", "ask me a question", "trivia question"][..], "quiz"),
        (&["quote", "quotes", "inspire me", "inspiration", "inspirational", "motivate me", "motivation", "motivational", "wise words", "wisdom"][..], "quote"),
        (&["advice", "give me advice", "life advice"][..], "advice"),
        (&["riddle", "riddles", "riddle me", "a riddle"][..], "riddle"),
        (&["would you rather", "wyr", "play would you rather", "ask me would you rather"][..], "wyr"),
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
        (&["shut up", "be quiet", "go away", "leave me alone", "stop talking", "silence", "mute yourself", "shh", "stfu", "quiet", "hush", "pipe down", "stop bothering me", "not now", "annoying me"][..], "quiet"),
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
        (&["install the update", "install update", "update now", "update yourself now", "install the new version"][..], "install update"),
        (&["check for updates", "check for update", "check for a new version", "is there an update", "is there a new version", "update clipster", "update yourself", "new version of clipster", "what version are you", "which version are you", "your version", "clipster version"][..], "updates"),
        (&["holiday", "holidays", "public holiday", "bank holiday", "day off", "days off"][..], "holiday"),
        (&["local ip", "lan", "local network", "wifi address", "network interfaces"][..], "lan"),
        (&["public ip", "where am i", "my location", "my ip", "ip address", "ip", "my isp", "internet provider"][..], "ip"),
    ] {
        if has(s, kws) {
            return go(cmd.into());
        }
    }
    // a picture on "show me a dog", "dog pic", "puppies"; never for "my dog died" or "the cat is sick"
    let sad = has(s, &["died", "dead", "dying", "sick", "ill", "lost", "passed", "passed away", "missing", "put down", "vet", "hurt", "injured", "gone"]);
    let picture = !sad && (words <= 2 || has(s, &["picture", "pic", "pics", "photo", "photos", "image", "gif", "random", "show me", "cute"]));
    for (kws, kind) in [(&["dog", "dogs", "puppy", "puppies", "doggo", "doggy", "pupper"][..], "dog"), (&["cat", "cats", "kitten", "kittens", "kitty"][..], "cat"), (&["fox", "foxes"][..], "fox")] {
        if picture && has(s, kws) {
            return go(kind.into());
        }
    }
    for (kws, cmd) in [
        (&["what time", "the time", "time is it", "what day", "which day", "what date", "the date", "todays date", "what year", "day is it", "what month", "time now", "current time"][..], "time"),
        (&["battery", "charge left", "charging", "power left", "plugged in", "how much power"][..], "battery"),
        (&["disk", "disks", "storage", "space left", "free space", "hard drive", "ssd", "drive space", "disk space"][..], "disk"),
        (&["network", "wifi", "wi fi", "internet connection", "am i online", "am i connected"][..], "lan"),
        (&["uptime", "how long has my computer", "been on for", "been running", "since boot", "last reboot"][..], "uptime"),
        (&["hogging", "using all", "using the most", "slowing down my computer", "slowing my computer", "slowing down my pc", "what is using", "who is using", "most cpu", "task manager", "processes", "eating my ram", "eating my cpu", "eating my memory"][..], "top"),
        (&["how is my computer", "how is my pc", "how is my laptop", "computer doing", "pc doing", "laptop doing", "computer ok", "computer slow", "computer is slow", "pc is slow", "laptop is slow", "cpu usage", "ram usage", "memory usage", "system stats", "system status", "my specs", "computer specs", "system health", "computer health"][..], "stats"),
    ] {
        // encyclopedia questions ("what is ram", "what is a battery") are questions, except the time and date
        if has(s, kws) && (cmd == "time" || !knowledge(s)) {
            return go(cmd.into());
        }
    }
    // a word on its own: "time", "lock", "ram"
    let alone: &[(&[&str], &str)] = &[
        (&["time", "clock", "date", "today"], "time"),
        (&["lock", "lock it", "lock the screen", "lock screen", "lock my screen", "lock my computer", "lock the computer", "lock my pc", "lock the pc", "lock my laptop"], "lock"),
        (&["ram", "memory", "cpu", "processor", "stats", "status", "specs", "system", "performance", "load"], "stats"),
    ];
    if let Some((_, cmd)) = alone.iter().find(|(ws, _)| ws.contains(&s)) {
        return go(cmd.to_string());
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
            const REAL: &[&str] = &["better", "rolls", "rock", "rocks", "hours", "mock", "sock", "look", "book", "cook", "took", "hook", "late", "rate", "mate", "gate", "tell", "sell", "well", "fell", "bell", "vote", "word", "worm", "form", "fork", "home", "hope", "some", "same", "came", "make", "take", "lake", "cake", "bake", "fake", "wake", "more", "core", "bore", "sore", "store", "stone", "alone", "alive", "later", "water", "after", "other", "bitter", "butter", "matter", "winter", "dinner", "power", "tower", "lower", "flower", "money", "honey", "funny", "sunny", "happy", "sorry", "worry", "party", "times", "lines", "files", "miles", "smile", "while", "white", "write", "quite", "quiet", "quit", "wait", "want", "went", "west", "best", "test", "rest", "nest", "jest", "sent", "rent", "tent", "dent", "bent", "what", "that", "this", "then", "than", "them", "they", "there", "these", "those", "where", "were", "here", "hear", "near", "year", "dear", "fear", "gear", "tear", "wear", "bear", "beer", "deer", "peer", "seer", "need", "feed", "seed", "weed", "read", "lead", "dead", "head", "bread", "dread"];
            if w.len() < 4 || VOCAB.contains(&w) || REAL.contains(&w) || !w.chars().all(|c| c.is_ascii_alphabetic()) {
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
    // "work is stressful" is a statement, not a name to look up
    let sentence = words.iter().any(|w| ["is", "are", "was", "were", "am", "feel", "feels", "seems", "looks", "sucks", "rocks", "hurts", "go", "die", "be"].contains(w));
    if knowledge(s) || (!personal && !sentence && words.len() <= 3 && scored.is_empty()) {
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
    fn told(pairs: &[(&str, &str)]) -> Effect {
        Effect::Remember(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    }

    #[test]
    fn facts_about_you() {
        assert_eq!(eff("my cat is called Mo"), told(&[("pet", "Mo"), ("pet_kind", "cat")]));
        assert_eq!(eff("I have a dog named Rex Junior."), told(&[("pet", "Rex Junior"), ("pet_kind", "dog")]));
        assert_eq!(eff("my wife's name is Anna"), told(&[("partner", "Anna"), ("partner_word", "wife")]));
        assert_eq!(eff("I work as a nurse"), told(&[("job", "a nurse")]));
        assert_eq!(eff("I live in Bern"), told(&[("city", "Bern")]));
        assert_eq!(eff("I'm from New York"), told(&[("city", "New York")]));
        assert_eq!(eff("my favourite color is green"), told(&[("fav_color", "green")]), "a fact, not a neon skin");
        assert_eq!(eff("my favorite film is Alien"), told(&[("fav_movie", "Alien")]));
        assert!(!matches!(eff("my cat is sleeping on my keyboard"), Effect::Remember(_)), "no name, no fact");
        assert!(!matches!(eff("I live in fear of Mondays"), Effect::Remember(_)));
        assert_eq!(eff("forget my job"), Effect::Forget(vec!["job".into()]));
        let all = respond("forget everything about me");
        assert!(all.effect == Effect::None && all.buttons.iter().any(|(_, a)| matches!(a, Act::Run(c) if c == "forgetfact all")), "everything only after a yes");
        assert_eq!(eff("forget notes"), respond("forget notes").effect, "notes keep their own forget");
        assert!(!matches!(eff("forget my notes"), Effect::Forget(_)));
        assert!(!text("what's my ip").contains("told me"), "not a question about facts");
    }

    #[test]
    fn proofreading_in_plain_words() {
        let r = respond("proofread Please send the the report today.");
        assert!(r.buttons.iter().any(|(_, a)| matches!(a, crate::brain::Act::Run(c) if c == "proofread fix")), "offers the easy fixes: {} {:?}", r.text, r.buttons);
    }

    #[test]
    fn settings_live_on_the_desk() {
        assert!(matches!(respond("pref clip off").effect, Effect::Pref(k, v) if k == "clip" && v == "off"));
        assert!(matches!(respond("pref budget video 30").effect, Effect::Pref(k, v) if k == "budget" && v == "video 30"));
        for s in ["settings", "options", "change your settings"] {
            assert!(matches!(respond(s).effect, Effect::OpenDesk(t) if t == "settings"), "{s}");
        }
        assert!(matches!(respond("desk week").effect, Effect::OpenDesk(t) if t == "week"));
        assert!(!matches!(respond("desk lamp").effect, Effect::OpenDesk(_)), "only real tab names");
        assert_eq!(respond("clip copy h2").effect, Effect::Clip("copy h2".into()));
        assert_eq!(respond("forgetfact pet").effect, Effect::Forget(vec!["pet".into()]));
        assert_eq!(respond("forgetfact all").effect, Effect::Forget(vec![]));
        assert!(matches!(respond("desk you").effect, Effect::OpenDesk(t) if t == "you"));
    }

    #[test]
    fn goals_limits_and_the_day_in_plain_words() {
        let pref = |s: &str| match respond(s).effect {
            Effect::Pref(k, v) => format!("{k} {v}"),
            e => format!("{e:?}"),
        };
        assert_eq!(pref("set my focus goal to 2 hours"), "goal 120");
        assert_eq!(pref("focus goal 90 minutes"), "goal 90", "a goal, not a focus session");
        assert_eq!(pref("goal 45"), "goal 45");
        assert_eq!(pref("remove my focus goal"), "goal 0");
        assert_eq!(pref("limit video to 30 minutes a day"), "budget video 30");
        assert_eq!(pref("limit social media to 20 minutes"), "budget social 20");
        assert_eq!(pref("no limit for youtube"), "budget video 0");
        assert!(!respond("set my focus goal to 2 hours").text.is_empty(), "typed: he answers");
        assert_eq!(respond("how am i doing on my goals").effect, Effect::Goals);
        assert_eq!(respond("wrap up my day").effect, Effect::Wrapup);
        assert_eq!(respond("my week in review").effect, Effect::Review(false));
        assert_eq!(respond("review copy").effect, Effect::Review(true));
        let plan = respond("plan my day");
        assert!(matches!(&plan.effect, Effect::OpenDesk(t) if t == "plan") || plan.buttons.iter().any(|b| b.0 == "Open the Desk"), "plan my day: {}", plan.text);
        assert!(matches!(respond("todo #3 star").effect, Effect::Todo(crate::todo::Op::StarAt(3))));
        assert!(matches!(respond("todo #2 edit call Anna").effect, Effect::Todo(crate::todo::Op::EditAt(2, t)) if t == "call Anna"));
        assert!(matches!(respond("todo water the plants every monday").effect, Effect::Todo(crate::todo::Op::Add(_))));
    }

    #[test]
    fn desk_commands_and_todo_words_arent_hijacked() {
        assert_eq!(respond("upkeep check").effect, Effect::UpkeepCheck);
        assert!(respond("upkeep cleartemp").buttons.iter().any(|b| b.0.starts_with("Yes")));
        for t in ["todo clear the desk", "todo wrap up the report", "todo review my week", "todo my goals", "todo remind Anna on friday", "to do empty the dishwasher"] {
            assert!(matches!(respond(t).effect, Effect::Todo(crate::todo::Op::Add(_))), "{t}: {:?}", respond(t).effect);
        }
        assert!(matches!(respond("done wrap up report").effect, Effect::Todo(crate::todo::Op::Done(_))));
        assert!(!matches!(respond("todo clear").effect, Effect::Todo(crate::todo::Op::Add(_))), "that's clearing the finished ones");
        assert_eq!(respond("wrap up my day").effect, Effect::Wrapup);
        assert_eq!(respond("how am i doing on my goals").effect, Effect::Goals);
    }

    #[test]
    fn tips_for_the_app_youre_in() {
        assert_eq!(eff("any tips for this app?"), Effect::AppTips);
        assert_eq!(eff("keyboard shortcuts"), Effect::AppTips);
    }

    #[test]
    fn tidying_in_plain_words() {
        assert_eq!(eff("undo the tidy"), Effect::TidyUndo);
        assert!(matches!(eff("find duplicates in my downloads"), Effect::Net(Query::Dupes(p)) if p == "downloads"));
        assert!(matches!(eff("show me big old files"), Effect::Net(Query::OldFiles(_))));
        let plan = respond("tidy up my desktop");
        assert!(plan.effect == Effect::None, "tidying needs a yes first");
        assert_eq!(respond("tidy desktop confirmed").effect, Effect::Tidy("desktop".into()));
    }

    #[test]
    fn focus_in_plain_words() {
        assert_eq!(eff("focus for 45 minutes on the report"), Effect::Focus(2700, "the report".into()));
        assert_eq!(eff("help me focus"), Effect::Focus(1500, String::new()));
        assert_eq!(eff("stop focusing"), Effect::FocusStop);
        assert_eq!(eff("where did my time go today?"), Effect::TimeReport);
    }

    #[test]
    fn todos_in_plain_words() {
        use crate::todo::Op;
        assert_eq!(eff("add call Anna to my to do list"), Effect::Todo(Op::Add("call Anna".into())));
        assert_eq!(eff("todo send the report by friday"), Effect::Todo(Op::Add("send the report by friday".into())));
        assert_eq!(eff("add milk to my list"), Effect::Todo(Op::Add("milk".into())));
        assert_eq!(eff("mark the report as done"), Effect::Todo(Op::Done("report".into())));
        assert_eq!(eff("tick off milk"), Effect::Todo(Op::Done("milk".into())));
        assert_eq!(eff("done call Anna"), Effect::Todo(Op::Done("call Anna".into())));
        assert!(!matches!(eff("clear my to do list"), Effect::Todo(_)), "asks first");
        assert!(matches!(eff("note buy milk"), Effect::Note(n) if n == "add buy milk"), "notes are still notes");
    }

    #[test]
    fn the_workday() {
        assert_eq!(eff("start my workday"), Effect::Workday("start".into()));
        assert_eq!(eff("add Firefox to my workday"), Effect::Workday("add Firefox".into()));
        assert_eq!(respond("workday add https://mail.example.com").effect, Effect::Workday("add https://mail.example.com".into()));
        assert!(matches!(eff("my workday"), Effect::OpenDesk(t) if t == "workday"));
    }

    #[test]
    fn people_in_plain_words() {
        assert_eq!(eff("Anna's birthday is on 3 March"), Effect::People("birthday Anna|on 3 March".into()));
        assert_eq!(eff("my mum's birthday is March 3rd"), Effect::People("birthday mum|March 3rd".into()));
        assert_eq!(eff("Ben's email is ben@example.org"), Effect::People("email Ben|ben@example.org".into()));
        assert_eq!(eff("keep in touch with Mum every 2 weeks"), Effect::People("every Mum|14".into()));
        assert_eq!(eff("my birthday is june 3"), Effect::SetBirthday(6, 3), "your own birthday is still yours");
        assert_eq!(eff("upcoming birthdays"), Effect::People("birthdays".into()));
        crate::people::set_names(&[crate::people::Person::new("Mum")]);
        assert_eq!(eff("I called mum"), Effect::People("met mum".into()));
        assert!(!matches!(eff("I talked to my boss"), Effect::People(_)), "only people he knows");
        assert_eq!(eff("when did I last talk to Mum"), Effect::People("last Mum".into()));
        assert_eq!(eff("I called my mum today"), Effect::People("met mum".into()));
        assert_eq!(eff("keep in touch with Mum every other week"), Effect::People("every Mum|14".into()));
        assert!(matches!(eff("İpek's birthday is on 3 March"), Effect::People(c) if c == "birthday İpek|on 3 March"), "{:?}", eff("İpek's birthday is on 3 March"));
    }

    #[test]
    fn the_calendar_in_plain_words() {
        assert_eq!(eff("what's on tomorrow?"), Effect::Calendar("tomorrow".into()));
        assert_eq!(eff("my agenda"), Effect::Calendar("today".into()));
        assert_eq!(eff("when is my next meeting"), Effect::Calendar("next".into()));
        assert_eq!(eff("add my calendar webcal://example.com/Cal/SeCrEt.ics"), Effect::Calendar("add webcal://example.com/Cal/SeCrEt.ics".into()), "the link as typed");
        let friday = match eff("what's on friday") {
            Effect::Calendar(c) => c,
            e => panic!("{e:?}"),
        };
        assert!(friday.starts_with("calendar day ") || friday.starts_with("day "), "{friday}");
        match eff("am I free on Friday at 3") {
            Effect::Calendar(c) => {
                let t: i64 = c.strip_prefix("free ").unwrap().parse().unwrap();
                let at = chrono::DateTime::from_timestamp(t, 0).unwrap().with_timezone(&chrono::Local);
                assert_eq!((chrono::Datelike::weekday(&at), chrono::Timelike::hour(&at)), (chrono::Weekday::Fri, 15));
            }
            e => panic!("{e:?}"),
        }
        assert!(matches!(eff("am i free friday at 3"), Effect::Calendar(c) if c.starts_with("free ")));
        assert!(matches!(eff("am i busy at 11:30pm"), Effect::Calendar(c) if c.starts_with("free ")) || chrono::Timelike::hour(&chrono::Local::now()) == 23);
        assert!(!matches!(eff("add lunch with Anna to my calendar"), Effect::Calendar(_)), "no link, not a calendar to link");
        assert!(!matches!(respond("calendar app").effect, Effect::Calendar(_)), "a literal head with an unknown verb isn't a command");
        assert!(!matches!(respond("workday was long").effect, Effect::Workday(_)));
        assert!(!matches!(respond("people are strange").effect, Effect::People(_)));
        assert_eq!(respond("calendar https://example.com/cal/basic.ics").effect, Effect::Calendar("add https://example.com/cal/basic.ics".into()));
    }

    #[test]
    fn the_reading_list() {
        assert_eq!(eff("save this link"), Effect::Reading("save".into()));
        assert_eq!(eff("read it later"), Effect::Reading("save".into()));
        assert_eq!(eff("show my reading list"), Effect::Reading("list".into()));
        assert_eq!(respond("readlater https://example.com/a?b=1").effect, Effect::Reading("save https://example.com/a?b=1".into()));
        assert_eq!(respond("reading done 2").effect, Effect::Reading("done 2".into()));
        assert!(crate::brain::react_clip("https://example.org/article").unwrap().buttons.iter().any(|(l, _)| l == "Read later"));
        assert_eq!(crate::brain::short_link("https://www.example.com/a/b/?utm=x"), "example.com/a/b");
        assert!(crate::brain::is_link("https://example.com/x") && !crate::brain::is_link("see https://example.com"));
    }

    #[test]
    fn named_lists_in_plain_words() {
        use crate::todo::Op;
        let add = |l: &str, items: &[&str]| Effect::Todo(Op::AddTo(l.into(), items.iter().map(|s| s.to_string()).collect()));
        assert_eq!(eff("add eggs, milk and bread to my shopping list"), add("shopping", &["eggs", "milk", "bread"]));
        assert_eq!(eff("put Sunscreen on the packing list"), add("packing", &["Sunscreen"]));
        assert!(text("what's on my shopping list").len() > 0);
        assert_eq!(respond("what's on my shopping list").buttons.len(), respond("onlist shopping").buttons.len());
        assert_eq!(eff("clear my shopping list"), Effect::Todo(Op::ClearList("shopping".into())));
        assert_eq!(eff("add milk to my list"), Effect::Todo(Op::Add("milk".into())), "no name: the to-do list");
        assert!(!matches!(eff("clear my to do list"), Effect::Todo(Op::ClearList(_))));
        assert!(!matches!(eff("add lunch with Anna to my calendar"), Effect::Todo(Op::AddTo(..))));
        assert!(!matches!(eff("show my reading list"), Effect::Todo(_)), "the reading list is something else");
        assert!(!matches!(eff("add milk to the whole list"), Effect::Todo(Op::AddTo(..))), "'whole' isn't a list name");
        assert!(!matches!(eff("clear the whole list"), Effect::Todo(Op::ClearList(_))));
    }

    #[test]
    fn help_by_topic() {
        assert!(text("what can you do with files?").contains("tidy my downloads"));
        assert!(text("help with writing").contains("check my text"));
        assert!(respond("help").buttons.iter().any(|(_, a)| matches!(a, crate::brain::Act::Run(c) if c == "help desk")));
        assert!(respond("count hello world").buttons.iter().any(|(l, _)| l == "Copy it"));
    }

    #[test]
    fn reminders_on_other_days() {
        let timer = |s: &str| match eff(s) {
            Effect::Timer(secs, label) => Some((secs, label)),
            _ => None,
        };
        let day = 24 * 3600;
        let (secs, label) = timer("remind me tomorrow at 3 to call Anna").expect("tomorrow");
        assert_eq!(label, "call Anna");
        assert!(secs > 3600 && secs < 2 * day, "tomorrow, not today: {secs}");
        let (secs, label) = timer("remind me to water the plants on friday").expect("a weekday");
        assert_eq!(label, "water the plants");
        assert!(secs >= 60 && secs <= 7 * day + 3600);
        let (secs, _) = timer("remind me in 2 days to pay rent").expect("in 2 days");
        assert!(secs > day && secs < 3 * day, "2 days, not 2 minutes: {secs}"); // 9:00 on that day, so up to 2 days and 9 hours
        let (secs, label) = timer("remind me next monday at 9:30am about the dentist").expect("next monday");
        assert_eq!(label, "dentist");
        assert!(secs > 3600);
        assert!(timer("remind me on june 3 to send the report").is_some());
        assert!(matches!(eff("remind me in 10 minutes to stretch"), Effect::Timer(600, _)), "plain timers still work");
        assert_eq!(eff("cancel the tea timer"), Effect::CancelTimer("tea".into()));
        assert_eq!(eff("cancel all timers"), Effect::CancelTimers);
    }

    #[test]
    fn notes_are_never_wiped_without_asking() {
        assert!(exact("forget it").is_none(), "\"forget it\" isn't about notes");
        for s in ["forget notes", "clear my notes", "forget"] {
            let r = respond(s);
            assert!(r.buttons.iter().all(|(_, a)| !matches!(a, crate::brain::Act::Run(c) if c == "forget notes confirmed")) || r.text.contains("Forget all"), "{s} asks first");
        }
        assert!(!respond("forget it").text.contains("Poof"));
    }

    #[test]
    fn follow_ups_and_answers() {
        for s in ["why?", "really?", "tell me more", "go on"] {
            assert!(!matches!(eff(s), Effect::Net(_)), "{s} is about what he said, not a lookup");
        }
        assert_eq!(answer_to("city", "Bern").effect, told(&[("city", "Bern")]));
        assert_eq!(answer_to("city", "in New York").effect, told(&[("city", "New York")]));
        assert_eq!(answer_to("job", "I'm a teacher").effect, told(&[("job", "a teacher")]));
        assert_eq!(answer_to("pet", "Mo").effect, told(&[("pet", "Mo"), ("pet_kind", "pet")]));
        assert_eq!(answer_to("pet", "I have a cat called Mo").effect, told(&[("pet", "Mo"), ("pet_kind", "cat")]));
        assert_eq!(answer_to("pet", "nope").effect, Effect::None, "no pet, no fact");
        assert_eq!(answer_to("lunch", "soup").effect, Effect::None, "not a fact question");
        assert_eq!(respond("answer-to city Zurich").effect, told(&[("city", "Zurich")]), "taken literally");
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
        assert_eq!(eff("start the tour"), Effect::Tour);
        assert_eq!(eff("show me around"), Effect::Tour);
        assert_eq!(eff("wear a halo"), Effect::Hat("halo".into()));
        assert_eq!(eff("turn yourself beige"), Effect::Skin("beige".into()));
        assert_eq!(eff("let's play minesweeper"), Effect::Play("bughunt".into()));
        assert_eq!(eff("i want to play chess"), Effect::Play("chess".into()));
        assert_eq!(eff("let's play 2048"), Effect::Play("bytesize".into()));
        assert_eq!(eff("let's play snake"), Effect::Play("clipchain".into()));
        assert_eq!(eff("open clip chain"), Effect::Play("clipchain".into()));
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
        assert!(respond("what can you do").buttons.iter().any(|(_, a)| matches!(a, crate::brain::Act::Run(c) if c == "help numbers")), "help offers its topics");
        assert_eq!(keep("Note that Bob's birthday is Friday", "bob's birthday is friday"), "Bob's birthday is Friday");
        assert!(text("make HELLO lowercase").starts_with("hello"));
        assert!(matches!(eff("asdkjh qwe"), Effect::Net(Query::Ask(_))));
        assert_eq!(eff("find the file called Report"), Effect::Net(Query::Files("Report".into())));
        assert_eq!(eff("find my CV"), Effect::Net(Query::Files("CV".into())));
        assert_eq!(eff("where is my tax pdf"), Effect::Net(Query::Files("tax pdf".into())));
        assert_eq!(eff("where did I save the invoice"), Effect::Net(Query::Files("invoice".into())));
        assert_eq!(eff("recent files"), Effect::Net(Query::Recent));
        assert!(!matches!(eff("where is my phone"), Effect::Net(Query::Files(_))), "a phone isn't a file");
        assert!(!matches!(eff("find me a recipe for lasagne"), Effect::Net(Query::Files(_))));
        assert!(!matches!(eff("where is my motivation"), Effect::Net(Query::Files(_))));
        assert!(!matches!(eff("find my car keys"), Effect::Net(Query::Files(_))), "keys aren't files");
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
        assert!(matches!(eff("shut down firefox"), Effect::None)); // closes the app (or says it isn't running), not the computer
        assert!(!matches!(eff("stop worrying"), Effect::Shell(_)));
        assert!(!matches!(eff("stop"), Effect::Exit), "a bare stop is for a ringing alarm");
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

    #[test]
    fn rescheduling_and_finished() {
        use crate::todo::Op;
        let today = chrono::Local::now().date_naive();
        let tomorrow = today + chrono::Days::new(1);
        assert_eq!(eff("push the report to tomorrow"), Effect::Todo(Op::DueFor("report".into(), Some(tomorrow))));
        assert!(matches!(eff("move call Anna to friday"), Effect::Todo(Op::DueFor(w, Some(_))) if w == "call Anna"), "the text keeps its capitals");
        assert_eq!(eff("the report has no deadline"), Effect::Todo(Op::DueFor("report".into(), None)));
        assert_eq!(eff("move everything overdue to today"), Effect::Todo(Op::OverdueToToday));
        assert!(!matches!(eff("move the files to downloads"), Effect::Todo(_)), "a folder isn't a date");
        assert!(!matches!(eff("move to friday"), Effect::Todo(_)));
        assert!(respond("what did i finish this week").text.len() > 3);
        assert_eq!(exact("todo #3 due none").map(|r| r.effect), Some(Effect::Todo(Op::DueAt(3, None))));
        assert_eq!(exact("todo #3 due tomorrow").map(|r| r.effect), Some(Effect::Todo(Op::DueAt(3, Some(tomorrow)))));
        assert!(exact("todo #3 due blue").is_some_and(|r| r.effect == Effect::None), "an unknown day is answered, not dropped");
        assert_eq!(exact("todo #2 at 3pm").map(|r| r.effect), Some(Effect::Todo(Op::AtTimeAt(2, chrono::NaiveTime::from_hms_opt(15, 0, 0)))));
        assert_eq!(exact("todo #2 link none").map(|r| r.effect), Some(Effect::Todo(Op::LinkAt(2, None))));
        assert_eq!(exact("todo #2 steps add a, b and c").map(|r| r.effect), Some(Effect::Todo(Op::AddSteps(2, vec!["a".into(), "b".into(), "c".into()]))));
        assert_eq!(exact("onlist packing rename beach trip").map(|r| r.effect), Some(Effect::Todo(Op::RenameList("packing".into(), "beach trip".into()))));
        assert_eq!(exact("onlist packing delete").map(|r| r.effect), Some(Effect::None), "deleting a list asks first");
        assert_eq!(exact("note #2 pin").map(|r| r.effect), Some(Effect::Note("pin 2".into())));
        assert_eq!(exact("note #2 edit call Anna").map(|r| r.effect), Some(Effect::Note("edit 2 call Anna".into())));
        assert_eq!(exact("note #2 is the printer code").map(|r| r.effect), Some(Effect::Note("add #2 is the printer code".into())), "a note can start with #2");
    }

    #[test]
    fn crisis_phrases_get_the_helpline() {
        let helpline = |t: &str| respond(t).buttons.iter().any(|(_, a)| matches!(a, Act::Open(u) if u.contains("findahelpline")));
        for t in [
            "i want to die", "i wanna die", "i would like to die", "i need to die", "i want to be dead", "i wish i was dead", "i want it all to end",
            "life is not worth living", "i want to end it all", "kill me", "die", "please die", "let me die", "just let me die", "i want to die now",
            "I want to die today.", "i just want to die", "i want to kill myself",
        ] {
            assert!(helpline(t), "{t}: {}", respond(t).text);
        }
        for t in ["roll a die", "roll a d20", "this meeting is killing me", "my phone died", "kill firefox"] {
            assert!(!helpline(t), "{t} isn't a crisis");
        }
    }

    #[test]
    #[ignore]
    fn probe() {
        for t in std::env::var("PROBE").unwrap_or_default().split(';') {
            let r = respond(t);
            println!("{t:?} -> {:?} | {} | {:?}", r.effect, r.text.chars().take(90).collect::<String>(), r.buttons.iter().map(|b| (&b.0, &b.1)).collect::<Vec<_>>());
        }
    }

    #[test]
    fn loss_gets_kindness_not_jokes() {
        let kind = |t: &str| lines::all("grief").contains(&respond(t).text) || lines::all("grief").is_empty();
        for t in ["my dog died", "My cat died yesterday", "my grandma passed away", "our dog is dead", "i have cancer", "my dad was diagnosed with cancer", "the funeral is tomorrow"] {
            assert!(kind(t), "{t}: {}", respond(t).text);
            assert!(!matches!(respond(t).effect, Effect::Net(_)), "{t}: no photo, no lookup");
        }
        for t in ["my phone died", "my battery died", "my laptop is dead", "show me a dog", "i passed the exam", "my kettle just died", "the party died down"] {
            assert!(!lines::all("grief").contains(&respond(t).text), "{t} isn't a loss");
        }
    }

    #[test]
    fn ordinary_sentences_have_no_side_effects() {
        let shell = |t: &str| matches!(eff(t), Effect::Shell(_));
        for t in ["is the internet off", "my internet keeps cutting off", "what is a lock", "did i lock the door", "what is a screenshot", "rock and roll", "close the meeting notes"] {
            assert!(!shell(t), "{t}: {:?}", eff(t));
        }
        assert!(shell("turn the wifi off") && shell("lock") && shell("lock my computer"));
        assert!(!matches!(eff("find recipes on the internet"), Effect::Shell(_)));
        for t in ["what is a password", "my password is not working", "what is a coin", "i want to flip my mattress", "who invented dice", "i have no time", "today is a good day"] {
            let r = respond(t);
            assert!(!r.text.contains("Here's a fresh password") && !r.text.starts_with("Heads") && !r.text.starts_with("Tails") && !r.text.starts_with("🎲") && !r.text.starts_with("It's "), "{t}: {}", r.text);
        }
        assert!(!matches!(eff("start reboot"), Effect::Open(_)), "power programs ask first");
        assert!(!matches!(eff("open shutdown"), Effect::Open(_)));
        assert!(!matches!(eff("go to bed"), Effect::Open(_)) && !matches!(eff("go to sleep"), Effect::Open(_)));
        assert!(matches!(eff("go to reddit"), Effect::Open(u) if u == "https://reddit.com"));
        assert!(!matches!(eff("stop"), Effect::Exit) && !matches!(eff("stop it"), Effect::Exit) && !matches!(eff("end"), Effect::Exit));
        assert!(matches!(eff("quit clipster"), Effect::Exit));
        assert!(!matches!(eff("in 2024 i went to paris"), Effect::Timer(..)));
        assert!(!matches!(eff("in 5 years i want to be rich"), Effect::Timer(..)));
        assert!(matches!(eff("set an alarm for 6am"), Effect::Timer(s, _) if s > 60 * 60 || chrono::Timelike::hour(&chrono::Local::now()) == 5));
        assert!(matches!(eff("remind me on friday to water the plants"), Effect::Timer(_, l) if l == "water the plants"));
        assert!(!matches!(eff("call me later"), Effect::SetName(_)) && !matches!(eff("i go by bus"), Effect::SetName(_)) && !matches!(eff("can you call me in 5 minutes"), Effect::SetName(_)));
        assert!(matches!(eff("my name is Bob"), Effect::SetName(n) if n == "Bob"));
        assert!(!matches!(eff("my job is boring"), Effect::Remember(_)) && !matches!(eff("i live in constant fear"), Effect::Remember(_)) && !matches!(eff("i am from the future"), Effect::Remember(_)));
        assert!(matches!(eff("my partner's name is Art"), Effect::Remember(f) if f.iter().any(|(k, v)| k == "partner" && v == "Art")));
        assert!(!respond("where is my mouse").text.contains("You haven't told me"));
        assert_eq!(eff("remove eggs from my shopping list"), Effect::Todo(crate::todo::Op::RemoveFrom("shopping".into(), "eggs".into())));
        assert_eq!(eff("tick off eggs on my shopping list"), Effect::Todo(crate::todo::Op::DoneOn("shopping".into(), "eggs".into())));
        assert!(!matches!(eff("done with it"), Effect::Todo(_)));
        assert!(!matches!(eff("remember me?"), Effect::Note(_)) && !matches!(eff("remember when we played chess"), Effect::Note(_)));
        assert!(matches!(eff("back up now"), Effect::Backup(_)));
        assert!(respond("42 in binary").text.contains("101010"), "{}", respond("42 in binary").text);
        assert!(!respond("fortune").text.starts_with("🎱"));
        assert!(!matches!(eff("mute"), Effect::Quiet(_)));
        assert!(!matches!(eff("youtube tutorial on cooking"), Effect::Tour) && matches!(eff("tutorial"), Effect::Tour));
        assert!(!matches!(eff("if i say yes, will you tell me a joke"), Effect::Teach(..)));
        assert!(!matches!(eff("when i say yes, say no"), Effect::Teach(..)), "common words can't be taken over");
        assert!(!respond("time in istanbul").text.starts_with("It's "));
    }
}
