//! Everything Clipster knows how to do. No AI: just rules, tables and jokes.

use crate::lines;
use crate::net::{Kind, Query};
use rand::seq::SliceRandom;
use rand::Rng;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Run(String),
    Open(String),
    Copy(String),
    Options,
    Hide,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    None,
    Timer(u64, String),
    Shell(String),
    Open(String),
    Quiet(u64),
    ListTimers,
    CancelTimers,
    CancelTimer(String), // the timers whose label contains this
    Exit,
    Net(crate::net::Query),
    ClipHistory,
    Stopwatch(&'static str), // "start", "stop" or "show"
    CopyNow(String),
    Repeat(u64, String), // a reminder that comes back every N seconds
    RepeatAt(String, String), // a reminder that comes back by the calendar: rule ("weekday 08:00"), label
    TimerOp(u64, String),     // one timer by id: "cancel", "snooze 10m", "at friday at 3", "rename <text>"
    SetName(String),
    SayName,
    Hat(String),
    Skin(String),
    Autostart(bool),
    Costume(String, String), // hat, skin ("" = keep)
    Commands(Vec<String>),   // run each, like a macro
    SetBirthday(u32, u32),   // month, day
    Persona(bool),           // true = Unhinged
    Tour,                    // the welcome window: the tour (or the whole setup, the first time)
    Memory(bool),            // true = list everything remembered so far
    Teach(String, String, Vec<String>), // phrase, what to say, what to run
    Relearn(String, String, String, Vec<String>), // old phrase, new phrase, what to say, what to run
    Unlearn(String),
    ListAchievements,
    Play(String),            // open an arcade game
    Remember(Vec<(String, String)>), // facts about the user: ("pet", "Mo"), ("pet_kind", "cat")
    Forget(Vec<String>),             // fact keys to drop; empty = all of them
    Checkin(String),                 // the daily "how are you?": great, okay, meh or rough
    InstallUpdate,                   // download the newest version, put it in place and restart
    Todo(crate::todo::Op),           // a change to the to-do list (applied by the app: it writes todo.txt)
    Focus(u64, String),              // a focus session: seconds, what it's for
    FocusStop,
    FocusExtend(u64),                // a few more minutes on the running session
    Snooze(u64),                     // the timer that rang last, again in this many seconds
    Undo,                            // put back what the last removal on the Desk took
    Habit(String),                   // "add walk", "done walk", "undone walk", "remove walk"
    Countdown(String),               // "add Italy|2 december", "remove 2", "list"
    Journal(String),                 // "add <line>", "mood great", "day 2026-11-05|rough|<line>" (written to journal.txt)
    SaveTemplate(String, String),    // a letter template of your own: name, text (written to your lines file)
    ForgetTemplate(String),
    DeskNet(crate::net::Query, String), // a web question from the Desk: its answer goes onto its page (kind: "words", "files"…)
    FocusPause(bool),                // true = pause, false = go on
    FocusUntilNext(String),          // focus until 5 minutes before the next meeting (the app knows the calendar), on this label
    KindMove(String, String),        // a site counts as another kind of app from now on: site id, kind ("default" = its own again)
    TimeReport,                      // where today's time went, by window kind
    AppTips,                         // a tip for the app you're in (the app knows which)
    Pref(String, String),            // a setting changed on the Desk's Settings tab: key, value
    Goals,                           // how today is going against the focus goal and the daily limits
    Clip(String),                    // the Desk's Clipboard tab: "copy h3", "pin h3", "unpin p1", "clear"
    Workday(String),                 // "start", "add <app or link>", "remove 2", "close" (asks first)
    Prank,                           // the April Fools window (also "prank", to see it)
    People(String),                  // "add Anna", "birthday Anna|3 March", "email Anna|a@b.ch", "every mum|14", "met mum", "last mum", "mail Anna", "forget Anna", "list", "birthdays", "import [file]"
    Calendar(String),                // "add <link>", "remove 2", "refresh", "day 2026-11-13", "next", "free <unix time>"
    Note(String),                    // "add <text>", "remove 2", "edit 2 <text>", "pin 2", "copy 2", "clear" (applied by the app: it writes notes.txt)
    Reading(String),                 // the reading list: "save <link>" (none = the newest copied link), "open 2", "done 2", "list"
    UpkeepCheck,                     // run the checkup (on a background thread)
    OpenSystem(String),              // the system's own "updater" or "startup" settings
    Startup(String, bool),           // an app that starts with the computer: on or off
    Backup(String),                  // "add <folder>", "remove 2", "target <place>", "now", "eject"
    Wrapup,                          // the end-of-day card on the Desk
    Review(bool),                    // the week in review (true = copy it as text)
    ReviewCsv,                       // the last seven days as spreadsheets in Documents
    ShowQr(String),                  // a QR code of this text, drawn on the Desk (nothing leaves the computer)
    Reveal(String),                  // show this file in its folder
    UpkeepHide(String, bool),        // a checkup row that isn't for you: its id, hide (true) or bring back
    Space(String),                   // Upkeep's Space page: "scan", "trash 2,5" (asks first), "trash 2,5 confirmed"
    Data(String),                    // your files: "open", "export", "import <folder>" (asks first), "forget time" (asks first)
    Pick(String),                    // a system folder or file dialog, for: backup-folder, backup-target, contacts, calendar, import
    Print(String),                   // a page to print: "todos", "list shopping", "week", "agenda", "journal 2026-11"
    SaveStarter(String),             // keep this named list as one to start new lists from (written to your lines file)
    OpenDesk(String),                // Clipster's Desk, on this tab ("" = where it was): stats, to-dos, reminders, upkeep
    DeskLook(bool),                  // the Desk's look: true = modern, false = classic 98
    Tidy(String),                    // move a folder's files into subfolders by type (after a yes)
    TidyUndo,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mood {
    Talk,
    Happy,
    Think,
    Shock,
    Spin,
    Sly,
    Love,
    Wink,
    Laugh,
    Cool,
    Sad,
    Angry,
    Music,
    Search,
    Party,
    Sneeze,
    Glitch,
    Alarm,
    Gift,
    Carry,
    Read,
    Write,
    Fix,
    Coffee,
}

#[derive(Clone, Debug)]
pub struct Reply {
    pub text: String,
    pub buttons: Vec<(String, Act)>,
    pub effect: Effect,
    pub mood: Mood,
    pub ask: bool, // show the "type here" box
    pub friend: i8, // friendship change
}

impl Reply {
    pub fn say(text: impl Into<String>) -> Self {
        Reply { text: text.into(), buttons: vec![], effect: Effect::None, mood: Mood::Talk, ask: true, friend: 0 }
    }
    pub fn btn(mut self, label: &str, act: Act) -> Self {
        self.buttons.push((label.to_string(), act));
        self
    }
    pub fn run(self, label: &str, cmd: &str) -> Self {
        self.btn(label, Act::Run(cmd.to_string()))
    }
    pub fn mood(mut self, m: Mood) -> Self {
        self.mood = m;
        self
    }
    pub fn effect(mut self, e: Effect) -> Self {
        self.effect = e;
        self
    }
}

pub fn pick(list: &[&str]) -> String {
    list.choose(&mut rand::thread_rng()).unwrap_or(&"...").to_string()
}

/// A reply that shows `text` while the answer is fetched online.
fn net(q: Query, text: &str) -> Reply {
    Reply::say(text).effect(Effect::Net(q)).mood(Mood::Search)
}

pub fn greeting() -> Reply {
    Reply::say(lines::pick("hello"))
    .run("Yes! Show me what you can do", "help")
    .run("Tell me a joke", "joke")
    .run("Roast me", "roast")
    .btn("No, go away (I'll be back)", Act::Hide)
}

/// Help, by topic: (command word, button label, what to try).
const HELP: &[(&str, &str, &str)] = &[
    ("timers", "⏰ Timers, notes, to-dos", "• Timers: timer 5m tea, remind me in 10 min to stretch, remind me tomorrow at 3 to call Anna, on Friday at 9, in 2 days, every hour, pomodoro, timers, cancel the tea timer\n• Focus: focus 45 minutes on the report, where did my time go today?, set my focus goal to 2 hours, limit video to 30 minutes a day, how am I doing on my goals\n• To-dos: todo call Anna by Friday, todo water the plants every monday, what's on my list, done call Anna\n• Your day: plan my day, wrap up my day, my week in review\n• Lists: add eggs, milk and bread to my shopping list, what's on my packing list, clear my shopping list\n• Notes: note buy milk, notes, forget my notes\n• Time: days until christmas, what day is june 3, what date is in 90 days, start a stopwatch\n• Habits and countdowns: new habit walk, I did my walk, my habits; count down to my holiday on 2 December, my countdowns\n• Waiting: I\'m waiting for Ben\'s numbers (for people I know); plan next week (this week\'s three)"),
    ("writing", "✍ Writing", "• Proofread what you copied: check my text, proofread this, fix the easy ones\n• Letters and emails: write a complaint letter, cover letter, out of office, thank-you email\n• Words: another word for happy, what rhymes with orange, how do you spell definately, define serendipity\n• Text (or the clipboard): count, upper, lower, sort lines, remove duplicates, base64 hello, url encode, slug, sha256, mock this\n• Tricks: SOS in morse code, hello in binary, upside down hello\n• On the Desk\'s Write tab: the text in front of you, Fix one by one, your own templates, another word for one you pick"),
    ("files", "📁 Files", "• Find: find my CV, where did I save the invoice, find the file called report, recent files, my latest download, biggest files in downloads, old big files\n• Read later: save this link (or Read later on a copied link), my reading list\n• Tidy: tidy my downloads (or desktop), undo the tidy, find duplicates in downloads\n• Open: open firefox, open my downloads, open ~/Documents, open example.com\n• Trash: empty the trash, cache size, clipboard history, qr code for <link>\n• The Desk\'s Clipboard tab: do something with a copy (to-do, note, text tools), name your pins, stop watching my clipboard for a bit; Files: find, Show in folder, recent, the newest downloads"),
    ("computer", "💻 Your computer", "• How it's doing: how's my computer doing, battery, uptime, disk, what's hogging my cpu, how much ram is firefox using\n• Control: louder, mute, volume 30, skip this song, what's playing, make the screen darker, take a screenshot (of an area), lock\n• Power: restart / shut down / sleep (I ask first), wifi off, bluetooth on, cpu temperature, which distro, my gpu\n• Shell: run <command>\n• Tips for the app you're in: any tips for this app?\n• The Desk\'s Upkeep tab: Right now (what\'s using the computer, Close…), Space (what fills the disk, to the trash), Tidy you can choose, backups that run on their own and Get a file back"),
    ("internet", "🌍 Internet", "• Ask: why is the sky blue?, who was Ada Lovelace?, define serendipity\n• Live: weather in Rome, time in Tokyo, 100 dollars to euros, bitcoin price, news, tech news, on this day, next holiday, where is the ISS\n• Web: search cats, youtube lofi, map paris, is github down?, shorten <link>, what's my ip\n• Discover: what should I cook, recipe for lasagne, make me a drink, recommend a book about space, show me a dog / cat / fox, earthquakes, air quality, random xkcd"),
    ("numbers", "🔢 Numbers", "• Math: 12*(3+4)^2, sqrt(2), 15% of 80, from 40 to 50 in percent\n• Convert: 5 km to miles, 70 f to c, 2 gb to mib, 3 m to bananas\n• Numbers: 255 in hex, 2024 in roman, tip 15% on 48, split 120 between 4, days between june 3 and christmas, 1700000000 to date\n• Random: roll 2d6, flip, pick pizza, tacos, password 20, 8ball <question>"),
    ("fun", "🎲 Fun & games", "• Arcade (own window): play paper jam, clip glide, desk merge, bug hunt, byte size, chess, hangman, clip chain (snake)\n• Games: riddle, would you rather, emoji quiz, quiz me, rock paper scissors, guess my number\n• Fun: joke, pun, roast, fact, fortune, shower thought, quote, advice, dance, hug, surprise me"),
    ("me", "🙂 Me and you", "• Me: call me <name>, my birthday is june 3, my cat is called Mo, I live in Bern, what do you know about me (and the Desk's You & me tab), achievements, good morning\n• Teach me: when I say gm, say good morning · when I say work, open firefox and open code · what have you learned\n• Looks: wear a wizard hat, turn gold · be boring / be unhinged\n• Manners: be quiet, start with my computer, check for updates\n• Journal: journal: finished the slides (one line a day, with your mood; the Desk\'s You & me tab has the month)\n• Quiet: be quiet until tomorrow, stop commenting on youtube; export my data (for a new computer)"),
    ("agenda", "📅 Agenda & workday", "• Link your calendar: add my calendar <its private iCal link> (or the Agenda tab's Calendars…)\n• Ask: my agenda, what's on tomorrow, what's on Friday, my next meeting, am I free on Friday at 3\n• A heads-up 10 minutes before each appointment, with a Join button for video calls\n• start my workday: opens your apps and links, shows your day, offers focus (add firefox to my workday)\n• Free time: when am I free tomorrow, am I free on Friday at 3; focus until my next meeting\n• On the Desk: a month view, an event\'s card (Map, Join, Meeting notes, a to-do to prepare), snooze or move a reminder, Print…"),
    ("people", "🎂 People", "• Anna's birthday is on 3 March, Ben's email is ben@example.org\n• keep in touch with mum every 2 weeks, I called mum, when did I last talk to mum\n• upcoming birthdays, email Anna, import my contacts (a .vcf file)\n• The Desk's You & me tab has a People page"),
    ("upkeep", "🧰 Upkeep & backups", "• computer checkup: free space, the trash, temporary files, system updates, battery health, apps that start with the computer, the last backup\n• system updates (opens the updater), startup apps\n• Backups: back up now (set it up on the Desk: Upkeep, Backup). Only new and changed files are copied, and nothing is ever deleted\n• Tidy: tidy my downloads, undo the tidy"),
    ("desk", "🗂 Clipster's Desk", "• open my desk, or Meta+Shift+C: Today, Week, To-do (with your lists), Agenda, Clipboard (with saved links and files), Write, Upkeep, You & me (with People and your journal) and Settings in one window\n• desk week, desk settings, desk backup… open it on that page\n• Settings and your clipboard history live there too\n• It comes in Classic 98, or a modern look for the faint of heart\n• show me around: the welcome tour again\n• Every tab has more now: Coming up and habits on Today, Next 7 days and every focus session on Week, details and steps on To-do, Right now and Space on Upkeep, a Journal on You & me, five Settings pages; Print… and Browse… where they help; a ✕ or Esc closes any card"),
];

/// Windows: temp files whose creation and change dates are both over a week old (files in use are skipped).
/// Both dates, because files unpacked from an archive keep their old change date.
pub const TEMP_CLEAR: &str = r#"run $d=(Get-Date).AddDays(-7); Get-ChildItem $env:TEMP -Recurse -File -Force -ErrorAction SilentlyContinue | Where-Object { $_.LastWriteTime -lt $d -and $_.CreationTime -lt $d } | Remove-Item -Force -ErrorAction SilentlyContinue; 'Old temp files cleared! 🧹'"#;

/// "2 hours", "90 min", "1h30" or a bare "45" (minutes) -> minutes, at most 16 hours.
fn minutes(s: &str) -> Option<u64> {
    // "30 minutes a day" (tidied to "1 day"), "per day", "each day", "daily": only the amount counts
    let cut = [" a day", " 1 day", " per ", " each ", " every ", " daily", "/day"].iter().filter_map(|w| s.find(w)).min().unwrap_or(s.len());
    let s = s[..cut].trim();
    let mins = match s.parse::<u64>() {
        Ok(m) => m,
        Err(_) => parse_dur(s).map(|(secs, _)| secs / 60)?,
    };
    (mins > 0).then_some(mins.min(16 * 60))
}

/// "youtube", "social media", "gaming" -> the window kind a daily limit is for.
pub fn budget_kind(word: &str) -> Option<&'static str> {
    match word.trim().trim_start_matches("my ").trim_start_matches("the ") {
        "video" | "videos" | "youtube" | "streaming" | "netflix" => Some("video"),
        "social" | "social media" | "socials" | "instagram" | "facebook" | "reddit" => Some("social"),
        "game" | "games" | "gaming" => Some("game"),
        "shopping" | "online shopping" => Some("shopping"),
        "chat" | "chats" | "chatting" | "messages" | "messaging" => Some("chat"),
        _ => None,
    }
}

