//! Chess: the full rules, click to move, two players or against Clipster.
//! Clipster plays with a small alpha-beta search over material and piece-square tables: classic search, no AI models.

use super::{win98, Game, Outcome};
use eframe::egui::{pos2, vec2, Align2, Color32, FontId, Id, Key, Rect, Sense, Stroke, Ui, Vec2};
use std::sync::mpsc;

pub const PAWN: i8 = 1;
pub const KNIGHT: i8 = 2;
pub const BISHOP: i8 = 3;
pub const ROOK: i8 = 4;
pub const QUEEN: i8 = 5;
pub const KING: i8 = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    pub promo: i8, // 0, or the piece kind a pawn becomes
}

/// A position. Squares are rank * 8 + file, a1 = 0, h8 = 63. White pieces are positive, black negative.
#[derive(Clone, PartialEq, Debug)]
pub struct Pos {
    pub b: [i8; 64],
    pub white: bool,
    pub castle: u8, // 1 = white short, 2 = white long, 4 = black short, 8 = black long
    pub ep: Option<u8>,
    pub half: u16, // moves since a capture or pawn move (the 50-move rule)
}

const KNIGHT_STEPS: [(i32, i32); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
const KING_STEPS: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
const STRAIGHT: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const DIAGONAL: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

fn sq(f: i32, r: i32) -> Option<usize> {
    ((0..8).contains(&f) && (0..8).contains(&r)).then(|| (r * 8 + f) as usize)
}

pub fn name(s: usize) -> String {
    format!("{}{}", (b'a' + (s % 8) as u8) as char, s / 8 + 1)
}

impl Pos {
    pub fn start() -> Pos {
        Pos::fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").expect("start position")
    }

    pub fn fen(s: &str) -> Option<Pos> {
        let mut parts = s.split_whitespace();
        let mut b = [0i8; 64];
        for (i, row) in parts.next()?.split('/').enumerate() {
            let r = 7 - i as i32;
            let mut f = 0;
            for c in row.chars() {
                if let Some(d) = c.to_digit(10) {
                    f += d as i32;
                    continue;
                }
                let kind = match c.to_ascii_lowercase() {
                    'p' => PAWN,
                    'n' => KNIGHT,
                    'b' => BISHOP,
                    'r' => ROOK,
                    'q' => QUEEN,
                    'k' => KING,
                    _ => return None,
                };
                b[sq(f, r)?] = if c.is_ascii_uppercase() { kind } else { -kind };
                f += 1;
            }
        }
        let white = parts.next()? == "w";
        let castle = parts.next()?.chars().map(|c| match c {
            'K' => 1,
            'Q' => 2,
            'k' => 4,
            'q' => 8,
            _ => 0,
        }).sum();
        let ep = parts.next().and_then(|e| {
            let e = e.as_bytes();
            (e.len() == 2).then(|| sq((e[0] as i32) - ('a' as i32), (e[1] as i32) - ('1' as i32))).flatten().map(|s| s as u8)
        });
        let half = parts.next().and_then(|h| h.parse().ok()).unwrap_or(0);
        Some(Pos { b, white, castle, ep, half })
    }

    /// Is square `s` attacked by the given side?
    pub fn attacked(&self, s: usize, by_white: bool) -> bool {
        let side = if by_white { 1 } else { -1 };
        let (f, r) = ((s % 8) as i32, (s / 8) as i32);
        let at = |df: i32, dr: i32| sq(f + df, r + dr).map_or(0, |t| self.b[t]);
        if at(-1, -side as i32) == side * PAWN || at(1, -side as i32) == side * PAWN {
            return true;
        }
        if KNIGHT_STEPS.iter().any(|&(df, dr)| at(df, dr) == side * KNIGHT) || KING_STEPS.iter().any(|&(df, dr)| at(df, dr) == side * KING) {
            return true;
        }
        for (dirs, kind) in [(STRAIGHT, ROOK), (DIAGONAL, BISHOP)] {
            for (df, dr) in dirs {
                let (mut x, mut y) = (f + df, r + dr);
                while let Some(t) = sq(x, y) {
                    let p = self.b[t];
                    if p != 0 {
                        if p == side * kind || p == side * QUEEN {
                            return true;
                        }
                        break;
                    }
                    x += df;
                    y += dr;
                }
            }
        }
        false
    }

    pub fn in_check(&self, white: bool) -> bool {
        let king = if white { KING } else { -KING };
        self.b.iter().position(|&p| p == king).is_some_and(|k| self.attacked(k, !white))
    }

    /// Every move the side to move could make, before checking it leaves their king safe.
    fn pseudo(&self) -> Vec<Move> {
        let side: i8 = if self.white { 1 } else { -1 };
        let mut out = Vec::with_capacity(48);
        let add = |from: usize, to: usize, out: &mut Vec<Move>| {
            let last = to / 8 == 7 || to / 8 == 0;
            if self.b[from].abs() == PAWN && last {
                for k in [QUEEN, ROOK, BISHOP, KNIGHT] {
                    out.push(Move { from: from as u8, to: to as u8, promo: k });
                }
            } else {
                out.push(Move { from: from as u8, to: to as u8, promo: 0 });
            }
        };
        for from in 0..64 {
            let p = self.b[from];
            if p * side <= 0 {
                continue;
            }
            let (f, r) = ((from % 8) as i32, (from / 8) as i32);
            let enemy = |t: usize| self.b[t] * side < 0;
            match p.abs() {
                PAWN => {
                    let dr = side as i32;
                    if let Some(t) = sq(f, r + dr).filter(|&t| self.b[t] == 0) {
                        add(from, t, &mut out);
                        let home = if side > 0 { 1 } else { 6 };
                        if let Some(t2) = sq(f, r + 2 * dr).filter(|&t2| r == home && self.b[t2] == 0) {
                            add(from, t2, &mut out);
                        }
                    }
                    for df in [-1, 1] {
                        if let Some(t) = sq(f + df, r + dr) {
                            if enemy(t) || self.ep == Some(t as u8) {
                                add(from, t, &mut out);
                            }
                        }
                    }
                }
                KNIGHT | KING => {
                    let steps = if p.abs() == KNIGHT { &KNIGHT_STEPS } else { &KING_STEPS };
                    for &(df, dr) in steps {
                        if let Some(t) = sq(f + df, r + dr).filter(|&t| self.b[t] == 0 || enemy(t)) {
                            add(from, t, &mut out);
                        }
                    }
                }
                kind => {
                    let dirs: &[(i32, i32)] = match kind {
                        BISHOP => &DIAGONAL,
                        ROOK => &STRAIGHT,
                        _ => &KING_STEPS, // the queen goes all eight ways
                    };
                    for &(df, dr) in dirs {
                        let (mut x, mut y) = (f + df, r + dr);
                        while let Some(t) = sq(x, y) {
                            if self.b[t] * side > 0 {
                                break;
                            }
                            add(from, t, &mut out);
                            if self.b[t] != 0 {
                                break;
                            }
                            x += df;
                            y += dr;
                        }
                    }
                }
            }
        }
        // castling: rights, empty squares between, and the king not in, through or into check
        let base = if self.white { 0 } else { 56 };
        let (short, long) = if self.white { (1, 2) } else { (4, 8) };
        if self.b[base + 4] == side * KING && !self.attacked(base + 4, !self.white) {
            if self.castle & short != 0 && self.b[base + 7] == side * ROOK && self.b[base + 5] == 0 && self.b[base + 6] == 0 && !self.attacked(base + 5, !self.white) {
                out.push(Move { from: (base + 4) as u8, to: (base + 6) as u8, promo: 0 });
            }
            if self.castle & long != 0 && self.b[base] == side * ROOK && self.b[base + 1..base + 4].iter().all(|&p| p == 0) && !self.attacked(base + 3, !self.white) {
                out.push(Move { from: (base + 4) as u8, to: (base + 2) as u8, promo: 0 });
            }
        }
        out
    }

    pub fn legal(&self) -> Vec<Move> {
        self.pseudo().into_iter().filter(|&m| !self.play(m).in_check(self.white)).collect()
    }

    /// The position after a move (which must come from `pseudo` or `legal`).
    pub fn play(&self, m: Move) -> Pos {
        let mut n = self.clone();
        let (from, to) = (m.from as usize, m.to as usize);
        let p = self.b[from];
        let kind = p.abs();
        let captured = self.b[to];
        n.b[to] = if m.promo != 0 { m.promo * p.signum() } else { p };
        n.b[from] = 0;
        if kind == PAWN && self.ep == Some(m.to) && captured == 0 && from % 8 != to % 8 {
            n.b[(from / 8) * 8 + to % 8] = 0; // en passant takes the pawn beside us
        }
        if kind == KING && from.abs_diff(to) == 2 {
            let (rf, rt) = if to > from { (from + 3, from + 1) } else { (from - 4, from - 1) };
            n.b[rt] = n.b[rf];
            n.b[rf] = 0;
        }
        n.ep = (kind == PAWN && from.abs_diff(to) == 16).then(|| ((from + to) / 2) as u8);
        for s in [from, to] {
            n.castle &= match s {
                0 => !2,
                7 => !1,
                4 => !3,
                56 => !8,
                63 => !4,
                60 => !12,
                _ => !0,
            };
        }
        n.half = if kind == PAWN || captured != 0 { 0 } else { self.half + 1 };
        n.white = !self.white;
        n
    }

    /// Only kings, or a king and one knight or bishop against a bare king: nobody can mate.
    pub fn dead_draw(&self) -> bool {
        let rest: Vec<i8> = self.b.iter().copied().filter(|p| *p != 0 && p.abs() != KING).collect();
        rest.is_empty() || (rest.len() == 1 && matches!(rest[0].abs(), KNIGHT | BISHOP))
    }
}

// ---------------- Clipster's search ----------------

const VALUE: [i32; 7] = [0, 100, 320, 330, 500, 900, 0];
const MATE: i32 = 100_000;

/// Material plus a few habits (centre, pawns forward, king tucked away), from White's side.
pub fn eval(p: &Pos) -> i32 {
    let mut score = 0;
    for (s, &pc) in p.b.iter().enumerate() {
        if pc == 0 {
            continue;
        }
        let (f, r) = ((s % 8) as i32, (s / 8) as i32);
        let r = if pc > 0 { r } else { 7 - r }; // ranks counted from the piece's own side
        let centre = 3 - (f - 3).abs().min((f - 4).abs()).max((r - 3).abs().min((r - 4).abs())); // 0 at the edge .. 3 in the middle
        let kind = pc.abs();
        let habit = match kind {
            PAWN => r * 8 + if (3..=4).contains(&f) && r >= 3 { 12 } else { 0 },
            KNIGHT => centre * 12 - 18,
            BISHOP => centre * 6,
            ROOK => if r == 6 { 15 } else { 0 },
            QUEEN => centre * 3,
            _ => (if r == 0 { 12 } else { -10 * r }) - centre * 6,
        };
        score += pc.signum() as i32 * (VALUE[kind as usize] + habit);
    }
    score
}

/// Captures first, biggest victim by smallest attacker; promotions too.
fn ordered(p: &Pos, mut moves: Vec<Move>) -> Vec<Move> {
    moves.sort_by_key(|m| {
        let victim = VALUE[p.b[m.to as usize].unsigned_abs() as usize];
        let attacker = if victim > 0 { VALUE[p.b[m.from as usize].unsigned_abs() as usize] } else { 0 };
        let promo = if m.promo != 0 { VALUE[m.promo as usize] } else { 0 };
        attacker - victim * 10 - promo
    });
    moves
}

/// Only captures, until things are quiet, so a search never stops in the middle of a trade.
fn quiet(p: &Pos, mut alpha: i32, beta: i32, depth: u32) -> i32 {
    let stand = eval(p) * if p.white { 1 } else { -1 };
    if stand >= beta || depth == 0 {
        return stand;
    }
    alpha = alpha.max(stand);
    let caps: Vec<Move> = p.pseudo().into_iter().filter(|m| p.b[m.to as usize] != 0 || m.promo != 0).collect();
    for m in ordered(p, caps) {
        let n = p.play(m);
        if n.in_check(p.white) {
            continue;
        }
        let s = -quiet(&n, -beta, -alpha, depth - 1);
        if s >= beta {
            return s;
        }
        alpha = alpha.max(s);
    }
    alpha
}

fn search(p: &Pos, depth: u32, mut alpha: i32, beta: i32, ply: i32) -> i32 {
    if p.half >= 100 || p.dead_draw() {
        return 0;
    }
    let moves = p.legal();
    if moves.is_empty() {
        return if p.in_check(p.white) { -MATE + ply } else { 0 };
    }
    if depth == 0 {
        return quiet(p, alpha, beta, 4);
    }
    let mut best = -MATE;
    for m in ordered(p, moves) {
        let s = -search(&p.play(m), depth - 1, -beta, -alpha, ply + 1);
        best = best.max(s);
        alpha = alpha.max(s);
        if alpha >= beta {
            break;
        }
    }
    best
}

/// Clipster's pick. `wobble` adds a little noise so easy games differ.
pub fn best_move(p: &Pos, depth: u32, wobble: i32) -> Option<Move> {
    use rand::{seq::SliceRandom, Rng};
    let mut rng = rand::thread_rng();
    let mut moves = p.legal();
    moves.shuffle(&mut rng); // equal moves come out in a different order each game
    let mut best = None;
    let mut alpha = -MATE - 1;
    for m in ordered(p, moves) {
        let noise = if wobble > 0 { rng.gen_range(-wobble..=wobble) } else { 0 };
        let s = -search(&p.play(m), depth - 1, -MATE - 1, -alpha + noise, 1) + noise;
        if s > alpha || best.is_none() {
            alpha = s;
            best = Some(m);
        }
    }
    best
}

// ---------------- the window ----------------

const SQ: f32 = 50.0;
const EDGE: f32 = 18.0; // room for the a–h / 1–8 labels

pub struct Chess {
    pub pos: Pos,
    legal: Vec<Move>,
    sel: Option<usize>,
    last: Option<Move>,
    promo: Option<(u8, u8)>,
    mode: usize, // 0 = Clipster easy, 1 = Clipster hard, 2 = two players
    think: Option<mpsc::Receiver<Option<Move>>>,
    end: Option<String>,
}

impl Chess {
    pub fn new() -> Self {
        let pos = Pos::start();
        Chess { legal: pos.legal(), pos, sel: None, last: None, promo: None, mode: 0, think: None, end: None }
    }

    fn vs_clipster(&self) -> bool {
        self.mode < 2
    }

    fn make(&mut self, m: Move, out: &mut Vec<Outcome>) {
        let mover_white = self.pos.white;
        self.pos = self.pos.play(m);
        self.last = Some(m);
        self.sel = None;
        self.legal = self.pos.legal();
        let who = if mover_white { "White" } else { "Black" };
        self.end = if self.legal.is_empty() && self.pos.in_check(self.pos.white) {
            out.push(if self.vs_clipster() && !mover_white { Outcome::Lost("chess") } else { Outcome::Won("chess") });
            Some(if !self.vs_clipster() {
                format!("Checkmate! {who} wins.")
            } else if mover_white {
                "Checkmate! You beat Clipster.".into()
            } else {
                "Checkmate. Clipster wins this one.".into()
            })
        } else if self.legal.is_empty() {
            Some("Stalemate: nobody can move. It's a draw.".into())
        } else if self.pos.half >= 100 {
            Some("50 moves without a capture or pawn move: draw.".into())
        } else if self.pos.dead_draw() {
            Some("Not enough pieces left to mate: draw.".into())
        } else {
            None
        };
    }
}

impl Game for Chess {
    fn size(&self) -> Vec2 {
        vec2(EDGE + 8.0 * SQ + 4.0, 8.0 * SQ + 4.0 + EDGE)
    }

    fn new_round(&mut self) {
        let mode = self.mode;
        *self = Chess::new();
        self.mode = mode;
    }

    fn levels(&self) -> &'static [&'static str] {
        &["Clipster (easy)", "Clipster (hard)", "Two players"]
    }
    fn level(&self) -> usize {
        self.mode
    }
    fn set_level(&mut self, i: usize) {
        self.mode = i.min(2);
        self.new_round();
    }

    fn status(&self) -> String {
        if let Some(e) = &self.end {
            return e.clone();
        }
        let check = if self.pos.in_check(self.pos.white) { "Check! " } else { "" };
        if self.think.is_some() {
            format!("{check}Clipster is thinking…")
        } else if self.vs_clipster() {
            let last = self.last.map_or(String::new(), |m| format!("Clipster moved {}–{}. ", name(m.from as usize), name(m.to as usize)));
            format!("{check}{last}Your move: you play White.")
        } else {
            format!("{check}{} to move.", if self.pos.white { "White" } else { "Black" })
        }
    }

    fn finished(&self) -> bool {
        self.end.is_some()
    }

    fn live(&self) -> bool {
        self.think.is_some()
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, input: bool, out: &mut Vec<Outcome>) {
        // Clipster's turn: think on a thread so the window never freezes
        if self.vs_clipster() && !self.pos.white && self.end.is_none() {
            match &self.think {
                None => {
                    let (tx, rx) = mpsc::channel();
                    let (pos, depth, wobble) = (self.pos.clone(), if self.mode == 0 { 2 } else { 3 }, if self.mode == 0 { 30 } else { 4 });
                    std::thread::spawn(move || {
                        let _ = tx.send(best_move(&pos, depth, wobble));
                    });
                    self.think = Some(rx);
                }
                Some(rx) => match rx.try_recv() {
                    Ok(m) => {
                        self.think = None;
                        if let Some(m) = m {
                            self.make(m, out);
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => self.think = None,
                    Err(mpsc::TryRecvError::Empty) => {}
                },
            }
        }

        let board = Rect::from_min_size(r.min + vec2(EDGE, 0.0), vec2(8.0 * SQ + 4.0, 8.0 * SQ + 4.0));
        let o = board.min + vec2(2.0, 2.0);
        let square = |s: usize| Rect::from_min_size(o + vec2((s % 8) as f32 * SQ, (7 - s / 8) as f32 * SQ), vec2(SQ, SQ));
        let resp = ui.interact(board, Id::unique("chess-board"), Sense::click());
        let my_turn = self.end.is_none() && self.think.is_none() && (!self.vs_clipster() || self.pos.white);

        if input && my_turn && self.promo.is_none() {
            if let Some(pt) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
                let v = pt - o;
                let (f, rk) = ((v.x / SQ).floor() as i32, 7 - (v.y / SQ).floor() as i32);
                if let Some(s) = sq(f, rk) {
                    let mine = self.pos.b[s] != 0 && (self.pos.b[s] > 0) == self.pos.white;
                    let target = self.sel.and_then(|from| self.legal.iter().find(|m| m.from as usize == from && m.to as usize == s).copied());
                    match target {
                        Some(m) if m.promo != 0 => self.promo = Some((m.from, m.to)),
                        Some(m) => self.make(m, out),
                        None if mine => self.sel = if self.sel == Some(s) { None } else { Some(s) },
                        None => self.sel = None,
                    }
                }
            }
        }

        // draw the board
        let p = ui.painter().clone();
        let p = &p;
        win98::bevel(p, board, false);
        let king = if self.pos.white { KING } else { -KING };
        let checked = self.pos.in_check(self.pos.white).then(|| self.pos.b.iter().position(|&x| x == king)).flatten();
        for s in 0..64 {
            let light = (s % 8 + s / 8) % 2 == 1;
            let mut c = if light { Color32::from_rgb(238, 232, 214) } else { Color32::from_rgb(132, 156, 182) };
            if self.last.is_some_and(|m| m.from as usize == s || m.to as usize == s) {
                c = c.lerp_to_gamma(Color32::from_rgb(230, 210, 90), 0.45);
            }
            if self.sel == Some(s) {
                c = c.lerp_to_gamma(Color32::from_rgb(250, 230, 80), 0.7);
            }
            if checked == Some(s) {
                c = c.lerp_to_gamma(Color32::from_rgb(230, 60, 50), 0.6);
            }
            p.rect_filled(square(s), 0.0, c);
        }
        if let Some(from) = self.sel {
            for m in self.legal.iter().filter(|m| m.from as usize == from) {
                let t = square(m.to as usize);
                if self.pos.b[m.to as usize] != 0 {
                    p.circle_stroke(t.center(), SQ * 0.44, Stroke::new(3.0, Color32::from_black_alpha(70)));
                } else {
                    p.circle_filled(t.center(), SQ * 0.14, Color32::from_black_alpha(70));
                }
            }
        }
        for s in 0..64 {
            if self.pos.b[s] != 0 && self.promo.is_none_or(|(from, _)| from as usize != s) {
                piece(p, square(s).center(), self.pos.b[s], 40.0);
            }
        }
        for i in 0..8 {
            let f = FontId::proportional(12.0);
            p.text(pos2(o.x + (i as f32 + 0.5) * SQ, board.max.y + 3.0), Align2::CENTER_TOP, ((b'a' + i) as char).to_string(), f.clone(), win98::TEXT);
            p.text(pos2(r.min.x + EDGE / 2.0, o.y + (7.0 - i as f32 + 0.5) * SQ), Align2::CENTER_CENTER, (i + 1).to_string(), f, win98::TEXT);
        }
        // a pawn reached the end: pick what it becomes
        if let Some((from, to)) = self.promo {
            let pop = Rect::from_center_size(board.center(), vec2(4.0 * 54.0 + 16.0, 92.0));
            win98::panel(p, pop, win98::FACE, true);
            p.text(pop.min + vec2(10.0, 8.0), Align2::LEFT_TOP, "Promote the pawn to:", win98::font(13.0), win98::TEXT);
            let side = self.pos.b[from as usize].signum();
            let mut chosen = None;
            for (i, k) in [QUEEN, ROOK, BISHOP, KNIGHT].into_iter().enumerate() {
                let br = Rect::from_min_size(pop.min + vec2(8.0 + i as f32 * 54.0, 30.0), vec2(50.0, 50.0));
                if win98::button(ui, br, Id::unique(("promo", i)), "", input).clicked() {
                    chosen = Some(k);
                }
                piece(p, br.center(), k * side, 36.0);
            }
            if input && ui.input(|i| i.key_pressed(Key::Escape)) {
                self.promo = None;
            }
            if let Some(k) = chosen {
                self.promo = None;
                self.make(Move { from, to, promo: k }, out);
            }
        }
        if let Some(e) = &self.end {
            let b = Rect::from_center_size(board.center(), vec2(300.0, 60.0));
            win98::panel(p, b, win98::FACE, true);
            p.text(b.center() - vec2(0.0, 9.0), Align2::CENTER_CENTER, e, win98::font(14.0), win98::TEXT);
            p.text(b.center() + vec2(0.0, 13.0), Align2::CENTER_CENTER, "Press any key or click for a new game", win98::font(12.0), win98::GRAY_TEXT);
        }
    }
}

/// White pieces: a white solid glyph under a black outline glyph. Black pieces: the solid glyph in black.
fn piece(p: &eframe::egui::Painter, c: eframe::egui::Pos2, pc: i8, size: f32) {
    let solid = ["", "♟", "♞", "♝", "♜", "♛", "♚"][pc.unsigned_abs() as usize];
    let outline = ["", "♙", "♘", "♗", "♖", "♕", "♔"][pc.unsigned_abs() as usize];
    let f = FontId::proportional(size);
    let at = c + vec2(0.0, size * 0.04);
    if pc > 0 {
        p.text(at, Align2::CENTER_CENTER, solid, f.clone(), Color32::WHITE);
        p.text(at, Align2::CENTER_CENTER, outline, f, Color32::BLACK);
    } else {
        p.text(at, Align2::CENTER_CENTER, solid, f.clone(), Color32::from_rgb(24, 24, 28));
        p.text(at, Align2::CENTER_CENTER, outline, f, Color32::from_rgb(96, 96, 104)); // shows the inner lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perft(p: &Pos, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        p.legal().into_iter().map(|m| perft(&p.play(m), depth - 1)).sum()
    }

    #[test]
    fn perft_start_position() {
        let p = Pos::start();
        assert_eq!(perft(&p, 1), 20);
        assert_eq!(perft(&p, 2), 400);
        assert_eq!(perft(&p, 3), 8_902);
    }

    #[test]
    fn perft_kiwipete() {
        let p = Pos::fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
        assert_eq!(perft(&p, 1), 48);
        assert_eq!(perft(&p, 2), 2_039);
    }

    #[test]
    fn mate_stalemate_and_promotion() {
        // fool's mate: white is checkmated
        let p = Pos::fen("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3").unwrap();
        assert!(p.legal().is_empty() && p.in_check(true));
        // classic stalemate: black king boxed in, not in check
        let p = Pos::fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
        assert!(p.legal().is_empty() && !p.in_check(false));
        // a pawn on the 7th has four promotions
        let p = Pos::fen("8/P7/8/8/8/8/8/k6K w - - 0 1").unwrap();
        assert_eq!(p.legal().iter().filter(|m| m.from == 48).count(), 4);
    }

    #[test]
    fn clipster_takes_a_free_queen_and_finds_mate() {
        let p = Pos::fen("4k3/8/8/3q4/4P3/8/8/4K3 w - - 0 1").unwrap();
        let m = best_move(&p, 2, 0).unwrap();
        assert_eq!((name(m.from as usize), name(m.to as usize)), ("e4".into(), "d5".into()));
        // mate in one with the rook
        let p = Pos::fen("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1").unwrap();
        let m = best_move(&p, 2, 0).unwrap();
        assert_eq!(name(m.to as usize), "a8");
    }
}
