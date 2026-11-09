//! Control the computer: sound, music, screen, power, files, dates and handy little generators.

use crate::brain::{shell, Act, Effect, Mood, Reply};
use chrono::{Datelike, Duration, Local, NaiveDate};
use rand::Rng;

/// Put text inside single quotes for the shell.
fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn num(s: &str) -> Option<u32> {
    s.split(|c: char| !c.is_ascii_digit()).find_map(|w| w.parse().ok())
}

// shortcut: sound uses PipeWire (wpctl), music uses MPRIS via qdbus6 and brightness uses KDE's service; add pactl/playerctl/brightnessctl fallbacks if other desktops matter.
/// Windows has no volume or media command line, so press the keyboard's media keys (a volume step is 2%).
fn media_keys(key: u8, times: u32) -> String {
    format!("$w = New-Object -ComObject WScript.Shell; 1..{times} | ForEach-Object {{ $w.SendKeys([char]{key}) }}")
}

pub fn volume(rest: &str) -> Reply {
    // "increase the volume by 10" is a step, not 10 %
    let up = ["up", "increase", "raise", "louder", "more"].iter().any(|w| rest.contains(w));
    let down = ["down", "decrease", "lower", "quieter", "softer", "less", "reduce"].iter().any(|w| rest.contains(w));
    let step = num(rest).filter(|_| (up || down) && (rest.contains("by ") || up || down)).map(|n| n.clamp(1, 100));
    if let Some(n) = step {
        if cfg!(windows) {
            let cmd = media_keys(if up { 175 } else { 174 }, (n / 2).max(1));
            return shell(&format!("{cmd}; 'Done.'"), if up { "Turning it up! 🔊" } else { "A little quieter..." }).run("Louder", "volume up").run("Quieter", "volume down").run("Mute", "volume mute");
        }
        const SINK: &str = "@DEFAULT_AUDIO_SINK@";
        let cmd = format!("wpctl set-mute {SINK} 0; wpctl set-volume -l 1.0 {SINK} {n}%{}", if up { "+" } else { "-" });
        return shell(&cmd, if up { "Turning it up! 🔊" } else { "A little quieter..." });
    }
    if cfg!(windows) {
        let (cmd, intro) = if rest.contains("unmute") {
            (media_keys(175, 1), "Sound's back on! 🔊") // any volume key unmutes
        } else if rest.contains("mute") {
            (media_keys(173, 1), "Mute toggled. 🔇")
        } else if let Some(n) = num(rest) {
            (format!("{}; {}", media_keys(174, 50), media_keys(175, n.min(100) / 2)), "Volume set!")
        } else if ["down", "lower", "quieter", "softer", "less"].iter().any(|w| rest.contains(w)) {
            (media_keys(174, 5), "A little quieter...")
        } else if ["max", "full"].iter().any(|w| rest.contains(w)) {
            (media_keys(175, 50), "MAXIMUM VOLUME! 🎸")
        } else if rest.is_empty() || rest.contains("what") || rest.contains("how") {
            return Reply::say("Windows won't tell me the exact volume, but I can change it:").run("Louder", "volume up").run("Quieter", "volume down").run("Mute", "volume mute");
        } else {
            (media_keys(175, 5), "Turning it up! 🔊")
        };
        return shell(&format!("{cmd}; 'Done.'"), intro).run("Louder", "volume up").run("Quieter", "volume down").run("Mute", "volume mute");
    }
    const SINK: &str = "@DEFAULT_AUDIO_SINK@";
    let (cmd, intro) = if rest.contains("unmute") {
        (format!("wpctl set-mute {SINK} 0"), "Sound's back on! 🔊")
    } else if rest.contains("mute") {
        (format!("wpctl set-mute {SINK} 1"), "Shhh. Muted. 🔇")
    } else if let Some(n) = num(rest) {
        (format!("wpctl set-mute {SINK} 0; wpctl set-volume -l 1.0 {SINK} {}%", n.min(100)), "Volume set!")
    } else if ["down", "lower", "quieter", "softer", "less"].iter().any(|w| rest.contains(w)) {
        (format!("wpctl set-volume {SINK} 10%-"), "A little quieter...")
    } else if ["max", "full"].iter().any(|w| rest.contains(w)) {
        (format!("wpctl set-mute {SINK} 0; wpctl set-volume {SINK} 100%"), "MAXIMUM VOLUME! 🎸")
    } else if rest.is_empty() || rest.contains("what") || rest.contains("how") {
        (String::new(), "Current volume:")
    } else {
        (format!("wpctl set-mute {SINK} 0; wpctl set-volume -l 1.0 {SINK} 10%+"), "Turning it up! 🔊")
    };
    shell(&format!("{cmd}; wpctl get-volume {SINK}").trim_start_matches("; "), intro)
        .run("Louder", "volume up")
        .run("Quieter", "volume down")
        .run("Mute", "volume mute")
}

