//! Writing help: an offline proofreader for what you copied, and letter/email templates (`## template <id>`).
//! Rules only: a list of common misspellings, a few grammar patterns, sentence length and a readability score.

use crate::brain::{Act, Effect, Mood, Reply};
use crate::lines;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Issue {
    Doubled(String),
    Misspelled(String, String),
    AAn(String),
    Its,
    Your,
    Then,
    Spaces,
    Capital,
    Shouting,
    Long(usize),
    Hard,
}

/// "über" -> "Über": the first letter in capitals, whatever its size in bytes.
pub fn cap_first(word: &str) -> String {
    let mut c = word.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// The `misspellings` data pool: wrong -> right.
pub fn misspellings() -> HashMap<String, String> {
    lines::all("misspellings").iter().filter_map(|l| l.split_once(" | ").map(|(w, r)| (w.trim().to_lowercase(), r.trim().to_string()))).collect()
}

fn bare(w: &str) -> String {
    w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'').to_lowercase()
}

/// "an hour", "a unicorn": h-words that sound like vowels, and vowel letters that sound like consonants.
fn wants_an(word: &str) -> Option<bool> {
    let w = bare(word);
    let first = w.chars().next()?;
    if !first.is_alphabetic() {
        return None;
    }
    // "an MRI", "an FAQ", "a 4K screen": how acronyms and numbers sound isn't in their letters
    let letters = word.trim_matches(|c: char| !c.is_alphanumeric());
    if letters.chars().count() >= 2 && letters.chars().all(|c| c.is_uppercase() || c.is_ascii_digit()) || letters.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if ["hour", "honest", "honor", "honour", "heir"].iter().any(|h| w.starts_with(h)) {
        return Some(true);
    }
    if ["one", "once", "uni", "use", "usu", "uti", "euro", "eu", "ur"].iter().any(|p| w.starts_with(p)) || first == 'u' {
        return None; // depends on how it sounds: leave it alone
    }
    Some("aeio".contains(first))
}

/// Words that are fine twice in a row ("knock knock", "so so", "very very").
const DOUBLES: &[&str] = &["had", "that", "is", "ha", "bye", "no", "very", "so", "knock", "bla", "blah", "la", "ho", "hey", "yeah", "really", "far", "many", "more", "again", "go", "tsk", "tut", "chop", "night", "boo", "yum", "choo", "now", "well"];

/// Everything that looks off in `text`, most useful first.
pub fn proofread(text: &str, wrong: &HashMap<String, String>) -> Vec<Issue> {
    let mut out = vec![];
    let words: Vec<&str> = text.split_whitespace().collect();
    for pair in words.windows(2) {
        let (a, b) = (bare(pair[0]), bare(pair[1]));
        if a == b && !a.is_empty() && !a.chars().all(|c| c.is_ascii_digit()) && !DOUBLES.contains(&a.as_str()) && !pair[0].ends_with(['.', ',', ';', ':', '!', '?']) {
            out.push(Issue::Doubled(a.clone()));
        }
        match (a.as_str(), wants_an(pair[1])) {
            ("a", Some(true)) | ("an", Some(false)) => out.push(Issue::AAn(pair[1].trim_matches(|c: char| !c.is_alphanumeric()).to_string())),
            _ => {}
        }
        let followers = ["a", "an", "the", "been", "not", "going", "just", "very", "so", "time", "too", "really", "gonna"];
        if a == "its" && followers.contains(&b.as_str()) || a == "it's" && b == "own" {
            out.push(Issue::Its);
        }
        if a == "your" && ["welcome", "right", "wrong", "going", "not", "a", "the", "very", "so", "being", "gonna", "late"].contains(&b.as_str()) {
            out.push(Issue::Your);
        }
        if b == "then" && (["more", "less", "rather", "other", "better", "worse", "fewer", "greater", "bigger", "smaller", "faster", "slower", "older", "younger"].contains(&a.as_str())) {
            out.push(Issue::Then);
        }
    }
    let mut seen = std::collections::HashSet::new();
    for w in &words {
        let b = bare(w);
        if let Some(right) = wrong.get(&b).filter(|_| seen.insert(b.clone())) {
            out.push(Issue::Misspelled(b, right.clone()));
        }
    }
    if text.lines().any(|l| l.trim_start().contains("  ")) {
        out.push(Issue::Spaces);
    }
    // a lowercase letter starting a sentence (not after "e.g." and friends)
    let abbrev = ["e.g.", "i.e.", "etc.", "vs.", "approx.", "dr.", "mr.", "mrs.", "ms.", "no."];
    if words.windows(2).any(|p| p[0].ends_with('.') && !abbrev.contains(&p[0].to_lowercase().as_str()) && p[1].chars().next().is_some_and(|c| c.is_lowercase())) {
        out.push(Issue::Capital);
    }
    if text.contains("!!") || text.contains("??") || words.iter().filter(|w| w.len() >= 5 && w.chars().all(|c| c.is_uppercase() || !c.is_alphabetic()) && w.chars().any(|c| c.is_alphabetic())).count() >= 2 {
        out.push(Issue::Shouting);
    }
    let sentences: Vec<usize> = text.split(['.', '!', '?']).map(|s| s.split_whitespace().count()).filter(|n| *n > 0).collect();
    if let Some(&n) = sentences.iter().filter(|n| **n > 35).max() {
        out.push(Issue::Long(n));
    }
    if words.len() >= 30 && readability(text) < 30.0 {
        out.push(Issue::Hard);
    }
    out
}