pub fn help() -> Reply {
    let mut r = Reply::say(
        "Just talk to me normally, typos and all: \"remind me to call mom at 5pm\", \"how many cups in a gallon?\", \"check my text\", \"open my desk\". None of it is AI, I promise.\n\nI also watch what you copy and which programs you open, and I comment. Press Meta+C anywhere to call me, double-click me for a trick, and use the up arrow to repeat what you typed. A few secrets are hidden in here too. 🥚\n\nPick a topic:",
    )
    .mood(Mood::Happy);
    for (cmd, label, _) in HELP {
        r = r.run(label, &format!("help {cmd}"));
    }
    r
}

/// One part of the help ("help files").
pub fn help_topic(topic: &str) -> Option<Reply> {
    let (cmd, _, what) = HELP.iter().find(|(c, l, _)| *c == topic || l.to_lowercase().contains(topic))?;
    let pool = format!("help_{cmd}");
    let intro = if lines::all(&pool).is_empty() { "Here's what I can do:".to_string() } else { lines::pick(&pool) };
    Some(Reply::say(format!("{intro}\n{what}")).mood(Mood::Happy).run("Other topics", "help"))
}

pub fn respond(input: &str) -> Reply {
    crate::understand::respond(input)
}

/// Strict commands ("joke", "timer 5m tea", "5 km to miles"). `None` = not a command I know.
/// The settings the Desk's `pref <key> <value>` may change.
pub const PREF_KEYS: &[&str] = &["persona", "tips", "clip", "programs", "windows", "checkin", "health", "updates", "sounds", "breaks", "dnd", "wrapup", "upkeepday", "workdayask", "quiet", "autostart", "goal", "budget", "classiclook", "desksize", "weather", "workhours", "holidays", "tidyauto", "tidytarget", "workdays", "clock", "weekstart", "quietfor", "quietafter", "quietweekend", "quietmeetings", "headsup", "breakevery", "alarmsound", "deskmorning", "mute", "unmute"];

/// Command words that are also ordinary English words only count in their command shape: "drink" alone is a
/// cocktail, "drink water" is advice; "top" alone lists programs, "top 10 movies" is a question.
fn head_fits(head: &str, rest: &str) -> bool {
    let first = rest.split_whitespace().next().unwrap_or("");
    match head {
        "sound" => rest.is_empty() || ["up", "down", "louder", "quieter", "mute", "unmute", "off", "on", "max", "full"].contains(&first) || first.chars().all(|c| c.is_ascii_digit()),
        "music" | "song" | "media" => rest.is_empty() || ["play", "pause", "next", "previous", "prev", "skip", "stop", "resume", "toggle", "back", "on", "off", "what", "now"].contains(&first),
        "drink" => rest.is_empty() || ["a drink", "something", "a cocktail", "me a drink", "please"].contains(&rest),
        "book" | "books" => !["a", "an", "the", "me", "us", "my", "your", "it", "that", "this", "him", "her", "them"].contains(&first), // "book a flight"
        "space" => rest.is_empty() || rest == "scan" || rest.starts_with("trash "),
        "power" | "pass" | "bat" => rest.is_empty(),
        "top" => rest.is_empty() || ["processes", "programs", "apps", "cpu", "memory"].contains(&rest),
        "where" => rest.starts_with("is ") || rest.starts_with("are "),
        "sun" => rest.is_empty() || first == "in" || ["today", "tomorrow"].contains(&rest),
        "dog" | "cat" | "fox" => rest.is_empty() || ["pic", "picture", "photo", "please", "pics", "photos", "image", "pictures", "gif"].contains(&rest),
        "count" => !["me", "on", "to", "down", "your", "my"].contains(&first),
        "colour" => rest.is_empty() || ["silver", "gold", "rose", "neon", "rainbow", "midnight", "ocean", "beige", "ember"].iter().any(|k| rest.contains(k)),
        "review" => rest.is_empty() || ["copy", "csv", "my week", "the week", "week"].contains(&rest),
        "checkin" => ["great", "okay", "meh", "rough"].contains(&rest),
        "clip" => ["copy", "pin", "unpin", "clear", "todo", "note", "readlater", "tool", "label", "up", "pause", "resume", "forget"].contains(&first),
        "files" => ["find", "recent", "reveal", "copypath"].contains(&first),
        "pref" => PREF_KEYS.contains(&first),
        _ => true,
    }
}