pub fn media(rest: &str) -> Reply {
    if cfg!(windows) {
        let key = match rest {
            r if r.contains("next") || r.contains("skip") => Some((176, "Skipping! ⏭")),
            r if r.contains("prev") || r.contains("back") || r.contains("last") => Some((177, "Going back! ⏮")),
            r if r.contains("what") || r.contains("song") || r.contains("playing") => None,
            _ => Some((179, "Play/pause! ⏯")),
        };
        // the song is in the player's window title
        let what = r#"$t = Get-Process | Where-Object { $_.MainWindowTitle -and $_.ProcessName -match 'spotify|vlc|foobar2000|musicbee|aimp|winamp|itunes' } | ForEach-Object MainWindowTitle; if ($t) { "Now playing: $t" } else { 'No music player is showing a song title.' }"#;
        let (cmd, intro) = match key {
            Some((k, intro)) => (format!("{}; Start-Sleep -Milliseconds 400; {what}", media_keys(k, 1)), intro),
            None => (what.to_string(), ""),
        };
        return shell(&cmd, intro).mood(Mood::Spin).run("Pause / play", "media toggle").run("Next song", "media next");
    }
    let (method, intro) = match rest {
        r if r.contains("next") || r.contains("skip") => ("Next", "Skipping! ⏭"),
        r if r.contains("prev") || r.contains("back") || r.contains("last") => ("Previous", "Going back! ⏮"),
        r if r.contains("pause") || r.contains("stop") => ("Pause", "Paused. ⏸"),
        r if r.contains("play") || r.contains("resume") => ("Play", "Playing! ▶"),
        r if r.contains("what") || r.contains("song") || r.contains("playing") => ("", "Now playing:"),
        _ => ("PlayPause", "Play/pause! ⏯"),
    };
    let player = "p=$(qdbus6 | grep -m1 org.mpris.MediaPlayer2); [ -z \"$p\" ] && { echo 'No music player is running. Want me to open one?'; exit; }";
    let what = "qdbus6 $p /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player.Metadata | grep -E 'xesam:(title|artist)' | sed 's/^xesam://'";
    let cmd = if method.is_empty() {
        format!("{player}; {what}")
    } else {
        format!("{player}; qdbus6 $p /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player.{method}; sleep 0.4; {what}")
    };
    shell(&cmd, intro).mood(Mood::Spin).run("Pause / play", "media toggle").run("Next song", "media next")
}

pub fn brightness(rest: &str) -> Reply {
    if cfg!(windows) {
        let level = if let Some(n) = num(rest) {
            n.clamp(1, 100).to_string()
        } else if ["down", "lower", "dim", "darker", "less"].iter().any(|w| rest.contains(w)) {
            "[math]::Max(1, $c - 15)".into()
        } else if rest.is_empty() || rest.contains("what") {
            "$c".into()
        } else {
            "[math]::Min(100, $c + 15)".into()
        };
        let cmd = format!(
            r#"try {{ $c = (Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightness -ErrorAction Stop).CurrentBrightness; $n = {level}; $m = Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightnessMethods -ErrorAction Stop; Invoke-CimMethod -InputObject $m -MethodName WmiSetBrightness -Arguments @{{Timeout=1; Brightness=[byte]$n}} | Out-Null; "Brightness: $n%" }} catch {{ "This screen won't let me change its brightness. (Desktop monitors usually don't.)" }}"#
        );
        return shell(&cmd, "Adjusting the screen...").run("Brighter", "brightness up").run("Dimmer", "brightness down");
    }
    let svc = "qdbus6 org.kde.ScreenBrightness";
    let display = format!("d=$({svc} /org/kde/ScreenBrightness org.kde.ScreenBrightness.DisplaysDBusNames | head -1)");
    let read = format!("echo \"Brightness: $(( $({svc} /org/kde/ScreenBrightness/$d org.kde.ScreenBrightness.Display.Brightness) * 100 / $({svc} /org/kde/ScreenBrightness/$d org.kde.ScreenBrightness.Display.MaxBrightness) ))%\"");
    let (cmd, intro) = if let Some(n) = num(rest) {
        (format!("{display}; m=$({svc} /org/kde/ScreenBrightness/$d org.kde.ScreenBrightness.Display.MaxBrightness); {svc} /org/kde/ScreenBrightness/$d org.kde.ScreenBrightness.Display.SetBrightness $((m * {} / 100)) 0", n.clamp(1, 100)), "Adjusting the screen...")
    } else if ["down", "lower", "dim", "darker", "less"].iter().any(|w| rest.contains(w)) {
        (format!("{display}; {svc} /org/kde/ScreenBrightness org.kde.ScreenBrightness.AdjustBrightnessRatio -0.15 0"), "Dimming the lights... 🌙")
    } else if rest.is_empty() || rest.contains("what") {
        (display.clone(), "Screen brightness:")
    } else {
        (format!("{display}; {svc} /org/kde/ScreenBrightness org.kde.ScreenBrightness.AdjustBrightnessRatio 0.15 0"), "Let there be light! ☀")
    };
    shell(&format!("{cmd}; sleep 0.3; {read}"), intro).run("Brighter", "brightness up").run("Dimmer", "brightness down")
}

