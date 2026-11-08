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
    Exit,
    Net(crate::net::Query),
    ClipHistory,
    Stopwatch(&'static str), // "start", "stop" or "show"
    CopyNow(String),
    Repeat(u64, String), // a reminder that comes back every N seconds
    SetName(String),
    SayName,
    Hat(String),
    Skin(String),
    Autostart(bool),
    Costume(String, String), // hat, skin ("" = keep)
    Commands(Vec<String>),   // run each, like a macro
    SetBirthday(u32, u32),   // month, day
    Persona(bool),           // true = Unhinged
    Onboard(String),         // a step of the first-run setup: "", "name Sam", "persona boring", ...
    Memory(bool),            // true = list everything remembered so far
    Teach(String, String, Vec<String>), // phrase, what to say, what to run
    Unlearn(String),
    ListAchievements,
    Play(String),            // open an arcade game
    Remember(Vec<(String, String)>), // facts about the user: ("pet", "Mo"), ("pet_kind", "cat")
    Forget(Vec<String>),             // fact keys to drop; empty = all of them
    Checkin(String),                 // the daily "how are you?": great, okay, meh or rough
    InstallUpdate,                   // download the newest version, put it in place and restart
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

pub fn help() -> Reply {
    Reply::say(
        "Just talk to me normally, typos and all: \"remind me to call mom at 5pm\", \"how many cups in a gallon?\", \"open my downloads\", \"is my computer slow?\". I do a LOT (and none of it is AI, I promise):\n\
         • Math: 12*(3+4)^2, sqrt(2), 15% of 80\n\
         • Convert: 5 km to miles, 70 f to c, 2 gb to mib, 3 m to bananas\n\
         • Timers: timer 5m tea, remind me in 10 min to stretch, timers\n\
         • Notes: note buy milk, notes, forget notes\n\
         • Launch: open firefox, open ~/Downloads, open example.com\n\
         • Ask me anything: why is the sky blue?, who was Ada Lovelace?, define serendipity\n\
         • Live: weather in Rome, time in Tokyo, 100 dollars to euros, bitcoin price, news, tech news, on this day, next holiday, where is the ISS\n\
         • Words: another word for happy, what rhymes with orange, how do you spell definately\n\
         • Internet: is github down?, shorten <link>, what's my ip, show me a dog / cat / fox\n\
         • Web: search cats, youtube lofi, map paris\n\
         • Control: louder, mute, volume 30, skip this song, what's playing, make the screen darker, take a screenshot (of an area)\n\
         • Files: find the file called report, my latest download, clipboard history, empty the trash, qr code for <link>\n\
         • Time: days until christmas, what day is june 3, what date is in 90 days, start a stopwatch, pomodoro\n\
         • System: battery, stats, uptime, disk, ip, top, lock, restart / shut down / sleep, run <command>\n\
         • Text: count, upper, lower, reverse, mock <text>\n\
         • Random: roll 2d6, flip, pick pizza, tacos, password 20, 8ball <question>\n\
         • Fun: joke, pun, roast, fact, fortune, shower thought, quiz me, quote, advice, tip, dance, hug, shrug, tableflip, quiet, bye\n\
         • Arcade (own window): play paper jam, clip glide, desk merge, bug hunt, byte size, chess, hangman\n\
         • Games: riddle, would you rather, emoji quiz, rock paper scissors, guess my number, surprise me\n\
         • Teach me: when I say gm, say good morning · when I say work, open firefox and open code · what have you learned\n\
         • Numbers: 255 in hex, 2024 in roman, tip 15% on 48, split 120 between 4, days between june 3 and christmas, 1700000000 to date\n\
         • Text tools (or the clipboard): base64 hello, url encode, slug, sort lines, remove duplicates, sha256\n\
         • Powers: wifi off, bluetooth on, how much ram is firefox using, cpu temperature, biggest files in downloads, cache size, which distro, my gpu\n\
         • Discover: what should I cook, recipe for lasagne, make me a drink, recommend a book about space, tell me about the show severance, pikachu pokemon\n\
         • World: earthquakes, air quality, moon phase, random xkcd, cat fact, I'm bored, programming joke, chuck norris joke\n\
         • Text tricks: SOS in morse code, hello in binary, upside down hello, leet hacker\n\
         • Me: call me <name>, my birthday is june 3, do you like me, achievements, good morning (daily briefing), wear a wizard hat, turn gold, remind me every hour to drink water, start with my computer\n\n\
         I also watch what you copy (links, JSON, errors, base64, timestamps...) and which programs you open, and I comment. Constantly. Press Meta+C anywhere to call me, double-click me for a trick, and use the up arrow to repeat what you typed. There are also a few secrets hidden in here. 🥚",
    )
    .mood(Mood::Happy)
    .run("Tell me a joke", "joke")
    .run("How's my computer doing?", "stats")
    .btn("Options", Act::Options)
}

pub fn respond(input: &str) -> Reply {
    crate::understand::respond(input)
}

/// Strict commands ("joke", "timer 5m tea", "5 km to miles"). `None` = not a command I know.
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
    match head.as_str() {
        "" | "help" | "?" | "commands" | "menu" => return Some(help()),
        "hi" | "hello" | "hey" | "yo" | "sup" => return Some(greeting()),
        "joke" | "jokes" | "funny" => {
            // half from my own collection, half fresh from the internet
            let kind = ["dad", "dev", "chuck"].into_iter().find(|k| rest.contains(k)).unwrap_or("");
            if rest == "local" || kind.is_empty() && rand::thread_rng().gen_bool(0.4) {
                return Some(Reply::say(lines::pick("jokes")).mood(Mood::Laugh).run("Another one!", "joke").run("That was bad. Roast me instead", "roast"));
            }
            return Some(net(Query::Joke(kind.into()), pick(&["Hold on, fetching a fresh one from the internet...", "Let me find a good one... *flips through joke book*", "One joke, coming right up!"]).as_str()));
        }
        "roast" | "insult" => return Some(Reply::say(lines::pick("roasts")).mood(Mood::Sly).run("Ouch. Again!", "roast").run("Say something nice", "hug")),
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
        "checkin" => return Some(Reply::say("").effect(Effect::Checkin(rest.to_lowercase()))),
        "answer-to" => {
            let (id, text) = rest.split_once(' ').unwrap_or((rest, ""));
            return Some(crate::understand::answer_to(id, text));
        }
        // shows the comment for a window title, e.g. "window Untitled 1 - LibreOffice Writer" (for trying it out)
        "window" => return Some(lines::window_kind(rest).map(|k| react_window(&k)).unwrap_or_else(|| Reply::say("No comment on that window."))),
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
        "onboard" => return Some(Reply::say("").effect(Effect::Onboard(rest.into()))),
        "tour" | "tutorial" | "onboarding" => return Some(Reply::say("").effect(Effect::Onboard(String::new()))),
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
        "brightness" => return Some(crate::powers::brightness(rest)),
        "screenshot" => return Some(crate::powers::screenshot(rest)),
        "shutdown" | "reboot" | "restart" | "suspend" | "logout" => return Some(crate::powers::power(&format!("{head} {rest}"))),
        "trash" => return Some(crate::powers::trash()),
        "lastdownload" => return Some(crate::powers::latest_download()),
        "kaomoji" | "shrug" | "tableflip" | "lenny" => return Some(crate::powers::kaomoji(&format!("{head} {rest}"))),
        "uuid" | "guid" => return Some(crate::powers::uuid()),
        "lorem" => return Some(crate::powers::lorem()),
        "qr" => return Some(crate::powers::qr(&or_clipboard(rest))),
        "until" | "countdown" => return Some(crate::powers::until(rest)),
        "findfile" | "locate" => return Some(net(Query::Files(rest.into()), "Rummaging through your files... 🔍")),
        "cliphistory" => return Some(Reply::say("").effect(Effect::ClipHistory)),
        "stopwatch" => {
            let rest = rest.replace("stopwatch", "").replace("stop watch", "");
            let what = if rest.contains("stop") || rest.contains("end") || rest.contains("reset") { "stop" } else if rest.contains("start") || rest.contains("go") { "start" } else { "show" };
            return Some(Reply::say("").effect(Effect::Stopwatch(what)));
        }
        "pomodoro" | "focus" => return Some(timer("25m", "Focus session done! 🍅 Take a 5-minute break.").run("Start a 5-minute break", "timer 5m Break's over, back to work!")),
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
        "password" | "pass" | "pw" | "passwd" => return Some(password(rest)),
        "time" if !rest.is_empty() && !rest.starts_with("is") => return Some(net(Query::Place(Kind::Time, rest.into()), &format!("Checking the clocks in {rest}..."))),
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
        "timer" | "alarm" => return Some(timer(rest, "")),
        "remind" | "reminder" => return Some(remind(rest)),
        "timers" => return Some(Reply::say("").effect(Effect::ListTimers)),
        "cancel" | "stop" => return Some(Reply::say(lines::pick("say_cancel")).effect(Effect::CancelTimers)),
        "note" | "remember" | "todo" => return Some(note_add(rest)),
        "notes" | "todos" => return Some(notes_list()),
        // notes are only wiped after a yes, and only when it's clearly about them ("forget it" is not)
        "forget" if rest == "notes confirmed" => return Some(notes_clear()),
        "forget" if rest.is_empty() || ["notes", "my notes", "all notes", "all my notes", "the notes", "my list", "the list"].contains(&rest.to_lowercase().as_str()) => {
            return Some(notes_confirm());
        }
        "count" | "wc" => return Some(count(rest)),
        "upper" | "lower" | "reverse" | "mock" | "title" => return Some(transform(&head, rest)),
        "quiet" | "shh" | "shush" | "silence" | "mute" => return Some(quiet()),
        "bye" | "quit" | "exit" | "goodbye" => return Some(Reply::say(lines::pick("goodbye")).effect(Effect::Exit).mood(Mood::Spin)),
        "birthday" | "bday" => {
            use chrono::Datelike;
            let Some((date, _)) = crate::powers::parse_day(rest, chrono::Local::now().date_naive()) else {
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
            return Some(shell(r#"'{0:N0} MB of temporary files' -f ((Get-ChildItem $env:TEMP -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum/1MB)"#, "Your temp folder (programs make new files when they need them, so clearing is safe):").run("Clear temp files", r#"run Remove-Item "$env:TEMP\*" -Recurse -Force -ErrorAction SilentlyContinue; 'Temp files cleared! 🧹'"#));
        }
        "cache" => return Some(shell("du -sh ~/.cache 2>/dev/null; du -sh ~/.cache/* 2>/dev/null | sort -rh | head -n 6", "Your cache (programs rebuild it, so it's mostly safe to clear):").run("Clear thumbnails", "run rm -rf ~/.cache/thumbnails/* && echo 'Thumbnails cleared! 🧹'")),
        "options" | "settings" | "prefs" => return Some(Reply::say("Here's what you can change:").btn("Open options", Act::Options)),
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
        if let Some(d) = chrono::Local.timestamp_opt(secs, 0).single().filter(|_| (978_307_200..4_102_444_800).contains(&secs)) {
            let date = d.format("%A, %B %-d %Y, %H:%M").to_string();
            return Some(Reply::say(lines::pick("clip_timestamp").replace("{date}", &date)).mood(Mood::Think).btn("Copy the date", Act::Copy(date)));
        }
    }
    if one_line && t.len() < 80 && (t.chars().any(|c| "+*/^%×÷".contains(c)) || t.contains(" - ")) {
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

/// Window kinds that get a comment only after a long stretch (20 minutes), never right away.
pub const TIME_SINKS: &[&str] = &["video", "social"];

/// Window comments: at most one every 3 minutes, and the same kind at most every 30.
pub fn window_ok(last_any: Option<std::time::Instant>, last_same: Option<std::time::Instant>, now: std::time::Instant) -> bool {
    let since = |t: std::time::Instant, secs| now.duration_since(t) >= std::time::Duration::from_secs(secs);
    last_any.is_none_or(|t| since(t, 180)) && last_same.is_none_or(|t| since(t, 1800))
}

/// What to say about the window kind `kind` (a `## window <kind>` section).
pub fn react_window(kind: &str) -> Reply {
    lines::reply(&format!("window {kind}")).btn("Thanks, Clipster", Act::Hide)
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
        "do a barrel roll" | "barrel roll" => Reply::say("WHEEEE! 🌀 Press Z or R twice!").mood(Mood::Spin),
        "up up down down left right left right b a" | "konami code" | "konami" | "↑↑↓↓←→←→ba" => Reply::say("🎮 KONAMI CODE ACTIVATED! +30 lives. Rainbow mode unlocked. Crown equipped. I am now unstoppable.").mood(Mood::Party).effect(Effect::Costume("crown".into(), "rainbow".into())),
        "sudo make me a sandwich" => Reply::say("Okay. 🥪 (You said the magic word.)").mood(Mood::Happy),
        "make me a sandwich" => Reply::say("What? Make it yourself. (Try saying sudo.)").mood(Mood::Sly),
        "hello world" => Reply::say("Hello, programmer! 👋 Your first program ran successfully. Everything is downhill from here.").mood(Mood::Happy),
        "open the pod bay doors" | "open the pod bay doors hal" => Reply::say("I'm sorry, Dave. I'm afraid I can't do that. 🔴\n...Kidding! I can't open any doors. I'm a paperclip.").mood(Mood::Sly),
        "rick roll" | "rickroll" | "rick roll me" | "never gonna give you up" => Reply::say("You asked for it. 🕺").mood(Mood::Music).effect(Effect::Open("https://www.youtube.com/watch?v=dQw4w9WgXcQ".into())),
        "self destruct" | "self-destruct" | "self destruct sequence" | "initiate self destruct" => Reply::say("⚠ SELF-DESTRUCT IN 3... 2... 1...\n💥\n...\nJust kidding. I'm a paperclip. The worst I can do is poke you.").mood(Mood::Shock),
        "are you clippy" | "clippy" | "is your name clippy" | "you are clippy" => Reply::say("Clippy? Never heard of him. 👀 *nervous wire noises* I'm Clipster. Totally different paperclip. Completely unrelated. Please stop asking.").mood(Mood::Sly),
        "xyzzy" | "plugh" => Reply::say("Nothing happens. 🕳 (A hollow voice says \"fool\".)").mood(Mood::Think),
        "is the cake a lie" | "the cake is a lie" | "cake" => Reply::say("🎂 The cake is a lie. But the paperclip is real.").mood(Mood::Sly),
        "make coffee" | "make me coffee" | "brew coffee" | "coffee" => Reply::say("☕ Error 418: I'm a teapot. Er, paperclip. Either way: no coffee.").mood(Mood::Sly),
        "matrix" | "enter the matrix" | "red pill" | "take the red pill" => Reply::say("💊 Wake up... There is no paperclip.").mood(Mood::Cool).effect(Effect::Costume(String::new(), "neon".into())),
        "blue pill" | "take the blue pill" => Reply::say("💊 The story ends. You wake up in your bed and believe whatever you want to believe. Back to classic silver.").mood(Mood::Sly).effect(Effect::Costume(String::new(), "silver".into())),
        "party mode" | "party time" | "disco" | "lets party" | "let us party" => Reply::say("🎉 PARTY MODE! Unz unz unz unz!").mood(Mood::Party).effect(Effect::Costume("party".into(), "rainbow".into())),
        "it is my birthday" | "today is my birthday" | "my birthday" | "it is my bday" => Reply::say("🎂 HAPPY BIRTHDAY!!! 🎉 I got you a present: me, in a party hat. You're welcome.").mood(Mood::Party).effect(Effect::Costume("party".into(), String::new())),
        "sneeze" | "achoo" | "bless you" => Reply::say("ACHOO! 🤧 Sorry. Dusty desktop.").mood(Mood::Sneeze),
        "what does the fox say" => Reply::say("🦊 Ring-ding-ding-ding-dingeringeding! (Here's a fox, for science.)").mood(Mood::Music).effect(Effect::Open(format!("https://randomfox.ca/images/{}.jpg", rand::thread_rng().gen_range(1..=120)))),
        "sing" | "sing a song" | "sing me a song" | "sing for me" => Reply::say(lines::pick("songs")).mood(Mood::Music),
        "tell me a secret" | "secret" | "do you have a secret" => Reply::say(lines::pick("secrets")).mood(Mood::Wink),
        "beam me up" | "beam me up scotty" => Reply::say("🖖 Energizing! ...Hm. You're still here. My transporter is a USB port.").mood(Mood::Spin),
        "flip a table" => return None,
        "i love you clipster" | "marry me clipster" => Reply::say("💍 Oh! I... I need to ask the stapler first.").mood(Mood::Love),
        "who let the dogs out" => Reply::say("🐶 WHO? WHO? WHO? WHO?").mood(Mood::Music),
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

pub const LETTER: &str = "Dear Sir or Madam,\n\nI hope this letter finds you well, and not being followed by a paperclip.\n\n[Your important point here.]\n\nWarm regards,\n[Your name]";

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
    let mut p = Parser { c, i: 0 };
    let v = p.expr()?;
    (p.i == p.c.len() && v.is_finite()).then_some(v)
}

struct Parser {
    c: Vec<char>,
    i: usize,
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
    (found && total >= 1.0).then(|| (total as u64, s[end..].trim()))
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
    // no time given: keep it as a note instead of failing
    let what = r.trim_start_matches("to ").trim();
    if what.is_empty() {
        return Reply::say("Remind you of what, and when? Try: remind me in 10 min to stretch");
    }
    let mut n = note_add(what);
    n.text += "\n(You didn't say when, so I put it in your notes. Add a time like \"in 20 minutes\" or \"at 5pm\" for an alarm.)";
    n
}

fn random_number(rest: &str) -> Reply {
    let nums: Vec<i64> = rest.split(|c: char| !c.is_ascii_digit()).filter_map(|w| w.parse().ok()).collect();
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
        (h, m, _) => format!("{h}h {m}m"),
    }
}

// ---------- notes ----------

pub fn notes_path() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("clipster").join("notes.txt")
}

fn note_add(text: &str) -> Reply {
    if text.is_empty() {
        return Reply::say("Note what? Try: note call grandma");
    }
    let p = notes_path();
    let ok = std::fs::create_dir_all(p.parent().unwrap()).is_ok()
        && std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)
            .and_then(|mut f| std::io::Write::write_all(&mut f, format!("{text}\n").as_bytes()))
            .is_ok();
    if ok {
        Reply::say(lines::pick("say_note").replace("{note}", text)).mood(Mood::Happy).run("Show all notes", "notes")
    } else {
        Reply::say("Oops, I couldn't save that note. My filing cabinet is stuck.").mood(Mood::Shock)
    }
}

fn notes_list() -> Reply {
    let text = std::fs::read_to_string(notes_path()).unwrap_or_default();
    if text.trim().is_empty() {
        return Reply::say("No notes yet. Try: note buy more paperclips");
    }
    let list: String = text.lines().enumerate().map(|(i, l)| format!("{}. {l}\n", i + 1)).collect();
    Reply::say(format!("Your notes:\n{list}"))
        .btn("Open notes file", Act::Open(notes_path().to_string_lossy().into_owned()))
        .btn("Copy all", Act::Copy(text))
        .run("Forget them all", "forget notes")
}

fn notes_confirm() -> Reply {
    let n = std::fs::read_to_string(notes_path()).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).count();
    if n == 0 {
        return Reply::say("There are no notes to forget. Clean slate already.");
    }
    Reply::say(format!("Forget all {n} note(s)? There's no undo, not even for me.")).mood(Mood::Shock).run("Yes, forget them", "forget notes confirmed").btn("Keep them", Act::Hide)
}

fn notes_clear() -> Reply {
    let _ = std::fs::remove_file(notes_path());
    Reply::say("Poof! All notes forgotten. What notes? I don't know what you're talking about.").mood(Mood::Sly)
}

// ---------- text ----------

fn count(t: &str) -> Reply {
    let t = &or_clipboard(t);
    if t.is_empty() {
        return Reply::say("Count what? Try: count the quick brown fox");
    }
    Reply::say(format!(
        "{} words, {} characters, {} lines. Riveting stuff.",
        t.split_whitespace().count(),
        t.chars().count(),
        t.lines().count()
    ))
    .mood(Mood::Think)
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
    Reply::say(format!("It's {}. {}", now.format("%H:%M on %A, %B %-d, %Y"), pick(&[
        "Time flies when you're with a paperclip.",
        "Shouldn't you be doing something?",
        "Perfect time for a snack.",
    ])))
    .mood(Mood::Think)
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

fn expand_home(p: &str) -> PathBuf {
    match p.strip_prefix("~") {
        Some(r) => dirs::home_dir().unwrap_or_default().join(r.trim_start_matches('/')),
        None => PathBuf::from(p),
    }
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

fn web(base: &str, q: &str, intro: &str) -> Reply {
    Reply::say(format!("{intro}{}...", if q.is_empty() { String::new() } else { format!(" for \"{q}\"") }))
        .effect(Effect::Open(format!("{base}{}", url_encode(q))))
        .mood(Mood::Happy)
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