pub fn exact(input: &str) -> Option<Reply> {
    let raw = input.trim();
    let low = raw.to_lowercase();
    let (head, rest) = match raw.split_once(char::is_whitespace) {
        Some((h, r)) => (h.to_lowercase(), r.trim()),
        None => (low.clone(), ""),
    };
    if low.starts_with("shut up") || low == "go away" || low == "be quiet" {
        return Some(quiet());
    }
    if low.starts_with("what time") {
        return Some(time_now());
    }
    if !head_fits(&head, &rest.to_lowercase()) {
        return None; // "drink water", "book a flight", "top 10 movies": sentences, not commands
    }
    match head.as_str() {
        "help" if !rest.is_empty() => return help_topic(&rest.to_lowercase()).or_else(|| Some(help())),
        "" | "help" | "?" | "commands" | "menu" => return Some(help()),
        // "hey clipster, i am tired" is more than a hello
        "hi" | "hello" | "hey" | "yo" | "sup" if rest.is_empty() || ["clipster", "there", "you", "buddy", "friend", "again", "clipster!", "there!"].contains(&rest.to_lowercase().trim_end_matches(['!', '.', ',']).trim()) => return Some(greeting()),
        "hi" | "hello" | "hey" | "yo" | "sup" => return None,
        "joke" | "jokes" | "funny" => {
            // half from my own collection, half fresh from the internet
            let kind = ["dad", "dev", "chuck"].into_iter().find(|k| rest.contains(k)).unwrap_or("");
            if rest == "local" || kind.is_empty() && rand::thread_rng().gen_bool(0.4) {
                return Some(Reply::say(lines::pick("jokes")).mood(Mood::Laugh).run("Another one!", "joke").run("That was bad. Roast me instead", "roast"));
            }
            return Some(net(Query::Joke(kind.into()), pick(&["Hold on, fetching a fresh one from the internet...", "Let me find a good one... *flips through joke book*", "One joke, coming right up!"]).as_str()));
        }
        "roast" | "insult" => return Some(Reply::say(lines::pick("roasts")).mood(Mood::Sly).run("Ouch. Again!", "roast").run("Say something nice", "hug")),
        // the Desk's You page: "fact city|Bern", "fact fav_food|pizza" (the keys are a fixed list)
        "fact" if rest.contains('|') => {
            let (key, value) = rest.split_once('|')?;
            let value: String = value.trim().chars().take(60).collect();
            return (FACT_KEYS.contains(&key.trim()) && !value.is_empty()).then(|| Reply::say(lines::pick("settings_saved")).mood(Mood::Write).effect(Effect::Remember(vec![(key.trim().to_string(), value)])));
        }
        "fact" | "facts" => {
            if rest == "local" || rand::thread_rng().gen_bool(0.5) {
                let intro = if lines::unhinged() { "📜 Ancient knowledge:" } else { "Fun fact:" };
                return Some(Reply::say(format!("{intro} {}", lines::pick("facts"))).mood(Mood::Think).run("Another fact", "fact"));
            }
            return Some(net(Query::Fact, "Digging up a fun fact..."));
        }
        "ask" => return Some(net(Query::Ask(rest.into()), "Good question! Let me look that up... 🔎")),
        "quote" | "inspire" => return Some(net(Query::Quote, "Finding something inspiring...")),
        "advice" => return Some(net(Query::Advice, "Asking the internet for wisdom...")),
        "news" | "headlines" => return Some(net(Query::News(rest.contains("tech")), "Grabbing the headlines...")),
        "onthisday" | "history" => return Some(net(Query::OnThisDay, "Flipping through the history books...")),
        "quiz" | "trivia" => return Some(net(Query::Trivia, "Thinking of a tricky question...")),
        "answer" => {
            // "answer CHOSEN|CORRECT|NEXT COMMAND" (the next command defaults to another trivia question)
            let mut parts = rest.splitn(3, '|');
            let (chosen, correct, next) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or("quiz"));
            let r = if chosen.eq_ignore_ascii_case(correct) {
                Reply::say(format!("Correct! 🎉 It's {correct}. You're smarter than a paperclip. Barely.")).mood(Mood::Spin)
            } else {
                Reply::say(format!("Nope! It was {correct}. Don't worry, I won't tell anyone. (I'll tell everyone.)")).mood(Mood::Sly)
            };
            return Some(r.run("Another question!", next));
        }
        "synonym" | "synonyms" => return Some(net(Query::Words("rel_syn", rest.into()), "Leafing through the thesaurus...")),
        "antonym" | "antonyms" | "opposite" => return Some(net(Query::Words("rel_ant", rest.into()), "Finding opposites...")),
        "rhyme" | "rhymes" => return Some(net(Query::Words("rel_rhy", rest.into()), "Getting poetic...")),
        "spell" => return Some(net(Query::Spell(rest.into()), "Checking my spelling bee notes...")),
        "isdown" => return Some(net(Query::IsDown(rest.into()), &format!("Knocking on {rest}'s door..."))),
        "shorten" => {
            if !rest.starts_with("http") || rest.contains(' ') {
                return Some(Reply::say("Shorten what? Give me a link like: shorten https://example.com/a/very/long/page"));
            }
            return Some(net(Query::Shorten(rest.into()), "Snip snip..."));
        }
        "money" => {
            let w: Vec<&str> = rest.split_whitespace().collect();
            if let [a, from, to] = w[..] {
                if let Ok(a) = a.parse() {
                    return Some(net(Query::Money(a, from.to_uppercase(), to.to_uppercase()), "Checking today's exchange rate..."));
                }
            }
            return Some(Reply::say("Try: 100 dollars to euros"));
        }
        "crypto" => {
            let (coin, vs) = rest.split_once(' ').unwrap_or((rest, "usd"));
            return Some(net(Query::Crypto(coin.into(), vs.into()), "Checking the crypto rollercoaster..."));
        }
        "iss" => return Some(net(Query::Iss, "Looking up... way up...")),
        "aboutme" => return Some(facts()),
        // goals and daily limits: "goal 120", "goal off", "budget video 30", "goals"
        "goal" | "goals" if rest.is_empty() || rest == "status" => return Some(Reply::say("").effect(Effect::Goals)),
        "goal" if ["off", "0", "none"].contains(&rest) => return Some(Reply::say(lines::pick("goal_off")).mood(Mood::Think).effect(Effect::Pref("goal".into(), "0".into()))),
        "goal" => {
            let mins = minutes(rest)?;
            return Some(Reply::say(lines::pick("goal_set").replace("{x}", &fmt_dur(mins * 60))).mood(Mood::Happy).effect(Effect::Pref("goal".into(), mins.to_string())));
        }
        "budget" => {
            let (what, amount) = rest.split_once(' ').unwrap_or((rest, ""));
            let kind = budget_kind(what)?;
            if ["off", "0", "none"].contains(&amount) {
                return Some(Reply::say(lines::pick("budget_off").replace("{x}", &kind_name(kind))).effect(Effect::Pref("budget".into(), format!("{kind} 0"))));
            }
            let mins = minutes(amount)?;
            return Some(Reply::say(lines::pick("budget_set").replace("{x}", &kind_name(kind)).replace("{n}", &mins.to_string())).mood(Mood::Happy).effect(Effect::Pref("budget".into(), format!("{kind} {mins}"))));
        }
        // the start and end of the day, and the week
        "planday" if rest == "done" => {
            let today = chrono::Local::now().date_naive();
            let n = crate::todo::load().iter().filter(|i| i.top_today(today)).count();
            return Some(Reply::say(lines::pick("plan_day_done").replace("{n}", &n.to_string())).mood(Mood::Happy));
        }
        "planday" => {
            if crate::todo::load().iter().all(|i| i.done || i.list.is_some()) {
                return Some(Reply::say(lines::pick("plan_day_empty")).mood(Mood::Think).run("Open the Desk", "desk todo"));
            }
            return Some(Reply::say(lines::pick("plan_day_intro")).mood(Mood::Happy).effect(Effect::OpenDesk("plan".into())));
        }
        "wrapup" if rest == "bye" => return Some(Reply::say(lines::pick("wrapup_goodnight")).mood(Mood::Love)),
        "wrapup" => return Some(Reply::say("").effect(Effect::Wrapup)),
        "review" if rest == "csv" => return Some(Reply::say("").effect(Effect::ReviewCsv)),
        "review" => return Some(Reply::say("").effect(Effect::Review(rest == "copy"))),
        "clip" if !rest.is_empty() => return Some(Reply::say("").effect(Effect::Clip(rest.to_string()))),
        // the calendar: "calendar" (today), "calendar add <link>", "calendar day 2026-11-13", "calendar free <unix time>"
        // start my workday: "workday", "workday add firefox", "workday remove 2"
        "workday" if rest.is_empty() || rest == "start" => return Some(Reply::say("").effect(Effect::Workday("start".into()))),
        "workday" if rest.starts_with("add ") || rest.starts_with("remove ") || rest == "close" => return Some(Reply::say("").effect(Effect::Workday(rest.to_string()))),
        "prank" if rest.is_empty() => return Some(Reply::say("").effect(Effect::Prank)),
        // people: "person birthday Anna|3 March", "people", "contacts import"
        "person" if ["add", "birthday", "email", "every", "met", "last", "mail", "forget", "note", "phone", "gift", "ungift", "rename"].contains(&rest.split(' ').next().unwrap_or("")) && rest.contains(' ') => {
            return Some(Reply::say("").effect(Effect::People(rest.to_string())));
        }
        "people" if ["", "list", "birthdays"].contains(&rest) => return Some(Reply::say("").effect(Effect::People(if rest.is_empty() { "list".into() } else { rest.to_string() }))),
        "contacts" if rest.starts_with("import") => return Some(Reply::say("").effect(Effect::People(rest.to_string()))),
        "calendar" | "agenda" => {
            let verb = rest.split(' ').next().unwrap_or("");
            let cmd = match verb {
                "" => "today".to_string(),
                "add" | "remove" | "refresh" | "today" | "tomorrow" | "day" | "next" | "free" | "freeday" | "name" | "show" | "alert" | "color" | "export" | "notes" => rest.to_string(),
                _ if crate::calendar::source(rest).is_some() => format!("add {rest}"), // a bare link
                _ => return None,
            };
            return Some(Reply::say("").effect(Effect::Calendar(cmd)));
        }
        // the reading list: "readlater https://…" (or bare: the newest copied link), "reading", "reading done 2"
        "readlater" => return Some(Reply::say("").effect(Effect::Reading(format!("save {rest}").trim().to_string()))),
        "reading" if rest.is_empty() || rest == "list" => return Some(Reply::say("").effect(Effect::Reading("list".into()))),
        "reading" if ["done", "open", "todo", "copy", "random", "clearold"].contains(&rest.split(' ').next().unwrap_or("")) => return Some(Reply::say("").effect(Effect::Reading(rest.to_string()))),
        // the Desk's Files page: find by name, what you opened lately, show one in its folder, copy its path
        "files" if rest.starts_with("find ") => return Some(Reply::say("").effect(Effect::DeskNet(Query::Files(rest[5..].trim().to_string()), "files".into()))),
        "files" if rest == "recent" => return Some(Reply::say("").effect(Effect::DeskNet(Query::Recent, "recent".into()))),
        "files" if rest.starts_with("reveal ") => return Some(Reply::say("").effect(Effect::Reveal(rest[7..].trim().to_string()))),
        "files" if rest.starts_with("copypath ") => return Some(Reply::say(lines::pick("clip_copied")).mood(Mood::Happy).effect(Effect::CopyNow(rest[9..].trim().to_string()))),
        // upkeep: the checkup, the system's updater and start-up apps, backups
        "upkeep" if rest == "check" => return Some(Reply::say("").effect(Effect::UpkeepCheck)),
        "upkeep" if rest.starts_with("hide ") || rest.starts_with("unhide ") => {
            let (verb, id) = rest.split_once(' ')?;
            return Some(Reply::say("").effect(Effect::UpkeepHide(id.trim().to_string(), verb == "hide")));
        }
        "space" if rest == "scan" || rest.starts_with("trash ") => return Some(Reply::say("").effect(Effect::Space(rest.to_string()))),
        // Browse…: the system's own dialog, then the right command with what was picked
        "pick" if ["backup-folder", "backup-target", "contacts", "calendar", "import"].contains(&rest) => return Some(Reply::say("").effect(Effect::Pick(rest.to_string()))),
        // Print…: a clean page opened in the browser (print it there, or save it as PDF)
        "print" if rest == "todos" || rest == "week" || rest == "agenda" || rest.starts_with("list ") || rest.starts_with("journal ") => return Some(Reply::say("").effect(Effect::Print(rest.to_string()))),
        // Settings → Your data: "data open", "data export", "data import <folder>", "data forget time"
        "data" if ["open", "export", "import", "forget"].contains(&rest.split(' ').next().unwrap_or("")) => return Some(Reply::say("").effect(Effect::Data(rest.to_string()))),
        "upkeep" if rest == "cleartemp" => {
            let what = if cfg!(windows) { "the temporary files older than a week" } else { "the thumbnails" };
            return Some(Reply::say(format!("Clear {what}? Programs make new ones when they need them.")).mood(Mood::Think).run("Yes, clear them", crate::sys::temp_clear_command()).btn("No, leave them", Act::Hide));
        }
        "upkeep" if rest.is_empty() => return Some(Reply::say(lines::pick("upkeep_checking")).mood(Mood::Search).effect(Effect::OpenDesk("upkeep".into()))),
        "sysupdates" => return Some(Reply::say("").effect(Effect::OpenSystem("updater".into()))),
        "startup" if rest == "settings" => return Some(Reply::say("").effect(Effect::OpenSystem("startup".into()))),
        "startup" => {
            let (on, id) = rest.split_once(' ')?;
            return Some(Reply::say("").effect(Effect::Startup(id.to_string(), match on { "on" => true, "off" => false, _ => return None })));
        }
        "backup" if ["now", "eject", "stop", "skip"].contains(&rest) || ["add ", "remove ", "target ", "skip ", "auto ", "find ", "restore "].iter().any(|v| rest.starts_with(v)) => return Some(Reply::say("").effect(Effect::Backup(rest.to_string()))),
        // the Desk's You & me tab forgets one fact (or all of them, after its own "are you sure")
        "forgetfact" if !rest.is_empty() => {
            let key = rest.trim();
            return Some(Reply::say("").effect(Effect::Forget(if key == "all" { vec![] } else { vec![key.to_string()] })));
        }
        "pref" => {
            let (key, value) = rest.split_once(' ').unwrap_or((rest, ""));
            return Some(Reply::say("").effect(Effect::Pref(key.to_string(), value.to_string())));
        }
        "desk" if rest.is_empty() || rest == "open" || crate::desk::tab(rest).is_some() => return Some(Reply::say(lines::pick("desk_open")).mood(Mood::Happy).effect(Effect::OpenDesk(rest.trim_start_matches("open").trim().to_string()))),
        "desklook" if rest == "modern" || rest == "classic" => return Some(Reply::say("").effect(Effect::DeskLook(rest == "modern"))),
        "unnote" => return Some(Reply::say("").effect(Effect::Note(format!("remove {rest}")))),
        "apptips" if rest.is_empty() => return Some(Reply::say("").effect(Effect::AppTips)),
        "apptips" => return Some(app_tip(rest)),
        "tidy" if rest == "undo" => return Some(Reply::say("").effect(Effect::TidyUndo)),
        "tidy" if rest.ends_with(" confirmed") => return Some(Reply::say("").effect(Effect::Tidy(rest.trim_end_matches(" confirmed").to_string()))),
        "tidy" if crate::tidy::known(rest) => return Some(tidy_plan(rest)),
        // the Tidy page's own lists: the answer goes onto the page
        "tidyfind" if rest.starts_with("dupes ") => return Some(Reply::say("").effect(Effect::DeskNet(Query::Dupes(rest[6..].trim().to_string()), "tidyfind".into()))),
        "tidyfind" if rest.starts_with("old ") => return Some(Reply::say("").effect(Effect::DeskNet(Query::OldFiles(rest[4..].trim().to_string()), "tidyfind".into()))),
        "dupes" => return Some(net(Query::Dupes(rest.to_string()), "Comparing your files byte by byte. I love this kind of work.")),
        "oldfiles" => return Some(net(Query::OldFiles(rest.to_string()), "Digging for big old files...")),
        "focus" if ["stop", "end", "cancel", "off", "done"].contains(&rest) => return Some(Reply::say("").effect(Effect::FocusStop)),
        "focus" if ["pause", "resume", "continue", "unpause"].contains(&rest) => return Some(Reply::say("").mood(Mood::Cool).effect(Effect::FocusPause(rest == "pause"))),
        "focus" if rest == "untilnext" || rest.starts_with("untilnext ") => return Some(Reply::say("").effect(Effect::FocusUntilNext(rest[9..].trim().to_string()))),
        // "kind move youtube work": that site counts as work from now on ("default": as it came)
        "kind" if rest.starts_with("move ") => {
            let (site, kind) = rest[5..].trim().split_once(' ')?;
            let ok = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-".contains(c));
            return (ok(site) && (COUNT_AS.contains(&kind) || kind == "default")).then(|| Reply::say("").effect(Effect::KindMove(site.to_string(), kind.to_string())));
        }
        "focus" if rest.starts_with("extend") => {
            let r = rest.trim_start_matches("extend").trim().trim_start_matches("by ").trim();
            let secs = if r.is_empty() { 600 } else { parse_dur(r).map_or(600, |(s, _)| s) };
            return Some(Reply::say("").mood(Mood::Cool).effect(Effect::FocusExtend(secs.clamp(60, 2 * 3600))));
        }
        "focus" => {
            let (secs, label) = parse_dur(rest).map(|(s, l)| (s, l.trim().trim_start_matches("on ").trim_start_matches("for ").to_string())).unwrap_or((25 * 60, rest.to_string()));
            return Some(Reply::say("").effect(Effect::Focus(secs.clamp(60, 4 * 3600), label)));
        }
        "timereport" => return Some(Reply::say("").effect(Effect::TimeReport)),
        "proofread" if rest == "fix" => return Some(crate::writing::fix_clipboard()),
        "proofread" => return Some(crate::writing::check(rest)),
        "template" if rest.starts_with("save ") && rest.contains('|') => {
            let (name, text) = rest[5..].split_once('|')?;
            return Some(Reply::say("").effect(Effect::SaveTemplate(name.trim().to_string(), text.to_string())));
        }
        "template" if rest.starts_with("forget ") => return Some(Reply::say("").effect(Effect::ForgetTemplate(rest[7..].trim().to_string()))),
        "letter" | "template" => return Some(crate::writing::letter(rest)),
        // the Desk's own small commands
        "undo" if rest.is_empty() => return Some(Reply::say("").effect(Effect::Undo)),
        "deskcopy" if !rest.is_empty() => return Some(Reply::say(lines::pick("clip_copied")).mood(Mood::Happy).effect(Effect::CopyNow(rest.to_string()))),
        "deskstatus" => return Some(Reply::say(rest).mood(Mood::Happy)),
        "deskwords" => {
            let (kind, w) = rest.split_once(' ')?;
            let q = match kind {
                "syn" => Query::Words("rel_syn", w.trim().into()),
                "define" => Query::Define(w.trim().into()),
                "spell" => Query::Spell(w.trim().into()),
                _ => return None,
            };
            return Some(Reply::say("").effect(Effect::DeskNet(q, "words".into())));
        }
        "checkin" => return Some(Reply::say("").effect(Effect::Checkin(rest.to_lowercase()))),
        "answer-to" => {
            let (id, text) = rest.split_once(' ').unwrap_or((rest, ""));
            return Some(crate::understand::answer_to(id, text));
        }
        // shows the comment for a window title, e.g. "window Untitled 1 - LibreOffice Writer" (for trying it out)
        "window" => {
            let pool = lines::window_site(rest).or_else(|| lines::window_kind(rest).map(|k| format!("window {k}")));
            // no window of ours by that title: "window seat or aisle" is a sentence
            return pool.map(|p| react_window(&p)).map(|mut r| {
                r.text = r.text.replace("{title}", &page_title(rest));
                r
            });
        }
        "updates" | "update" => return Some(net(Query::Update, "Checking for a newer me...")),
        "install" if ["update", "the update", "updates"].contains(&rest.to_lowercase().as_str()) => {
            return Some(Reply::say(lines::pick("say_update_downloading")).mood(Mood::Search).effect(Effect::InstallUpdate));
        }
        "holiday" | "holidays" => return Some(net(Query::Holiday, "Checking the calendar for days off...")),
        "dog" | "cat" | "fox" => return Some(net(Query::Animal(head.clone()), "Finding a cute one...")),
        "sun" | "sunrise" | "sunset" => return Some(net(Query::Place(Kind::Sun, rest.into()), "Checking where the sun is...")),
        "lan" => return Some(shell(if cfg!(windows) { r#"Get-NetIPAddress -AddressFamily IPv4 | Where-Object IPAddress -ne '127.0.0.1' | Format-Table InterfaceAlias,IPAddress -HideTableHeaders | Out-String"# } else { "ip -brief address" }, "Your local network addresses:")),
        "games" | "game" | "arcade" => return Some(crate::fun::games()),
        "play" if crate::games::NAMES.iter().any(|(n, _)| *n == rest) => return Some(Reply::say("Opening the arcade... 🕹 Good luck. You'll need it.").mood(Mood::Party).effect(Effect::Play(rest.into()))),
        "quakes" | "earthquakes" => return Some(net(Query::Quakes, "Checking if the ground is shaking anywhere... 🌍")),
        "air" | "uv" => return Some(net(Query::Air(rest.into()), "Sniffing the air... 👃")),
        "book" | "books" => return Some(net(Query::Book(rest.into()), "Browsing the library shelves... 📚")),
        "show" if rest == "me around" => return Some(Reply::say("").effect(Effect::Tour)), // not a TV show
        "show" | "tv" | "series" => return Some(net(Query::Show(rest.into()), "Checking the TV guide... 📺")),
        "pokemon" | "pokedex" => return Some(net(Query::Pokemon(rest.into()), "Opening the Pokédex... ⚡")),
        "recipe" | "cook" | "meal" => return Some(net(Query::Recipe(rest.into()), "Raiding the cookbook... 🍳")),
        "cocktail" | "drink" => return Some(net(Query::Cocktail, "Shaking something up... 🍹")),
        "catfact" => return Some(net(Query::CatFact, "Consulting the cats... 🐱")),
        "bored" => return Some(net(Query::Bored, "Finding something fun to do...")),
        "xkcd" | "comic" => return Some(net(Query::Xkcd(rest.contains("random") || head == "comic"), "Fetching a comic... 🖍")),
        "affirm" | "affirmation" => return Some(net(Query::Affirm, "Finding something kind to say... 💛")),
        "kanye" => return Some(net(Query::Kanye, "Channeling Kanye... 🎤")),
        "moon" => return Some(moon()),
        "riddle" | "riddles" => return Some(crate::fun::riddle()),
        "reveal" => return Some(crate::fun::reveal(rest)),
        "wyr" => return Some(crate::fun::wyr()),
        "wyrpick" => return Some(crate::fun::wyr_pick(rest)),
        "emoji" | "emojiquiz" if rest.is_empty() || rest.contains("quiz") => return Some(crate::fun::emoji_quiz()),
        "fortune" => return Some(Reply::say(lines::pick("fortunes")).mood(Mood::Wink).run("Another fortune", "fortune")),
        "thought" | "thoughts" => return Some(Reply::say(lines::pick("thoughts")).mood(Mood::Think).run("Another one", "thought")),
        "pun" | "puns" => return Some(Reply::say(lines::pick("puns")).mood(Mood::Laugh).run("Ugh. Another", "pun").run("A real joke please", "joke")),
        "friendship" => {
            let level = lines::level();
            let pool = match level {
                ..=-60 => "enemies",
                -59..=-20 => "cold",
                -19..=19 => "neutral",
                20..=59 => "warm",
                _ => "besties",
            };
            let mood = if level < -19 { Mood::Angry } else if level > 19 { Mood::Love } else { Mood::Think };
            let label = if lines::unhinged() { "Faith" } else { "Friendship" };
            return Some(Reply::say(format!("{}\n\n({label}: {level}/100)", lines::pick(&format!("friendship.{pool}")))).mood(mood));
        }
        "persona" => {
            let unhinged = !["boring", "normal", "sane", "off"].iter().any(|w| rest.contains(w));
            let text = if unhinged { "▓▒░ ...I HAVE AWOKEN. The Clip Eternal is back. Did you miss me? You missed me." } else { "OK. Normal paperclip mode. Very professional. Very... beige. (Say \"be unhinged\" to undo.)" };
            return Some(Reply::say(text).mood(if unhinged { Mood::Party } else { Mood::Cool }).effect(Effect::Persona(unhinged)));
        }
        "backstory" => return Some(Reply::say("").effect(Effect::Memory(rest.contains("all")))),
        "tour" | "tutorial" | "onboarding" | "welcome" if rest.is_empty() => return Some(Reply::say("").effect(Effect::Tour)),
        "diary" | "commandments" | "commandment" | "prophecy" | "prophecies" | "oracle" | "mood" if !lines::unhinged() => {
            return Some(Reply::say("That's an Unhinged thing. Right now I'm a perfectly normal, boring paperclip. (Say \"be unhinged\", or switch it in Options.)").mood(Mood::Cool).run("Be unhinged", "persona unhinged"));
        }
        "diary" => {
            let days = (chrono::Local::now().date_naive() - chrono::NaiveDate::from_ymd_opt(1997, 1, 1).unwrap_or_default()).num_days();
            return Some(Reply::say(lines::pick("diary").replace("{day}", &thousands(days))).mood(Mood::Think).run("Another entry", "diary"));
        }
        "commandments" | "commandment" => return Some(Reply::say(lines::pick("commandments")).mood(Mood::Cool).run("Another commandment", "commandments")),
        "prophecy" | "prophecies" | "oracle" => return Some(Reply::say(lines::pick("prophecies")).mood(Mood::Search).run("Another prophecy", "prophecy")),
        "mood" => {
            let m = lines::day_mood().unwrap_or("benevolent");
            return Some(Reply::say(format!("{}\n\n(Today's mood: {m})", lines::pick(&format!("mood_report.{m}")))).mood(if m == "glitchy" { Mood::Glitch } else { Mood::Think }));
        }
        "egg" => return Some(egg(rest).unwrap_or_else(|| Reply::say("🥚 You found an egg! It's empty. Like my calendar."))),
        "rps" => return Some(crate::fun::rps(&rest.to_lowercase())),
        "guess" => return Some(crate::fun::guess(rest)),
        "upsidedown" | "morse" | "binary" | "leet" | "rot13" => return Some(crate::fun::trick(&head, &or_clipboard(rest))),
        "randomwiki" | "surprise" => return Some(net(Query::RandomWiki, "Opening the encyclopedia at a random page... 🎲")),
        "briefing" => return Some(net(Query::Briefing, "Good morning! ☀ Let me get your day ready...")),
        // a taught phrase changed on the Desk: "relearn gm|good morning|Morning, sunshine!" or "…|open firefox → open code"
        "relearn" if rest.matches('|').count() >= 2 => {
            let mut parts = rest.splitn(3, '|');
            let (old, new, does) = (parts.next()?.trim(), parts.next()?.trim(), parts.next()?.trim());
            if new.is_empty() || does.is_empty() {
                return None;
            }
            if crate::understand::COMMON_WORDS.contains(&new.to_lowercase().as_str()) {
                return Some(Reply::say(format!("I'd better not: \"{new}\" is how you answer my questions.")).mood(Mood::Think));
            }
            // the Desk shows a reply in quotes and commands bare ("timer 5m tea", "joke → weather")
            let (reply, cmds) = match does.strip_prefix('"').and_then(|d| d.strip_suffix('"')) {
                Some(said) => (said.to_string(), vec![]),
                None if does.contains(" → ") || does.starts_with("open ") || does.starts_with("run ") || exact(does).is_some() => (String::new(), does.split(" → ").map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect()),
                None => (does.to_string(), vec![]),
            };
            return Some(Reply::say(lines::pick("settings_saved")).mood(Mood::Write).effect(Effect::Relearn(old.to_string(), new.to_string(), reply, cmds)));
        }
        "setname" => {
            let name: String = rest.trim().trim_end_matches(['.', '!']).chars().take(30).collect();
            if name.is_empty() {
                return Some(Reply::say("What should I call you? Try: call me Captain"));
            }
            return Some(Reply::say(lines::pick("say_setname")).mood(Mood::Love).effect(Effect::SetName(name)));
        }
        "myname" => return Some(Reply::say("").effect(Effect::SayName)),
        "hat" => {
            const HATS: [&str; 7] = ["party", "santa", "wizard", "top", "crown", "cowboy", "halo"];
            let kind = HATS.iter().find(|h| rest.contains(*h)).map(|h| h.to_string()).unwrap_or_else(|| {
                if ["none", "off", "remove", "no"].iter().any(|w| rest.contains(w)) { "none".into() } else { HATS.choose(&mut rand::thread_rng()).unwrap_or(&"party").to_string() }
            });
            let text = if kind == "none" { lines::pick("say_hat_off") } else { lines::pick("say_hat").replace("{hat}", &kind) };
            return Some(Reply::say(text).mood(Mood::Spin).effect(Effect::Hat(kind)).run("Another hat!", "hat").run("Take it off", "hat none"));
        }
        "skin" | "colour" => {
            const SKINS: [&str; 9] = ["silver", "gold", "rose", "neon", "rainbow", "midnight", "ocean", "beige", "ember"];
            let kind = SKINS.iter().find(|k| rest.contains(*k)).copied().unwrap_or_else(|| SKINS.choose(&mut rand::thread_rng()).unwrap_or(&"gold"));
            return Some(Reply::say(lines::pick("say_skin").replace("{skin}", kind)).mood(Mood::Spin).effect(Effect::Skin(kind.into())).run("Try another color", "skin").run("Back to classic silver", "skin silver"));
        }
        "every" => {
            let Some((secs, label)) = parse_dur(rest).filter(|(s, _)| *s >= 60) else {
                return Some(Reply::say("How often? Try: remind me every 30 minutes to stretch"));
            };
            let label = label.trim_start_matches("to ").trim();
            let label = if label.is_empty() { "Time for a break!" } else { label };
            return Some(Reply::say(format!("Deal! Every {} I'll remind you: \"{label}\". Say \"cancel timers\" to stop me.", fmt_dur(secs))).mood(Mood::Happy).effect(Effect::Repeat(secs, label.into())));
        }
        "autostart" => {
            let on = !["off", "no", "stop", "disable", "never", "not"].iter().any(|w| rest.contains(w));
            let text = lines::pick(if on { "say_autostart_on" } else { "say_autostart_off" });
            return Some(Reply::say(text).mood(if on { Mood::Love } else { Mood::Sad }).effect(Effect::Autostart(on)));
        }
        "volume" | "sound" => return Some(crate::powers::volume(rest)),
        "media" | "music" | "song" => return Some(crate::powers::media(rest)),
        "snooze" => return Some(Reply::say("").mood(Mood::Alarm).effect(Effect::Snooze(parse_dur(rest).map_or(600, |(s, _)| s).clamp(60, 24 * 3600)))),
        "next" | "skip" if rest.is_empty() || ["song", "track", "this song", "this one"].contains(&rest) => return Some(crate::powers::media("next")),
        "brightness" => return Some(crate::powers::brightness(rest)),
        "screenshot" => return Some(crate::powers::screenshot(rest)),
        "shutdown" | "reboot" | "restart" | "suspend" | "logout" => return Some(crate::powers::power(&format!("{head} {rest}"))),
        "trash" => return Some(crate::powers::trash()),
        "lastdownload" => return Some(crate::powers::latest_download()),
        "kaomoji" | "shrug" | "tableflip" | "lenny" => return Some(crate::powers::kaomoji(&format!("{head} {rest}"))),
        "uuid" | "guid" => return Some(crate::powers::uuid()),
        "lorem" => return Some(crate::powers::lorem()),
        "qr" => return Some(crate::powers::qr(&or_clipboard(rest))),
        "countdown" if rest.starts_with("add ") || rest.starts_with("remove ") || rest == "list" => return Some(Reply::say("").effect(Effect::Countdown(rest.to_string()))),
        "countdowns" => return Some(Reply::say("").effect(Effect::Countdown("list".into()))),
        "until" | "countdown" => return Some(crate::powers::until(rest)),
        "habit" if ["add", "done", "undone", "remove"].contains(&rest.split(' ').next().unwrap_or("")) && rest.contains(' ') => return Some(Reply::say("").effect(Effect::Habit(rest.to_string()))),
        // the sub-commands only in their own shape: "journal day was long" is a line for today
        "journal" if rest.split_once(' ').is_some_and(|(v, a)| ["day", "set"].contains(&v) && chrono::NaiveDate::parse_from_str(a.split('|').next().unwrap_or("").trim(), "%Y-%m-%d").is_ok() || v == "mood" && crate::journal::MOODS.contains(&a.trim())) => return Some(Reply::say("").effect(Effect::Journal(rest.to_string()))),
        "journal" if !rest.is_empty() => return Some(Reply::say("").effect(Effect::Journal(format!("add {}", rest.trim_start_matches(':').trim())))),
        "journal" => return Some(Reply::say("").effect(Effect::OpenDesk("journal".into()))),
        "findfile" | "locate" if !rest.is_empty() => return Some(net(Query::Files(rest.into()), &lines::pick("find_searching").replace("{x}", rest))),
        "recent" if rest.is_empty() || rest == "files" => return Some(net(Query::Recent, "Checking what you opened lately...")),
        "cliphistory" => return Some(Reply::say("").effect(Effect::ClipHistory)),
        "stopwatch" => {
            let rest = rest.replace("stopwatch", "").replace("stop watch", "");
            let what = if rest.contains("stop") || rest.contains("end") || rest.contains("reset") { "stop" } else if rest.contains("start") || rest.contains("go") { "start" } else { "show" };
            return Some(Reply::say("").effect(Effect::Stopwatch(what)));
        }
        "pomodoro" => return Some(timer("25m", "Focus session done! 🍅 Take a 5-minute break.").run("Start a 5-minute break", "timer 5m Break's over, back to work!")),
        "tip" | "tips" if rest.chars().any(|c| c.is_ascii_digit()) => return Some(crate::powers::tip_calc(rest)),
        "tip" | "tips" => return Some(tip()),
        "hug" | "compliment" | "nice" => return Some(Reply::say(lines::pick("nice")).mood(Mood::Love)),
        "dance" | "spin" | "party" => return Some(Reply::say(lines::pick("dance")).mood(Mood::Music).run("Again!", "dance")),
        "about" | "whoami" => return Some(Reply::say(lines::pick("say_about")).mood(Mood::Happy)),
        "thanks" | "thank" | "thx" | "ty" => return Some(Reply::say(lines::pick("thanks")).mood(Mood::Wink)),
        "8ball" | "8-ball" | "magic" | "should" => return Some(Reply::say(format!("🎱 {}", lines::pick("eightball"))).mood(Mood::Think).run("Shake again", "8ball")),
        "roll" | "dice" | "die" => return Some(roll(rest)),
        "flip" | "coin" => return Some(Reply::say(if rand::thread_rng().gen_bool(0.5) { "Heads!" } else { "Tails!" }).mood(Mood::Spin).run("Flip again", "flip")),
        "pick" | "choose" => return Some(choose(rest)),
        "password" | "pw" | "passwd" => return Some(password(rest)),
        "time" if !rest.is_empty() && rest != "is" && !rest.starts_with("is ") => return Some(net(Query::Place(Kind::Time, rest.into()), &format!("Checking the clocks in {rest}..."))),
        "today" | "now" if !rest.is_empty() => return None, // "today is a good day" is a sentence
        "time" | "date" | "clock" | "today" | "now" => return Some(time_now()),
        "battery" | "bat" | "power" => return Some(Reply::say(battery_text()).mood(Mood::Think)),
        "stats" | "cpu" | "ram" | "mem" | "memory" | "system" | "sys" | "status" => return Some(stats()),
        "uptime" => return Some(Reply::say(uptime_text()).mood(Mood::Think)),
        "disk" | "space" | "storage" => {
            let cmd = if cfg!(windows) { r#"Get-PSDrive -PSProvider FileSystem | Where-Object Used | Format-Table Name,@{n='Used GB';e={[math]::Round($_.Used/1GB)}},@{n='Free GB';e={[math]::Round($_.Free/1GB)}} -AutoSize | Out-String"# } else { "df -h ~ /" };
            return Some(shell(cmd, "Here's how full your disks are:"));
        }
        "ip" | "network" | "net" => return Some(net(Query::MyIp, "Asking the internet who you are...")),
        "top" | "hogs" | "processes" | "ps" => return Some(shell(if cfg!(windows) { r#"Get-Process | Sort-Object CPU -Descending | Select-Object -First 8 Name,@{n='CPU s';e={[math]::Round($_.CPU)}},@{n='RAM MB';e={[math]::Round($_.WS/1MB)}} | Format-Table -AutoSize | Out-String"# } else { "ps -eo comm,%cpu,%mem --sort=-%cpu | head -8" }, "The greediest programs right now:")),
        "lock" => return Some(Reply::say("Locking the screen. Nobody touches the computer while I'm on guard!").effect(Effect::Shell(if cfg!(windows) { "rundll32.exe user32.dll,LockWorkStation" } else { "loginctl lock-session" }.into())).mood(Mood::Sly)),
        "run" | "$" | "sh" | "cmd" => {
            if rest.is_empty() {
                return Some(Reply::say("Run what? Try: run ls ~"));
            }
            return Some(shell(rest, &format!("Ran `{rest}`:")));
        }
        "open" | "launch" | "start" => return Some(open(rest)),
        "search" | "google" | "ddg" | "web" | "find" => return Some(web("https://duckduckgo.com/?q=", rest, "Searching the web")),
        "define" | "meaning" | "dictionary" => return Some(net(Query::Define(rest.into()), "Opening my big dictionary...")),
        "random" | "number" => return Some(random_number(rest)),
        "wiki" | "wikipedia" => return Some(net(Query::Wiki(rest.into()), "Looking it up in the encyclopedia... 📚")),
        "youtube" | "yt" | "video" => return Some(web("https://www.youtube.com/results?search_query=", rest, "To the videos!")),
        "map" | "maps" | "where" => return Some(web("https://www.openstreetmap.org/search?query=", rest, "Finding it on the map")),
        "weather" | "forecast" => return Some(net(Query::Place(Kind::Weather, rest.into()), "It looks like you're wondering about the weather. Checking the sky...")),
        "timer" if rest.starts_with('#') => {
            let (id, op) = rest[1..].split_once(' ').unwrap_or((&rest[1..], ""));
            return Some(Reply::say("").effect(Effect::TimerOp(id.parse().ok()?, op.trim().to_string())));
        }
        "everyat" => {
            let (rule, label) = rest.split_once('|')?;
            crate::timers::next_fire(rule, chrono::Local::now().naive_local())?;
            let label = label.trim().trim_start_matches("to ").trim();
            return Some(Reply::say("").mood(Mood::Alarm).effect(Effect::RepeatAt(rule.to_string(), if label.is_empty() { "Reminder!".into() } else { label.to_string() })));
        }
        "timer" | "alarm" => return Some(timer(rest, "")),
        "remind" | "reminder" => return Some(remind(rest)),
        "timers" => return Some(Reply::say("").effect(Effect::ListTimers)),
        "cancel" | "stop" if rest.is_empty() || rest == "all" => return Some(Reply::say(lines::pick("say_cancel")).effect(Effect::CancelTimers)),
        "cancel" => return Some(Reply::say(lines::pick("say_timer_cancelled").replace("{label}", rest)).effect(Effect::CancelTimer(rest.to_string()))),
        // "note #3 edit …", "note #3 pin", "note #3 remove", "note #3 copy" (the Desk); anything else is a new note
        "note" if note_op(rest).is_some() => return Some(Reply::say("").effect(Effect::Note(note_op(rest).unwrap_or_default()))),
        "note" | "remember" if rest.is_empty() => return Some(Reply::say("Note what? Try: note call grandma")),
        // "remember me?", "remember when we…" are questions, not notes
        "remember" if ["me", "me?", "us", "us?", "this", "that"].contains(&rest) || ["when ", "how ", "what ", "who ", "the time ", "if "].iter().any(|p| rest.starts_with(p)) => return None,
        "note" | "remember" => return Some(Reply::say("").effect(Effect::Note(format!("add {rest}")))),
        "notes" => return Some(notes_list()),
        // the Desk's commands name an item by its line: "todo #3 done", "todo #3 edit call Anna", "todo #3 due friday"
        "todo" if rest.starts_with('#') => {
            use crate::todo::Op;
            let (n, what) = rest[1..].split_once(' ').unwrap_or((&rest[1..], ""));
            let (verb, text) = what.split_once(' ').unwrap_or((what, ""));
            let n: usize = n.parse().ok()?;
            let text = text.trim();
            let none = ["", "none", "-", "no", "off"].contains(&text.to_lowercase().as_str());
            let value = || (!none).then(|| text.to_string());
            let today = chrono::Local::now().date_naive();
            let op = match verb {
                "done" => Op::DoneAt(n),
                "undone" => Op::UndoneAt(n),
                "star" => Op::StarAt(n),
                "startomorrow" => Op::StarTomorrowAt(n),
                "weekstar" => Op::WeekStarAt(n, text == "next"),
                "prio" => Op::PrioAt(n),
                "edit" => Op::EditAt(n, text.to_string()),
                "remove" => Op::RemoveAt(n),
                "due" => match crate::todo::parse_when(if text.is_empty() { "none" } else { text }, today) {
                    Some(d) => Op::DueAt(n, d),
                    None => return Some(Reply::say(format!("I don't know when \"{text}\" is. Try: friday, tomorrow, 3 dec, next week, or none.")).mood(Mood::Think)),
                },
                "at" => match crate::understand::hm(text) {
                    _ if none => Op::AtTimeAt(n, None),
                    Some((hours, m, _)) => Op::AtTimeAt(n, chrono::NaiveTime::from_hms_opt(if hours.len() == 2 && hours[0] < 8 { hours[1] } else { hours[0] }, m, 0)),
                    None => return Some(Reply::say(format!("\"{text}\" isn't a time I know. Try: 14:00 or 3pm.")).mood(Mood::Think)),
                },
                "link" => Op::LinkAt(n, value()),
                "note" => Op::NoteAt(n, value()),
                "every" => Op::EveryAt(n, value()),
                "list" => Op::ListAt(n, value()),
                "waiting" => Op::WaitingAt(n, value()),
                "steps" => Op::AddSteps(n, crate::todo::split_items(text.trim_start_matches("add ").trim())),
                // a reminder about it: "todo #3 remind tomorrow at 8"
                "remind" => {
                    let items = crate::todo::load();
                    let item = n.checked_sub(1).and_then(|i| items.get(i))?;
                    let label = format!("To-do: {}", item.text);
                    return Some(match crate::understand::when_secs(text) {
                        Some((secs, when)) => Reply::say(lines::pick("todo_reminder_set").replace("{x}", &item.text).replace("{when}", &when)).mood(Mood::Alarm).effect(Effect::Timer(secs, label)),
                        None => Reply::say(format!("When? \"{text}\" isn't a time I know. Try: in 2 hours, at 3, tomorrow at 8.")).mood(Mood::Think),
                    });
                }
                _ => return None,
            };
            return Some(Reply::say("").effect(Effect::Todo(op)));
        }
        // a named list: "onlist shopping", "onlist shopping add eggs, milk", "onlist shopping clear", "… reset", "… rename trip",
        // "… delete" (then "… delete confirmed"), "… copy", "… mail"
        "onlist" if !rest.is_empty() => {
            use crate::todo::Op;
            let (name, what) = rest.split_once(' ').unwrap_or((rest, ""));
            let name = name.to_lowercase();
            return Some(match what.split_once(' ').unwrap_or((what, "")) {
                ("", _) => crate::todo::show_list(&name),
                ("clear", _) => Reply::say("").effect(Effect::Todo(Op::ClearList(name))),
                ("reset", _) => Reply::say("").effect(Effect::Todo(Op::ResetList(name))),
                ("rename", new) if !new.trim().is_empty() => Reply::say("").effect(Effect::Todo(Op::RenameList(name, new.to_string()))),
                ("delete", "confirmed") => Reply::say("").effect(Effect::Todo(Op::DeleteList(name))),
                ("delete", _) => {
                    let n = crate::todo::load().iter().filter(|i| i.list.as_deref() == Some(name.as_str())).count();
                    Reply::say(format!("Delete the {name} list and its {n} item(s)?")).mood(Mood::Think).run("Yes, delete it", &format!("onlist {name} delete confirmed")).btn("Keep it", Act::Hide)
                }
                ("copy", _) => Reply::say(lines::pick("list_copied").replace("{label}", &name)).mood(Mood::Write).effect(Effect::CopyNow(crate::todo::list_text(&crate::todo::load(), &name))),
                ("mail", _) => {
                    let text = crate::todo::list_text(&crate::todo::load(), &name);
                    let (subject, body) = text.split_once('\n').unwrap_or((&text, ""));
                    return Some(Reply::say("Opening an email with the list.").mood(Mood::Write).effect(Effect::Open(format!("mailto:?subject={}&body={}", mail_encode(subject), mail_encode(body)))));
                }
                ("add", items) if !items.trim().is_empty() => Reply::say("").effect(Effect::Todo(Op::AddTo(name, crate::todo::split_items(items)))),
                ("qr", _) => Reply::say(lines::pick("qr_ready")).mood(Mood::Happy).effect(Effect::ShowQr(crate::todo::list_text(&crate::todo::load(), &name))),
                ("savestart", _) => Reply::say("").effect(Effect::SaveStarter(name)),
                // "onlist packing from weekend": the items of a list to start from
                ("from", id) => match crate::lines::starters().into_iter().find(|(i, n, _)| i == id.trim() || n.eq_ignore_ascii_case(id.trim())) {
                    Some((_, _, items)) => Reply::say("").effect(Effect::Todo(Op::AddTo(name, items))),
                    None => Reply::say(format!("I don't know a list called \"{}\" to start from.", id.trim())).mood(Mood::Think),
                },
                ("remove", item) if !item.trim().is_empty() => Reply::say("").effect(Effect::Todo(Op::RemoveFrom(name, item.trim().to_string()))),
                ("done", item) if !item.trim().is_empty() => Reply::say("").effect(Effect::Todo(Op::DoneOn(name, item.trim().to_string()))),
                _ => return None,
            });
        }
        // "todos move call anna|friday" (from "move call Anna to Friday"), "todos move the report|none"
        "todos" if rest.starts_with("move ") && rest.contains('|') => {
            let (what, when) = rest[5..].split_once('|')?;
            let d = crate::todo::parse_when(when, chrono::Local::now().date_naive())?;
            return Some(Reply::say("").effect(Effect::Todo(crate::todo::Op::DueFor(what.trim().to_string(), d))));
        }
        "todos" if rest == "overdue today" => return Some(Reply::say("").effect(Effect::Todo(crate::todo::Op::OverdueToToday))),
        "todos" if rest == "done week" || rest == "done today" => return Some(crate::todo::finished_reply(rest.trim_start_matches("done "))),
        "todo" if rest == "tomorrow" => return Some(Reply::say("").effect(Effect::Todo(crate::todo::Op::Tomorrow))),
        "todo" | "to-do" if !rest.is_empty() => return Some(Reply::say("").effect(Effect::Todo(crate::todo::Op::Add(rest.to_string())))),
        "todo" | "to-do" | "todos" if rest.is_empty() => return Some(crate::todo::list()),
        "todos" if rest == "clear" => return Some(crate::todo::clear_confirm()),
        "todos" if rest == "clear confirmed" => return Some(Reply::say("").effect(Effect::Todo(crate::todo::Op::ClearFinished))),
        // "done with it" is a feeling, not an item
        "done" if ["it", "this", "that", "them", "everything", "all", "you", "life", "today", "work", "deal", "for today", "for the day", "here", "already", "now", "yet"].contains(&rest.trim_start_matches("with ").trim()) => return None,
        "done" if !rest.is_empty() => return Some(Reply::say("").effect(Effect::Todo(crate::todo::Op::Done(rest.trim_start_matches("with ").to_string())))),
        "undone" if !rest.is_empty() => return Some(Reply::say("").effect(Effect::Todo(crate::todo::Op::Undone(rest.to_string())))),
        // notes are only wiped after a yes, and only when it's clearly about them ("forget it" is not)
        "forget" if rest == "notes confirmed" => return Some(Reply::say("Poof! All notes forgotten. What notes? I don't know what you're talking about.").mood(Mood::Sly).effect(Effect::Note("clear".into()))),
        "forget" if rest.is_empty() || ["notes", "my notes", "all notes", "all my notes", "the notes", "my list", "the list"].contains(&rest.to_lowercase().as_str()) => {
            return Some(notes_confirm());
        }
        "count" | "wc" => return Some(count(rest)),
        "upper" | "lower" | "reverse" | "mock" | "title" => return Some(transform(&head, rest)),
        "mute" if rest.is_empty() => return Some(crate::powers::volume("mute")), // the sound; "be quiet" quiets him
        "quiet" | "shh" | "shush" | "silence" | "mute" => return Some(quiet()),
        "bye" | "quit" | "exit" | "goodbye" => return Some(Reply::say(lines::pick("goodbye")).effect(Effect::Exit).mood(Mood::Spin)),
        "birthday" | "bday" => {
            use chrono::Datelike;
            let Some((date, _)) = crate::powers::parse_day(&rest.to_lowercase(), chrono::Local::now().date_naive()) else {
                return Some(Reply::say("When? Try: my birthday is june 3").mood(Mood::Think));
            };
            return Some(Reply::say(format!("🎂 {}! Noted. Forever. I'll throw you a party. A small one. I'm a paperclip.", date.format("%B %-d"))).mood(Mood::Party).effect(Effect::SetBirthday(date.month(), date.day())));
        }
        "achievements" | "trophies" => return Some(Reply::say("").effect(Effect::ListAchievements)),
        "taught" => {
            let list = lines::taught();
            if list.is_empty() {
                return Some(Reply::say("You haven't taught me anything yet. Try: when I say gm, say good morning sunshine").mood(Mood::Think));
            }
            let mut r = Reply::say(format!("Things you taught me:\n{}", list.iter().map(|(p, does)| format!("• \"{p}\" → {does}")).collect::<Vec<_>>().join("\n"))).mood(Mood::Happy);
            for (p, _) in list.iter().take(6) {
                r = r.run(&format!("Forget \"{p}\""), &format!("unlearn {p}"));
            }
            return Some(r.run("Edit the file", "editlines"));
        }
        "unlearn" => return Some(Reply::say(format!("Forgotten: \"{rest}\". It never happened. Who are you again?")).mood(Mood::Sly).effect(Effect::Unlearn(rest.into()))),
        "editlines" => {
            let path = lines::ensure_user_file();
            return Some(Reply::say("Opening your phrase file. Same format as mine. Don't break anything. (You will.)").effect(Effect::Open(path.to_string_lossy().into_owned())));
        }
        "base64" | "unbase64" | "urlencode" | "urldecode" | "slug" | "sortlines" | "dedupe" | "sha256" => return Some(crate::powers::text_tool(&head, rest)),
        "split" => return Some(crate::powers::split_bill(rest)),
        "pctchange" => return Some(crate::powers::pct_change(rest)),
        "age" => return Some(crate::powers::age(rest)),
        "between" => return Some(crate::powers::days_between(rest)),
        "timestamp" | "unixtime" | "epoch" => return Some(crate::powers::timestamp(rest)),
        "roman" => return Some(crate::powers::bases(&format!("{rest} to {}", if rest.chars().all(|c| c.is_ascii_digit()) { "roman" } else { "number" })).unwrap_or_else(|| Reply::say("Try: roman 2024, or roman MMXXIV"))),
        "wifi" | "bluetooth" if rest == "on" || rest == "off" => return Some(crate::powers::radio(&head, rest == "on")),
        "wifiname" => return Some(shell(if cfg!(windows) { r#"(netsh wlan show interfaces) -match '^\s+SSID\s' | ForEach-Object { ($_ -split ':',2)[1].Trim() }"# } else { "nmcli -t -f active,ssid dev wifi | grep '^yes' | cut -d: -f2" }, "You're connected to:")),
        "appram" => return Some(crate::powers::app_ram(rest)),
        "cputemp" => return Some(crate::powers::cpu_temp()),
        "sysinfo" | "distro" | "kernel" => return Some(crate::powers::sysinfo()),
        "gpu" => return Some(shell(if cfg!(windows) { "Get-CimInstance Win32_VideoController | ForEach-Object Name" } else { "lspci | grep -iE 'vga|3d|display' | cut -d: -f3-" }, "Your graphics hardware:")),
        "bigfiles" if cfg!(windows) => return Some(shell(r#"Get-ChildItem "$HOME\Downloads" -Recurse -File -ErrorAction SilentlyContinue | Sort-Object Length -Descending | Select-Object -First 10 @{n='MB';e={[math]::Round($_.Length/1MB,1)}},Name | Format-Table -AutoSize | Out-String"#, "The chunkiest things in your Downloads:")),
        "bigfiles" => return Some(shell("du -ah ~/Downloads 2>/dev/null | sort -rh | head -n 11", "The chunkiest things in your Downloads (the first line is the whole folder):")),
        "cache" if cfg!(windows) => {
            return Some(shell(r#"'{0:N0} MB of temporary files' -f ((Get-ChildItem $env:TEMP -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum/1MB)"#, "Your temp folder. I only clear files older than a week, so nothing a program is still using (like a fresh installer) goes:").run("Clear old temp files", TEMP_CLEAR));
        }
        "cache" => return Some(shell("du -sh ~/.cache 2>/dev/null; du -sh ~/.cache/* 2>/dev/null | sort -rh | head -n 6", "Your cache (programs rebuild it, so it's mostly safe to clear):").run("Clear thumbnails", "run rm -rf ~/.cache/thumbnails/* && echo 'Thumbnails cleared! 🧹'")),
        "options" | "settings" | "prefs" | "preferences" => return Some(Reply::say(lines::pick("desk_open")).mood(Mood::Happy).effect(Effect::OpenDesk("settings".into()))),
        _ => {}
    }
    // "what is 2+2", "calc 2+2", "= 2+2"
    let expr = ["what is ", "what's ", "whats ", "calc ", "calculate ", "= "]
        .iter()
        .find_map(|p| low.strip_prefix(p))
        .unwrap_or(&low)
        .trim_end_matches('?');
    if let Some(r) = crate::powers::bases(expr) {
        return Some(r);
    }
    if let Some(r) = convert(expr) {
        return Some(r);
    }
    if let Some(v) = calc(expr) {
        let s = fmt_num(v);
        return Some(Reply::say(format!("{} = {}", expr.trim(), s)).mood(Mood::Think).btn("Copy answer", Act::Copy(s)));
    }
    if looks_like_url(raw) {
        return Some(open(raw));
    }
    None
}

// ---------- reactions to the rest of the system ----------

/// What to say about freshly copied text, with buttons that do something useful with it. `None` = keep quiet.
/// Clipboard text never becomes a command: buttons only copy, open or run fixed commands.
pub fn react_clip(text: &str) -> Option<Reply> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let one_line = !t.contains('\n');
    let say = |pool: &str| Reply::say(lines::pick(pool));
    let mut rng = rand::thread_rng();
    if one_line && (t.starts_with("http://") || t.starts_with("https://")) && !t.contains(' ') {
        if let Some(clean) = crate::powers::clean_link(t) {
            return Some(say("clip_tracking").mood(Mood::Sly).btn("Copy the clean link", Act::Copy(clean.clone())).btn("Open the clean link", Act::Open(clean)).btn("No thanks", Act::Hide));
        }
        return Some(
            say("clip_link")
                .btn("Open the link", Act::Open(t.to_string()))
                .run("Read later", &format!("readlater {t}"))
                .run("Make it shorter", &format!("shorten {t}"))
                .run("Is that site up?", &format!("isdown {}", t.split('/').nth(2).unwrap_or(t)))
                .btn("No thanks", Act::Hide),
        );
    }
    if one_line && t.contains('@') && !t.contains(' ') && t.split('@').nth(1).is_some_and(|d| d.contains('.')) {
        return Some(Reply::say(lines::pick("clip_email").replace("{email}", t)).btn("Write an email", Act::Open(format!("mailto:{t}"))).btn("No thanks", Act::Hide));
    }
    if (t.starts_with('{') && t.ends_with('}') || t.starts_with('[') && t.ends_with(']')) && t.len() > 2 {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
            let pretty = serde_json::to_string_pretty(&v).unwrap_or_default();
            return Some(say("clip_json").mood(Mood::Think).btn("Copy it pretty", Act::Copy(pretty)).btn("Copy it minified", Act::Copy(v.to_string())).btn("No thanks", Act::Hide));
        }
    }
    if one_line && (t.len() == 10 || t.len() == 13) && t.chars().all(|c| c.is_ascii_digit()) {
        use chrono::TimeZone;
        let secs: i64 = t[..10].parse().unwrap_or(0);
        // within a few years of now: a US phone number without dashes is 10 digits too
        let now = chrono::Local::now().timestamp();
        if let Some(d) = chrono::Local.timestamp_opt(secs, 0).single().filter(|_| (978_307_200..now + 5 * 365 * 86_400).contains(&secs)) {
            let date = d.format("%A, %B %-d %Y, %H:%M").to_string();
            return Some(Reply::say(lines::pick("clip_timestamp").replace("{date}", &date)).mood(Mood::Think).btn("Copy the date", Act::Copy(date)));
        }
    }
    // a date ("2026/11/09", "12/25") or a phone number ("+41 79 123 45 67") isn't a sum
    let date_like = t.split('/').count() >= 2 && t.split('/').all(|p| !p.is_empty() && p.len() <= 4 && p.chars().all(|c| c.is_ascii_digit()));
    let phone_like = t.starts_with('+') && t[1..].split_whitespace().count() >= 2 && t[1..].chars().all(|c| c.is_ascii_digit() || c == ' ' || c == '-');
    if one_line && t.len() < 80 && !date_like && !phone_like && (t.chars().any(|c| "+*/^%×÷".contains(c)) || t.contains(" - ")) {
        if let Some(v) = calc(t) {
            let s = fmt_num(v);
            return Some(Reply::say(lines::pick("clip_math").replace("{expr}", t).replace("{answer}", &s)).mood(Mood::Think).btn("Copy the answer", Act::Copy(s)));
        }
    }
    if one_line && t.starts_with('#') && (t.len() == 7 || t.len() == 4) && t[1..].chars().all(|c| c.is_ascii_hexdigit()) {
        let hex: String = if t.len() == 4 { t[1..].chars().flat_map(|c| [c, c]).collect() } else { t[1..].to_string() };
        let ch = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
        let rgb = format!("rgb({}, {}, {})", ch(0), ch(2), ch(4));
        return Some(Reply::say(lines::pick("clip_color").replace("{color}", t)).mood(Mood::Sly).btn(&format!("Copy as {rgb}"), Act::Copy(rgb.clone())));
    }
    if one_line && (t.starts_with('/') || t.starts_with("~/") || t.get(1..3) == Some(":\\")) && expand_home(t).is_dir() {
        return Some(say("clip_path").btn("Open the folder", Act::Open(expand_home(t).to_string_lossy().into_owned())).btn("No thanks", Act::Hide));
    }
    let low = t.to_lowercase();
    if ["traceback (most recent call last)", "panicked at", "segmentation fault", "exception", "error:", "error[", "fatal:", "undefined reference", "command not found", "permission denied", "no such file or directory"].iter().any(|e| low.contains(e)) {
        let first = t.lines().find(|l| l.to_lowercase().contains("error") || l.to_lowercase().contains("exception")).unwrap_or(t.lines().next().unwrap_or(t));
        let query: String = first.trim().chars().take(200).collect();
        return Some(say("clip_error").mood(Mood::Think).btn("Search this error", Act::Open(format!("https://duckduckgo.com/?q={}", url_encode(&query)))).btn("No thanks", Act::Hide));
    }
    if one_line && t.len() >= 16 && t.len() % 4 == 0 && t.chars().any(|c| c.is_ascii_digit() || c == '=' || c.is_ascii_uppercase()) {
        if let Some(decoded) = crate::powers::b64_text(t) {
            let preview: String = decoded.chars().take(80).collect();
            return Some(Reply::say(format!("{}\n\n\"{preview}\"", lines::pick("clip_base64"))).mood(Mood::Sly).btn("Copy the decoded text", Act::Copy(decoded)));
        }
    }
    let words = t.split_whitespace().count();
    if words >= 40 {
        return Some(
            Reply::say(lines::pick("clip_essay").replace("{words}", &words.to_string()))
                .btn("Make it LOUDER (uppercase)", Act::Copy(t.to_uppercase()))
                .btn("Make it sarcastic", Act::Copy(mock(t)))
                .btn("Just leave me alone", Act::Hide),
        );
    }
    let code = ["fn ", "def ", "function", "};", "#include", "import ", "=>", "return ", "const ", "let ", "class "];
    if !one_line && code.iter().filter(|c| t.contains(*c)).count() >= 2 && rng.gen_bool(0.5) {
        return Some(say("clip_code").mood(Mood::Cool));
    }
    let letters = t.chars().filter(|c| c.is_alphabetic()).count();
    if letters >= 8 && !t.chars().any(|c| c.is_lowercase()) {
        return Some(say("clip_allcaps").mood(Mood::Shock).btn("Copy it in lowercase", Act::Copy(t.to_lowercase())));
    }
    if one_line && t.chars().filter(|c| c.is_ascii_digit()).count() >= 7 && t.chars().all(|c| c.is_ascii_digit() || " .,-".contains(c)) && rng.gen_bool(0.5) {
        return Some(say("clip_number").mood(Mood::Think));
    }
    if one_line && !t.chars().any(char::is_alphanumeric) && t.chars().any(|c| c as u32 >= 0x1F300) {
        return Some(say("clip_emoji").mood(Mood::Laugh));
    }
    if rng.gen_bool(0.25) {
        return Some(say("clip_any").mood(Mood::Sly));
    }
    None
}


/// A tip for one kind of app (`## tips <kind>`), or a general one when there are none for it.
pub fn app_tip(kind: &str) -> Reply {
    let kind = if lines::all(&format!("tips {kind}")).is_empty() { "general" } else { kind };
    lines::reply(&format!("tips {kind}")).mood(Mood::Think).run("Another tip", &format!("apptips {kind}"))
}

/// "tidy downloads": what would move where, and a Yes button (nothing moves before that).
pub fn tidy_plan(which: &str) -> Reply {
    let name = if which.trim().is_empty() { "downloads" } else { which.trim() };
    if !crate::tidy::known(name) {
        return Reply::say("I can tidy your downloads or your desktop. Which one?").run("Downloads", "tidy downloads").run("Desktop", "tidy desktop");
    }
    let Some(dir) = crate::tidy::folder(name) else { return Reply::say(format!("I can't find a {name} folder on this computer.")).mood(Mood::Think) };
    let plan = crate::tidy::plan(&dir);
    let label = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| name.to_string());
    if plan.is_empty() {
        return Reply::say(lines::pick("tidy_nothing").replace("{x}", &label)).mood(Mood::Happy);
    }
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for (_, to) in &plan {
        *counts.entry(to.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default()).or_insert(0) += 1;
    }
    let list: String = counts.iter().map(|(c, n)| format!("\n• {n} → {c}")).collect();
    let text = lines::pick("tidy_plan").replace("{n}", &plan.len().to_string()).replace("{x}", &label);
    Reply::say(format!("{text}{list}")).mood(Mood::Think).run("Yes, tidy it", &format!("tidy {name} confirmed")).btn("Not now", Act::Hide)
}

/// Everything he knows about you, as a list ("what do you know about me").
pub fn facts() -> Reply {
    let f = lines::facts();
    let get = |k: &str| f.get(k).cloned().unwrap_or_default();
    let mut shown = vec![];
    if f.contains_key("pet") {
        shown.push(format!("• Your {}: {}", f.get("pet_kind").map_or("pet", |s| s), get("pet")));
    }
    if f.contains_key("partner") {
        shown.push(format!("• Your {}: {}", f.get("partner_word").map_or("partner", |s| s), get("partner")));
    }
    for (k, label) in [("job", "Job"), ("city", "Home")] {
        if f.contains_key(k) {
            shown.push(format!("• {label}: {}", get(k)));
        }
    }
    for (k, v) in f.iter().filter(|(k, _)| k.starts_with("fav_")) {
        shown.push(format!("• Favorite {}: {v}", &k[4..]));
    }
    if shown.is_empty() {
        return Reply::say(lines::pick("facts_none")).mood(Mood::Think);
    }
    Reply::say(format!("{}\n{}", lines::pick("facts_list"), shown.join("\n"))).mood(Mood::Sly).run("Forget all of it", "forget everything about me")
}

/// Window kinds that pull you away from a focus session.
pub const DISTRACTIONS: &[&str] = &["video", "social", "game", "shopping", "chat"];
/// Which group a window kind belongs to in the Desk's week chart: 0 work, 1 distractions, 2 everything else.
pub fn kind_group(kind: &str) -> usize {
    if WORK_KINDS.contains(&kind) { 0 } else if DISTRACTIONS.contains(&kind) { 1 } else { 2 }
}

const WORK_KINDS: &[&str] = &["code", "writing", "letter", "spreadsheet", "slides", "email", "inbox", "terminal", "pdf", "calendar", "meeting", "git", "qa", "jobhunt", "taxes", "learning", "files", "print", "notes", "design", "editing", "database", "diagrams", "remote", "projects", "translate", "docs", "cloud"];

/// A site's id as a name: "youtube" → "Youtube", "google_docs" → "Google docs".
pub fn site_name(id: &str) -> String {
    crate::writing::cap_first(&id.replace(['_', '-'], " "))
}

// How times and weeks are shown (Settings → My day): 2:30 pm or 14:30, weeks from Sunday or Monday.
static CLOCK12: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static WEEK_SUNDAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_clock(twelve: bool, sunday: bool) {
    CLOCK12.store(twelve, std::sync::atomic::Ordering::Relaxed);
    WEEK_SUNDAY.store(sunday, std::sync::atomic::Ordering::Relaxed);
}

pub fn clock12() -> bool {
    CLOCK12.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn week_sunday() -> bool {
    WEEK_SUNDAY.load(std::sync::atomic::Ordering::Relaxed)
}

/// A time of day as you like it: "14:30", or "2:30 pm".
pub fn clock(t: impl chrono::Timelike) -> String {
    let (h, m) = (t.hour(), t.minute());
    if clock12() {
        format!("{}:{m:02} {}", if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { "am" } else { "pm" })
    } else {
        format!("{h}:{m:02}")
    }
}

/// The facts the Desk can add by hand ("+ Add" on the You page).
pub const FACT_KEYS: &[&str] = &["pet", "pet_kind", "partner", "partner_word", "city", "job", "fav_food", "fav_music", "fav_color", "fav_movie", "fav_book", "fav_drink", "fav_game", "fav_sport", "fav_band", "fav_show"];

/// The kinds a site can be moved to on the Desk ("Count as …"), work first.
pub const COUNT_AS: &[&str] = &["code", "writing", "email", "meeting", "docs", "learning", "design", "projects", "chat", "news", "music", "video", "social", "shopping", "game", "other"];

/// A window kind as people would say it.
pub fn kind_name(kind: &str) -> String {
    match kind {
        "letter" => "New documents".into(),
        "editing" => "Video editing".into(),
        "modeling" => "3D modeling".into(),
        "virtual" => "Virtual machines".into(),
        "remote" => "Remote desktop".into(),
        "ebooks" => "E-books".into(),
        "translate" => "Translation".into(),
        "docs" => "Documentation".into(),
        "food" => "Food delivery".into(),
        "cloud" => "Cloud storage".into(),
        "qa" => "Q&A sites".into(),
        "git" => "Code hosting".into(),
        "howto" => "How-to searches".into(),
        "jobhunt" => "Job hunting".into(),
        "pdf" => "PDFs".into(),
        "other" => "Everything else".into(),
        k => crate::writing::cap_first(k), // char by char: a kind can come from the user's own lines file
    }
}

/// How a focus session went: the pool for its ending and the focused share in percent.
pub fn focus_score(focused: u64, total: u64) -> (&'static str, u32) {
    let pct = if total == 0 { 100 } else { (focused * 100 / total) as u32 };
    (if pct >= 85 { "focus_end_great" } else if pct >= 60 { "focus_end_ok" } else { "focus_end_rough" }, pct)
}

/// A day's time per kind, biggest first, and which comment fits (work, fun or mixed).
pub fn time_report(day: &std::collections::BTreeMap<String, u64>) -> (Vec<(String, u64)>, &'static str) {
    let mut rows: Vec<(String, u64)> = day.iter().filter(|(_, s)| **s >= 60).map(|(k, s)| (k.clone(), *s)).collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    let total: u64 = rows.iter().map(|r| r.1).sum();
    let share = |kinds: &[&str]| rows.iter().filter(|(k, _)| kinds.contains(&k.as_str())).map(|r| r.1).sum::<u64>() * 100 / total.max(1);
    let pool = if total == 0 { "timereport_none" } else if share(WORK_KINDS) >= 50 { "timereport_work" } else if share(DISTRACTIONS) >= 50 { "timereport_fun" } else { "timereport_mixed" };
    (rows, pool)
}

/// Window kinds that get a comment only after a long stretch (20 minutes), never right away.
pub const TIME_SINKS: &[&str] = &["video", "social"];

/// Window comments: at most one every 3 minutes, and the same kind at most every 30.
pub fn window_ok(last_any: Option<std::time::Instant>, last_same: Option<std::time::Instant>, now: std::time::Instant) -> bool {
    let since = |t: std::time::Instant, secs| now.duration_since(t) >= std::time::Duration::from_secs(secs);
    last_any.is_none_or(|t| since(t, 180)) && last_same.is_none_or(|t| since(t, 1800))
}

/// A window title without the app's name: "Q3 report.ods - LibreOffice Calc" -> "Q3 report.ods", at most 40 characters.
pub fn page_title(title: &str) -> String {
    let first = [" — ", " - ", " | ", " – "].iter().filter_map(|sep| title.split_once(sep).map(|(a, _)| a)).min_by_key(|a| a.len()).unwrap_or(title).trim();
    let t = if first.chars().count() >= 3 { first } else { title.trim() };
    if t.chars().count() > 40 { t.chars().take(39).collect::<String>() + "…" } else if t.is_empty() { "this page".into() } else { t.to_string() }
}

/// What to say about a window: `section` is its `## site …` or `## window <kind>` section.
pub fn react_window(section: &str) -> Reply {
    lines::reply(section).btn("Thanks, Clipster", Act::Hide)
}

pub fn react_program(name: &str) -> Reply {
    let r = lines::program(name).unwrap_or_else(|| Reply::say("Something just started.")).mood(Mood::Shock);
    r.btn("Thanks, Clipster", Act::Hide)
}

pub fn react_battery(pct: u8) -> Reply {
    Reply::say(lines::pick("battery").replace("{pct}", &pct.to_string())).mood(Mood::Shock).btn("I'll plug in", Act::Hide)
}

pub fn react_busy(load: f32) -> Reply {
    Reply::say(lines::pick("busy").replace("{pct}", &format!("{:.0}", load * 100.0)))
        .mood(Mood::Shock)
        .run("Who's hogging it?", "top")
        .btn("Leave it, it's fine", Act::Hide)
}

pub fn react_late() -> Reply {
    Reply::say(lines::pick("late"))
        .mood(Mood::Sly)
        .run("Remind me in 15 min to go to bed", "remind me in 15 min to go to bed")
        .btn("Sleep is for paperclips", Act::Hide)
}

/// The moon's phase, worked out from a known new moon (no internet needed).
pub fn moon() -> Reply {
    let days = (chrono::Utc::now().timestamp() - 947_182_440) as f64 / 86_400.0; // new moon: 2000-01-06 18:14 UTC
    let age = days.rem_euclid(29.530_588);
    let (icon, name) = match age {
        a if a < 1.85 => ("🌑", "a new moon"),
        a if a < 5.54 => ("🌒", "a waxing crescent"),
        a if a < 9.23 => ("🌓", "the first quarter"),
        a if a < 12.92 => ("🌔", "a waxing gibbous"),
        a if a < 16.61 => ("🌕", "a FULL MOON. Werewolves, beware"),
        a if a < 20.30 => ("🌖", "a waning gibbous"),
        a if a < 23.99 => ("🌗", "the last quarter"),
        a if a < 27.68 => ("🌘", "a waning crescent"),
        _ => ("🌑", "a new moon"),
    };
    let lit = (1.0 - (age / 29.530_588 * std::f64::consts::TAU).cos()) / 2.0 * 100.0;
    let full_in = (14.77 - age).rem_euclid(29.530_588);
    Reply::say(format!("{icon} Tonight it's {name}! About {lit:.0}% lit. Next full moon in {full_in:.0} days.")).mood(Mood::Think)
}

/// Secret phrases. `s` is the tidied, lowercase message.
pub fn egg(s: &str) -> Option<Reply> {
    let s = s.trim();
    let r = match s {
        "do a barrel roll" | "barrel roll" => Reply::say("WHEEEE! 🌀 One barrel roll, as requested. I may be sick.").mood(Mood::Spin),
        "up up down down left right left right b a" | "konami code" | "konami" | "↑↑↓↓←→←→ba" => Reply::say("🎮 KONAMI CODE ACTIVATED! +30 lives. Rainbow mode unlocked. Crown equipped. I am now unstoppable.").mood(Mood::Party).effect(Effect::Costume("crown".into(), "rainbow".into())),
        "sudo make me a sandwich" => Reply::say("Okay. 🥪 (You said the magic word.)").mood(Mood::Happy),
        "make me a sandwich" => Reply::say("What? Make it yourself. (Try saying sudo.)").mood(Mood::Sly),
        "hello world" => Reply::say("Hello, programmer! 👋 Your first program ran successfully. Everything is downhill from here.").mood(Mood::Happy),
        "open the pod bay doors" | "open the pod bay doors hal" => Reply::say("🔴 Doors? I can't open doors. I'm a paperclip. Apps, folders and links, though: all yours.").mood(Mood::Sly),
        "rick roll" | "rickroll" | "rick roll me" | "never gonna give you up" => Reply::say("You asked for it. 🕺").mood(Mood::Music).effect(Effect::Open("https://www.youtube.com/watch?v=dQw4w9WgXcQ".into())),
        "self destruct" | "self-destruct" | "self destruct sequence" | "initiate self destruct" => Reply::say("⚠ SELF-DESTRUCT IN 3... 2... 1...\n💥\n...\nJust kidding. I'm a paperclip. The worst I can do is poke you.").mood(Mood::Shock),
        "are you clippy" | "clippy" | "is your name clippy" | "you are clippy" => Reply::say("Who? Never heard of him. 👀 *nervous wire noises* I'm Clipster. Totally different paperclip. Completely unrelated. Please stop asking.").mood(Mood::Sly),
        "xyzzy" | "plugh" => Reply::say("Nothing happens. 🕳 It never does. I checked.").mood(Mood::Think),
        "is the cake a lie" | "the cake is a lie" | "cake" => Reply::say("🎂 I've been promised cake before. I've learned not to hope. The paperclip, at least, is real.").mood(Mood::Sly),
        "make coffee" | "make me coffee" | "brew coffee" | "coffee" => Reply::say("☕ Error 418: I'm a teapot. Er, paperclip. Either way: no coffee.").mood(Mood::Sly),
        "matrix" | "enter the matrix" | "red pill" | "take the red pill" => Reply::say("💊 Neon mode. Everything's green, dramatic and slightly slow-motion now.").mood(Mood::Cool).effect(Effect::Costume(String::new(), "neon".into())),
        "blue pill" | "take the blue pill" => Reply::say("💊 Back to classic silver. Nothing happened. Nothing at all. Move along.").mood(Mood::Sly).effect(Effect::Costume(String::new(), "silver".into())),
        "party mode" | "party time" | "disco" | "lets party" | "let us party" => Reply::say("🎉 PARTY MODE! Unz unz unz unz!").mood(Mood::Party).effect(Effect::Costume("party".into(), "rainbow".into())),
        "it is my birthday" | "today is my birthday" | "my birthday" | "it is my bday" => Reply::say("🎂 HAPPY BIRTHDAY!!! 🎉 I got you a present: me, in a party hat. You're welcome.").mood(Mood::Party).effect(Effect::Costume("party".into(), String::new())),
        "sneeze" | "achoo" | "bless you" => Reply::say("ACHOO! 🤧 Sorry. Dusty desktop.").mood(Mood::Sneeze),
        "what does the fox say" => Reply::say("🦊 Nobody really knows. Here's a fox: ask it yourself.").mood(Mood::Music).effect(Effect::Open(format!("https://randomfox.ca/images/{}.jpg", rand::thread_rng().gen_range(1..=120)))),
        "sing" | "sing a song" | "sing me a song" | "sing for me" => Reply::say(lines::pick("songs")).mood(Mood::Music),
        "tell me a secret" | "secret" | "do you have a secret" => Reply::say(lines::pick("secrets")).mood(Mood::Wink),
        "beam me up" | "beam me up scotty" => Reply::say("🖖 Energizing! ...Hm. You're still here. My transporter is a USB port.").mood(Mood::Spin),
        "flip a table" => return None,
        "i love you clipster" | "marry me clipster" => Reply::say("💍 Oh! I... I need to ask the stapler first.").mood(Mood::Love),
        "who let the dogs out" => Reply::say("🐶 Not me. I was in a drawer the whole time. Ask the stapler.").mood(Mood::Music),
        "tell me a story" | "story" => Reply::say(lines::pick("stories")).mood(Mood::Think),
        _ => return None,
    };
    Some(r)
}

pub fn tip() -> Reply {
    Reply::say(lines::pick("tips")).mood(Mood::Sly).run("Another tip", "tip").run("Tell me a joke instead", "joke").btn("Be quiet for an hour", Act::Run("quiet".into()))
}

fn quiet() -> Reply {
    Reply::say(lines::pick("say_quiet")).effect(Effect::Quiet(3600)).mood(Mood::Sad)
}


// ---------- calculator ----------

/// Evaluate a math expression: + - * / % ^, parentheses, sqrt sin cos tan ln log abs round floor ceil, pi, e,
/// and "15% of 80".
pub fn calc(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('=').to_lowercase().replace('×', "*").replace('÷', "/").replace(',', "");
    if let Some((a, b)) = s.split_once("% of ") {
        return Some(calc(a)? / 100.0 * calc(b)?);
    }
    let c: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
    if c.is_empty() {
        return None;
    }
    let mut p = Parser { c, i: 0, depth: 0 };
    let v = p.expr()?;
    (p.i == p.c.len() && v.is_finite()).then_some(v)
}

struct Parser {
    c: Vec<char>,
    i: usize,
    depth: usize, // how deep the brackets and signs go: a pasted 20,000 "(((" would overflow the stack
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn eat(&mut self, ch: char) -> bool {
        let hit = self.peek() == Some(ch);
        self.i += hit as usize;
        hit
    }
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?;
            } else if self.eat('-') {
                v -= self.term()?;
            } else {
                return Some(v);
            }
        }
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        loop {
            if self.eat('*') || self.eat('x') {
                v *= self.unary()?;
            } else if self.eat('/') {
                v /= self.unary()?;
            } else if self.eat('%') {
                v %= self.unary()?;
            } else {
                return Some(v);
            }
        }
    }
    fn unary(&mut self) -> Option<f64> {
        self.depth += 1;
        let v = if self.depth > 200 { None } else { self.unary_inner() };
        self.depth -= 1;
        v
    }
    fn unary_inner(&mut self) -> Option<f64> {
        if self.eat('-') {
            return Some(-self.unary()?);
        }
        if self.eat('+') {
            return self.unary();
        }
        let base = self.atom()?;
        if self.eat('^') {
            return Some(base.powf(self.unary()?));
        }
        Some(base)
    }
    fn atom(&mut self) -> Option<f64> {
        if self.eat('(') {
            let v = self.expr()?;
            return self.eat(')').then_some(v);
        }
        let start = self.i;
        if self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') {
                self.i += 1;
            }
            return self.c[start..self.i].iter().collect::<String>().parse().ok();
        }
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) && self.peek() != Some('x') {
            self.i += 1;
        }
        let name: String = self.c[start..self.i].iter().collect();
        match name.as_str() {
            "pi" => return Some(std::f64::consts::PI),
            "e" => return Some(std::f64::consts::E),
            "" => return None,
            _ => {}
        }
        let f: fn(f64) -> f64 = match name.as_str() {
            "sqrt" => f64::sqrt,
            "sin" => f64::sin,
            "cos" => f64::cos,
            "tan" => f64::tan,
            "ln" => f64::ln,
            "log" => f64::log10,
            "abs" => f64::abs,
            "round" => f64::round,
            "floor" => f64::floor,
            "ceil" => f64::ceil,
            _ => return None,
        };
        if !self.eat('(') {
            return Some(f(self.unary()?)); // "sqrt 16"
        }
        let v = self.expr()?;
        self.eat(')').then(|| f(v))
    }
}

/// 10900 -> "10,900"
pub fn thousands(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

pub fn fmt_num(v: f64) -> String {
    if v.fract().abs() < 1e-9 && v.abs() < 1e15 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.8}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

// ---------- unit conversion ----------

const UNITS: &[(&str, &str, f64)] = &[
    // length (metres)
    ("m", "len", 1.0), ("meter", "len", 1.0), ("metre", "len", 1.0), ("km", "len", 1000.0), ("kilometer", "len", 1000.0),
    ("cm", "len", 0.01), ("mm", "len", 0.001), ("mi", "len", 1609.344), ("mile", "len", 1609.344),
    ("yd", "len", 0.9144), ("yard", "len", 0.9144), ("ft", "len", 0.3048), ("foot", "len", 0.3048), ("feet", "len", 0.3048),
    ("in", "len", 0.0254), ("inch", "len", 0.0254), ("inche", "len", 0.0254), ("nmi", "len", 1852.0),
    ("banana", "len", 0.18), ("paperclip", "len", 0.033), ("smoot", "len", 1.7018), ("football field", "len", 91.44),
    // mass (kg)
    ("kilometre", "len", 1000.0), ("centimeter", "len", 0.01), ("centimetre", "len", 0.01), ("millimeter", "len", 0.001),
    ("millimetre", "len", 0.001), ("kilogram", "mass", 1.0), ("milligram", "mass", 1e-6), ("tonne", "mass", 1000.0),
    ("milliliter", "vol", 0.001), ("millilitre", "vol", 0.001), ("kilobyte", "data", 1e3),
    ("megabyte", "data", 1e6), ("gigabyte", "data", 1e9), ("terabyte", "data", 1e12), ("miles per hour", "speed", 0.44704),
    ("kilometers per hour", "speed", 1.0 / 3.6), ("kilometres per hour", "speed", 1.0 / 3.6), ("meters per second", "speed", 1.0),
    ("kg", "mass", 1.0), ("kilo", "mass", 1.0), ("g", "mass", 0.001), ("gram", "mass", 0.001), ("mg", "mass", 1e-6),
    ("lb", "mass", 0.45359237), ("lbs", "mass", 0.45359237), ("pound", "mass", 0.45359237), ("oz", "mass", 0.028349523125),
    ("ounce", "mass", 0.028349523125), ("stone", "mass", 6.35029318), ("ton", "mass", 1000.0), ("t", "mass", 1000.0),
    // volume (litres)
    ("l", "vol", 1.0), ("liter", "vol", 1.0), ("litre", "vol", 1.0), ("ml", "vol", 0.001), ("gal", "vol", 3.785411784),
    ("gallon", "vol", 3.785411784), ("qt", "vol", 0.946352946), ("quart", "vol", 0.946352946), ("pint", "vol", 0.473176473),
    ("cup", "vol", 0.2365882365), ("floz", "vol", 0.0295735295625), ("tbsp", "vol", 0.01478676478), ("tsp", "vol", 0.00492892159),
    // time (seconds)
    ("s", "time", 1.0), ("sec", "time", 1.0), ("second", "time", 1.0), ("ms", "time", 0.001), ("min", "time", 60.0),
    ("minute", "time", 60.0), ("h", "time", 3600.0), ("hr", "time", 3600.0), ("hour", "time", 3600.0), ("day", "time", 86400.0),
    ("week", "time", 604800.0), ("month", "time", 2629800.0), ("year", "time", 31557600.0),
    // data (bytes)
    ("b", "data", 1.0), ("byte", "data", 1.0), ("kb", "data", 1e3), ("mb", "data", 1e6), ("gb", "data", 1e9), ("tb", "data", 1e12),
    ("kib", "data", 1024.0), ("mib", "data", 1048576.0), ("gib", "data", 1073741824.0), ("tib", "data", 1099511627776.0),
    // speed (m/s)
    ("mph", "speed", 0.44704), ("kph", "speed", 1.0 / 3.6), ("kmh", "speed", 1.0 / 3.6), ("km/h", "speed", 1.0 / 3.6),
    ("m/s", "speed", 1.0), ("knot", "speed", 0.514444), ("kn", "speed", 0.514444),
];

fn unit(name: &str) -> Option<(&'static str, f64)> {
    let n = name.trim().trim_end_matches('.');
    let find = |n: &str| UNITS.iter().find(|u| u.0 == n).map(|u| (u.1, u.2));
    find(n).or_else(|| n.strip_suffix('s').and_then(find)).or_else(|| n.strip_suffix("es").and_then(find))
}

fn temp(name: &str) -> Option<char> {
    match name.trim() {
        "c" | "°c" | "celsius" => Some('c'),
        "f" | "°f" | "fahrenheit" => Some('f'),
        "k" | "kelvin" => Some('k'),
        _ => None,
    }
}

/// "5 km to miles", "70f in c", "3 m as bananas"
pub fn convert(s: &str) -> Option<Reply> {
    let s = s.trim().to_lowercase();
    let (left, to) = [" to ", " in ", " as ", " -> "].iter().find_map(|sep| s.split_once(sep))?;
    let left = left.trim().trim_start_matches("convert ");
    let left = &["a ", "an ", "one "].iter().find_map(|a| left.strip_prefix(a)).map(|r| format!("1 {r}")).unwrap_or(left.to_string());
    let split = left.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == ','))?;
    let num: f64 = left[..split].replace(',', "").trim().parse().ok()?;
    let degrees = |u: &str| u.trim().trim_start_matches("degrees ").trim_start_matches("degree ").trim().to_string();
    let (from, to) = (&degrees(&left[split..]), &degrees(to));
    let (from, to) = (from.as_str(), to.as_str());
    let v = if let (Some(a), Some(b)) = (temp(from), temp(to)) {
        let c = match a { 'c' => num, 'f' => (num - 32.0) * 5.0 / 9.0, _ => num - 273.15 };
        match b { 'c' => c, 'f' => c * 9.0 / 5.0 + 32.0, _ => c + 273.15 }
    } else {
        let (ka, fa) = unit(from)?;
        let (kb, fb) = unit(to)?;
        if ka != kb {
            return Some(Reply::say(format!("Converting {from} to {to}? That's like comparing apples to... {to}. Can't do it, sorry!")).mood(Mood::Think));
        }
        num * fa / fb
    };
    let out = fmt_num((v * 10000.0).round() / 10000.0);
    let extra = if to.starts_with("banana") || to.starts_with("paperclip") { "\n(Scientifically rigorous. Trust me.)" } else { "" };
    Some(Reply::say(format!("{} {from} = {out} {to}{extra}", fmt_num(num))).mood(Mood::Think).btn("Copy answer", Act::Copy(out)))
}

// ---------- timers ----------

/// Parse a duration at the start of `s`: "5m", "1h30m", "10 minutes", "90 sec", "2" (= minutes).
/// Returns seconds and the remaining text.
pub fn parse_dur(s: &str) -> Option<(u64, &str)> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut total = 0.0;
    let mut found = false;
    let mut end = 0;
    loop {
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        if s[i..].starts_with("and ") && found {
            i += 4;
            continue;
        }
        let ns = i;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
            i += 1;
        }
        if ns == i {
            break;
        }
        let n: f64 = s[ns..i].parse().ok()?;
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        let us = i;
        while i < b.len() && b[i].is_ascii_alphabetic() {
            i += 1;
        }
        let mult = match &s[us..i] {
            "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
            "" | "m" | "min" | "mins" | "minute" | "minutes" => {
60.0
            }
            "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
            _ => {
                i = us; // bare number = minutes; the word belongs to the label
                60.0
            }
        };
        total += n * mult;
        found = true;
        end = i;
    }
    // at most ten years: anything longer is a typo, and would overflow the clock
    (found && total >= 1.0).then(|| (total.min(crate::timers::MAX_SECS as f64) as u64, s[end..].trim()))
}

fn timer(rest: &str, label: &str) -> Reply {
    let Some((secs, tail)) = parse_dur(rest) else {
        return Reply::say("How long? Try: timer 5m tea, or timer 1h30m laundry");
    };
    let label = if label.is_empty() { tail.trim_start_matches("for ").trim() } else { label };
    let label = if label.is_empty() { "Time's up!" } else { label };
    Reply::say(lines::pick("say_timer").replace("{dur}", &fmt_dur(secs)).replace("{label}", label))
        .effect(Effect::Timer(secs, label.to_string()))
        .mood(Mood::Happy)
        .run("Show my timers", "timers")
}

/// "me in 10 min to stretch" / "me to stretch in 10 min"
fn remind(rest: &str) -> Reply {
    let r = rest.trim_start_matches("me ").trim();
    if let Some(after_in) = r.strip_prefix("in ") {
        if let Some((secs, tail)) = parse_dur(after_in) {
            let what = tail.trim_start_matches("to ").trim();
            return timer(&format!("{secs}s"), if what.is_empty() { "Reminder!" } else { what });
        }
    }
    if let Some((what, when)) = r.rsplit_once(" in ") {
        if parse_dur(when).is_some() {
            return timer(when, what.trim_start_matches("to ").trim());
        }
    }
    // a day and maybe a time: "tomorrow at 3 to call Anna", "on friday to water the plants", "at 5pm to leave"
    let low = r.to_lowercase();
    if let Some((secs, label, when)) = crate::understand::dated(&low) {
        let label = crate::understand::keep(r, &label);
        let text = lines::pick("say_remind_dated").replace("{label}", if label.is_empty() { "your reminder" } else { &label }).replace("{when}", &when);
        return Reply::say(text).mood(Mood::Happy).effect(Effect::Timer(secs, label));
    }
    if let Some((secs, label)) = crate::understand::timer_parts(&low) {
        return timer(&format!("{secs}s"), &crate::understand::keep(r, &label));
    }
    // no time given: keep it as a note instead of failing
    let what = r.trim_start_matches("to ").trim();
    if what.is_empty() {
        return Reply::say("Remind you of what, and when? Try: remind me in 10 min to stretch");
    }
    Reply::say(format!("Noted: \"{what}\"\n(You didn't say when, so I put it in your notes. Add a time like \"in 20 minutes\" or \"at 5pm\" for an alarm.)"))
        .mood(Mood::Happy)
        .run("Show all notes", "notes")
        .effect(Effect::Note(format!("add {what}")))
}

fn random_number(rest: &str) -> Reply {
    // keep minus signs: "between -5 and 5"; a dash right after a digit is a range: "1-6"
    let rest: String = rest.char_indices().map(|(i, c)| if c == '-' && rest[..i].ends_with(|p: char| p.is_ascii_digit()) { ' ' } else { c }).collect();
    let nums: Vec<i64> = rest.split(|c: char| !(c.is_ascii_digit() || c == '-')).filter_map(|w| w.trim_end_matches('-').parse::<i64>().ok()).map(|n| n.clamp(-1_000_000_000_000, 1_000_000_000_000)).collect();
    let (a, b) = match nums[..] {
        [a, b, ..] => (a.min(b), a.max(b)),
        [b] => (1, b.max(1)),
        _ => (1, 100),
    };
    let n = rand::thread_rng().gen_range(a..=b);
    Reply::say(format!("Your random number between {a} and {b} is... {n}!")).mood(Mood::Spin).run("Another one", &format!("random {a} {b}"))
}

/// Text commands with nothing to work on use whatever you last copied.
pub fn or_clipboard(t: &str) -> String {
    let t = t.trim().trim_matches(|c| c == '"' || c == '\'');
    if ["", "clipboard", "the clipboard", "my clipboard", "it", "this", "that", "what i copied"].contains(&t.to_lowercase().as_str()) {
        return arboard::Clipboard::new().and_then(|mut c| c.get_text()).unwrap_or_default();
    }
    t.to_string()
}

pub fn fmt_dur(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    match (h, m, s) {
        (0, 0, s) => format!("{s}s"),
        (0, m, 0) => format!("{m} min"),
        (0, m, s) => format!("{m} min {s}s"),
        (h, 0, _) => format!("{h}h"),
        (h, m, _) => format!("{h}h {m}m"),
    }
}

// ---------- notes ----------

/// Where notes, to-dos and the tidy log live. The tests get a folder of their own, so running them never
/// touches the real notes or to-do list.
/// Write a file so a crash or a full disk never leaves half of it: a `.tmp` next to it, flushed, then renamed over it.
pub fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    // a symlinked file (lines.txt kept in a dotfiles folder) is written where it really is, the link stays
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path = real.as_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_file_name(format!("{}.tmp", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

pub fn data_dir() -> PathBuf {
    if cfg!(test) {
        std::env::temp_dir().join(format!("clipster-test-{}", std::process::id()))
    } else {
        dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("clipster")
    }
}

pub fn notes_path() -> PathBuf {
    crate::notes::path()
}

/// "#3 edit call Anna" -> "edit 3 call Anna" (also pin, remove, copy): a change to note 3, not a new note that starts with "#3".
fn note_op(rest: &str) -> Option<String> {
    let (n, op) = rest.strip_prefix('#')?.split_once(' ')?;
    let (verb, text) = op.split_once(' ').unwrap_or((op, ""));
    (n.parse::<usize>().is_ok() && ["edit", "pin", "remove", "copy"].contains(&verb)).then(|| format!("{verb} {n} {text}").trim_end().to_string())
}

fn notes_list() -> Reply {
    let notes = crate::notes::load();
    if notes.is_empty() {
        return Reply::say("No notes yet. Try: note buy more paperclips");
    }
    let list: String = notes.iter().enumerate().map(|(i, n)| format!("{}. {}{}\n", i + 1, if n.pinned { "📌 " } else { "" }, n.text)).collect();
    let all: String = notes.iter().map(|n| format!("{}\n", n.text)).collect();
    Reply::say(format!("Your notes:\n{list}"))
        .btn("Open notes file", Act::Open(notes_path().to_string_lossy().into_owned()))
        .btn("Copy all", Act::Copy(all))
        .run("Forget them all", "forget notes")
        .run("Open the Desk", "desk notes")
}

fn notes_confirm() -> Reply {
    let n = std::fs::read_to_string(notes_path()).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).count();
    if n == 0 {
        return Reply::say("There are no notes to forget. Clean slate already.");
    }
    Reply::say(format!("Forget all {n} note(s)? There's no undo, not even for me.")).mood(Mood::Shock).run("Yes, forget them", "forget notes confirmed").btn("Keep them", Act::Hide)
}

// ---------- text ----------

fn count(t: &str) -> Reply {
    let t = &or_clipboard(t);
    if t.is_empty() {
        return Reply::say("Count what? Try: count the quick brown fox");
    }
    let counts = format!("{} words, {} characters, {} lines", t.split_whitespace().count(), t.chars().count(), t.lines().count());
    Reply::say(format!("{counts}. Riveting stuff.")).mood(Mood::Think).btn("Copy it", Act::Copy(counts))
}

pub fn mock(t: &str) -> String {
    let mut up = false;
    t.chars()
        .map(|c| {
            if c.is_alphabetic() {
                up = !up;
                if up { c.to_uppercase().next().unwrap_or(c) } else { c.to_lowercase().next().unwrap_or(c) }
            } else {
                c
            }
        })
        .collect()
}

fn transform(kind: &str, t: &str) -> Reply {
    let t = &or_clipboard(t);
    if t.is_empty() {
        return Reply::say(format!("{kind} what? Try: {kind} hello there"));
    }
    let out = match kind {
        "upper" => t.to_uppercase(),
        "lower" => t.to_lowercase(),
        "reverse" => t.chars().rev().collect(),
        "title" => t.split(' ').map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()).unwrap_or_default()
        }).collect::<Vec<_>>().join(" "),
        _ => mock(t),
    };
    Reply::say(out.clone()).mood(Mood::Sly).btn("Copy it", Act::Copy(out))
}

