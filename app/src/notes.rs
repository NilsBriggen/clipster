//! Notes: `notes.txt` in the data folder, one note per line. A line may start with the day it was written and a
//! pin mark: `2026-11-09 * | printer code 4711` (pinned) or `2026-11-09 | gerald says hi`; older plain lines still read.
//! Changes come in as `brain::Effect::Note` and are applied by the app, so tests never touch the real file.

use chrono::NaiveDate;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Note {
    pub day: Option<NaiveDate>,
    pub pinned: bool,
    pub text: String,
}

pub fn path() -> std::path::PathBuf {
    crate::brain::data_dir().join("notes.txt")
}

pub fn parse(text: &str) -> Vec<Note> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let head = l.get(..10).and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
            match (head, l.get(10..)) {
                (Some(day), Some(rest)) if rest.starts_with(" * | ") => Note { day: Some(day), pinned: true, text: rest[5..].to_string() },
                (Some(day), Some(rest)) if rest.starts_with(" | ") => Note { day: Some(day), pinned: false, text: rest[3..].to_string() },
                _ => Note { day: None, pinned: false, text: l.trim_end().to_string() },
            }
        })
        .collect()
}

pub fn format(notes: &[Note]) -> String {
    notes
        .iter()
        .map(|n| match n.day {
            Some(d) => format!("{d}{} | {}\n", if n.pinned { " *" } else { "" }, n.text),
            None => format!("{}\n", n.text), // pinning always dates a note, so only unpinned ones get here
        })
        .collect()
}

pub fn load() -> Vec<Note> {
    std::fs::read_to_string(path()).map(|t| parse(&t)).unwrap_or_default()
}

/// Pinned notes first, then newest first: (number in the file, 1-based, note).
pub fn ordered(notes: &[Note]) -> Vec<(usize, &Note)> {
    let mut v: Vec<(usize, &Note)> = notes.iter().enumerate().map(|(i, n)| (i + 1, n)).collect();
    v.sort_by_key(|(i, n)| (!n.pinned, std::cmp::Reverse(*i)));
    v
}

/// "add <text>", "remove <n>", "edit <n> <text>", "pin <n>" (pin again to unpin), "clear".
/// Returns the status text, or None when the command is wrong.
pub fn change(notes: &mut Vec<Note>, cmd: &str, today: NaiveDate) -> Option<String> {
    let (verb, rest) = cmd.split_once(' ').unwrap_or((cmd, ""));
    let index = |s: &str, len: usize| s.trim().trim_start_matches('#').parse::<usize>().ok().and_then(|n| n.checked_sub(1)).filter(|i| *i < len);
    match verb {
        "add" if !rest.trim().is_empty() => {
            notes.push(Note { day: Some(today), pinned: false, text: one_line(rest) });
            Some(format!("Noted: \"{}\"", one_line(rest)))
        }
        "remove" => {
            let i = index(rest, notes.len())?;
            Some(format!("Gone: \"{}\"", notes.remove(i).text))
        }
        "edit" => {
            let (n, text) = rest.split_once(' ')?;
            let i = index(n, notes.len()).filter(|_| !text.trim().is_empty())?;
            notes[i].text = one_line(text);
            Some("Note changed.".into())
        }
        "pin" => {
            let i = index(rest, notes.len())?;
            notes[i].pinned = !notes[i].pinned;
            notes[i].day.get_or_insert(today);
            Some(if notes[i].pinned { "Pinned to the top." } else { "Unpinned." }.into())
        }
        "clear" => {
            notes.clear();
            Some(String::new())
        }
        _ => None,
    }
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_round_trip_and_change() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        let mut notes = parse("printer code 4711\n\n2026-11-02 * | wifi: hunter2\n2026-11-03 | gerald | says hi\n");
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[0], Note { day: None, pinned: false, text: "printer code 4711".into() }, "old plain lines still read");
        assert!(notes[1].pinned && notes[1].day.is_some());
        assert_eq!(notes[2].text, "gerald | says hi", "a bar in the text stays text");
        assert_eq!(parse(&format(&notes)), notes);
        assert_eq!(change(&mut notes, "add  buy\nmilk ", today).as_deref(), Some("Noted: \"buy milk\""));
        assert_eq!(notes[3], Note { day: Some(today), pinned: false, text: "buy milk".into() });
        assert!(change(&mut notes, "edit 1 printer code 4712", today).is_some());
        assert_eq!(notes[0].text, "printer code 4712");
        change(&mut notes, "pin 1", today);
        assert!(notes[0].pinned && notes[0].day == Some(today), "pinning an old note dates it, so the pin is saved");
        assert_eq!(ordered(&notes).iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![2, 1, 4, 3], "pinned first, newest first");
        assert!(change(&mut notes, "remove 9", today).is_none());
        assert!(change(&mut notes, "edit 2 ", today).is_none(), "an empty edit changes nothing");
        assert_eq!(change(&mut notes, "remove 2", today).as_deref(), Some("Gone: \"wifi: hunter2\""));
        assert_eq!(parse(&format(&notes)), notes);
        assert_eq!(parse("2026-13-40 | not a date\n")[0].day, None, "a bad date is just text");
    }
}