/// Flesch reading ease (higher is easier; 60-70 is plain English).
pub fn readability(text: &str) -> f32 {
    let words: Vec<String> = text.split_whitespace().map(bare).filter(|w| !w.is_empty()).collect();
    let sentences = text.split(['.', '!', '?']).filter(|s| !s.trim().is_empty()).count().max(1) as f32;
    let syllables: usize = words.iter().map(|w| syllables(w)).sum();
    let n = words.len().max(1) as f32;
    206.835 - 1.015 * (n / sentences) - 84.6 * (syllables as f32 / n)
}

fn syllables(w: &str) -> usize {
    let mut count = 0;
    let mut prev = false;
    for c in w.chars() {
        let v = "aeiouy".contains(c);
        if v && !prev {
            count += 1;
        }
        prev = v;
    }
    if w.ends_with('e') && count > 1 {
        count -= 1; // the silent e
    }
    count.max(1)
}

/// The safe fixes only: doubled words, listed misspellings, double spaces, a/an, capitals after a full stop.
pub fn fix(text: &str, wrong: &HashMap<String, String>) -> String {
    let mut out: Vec<String> = vec![];
    let mut cap = true;
    for line in text.split('\n') {
        let indent = &line[..line.len() - line.trim_start().len()]; // code, lists and poems keep their indentation
        let mut fixed: Vec<String> = vec![];
        for w in line.split(' ').filter(|w| !w.is_empty()) {
            let b = bare(w);
            let doubled = !b.is_empty() && !b.chars().all(|c| c.is_ascii_digit()) && !DOUBLES.contains(&b.as_str());
            if doubled && fixed.last().is_some_and(|p| bare(p) == b && !p.ends_with(['.', ',', ';', ':', '!', '?'])) {
                // the dropped word's full stop or comma stays: "the the." -> "the."
                let tail = &w[w.trim_end_matches(|c: char| !c.is_alphanumeric()).len()..];
                if let Some(p) = fixed.last_mut() {
                    p.push_str(tail);
                }
                continue;
            }
            let mut word = match wrong.get(&b) {
                Some(right) => {
                    let start = w.find(|c: char| c.is_alphanumeric()).unwrap_or(0);
                    let end = w.rfind(|c: char| c.is_alphanumeric() || c == '\'').map_or(w.len(), |i| i + w[i..].chars().next().map_or(1, char::len_utf8));
                    let right = if w[start..].starts_with(|c: char| c.is_uppercase()) { cap_first(right) } else { right.clone() };
                    format!("{}{right}{}", &w[..start], &w[end..])
                }
                None => w.to_string(),
            };
            if cap && word.starts_with(|c: char| c.is_lowercase()) && !fixed.is_empty() {
                word = cap_first(&word);
            }
            if let (Some(prev), Some(an)) = (fixed.last_mut(), wants_an(&word)) {
                match (prev.as_str(), an) {
                    ("a", true) => *prev = "an".into(),
                    ("A", true) => *prev = "An".into(),
                    ("an", false) => *prev = "a".into(),
                    ("An", false) => *prev = "A".into(),
                    _ => {}
                }
            }
            cap = word.ends_with(['.', '!', '?']) && !["e.g.", "i.e.", "etc.", "vs."].contains(&word.to_lowercase().as_str());
            fixed.push(word);
        }
        out.push(format!("{indent}{}", fixed.join(" ")));
    }
    out.join("\n")
}

