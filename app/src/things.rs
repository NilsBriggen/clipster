//! Where things are: "my passport is in the top drawer", "where is my passport". A small map in settings.json
//! (thing → place, and the day you said it). It never leaves the computer, and it never keeps codes or passwords.
//! Changes come in as `brain::Effect::Things`.

use std::collections::BTreeMap;

/// thing → (place, "2026-11-09")
pub type Things = BTreeMap<String, (String, String)>;

/// Words that make a thing a secret: kept in a password manager, not here.
const SECRETS: &[&str] = &["pin", "pin code", "password", "passcode", "passwort", "puk", "combination", "safe code", "door code", "alarm code"];
/// "pin" is a secret only on its own or with these ("bank pin"), not "rolling pin"
const PIN_OWNERS: &[&str] = &["bank", "card", "phone", "sim", "atm", "debit", "credit"];

/// "my passport", "The spare key" -> "passport", "spare key".
pub fn key(thing: &str) -> String {
    let t = thing.trim().to_lowercase();
    ["my ", "the ", "our ", "a "].iter().fold(t, |t, p| t.strip_prefix(p).map(str::to_string).unwrap_or(t)).trim().to_string()
}

pub fn secret(thing: &str) -> bool {
    let t = key(thing);
    let words: Vec<&str> = t.split_whitespace().collect();
    let pin = words.iter().position(|w| *w == "pin").is_some_and(|i| words.len() == 1 || i > 0 && PIN_OWNERS.contains(&words[i - 1]) || words.get(i + 1) == Some(&"code"));
    pin || SECRETS.iter().filter(|s| **s != "pin").any(|s| t == *s || t.ends_with(&format!(" {s}")) || t.starts_with(&format!("{s} ")))
}

/// "set <thing>|<place>" or "forget <thing>": what to say, and the command that undoes it.
pub fn change(things: &mut Things, cmd: &str, today: &str) -> (String, Option<String>) {
    let (verb, rest) = cmd.split_once(' ').unwrap_or((cmd, ""));
    match verb {
        "set" => {
            let Some((thing, place)) = rest.split_once('|').map(|(t, p)| (key(t), p.trim().to_string())).filter(|(t, p)| !t.is_empty() && !p.is_empty()) else {
                return ("Where is what? Try: my passport is in the top drawer".into(), None);
            };
            if secret(&thing) {
                return (format!("I'd rather not keep your {thing}. A password manager is the place for that."), None);
            }
            let before = things.insert(thing.clone(), (place.clone(), today.to_string()));
            let undo = match before {
                Some((old, _)) => format!("thing set {thing}|{old}"),
                None => format!("thing forget {thing}"),
            };
            (format!("Got it: {thing} → {place}."), Some(undo))
        }
        "forget" => match things.remove(&key(rest)) {
            Some((place, _)) => (format!("Forgotten where the {} is.", key(rest)), Some(format!("thing set {}|{place}", key(rest)))),
            None => (format!("I didn't know where the {} was anyway.", key(rest)), None),
        },
        _ => ("I can keep where things are: my passport is in the top drawer.".into(), None),
    }
}

/// The place of a thing: the same name, or one that holds all its words ("key" finds "spare key" when it's the only one).
pub fn find<'a>(things: &'a Things, thing: &str) -> Option<(&'a String, &'a (String, String))> {
    let k = key(thing);
    things.get_key_value(&k).or_else(|| {
        // whole words ("key" finds "spare key", "monkey" doesn't)
        let words = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
        let kw = words(&k);
        let hits: Vec<_> = things
            .iter()
            .filter(|(t, _)| {
                let tw = words(t);
                kw.iter().all(|w| tw.contains(w)) || tw.iter().all(|w| kw.contains(w))
            })
            .collect();
        (hits.len() == 1).then(|| hits[0])
    })
}

/// What's kept in a place ("blue bag" finds everything "in the blue bag").
pub fn in_place<'a>(things: &'a Things, place: &str) -> Vec<&'a String> {
    let p = key(place);
    things.iter().filter(|(_, (pl, _))| key(pl).trim_start_matches("in ").trim_start_matches("the ").contains(&p)).map(|(t, _)| t).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn where_things_are() {
        let mut t = Things::new();
        assert_eq!(change(&mut t, "set My Passport|in the top drawer", "2026-11-09").1.as_deref(), Some("thing forget passport"));
        assert_eq!(find(&t, "my passport").map(|x| x.1 .0.as_str()), Some("in the top drawer"));
        assert_eq!(change(&mut t, "set passport|in the safe", "2026-11-10").1.as_deref(), Some("thing set passport|in the top drawer"), "undo puts the old place back");
        change(&mut t, "set spare key|with Anna", "2026-11-10");
        change(&mut t, "set charger|in the blue bag", "2026-11-10");
        assert_eq!(find(&t, "key").map(|x| x.0.as_str()), Some("spare key"));
        assert_eq!(in_place(&t, "the blue bag"), vec!["charger"]);
        assert!(change(&mut t, "set bank PIN|1234", "2026-11-10").0.contains("password manager") && !t.contains_key("bank pin"), "never secrets");
        assert!(change(&mut t, "forget the charger", "2026-11-10").1.is_some() && find(&t, "charger").is_none());
        assert!(change(&mut t, "set rolling pin|in the drawer", "2026-11-10").0.starts_with("Got it"), "a rolling pin isn't a secret");
        assert!(secret("pin") && secret("bank pin") && secret("pin code"));
        assert!(find(&t, "monkey").is_none(), "whole words only");
    }
}
