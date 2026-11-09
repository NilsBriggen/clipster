//! Games and silly-but-handy text tricks.

use crate::brain::{Act, Mood, Reply};
use crate::lines;
use rand::seq::SliceRandom;

pub fn games() -> Reply {
    let mut r = Reply::say("Let's play! Pick a game. The arcade ones open in their own window:").mood(Mood::Happy);
    for (id, title) in crate::games::NAMES {
        r = r.run(&format!("🕹 {title}"), &format!("play {id}"));
    }
    r
        .run("Quiz me (trivia)", "quiz")
        .run("Rock, paper, scissors", "rps")
        .run("I'll guess your number", "guess")
        .run("Ask the magic 8-ball", "8ball")
        .run("Roll the dice", "roll 2d6")
        .run("Flip a coin", "flip")
}

pub fn rps(choice: &str) -> Reply {
    const MOVES: [&str; 3] = ["rock", "paper", "scissors"];
    let Some(you) = MOVES.iter().find(|m| choice.contains(*m)) else {
        return Reply::say("Rock, paper or scissors? Choose wisely...").mood(Mood::Sly).run("🪨 Rock", "rps rock").run("📄 Paper", "rps paper").run("✂ Scissors", "rps scissors");
    };
    let me = MOVES.choose(&mut rand::thread_rng()).unwrap_or(&"rock");
    let beats = |a: &str, b: &str| matches!((a, b), ("rock", "scissors") | ("paper", "rock") | ("scissors", "paper"));
    let (text, mood) = if *you == *me {
        ("A tie! Great minds think alike. Mine is just bendier.", Mood::Think)
    } else if beats(you, me) {
        ("You win! 🎉 I demand a rematch.", Mood::Shock)
    } else {
        ("I win! 😎 Paperclips: 1, Humans: 0.", Mood::Spin)
    };
    Reply::say(format!("You: {you}. Me: {me}.\n{text}")).mood(mood).run("🪨 Rock", "rps rock").run("📄 Paper", "rps paper").run("✂ Scissors", "rps scissors")
}