/// "proofread" (what you copied, or text after the command): the issues, and a button to copy a fixed version.
pub fn check(text: &str) -> Reply {
    let text = crate::brain::or_clipboard(text);
    if text.trim().is_empty() {
        return Reply::say("Copy some text first (or type: proofread <text>), and I'll go through it with my red pen.").mood(Mood::Think);
    }
    let wrong = misspellings();
    let issues = proofread(&text, &wrong);
    let fixable = fix(&text, &wrong) != text;
    if issues.is_empty() {
        let easy = text.split_whitespace().count() >= 30 && readability(&text) > 70.0;
        let r = Reply::say(format!("{}{}", lines::pick("proof_clean"), if easy { format!(" {}", lines::pick("proof_easy")) } else { String::new() }));
        return if fixable { r.mood(Mood::Happy).run("Tidy the spacing", "proofread fix") } else { r.mood(Mood::Happy) };
    }
    let say = |pool: &str, x: &str, y: &str| lines::pick(pool).replace("{x}", x).replace("{y}", y).replace("{n}", x);
    let mut text_out = lines::pick("proof_intro").replace("{n}", &issues.len().to_string());
    for i in issues.iter().take(6) {
        let line = match i {
            Issue::Doubled(w) => say("proof_doubled", w, ""),
            Issue::Misspelled(w, r) => say("proof_misspelled", w, r),
            Issue::AAn(w) => say("proof_a_an", w, ""),
            Issue::Its => say("proof_its", "", ""),
            Issue::Your => say("proof_your", "", ""),
            Issue::Then => say("proof_then", "", ""),
            Issue::Spaces => say("proof_spaces", "", ""),
            Issue::Capital => say("proof_capital", "", ""),
            Issue::Shouting => say("proof_shouting", "", ""),
            Issue::Long(n) => say("proof_long", &n.to_string(), ""),
            Issue::Hard => say("proof_hard", "", ""),
        };
        text_out += &format!("\n• {line}");
    }
    let r = Reply::say(text_out).mood(Mood::Think);
    let r = if fixable { r.run("Fix the easy ones", "proofread fix") } else { r };
    r.run("Check again", "proofread")
}

/// One issue in plain words, for the Desk's Write tab.
pub fn issue_text(i: &Issue) -> String {
    let say = |pool: &str, x: &str, y: &str| lines::pick(pool).replace("{x}", x).replace("{y}", y).replace("{n}", x);
    match i {
        Issue::Doubled(w) => say("proof_doubled", w, ""),
        Issue::Misspelled(w, r) => say("proof_misspelled", w, r),
        Issue::AAn(w) => say("proof_a_an", w, ""),
        Issue::Its => say("proof_its", "", ""),
        Issue::Your => say("proof_your", "", ""),
        Issue::Then => say("proof_then", "", ""),
        Issue::Spaces => say("proof_spaces", "", ""),
        Issue::Capital => say("proof_capital", "", ""),
        Issue::Shouting => say("proof_shouting", "", ""),
        Issue::Long(n) => say("proof_long", &n.to_string(), ""),
        Issue::Hard => say("proof_hard", "", ""),
    }
}

/// Whether "Fix" can do it safely (the rest are hints for you to judge).
pub fn fixable(i: &Issue) -> bool {
    matches!(i, Issue::Doubled(_) | Issue::Misspelled(..) | Issue::AAn(_) | Issue::Spaces | Issue::Capital)
}

/// Apply one safe fix to the text (the Write tab's "Fix" next to an issue).
pub fn fix_one(text: &str, issue: &Issue) -> String {
    match issue {
        Issue::Misspelled(w, right) => text
            .split_inclusive(|c: char| c.is_whitespace())
            .map(|tok| {
                let core = tok.trim_end();
                if bare(core) == *w {
                    let start = core.find(|c: char| c.is_alphanumeric()).unwrap_or(0);
                    let end = core.rfind(|c: char| c.is_alphanumeric() || c == '\'').map_or(core.len(), |i| i + core[i..].chars().next().map_or(1, char::len_utf8));
                    let r = if core[start..].starts_with(|c: char| c.is_uppercase()) { cap_first(right) } else { right.clone() };
                    format!("{}{r}{}{}", &core[..start], &core[end..], &tok[core.len()..])
                } else {
                    tok.to_string()
                }
            })
            .collect(),
        Issue::Doubled(_) | Issue::AAn(_) | Issue::Spaces | Issue::Capital => {
            // the safe fixer, with only the misspellings left out (each has its own button)
            fix(text, &HashMap::new())
        }
        _ => text.to_string(),
    }
}