pub fn screenshot(rest: &str) -> Reply {
    let dir = dirs::picture_dir().or_else(dirs::home_dir).unwrap_or_default();
    let path = dir.join(format!("Clipster screenshot {}.png", Local::now().format("%Y-%m-%d %H-%M-%S")));
    let p = q(&path.to_string_lossy());
    let area = rest.contains("area") || rest.contains("region") || rest.contains("part") || rest.contains("select");
    if cfg!(windows) {
        if area {
            return Reply::say("Draw a box around what you want. It goes on your clipboard: paste it anywhere with Ctrl+V. 📸").effect(Effect::Open("ms-screenclip:".into()));
        }
        let win_path = crate::sys::ps_quote(&path.to_string_lossy());
        let cmd = format!("Add-Type -AssemblyName System.Windows.Forms,System.Drawing; Start-Sleep -Milliseconds 500; $b = [Windows.Forms.SystemInformation]::VirtualScreen; $i = New-Object Drawing.Bitmap $b.Width,$b.Height; $g = [Drawing.Graphics]::FromImage($i); $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $i.Size); $i.Save('{win_path}'); 'Saved to {win_path}'");
        return shell(&cmd, "Say cheese! 📸")
            .btn("Open my pictures", Act::Open(dir.to_string_lossy().into_owned()))
            .btn("Open the screenshot", Act::Open(path.to_string_lossy().into_owned()));
    }
    let mode = if area { "-r" } else if rest.contains("window") { "-a" } else { "-f" };
    let cmd = format!("sleep 0.5; (spectacle -b -n {mode} -o {p} || grim {p} || gnome-screenshot -f {p}) >/dev/null 2>&1 && echo Saved to {p} || echo 'No screenshot tool worked, sorry!'");
    shell(&cmd, "Say cheese! 📸")
        .btn("Open my pictures", Act::Open(dir.to_string_lossy().into_owned()))
        .btn("Open the screenshot", Act::Open(path.to_string_lossy().into_owned()))
}

pub fn power(rest: &str) -> Reply {
    let win = cfg!(windows);
    let (what, cmd) = match rest {
        r if r.contains("restart") || r.contains("reboot") => ("restart the computer", if win { "shutdown /r /t 0" } else { "systemctl reboot" }),
        // rundll32 can't pass SetSuspendState's arguments and hibernates instead; this sleeps
        r if r.contains("sleep") || r.contains("suspend") => ("put the computer to sleep", if win { "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Application]::SetSuspendState('Suspend', $false, $false)" } else { "systemctl suspend" }),
        r if r.contains("hibernate") => ("hibernate the computer", if win { "shutdown /h" } else { "systemctl hibernate" }),
        r if r.contains("log") || r.contains("sign") => ("log you out", if win { "shutdown /l" } else { "loginctl terminate-session $XDG_SESSION_ID" }),
        _ => ("shut down the computer", if win { "shutdown /s /t 0" } else { "systemctl poweroff" }),
    };
    Reply::say(format!("You want me to {what}? Save your work first! I'll wait. Nervously."))
        .mood(Mood::Shock)
        .run(&format!("Yes, {what}"), &format!("run {cmd}"))
        .btn("No! Cancel!", Act::Hide)
}

pub fn trash() -> Reply {
    Reply::say("Empty the trash? Everything in it will be gone forever. Like my dignity after the 90s.")
        .mood(Mood::Shock)
        .run("Yes, empty it", if cfg!(windows) { "run Clear-RecycleBin -Force -ErrorAction SilentlyContinue; 'Trash emptied! 🗑'" } else { "run { gio trash --empty 2>/dev/null || ktrash6 --empty 2>/dev/null || ktrash5 --empty; } && echo 'Trash emptied! 🗑'" })
        .btn("Show me the trash first", Act::Open(if cfg!(windows) { "shell:RecycleBinFolder" } else { "trash:///" }.into()))
        .btn("No, keep it", Act::Hide)
}

pub fn latest_download() -> Reply {
    let dir = dirs::download_dir().or_else(|| dirs::home_dir().map(|h| h.join("Downloads"))).unwrap_or_default();
    let newest = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()) && !e.file_name().to_string_lossy().starts_with('.'))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    match newest {
        Some(e) => Reply::say(format!("Your newest download is \"{}\".", e.file_name().to_string_lossy()))
            .mood(Mood::Happy)
            .btn("Open it", Act::Open(e.path().to_string_lossy().into_owned()))
            .btn("Open the downloads folder", Act::Open(dir.to_string_lossy().into_owned())),
        None => Reply::say("Your downloads folder is empty. Squeaky clean!").mood(Mood::Happy),
    }
}

pub fn kaomoji(rest: &str) -> Reply {
    let face = match rest {
        r if r.contains("unflip") || r.contains("back") => "┬─┬ノ( º _ ºノ)",
        r if r.contains("flip") || r.contains("table") => "(╯°□°)╯︵ ┻━┻",
        r if r.contains("lenny") => "( ͡° ͜ʖ ͡°)",
        r if r.contains("disapprov") => "ಠ_ಠ",
        r if r.contains("hug") => "(づ｡◕‿‿◕｡)づ",
        r if r.contains("happy") || r.contains("joy") => "\\(^o^)/",
        _ => "¯\\_(ツ)_/¯",
    };
    Reply::say(format!("{face}\nI put it on your clipboard for you.")).mood(Mood::Wink).btn("Copy it again", Act::Copy(face.into())).effect(Effect::CopyNow(face.into()))
}

pub fn uuid() -> Reply {
    let mut b: [u8; 16] = rand::thread_rng().gen();
    b[6] = b[6] & 0x0f | 0x40; // version 4
    b[8] = b[8] & 0x3f | 0x80; // the RFC 4122 variant
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let id = format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32]);
    Reply::say(format!("Fresh unique ID:\n{id}")).btn("Copy it", Act::Copy(id))
}