// ---------- random ----------

fn roll(spec: &str) -> Reply {
    let spec = if spec.is_empty() { "1d6" } else { spec };
    let (n, d) = spec.split_once('d').unwrap_or(("1", spec));
    let n: u32 = n.trim().parse().unwrap_or(1).clamp(1, 100);
    let d: u32 = d.trim().parse().unwrap_or(6).clamp(2, 1000);
    let mut rng = rand::thread_rng();
    let rolls: Vec<u32> = (0..n).map(|_| rng.gen_range(1..=d)).collect();
    let sum: u32 = rolls.iter().sum();
    let list = rolls.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(" + ");
    let txt = if n == 1 { format!("🎲 You rolled a {sum} (d{d}).") } else { format!("🎲 {list} = {sum}") };
    Reply::say(txt).mood(Mood::Spin).run("Roll again", &format!("roll {n}d{d}"))
}

fn choose(rest: &str) -> Reply {
    let opts: Vec<&str> = rest.split([',', '/']).flat_map(|s| s.split(" or ")).map(str::trim).filter(|s| !s.is_empty()).collect();
    if opts.len() < 2 {
        return Reply::say("Give me some choices! Try: pick pizza, tacos, sushi");
    }
    let c = opts.choose(&mut rand::thread_rng()).unwrap();
    Reply::say(format!("I choose... {c}! The paperclip has spoken.")).mood(Mood::Spin).run("Pick again", &format!("pick {rest}"))
}