/// The `[blanks]` of a template, each once, in order: "[Recipient]", "[the product]".
pub fn fields(template: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut rest = template;
    while let Some(a) = rest.find('[') {
        let Some(b) = rest[a..].find(']') else { break };
        let f = rest[a + 1..a + b].to_string();
        if !f.is_empty() && f.len() < 60 && !out.contains(&f) {
            out.push(f);
        }
        rest = &rest[a + b + 1..];
    }
    out
}

/// A template with the blanks you filled in (the empty ones stay as they are).
pub fn fill_fields(template: &str, values: &std::collections::BTreeMap<String, String>) -> String {
    values.iter().filter(|(_, v)| !v.trim().is_empty()).fold(template.to_string(), |t, (k, v)| t.replace(&format!("[{k}]"), v.trim()))
}

/// Every letter template: (id, name), yours first.
pub fn templates() -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = lines::sections_with_prefix("template ").into_iter().map(|(id, _)| (id.clone(), id.replace('_', " "))).collect();
    all.sort_by_key(|(id, _)| !id.starts_with("my_"));
    all
}

/// "Fix the easy ones": the fixed text goes onto the clipboard.
pub fn fix_clipboard() -> Reply {
    let text = crate::brain::or_clipboard("");
    let fixed = fix(&text, &misspellings());
    if fixed == text {
        return Reply::say("Nothing I can safely fix on my own. The rest needs your judgement.");
    }
    Reply::say(lines::pick("proof_fixed")).mood(Mood::Happy).effect(Effect::CopyNow(fixed))
}

/// Template ids with their keywords (`## template <id>` sections).
pub fn template_for(sentence: &str) -> Option<String> {
    let s = format!(" {} ", sentence.to_lowercase());
    lines::sections_with_prefix("template ")
        .into_iter()
        .filter_map(|(id, triggers)| triggers.iter().filter(|t| s.contains(&format!(" {} ", t.to_lowercase()))).map(|t| t.len()).max().map(|n| (n, id)))
        .max_by_key(|(n, _)| *n)
        .map(|(_, id)| id)
}

/// A filled-in template: today's date in, `{name}` left for the app to fill.
pub fn fill(template: &str, date: &str) -> String {
    template.replace("{date}", date)
}