pub fn lorem() -> Reply {
    const L: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";
    Reply::say(format!("{L}\n(Fancy Latin for \"text goes here\".)")).btn("Copy it", Act::Copy(L.into()))
}

pub fn qr(rest: &str) -> Reply {
    if rest.trim().is_empty() {
        return Reply::say("A QR code for what? Try: qr code for https://example.com");
    }
    // drawn here, on the Desk: the text never goes to a website
    Reply::say(crate::lines::pick("qr_ready")).mood(Mood::Happy).effect(Effect::ShowQr(rest.trim().to_string()))
}

// ---------- dates ----------

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const DAYS: [&str; 7] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];

/// "christmas", "2027-03-01", "june 3rd", "3 june", "friday", "tomorrow" -> the next such date.
pub fn parse_day(s: &str, today: NaiveDate) -> Option<(NaiveDate, String)> {
    let s = s.trim().trim_start_matches("the ").trim_start_matches("next ").trim_end_matches('?').trim();
    // the next time that date comes round: 29 February looks ahead to the next leap year
    let next = |m: u32, d: u32| (0..=8).filter_map(|k| NaiveDate::from_ymd_opt(today.year() + k, m, d)).find(|x| *x >= today);
    let named = [
        ("christmas eve", (12, 24)), ("christmas", (12, 25)), ("xmas", (12, 25)), ("new years eve", (12, 31)), ("new year's eve", (12, 31)),
        ("new year", (1, 1)), ("halloween", (10, 31)), ("valentine", (2, 14)), ("pi day", (3, 14)), ("star wars day", (5, 4)),
    ];
    if let Some((name, (m, d))) = named.iter().find(|(n, _)| s.starts_with(n)) {
        return Some((next(*m, *d)?, name.to_string()));
    }
    match s {
        "tomorrow" => return Some((today + Duration::days(1), "tomorrow".into())),
        "weekend" | "the weekend" => return parse_day("saturday", today).map(|(d, _)| (d, "the weekend".into())),
        _ => {}
    }
    if let Some(i) = DAYS.iter().position(|d| s.starts_with(d)) {
        let ahead = (i as i64 - today.weekday().num_days_from_monday() as i64).rem_euclid(7);
        return Some((today + Duration::days(if ahead == 0 { 7 } else { ahead }), DAYS[i].to_string()));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some((d, d.format("%B %-d, %Y").to_string()));
    }
    // "the 15th": the next 15th (a 31st skips the months without one)
    if let Some(n) = ["st", "nd", "rd", "th"].iter().find_map(|suf| s.strip_suffix(suf)).and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=31).contains(n)) {
        let date = (0..400).map(|k| today + Duration::days(k)).find(|d| d.day() == n)?;
        return Some((date, date.format("%B %-d").to_string()));
    }
    let m = s.split(|c: char| !c.is_ascii_alphabetic()).find_map(|w| MONTHS.iter().position(|m| w.len() >= 3 && w.starts_with(m)))? as u32 + 1;
    let day = num(s)?;
    let year = s.split(|c: char| !c.is_ascii_digit()).filter_map(|w| w.parse::<i32>().ok()).find(|y| *y > 1000);
    let date = match year {
        Some(y) => NaiveDate::from_ymd_opt(y, m, day)?,
        None => next(m, day)?,
    };
    Some((date, date.format("%B %-d").to_string()))
}

/// "until christmas", "until in 90 days", "until 2027-01-01"
pub fn until(rest: &str) -> Reply {
    let today = Local::now().date_naive();
    let rest = rest.trim();
    if let Some(r) = rest.strip_prefix("in ") {
        let n = num(r).unwrap_or(1).min(100_000) as i64;
        let days = match r {
            r if r.contains("week") => n * 7,
            r if r.contains("month") => n * 30,
            r if r.contains("year") => n * 365,
            _ => n,
        };
        let Some(d) = today.checked_add_signed(Duration::days(days)) else {
            return Reply::say("That's further away than my calendar goes.").mood(Mood::Think);
        };
        return Reply::say(format!("📅 In {} it'll be {}.", r.trim(), d.format("%A, %B %-d, %Y"))).mood(Mood::Think);
    }
    let Some((date, label)) = parse_day(rest, today) else {
        return Reply::say("Until when? Try: days until christmas, days until june 3, or what date is in 90 days");
    };
    let label = label[..1].to_uppercase() + &label[1..];
    let days = (date - today).num_days();
    let when = date.format("%A, %B %-d, %Y");
    let text = match days {
        0 => format!("🎉 {label} is TODAY! ({when})"),
        d if d < 0 => format!("{label} was {} days ago ({when}). Time flies!", -d),
        1 => format!("Just 1 day until {label} ({when})! Get excited!"),
        d => format!("📅 {d} days until {label} (that's {:.1} weeks). It's a {when}.", d as f64 / 7.0),
    };
    Reply::say(text).mood(Mood::Happy)
}

