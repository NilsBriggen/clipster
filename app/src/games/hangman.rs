//! Hangman, office style: every wrong letter unbends the paperclip a little more.

use super::{win98, wire, Game, Outcome};
use eframe::egui::{pos2, vec2, Align2, Color32, Event, FontId, Id, Pos2, Rect, Shape, Stroke, Ui, Vec2};
use rand::seq::SliceRandom;

pub const MAX: u8 = 7;
const W: f32 = 460.0;
const ART_H: f32 = 150.0;

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
    reported: bool,
}

fn idx(c: char) -> Option<usize> {
    c.is_ascii_alphabetic().then(|| (c.to_ascii_uppercase() as u8 - b'A') as usize)
}

impl Hang {
    pub fn with_word(w: &str) -> Self {
        Hang { word: w.to_uppercase().chars().collect(), guessed: [false; 26], wrong: 0, reported: false }
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
}

/// The clip, unbent by `u` (0 = a proper clip, 1 = a straight wire), with eyes that show how it's going.
fn draw_clip(p: &eframe::egui::Painter, area: Rect, u: f32, mood: i8) {
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
    let at = |v: Vec2| area.center() + (v - mid) * scale + vec2(0.0, 3.0 * scale);
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

impl Game for Hang {
    fn size(&self) -> Vec2 {
        vec2(W, ART_H + 150.0)
    }

    fn new_round(&mut self) {
        let last = self.word.clone();
        for _ in 0..5 {
            *self = Hang::new();
            if self.word != last {
                break;
            }
        }
    }

    fn status(&self) -> String {
        if self.won() {
            "Saved! Clipster keeps his shape. Any key or click: another word.".into()
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
        false
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, input: bool, out: &mut Vec<Outcome>) {
        let over = self.won() || self.lost();
        if input {
            let typed: Vec<char> = ui.input(|i| i.events.iter().filter_map(|e| if let Event::Text(t) = e { Some(t.clone()) } else { None }).flat_map(|t| t.chars().collect::<Vec<_>>()).collect());
            for c in typed {
                self.guess(c);
            }
        }

        let p = ui.painter().clone();
        let art = Rect::from_min_size(r.min, vec2(W, ART_H));
        p.rect_filled(art, 0.0, Color32::from_rgb(250, 250, 244));
        win98::bevel(&p, art, false);
        for k in 1..7 {
            let y = art.min.y + k as f32 * 21.0;
            p.line_segment([pos2(art.min.x + 3.0, y), pos2(art.max.x - 3.0, y)], Stroke::new(1.0, Color32::from_rgb(214, 226, 240)));
        }
        p.line_segment([pos2(art.min.x + 36.0, art.min.y + 2.0), pos2(art.min.x + 36.0, art.max.y - 2.0)], Stroke::new(1.0, Color32::from_rgb(240, 180, 180)));
        let mood = if self.won() { 1 } else if self.lost() { -1 } else { 0 };
        draw_clip(&p, art, self.wrong as f32 / MAX as f32, mood);

        // the word, as slots
        let n = self.word.len() as f32;
        let slot = (28.0f32).min((W - 20.0) / n);
        let x0 = r.center().x - slot * n / 2.0;
        let wy = art.max.y + 26.0;
        for (k, &c) in self.word.iter().enumerate() {
            let cx = x0 + slot * (k as f32 + 0.5);
            let Some(i) = idx(c) else { continue };
            p.line_segment([pos2(cx - slot * 0.38, wy + 14.0), pos2(cx + slot * 0.38, wy + 14.0)], Stroke::new(2.0, win98::TEXT));
            if self.guessed[i] || over {
                let col = if self.guessed[i] { win98::TEXT } else { Color32::from_rgb(190, 40, 40) };
                p.text(pos2(cx, wy + 12.0), Align2::CENTER_BOTTOM, c, FontId::monospace(22.0), col);
            }
        }
        let missed: String = (0..26).filter(|&i| self.guessed[i] && !self.word.iter().any(|&w| idx(w) == Some(i))).map(|i| (b'A' + i as u8) as char).flat_map(|c| [c, ' ']).collect();
        p.text(pos2(r.min.x + 4.0, wy + 26.0), Align2::LEFT_TOP, format!("Wrong: {} of {}   {}", self.wrong, MAX, missed), win98::font(13.0), win98::TEXT);

        // the letter buttons
        let ky = wy + 50.0;
        for i in 0..26 {
            let (row, col) = (i / 13, i % 13);
            let br = Rect::from_min_size(pos2(r.center().x - 13.0 * 34.0 / 2.0 + col as f32 * 34.0, ky + row as f32 * 32.0), vec2(32.0, 29.0));
            let c = (b'A' + i as u8) as char;
            if win98::button(ui, br, Id::unique(("hang-key", i)), &c.to_string(), input && !self.guessed[i] && !over).clicked() {
                self.guess(c);
            }
        }

        if !self.reported && (self.won() || self.lost()) {
            self.reported = true;
            out.push(if self.won() { Outcome::Won("hangman") } else { Outcome::Lost("hangman") });
        }
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
    fn falls_back_to_office_words() {
        assert!(!words().is_empty());
        assert!(Hang::new().word.iter().any(|c| c.is_ascii_uppercase()));
    }
}
