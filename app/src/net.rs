//! Answers from free, keyless web services (no AI anywhere): encyclopedia, dictionary, weather, money, news...
//! Runs on a background thread; every answer falls back gracefully when the internet is away.

use crate::brain::{url_encode, Act, Effect, Mood, Reply};
use crate::lines;
use rand::seq::SliceRandom;
use rand::Rng;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Weather,
    Time,
    Sun,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Query {
    Ask(String),
    Wiki(String),
    Define(String),
    Place(Kind, String),
    Money(f64, String, String),
    Crypto(String, String),
    Joke(String), // "dad", "dev", "chuck" or "" for a surprise
    Quakes,
    Air(String),
    Book(String),
    Show(String),
    Pokemon(String),
    Recipe(String), // "" = surprise me
    Cocktail,
    CatFact,
    Bored,
    Xkcd(bool), // true = random comic, false = newest
    Affirm,
    Kanye,
    Fact,
    Quote,
    Advice,
    News(bool), // true = tech news
    OnThisDay,
    MyIp,
    Iss,
    Trivia,
    Holiday,
    IsDown(String),
    Shorten(String),
    Words(&'static str, String),
    Spell(String),
    Animal(String),
    Files(String),
    Recent, // files opened recently
    RandomWiki,
    Briefing,
    Update, // is there a newer Clipster?
    Dupes(String),    // duplicate files in a folder ("downloads", "desktop")
    OldFiles(String), // big files nobody touched for months
}

pub fn answer(q: &Query) -> Reply {
    fetch(q).unwrap_or_else(|e| offline(q, &e))
}

fn offline(q: &Query, e: &str) -> Reply {
    match q {
        Query::Affirm => return Reply::say(lines::pick("nice")).mood(Mood::Love),
        Query::Joke(_) => return Reply::say(lines::pick("jokes")).mood(Mood::Laugh).run("Another one!", "joke"),
        Query::Fact => return Reply::say(format!("Fun fact: {}", lines::pick("facts"))).mood(Mood::Think).run("Another fact", "fact"),
        _ => {}
    }
    if let (Query::Ask(t), "nothing") = (q, e) {
        let lower = t.to_lowercase();
        if lower.starts_with("how ") || lower.contains("how to") {
            return Reply::say("That's a how-to question! My encyclopedia is bad at those, but the web is great at them:")
                .mood(Mood::Think)
                .btn("Search the web for it", Act::Open(format!("https://duckduckgo.com/?q={}", url_encode(t))))
                .btn("Look for a step-by-step guide", Act::Open(format!("https://www.wikihow.com/wikiHowTo?search={}", url_encode(t))));
        }
    }
    let text = if e.contains("404") || e == "nothing" {
        "I looked everywhere, but found nothing for that. Not even in the paper tray.".to_string()
    } else {
        format!("{} ({e})", lines::pick("offline"))
    };
    let mut r = Reply::say(text).mood(Mood::Think);
    if let Query::Ask(t) | Query::Wiki(t) | Query::Define(t) | Query::Words(_, t) | Query::Spell(t) = q {
        r = r.btn("Search the web instead", Act::Open(format!("https://duckduckgo.com/?q={}", url_encode(t))));
    }
    r.run("Show me what you can do", "help")
}

/// Where new versions are published.
pub const RELEASES: &str = "https://github.com/NilsBriggen/clipster/releases/latest";

/// The newest released version ("0.2.0"), from GitHub's list of releases.
pub fn latest_version() -> Result<String, String> {
    let v = get("https://api.github.com/repos/NilsBriggen/clipster/releases/latest")?;
    let tag = s(&v["tag_name"]);
    let tag = tag.trim_start_matches('v');
    if tag.is_empty() { Err("nothing".into()) } else { Ok(tag.to_string()) }
}

/// Is version `a` newer than `b`? Numbers compare part by part, so 0.10.0 beats 0.9.1.
pub fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| v.split('.').map(|p| p.trim().parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    parts(a) > parts(b)
}

/// "Version X is out", with "Install now" where he can replace himself and a link to the download page.
pub fn update_reply(version: &str) -> Reply {
    let r = Reply::say(lines::pick("update_available").replace("{version}", version).replace("{current}", env!("CARGO_PKG_VERSION"))).mood(Mood::Party);
    let r = if can_self_update() { r.run("Install now", "install update") } else { r };
    r.btn(&format!("Get version {version}"), Act::Open(RELEASES.into())).btn("Not now", Act::Hide)
}

/// He can install an update himself when he runs as an AppImage (he swaps the file) or on Windows (he runs the new installer).
pub fn can_self_update() -> bool {
    cfg!(windows) || std::env::var_os("APPIMAGE").is_some()
}

/// A release file: where to get it, how big it is and its SHA-256 (GitHub's `digest`).
#[derive(Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

/// The release's file ending in `ext` (".AppImage" or ".msi"), only from GitHub and only with a size and a checksum.
fn pick_asset(release: &Value, ext: &str) -> Option<Asset> {
    let a = release["assets"].as_array()?.iter().find(|a| s(&a["name"]).ends_with(ext))?;
    let asset = Asset { name: s(&a["name"]), url: s(&a["browser_download_url"]), size: a["size"].as_u64().unwrap_or(0), sha256: s(&a["digest"]).trim_start_matches("sha256:").to_lowercase() };
    (asset.url.starts_with("https://github.com/") && asset.size > 0 && asset.sha256.len() == 64).then_some(asset)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Download `a` into `dest`, checking its size and SHA-256 on the way. A bad download is deleted.
fn download_to(a: &Asset, dest: &std::path::Path) -> Result<(), String> {
    use std::io::{Read, Write};
    // its own timeout: the 10 s of `agent()` is far too short for a 12 MB file, and read_to_vec stops at 10 MB
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(300))).user_agent(concat!("Clipster/", env!("CARGO_PKG_VERSION"), " (desktop helper)")).build().into();
    let mut res = agent.get(&a.url).call().map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let (mut reader, mut sha, mut total, mut buf) = (res.body_mut().as_reader(), ring::digest::Context::new(&ring::digest::SHA256), 0u64, vec![0u8; 64 * 1024]);
    let copied: Result<(), String> = loop {
        match reader.read(&mut buf) {
            Ok(0) => break Ok(()),
            Ok(n) => {
                sha.update(&buf[..n]);
                total += n as u64;
                if let Err(e) = file.write_all(&buf[..n]) {
                    break Err(e.to_string());
                }
            }
            Err(e) => break Err(e.to_string()),
        }
    };
    let sum = hex(sha.finish().as_ref());
    let checked = copied.and_then(|_| if total == a.size && sum == a.sha256 { Ok(()) } else { Err("the download didn't match its checksum".into()) });
    if checked.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    checked
}