// ---------- text tools ----------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= chunk.len() { B64[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// Standard or URL-safe base64; whitespace is ignored. `None` = not base64.
pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let (mut out, mut acc, mut bits) = (vec![], 0u32, 0);
    for c in s.trim().trim_end_matches('=').bytes().filter(|c| !c.is_ascii_whitespace()) {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Base64 that decodes to readable text (what people actually paste around).
pub fn b64_text(s: &str) -> Option<String> {
    String::from_utf8(b64_decode(s)?).ok().filter(|t| !t.is_empty() && t.chars().all(|c| !c.is_control() || c == '\n' || c == '\t'))
}

pub fn slug(t: &str) -> String {
    t.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join("-")
}

/// A link without its tracking junk (utm_*, fbclid, ...). `None` = there was none.
pub fn clean_link(url: &str) -> Option<String> {
    let (base, rest) = url.split_once('?')?;
    let (query, frag) = rest.split_once('#').map_or((rest, None), |(q, f)| (q, Some(f)));
    let junk = |k: &str| k.starts_with("utm_") || ["fbclid", "gclid", "dclid", "msclkid", "mc_cid", "mc_eid", "igshid", "yclid", "_hsenc", "_hsmi", "si", "ref_src", "spm"].contains(&k);
    let kept: Vec<&str> = query.split('&').filter(|p| !junk(p.split('=').next().unwrap_or(""))).collect();
    if kept.len() == query.split('&').count() {
        return None;
    }
    let mut out = base.to_string();
    if !kept.is_empty() {
        out += &format!("?{}", kept.join("&"));
    }
    if let Some(f) = frag {
        out += &format!("#{f}");
    }
    Some(out)
}

fn sha256(t: &str) -> Option<String> {
    use std::io::Write;
    use std::process::Stdio;
    // the text goes in on stdin, never into a command line
    let mut child = if cfg!(windows) {
        crate::sys::shell("[Console]::InputEncoding = [Text.Encoding]::UTF8; $s = [Console]::In.ReadToEnd(); -join ([Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($s)) | ForEach-Object { $_.ToString('x2') })")
    } else {
        crate::sys::command("sha256sum")
    }
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .ok()?;
    child.stdin.take()?.write_all(t.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    String::from_utf8(out.stdout).ok()?.split_whitespace().next().map(String::from)
}

/// The Desk's text tools on a copy or a pin: the changed text, or None for a tool it doesn't know (or base64 that
/// isn't). Pure, so the Clipboard tab and the tests agree.
pub const CLIP_TOOLS: [(&str, &str); 11] = [("fix", "Fix the easy mistakes"), ("upper", "UPPER CASE"), ("lower", "lower case"), ("title", "Title Case"), ("oneline", "One line"), ("sortlines", "Sort the lines"), ("dedupe", "Remove repeated lines"), ("slug", "As a slug"), ("urldecode", "Decode a web address"), ("base64", "To base64"), ("unbase64", "From base64")];

pub fn clip_tool(tool: &str, t: &str) -> Option<String> {
    Some(match tool {
        "fix" => crate::writing::fix(t, &crate::writing::misspellings()),
        "upper" => t.to_uppercase(),
        "lower" => t.to_lowercase(),
        "title" => t.split(' ').map(|w| crate::writing::cap_first(&w.to_lowercase())).collect::<Vec<_>>().join(" "),
        "oneline" => t.split_whitespace().collect::<Vec<_>>().join(" "),
        "sortlines" => {
            let mut l: Vec<&str> = t.lines().collect();
            l.sort_by_key(|x| x.to_lowercase());
            l.join("\n")
        }
        "dedupe" => {
            let mut seen = std::collections::HashSet::new();
            t.lines().filter(|l| seen.insert(*l)).collect::<Vec<_>>().join("\n")
        }
        "slug" => slug(t),
        "urldecode" => crate::net::pct_decode(t),
        "base64" => b64_encode(t.as_bytes()),
        "unbase64" => b64_text(t)?,
        _ => return None,
    })
}

/// base64, unbase64, urlencode, urldecode, slug, sortlines, dedupe, sha256 (on the text, or the clipboard).
pub fn text_tool(kind: &str, rest: &str) -> Reply {
    let t = crate::brain::or_clipboard(rest);
    if t.is_empty() {
        return Reply::say(format!("{kind} what? Give me some text, or copy some first.")).mood(Mood::Think);
    }
    let out = match kind {
        "base64" => b64_encode(t.as_bytes()),
        "unbase64" => match b64_text(&t) {
            Some(s) => s,
            None => return Reply::say("That's not base64. Or it is, and it's hiding binary goblins. Either way: no.").mood(Mood::Think),
        },
        "urlencode" => crate::brain::url_encode(&t),
        "urldecode" => crate::net::pct_decode(&t),
        "slug" => slug(&t),
        "sortlines" => {
            let mut l: Vec<&str> = t.lines().collect();
            l.sort_by_key(|x| x.to_lowercase());
            l.join("\n")
        }
        "dedupe" => {
            let mut seen = std::collections::HashSet::new();
            t.lines().filter(|l| seen.insert(*l)).collect::<Vec<_>>().join("\n")
        }
        "sha256" => match sha256(&t) {
            Some(h) => h,
            None => return Reply::say("I can't find sha256sum on this computer. Hashing by hand is beyond even me.").mood(Mood::Sad),
        },
        _ => t,
    };
    Reply::say(out.clone()).mood(Mood::Cool).btn("Copy it", Act::Copy(out))
}

// ---------- number tools ----------

pub fn roman(mut n: u32) -> Option<String> {
    const R: [(u32, &str); 13] = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    if n == 0 || n > 3999 {
        return None;
    }
    let mut s = String::new();
    for (v, r) in R {
        while n >= v {
            s += r;
            n -= v;
        }
    }
    Some(s)
}

pub fn from_roman(s: &str) -> Option<u32> {
    let val = |c| match c {
        'I' => 1,
        'V' => 5,
        'X' => 10,
        'L' => 50,
        'C' => 100,
        'D' => 500,
        'M' => 1000,
        _ => 0,
    };
    let up = s.trim().to_uppercase();
    let v: Vec<i64> = up.chars().map(val).collect();
    if v.is_empty() || v.contains(&0) {
        return None;
    }
    let n: i64 = v.iter().enumerate().map(|(i, x)| if v.get(i + 1).is_some_and(|y| y > x) { -x } else { *x }).sum();
    let n = u32::try_from(n).ok()?;
    (roman(n)? == up).then_some(n) // "IIII" is not a number, it's a cry for help
}

/// "255 in hex", "0xff to decimal", "2024 in roman", "MMXXIV to number"
pub fn bases(s: &str) -> Option<Reply> {
    let s = s.trim().trim_end_matches('?').to_lowercase();
    let (num, target) = [" in ", " to ", " as ", " into "].iter().find_map(|k| s.rsplit_once(k))?;
    let num = ["convert ", "what is ", "whats "].iter().fold(num.trim(), |n, p| n.trim_start_matches(p)).trim();
    let n = if let Some(h) = num.strip_prefix("0x") {
        u64::from_str_radix(h, 16).ok()
    } else if let Some(b) = num.strip_prefix("0b") {
        u64::from_str_radix(b, 2).ok()
    } else if let Some(o) = num.strip_prefix("0o") {
        u64::from_str_radix(o, 8).ok()
    } else {
        num.replace(['_', ','], "").parse().ok().or_else(|| from_roman(num).map(u64::from))
    }?;
    let out = match target.trim().trim_start_matches("a ") {
        "hex" | "hexadecimal" => format!("0x{n:X}"),
        "binary" | "bin" | "bits" => format!("0b{n:b}"),
        "octal" | "oct" => format!("0o{n:o}"),
        "decimal" | "dec" | "number" | "normal number" | "arabic" | "digits" => n.to_string(),
        "roman" | "roman numerals" | "roman numeral" | "roman number" => roman(u32::try_from(n).ok()?)?,
        _ => return None,
    };
    Some(Reply::say(format!("{} = {out}", num.to_uppercase().replace("0X", "0x").replace("0B", "0b").replace("0O", "0o"))).mood(Mood::Think).btn("Copy answer", Act::Copy(out)))
}

fn numbers(s: &str) -> Vec<f64> {
    s.replace(['$', '€', '£', '%', ','], " ").split_whitespace().filter_map(|w| w.parse().ok()).collect()
}

/// "tip 15% on 48", "tip on 48"
pub fn tip_calc(rest: &str) -> Reply {
    let words: Vec<String> = rest.replace('%', " % ").replace("percent", " % ").split_whitespace().map(String::from).collect();
    let (mut pct, mut bill) = (None, None);
    for (i, w) in words.iter().enumerate() {
        if let Ok(v) = w.trim_start_matches(['$', '€', '£']).parse::<f64>() {
            if words.get(i + 1).is_some_and(|n| n == "%") { pct = Some(v) } else { bill = Some(v) }
        }
    }
    let Some(bill) = bill else { return Reply::say("Tip on what? Try: tip 15% on 48") };
    let text: Vec<String> = pct.map_or(vec![15.0, 18.0, 20.0], |p| vec![p]).iter().map(|p| format!("{p}% tip: {:.2} → total {:.2}", bill * p / 100.0, bill * (1.0 + p / 100.0))).collect();
    Reply::say(format!("💸 On a bill of {bill:.2}:\n{}\n\nOr tip in compliments. Waiters love those. (They don't.)", text.join("\n"))).mood(Mood::Think).btn("Copy it", Act::Copy(text.join("\n")))
}

/// "split 120 between 4"
pub fn split_bill(rest: &str) -> Reply {
    match numbers(rest)[..] {
        [total, people, ..] if people >= 1.0 => Reply::say(format!("🧾 {total:.2} split {people} ways: {:.2} each. Someone will still \"forget their wallet\".", total / people.floor()))
            .mood(Mood::Think)
            .btn("Copy amount", Act::Copy(format!("{:.2}", total / people.floor()))),
        _ => Reply::say("Split what between how many? Try: split 120 between 4"),
    }
}

/// "pctchange 80 100"
pub fn pct_change(rest: &str) -> Reply {
    match numbers(rest)[..] {
        [from, to, ..] if from != 0.0 => {
            let p = (to - from) / from.abs() * 100.0;
            let pct = format!("{}{p:.1}%", if p > 0.0 { "+" } else { "" });
            Reply::say(format!("📈 From {from} to {to} is {pct}.")).mood(Mood::Think).btn("Copy it", Act::Copy(pct))
        }
        _ => Reply::say("Try: percent change from 80 to 100"),
    }
}

fn a_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim().trim_start_matches("on ").trim_start_matches("in ").trim();
    // a bare year: "born in 1990" (1 January, so the age is right from then on)
    if let Some(y) = s.parse::<i32>().ok().filter(|y| (1000..=9999).contains(y)) {
        return NaiveDate::from_ymd_opt(y, 1, 1);
    }
    ["%d.%m.%Y", "%m/%d/%Y", "%d/%m/%Y"].iter().find_map(|f| NaiveDate::parse_from_str(s, f).ok()).or_else(|| parse_day(s, Local::now().date_naive()).map(|(d, _)| d))
}

/// "age 1990-05-03": how old someone born then is today.
pub fn age(rest: &str) -> Reply {
    let today = Local::now().date_naive();
    let Some(born) = a_date(rest).filter(|d| *d <= today) else {
        return Reply::say("Born when? Try: how old am I if I was born 1990-05-03").mood(Mood::Think);
    };
    let years = today.years_since(born).unwrap_or(0);
    let days = (today - born).num_days();
    Reply::say(format!("🎂 {years} years old. That's {days} days of being a human. Exhausting, I bet.")).mood(Mood::Think).btn("Copy it", Act::Copy(years.to_string()))
}

/// "between june 3 and christmas"
pub fn days_between(rest: &str) -> Reply {
    let dates = rest.split_once(" and ").and_then(|(a, b)| Some((a_date(a)?, a_date(b)?)));
    let Some((a, b)) = dates else { return Reply::say("Between which dates? Try: days between june 3 and christmas") };
    let d = (b - a).num_days().abs();
    Reply::say(format!("📅 {d} days between them ({} weeks and {} days). Not that anyone's counting. I am. I'm counting.", d / 7, d % 7)).mood(Mood::Think).btn("Copy it", Act::Copy(d.to_string()))
}

/// "timestamp" (now), "timestamp 1700000000" (to a date), "timestamp 2026-01-01" (to unix time).
pub fn timestamp(rest: &str) -> Reply {
    use chrono::TimeZone;
    let rest = rest.trim();
    let digits = rest.chars().all(|c| c.is_ascii_digit());
    if digits && (rest.len() == 10 || rest.len() == 13) {
        let secs: i64 = rest[..10].parse().unwrap_or(0);
        if let Some(d) = Local.timestamp_opt(secs, 0).single() {
            let text = format!("{}, {}", d.format("%A, %B %-d %Y"), crate::brain::clock(d));
            return Reply::say(format!("🕰 {rest} is {text} (your time).")).mood(Mood::Think).btn("Copy the date", Act::Copy(text));
        }
    }
    let at = if rest.is_empty() || rest == "now" { Some(Local::now().timestamp()) } else { a_date(rest).and_then(|d| Local.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single()).map(|d| d.timestamp()) };
    match at {
        Some(t) => Reply::say(format!("🕰 Unix time{}: {t}", if rest.is_empty() || rest == "now" { " right now".to_string() } else { format!(" for {rest}") })).mood(Mood::Think).btn("Copy it", Act::Copy(t.to_string())),
        None => Reply::say("Try: timestamp, timestamp 1700000000, or timestamp 2026-01-01"),
    }
}

// ---------- system powers ----------

pub fn radio(kind: &str, on: bool) -> Reply {
    if cfg!(windows) {
        let (page, name) = if kind == "bluetooth" { ("ms-settings:bluetooth", "Bluetooth") } else { ("ms-settings:network-wifi", "Wi-Fi") };
        return Reply::say(format!("Windows won't let me flip the {name} switch myself, so here it is. One click, and it's {}.", if on { "on" } else { "off" })).mood(Mood::Sly).effect(Effect::Open(page.into()));
    }
    let (cmd, name) = if kind == "bluetooth" { ("bluetoothctl power", "Bluetooth") } else { ("nmcli radio wifi", "Wi-Fi") };
    let state = if on { "on" } else { "off" };
    let intro = if on { format!("{name} on. Welcome back to the internet. It missed you. Mostly the ads did.") } else { format!("{name} off. Enjoy the silence. I'll still be here. Offline. Judging.") };
    shell(&format!("{cmd} {state}"), &intro).mood(if on { Mood::Happy } else { Mood::Cool })
}

/// How much memory a running program uses (all its processes together).
pub fn app_ram(name: &str) -> Reply {
    let name = crate::sys::program_name(name);
    if name.is_empty() {
        return Reply::say("Which program? Try: how much ram is firefox using");
    }
    let kb = crate::sys::program_memory_kb(&name);
    if kb.is_empty() {
        return Reply::say(format!("I can't find \"{name}\" running. Either it's closed, or it's hiding from me. Smart program.")).mood(Mood::Think);
    }
    let mb = kb.iter().sum::<u64>() as f64 / 1024.0;
    let verdict = if mb > 2000.0 { "That's a lot. It's basically renting your RAM." } else if mb > 500.0 { "Hungry, but not criminal." } else { "Light as a paperclip." };
    Reply::say(format!("🧠 {name} is using about {mb:.0} MB of RAM across {} process(es). {verdict}", kb.len())).mood(Mood::Think).run("Who else is hogging it?", "top")
}

/// The hottest CPU sensor the kernel reports, in °C.
pub fn cpu_temp() -> Reply {
    if cfg!(windows) {
        return Reply::say("Windows keeps the temperature sensors locked away from me. A hardware monitor app can read them; I just get to guess. I guess: warm.").mood(Mood::Think).run("What's making it work?", "top");
    }
    let zones = std::fs::read_dir("/sys/class/thermal").into_iter().flatten().flatten();
    let temps: Vec<(String, f64)> = zones
        .filter_map(|z| {
            let t: f64 = std::fs::read_to_string(z.path().join("temp")).ok()?.trim().parse().ok()?;
            let kind = std::fs::read_to_string(z.path().join("type")).unwrap_or_default().trim().to_string();
            Some((kind, t / 1000.0))
        })
        .filter(|(_, t)| *t > 0.0 && *t < 150.0)
        .collect();
    let cpu = temps.iter().filter(|(k, _)| ["x86_pkg_temp", "cpu", "k10temp", "coretemp", "soc"].iter().any(|n| k.to_lowercase().contains(n))).map(|(_, t)| *t).reduce(f64::max);
    let Some(t) = cpu.or_else(|| temps.iter().map(|(_, t)| *t).reduce(f64::max)) else {
        return Reply::say("I can't find any temperature sensors. Either your computer is very cool, or very secretive.").mood(Mood::Think);
    };
    let verdict = match t as u32 {
        0..=45 => "Chilly. Like my personality.",
        46..=65 => "Normal. Boring, even.",
        66..=80 => "Warm. Working hard, or hardly working?",
        _ => "HOT. You could fry an egg. Please don't fry an egg.",
    };
    Reply::say(format!("🌡 Your CPU is at {t:.0}°C. {verdict}")).mood(if t > 80.0 { Mood::Shock } else { Mood::Think }).run("What's making it work?", "top")
}

/// Which Linux, which kernel.
pub fn sysinfo() -> Reply {
    let icon = if cfg!(windows) { "🪟" } else { "🐧" };
    Reply::say(format!("{icon} You're running {}. Very sophisticated. I'm running on a bent wire.", crate::sys::os_name())).mood(Mood::Cool).run("Show my graphics card", "gpu")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_tools_on_a_copy() {
        assert_eq!(clip_tool("oneline", "MUSTER AG\n  Bahnhofstrasse 1\n8000 ZÜRICH").as_deref(), Some("MUSTER AG Bahnhofstrasse 1 8000 ZÜRICH"));
        assert_eq!(clip_tool("title", "MUSTER AG bahnhofstrasse").as_deref(), Some("Muster Ag Bahnhofstrasse"));
        assert_eq!(clip_tool("dedupe", "a\nb\na").as_deref(), Some("a\nb"));
        assert_eq!(clip_tool("unbase64", "aGVsbG8=").as_deref(), Some("hello"));
        assert_eq!(clip_tool("unbase64", "not base64!"), None);
        assert_eq!(clip_tool("explode", "x"), None);
        assert!(CLIP_TOOLS.iter().all(|(t, _)| clip_tool(t, "aGVsbG8=").is_some()), "every listed tool works");
    }

    #[test]
    fn text_and_number_tools() {
        for t in ["", "a", "ab", "abc", "hello world", "Grüße 👋"] {
            assert_eq!(b64_decode(&b64_encode(t.as_bytes())).unwrap(), t.as_bytes());
        }
        assert_eq!(b64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(b64_text("aGVsbG8gd29ybGQ="), Some("hello world".into()));
        assert_eq!(b64_text("not base64!"), None);
        assert_eq!(slug("Hello, World! 2026"), "hello-world-2026");
        assert_eq!(clean_link("https://x.com/a?utm_source=t&id=5&fbclid=9#top"), Some("https://x.com/a?id=5#top".into()));
        assert_eq!(clean_link("https://x.com/a?utm_source=t"), Some("https://x.com/a".into()));
        assert_eq!(clean_link("https://x.com/a?id=5"), None);
        assert_eq!((roman(2024), roman(3999), roman(0)), (Some("MMXXIV".into()), Some("MMMCMXCIX".into()), None));
        assert_eq!((from_roman("mmxxiv"), from_roman("IIII"), from_roman("hello")), (Some(2024), None, None));
        assert!(bases("255 in hex").unwrap().text.ends_with("0xFF"));
        assert!(bases("0b1010 to decimal").unwrap().text.ends_with("= 10"));
        assert!(bases("mmxxiv to number").unwrap().text.ends_with("= 2024"));
        assert!(bases("5 km to miles").is_none());
        assert!(tip_calc("15% on 48").text.contains("7.20 → total 55.20"));
        assert!(split_bill("120 between 4").text.contains("30.00 each"));
        assert!(pct_change("from 80 to 100").text.contains("+25.0%"));
        assert!(timestamp("0000000000").text.contains("1970") || timestamp("0000000000").text.contains("1969"));
    }


    #[test]
    fn dates() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap(); // a Thursday
        assert_eq!(parse_day("christmas", today).unwrap().0, NaiveDate::from_ymd_opt(2026, 12, 25).unwrap());
        assert_eq!(parse_day("june 3rd", today).unwrap().0, NaiveDate::from_ymd_opt(2027, 6, 3).unwrap());
        assert_eq!(parse_day("3 december", today).unwrap().0, NaiveDate::from_ymd_opt(2026, 12, 3).unwrap());
        assert_eq!(parse_day("friday", today).unwrap().0, NaiveDate::from_ymd_opt(2026, 11, 6).unwrap());
        assert_eq!(parse_day("thursday", today).unwrap().0, NaiveDate::from_ymd_opt(2026, 11, 12).unwrap());
        assert_eq!(parse_day("2027-01-01", today).unwrap().0, NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
        assert!(parse_day("banana", today).is_none());
        assert_eq!(q("it's"), r"'it'\''s'");
        assert_eq!(uuid().text.lines().nth(1).unwrap().len(), 36);
    }
}