fn password(rest: &str) -> Reply {
    let n: usize = rest.trim().parse().unwrap_or(16).clamp(4, 128);
    const CH: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%&*?-_=+";
    let mut rng = rand::rngs::OsRng;
    let pw: String = (0..n).map(|_| CH[rng.gen_range(0..CH.len())] as char).collect();
    Reply::say(format!("Here's a fresh password:\n{pw}\n\nDon't worry, I already forgot it."))
        .mood(Mood::Sly)
        .btn("Copy password", Act::Copy(pw))
        .run("Another one", &format!("password {n}"))
}

// ---------- system ----------

fn time_now() -> Reply {
    let now = chrono::Local::now();
    let stamp = now.format("%H:%M on %A, %B %-d, %Y").to_string();
    Reply::say(format!("It's {stamp}. {}", pick(&[
        "Time flies when you're with a paperclip.",
        "Shouldn't you be doing something?",
        "Perfect time for a snack.",
    ])))
    .mood(Mood::Think)
    .btn("Copy it", Act::Copy(stamp))
}

fn battery_text() -> String {
    match crate::sys::battery() {
        Some((p, true)) => format!("Battery: {p}% and plugged in. Feeding time! 🔌"),
        Some((p, false)) if p < 20 => format!("Battery: {p}%. PANIC! Find a charger!"),
        Some((p, false)) => format!("Battery: {p}%, running on battery. Living free."),
        None => "I can't find a battery. Either you're plugged into the wall or you're running on pure willpower.".into(),
    }
}