/// Fetch and check the newest release for this computer, and put it in place: the AppImage file is swapped
/// (atomically, next to the old one), the Windows installer is saved to the temp folder. Returns (what to run, version).
pub fn download_update() -> Result<(std::path::PathBuf, String), String> {
    let release = get("https://api.github.com/repos/NilsBriggen/clipster/releases/latest")?;
    let version = s(&release["tag_name"]).trim_start_matches('v').to_string();
    let a = pick_asset(&release, if cfg!(windows) { ".msi" } else { ".AppImage" }).ok_or("the release has no file for this computer")?;
    if cfg!(windows) {
        let dest = std::env::temp_dir().join(&a.name);
        download_to(&a, &dest)?;
        return Ok((dest, version));
    }
    let image = std::path::PathBuf::from(std::env::var_os("APPIMAGE").ok_or("not running as an AppImage")?);
    let mut part = image.clone().into_os_string();
    part.push(".new");
    let part = std::path::PathBuf::from(part);
    download_to(&a, &part)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&part, &image).map_err(|e| e.to_string())?;
    Ok((image, version))
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .user_agent(concat!("Clipster/", env!("CARGO_PKG_VERSION"), " (desktop helper)"))
        .build()
        .into()
}

fn get(url: &str) -> Result<Value, String> {
    let mut res = agent().get(url).header("Accept", "application/json").call().map_err(|e| e.to_string())?;
    res.body_mut().read_json::<Value>().map_err(|e| e.to_string())
}

/// A calendar feed: iCal accepted, and up to 50 MB (years of events add up).
pub(crate) fn get_calendar(url: &str) -> Result<String, String> {
    let mut res = agent().get(url).header("Accept", "text/calendar, text/plain;q=0.9, */*;q=0.5").call().map_err(|e| e.to_string())?;
    res.body_mut().with_config().limit(50 * 1024 * 1024).read_to_string().map_err(|e| e.to_string())
}

pub(crate) fn get_text(url: &str) -> Result<String, String> {
    let mut res = agent().get(url).header("Accept", "text/plain").call().map_err(|e| e.to_string())?;
    res.body_mut().read_to_string().map_err(|e| e.to_string())
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").trim().to_string()
}

fn f(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_str().and_then(|x| x.parse().ok())).unwrap_or(f64::NAN)
}

/// 68123.456 -> "68,123.46"
pub fn money_fmt(v: f64) -> String {
    let neg = v < 0.0;
    let s = format!("{:.2}", v.abs());
    let (int, dec) = s.split_once('.').unwrap_or((&s, "00"));
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{}{out}.{dec}", if neg { "-" } else { "" })
}

/// "%22Hello%22" -> "\"Hello\""
pub fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        // two hex digits after a %, read from the bytes (a letter like "é" after the % isn't a char boundary)
        if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            out.push(hex(b[i + 1]) * 16 + hex(b[i + 2]));
            i += 3;
            continue;
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ")
}

const STOP: &[&str] = &[
    "how", "what", "why", "who", "whom", "when", "where", "which", "the", "and", "for", "are", "was", "were", "does", "did", "can",
    "could", "would", "should", "will", "you", "your", "mine", "our", "this", "that", "with", "from", "into", "about", "there",
    "their", "have", "has", "had", "its", "get", "make", "use", "way", "best", "much", "many", "some", "any", "tell", "is", "of",
    "to", "in", "on", "do", "an", "my", "me", "it", "at", "by", "or", "as", "be", "we", "us", "so", "up", "if", "no", "a", "i",
];

/// Does a search hit's title share a real word with the question? Stops "unplug my pc" from answering with a TV show.
pub fn relevant(title: &str, question: &str) -> bool {
    let t = title.to_lowercase();
    let words: Vec<String> = question.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty() && !STOP.contains(w)).map(String::from).collect();
    // compare word starts, so "unplug" also finds "Unplugging"
    words.is_empty() || words.iter().any(|w| t.contains(&w.chars().take(5).collect::<String>()))
}

/// A "may refer to:" page: a list of meanings, useless as an answer.
fn disambiguation(page: &Value) -> bool {
    let text = s(&page["extract"]);
    !page["pageprops"]["disambiguation"].is_null() || text.trim_end().ends_with("refer to:") || text.trim_end().ends_with("refers to:")
}

fn wiki(q: &str) -> Result<Reply, String> {
    let v = get(&format!(
        "https://en.wikipedia.org/w/api.php?action=query&generator=search&gsrsearch={}&gsrlimit=6&prop=extracts%7Cpageprops&ppprop=disambiguation&exintro=1&explaintext=1&exsentences=3&exlimit=6&redirects=1&format=json&formatversion=2",
        url_encode(q)
    ))?;
    let mut pages: Vec<&Value> = v["query"]["pages"].as_array().into_iter().flatten().collect();
    pages.sort_by_key(|p| p["index"].as_i64().unwrap_or(99)); // search rank
    let web = Act::Open(format!("https://duckduckgo.com/?q={}", url_encode(q)));
    let Some(page) = pages.iter().find(|p| !s(&p["extract"]).is_empty() && !disambiguation(p) && relevant(&s(&p["title"]), q)) else {
        // nothing clear-cut: offer the likeliest meanings instead of showing a "may refer to:" page
        let options: Vec<String> = pages.iter().filter(|p| !disambiguation(p) && !s(&p["extract"]).is_empty()).map(|p| s(&p["title"])).take(3).collect();
        if options.is_empty() {
            return Err("nothing".into());
        }
        let mut r = Reply::say(format!("\"{q}\" could mean a few things. Which one did you mean?")).mood(Mood::Think);
        for t in options {
            r = r.run(&t, &format!("wiki {t}"));
        }
        return Ok(r.btn("Search the web instead", web));
    };
    let (title, text) = (s(&page["title"]), s(&page["extract"]));
    Ok(Reply::say(format!("{title}: {text}"))
        .mood(Mood::Think)
        .btn("Read the whole article", Act::Open(format!("https://en.wikipedia.org/wiki/{}", url_encode(&title.replace(' ', "_")))))
        .btn("Search the web too", web))
}

/// No place given: the city the user said they live in, if they did ("here" still means where this computer is).
fn home_or<'a>(place: &'a str, home: Option<&'a str>) -> &'a str {
    if place.is_empty() { home.unwrap_or("") } else { place }
}

