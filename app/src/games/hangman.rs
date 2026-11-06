//! Hangman, office style: every wrong letter unbends the paperclip a little more.

use super::{win98, wire, Frame, Game, Outcome};
use eframe::egui::{pos2, vec2, Align2, Color32, Event, Id, Painter, Pos2, Rect, Shape, Stroke, Ui, Vec2};
use rand::seq::SliceRandom;
use std::f32::consts::PI;

pub const MAX: u8 = 7;
const W: f32 = 460.0;
const ART_H: f32 = 150.0;
const EASE: f32 = 0.4; // seconds the clip takes to unbend one step
const WOBBLE: f32 = 0.5; // seconds it shudders after a miss
const POP: f32 = 0.25; // seconds a found letter takes to drop into its slot
const SHOW_RESULT: f32 = 0.9; // seconds to watch the clip before the result box opens

const FALLBACK: &[&str] = &[
    "STAPLER", "PAPERCLIP", "SPREADSHEET", "PRINTER", "KEYBOARD", "MONITOR", "DEADLINE", "MEETING", "CUBICLE", "COFFEE", "WHITEBOARD", "MEMO",
    "FOLDER", "BINDER", "SHREDDER", "PHOTOCOPIER", "INBOX", "PASSWORD", "DATABASE", "SOFTWARE", "COMPILER", "NETWORK", "ROUTER", "BROWSER",
    "FLOPPY DISK", "DESKTOP", "LAPTOP", "SCANNER", "CALENDAR", "STICKY NOTE", "HIGHLIGHTER", "INTERN", "PAYROLL", "AGENDA", "SCREENSAVER",
];

/// Words from the text database (`hangman_words`), or the built-in office list when it has none.
fn words() -> Vec<String> {
    let ok = |w: &String| (3..=18).contains(&w.len()) && w.chars().any(|c| c.is_ascii_uppercase()) && w.chars().all(|c| c.is_ascii_uppercase() || c == ' ' || c == '-');
    let mine: Vec<String> = crate::lines::all("hangman_words").iter().map(|w| w.trim().to_uppercase()).filter(ok).collect();
    if mine.is_empty() { FALLBACK.iter().map(|w| w.to_string()).collect() } else { mine }
}

pub struct Hang {
    pub word: Vec<char>,
    pub guessed: [bool; 26],
    pub wrong: u8,
    pub streak: u32, // words saved in a row
    record: bool,    // this win beat the saved best streak
    from: f32,       // the clip eases from `from` to wrong / MAX while `k` goes 0 → 1
    k: f32,
    wobble: f32,       // seconds of shudder left after a miss
    pop: f32,          // seconds since the last hit, whose letters drop in
    hit: Option<usize>,
    age: f32,          // seconds since the round was decided
    burst: bool,       // throw confetti on the next frame
    confetti: win98::Confetti,
}

fn idx(c: char) -> Option<usize> {
    c.is_ascii_alphabetic().then(|| (c.to_ascii_uppercase() as u8 - b'A') as usize)
}

impl Hang {
    pub fn with_word(w: &str) -> Self {
        Hang {
            word: w.to_uppercase().chars().collect(),
            guessed: [false; 26],
            wrong: 0,
            streak: 0,
            record: false,
            from: 0.0,
            k: 1.0,
            wobble: 0.0,
            pop: POP,
            hit: None,
            age: 0.0,
            burst: false,
            confetti: win98::Confetti::default(),
        }
    }

    pub fn new() -> Self {
        let list = words();
        Hang::with_word(list.choose(&mut rand::thread_rng()).map_or("PAPERCLIP", |w| w.as_str()))
    }

    /// Guess a letter. `Some(true)` = it's in the word, `Some(false)` = a miss, `None` = not a letter, already tried, or the round is over.
    pub fn guess(&mut self, c: char) -> Option<bool> {
        let i = idx(c)?;
        if self.guessed[i] || self.won() || self.lost() {
            return None;
        }
        self.guessed[i] = true;
        let hit = self.word.iter().any(|&w| idx(w) == Some(i));
        if !hit {
            self.wrong += 1;
        }
        Some(hit)
    }