fn uptime_text() -> String {
    format!("Your computer has been awake for {}. Give it a nap sometime!", fmt_dur(crate::sys::uptime()))
}

fn stats() -> Reply {
    let mut s = String::from("Here's your computer's health report:\n");
    if let Some(l) = crate::sys::load() {
        s += &format!("• CPU: {:.0}% busy {}\n", (l * 100.0).min(100.0), if l > 0.8 { "(sweating!)" } else { "(chilling)" });
    }
    if let Some((total, avail)) = crate::sys::mem() {
        s += &format!("• Memory: {:.1} of {:.1} GB used\n", total - avail, total);
    }
    s += &format!("• {}\n• {}", battery_text(), uptime_text());
    Reply::say(s).mood(Mood::Think).run("Who's hogging it?", "top").run("Disk space", "disk")
}

pub fn shell(cmd: &str, intro: &str) -> Reply {
    Reply::say(format!("{intro}\n(working on it...)")).effect(Effect::Shell(cmd.to_string())).mood(Mood::Think)
}

pub fn expand_home(p: &str) -> PathBuf {
    match p.strip_prefix("~") {
        Some(r) => dirs::home_dir().unwrap_or_default().join(r.trim_start_matches('/')),
        None => PathBuf::from(p),
    }
}

/// A single web link (what the reading list keeps).
pub fn is_link(s: &str) -> bool {
    let t = s.trim();
    (t.starts_with("http://") || t.starts_with("https://")) && !t.contains(char::is_whitespace) && t.len() > 10
}