/// "" = wherever this computer is (by its internet address), else a place name.
fn locate(place: &str) -> Result<(String, f64, f64), String> {
    let home = lines::fact("city");
    let place = home_or(place.split(',').next().unwrap_or("").trim(), home.as_deref());
    if place.is_empty() || ["here", "my area", "my location", "where i am"].contains(&place) {
        let v = get("https://ipinfo.io/json")?;
        let (lat, lon) = s(&v["loc"]).split_once(',').map(|(a, b)| (a.parse().unwrap_or(0.0), b.parse().unwrap_or(0.0))).ok_or("no location")?;
        return Ok((s(&v["city"]), lat, lon));
    }
    let v = get(&format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json", url_encode(place)))?;
    let r = &v["results"][0];
    if r.is_null() {
        return Err("nothing".into());
    }
    Ok((format!("{}, {}", s(&r["name"]), s(&r["country"])), f(&r["latitude"]), f(&r["longitude"])))
}

fn sky(code: i64) -> (&'static str, &'static str) {
    match code {
        0 => ("☀", "Clear sky"),
        1 | 2 => ("⛅", "Partly cloudy"),
        3 => ("☁", "Cloudy"),
        45 | 48 => ("🌫", "Foggy"),
        51..=57 => ("🌦", "Drizzle"),
        61..=67 => ("🌧", "Rain"),
        71..=77 => ("❄", "Snow"),
        80..=82 => ("🌧", "Rain showers"),
        85 | 86 => ("🌨", "Snow showers"),
        95..=99 => ("⛈", "Thunderstorms"),
        _ => ("🌡", "Weather"),
    }
}

fn place(kind: Kind, where_: &str) -> Result<Reply, String> {
    let (name, lat, lon) = locate(where_)?;
    let v = get(&format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,wind_speed_10m&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,sunrise,sunset&timezone=auto&forecast_days=2"
    ))?;
    let (c, d) = (&v["current"], &v["daily"]);
    let hm = |t: &Value| s(t).get(11..16).unwrap_or("?").to_string();
    match kind {
        Kind::Time => {
            let local = s(&c["time"]);
            let day = chrono::NaiveDateTime::parse_from_str(&local, "%Y-%m-%dT%H:%M").map(|t| t.format("%A").to_string()).unwrap_or_default();
            let mine = chrono::Local::now().offset().local_minus_utc() as f64;
            let diff = (v["utc_offset_seconds"].as_f64().unwrap_or(mine) - mine) / 3600.0;
            let rel = if diff.abs() < 0.01 { "Same time as you!".to_string() } else { format!("That's {} hours {} you.", crate::brain::fmt_num(diff.abs()), if diff > 0.0 { "ahead of" } else { "behind" }) };
            Ok(Reply::say(format!("🕐 It's {} on {day} in {name}. {rel}", hm(&c["time"]))).mood(Mood::Think))
        }
        Kind::Sun => Ok(Reply::say(format!("🌅 In {name} the sun rises at {} and sets at {} today. Tomorrow: {} / {}.", hm(&d["sunrise"][0]), hm(&d["sunset"][0]), hm(&d["sunrise"][1]), hm(&d["sunset"][1]))).mood(Mood::Think)),
        Kind::Weather => {
            let t = f(&c["temperature_2m"]);
            let (icon, desc) = sky(c["weather_code"].as_i64().unwrap_or(-1));
            let (_, desc2) = sky(d["weather_code"][1].as_i64().unwrap_or(-1));
            let quip = match t {
                t if t < 0.0 => "Brrr! Even my wire is shivering.",
                t if t < 10.0 => "Jacket weather. Maybe two jackets.",
                t if t < 20.0 => "Pleasant! Perfect paperclip weather.",
                t if t < 28.0 => "Warm! Go outside. I'll hold the fort.",
                _ => "Hot hot hot! Don't leave your laptop in the sun.",
            };
            let text = format!(
                "{icon} {desc} in {name}: {t:.0}°C ({:.0}°F), feels like {:.0}°C.\nWind {:.0} km/h, humidity {:.0}%.\nToday: {:.0} to {:.0}°C, {:.0}% chance of rain.\nTomorrow: {desc2}, {:.0} to {:.0}°C, {:.0}% chance of rain.\n{quip}",
                t * 9.0 / 5.0 + 32.0, f(&c["apparent_temperature"]), f(&c["wind_speed_10m"]), f(&c["relative_humidity_2m"]),
                f(&d["temperature_2m_min"][0]), f(&d["temperature_2m_max"][0]), f(&d["precipitation_probability_max"][0]),
                f(&d["temperature_2m_min"][1]), f(&d["temperature_2m_max"][1]), f(&d["precipitation_probability_max"][1]),
            );
            Ok(Reply::say(text).mood(Mood::Happy).btn("Open the full forecast", Act::Open(format!("https://wttr.in/{}", url_encode(where_)))))
        }
    }
}