/// Clipster guesses *your* number by halving: "guess" starts, "guess LO HI TRIES" continues, "guess win TRIES" ends.
pub fn guess(rest: &str) -> Reply {
    let w: Vec<&str> = rest.split_whitespace().collect();
    if let ["win", tries] = w[..] {
        return Reply::say(format!("WOOHOO! Got it in {tries} tries! 🧠 Paperclip intelligence at its finest.")).mood(Mood::Spin).run("Play again", "guess");
    }
    let n: Vec<i64> = w.iter().filter_map(|x| x.parse().ok()).collect();
    let [lo, hi, tries] = n[..] else {
        return Reply::say("Think of a number between 1 and 100. Don't tell me! I'll read your mind... with math.").mood(Mood::Sly).run("OK, I've got one!", "guess 1 100 1");
    };
    // typed numbers: kept to a range where the maths can't overflow
    let (lo, hi, tries) = (lo.clamp(-1_000_000_000, 1_000_000_000), hi.clamp(-1_000_000_000, 1_000_000_000), tries.clamp(0, 1000));
    if lo > hi {
        return Reply::say("Wait a minute... that's impossible! 🤨 Did you change your number? CHEATER!").mood(Mood::Shock).run("OK, let's play fair", "guess");
    }
    let mid = lo + (hi - lo) / 2;
    Reply::say(format!("Guess #{tries}: is it {mid}?"))
        .mood(Mood::Think)
        .run("Higher ⬆", &format!("guess {} {hi} {}", mid + 1, tries + 1))
        .run("Lower ⬇", &format!("guess {lo} {} {}", mid - 1, tries + 1))
        .run("Yes, that's it! 🎯", &format!("guess win {tries}"))
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// "riddle" asks one; its button reveals the answer.
pub fn riddle() -> Reply {
    let line = lines::pick("riddles");
    let (q, a) = line.split_once(" | ").unwrap_or((&line, "nobody knows"));
    Reply::say(format!("🧩 {q}")).mood(Mood::Think).run("Tell me!", &format!("reveal {a}")).run("Another one", "riddle")
}

pub fn reveal(answer: &str) -> Reply {
    Reply::say(format!("💡 {}. Obviously. Did you get it? Don't lie to a paperclip.", cap(answer))).mood(Mood::Laugh).run("Another riddle", "riddle")
}

/// Would you rather: two buttons, and Clipster judges your pick.
pub fn wyr() -> Reply {
    let line = lines::pick("wyr");
    let (a, b) = line.split_once(" | ").unwrap_or((&line, "neither"));
    Reply::say(format!("🤔 Would you rather {a}...\n...or {b}?")).mood(Mood::Sly).run(&cap(a), &format!("wyrpick {a}")).run(&cap(b), &format!("wyrpick {b}"))
}

pub fn wyr_pick(choice: &str) -> Reply {
    Reply::say(lines::pick("wyr_react").replace("{choice}", &cap(choice))).mood(Mood::Laugh).run("Another one!", "wyr")
}

/// Emojis in, three titles to choose from (one right, two from other puzzles).
pub fn emoji_quiz() -> Reply {
    let all = lines::all("emoji_quiz");
    let mut rng = rand::thread_rng();
    let Some((emoji, answer)) = all.choose(&mut rng).and_then(|l| l.split_once(" | ")) else {
        return Reply::say("I lost all my emojis. 😶");
    };
    let mut others: Vec<&str> = all.iter().filter_map(|l| l.split_once(" | ").map(|x| x.1)).filter(|o| *o != answer).collect();
    others.sort_unstable();
    others.dedup(); // two equal wrong answers would make two identical buttons
    let mut options: Vec<&str> = others.choose_multiple(&mut rng, 2).copied().collect();
    options.push(answer);
    options.shuffle(&mut rng);
    let mut r = Reply::say(format!("What's this?\n\n{emoji}")).mood(Mood::Think);
    for o in options {
        r = r.run(&cap(o), &format!("answer {o}|{answer}|emoji quiz"));
    }
    r
}

// ---------- text tricks ----------

pub fn upside_down(t: &str) -> String {
    const FROM: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,!?'\"()[]{}<>_&";
    const TO: &str = "ɐqɔpǝɟƃɥᴉɾʞlɯuodbɹsʇnʌʍxʎz∀ꓭƆꓷƎℲ⅁HIſꓘ⅂WNOԀΌꓤS⊥∩ΛMX⅄Z0ƖᄅƐㄣϛ9ㄥ86˙'¡¿,„)(][}{><‾⅋";
    let map: Vec<(char, char)> = FROM.chars().zip(TO.chars()).collect();
    t.chars().rev().map(|c| map.iter().find(|(a, _)| *a == c).map_or(c, |(_, b)| *b)).collect()
}

const MORSE: &[(char, &str)] = &[
    ('a', ".-"), ('b', "-..."), ('c', "-.-."), ('d', "-.."), ('e', "."), ('f', "..-."), ('g', "--."), ('h', "...."), ('i', ".."),
    ('j', ".---"), ('k', "-.-"), ('l', ".-.."), ('m', "--"), ('n', "-."), ('o', "---"), ('p', ".--."), ('q', "--.-"), ('r', ".-."),
    ('s', "..."), ('t', "-"), ('u', "..-"), ('v', "...-"), ('w', ".--"), ('x', "-..-"), ('y', "-.--"), ('z', "--.."), ('0', "-----"),
    ('1', ".----"), ('2', "..---"), ('3', "...--"), ('4', "....-"), ('5', "....."), ('6', "-...."), ('7', "--..."), ('8', "---.."),
    ('9', "----."), ('.', ".-.-.-"), (',', "--..--"), ('?', "..--.."), ('!', "-.-.--"),
];

/// Text -> morse, or morse (dots, dashes, spaces, slashes) -> text.
pub fn morse(t: &str) -> String {
    if t.chars().all(|c| ".-/ ".contains(c)) {
        return t.split(" / ").map(|w| w.split_whitespace().map(|code| MORSE.iter().find(|(_, m)| *m == code).map_or('?', |(c, _)| *c)).collect::<String>()).collect::<Vec<_>>().join(" ");
    }
    t.to_lowercase().split_whitespace().map(|w| w.chars().filter_map(|c| MORSE.iter().find(|(m, _)| *m == c).map(|(_, code)| *code)).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join(" / ")
}

/// Text -> 8-bit binary, or binary -> text.
pub fn binary(t: &str) -> String {
    if t.chars().all(|c| "01 ".contains(c)) {
        let bytes: Vec<u8> = t.split_whitespace().filter_map(|b| u8::from_str_radix(b, 2).ok()).collect();
        return String::from_utf8_lossy(&bytes).into_owned();
    }
    t.bytes().map(|b| format!("{b:08b}")).collect::<Vec<_>>().join(" ")
}

pub fn leet(t: &str) -> String {
    t.chars().map(|c| match c.to_ascii_lowercase() { 'a' => '4', 'e' => '3', 'i' => '1', 'o' => '0', 's' => '5', 't' => '7', 'b' => '8', 'g' => '9', _ => c }).collect()
}

pub fn rot13(t: &str) -> String {
    t.chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a' + 13) % 26) + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A' + 13) % 26) + b'A') as char,
            _ => c,
        })
        .collect()
}

/// One reply for every trick: the result plus a copy button.
pub fn trick(kind: &str, t: &str) -> Reply {
    if t.trim().is_empty() {
        return Reply::say(format!("{kind} what? Try: {kind} hello world"));
    }
    let out = match kind {
        "upsidedown" => upside_down(t),
        "morse" => morse(t),
        "binary" => binary(t),
        "leet" => leet(t),
        _ => rot13(t),
    };
    Reply::say(out.clone()).mood(Mood::Sly).btn("Copy it", Act::Copy(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tricks() {
        assert_eq!(morse("SOS hi"), "... --- ... / .... ..");
        assert_eq!(morse("... --- ... / .... .."), "sos hi");
        assert_eq!(binary("Hi"), "01001000 01101001");
        assert_eq!(binary("01001000 01101001"), "Hi");
        assert_eq!(rot13(&rot13("Hello")), "Hello");
        assert_eq!(upside_down("hi!"), "¡ᴉɥ");
        assert_eq!(leet("elite"), "3l173");
        assert!(guess("1 100 1").text.contains("50"));
        assert!(guess("60 40 3").text.contains("CHEATER"));
        assert!(rps("rock").text.starts_with("You: rock."));
    }
}