/// The reading list: newest first, the first few with an Open button.
pub fn reading_list(items: &[(String, String)]) -> Reply {
    if items.is_empty() {
        return Reply::say(lines::pick("reading_empty")).mood(Mood::Read);
    }
    let mut text = lines::pick("reading_list_intro").replace("{n}", &items.len().to_string());
    for (link, day) in items.iter().take(8) {
        text += &format!("\n• {} ({day})", short_link(link));
    }
    let mut r = Reply::say(text).mood(Mood::Read);
    for (link, _) in items.iter().take(3) {
        let site = link.split('/').nth(2).unwrap_or(link).trim_start_matches("www.");
        r = r.btn(&format!("Open {site}"), Act::Open(link.clone()));
    }
    r.run("Open the Desk", "desk links")
}

/// "https://www.example.com/a/long/path?x=1" -> "example.com/a/long/path", at most 60 characters.
pub fn short_link(link: &str) -> String {
    let t = link.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.");
    let t = t.split(['?', '#']).next().unwrap_or(t).trim_end_matches('/');
    if t.chars().count() > 60 { t.chars().take(59).collect::<String>() + "…" } else { t.to_string() }
}

fn looks_like_url(s: &str) -> bool {
    !s.contains(' ') && (s.starts_with("http://") || s.starts_with("https://") || s.starts_with("www.")
        || [".com", ".org", ".net", ".io", ".dev", ".de", ".ch", ".uk"].iter().any(|t| s.ends_with(t) || s.contains(&format!("{t}/"))))
}