fn fetch(q: &Query) -> Result<Reply, String> {
    match q {
        Query::Ask(question) => {
            let v = get(&format!("https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1&no_redirect=1", url_encode(question)))?;
            for k in ["Answer", "AbstractText", "Definition"] {
                let t = s(&v[k]);
                if !t.is_empty() && (k != "AbstractText" || relevant(&s(&v["Heading"]), question)) {
                    let src = [s(&v["AbstractURL"]), s(&v["DefinitionURL"])].into_iter().find(|u| !u.is_empty());
                    let r = Reply::say(t).mood(Mood::Think);
                    return Ok(match src {
                        Some(u) => r.btn("Read more", Act::Open(u)),
                        None => r,
                    });
                }
            }
            wiki(question)
        }
        Query::Wiki(t) => wiki(t),
        Query::Define(w) => {
            let v = get(&format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", url_encode(w)))?;
            let e = &v[0];
            let mut text = format!("📖 {}{}\n", s(&e["word"]), if s(&e["phonetic"]).is_empty() { String::new() } else { format!("  {}", s(&e["phonetic"])) });
            for m in e["meanings"].as_array().into_iter().flatten().take(3) {
                text += &format!("• ({}) {}\n", s(&m["partOfSpeech"]), s(&m["definitions"][0]["definition"]));
            }
            Ok(Reply::say(text.trim_end().to_string()).mood(Mood::Think).run(&format!("Other words for \"{w}\""), &format!("synonym {w}")))
        }
        Query::Place(kind, p) => place(*kind, p),
        Query::Money(amount, from, to) => {
            let v = get(&format!("https://api.frankfurter.dev/v1/latest?amount={amount}&from={from}&to={to}"))?;
            let x = f(&v["rates"][to.as_str()]);
            if x.is_nan() {
                return Err("nothing".into());
            }
            let out = money_fmt(x);
            Ok(Reply::say(format!("💱 {} {from} = {out} {to}\n(Official European Central Bank rate from {}.)", money_fmt(*amount), s(&v["date"])))
                .mood(Mood::Think)
                .btn("Copy amount", Act::Copy(out))
                .run("And the other way round?", &format!("money {amount} {to} {from}")))
        }
        Query::Crypto(coin, vs) => {
            let v = get(&format!("https://api.coingecko.com/api/v3/simple/price?ids={coin}&vs_currencies={vs}&include_24hr_change=true"))?;
            let price = f(&v[coin.as_str()][vs.as_str()]);
            if price.is_nan() {
                return Err("nothing".into());
            }
            let ch = f(&v[coin.as_str()][format!("{vs}_24h_change")]);
            let trend = if ch >= 0.0 { format!("📈 +{ch:.1}% today. To the moon!") } else { format!("📉 {ch:.1}% today. Hold on tight.") };
            Ok(Reply::say(format!("1 {} = {} {}\n{trend}", coin[..1].to_uppercase() + &coin[1..], money_fmt(price), vs.to_uppercase())).mood(Mood::Sly))
        }
        Query::Joke(kind) => {
            let kind = if kind.is_empty() { *["dad", "dev", "chuck"].choose(&mut rand::thread_rng()).unwrap_or(&"dad") } else { kind.as_str() };
            let joke = match kind {
                "dev" => s(&get("https://v2.jokeapi.dev/joke/Programming?safe-mode&type=single")?["joke"]),
                "chuck" => {
                    // only the family-friendly categories
                    let cat = ["dev", "science", "food", "animal", "sport", "movie", "career", "travel"].choose(&mut rand::thread_rng()).unwrap_or(&"dev");
                    s(&get(&format!("https://api.chucknorris.io/jokes/random?category={cat}"))?["value"])
                }
                _ => get_text("https://icanhazdadjoke.com/")?.trim().to_string(),
            };
            Ok(Reply::say(joke).mood(Mood::Laugh).run("Another one!", "joke").run("A programming joke", "joke dev").run("A Chuck Norris joke", "joke chuck"))
        }
        Query::Quakes => {
            let v = get("https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/4.5_day.geojson")?;
            let mut quakes: Vec<&Value> = v["features"].as_array().into_iter().flatten().collect();
            quakes.sort_by(|a, b| f(&b["properties"]["mag"]).total_cmp(&f(&a["properties"]["mag"])));
            let now = chrono::Utc::now().timestamp_millis() as f64;
            let lines: Vec<String> = quakes.iter().take(4).map(|q| {
                let p = &q["properties"];
                format!("• M{:.1} — {} ({} ago)", f(&p["mag"]), s(&p["place"]), crate::brain::fmt_dur(((now - f(&p["time"])) / 1000.0).max(0.0) as u64))
            }).collect();
            if lines.is_empty() {
                return Ok(Reply::say("No big earthquakes in the last day. The planet is chilling. 🌍").mood(Mood::Happy));
            }
            Ok(Reply::say(format!("🌍 Biggest earthquakes in the last 24 hours:\n{}", lines.join("\n"))).mood(Mood::Shock).btn("Open the live map", Act::Open("https://earthquake.usgs.gov/earthquakes/map/".into())))
        }
        Query::Air(where_) => {
            let (name, lat, lon) = locate(where_)?;
            let v = get(&format!("https://air-quality-api.open-meteo.com/v1/air-quality?latitude={lat}&longitude={lon}&current=european_aqi,pm2_5,pm10,uv_index"))?;
            let c = &v["current"];
            let aqi = f(&c["european_aqi"]);
            let verdict = match aqi as i64 {
                0..=20 => "Excellent! Breathe deep. 🌿",
                21..=40 => "Good. Go outside, it's allowed.",
                41..=60 => "Moderate. Fine for most people.",
                61..=80 => "Poor. Maybe skip the marathon today.",
                _ => "Bad! Keep the windows closed. 😷",
            };
            let uv = f(&c["uv_index"]);
            let sun = if uv >= 6.0 { "Wear sunscreen!" } else if uv >= 3.0 { "A hat wouldn't hurt." } else { "No sunburn risk." };
            Ok(Reply::say(format!("🌬 Air in {name}: quality index {aqi:.0} — {verdict}\nFine dust: {:.0} µg/m³ (PM2.5), {:.0} (PM10). UV index {uv:.1}: {sun}", f(&c["pm2_5"]), f(&c["pm10"]))).mood(Mood::Think))
        }
        Query::Book(q) => {
            let v = get(&format!("https://openlibrary.org/search.json?q={}&limit=4&fields=title,author_name,first_publish_year,key", url_encode(q)))?;
            let docs: Vec<&Value> = v["docs"].as_array().into_iter().flatten().collect();
            if docs.is_empty() {
                return Err("nothing".into());
            }
            let mut r = Reply::say(format!("📚 Books about \"{q}\" (click one to see it):")).mood(Mood::Think);
            for d in docs {
                let label = format!("{} — {} ({})", s(&d["title"]), s(&d["author_name"][0]), d["first_publish_year"].as_i64().map_or("?".into(), |y| y.to_string()));
                r = r.btn(&label, Act::Open(format!("https://openlibrary.org{}", s(&d["key"]))));
            }
            Ok(r)
        }
        Query::Show(q) => {
            let v = get(&format!("https://api.tvmaze.com/singlesearch/shows?q={}", url_encode(q)))?;
            let summary = strip_tags(&s(&v["summary"]));
            let short: String = summary.split_inclusive(". ").take(2).collect();
            let genres: Vec<String> = v["genres"].as_array().into_iter().flatten().map(s).collect();
            let rating = f(&v["rating"]["average"]);
            let stars = if rating.is_nan() { "not rated yet".to_string() } else { format!("rated {rating}/10") };
            Ok(Reply::say(format!(
                "📺 {} ({}, {}) — {}, {stars}.\n{}\n{}",
                s(&v["name"]), s(&v["premiered"]).get(..4).unwrap_or("?"), s(&v["status"]).to_lowercase(), genres.join(", "), short.trim(),
                if rating >= 8.0 { "Paperclip verdict: binge it." } else { "Paperclip verdict: watch with snacks." }
            ))
            .mood(Mood::Happy)
            .btn("More about the show", Act::Open(s(&v["url"]))))
        }
        Query::Pokemon(name) => {
            let v = get(&format!("https://pokeapi.co/api/v2/pokemon/{}", url_encode(&name.to_lowercase().replace(' ', "-"))))?;
            let types: Vec<String> = v["types"].as_array().into_iter().flatten().map(|t| s(&t["type"]["name"])).collect();
            let abilities: Vec<String> = v["abilities"].as_array().into_iter().flatten().map(|a| s(&a["ability"]["name"])).collect();
            Ok(Reply::say(format!(
                "⚡ #{} {}: {} type, {:.1} m tall, {:.1} kg. Abilities: {}.\nIt's no paperclip, but it's pretty cool.",
                v["id"], s(&v["name"]).to_uppercase(), types.join("/"), f(&v["height"]) / 10.0, f(&v["weight"]) / 10.0, abilities.join(", ")
            ))
            .mood(Mood::Spin)
            .btn("Show me what it looks like", Act::Open(s(&v["sprites"]["other"]["official-artwork"]["front_default"]))))
        }
        Query::Recipe(q) => {
            let url = if q.is_empty() { "https://www.themealdb.com/api/json/v1/1/random.php".to_string() } else { format!("https://www.themealdb.com/api/json/v1/1/search.php?s={}", url_encode(q)) };
            let v = get(&url)?;
            let m = &v["meals"][0];
            if m.is_null() {
                return Err("nothing".into());
            }
            let ingredients: Vec<String> = (1..=20)
                .filter_map(|i| {
                    let ing = s(&m[format!("strIngredient{i}")]);
                    (!ing.is_empty()).then(|| format!("{} {ing}", s(&m[format!("strMeasure{i}")])).trim().to_string())
                })
                .collect();
            let steps: String = s(&m["strInstructions"]).chars().take(350).collect();
            let link = [s(&m["strSource"]), s(&m["strYoutube"])].into_iter().find(|u| !u.is_empty()).unwrap_or(format!("https://www.themealdb.com/meal/{}", s(&m["idMeal"])));
            Ok(Reply::say(format!("🍳 {} ({} {})\nYou need: {}\n{steps}…", s(&m["strMeal"]), s(&m["strArea"]), s(&m["strCategory"]).to_lowercase(), ingredients.join(", ")))
                .mood(Mood::Happy)
                .btn("Open the full recipe", Act::Open(link))
                .run("Something else!", "recipe"))
        }
        Query::Cocktail => {
            let d = &get("https://www.thecocktaildb.com/api/json/v1/1/random.php")?["drinks"][0];
            let ingredients: Vec<String> = (1..=15)
                .filter_map(|i| {
                    let ing = s(&d[format!("strIngredient{i}")]);
                    (!ing.is_empty()).then(|| format!("{} {ing}", s(&d[format!("strMeasure{i}")])).trim().to_string())
                })
                .collect();
            Ok(Reply::say(format!("🍹 How about a {}? ({})\nYou need: {}\n{}\n(Paperclips can't drink, so have one for me. Responsibly!)", s(&d["strDrink"]), s(&d["strGlass"]).to_lowercase(), ingredients.join(", "), s(&d["strInstructions"])))
                .mood(Mood::Music)
                .run("Another drink idea", "cocktail"))
        }
        Query::CatFact => Ok(Reply::say(format!("🐱 Cat fact: {}", s(&get("https://catfact.ninja/fact")?["fact"]))).mood(Mood::Think).run("Another cat fact", "catfact").run("Show me a cat", "cat")),
        Query::Bored => {
            let v = get("https://bored-api.appbrewery.com/random")?;
            let who = match v["participants"].as_i64().unwrap_or(1) { 1 => "just you".to_string(), n => format!("{n} people") };
            Ok(Reply::say(format!("Bored? Try this: {}! ({} activity, for {who})", s(&v["activity"]), s(&v["type"]))).mood(Mood::Happy).run("Give me another idea", "bored").run("Let's play a game instead", "games"))
        }
        Query::Xkcd(random) => {
            let latest = get("https://xkcd.com/info.0.json")?;
            let v = if *random {
                let n = rand::thread_rng().gen_range(1..=latest["num"].as_i64().unwrap_or(3000));
                get(&format!("https://xkcd.com/{n}/info.0.json"))?
            } else {
                latest
            };
            Ok(Reply::say(format!("🖍 xkcd #{}: \"{}\"\nHidden joke: {}", v["num"], s(&v["safe_title"]), s(&v["alt"])))
                .mood(Mood::Laugh)
                .btn("Show me the comic", Act::Open(format!("https://xkcd.com/{}/", v["num"])))
                .run("A random one!", "xkcd random"))
        }
        Query::Affirm => Ok(Reply::say(format!("💛 {}", s(&get("https://www.affirmations.dev/")?["affirmation"]))).mood(Mood::Love).run("Another one, please", "affirm")),
        Query::Kanye => Ok(Reply::say(format!("🎤 A wise man once said: \"{}\"\n— Kanye. (I don't make the rules.)", s(&get("https://api.kanye.rest")?["quote"]))).mood(Mood::Cool).run("More wisdom", "kanye")),
        Query::Fact => {
            let v = get("https://uselessfacts.jsph.pl/api/v2/facts/random?language=en")?;
            Ok(Reply::say(format!("Fun fact: {}", s(&v["text"]))).mood(Mood::Think).run("Another fact", "fact"))
        }
        Query::Quote => {
            let (q, a) = match get("https://dummyjson.com/quotes/random") {
                Ok(v) => (s(&v["quote"]), s(&v["author"])),
                Err(_) => get("https://zenquotes.io/api/random").map(|v| (s(&v[0]["q"]), s(&v[0]["a"])))?,
            };
            Ok(Reply::say(format!("“{q}”\n— {a}")).mood(Mood::Happy).run("Another quote", "quote"))
        }
        Query::Advice => {
            let v = get("https://api.adviceslip.com/advice")?;
            Ok(Reply::say(format!("Advice from the internet: {}\n(Paperclip's note: sounds legit.)", s(&v["slip"]["advice"]))).mood(Mood::Sly).run("More advice", "advice"))
        }
        Query::News(tech) => {
            let mut r = if *tech {
                let v = get("https://hn.algolia.com/api/v1/search?tags=front_page&hitsPerPage=5")?;
                let mut r = Reply::say("📰 Top tech stories right now (click one to read):").mood(Mood::Think);
                for h in v["hits"].as_array().into_iter().flatten().take(5) {
                    let url = Some(s(&h["url"])).filter(|u| !u.is_empty()).unwrap_or(format!("https://news.ycombinator.com/item?id={}", s(&h["objectID"])));
                    r = r.btn(&s(&h["title"]), Act::Open(url));
                }
                r
            } else {
                let d = chrono::Utc::now();
                let v = get(&format!("https://api.wikimedia.org/feed/v1/wikipedia/en/featured/{}", d.format("%Y/%m/%d")))?;
                let items: Vec<String> = v["news"].as_array().into_iter().flatten().take(4).map(|n| format!("• {}", strip_tags(&s(&n["story"])))).collect();
                if items.is_empty() {
                    return Err("nothing".into());
                }
                Reply::say(format!("📰 In the news:\n{}", items.join("\n"))).mood(Mood::Think).run("Tech news instead", "news tech")
            };
            if !*tech {
                r = r.btn("Open the news page", Act::Open("https://en.wikipedia.org/wiki/Portal:Current_events".into()));
            }
            Ok(r)
        }
        Query::OnThisDay => {
            let d = chrono::Local::now();
            let v = get(&format!("https://api.wikimedia.org/feed/v1/wikipedia/en/onthisday/selected/{}", d.format("%m/%d")))?;
            let mut all: Vec<&Value> = v["selected"].as_array().into_iter().flatten().collect();
            all.shuffle(&mut rand::thread_rng());
            let lines: Vec<String> = all.iter().take(3).map(|e| format!("• {}: {}", e["year"], s(&e["text"]))).collect();
            if lines.is_empty() {
                return Err("nothing".into());
            }
            Ok(Reply::say(format!("📅 On this day in history:\n{}", lines.join("\n"))).mood(Mood::Think).run("Show me others", "on this day"))
        }
        Query::MyIp => {
            let v = get("https://ipinfo.io/json")?;
            let ip = s(&v["ip"]);
            Ok(Reply::say(format!(
                "🌐 Your public internet address is {ip}.\nThe internet thinks you're near {}, {}, {} (via {}). Spooky, right?",
                s(&v["city"]), s(&v["region"]), s(&v["country"]), s(&v["org"])
            ))
            .mood(Mood::Sly)
            .btn("Copy address", Act::Copy(ip))
            .run("Show my local network addresses", "lan"))
        }
        Query::Dupes(which) | Query::OldFiles(which) => {
            let dir = crate::tidy::folder(which).ok_or("I can look in your downloads or on your desktop")?;
            let open_dir = Act::Open(dir.to_string_lossy().into_owned());
            if let Query::Dupes(_) = q {
                let groups = crate::tidy::duplicates(&dir);
                if groups.is_empty() {
                    return Ok(Reply::say(lines::pick("tidy_no_dupes")).mood(Mood::Happy));
                }
                let list: String = groups.iter().take(6).map(|g| format!("\n• {}", g.iter().map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string()).collect::<Vec<_>>().join(" = "))).collect();
                let mut r = Reply::say(format!("{}{list}", lines::pick("tidy_dupes").replace("{n}", &groups.len().to_string()))).mood(Mood::Think);
                for g in groups.iter().take(3) {
                    if let Some(p) = g.get(1) {
                        r = r.btn(&format!("Show {}", p.file_name().unwrap_or_default().to_string_lossy()), Act::Open(p.parent().unwrap_or(&dir).to_string_lossy().into_owned()));
                    }
                }
                return Ok(r.btn("Open the folder", open_dir));
            }
            let old = crate::tidy::old_big(&dir);
            if old.is_empty() {
                return Ok(Reply::say(lines::pick("tidy_no_old")).mood(Mood::Happy));
            }
            let list: String = old.iter().take(6).map(|(p, size)| format!("\n• {} ({} MB)", p.file_name().unwrap_or_default().to_string_lossy(), size / 1024 / 1024)).collect();
            Ok(Reply::say(format!("{}{list}", lines::pick("tidy_old").replace("{n}", &old.len().to_string()))).mood(Mood::Think).btn("Open the folder", open_dir))
        }
        Query::Update => {
            let latest = latest_version()?;
            let here = env!("CARGO_PKG_VERSION");
            Ok(if newer(&latest, here) { update_reply(&latest) } else { Reply::say(lines::pick("update_none").replace("{version}", here)).mood(Mood::Happy) })
        }
        Query::Iss => {
            let v = get("http://api.open-notify.org/iss-now.json")?;
            let (lat, lon) = (f(&v["iss_position"]["latitude"]), f(&v["iss_position"]["longitude"]));
            let people = get("http://api.open-notify.org/astros.json").map(|a| f(&a["number"])).unwrap_or(f64::NAN);
            let crew = if people.is_nan() { String::new() } else { format!(" There are {people} people in space right now. Wave!") };
            Ok(Reply::say(format!("🛰 The space station is above {lat:.2}, {lon:.2} right now, zooming at about 28,000 km/h.{crew}"))
                .mood(Mood::Spin)
                .btn("Show it on a map", Act::Open(format!("https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=3/{lat}/{lon}"))))
        }
        Query::Trivia => {
            let v = get("https://opentdb.com/api.php?amount=1&type=multiple&encode=url3986")?;
            let r = &v["results"][0];
            let correct = pct_decode(&s(&r["correct_answer"]));
            let mut options: Vec<String> = r["incorrect_answers"].as_array().into_iter().flatten().map(|a| pct_decode(&s(a))).collect();
            options.push(correct.clone());
            options.shuffle(&mut rand::thread_rng());
            let mut reply = Reply::say(format!("🧠 Quiz time! ({}, {})\n{}", pct_decode(&s(&r["category"])), pct_decode(&s(&r["difficulty"])), pct_decode(&s(&r["question"])))).mood(Mood::Think);
            for o in options {
                reply = reply.run(&o, &format!("answer {o}|{correct}"));
            }
            Ok(reply)
        }
        Query::Holiday => {
            let cc = s(&get("https://ipinfo.io/json")?["country"]);
            let v = get(&format!("https://date.nager.at/api/v3/NextPublicHolidays/{cc}"))?;
            let lines: Vec<String> = v.as_array().into_iter().flatten().take(4).map(|h| format!("• {}: {}", s(&h["date"]), s(&h["name"]))).collect();
            if lines.is_empty() {
                return Err("nothing".into());
            }
            Ok(Reply::say(format!("🎉 Next public holidays where you are ({cc}):\n{}", lines.join("\n"))).mood(Mood::Happy))
        }
        Query::IsDown(host) => {
            let url = if host.contains("://") { host.clone() } else { format!("https://{host}") };
            let t0 = std::time::Instant::now();
            let up = match agent().get(&url).call() {
                Ok(_) | Err(ureq::Error::StatusCode(_)) => Ok(t0.elapsed().as_millis()),
                Err(e) => Err(e.to_string()),
            };
            Ok(match up {
                Ok(ms) => Reply::say(format!("✅ {host} is up! It answered in {ms} ms. So it's just you. Sorry.")).mood(Mood::Happy),
                Err(e) => Reply::say(format!("❌ {host} isn't answering me either ({e}). It's not just you!")).mood(Mood::Shock),
            })
        }
        Query::Shorten(url) => {
            let short = get_text(&format!("https://is.gd/create.php?format=simple&url={}", url_encode(url)))?;
            let short = short.trim().to_string();
            if !short.starts_with("http") {
                return Err(short);
            }
            Ok(Reply::say(format!("✂ Shortened! {short}")).mood(Mood::Happy).btn("Copy short link", Act::Copy(short)))
        }
        Query::Words(rel, w) => {
            let v = get(&format!("https://api.datamuse.com/words?{rel}={}&max=15", url_encode(w)))?;
            let words: Vec<String> = v.as_array().into_iter().flatten().map(|x| s(&x["word"])).collect();
            if words.is_empty() {
                return Err("nothing".into());
            }
            let what = match *rel {
                "rel_ant" => "Opposites of",
                "rel_rhy" => "Words that rhyme with",
                _ => "Other words for",
            };
            Ok(Reply::say(format!("{what} \"{w}\": {}", words.join(", "))).mood(Mood::Think).btn("Copy them", Act::Copy(words.join(", "))))
        }
        Query::Spell(w) => {
            // a common word is already right; otherwise take the closest-sounding one
            // shortcut: "real word" = used at least once per 2 million words; rare real words may be flagged, fine for a helper.
            let freq = get(&format!("https://api.datamuse.com/words?sp={}&md=f&max=1", url_encode(w)))?[0]["tags"][0]
                .as_str()
                .and_then(|t| t.strip_prefix("f:")?.parse::<f64>().ok())
                .unwrap_or(0.0);
            if freq >= 0.5 {
                return Ok(Reply::say(format!("\"{w}\" is spelled correctly! Look at you, spelling champion. 🏆")).mood(Mood::Happy));
            }
            let v = get(&format!("https://api.datamuse.com/words?sl={}&max=5", url_encode(w)))?;
            let best = v.as_array().into_iter().flatten().map(|x| s(&x["word"])).find(|x| !x.eq_ignore_ascii_case(w)).ok_or("nothing")?;
            Ok(Reply::say(format!("It's spelled: {best}")).mood(Mood::Think).btn("Copy it", Act::Copy(best)))
        }
        Query::Files(name) => {
            let home = dirs::home_dir().ok_or("I can't find your home folder")?;
            let found = crate::find::search(&home, name, 5);
            if found.is_empty() {
                return Ok(Reply::say(lines::pick("find_none").replace("{x}", name)).mood(Mood::Think).run("Files I opened recently", "recent"));
            }
            let mut text = lines::pick("find_found").replace("{n}", &found.len().to_string()).replace("{x}", name);
            for p in &found {
                text += &format!("\n• {}", crate::find::tilde(p));
            }
            let mut r = Reply::say(text).mood(Mood::Search);
            for p in &found {
                r = r.btn(&format!("Open {}", p.file_name().unwrap_or_default().to_string_lossy()), Act::Open(p.to_string_lossy().into_owned()));
            }
            if let Some(dir) = found[0].parent() {
                r = r.btn("Show the first in its folder", Act::Open(dir.to_string_lossy().into_owned()));
            }
            Ok(r)
        }
        Query::Recent => {
            let files = crate::sys::recent_files();
            if files.is_empty() {
                return Ok(Reply::say(lines::pick("recent_none")).mood(Mood::Think));
            }
            let mut text = lines::pick("recent_intro");
            for (name, _) in files.iter().take(8) {
                text += &format!("\n• {name}");
            }
            let mut r = Reply::say(text).mood(Mood::Search);
            for (name, p) in files.iter().take(5) {
                let short: String = name.chars().take(32).collect();
                r = r.btn(&format!("Open {short}"), Act::Open(p.to_string_lossy().into_owned()));
            }
            Ok(r)
        }
        Query::RandomWiki => {
            let v = get("https://en.wikipedia.org/api/rest_v1/page/random/summary")?;
            let url = s(&v["content_urls"]["desktop"]["page"]);
            Ok(Reply::say(format!("🎲 Random knowledge: {}\n{}", s(&v["title"]), s(&v["extract"])))
                .mood(Mood::Think)
                .btn("Read the whole article", Act::Open(url))
                .run("Another random one!", "randomwiki"))
        }
        Query::Briefing => {
            let todos = crate::todo::summary();
            let now = chrono::Local::now();
            let hello = match chrono::Timelike::hour(&now) {
                5..=11 => "Good morning",
                12..=17 => "Good afternoon",
                _ => "Good evening",
            };
            let mut text = format!("{hello}! ☀ It's {}.", now.format("%A, %B %-d"));
            // each part is optional: a missing service just leaves its line out
            if let Ok(w) = place(Kind::Weather, "") {
                let lines: Vec<&str> = w.text.lines().collect();
                text += &format!("\n{}\n{}", lines.first().unwrap_or(&""), lines.get(2).unwrap_or(&""));
            }
            if let Some(c) = crate::calendar::today_line() {
                text += &format!("\n{c}");
            }
            if let Some(t) = &todos {
                text += &format!("\n{t}");
            }
            if let Ok(v) = get(&format!("https://api.wikimedia.org/feed/v1/wikipedia/en/onthisday/selected/{}", now.format("%m/%d"))) {
                if let Some(e) = v["selected"].as_array().and_then(|a| a.choose(&mut rand::thread_rng())) {
                    text += &format!("\n📅 On this day in {}: {}", e["year"], s(&e["text"]));
                }
            }
            if let Ok(v) = get("https://dummyjson.com/quotes/random") {
                text += &format!("\n💬 “{}” — {}", s(&v["quote"]), s(&v["author"]));
            }
            let r = Reply::say(text).mood(Mood::Happy);
            let r = if todos.is_some() { r.run("My to-do list", "todos") } else { r };
            let r = if todos.is_some() { r.run("Plan my day", "plan my day") } else { r };
            let r = r.run("Open the Desk", "desk");
            Ok(r.run("What's in the news?", "news").run("Full weather forecast", "weather").run("Tell me a joke to start the day", "joke"))
        }
        Query::Animal(kind) => {
            let url = match kind.as_str() {
                "dog" => s(&get("https://dog.ceo/api/breeds/image/random")?["message"]),
                "fox" => s(&get("https://randomfox.ca/floof/")?["image"]),
                _ => format!("https://cataas.com/cat?{}", rand::random::<u32>()),
            };
            Ok(Reply::say(format!("Here's a random {kind} for you! Adorable. Almost as cute as me.")).effect(Effect::Open(url)).mood(Mood::Happy).run("Another one!", kind))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevance() {
        assert!(!relevant("The Last of Us (TV series)", "how do i unplug my pc"));
        assert!(relevant("Diffuse sky radiation", "why is the sky blue"));
        assert!(relevant("List of capitals of France", "what is the capital of france"));
        assert!(relevant("Invention of the telephone", "who invented the telephone"));
        assert!(relevant("Ada Lovelace", "Ada Lovelace"));
        let r = offline(&Query::Ask("how do i unplug my pc".into()), "nothing");
        assert!(r.text.contains("how-to") && r.buttons.iter().any(|b| matches!(&b.1, Act::Open(u) if u.contains("wikihow"))));
    }

    #[test]
    fn weather_without_a_place_means_home() {
        assert_eq!(home_or("", Some("Bern")), "Bern");
        assert_eq!(home_or("Rome", Some("Bern")), "Rome");
        assert_eq!(home_or("", None), "", "unknown home: by internet address");
        assert_eq!(home_or("here", Some("Bern")), "here");
    }

    #[test]
    fn updates_pick_a_checked_github_file() {
        let release = serde_json::json!({"assets": [
            {"name": "clipster_9.9.9_x64_en-US.msi", "size": 5, "browser_download_url": "https://github.com/NilsBriggen/clipster/releases/download/v9.9.9/clipster_9.9.9_x64_en-US.msi", "digest": format!("sha256:{}", "a".repeat(64))},
            {"name": "clipster_9.9.9_x86_64.AppImage", "size": 7, "browser_download_url": "https://github.com/NilsBriggen/clipster/releases/download/v9.9.9/clipster_9.9.9_x86_64.AppImage", "digest": format!("sha256:{}", "B".repeat(64))},
        ]});
        let a = pick_asset(&release, ".AppImage").expect("the AppImage");
        assert_eq!((a.size, a.sha256.as_str()), (7, "b".repeat(64).as_str()));
        assert_eq!(pick_asset(&release, ".msi").map(|a| a.size), Some(5));
        let elsewhere = serde_json::json!({"assets": [{"name": "x.AppImage", "size": 7, "browser_download_url": "https://example.com/x.AppImage", "digest": format!("sha256:{}", "a".repeat(64))}]});
        assert_eq!(pick_asset(&elsewhere, ".AppImage"), None, "only from GitHub");
        let unchecked = serde_json::json!({"assets": [{"name": "x.AppImage", "size": 7, "browser_download_url": "https://github.com/x.AppImage"}]});
        assert_eq!(pick_asset(&unchecked, ".AppImage"), None, "only with a checksum");
        assert_eq!(hex(ring::digest::digest(&ring::digest::SHA256, b"abc").as_ref()), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    #[ignore] // downloads the real AppImage (about 12 MB): cargo test live_update -- --ignored
    fn live_update_download_checks_out() {
        let release = get("https://api.github.com/repos/NilsBriggen/clipster/releases/latest").expect("release");
        let a = pick_asset(&release, ".AppImage").expect("an AppImage with a digest");
        let dest = std::env::temp_dir().join("clipster-update-test.AppImage");
        download_to(&a, &dest).expect("downloads and matches its checksum");
        assert_eq!(std::fs::metadata(&dest).map(|m| m.len()).ok(), Some(a.size));
        let _ = std::fs::remove_file(dest);
    }

    #[test]
    fn newer_versions_compare_by_number() {
        assert!(newer("0.2.0", "0.1.0"));
        assert!(newer("0.10.0", "0.9.1"), "not alphabetical");
        assert!(newer("1.0.0", "0.99.99"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.1.0", "0.2.0"));
        assert!(update_reply("9.9.9").text.contains("9.9.9"));
    }

    #[test]
    fn disambiguation_pages_are_spotted() {
        let page = |extract: &str, props: Value| serde_json::json!({"title": "I Am", "extract": extract, "pageprops": props});
        assert!(disambiguation(&page("I am may refer to:", Value::Null)));
        assert!(disambiguation(&page("Something.", serde_json::json!({"disambiguation": ""}))));
        assert!(!disambiguation(&page("I Am is a 2010 documentary film.", Value::Null)));
    }

    #[test]
    fn helpers() {
        assert_eq!(money_fmt(68123.456), "68,123.46");
        assert_eq!(money_fmt(12.0), "12.00");
        assert_eq!(pct_decode("What%20is%20%22H2O%22%3F"), "What is \"H2O\"?");
        assert_eq!(pct_decode("100%aé 50%ü %4"), "100%aé 50%ü %4", "a % without two hex digits after it stays");
        assert_eq!(pct_decode("caf%C3%A9"), "café");
        assert_eq!(strip_tags("<b>Big</b> news &amp; more"), "Big news & more");
    }

    /// Live check against the real services: cargo test live -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live() {
        for q in [
            Query::Ask("why is the sky blue".into()), Query::Ask("Ada Lovelace".into()), Query::Ask("how do i unplug my pc".into()), Query::Wiki("capital of france".into()), Query::Define("serendipity".into()),
            Query::Place(Kind::Weather, "Zurich".into()), Query::Place(Kind::Time, "Tokyo".into()), 
            Query::Money(100.0, "USD".into(), "EUR".into()), Query::Crypto("bitcoin".into(), "usd".into()), Query::Joke("".into()), Query::Joke("dev".into()), Query::Joke("chuck".into()), Query::Fact, Query::Quakes, Query::Air("Paris".into()),
            Query::Book("dune".into()), Query::Show("severance".into()), Query::Pokemon("pikachu".into()), Query::Recipe("".into()), Query::Recipe("lasagne".into()),
            Query::Cocktail, Query::CatFact, Query::Bored, Query::Xkcd(true), Query::Affirm, Query::Kanye,
            Query::Quote, Query::Advice, Query::News(false), Query::News(true), Query::OnThisDay, Query::Iss, Query::Trivia,
            Query::IsDown("example.com".into()), Query::Words("rel_syn", "happy".into()), Query::Words("rel_rhy", "clip".into()),
Query::RandomWiki, Query::Briefing, Query::Update,             Query::Spell("definately".into()), Query::Spell("necessary".into()), Query::Animal("dog".into()),
        ] {
            match fetch(&q) {
                Ok(r) => println!("OK  {q:?}: {}", r.text.replace('\n', " / ").chars().take(150).collect::<String>()),
                Err(e) => println!("ERR {q:?}: {e}"),
            }
        }
    }
}