/// "letter complaint": a short preview, Copy, Open as email.
pub fn letter(kind: &str) -> Reply {
    let all = lines::sections_with_prefix("template ");
    let id = all.iter().map(|(id, _)| id.clone()).find(|id| *id == kind.replace(' ', "_")).or_else(|| template_for(kind));
    let Some(id) = id else {
        let mut r = Reply::say(lines::pick("template_choose")).mood(Mood::Think);
        for (id, _) in all.iter().take(12) {
            r = r.run(&id.replace('_', " "), &format!("letter {id}"));
        }
        return r;
    };
    let Some(template) = lines::all(&format!("template {id}")).first().cloned() else { return Reply::say("I lost that template somewhere in the swap file.") };
    let full = fill(&template, &chrono::Local::now().format("%B %-d, %Y").to_string());
    let (subject, body) = full.split_once("\n\n").map(|(s, b)| (s.trim_start_matches("Subject: ").to_string(), b.to_string())).unwrap_or_default();
    let preview: String = body.lines().filter(|l| !l.trim().is_empty()).take(3).collect::<Vec<_>>().join("\n");
    Reply::say(format!("{}\n\n✉ {subject}\n{preview}\n…", lines::pick("template_intro")))
        .mood(Mood::Happy)
        .btn("Copy it", Act::Copy(full.clone()))
        .btn("Open as email", Act::Open(format!("mailto:?subject={}&body={}", crate::brain::mail_encode(&subject), crate::brain::mail_encode(&body))))
        .run("Another kind of letter", "letter")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrong() -> HashMap<String, String> {
        [("recieve", "receive"), ("definately", "definitely")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn finds_the_usual_suspects() {
        let i = proofread("I will definately recieve the the package. its a big one and an box.", &wrong());
        assert!(i.contains(&Issue::Doubled("the".into())));
        assert!(i.contains(&Issue::Misspelled("definately".into(), "definitely".into())));
        assert!(i.contains(&Issue::Misspelled("recieve".into(), "receive".into())));
        assert!(i.contains(&Issue::Its));
        assert!(i.contains(&Issue::AAn("box".into())));
        assert!(i.contains(&Issue::Capital), "a sentence starting lowercase");
        assert!(proofread("Your welcome. This is better then that.", &wrong()).contains(&Issue::Then));
        assert!(proofread("It takes an hour. A unicorn helps. I had had enough.", &wrong()).is_empty(), "no false alarms: {:?}", proofread("It takes an hour. A unicorn helps. I had had enough.", &wrong()));
        assert!(proofread("STOP DOING THAT!!", &wrong()).contains(&Issue::Shouting));
    }

    #[test]
    fn fixes_only_what_is_safe() {
        assert_eq!(fix("i will definately  recieve the the parcel. it is an box.", &wrong()), "i will definitely receive the parcel. It is a box.");
        assert_eq!(fix("Definately. A apple a day.", &wrong()), "Definitely. An apple a day.");
        assert_eq!(fix("Its fine.", &wrong()), "Its fine.", "its/it's needs a human");
    }

    #[test]
    fn accents_dont_break_the_fixes() {
        assert_eq!(fix("Hello there. élan is fine. über cool", &wrong()), "Hello there. Élan is fine. Über cool");
        let _ = proofread("Ça va. über alles", &wrong()); // only that it doesn't panic
    }

    #[test]
    fn readability_scores_go_the_right_way() {
        assert!(readability("The cat sat on the mat. It was warm. We had tea.") > readability("Notwithstanding considerable organisational heterogeneity, implementation necessitates comprehensive interdepartmental coordination."));
    }

    #[test]
    fn the_real_templates_and_misspellings_are_usable() {
        let ids: Vec<String> = lines::sections_with_prefix("template ").into_iter().map(|(id, _)| id).collect();
        for id in ["cover_letter", "complaint", "thank_you", "resignation", "out_of_office", "apology", "invitation", "follow_up", "meeting_request", "sick_day", "reference_request", "landlord"] {
            assert!(ids.iter().any(|i| i == id), "no template {id}");
            let t = lines::all(&format!("template {id}"));
            assert_eq!(t.len(), 1, "{id}: one line, the whole template");
            assert!(t[0].starts_with("Subject: ") && t[0].contains("\n\n") && t[0].contains("{name}"), "{id}: a subject, a body and a signature");
        }
        assert_eq!(template_for("write a complaint letter").as_deref(), Some("complaint"));
        assert_eq!(template_for("i need a cover letter for a job").as_deref(), Some("cover_letter"));
        let wrong = misspellings();
        assert!(wrong.len() >= 300, "only {} misspellings", wrong.len());
        let loops: Vec<&String> = wrong.iter().filter(|(w, r)| **w == r.to_lowercase() || wrong.contains_key(&r.to_lowercase())).map(|(w, _)| w).collect();
        assert!(loops.is_empty(), "fixes that are themselves on the list: {loops:?}");
        assert!(proofread("I definately agree.", &wrong).contains(&Issue::Misspelled("definately".into(), "definitely".into())));
    }

    #[test]
    fn templates_get_the_date() {
        assert_eq!(fill("Dear [Name],\n{date}\n{name}", "May 3, 2026"), "Dear [Name],\nMay 3, 2026\n{name}");
    }

    #[test]
    fn one_fix_at_a_time_and_template_blanks() {
        let w = wrong();
        let t = "I will Definately recieve it.";
        let issues = proofread(t, &w);
        let fixed = fix_one(t, &Issue::Misspelled("definately".into(), "definitely".into()));
        assert_eq!(fixed, "I will Definitely recieve it.", "only that word, keeping its capital");
        assert!(issues.iter().any(|i| fixable(i)));
        assert_eq!(fix_one("the the cat", &Issue::Doubled("the".into())), "the cat");
        assert!(!fixable(&Issue::Its), "its/it's is only ever a hint");
        let tpl = "Dear [Recipient],\n\nAbout [the product] ([order number]), [Recipient]…";
        assert_eq!(fields(tpl), vec!["Recipient", "the product", "order number"]);
        let mut v = std::collections::BTreeMap::new();
        v.insert("Recipient".to_string(), "Ms Keller".to_string());
        v.insert("order number".to_string(), " ".to_string());
        assert_eq!(fill_fields(tpl, &v), "Dear Ms Keller,\n\nAbout [the product] ([order number]), Ms Keller…");
    }
}