    pub fn won(&self) -> bool {
        self.word.iter().all(|&c| idx(c).is_none_or(|i| self.guessed[i]))
    }

    pub fn lost(&self) -> bool {
        self.wrong >= MAX
    }

    /// The word as the player sees it: found letters, `_` for the rest, spaces and hyphens as they are.
    pub fn shown(&self) -> String {
        self.word.iter().map(|&c| if idx(c).is_some_and(|i| !self.guessed[i]) { '_' } else { c }).collect()
    }

    /// A guess from the keyboard or a button, with everything it sets off. `best` = the saved best streak.
    fn press(&mut self, c: char, best: Option<u32>, out: &mut Vec<Outcome>) {
        let was = self.unbent();
        let Some(hit) = self.guess(c) else { return };
        if hit {
            (self.pop, self.hit) = (0.0, idx(c));
        } else {
            (self.from, self.k, self.wobble) = (was, 0.0, WOBBLE);
            if self.wrong == MAX - 1 {
                out.push(Outcome::Moment("hangman", "last"));
            }
        }
        if self.won() {
            self.streak += 1;
            self.record = self.streak > best.unwrap_or(0);
            self.burst = true;
            out.push(Outcome::Won("hangman"));
            out.push(Outcome::Score("hangman", self.streak));
            if self.streak % 3 == 0 {
                out.push(Outcome::Moment("hangman", "streak"));
            }
        } else if self.lost() {
            self.streak = 0;
            out.push(Outcome::Lost("hangman"));
        }
    }

    /// How far the clip is unbent right now (0 = a clip, 1 = straight): it eases towards wrong / MAX.
    pub fn unbent(&self) -> f32 {
        let t = self.k * self.k * (3.0 - 2.0 * self.k);
        self.from + (self.wrong as f32 / MAX as f32 - self.from) * t
    }

    fn tick(&mut self, dt: f32) {
        self.k = (self.k + dt / EASE).min(1.0);
        self.wobble = (self.wobble - dt).max(0.0);
        self.pop = (self.pop + dt).min(POP);
        if self.won() || self.lost() {
            self.age += dt;
        }
        self.confetti.update(dt);
    }

    fn animating(&self) -> bool {
        let deciding = (self.won() || self.lost()) && self.age <= SHOW_RESULT;
        self.k < 1.0 || self.wobble > 0.0 || self.pop < POP || deciding || self.confetti.active()
    }
}