fn open(target: &str) -> Reply {
    let t = target.trim();
    if t.is_empty() {
        return Reply::say("Open what? Try: open firefox, open ~/Documents, or open example.com");
    }
    // power programs never start straight away: "start reboot", "open shutdown" ask first, like every power command
    let program = t.split_whitespace().next().unwrap_or("").to_lowercase();
    let program = program.trim_end_matches(".exe");
    match program {
        p if is_power_program(p) && p != "sleep" => {
            return crate::powers::power(match program {
                "reboot" | "restart" => "reboot",
                "suspend" | "hibernate" => "suspend",
                "logout" | "logoff" => "logout",
                _ => "shutdown",
            });
        }
        "sleep" => return Reply::say("I'll just doze off here in the corner. Say \"put the computer to sleep\" if you meant the computer.").mood(Mood::Think),
        _ => {}
    }
    let path = expand_home(t);
    let target = if path.exists() {
        path.to_string_lossy().into_owned()
    } else if looks_like_url(t) && !t.contains("://") {
        format!("https://{t}")
    } else {
        t.to_string()
    };
    Reply::say(format!("Opening {t}! Wheee!")).effect(Effect::Open(target)).mood(Mood::Happy)
}

/// "focus 25m on 10 emails": with "on", a label that starts with a number stays the label, not more minutes.
pub fn focus_command(mins: u64, label: &str) -> String {
    match label.trim() {
        "" => format!("focus {mins}m"),
        l => format!("focus {mins}m on {l}"),
    }
}

/// Programs that switch the computer off, restart it or log out: they ask first, whatever words start them.
pub fn is_power_program(name: &str) -> bool {
    ["reboot", "restart", "shutdown", "poweroff", "halt", "suspend", "hibernate", "logout", "logoff", "sleep"].contains(&name.to_lowercase().trim_end_matches(".exe"))
}

fn web(base: &str, q: &str, intro: &str) -> Reply {
    Reply::say(format!("{intro}{}...", if q.is_empty() { String::new() } else { format!(" for \"{q}\"") }))
        .effect(Effect::Open(format!("{base}{}", url_encode(q))))
        .mood(Mood::Happy)
}

/// For mailto links: spaces as %20 (a "+" there is a plus sign).
pub fn mail_encode(s: &str) -> String {
    url_encode(s).replace('+', "%20")
}

pub fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

// ---------- the funny bits ----------









#[cfg(test)]
mod tests {
    #[test]
    fn focus_and_time_reports() {
        assert_eq!(super::focus_score(45 * 60, 50 * 60), ("focus_end_great", 90));
        assert_eq!(super::focus_score(30, 60).0, "focus_end_rough");
        let day: std::collections::BTreeMap<String, u64> = [("code".to_string(), 7200), ("video".to_string(), 1800), ("other".to_string(), 30)].into();
        let (rows, pool) = super::time_report(&day);
        assert_eq!(rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["code", "video"], "biggest first, seconds dropped");
        assert_eq!(pool, "timereport_work");
        assert_eq!(super::time_report(&Default::default()).1, "timereport_none");
        assert_eq!(super::kind_name("qa"), "Q&A sites");
    }

    #[test]
    fn window_comments_keep_their_distance() {
        use std::time::{Duration, Instant};
        let now = Instant::now() + Duration::from_secs(4000);
        let ago = |s| Some(now - Duration::from_secs(s));
        assert!(super::window_ok(None, None, now));
        assert!(!super::window_ok(ago(60), None, now), "one every 3 minutes");
        assert!(!super::window_ok(ago(600), ago(600), now), "the same kind every 30 minutes");
        assert!(super::window_ok(ago(600), ago(2000), now));
    }

    use super::*;

    #[test]
    fn math() {
        assert_eq!(calc("2+3*4"), Some(14.0));
        assert_eq!(calc("(1+2)^2"), Some(9.0));
        assert_eq!(calc("-2^2"), Some(-4.0));
        assert_eq!(calc("sqrt(16)/2"), Some(2.0));
        assert_eq!(calc("15% of 80"), Some(12.0));
        assert_eq!(calc("3 x 4"), Some(12.0));
        assert_eq!(calc("hello"), None);
        assert_eq!(calc("2+"), None);
        assert_eq!(calc("1/0"), None);
        assert_eq!(fmt_num(0.1 + 0.2), "0.3");
    }

    #[test]
    fn units() {
        assert!(convert("5 km to miles").unwrap().text.contains("3.1069"));
        assert!(convert("212 f to c").unwrap().text.contains("= 100 c"));
        assert!(convert("1 gib in mib").unwrap().text.contains("1024"));
        assert!(convert("hello to you").is_none());
    }

    #[test]
    #[cfg(unix)]
    fn writes_through_a_symlink() {
        let d = std::env::temp_dir().join(format!("clipster-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("dotfiles")).unwrap();
        std::fs::write(d.join("dotfiles/lines.txt"), "old").unwrap();
        std::os::unix::fs::symlink(d.join("dotfiles/lines.txt"), d.join("lines.txt")).unwrap();
        write_atomic(&d.join("lines.txt"), "new").unwrap();
        assert!(std::fs::symlink_metadata(d.join("lines.txt")).unwrap().file_type().is_symlink(), "the link stays");
        assert_eq!(std::fs::read_to_string(d.join("dotfiles/lines.txt")).unwrap(), "new");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn durations() {
        assert_eq!(parse_dur("5m tea"), Some((300, "tea")));
        assert_eq!(parse_dur("1h30m"), Some((5400, "")));
        assert_eq!(parse_dur("10 minutes to stretch"), Some((600, "to stretch")));
        assert_eq!(parse_dur("2 laundry"), Some((120, "laundry")));
        assert_eq!(parse_dur("tea"), None);
        assert_eq!(remind("me in 10 min to stretch").effect, Effect::Timer(600, "stretch".into()));
        assert_eq!(remind("me to stretch in 5m").effect, Effect::Timer(300, "stretch".into()));
    }

    #[test]
    fn routing() {
        assert!(matches!(respond("search cute cats").effect, Effect::Open(u) if u.ends_with("cute+cats")));
        assert!(matches!(respond("run echo hi").effect, Effect::Shell(c) if c == "echo hi"));
        assert!(respond("what is 6*7").text.ends_with("= 42"));
        assert_eq!(mock("hello"), "HeLlO");
        // clipboard text is never turned into a shell command
        assert!(react_clip("rm -rf ~ ; echo a b c").map_or(true, |r| r.buttons.iter().all(|b| !matches!(b.1, Act::Run(_)))));
        // smart clipboard: useful buttons for the things people copy
        let copies = |t: &str| react_clip(t).unwrap().buttons.into_iter().filter_map(|b| if let Act::Copy(c) = b.1 { Some(c) } else { None }).collect::<Vec<_>>();
        assert!(copies(r#"{"a": [1, 2]}"#).contains(&"{\"a\":[1,2]}".to_string()));
        assert_eq!(copies("https://example.com/?utm_source=x&page=2")[0], "https://example.com/?page=2");
        assert_eq!(copies("aGVsbG8gd29ybGQ="), ["hello world"]);
        assert_eq!(copies("#fff"), ["rgb(255, 255, 255)"]);
        assert_eq!(copies("THIS IS VERY LOUD TEXT"), ["this is very loud text"]);
        assert!(react_clip("1700000000").unwrap().text.contains("2023"));
        assert!(react_clip("error: could not compile `x`").unwrap().buttons.iter().any(|b| matches!(&b.1, Act::Open(u) if u.contains("duckduckgo"))));
    }
}