/// The clip, unbent by `u` (0 = a proper clip, 1 = a straight wire), with eyes that show how it's going.
/// `turn` tilts it (radians, the shudder after a miss) and `lift` raises it (pixels, the happy hop).
fn draw_clip(p: &Painter, area: Rect, u: f32, mood: i8, turn: f32, lift: f32) {
    let open = [(u * 3.0).clamp(0.0, 1.0), (u * 3.0 - 1.0).clamp(0.0, 1.0), (u * 3.0 - 2.0).clamp(0.0, 1.0)];
    let pts = wire(open);
    // lay the wire's long side flat (its length-weighted main axis): the half-open wire leans and would end up tiny
    let segs: Vec<(Vec2, f32)> = pts.windows(2).map(|w| ((w[0] + w[1]) / 2.0, (w[1] - w[0]).length())).collect();
    let mean = segs.iter().fold(Vec2::ZERO, |a, (m, l)| a + *m * *l) / segs.iter().map(|s| s.1).sum::<f32>();
    let (mut cxx, mut cyy, mut cxy) = (0.0, 0.0, 0.0);
    for (m, l) in &segs {
        let d = *m - mean;
        (cxx, cyy, cxy) = (cxx + d.x * d.x * l, cyy + d.y * d.y * l, cxy + d.x * d.y * l);
    }
    let tilt = 0.5 * (2.0 * cxy).atan2(cxx - cyy);
    let tilt = if tilt.abs() < 0.12 { 0.0 } else { tilt }; // a proper clip stays level
    let (s, c) = (-tilt).sin_cos();
    let pts: Vec<Vec2> = pts.iter().map(|v| vec2(v.x * c - v.y * s, v.x * s + v.y * c)).collect();
    let (mut lo, mut hi) = (pts[0], pts[0]);
    for q in &pts {
        lo = lo.min(*q);
        hi = hi.max(*q);
    }
    // as big as fits: the opened wire takes more room than the clip did
    let size = hi - lo + vec2(8.0, 16.0); // room for the eyes on top
    let scale = 3.85f32.min((area.width() - 30.0) / size.x).min((area.height() - 20.0) / size.y);
    let mid = (lo + hi) / 2.0;
    let (ts, tc) = turn.sin_cos();
    let at = |v: Vec2| {
        let d = (v - mid) * scale + vec2(0.0, 3.0 * scale);
        area.center() + vec2(d.x * tc - d.y * ts, d.x * ts + d.y * tc - lift)
    };
    // a soft shadow on the paper that stays put while he hops
    let shadow: Vec<Pos2> = pts.iter().map(|&v| at(v) + vec2(3.0, 4.0 + lift)).collect();
    p.add(Shape::line(shadow, Stroke::new(6.0, Color32::from_black_alpha(22))));
    let line: Vec<Pos2> = pts.iter().map(|&v| at(v)).collect();
    p.add(Shape::line(line.clone(), Stroke::new(7.0, Color32::from_rgb(80, 86, 100))));
    p.add(Shape::line(line, Stroke::new(3.0, Color32::from_rgb(210, 216, 228))));
    // the eyes sit on the long top wire (points 30..31 of the walk)
    let (a, b) = (pts[30], pts[31]);
    let d = (b - a).normalized();
    let up = vec2(d.y, -d.x);
    let up = if up.y > 0.0 { -up } else { up }; // eyes on top of the wire
    for (k, t) in [0.62f32, 0.8].into_iter().enumerate() {
        let c = at(a + (b - a) * t + up * 3.0);
        let r = (2.9 * scale).max(8.0);
        p.circle_filled(c, r, Color32::WHITE);
        p.circle_stroke(c, r, Stroke::new(1.5, Color32::BLACK));
        let ink = Stroke::new(2.5, Color32::BLACK);
        match mood {
            1 => p.add(Shape::line(vec![c + vec2(-6.0, 3.0), c + vec2(0.0, -3.0), c + vec2(6.0, 3.0)], ink)), // happy ^ ^
            -1 => {
                p.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], ink);
                p.add(Shape::line_segment([c + vec2(-5.0, 5.0), c + vec2(5.0, -5.0)], ink))
            }
            _ => {
                p.circle_filled(c + vec2(0.0, 2.0 + u * 3.0), 4.5, Color32::BLACK);
                // brows lift into a worried slant as the wire straightens
                let s = if k == 0 { 1.0 } else { -1.0 };
                p.add(Shape::line_segment([c + vec2(-8.0, -15.0 - s * u * 5.0), c + vec2(8.0, -15.0 + s * u * 5.0)], Stroke::new(2.0, Color32::from_rgb(60, 60, 60))))
            }
        };
    }
}

/// One life in the misses row: a little bent clip, or a straightened red wire once it's used up.
fn life(p: &Painter, c: Pos2, used: bool) {
    if used {
        let pts = vec![c + vec2(-11.0, 3.0), c + vec2(8.0, -3.0), c + vec2(10.0, -1.0), c + vec2(9.0, 1.5)];
        p.add(Shape::line(pts, Stroke::new(2.2, Color32::from_rgb(190, 50, 40))));
        return;
    }
    let pts = wire([0.0; 3]);
    let (lo, hi) = pts.iter().fold((pts[0], pts[0]), |(lo, hi), q| (lo.min(*q), hi.max(*q)));
    let k = 22.0 / (hi.x - lo.x);
    let line: Vec<Pos2> = pts.iter().map(|q| c + (*q - (lo + hi) / 2.0) * k).collect();
    p.add(Shape::line(line.clone(), Stroke::new(2.6, Color32::from_rgb(70, 76, 92))));
    p.add(Shape::line(line, Stroke::new(1.0, Color32::from_rgb(200, 206, 220))));
}

impl Game for Hang {
    fn size(&self) -> Vec2 {
        vec2(W, ART_H + 152.0)
    }

    fn new_round(&mut self) {
        // a streak survives a saved word or a skipped untouched one; giving up halfway ends it
        let keep = self.won() || self.guessed.iter().all(|g| !g);
        let (last, streak) = (self.word.clone(), if keep { self.streak } else { 0 });
        for _ in 0..5 {
            *self = Hang::new();
            if self.word != last {
                break;
            }
        }
        self.streak = streak;
    }

    fn status(&self) -> String {
        if self.won() {
            format!("Saved! Streak: {}. Any key or click: another word.", self.streak)
        } else if self.lost() {
            format!("Straight as a pin. The word was {}. Any key or click: try again.", self.word.iter().collect::<String>())
        } else {
            format!("Type or click letters. {} wrong guesses left.", MAX - self.wrong)
        }
    }

    fn finished(&self) -> bool {
        self.won() || self.lost()
    }

    fn live(&self) -> bool {
        self.animating()
    }

    fn takes_text(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, f: &mut Frame) {
        self.tick(f.dt);
        if f.input {
            let typed: Vec<char> = ui.input(|i| i.events.iter().filter_map(|e| if let Event::Text(t) = e { Some(t.clone()) } else { None }).flat_map(|t| t.chars().collect::<Vec<_>>()).collect());
            for c in typed {
                self.press(c, f.best, f.out);
            }
        }
        let (won, over) = (self.won(), self.won() || self.lost());

        // the notebook page with the clip on it
        let p = ui.painter().clone();
        let art = Rect::from_min_size(r.min, vec2(W, ART_H));
        p.rect_filled(art, 0.0, Color32::from_rgb(250, 250, 244));
        win98::bevel(&p, art, false);
        for k in 1..7 {
            let y = art.min.y + k as f32 * 21.0;
            p.line_segment([pos2(art.min.x + 3.0, y), pos2(art.max.x - 3.0, y)], Stroke::new(1.0, Color32::from_rgb(214, 226, 240)));
        }
        p.line_segment([pos2(art.min.x + 36.0, art.min.y + 2.0), pos2(art.min.x + 36.0, art.max.y - 2.0)], Stroke::new(1.0, Color32::from_rgb(240, 180, 180)));
        let mood = if won { 1 } else if self.lost() { -1 } else { 0 };
        let turn = (self.wobble * 38.0).sin() * 0.14 * self.wobble / WOBBLE;
        let lift = if won && self.age < 1.0 { (self.age * 3.0 * PI).sin().abs() * 14.0 * (1.0 - self.age) } else { 0.0 };
        draw_clip(&p, art, self.unbent(), mood, turn, lift);

        // the word, in slots; spaces stay gaps and hyphens show
        let n = self.word.len() as f32;
        let slot = 30f32.min((W - 16.0) / n);
        let size = if slot >= 26.0 { 26.0 } else { 20.0 };
        let x0 = r.center().x - slot * n / 2.0;
        let base = art.max.y + 46.0;
        for (k, (seen, c)) in self.shown().chars().zip(self.word.iter().copied()).enumerate() {
            let cx = x0 + slot * (k as f32 + 0.5);
            let Some(i) = idx(c) else {
                p.text(pos2(cx, base - 2.0), Align2::CENTER_BOTTOM, c, win98::bold(size), win98::TEXT);
                continue;
            };
            p.rect_filled(Rect::from_min_max(pos2(cx - slot * 0.36, base), pos2(cx + slot * 0.36, base + 2.5)), 0.0, win98::TEXT);
            if seen != '_' || over {
                let drop = if self.hit == Some(i) { 1.0 - self.pop / POP } else { 0.0 };
                let col = match (seen != '_', won) {
                    (false, _) => Color32::from_rgb(190, 40, 40), // the letters you never found
                    (true, true) => Color32::from_rgb(30, 120, 50),
                    _ => win98::TEXT,
                };
                p.text(pos2(cx, base - 3.0 - 12.0 * drop), Align2::CENTER_BOTTOM, c, win98::bold(size), col.gamma_multiply(1.0 - 0.8 * drop));
            }
        }

        // misses: a row of clips that straighten, the wrong letters, and the streak
        let row = base + 22.0;
        p.text(pos2(r.min.x + 4.0, row), Align2::LEFT_CENTER, "Misses:", win98::font(13.0), win98::TEXT);
        for k in 0..MAX {
            life(&p, pos2(r.min.x + 70.0 + k as f32 * 27.0, row), k < self.wrong);
        }
        let missed: String = (0..26).filter(|&i| self.guessed[i] && !self.word.iter().any(|&w| idx(w) == Some(i))).map(|i| (b'A' + i as u8) as char).collect();
        p.text(pos2(r.min.x + 70.0 + MAX as f32 * 27.0 - 6.0, row), Align2::LEFT_CENTER, missed, win98::bold(13.0), Color32::from_rgb(170, 40, 30));
        win98::field(&p, Rect::from_min_size(pos2(r.max.x - 88.0, row - 11.0), vec2(88.0, 22.0)), &format!("Streak {}", self.streak));

        // the letter keys: pressed-in and tinted once tried
        let ky = row + 18.0;
        for i in 0..26 {
            let (line, col) = (i / 13, i % 13);
            let br = Rect::from_min_size(pos2(r.center().x - 13.0 * 34.0 / 2.0 + col as f32 * 34.0, ky + line as f32 * 32.0), vec2(32.0, 29.0));
            let c = (b'A' + i as u8) as char;
            if self.guessed[i] {
                let hit = self.word.iter().any(|&w| idx(w) == Some(i));
                let (fill, ink) = if hit { (Color32::from_rgb(196, 228, 188), Color32::from_rgb(20, 100, 40)) } else { (Color32::from_rgb(238, 196, 188), Color32::from_rgb(160, 40, 30)) };
                win98::panel(&p, br, fill, false);
                p.text(br.center() + vec2(1.0, 1.0), Align2::CENTER_CENTER, c, win98::bold(13.0), ink);
            } else if win98::button(ui, br, Id::unique(("hang-key", i)), &c.to_string(), f.input && !over).clicked() {
                self.press(c, f.best, f.out);
            }
        }

        if std::mem::take(&mut self.burst) {
            self.confetti.burst(pos2(art.center().x, art.min.y + 50.0), 90);
        }
        if over && self.age > SHOW_RESULT {
            let word: String = self.word.iter().collect();
            let best = f.best.unwrap_or(0).max(self.streak);
            // the first line is bold and the box is sized for plain text: keep it short
            let (icon, lines) = if won {
                let icon = if self.record { win98::Icon::Trophy } else { win98::Icon::Info };
                let note = if self.record { format!("Streak: {}. A new best streak!", self.streak) } else { format!("Streak: {}    Best: {best}", self.streak) };
                (icon, [String::from("You saved Clipster!"), format!("The word was {word}."), format!("Misses: {} of {MAX}", self.wrong), note])
            } else {
                (win98::Icon::Warning, [String::from("Straight as a pin."), format!("The word was {word}."), format!("Misses: {MAX} of {MAX}"), format!("Streak: 0    Best: {best}")])
            };
            if win98::dialog(ui, r, "Hangman", icon, &lines, &["Next word"]).is_some() {
                self.new_round();
            }
        }
        self.confetti.draw(&p.with_clip_rect(r));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guessing_every_letter_wins() {
        let mut h = Hang::with_word("memo");
        assert_eq!(h.guess('m'), Some(true));
        assert_eq!(h.guess('M'), None, "a repeat doesn't count");
        assert_eq!(h.guess('e'), Some(true));
        assert!(!h.won());
        assert_eq!(h.guess('o'), Some(true));
        assert!(h.won() && !h.lost());
        assert_eq!(h.guess('z'), None, "the round is over");
    }

    #[test]
    fn seven_misses_lose() {
        let mut h = Hang::with_word("sticky note");
        for c in "abdfgh".chars() {
            assert_eq!(h.guess(c), Some(false));
        }
        assert!(!h.lost());
        assert_eq!(h.guess('j'), Some(false));
        assert!(h.lost() && !h.won());
        assert_eq!(h.wrong, MAX);
    }

    #[test]
    fn hyphens_and_spaces_show_and_need_no_guess() {
        let mut h = Hang::with_word("e-mail");
        assert_eq!(h.shown(), "_-____");
        h.guess('e');
        h.guess('m');
        assert_eq!(h.shown(), "E-M___");
        for c in "ail".chars() {
            h.guess(c);
        }
        assert!(h.won(), "the hyphen isn't a letter to find");
        assert_eq!(Hang::with_word("sticky note").shown(), "______ ____");
    }

    #[test]
    fn streak_counts_wins_and_resets_on_a_loss() {
        let mut out = Vec::new();
        let mut h = Hang::with_word("memo");
        for c in "meo".chars() {
            h.press(c, Some(1), &mut out);
        }
        assert_eq!(h.streak, 1);
        assert!(!h.record, "1 doesn't beat a best of 1");
        assert_eq!(out, vec![Outcome::Won("hangman"), Outcome::Score("hangman", 1)]);
        // two more wins: the third brings a streak comment and a record
        for _ in 0..2 {
            let streak = h.streak;
            h = Hang::with_word("inbox");
            h.streak = streak;
            for c in "inbox".chars() {
                h.press(c, Some(1), &mut out);
            }
        }
        assert_eq!(h.streak, 3);
        assert!(h.record);
        assert_eq!(out.last(), Some(&Outcome::Moment("hangman", "streak")));
        assert!(out.contains(&Outcome::Score("hangman", 3)));
        // a loss: one guess left gets its comment, the seventh miss ends the streak
        out.clear();
        let streak = h.streak;
        h = Hang::with_word("memo");
        h.streak = streak;
        for c in "abcdfg".chars() {
            h.press(c, None, &mut out);
        }
        assert_eq!(out, vec![Outcome::Moment("hangman", "last")]);
        h.press('h', None, &mut out);
        assert_eq!(out.last(), Some(&Outcome::Lost("hangman")));
        assert_eq!(h.streak, 0);
        assert!(!out.iter().any(|o| matches!(o, Outcome::Won(_))));
    }

    #[test]
    fn a_new_word_keeps_a_streak_but_giving_up_ends_it() {
        let mut h = Hang::with_word("memo");
        h.streak = 4;
        h.new_round();
        assert_eq!(h.streak, 4, "skipping an untouched word is fine");
        h.guess('q');
        h.new_round();
        assert_eq!(h.streak, 0, "abandoning a word you started");
    }

    #[test]
    fn the_clip_eases_to_its_new_shape() {
        let mut h = Hang::with_word("memo");
        h.press('z', None, &mut Vec::new());
        assert!(h.animating());
        assert_eq!(h.unbent(), 0.0, "it starts from where it was");
        h.tick(0.1);
        let early = h.unbent();
        assert!(early > 0.0 && early < 1.0 / MAX as f32);
        for _ in 0..10 {
            h.tick(0.1);
        }
        assert_eq!(h.unbent(), 1.0 / MAX as f32);
        assert!(!h.animating(), "and then it rests");
    }

    #[test]
    fn falls_back_to_office_words() {
        assert!(!words().is_empty());
        assert!(Hang::new().word.iter().any(|c| c.is_ascii_uppercase()));
    }
}
