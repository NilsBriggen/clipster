//! Chess: the full rules, click or drag to move, two players or against Clipster.
//! Clipster plays with an alpha-beta search over material and piece-square habits: classic search, no AI models.

use super::{win98, Frame, Game, Outcome};
use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, Id, Key, Mesh, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, OnceLock};
use std::time::{Duration, Instant};

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

    /// A hash of everything that makes two positions the same: pieces, side to move, castling rights, en passant.
    pub fn key(&self) -> u64 {
        let z = zobrist();
        let mut h = z[769 + (self.castle & 15) as usize];
        for (s, &p) in self.b.iter().enumerate() {
            if p != 0 {
                h ^= z[((p.unsigned_abs() as usize - 1) * 2 + (p < 0) as usize) * 64 + s];
            }
        }
        if !self.white {
            h ^= z[768];
        }
        if let Some(e) = self.ep {
            h ^= z[785 + (e % 8) as usize];
        }
        h
    }

    /// Only kings, or a king and one knight or bishop against a bare king: nobody can mate.
    pub fn dead_draw(&self) -> bool {
        let rest: Vec<i8> = self.b.iter().copied().filter(|p| *p != 0 && p.abs() != KING).collect();
        rest.is_empty() || (rest.len() == 1 && matches!(rest[0].abs(), KNIGHT | BISHOP))
    }
}

/// Random numbers for hashing positions (Zobrist): a piece on a square, the side to move, castling rights, the en-passant file.
fn zobrist() -> &'static [u64] {
    static KEYS: OnceLock<Vec<u64>> = OnceLock::new();
    KEYS.get_or_init(|| {
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        (0..12 * 64 + 1 + 16 + 8)
            .map(|_| {
                // splitmix64 from a fixed seed: a position always gets the same key
                s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = s;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^ (z >> 31)
            })
            .collect()
    })
}

/// A move in algebraic notation (e4, Nbd7, exd5, O-O, e8=Q+), from the position before it.
pub fn san(p: &Pos, m: Move) -> String {
    let (from, to) = (m.from as usize, m.to as usize);
    let kind = p.b[from].abs();
    let letter = |k: i8| b" PNBRQK"[k as usize] as char;
    let mut s = String::new();
    if kind == KING && from.abs_diff(to) == 2 {
        s += if to > from { "O-O" } else { "O-O-O" };
    } else {
        let capture = p.b[to] != 0 || (kind == PAWN && from % 8 != to % 8);
        if kind == PAWN {
            if capture {
                s.push(name(from).remove(0));
            }
        } else {
            s.push(letter(kind));
            // another piece of the same kind could go there too: say which one
            let rivals: Vec<usize> = p.legal().iter().filter(|o| o.to == m.to && o.from != m.from && p.b[o.from as usize] == p.b[from]).map(|o| o.from as usize).collect();
            if !rivals.is_empty() {
                let n = name(from);
                if rivals.iter().all(|o| o % 8 != from % 8) {
                    s.push_str(&n[..1]);
                } else if rivals.iter().all(|o| o / 8 != from / 8) {
                    s.push_str(&n[1..]);
                } else {
                    s += &n;
                }
            }
        }
        if capture {
            s.push('x');
        }
        s += &name(to);
        if m.promo != 0 {
            s.push('=');
            s.push(letter(m.promo));
        }
    }
    let n = p.play(m);
    if n.in_check(n.white) {
        s.push(if n.legal().is_empty() { '#' } else { '+' });
    }
    s
}

// ---------------- Clipster's search ----------------

const VALUE: [i32; 7] = [0, 100, 320, 330, 500, 900, 0];
const MATE: i32 = 100_000;
const MATED_SOON: i32 = MATE - 1000; // scores past this are mates, counted in plies

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

/// `first` (the move that was best here before) leads, then captures, biggest victim by smallest attacker, and promotions.
fn ordered(p: &Pos, mut moves: Vec<Move>, first: Option<Move>) -> Vec<Move> {
    moves.sort_by_key(|m| {
        if Some(*m) == first {
            return i32::MIN;
        }
        let victim = VALUE[p.b[m.to as usize].unsigned_abs() as usize];
        let attacker = if victim > 0 { VALUE[p.b[m.from as usize].unsigned_abs() as usize] } else { 0 };
        let promo = if m.promo != 0 { VALUE[m.promo as usize] } else { 0 };
        attacker - victim * 10 - promo
    });
    moves
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Bound {
    #[default]
    Exact,
    Lower,
    Upper,
}

/// What an earlier search learned about a position (the transposition table).
#[derive(Clone, Copy, Default)]
struct Slot {
    key: u64,
    mv: Option<Move>,
    score: i32,
    depth: i8,
    bound: Bound,
}

/// Mate scores count plies from the root; the table keeps them counted from the position itself.
fn to_table(v: i32, ply: i32) -> i32 {
    if v > MATED_SOON { v + ply } else if v < -MATED_SOON { v - ply } else { v }
}

fn from_table(v: i32, ply: i32) -> i32 {
    if v > MATED_SOON { v - ply } else if v < -MATED_SOON { v + ply } else { v }
}

struct Search<'a> {
    deadline: Instant,
    stop: &'a AtomicBool,
    out: bool, // ran out of time or was called off: every score from now on is meaningless
    nodes: u32,
    table: Vec<Slot>,
    path: Vec<u64>, // the game's positions, then the line being searched (for repetitions)
}

impl<'a> Search<'a> {
    fn new(secs: f32, stop: &'a AtomicBool, table: usize) -> Self {
        Search { deadline: Instant::now() + Duration::from_secs_f32(secs), stop, out: false, nodes: 0, table: vec![Slot::default(); table], path: Vec::new() }
    }

    fn halt(&mut self) -> bool {
        self.nodes += 1;
        if self.nodes.is_multiple_of(512) && (Instant::now() >= self.deadline || self.stop.load(Ordering::Relaxed)) {
            self.out = true;
        }
        self.out
    }

    /// Only captures, until things are quiet, so a search never stops in the middle of a trade.
    fn quiet(&mut self, p: &Pos, mut alpha: i32, beta: i32, depth: u32) -> i32 {
        if self.halt() {
            return 0;
        }
        let stand = eval(p) * if p.white { 1 } else { -1 };
        if stand >= beta || depth == 0 {
            return stand;
        }
        alpha = alpha.max(stand);
        let caps: Vec<Move> = p.pseudo().into_iter().filter(|m| p.b[m.to as usize] != 0 || m.promo != 0).collect();
        for m in ordered(p, caps, None) {
            let n = p.play(m);
            if n.in_check(p.white) {
                continue;
            }
            let s = -self.quiet(&n, -beta, -alpha, depth - 1);
            if s >= beta {
                return s;
            }
            alpha = alpha.max(s);
        }
        alpha
    }

    fn node(&mut self, p: &Pos, depth: i32, mut alpha: i32, beta: i32, ply: i32) -> i32 {
        if self.halt() {
            return 0;
        }
        let key = p.key();
        // a position seen before since the last capture or pawn move: heading for a repetition draw
        if p.half >= 100 || p.dead_draw() || self.path.iter().rev().take(p.half as usize).any(|&k| k == key) {
            return 0;
        }
        let moves = p.legal();
        if moves.is_empty() {
            return if p.in_check(p.white) { -MATE + ply } else { 0 };
        }
        if depth <= 0 {
            return self.quiet(p, alpha, beta, 4);
        }
        let i = key as usize & (self.table.len() - 1);
        let slot = self.table[i];
        let known = slot.key == key;
        if known && slot.depth as i32 >= depth {
            let v = from_table(slot.score, ply);
            if slot.bound == Bound::Exact || (slot.bound == Bound::Lower && v >= beta) || (slot.bound == Bound::Upper && v <= alpha) {
                return v;
            }
        }
        let start = alpha;
        let (mut best, mut best_move) = (-MATE, None);
        self.path.push(key);
        for m in ordered(p, moves, if known { slot.mv } else { None }) {
            let v = -self.node(&p.play(m), depth - 1, -beta, -alpha, ply + 1);
            if self.out {
                break;
            }
            if v > best {
                (best, best_move) = (v, Some(m));
            }
            alpha = alpha.max(v);
            if alpha >= beta {
                break;
            }
        }
        self.path.pop();
        if self.out {
            return 0;
        }
        let bound = if best <= start { Bound::Upper } else if best >= beta { Bound::Lower } else { Bound::Exact };
        self.table[i] = Slot { key, mv: best_move, score: to_table(best, ply), depth: depth as i8, bound };
        best
    }
}

/// Clipster's pick: he searches one ply deeper at a time until `secs` are up (or `stop` is set), at most `depth` plies.
/// `seen` = the game's earlier positions, for repetitions. `wobble` adds a little noise so easy games differ.
pub fn best_move(p: &Pos, seen: &[u64], depth: i32, secs: f32, wobble: i32, stop: &AtomicBool) -> Option<Move> {
    use rand::{seq::SliceRandom, Rng};
    let mut rng = rand::thread_rng();
    let mut moves = p.legal();
    moves.shuffle(&mut rng); // equal moves come out in a different order each game
    let mut moves: Vec<(Move, i32)> = ordered(p, moves, None).into_iter().map(|m| (m, if wobble > 0 { rng.gen_range(-wobble..=wobble) } else { 0 })).collect();
    let mut best = moves.first()?.0;
    let mut s = Search::new(secs, stop, 1 << 16);
    s.path = seen.to_vec();
    s.path.push(p.key());
    let start = Instant::now();
    for d in 1..=depth {
        let mut alpha = -MATE - 1;
        let mut pick = None;
        for &(m, noise) in &moves {
            let v = -s.node(&p.play(m), d - 1, -MATE - 1, noise - alpha, 1) + noise;
            if s.out {
                break;
            }
            if v > alpha {
                (alpha, pick) = (v, Some(m));
            }
        }
        // even a cut-short depth is good for its pick: the old best was searched first, so the pick beat it
        if let Some(m) = pick {
            best = m;
        }
        if s.out || alpha.abs() > MATED_SOON || start.elapsed().as_secs_f32() > secs / 2.0 {
            break; // out of time, a mate is certain, or the next depth wouldn't finish anyway
        }
        if let Some(i) = moves.iter().position(|x| x.0 == best) {
            let b = moves.remove(i);
            moves.insert(0, b);
        }
    }
    Some(best)
}

/// The move from `before` to `after` hands the other side at least a minor piece's worth of material by captures alone.
pub fn hangs(before: &Pos, after: &Pos) -> bool {
    static NEVER: AtomicBool = AtomicBool::new(false);
    let mut s = Search::new(0.1, &NEVER, 0);
    let then = eval(before) * if before.white { 1 } else { -1 };
    let now = -s.quiet(after, -MATE, MATE, 4);
    !s.out && then - now >= 280
}

// ---------------- the window ----------------

const SQ: f32 = 45.0;
const M: f32 = 14.0; // room for the a–h / 1–8 labels all around the board
const SIDE: f32 = 140.0; // the panel right of the board
const BOARD: f32 = 8.0 * SQ + 4.0 + 2.0 * M;
const PIECE: f32 = 44.0; // glyph size of a piece on the board
const ROW: f32 = 16.0; // a line of the move list
const SLIDE: f32 = 0.15; // seconds a move takes to slide
const THINK_MIN: Duration = Duration::from_millis(600);
const END_PAUSE: f32 = 0.4; // seconds to look at the final position before the result box

const LIGHT_SQ: Color32 = Color32::from_rgb(238, 232, 214);
const DARK_SQ: Color32 = Color32::from_rgb(132, 156, 182);

/// One move of the game, kept so it can be taken back and listed.
struct Ply {
    before: Pos,
    mv: Move,
    san: String,
    took: i8, // the piece it captured, 0 = none
}

/// Clipster's search, running on its own thread.
struct Thinking {
    rx: mpsc::Receiver<(u64, Option<Move>)>,
    id: u64,
    stop: Arc<AtomicBool>,
}

impl Drop for Thinking {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed); // a search nobody waits for any more stops at once
    }
}

/// A move being animated: the piece glides from `from` to `to`; what it took stays put until it arrives.
struct Slide {
    mv: Move,
    took: i8,
    took_at: usize,
    k: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum End {
    Mate { white_won: bool },
    Draw(&'static str),
}

pub struct Chess {
    pub pos: Pos,
    legal: Vec<Move>,
    played: Vec<Ply>,
    sel: Option<usize>,
    drag: Option<usize>,
    promo: Option<(u8, u8)>,
    slide: Option<Slide>,
    mode: usize, // see `levels`
    flip: bool,
    think: Option<Thinking>,
    searches: u64,
    end: Option<End>,
    reported: bool, // Won/Lost went out for this game (an undo and a second mate don't count twice)
    end_age: f32,
    scroll: f32, // move-list rows scrolled up from the newest
    clock: f32,
    burst: bool,
    confetti: win98::Confetti,
}

impl Chess {
    pub fn new() -> Self {
        let pos = Pos::start();
        Chess {
            legal: pos.legal(),
            pos,
            played: Vec::new(),
            sel: None,
            drag: None,
            promo: None,
            slide: None,
            mode: 0,
            flip: false,
            think: None,
            searches: 0,
            end: None,
            reported: false,
            end_age: 0.0,
            scroll: 0.0,
            clock: 0.0,
            burst: false,
            confetti: win98::Confetti::default(),
        }
    }

    fn vs_clipster(&self) -> bool {
        self.mode < 4
    }

    fn hard(&self) -> bool {
        self.mode == 1 || self.mode == 3
    }

    /// The colour at the bottom of the board, the player's own against Clipster.
    fn you_white(&self) -> bool {
        self.mode != 2 && self.mode != 3
    }

    fn clipster_to_move(&self) -> bool {
        self.vs_clipster() && self.pos.white != self.you_white()
    }

    fn my_turn(&self) -> bool {
        self.end.is_none() && !self.clipster_to_move()
    }

    fn mine(&self, s: usize) -> bool {
        self.pos.b[s] != 0 && (self.pos.b[s] > 0) == self.pos.white
    }

    fn find(&self, from: usize, to: usize) -> Option<Move> {
        self.legal.iter().find(|m| m.from as usize == from && m.to as usize == to).copied()
    }

    /// Who plays a colour, for the panel.
    fn who(&self, white: bool) -> &'static str {
        match (self.vs_clipster(), white == self.you_white(), white) {
            (true, true, _) => "You",
            (true, false, _) => "Clipster",
            (false, _, true) => "White",
            (false, _, false) => "Black",
        }
    }

    /// A pawn reaching the last rank asks what it becomes first.
    fn try_move(&mut self, m: Move, slide: bool, out: &mut Vec<Outcome>) {
        if m.promo != 0 {
            self.promo = Some((m.from, m.to));
        } else {
            self.make(m, slide, out);
        }
    }

    /// Play a legal move: record it, settle the game if it's over, and say what Clipster should comment on.
    fn make(&mut self, m: Move, slide: bool, out: &mut Vec<Outcome>) {
        let before = self.pos.clone();
        let piece = before.b[m.from as usize];
        let (took, took_at) = if before.b[m.to as usize] != 0 {
            (before.b[m.to as usize], m.to as usize)
        } else if piece.abs() == PAWN && m.from % 8 != m.to % 8 {
            (-piece.signum() * PAWN, (m.from / 8 * 8 + m.to % 8) as usize) // en passant
        } else {
            (0, 0)
        };
        self.pos = before.play(m);
        self.legal = self.pos.legal();
        self.played.push(Ply { san: san(&before, m), before, mv: m, took });
        (self.sel, self.drag, self.promo, self.scroll) = (None, None, None, 0.0);
        self.slide = slide.then_some(Slide { mv: m, took, took_at, k: 0.0 });
        self.end = self.verdict();
        if !self.vs_clipster() {
            return; // two players: nothing for Clipster to keep score of
        }
        let by_clipster = (piece > 0) != self.you_white();
        match self.end {
            Some(End::Mate { white_won }) if !self.reported => {
                self.reported = true;
                let you_won = white_won == self.you_white();
                self.burst = you_won;
                out.push(if you_won { Outcome::Won("chess") } else { Outcome::Lost("chess") });
            }
            Some(_) => {}
            None if by_clipster => {
                if self.pos.in_check(self.pos.white) {
                    out.push(Outcome::Moment("chess", "check"));
                }
                if took.abs() >= KNIGHT {
                    out.push(Outcome::Moment("chess", "capture"));
                }
            }
            None => {
                if hangs(&self.played[self.played.len() - 1].before, &self.pos) {
                    out.push(Outcome::Moment("chess", "blunder"));
                }
            }
        }
    }

    /// How many times the current position has stood on the board (threefold repetition at 3).
    fn repetitions(&self) -> usize {
        let key = self.pos.key();
        1 + self.played.iter().rev().take(self.pos.half as usize).filter(|p| p.before.key() == key).count()
    }

    fn verdict(&self) -> Option<End> {
        if self.legal.is_empty() {
            return Some(if self.pos.in_check(self.pos.white) { End::Mate { white_won: !self.pos.white } } else { End::Draw("Stalemate: nobody can move.") });
        }
        if self.pos.half >= 100 {
            Some(End::Draw("Fifty moves without a capture or pawn move."))
        } else if self.repetitions() >= 3 {
            Some(End::Draw("The same position came up three times."))
        } else if self.pos.dead_draw() {
            Some(End::Draw("Not enough pieces left to mate."))
        } else {
            None
        }
    }

    /// Against Clipster: is there a move of yours to take back?
    fn can_undo(&self) -> bool {
        if self.vs_clipster() { self.played.iter().any(|p| p.before.white == self.you_white()) } else { !self.played.is_empty() }
    }

    /// Take back the last move; against Clipster, your last move and his reply (and stop him thinking).
    fn undo(&mut self) {
        self.think = None;
        while let Some(ply) = self.played.pop() {
            self.pos = ply.before;
            if !self.vs_clipster() || self.pos.white == self.you_white() {
                break;
            }
        }
        self.legal = self.pos.legal();
        (self.end, self.sel, self.drag, self.promo, self.slide) = (None, None, None, None, None);
        (self.end_age, self.scroll) = (0.0, 0.0);
    }

    /// The result box's icon and lines.
    fn result(&self, end: End) -> (win98::Icon, String, String) {
        let moves = format!("{} moves played.", self.played.len().div_ceil(2));
        match end {
            End::Mate { white_won } if self.vs_clipster() && white_won == self.you_white() => (win98::Icon::Trophy, "You beat Clipster!".into(), format!("Checkmate. {moves}")),
            End::Mate { .. } if self.vs_clipster() => (win98::Icon::Info, "Clipster wins.".into(), format!("Checkmate. {moves}")),
            End::Mate { white_won } => (win98::Icon::Trophy, format!("{} wins!", if white_won { "White" } else { "Black" }), format!("Checkmate. {moves}")),
            End::Draw(why) => (win98::Icon::Info, "Draw.".into(), why.into()),
        }
    }

    /// Clipster's turn: think on a thread so the window never freezes, and pick up the answer when it's in.
    fn clipster(&mut self, ctx: &egui::Context, out: &mut Vec<Outcome>) {
        if let Some(t) = &self.think {
            match t.rx.try_recv() {
                Ok((id, m)) => {
                    let current = id == t.id; // a search from before New or Undo is ignored
                    self.think = None;
                    if let Some(m) = m.filter(|m| current && self.legal.contains(m)) {
                        self.make(m, true, out);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.think = None,
            }
            return;
        }
        if !self.clipster_to_move() || self.end.is_some() {
            return;
        }
        self.searches += 1;
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let (pos, id, halt, ctx) = (self.pos.clone(), self.searches, stop.clone(), ctx.clone());
        let seen: Vec<u64> = self.played.iter().map(|p| p.before.key()).collect();
        let (depth, secs, wobble) = if self.hard() { (8, 1.5, 4) } else { (2, 0.5, 30) };
        std::thread::spawn(move || {
            let start = Instant::now();
            let m = best_move(&pos, &seen, depth, secs, wobble, &halt);
            // never answer instantly: let the player watch their own move land first
            std::thread::sleep(THINK_MIN.saturating_sub(start.elapsed()));
            if tx.send((id, m)).is_ok() {
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
        });
        self.think = Some(Thinking { rx, id, stop });
    }

    /// The board: squares, highlights, pieces, coordinates, clicks and drags. Returns the dragged piece and where it is.
    fn board(&mut self, ui: &mut Ui, board: Rect, bottom_white: bool, input: bool, out: &mut Vec<Outcome>) -> Option<(Pos2, i8)> {
        let o = board.min + vec2(2.0, 2.0);
        let cell = |s: usize| {
            let (f, r) = ((s % 8) as f32, (s / 8) as f32);
            let (x, y) = if bottom_white { (f, 7.0 - r) } else { (7.0 - f, r) };
            Rect::from_min_size(o + vec2(x * SQ, y * SQ), vec2(SQ, SQ))
        };
        let at = |pt: Pos2| {
            let v = (pt - o) / SQ;
            let (x, y) = (v.x.floor() as i32, v.y.floor() as i32);
            let (f, r) = if bottom_white { (x, 7 - y) } else { (7 - x, y) };
            ((0..8).contains(&x) && (0..8).contains(&y)).then_some((r * 8 + f) as usize)
        };
        let resp = ui.interact(board, Id::unique("chess-board"), Sense::click_and_drag());
        let picking = self.promo.is_some();
        let touch = input && self.my_turn() && !picking;
        if touch {
            if resp.drag_started() {
                if let Some(s) = ui.input(|i| i.pointer.press_origin()).and_then(at).filter(|&s| self.mine(s)) {
                    (self.drag, self.sel) = (Some(s), Some(s));
                }
            }
            if resp.drag_stopped() {
                if let Some(from) = self.drag.take() {
                    let to = ui.input(|i| i.pointer.latest_pos()).and_then(at);
                    match to.and_then(|t| self.find(from, t)) {
                        Some(m) => self.try_move(m, false, out),
                        None if to == Some(from) => {} // dropped back: stays picked up, like a click
                        None => self.sel = None,
                    }
                }
            }
            if resp.clicked() {
                if let Some(s) = resp.interact_pointer_pos().and_then(at) {
                    match self.sel.and_then(|from| self.find(from, s)) {
                        Some(m) => self.try_move(m, true, out),
                        None if self.mine(s) && self.sel != Some(s) => self.sel = Some(s),
                        None => self.sel = None,
                    }
                }
            }
        } else {
            self.drag = None;
        }
        let hover = if touch { ui.input(|i| i.pointer.hover_pos()).filter(|p| board.contains(*p)).and_then(at) } else { None };
        let from = self.drag.or(self.sel);
        let targets: Vec<usize> = from.map_or(Vec::new(), |s| self.legal.iter().filter(|m| m.from as usize == s).map(|m| m.to as usize).collect());
        if touch {
            let icon = if self.drag.is_some() {
                Some(CursorIcon::Grabbing)
            } else if hover.is_some_and(|s| self.mine(s)) {
                Some(CursorIcon::Grab)
            } else if hover.is_some_and(|s| targets.contains(&s)) {
                Some(CursorIcon::PointingHand)
            } else {
                None
            };
            if let Some(icon) = icon {
                ui.ctx().set_cursor_icon(icon);
            }
        }

        let p = ui.painter().clone();
        win98::bevel(&p, board, false);
        let last = self.played.last().map(|p| p.mv);
        for s in 0..64 {
            let light = (s % 8 + s / 8) % 2 == 1;
            // amber, mixed for each square colour (a plain overlay turns olive on the blue squares)
            let c = if self.sel == Some(s) {
                if light { Color32::from_rgb(250, 208, 96) } else { Color32::from_rgb(222, 164, 62) }
            } else if last.is_some_and(|m| m.from as usize == s || m.to as usize == s) {
                if light { Color32::from_rgb(247, 224, 146) } else { Color32::from_rgb(224, 178, 84) }
            } else if light {
                LIGHT_SQ
            } else {
                DARK_SQ
            };
            p.rect_filled(cell(s), 0.0, c);
        }
        if self.pos.in_check(self.pos.white) {
            let king = if self.pos.white { KING } else { -KING };
            if let Some(k) = self.pos.b.iter().position(|&x| x == king) {
                glow(&p.with_clip_rect(cell(k)), cell(k).center(), SQ * 0.66, Color32::from_rgba_unmultiplied(235, 30, 20, 230));
            }
        }
        if let Some(h) = hover.filter(|&h| self.mine(h) || targets.contains(&h)) {
            p.rect_stroke(cell(h), 0.0, Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 210)), StrokeKind::Inside);
        }
        for &t in &targets {
            let c = cell(t).center();
            let ink = Color32::from_black_alpha(if hover == Some(t) { 110 } else { 70 });
            if self.pos.b[t] != 0 {
                p.circle_stroke(c, SQ * 0.44, Stroke::new(3.0, ink));
            } else {
                p.circle_filled(c, SQ * if hover == Some(t) { 0.18 } else { 0.14 }, ink);
            }
        }

        // pieces; a sliding move draws its piece (and a castling rook) on the way instead
        let castle_rook = |m: Move| {
            let (from, to) = (m.from as usize, m.to as usize);
            (self.pos.b[to].abs() == KING && from.abs_diff(to) == 2).then(|| if to > from { (from + 3, from + 1) } else { (from - 4, from - 1) })
        };
        let rook = self.slide.as_ref().and_then(|s| castle_rook(s.mv));
        for s in 0..64 {
            let pc = self.pos.b[s];
            let moving = self.slide.as_ref().is_some_and(|sl| sl.mv.to as usize == s) || rook.is_some_and(|(_, rt)| rt == s);
            if pc != 0 && !moving && self.drag != Some(s) && self.promo.is_none_or(|(f, _)| f as usize != s) {
                piece(&p, cell(s).center(), pc, PIECE, false);
            }
        }
        if let Some(sl) = &self.slide {
            let t = 1.0 - (1.0 - sl.k.clamp(0.0, 1.0)).powi(3);
            if sl.took != 0 && sl.k < 0.75 {
                piece(&p, cell(sl.took_at).center(), sl.took, PIECE, false);
            }
            if let Some((rf, rt)) = rook {
                piece(&p, cell(rf).center().lerp(cell(rt).center(), t), self.pos.b[rt], PIECE, false);
            }
            let (f, to) = (sl.mv.from as usize, sl.mv.to as usize);
            piece(&p, cell(f).center().lerp(cell(to).center(), t), self.pos.b[to], PIECE, false);
        }
        for i in 0..8 {
            let file = ((b'a' + if bottom_white { i } else { 7 - i }) as char).to_string();
            let rank = (if bottom_white { 8 - i } else { i + 1 }).to_string();
            let (x, y) = (o.x + (i as f32 + 0.5) * SQ, o.y + (i as f32 + 0.5) * SQ);
            for (pt, t) in [(pos2(x, board.min.y - M / 2.0), &file), (pos2(x, board.max.y + M / 2.0), &file), (pos2(board.min.x - M / 2.0, y), &rank), (pos2(board.max.x + M / 2.0, y), &rank)] {
                p.text(pt, Align2::CENTER_CENTER, t, win98::font(11.0), win98::TEXT);
            }
        }

        // a pawn reached the end: the choices pop up on its square, in a column towards the middle
        if let Some((from, to)) = self.promo {
            let side = self.pos.b[from as usize].signum();
            let c = cell(to as usize);
            let down = c.center().y < board.center().y;
            let column = Rect::from_min_size(pos2(c.min.x, if down { c.min.y } else { c.max.y - 4.0 * SQ }), vec2(SQ, 4.0 * SQ)).expand(3.0);
            p.rect_filled(column.translate(vec2(4.0, 4.0)), 0.0, Color32::from_black_alpha(70));
            win98::panel(&p, column, win98::FACE, true);
            let mut chosen = None;
            for (i, k) in [QUEEN, KNIGHT, ROOK, BISHOP].into_iter().enumerate() {
                let y = if down { c.min.y + i as f32 * SQ } else { c.max.y - (i + 1) as f32 * SQ };
                let br = Rect::from_min_size(pos2(c.min.x, y), vec2(SQ, SQ)).shrink(1.0);
                let b = win98::button(ui, br, Id::unique(("chess-promo", i)), "", input);
                let push = if b.is_pointer_button_down_on() { vec2(1.0, 1.0) } else { Vec2::ZERO };
                piece(&p, br.center() + push, k * side, PIECE - 4.0, false);
                if b.clicked() {
                    chosen = Some(k);
                }
            }
            if let Some(k) = chosen {
                self.make(Move { from, to, promo: k }, false, out);
            } else if input && picking && (resp.clicked() || ui.input(|i| i.key_pressed(Key::Escape))) {
                (self.promo, self.sel) = (None, None);
            }
        }
        self.drag.zip(ui.input(|i| i.pointer.latest_pos())).map(|(s, at)| (at, self.pos.b[s]))
    }

    /// The panel beside the board: both players with what they've captured, the move list, whose turn, and the buttons.
    fn side(&mut self, ui: &mut Ui, r: Rect, bottom_white: bool, input: bool) {
        let p = ui.painter().clone();
        let worth = [0, 1, 3, 3, 5, 9, 0];
        let lead: i32 = self.pos.b.iter().map(|&x| x.signum() as i32 * worth[x.unsigned_abs() as usize]).sum();
        for (y, white) in [(r.min.y, !bottom_white), (r.min.y + 292.0, bottom_white)] {
            let to_move = self.end.is_none() && self.pos.white == white;
            piece(&p, pos2(r.min.x + 8.0, y + 9.0), if white { KING } else { -KING }, 18.0, false);
            let name = self.who(white);
            let tag = Rect::from_min_size(pos2(r.min.x + 18.0, y), vec2(p.layout_no_wrap(name.into(), win98::bold(13.0), win98::TEXT).size().x + 8.0, 18.0));
            if to_move {
                p.rect_filled(tag, 0.0, win98::NAVY); // whose turn: selected, 98 style
            }
            p.text(pos2(tag.min.x + 4.0, tag.center().y), Align2::LEFT_CENTER, name, win98::bold(13.0), if to_move { Color32::WHITE } else { win98::TEXT });
            let ahead = if white { lead } else { -lead };
            if ahead > 0 {
                p.text(pos2(r.max.x - 2.0, y + 9.0), Align2::RIGHT_CENTER, format!("+{ahead}"), win98::bold(13.0), Color32::from_rgb(0, 110, 30));
            }
            let mut took: Vec<i8> = self.played.iter().filter(|p| p.before.white == white && p.took != 0).map(|p| p.took).collect();
            took.sort_by_key(|t| -VALUE[t.unsigned_abs() as usize]);
            let step = 11f32.min((r.width() - 12.0) / took.len().max(1) as f32);
            for (i, &t) in took.iter().enumerate() {
                piece(&p, pos2(r.min.x + 7.0 + i as f32 * step, y + 28.0), t, 18.0, false);
            }
        }

        // the moves so far; the newest is selected and the wheel scrolls back
        let list = Rect::from_min_max(pos2(r.min.x, r.min.y + 40.0), pos2(r.max.x, r.min.y + 288.0));
        p.rect_filled(list, 0.0, Color32::WHITE);
        win98::bevel(&p, list, false);
        let inner = list.shrink(3.0);
        let rows = self.played.len().div_ceil(2);
        let visible = (inner.height() / ROW) as usize;
        let hidden = rows.saturating_sub(visible);
        if ui.rect_contains_pointer(list) {
            self.scroll += ui.input(|i| i.smooth_scroll_delta.y) / ROW;
        }
        self.scroll = self.scroll.clamp(0.0, hidden as f32);
        let first = hidden - self.scroll.round() as usize;
        for row in first..(first + visible).min(rows) {
            let y = inner.min.y + (row - first) as f32 * ROW;
            p.text(pos2(inner.min.x + 22.0, y + ROW / 2.0), Align2::RIGHT_CENTER, format!("{}.", row + 1), win98::font(12.0), win98::GRAY_TEXT);
            for (side, ply) in self.played.iter().enumerate().skip(row * 2).take(2) {
                let cell = Rect::from_min_size(pos2(inner.min.x + 26.0 + (side % 2) as f32 * 48.0, y), vec2(46.0, ROW));
                let newest = side + 1 == self.played.len();
                if newest {
                    p.rect_filled(cell, 0.0, win98::NAVY);
                }
                p.text(pos2(cell.min.x + 3.0, cell.center().y), Align2::LEFT_CENTER, &ply.san, win98::font(12.5), if newest { Color32::WHITE } else { win98::TEXT });
            }
        }
        if hidden > 0 {
            // a 98 scroll bar showing where you are
            let track = Rect::from_min_max(pos2(inner.max.x - 12.0, inner.min.y), inner.max);
            p.rect_filled(track, 0.0, win98::SOFT);
            let h = (track.height() * visible as f32 / rows as f32).max(14.0);
            let y = track.min.y + (track.height() - h) * first as f32 / hidden as f32;
            win98::panel(&p, Rect::from_min_size(pos2(track.min.x, y), vec2(12.0, h)), win98::FACE, true);
        }

        // whose turn, or Clipster thinking with three hopping dots
        let y = r.min.y + 344.0;
        let check = self.end.is_none() && self.pos.in_check(self.pos.white);
        let (text, ink) = if self.end.is_some() {
            ("Game over".to_string(), win98::TEXT)
        } else if self.think.is_some() || self.clipster_to_move() {
            ("Clipster is thinking".to_string(), win98::NAVY)
        } else if self.vs_clipster() {
            (if check { "Check! Your move".into() } else { "Your move".into() }, if check { Color32::from_rgb(190, 20, 20) } else { win98::TEXT })
        } else {
            (format!("{}{} to move", if check { "Check! " } else { "" }, if self.pos.white { "White" } else { "Black" }), if check { Color32::from_rgb(190, 20, 20) } else { win98::TEXT })
        };
        let w = p.layout_no_wrap(text.clone(), win98::font(13.0), ink).size().x;
        p.text(pos2(r.min.x + 2.0, y), Align2::LEFT_CENTER, text, win98::font(13.0), ink);
        if self.think.is_some() {
            for i in 0..3 {
                let hop = (self.clock * 7.0 - i as f32 * 0.9).sin().max(0.0) * 3.0;
                p.circle_filled(pos2(r.min.x + w + 6.0 + i as f32 * 5.0, y + 4.0 - hop), 1.6, win98::NAVY);
            }
        }

        let bw = (r.width() - 8.0) / 3.0;
        let at = |i: f32| Rect::from_min_size(pos2((r.min.x + i * (bw + 4.0)).round(), r.max.y - 23.0), vec2(bw.round(), 23.0));
        if win98::button(ui, at(0.0), Id::unique("chess-undo"), "Undo", input && self.can_undo()).clicked() {
            self.undo();
        }
        if win98::button(ui, at(1.0), Id::unique("chess-flip"), "Flip", input).clicked() {
            self.flip = !self.flip;
        }
        if win98::button(ui, at(2.0), Id::unique("chess-new"), "New", input).clicked() {
            self.new_round();
        }
    }
}

/// A soft round glow (the king in check): `col` in the middle, fading out at radius `r`.
fn glow(p: &Painter, c: Pos2, r: f32, col: Color32) {
    let mut m = Mesh::default();
    m.colored_vertex(c, col);
    for i in 0..=32 {
        let a = i as f32 / 32.0 * std::f32::consts::TAU;
        m.colored_vertex(c + vec2(a.cos(), a.sin()) * r, Color32::TRANSPARENT);
    }
    for i in 0..32 {
        m.add_triangle(0, i + 1, i + 2);
    }
    p.add(m);
}

/// A piece from the bundled symbol font, so it looks the same on every system: the solid glyph with a
/// one-pixel rim that also draws its inner lines, dark around white pieces and light around black ones.
fn piece(p: &Painter, c: Pos2, pc: i8, size: f32, lifted: bool) {
    let glyph = ["", "♟", "♞", "♝", "♜", "♛", "♚"][pc.unsigned_abs() as usize];
    let f = win98::glyphs(size);
    let at = c + vec2(0.0, size * 0.02);
    let (body, rim) = if pc > 0 { (Color32::from_rgb(252, 251, 246), Color32::from_rgb(22, 22, 28)) } else { (Color32::from_rgb(36, 36, 44), Color32::from_rgb(228, 228, 234)) };
    let (drop, shade) = if lifted { (vec2(3.0, 6.0), 70) } else { (vec2(1.0, 2.0), 50) };
    p.text(at + drop, Align2::CENTER_CENTER, glyph, f.clone(), Color32::from_black_alpha(shade));
    for d in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0), vec2(1.0, -1.0)] {
        p.text(at + d, Align2::CENTER_CENTER, glyph, f.clone(), rim);
    }
    p.text(at, Align2::CENTER_CENTER, glyph, f, body);
}

impl Game for Chess {
    fn size(&self) -> Vec2 {
        vec2(BOARD + 8.0 + SIDE, BOARD)
    }

    fn new_round(&mut self) {
        let (mode, flip, searches) = (self.mode, self.flip, self.searches);
        *self = Chess::new(); // dropping a running search calls it off
        (self.mode, self.flip, self.searches) = (mode, flip, searches);
    }

    fn levels(&self) -> &'static [&'static str] {
        &["vs Clipster (easy)", "vs Clipster (hard)", "As Black (easy)", "As Black (hard)", "Two players"]
    }
    fn level(&self) -> usize {
        self.mode
    }
    fn set_level(&mut self, i: usize) {
        self.mode = i.min(4);
        self.flip = false;
        self.new_round();
    }

    fn status(&self) -> String {
        if let Some(end) = self.end {
            let (_, head, why) = self.result(end);
            return format!("{head} {why} Any key or click: new game.");
        }
        let check = if self.pos.in_check(self.pos.white) { "Check! " } else { "" };
        if self.think.is_some() || self.clipster_to_move() {
            format!("{check}Clipster is thinking…")
        } else if self.vs_clipster() {
            let last = self.played.last().map_or(String::new(), |p| format!("Clipster played {}. ", p.san));
            format!("{check}{last}Your move: you play {}.", if self.you_white() { "White" } else { "Black" })
        } else {
            format!("{check}{} to move.", if self.pos.white { "White" } else { "Black" })
        }
    }

    fn finished(&self) -> bool {
        self.end.is_some()
    }

    fn live(&self) -> bool {
        false // turn by turn: nothing to pause; the board asks for its own frames while something moves
    }

    fn ui(&mut self, ui: &mut Ui, r: Rect, f: &mut Frame) {
        let out = &mut *f.out;
        self.clock += f.dt;
        if let Some(s) = &mut self.slide {
            s.k += f.dt / SLIDE;
        }
        if self.slide.as_ref().is_some_and(|s| s.k >= 1.0) {
            self.slide = None;
        }
        if self.end.is_some() {
            self.end_age += f.dt;
        }
        self.confetti.update(f.dt);
        self.clipster(ui.ctx(), out);

        let board = Rect::from_min_size(r.min + vec2(M, M), vec2(8.0 * SQ + 4.0, 8.0 * SQ + 4.0));
        let bottom_white = self.you_white() != self.flip;
        let result = self.end.filter(|_| self.end_age > END_PAUSE);
        let input = f.input && result.is_none();
        let held = self.board(ui, board, bottom_white, input, out);
        self.side(ui, Rect::from_min_max(pos2(r.max.x - SIDE, r.min.y), r.max), bottom_white, input);
        if let Some((at, pc)) = held {
            piece(ui.painter(), at, pc, PIECE, true);
        }
        if std::mem::take(&mut self.burst) {
            self.confetti.burst(board.center(), 120);
        }
        if let Some(end) = result {
            let (icon, head, why) = self.result(end);
            let buttons: &[&str] = if self.can_undo() { &["New game", "Undo"] } else { &["New game"] };
            match win98::dialog(ui, r, "Chess", icon, &[head, why], buttons) {
                Some(0) => self.new_round(),
                Some(_) => self.undo(),
                None => {}
            }
        }
        self.confetti.draw(&ui.painter().with_clip_rect(r));
        let busy = self.think.is_some() || self.slide.is_some() || self.confetti.active() || (self.end.is_some() && self.end_age <= END_PAUSE);
        if busy {
            ui.ctx().request_repaint();
            ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        }
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

    fn sq_of(s: &str) -> u8 {
        let b = s.as_bytes();
        (b[1] - b'1') * 8 + (b[0] - b'a')
    }

    /// Play moves given as "e2e4" (a promotion picks the queen), as whoever is to move.
    fn play(c: &mut Chess, moves: &str, out: &mut Vec<Outcome>) {
        for mv in moves.split_whitespace() {
            let m = c.find(sq_of(&mv[..2]) as usize, sq_of(&mv[2..4]) as usize).unwrap_or_else(|| panic!("{mv} is legal"));
            c.make(m, false, out);
        }
    }

    fn decided(out: &[Outcome]) -> bool {
        out.iter().any(|o| matches!(o, Outcome::Won(_) | Outcome::Lost(_)))
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
        let free = AtomicBool::new(false);
        let p = Pos::fen("4k3/8/8/3q4/4P3/8/8/4K3 w - - 0 1").unwrap();
        let m = best_move(&p, &[], 3, 30.0, 0, &free).unwrap();
        assert_eq!((name(m.from as usize), name(m.to as usize)), ("e4".into(), "d5".into()));
        // mate in one with the rook
        let p = Pos::fen("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1").unwrap();
        let m = best_move(&p, &[], 3, 30.0, 0, &free).unwrap();
        assert_eq!(name(m.to as usize), "a8");
    }

    #[test]
    fn clipster_stops_on_time_or_when_called_off() {
        let free = AtomicBool::new(false);
        let t = Instant::now();
        assert!(best_move(&Pos::start(), &[], 40, 0.2, 0, &free).is_some());
        assert!(t.elapsed() < Duration::from_secs(3), "the time cap holds: {:?}", t.elapsed());
        let off = AtomicBool::new(true);
        let t = Instant::now();
        assert!(best_move(&Pos::start(), &[], 40, 60.0, 0, &off).is_some(), "still answers something");
        assert!(t.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn moves_read_in_algebraic_notation() {
        let p = Pos::start();
        let m = |a: &str, b: &str| Move { from: sq_of(a), to: sq_of(b), promo: 0 };
        assert_eq!(san(&p, m("e2", "e4")), "e4");
        assert_eq!(san(&p, m("g1", "f3")), "Nf3");
        let p = Pos::fen("r3k2r/8/8/3p4/4P3/8/8/R3K1NR w KQkq - 0 1").unwrap();
        assert_eq!(san(&p, m("e4", "d5")), "exd5");
        assert_eq!(san(&p, m("e1", "c1")), "O-O-O");
        // two knights can reach d2: name the file
        let p = Pos::fen("4k3/8/8/8/8/8/8/1N2KN2 w - - 0 1").unwrap();
        assert_eq!(san(&p, m("b1", "d2")), "Nbd2");
        // a promotion with mate
        let p = Pos::fen("k7/2P5/1K6/8/8/8/8/8 w - - 0 1").unwrap();
        assert_eq!(san(&p, Move { from: sq_of("c7"), to: sq_of("c8"), promo: QUEEN }), "c8=Q#");
    }

    #[test]
    fn threefold_repetition_is_a_quiet_draw() {
        for mode in [0, 4] {
            let mut c = Chess::new();
            c.mode = mode;
            let mut out = Vec::new();
            play(&mut c, "g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1", &mut out);
            assert_eq!(c.end, None, "twice isn't enough");
            assert_eq!(c.repetitions(), 2);
            play(&mut c, "f6g8", &mut out);
            assert_eq!(c.end, Some(End::Draw("The same position came up three times.")));
            assert!(!decided(&out), "a draw is neither won nor lost: {out:?}");
        }
    }

    #[test]
    fn stalemate_sends_no_result() {
        let mut c = Chess::new();
        c.pos = Pos::fen("7k/8/6K1/8/8/8/8/5Q2 w - - 0 1").unwrap();
        c.legal = c.pos.legal();
        let mut out = Vec::new();
        play(&mut c, "f1f7", &mut out);
        assert_eq!(c.end, Some(End::Draw("Stalemate: nobody can move.")));
        assert!(!decided(&out));
    }

    #[test]
    fn only_games_against_clipster_are_won_or_lost() {
        let fools = "f2f3 e7e5 g2g4 d8h4";
        let scholars = "e2e4 e7e5 f1c4 b8c6 d1h5 g8f6 h5f7";
        let result = |mode: usize, moves: &str| {
            let mut c = Chess::new();
            c.mode = mode;
            let mut out = Vec::new();
            play(&mut c, moves, &mut out);
            assert!(matches!(c.end, Some(End::Mate { .. })));
            out.into_iter().filter(|o| matches!(o, Outcome::Won(_) | Outcome::Lost(_))).collect::<Vec<_>>()
        };
        assert_eq!(result(4, fools), vec![], "two players: nobody beat Clipster");
        assert_eq!(result(4, scholars), vec![]);
        assert_eq!(result(0, scholars), vec![Outcome::Won("chess")]);
        assert_eq!(result(0, fools), vec![Outcome::Lost("chess")]);
        assert_eq!(result(2, fools), vec![Outcome::Won("chess")], "playing Black, you mated Clipster");
    }

    #[test]
    fn undo_takes_back_your_move_and_clipsters_reply() {
        let mut c = Chess::new();
        let mut out = Vec::new();
        play(&mut c, "e2e4 e7e5", &mut out);
        let earlier = c.pos.clone();
        play(&mut c, "g1f3 b8c6", &mut out);
        assert!(c.can_undo());
        c.undo();
        assert_eq!(c.pos, earlier, "exactly as it was, en passant square and all");
        assert_eq!(c.played.len(), 2);
        // Clipster hasn't answered yet: only your move goes
        play(&mut c, "d2d4", &mut out);
        c.undo();
        assert_eq!(c.pos, earlier);
        c.undo();
        assert_eq!(c.pos, Pos::start());
        assert!(!c.can_undo());
        // playing Black, Clipster's opening move alone can't be taken back
        c.mode = 2;
        play(&mut c, "e2e4", &mut out);
        assert!(!c.can_undo());
    }

    #[test]
    fn a_hanging_piece_is_a_blunder_but_a_fair_trade_isnt() {
        let m = |p: &Pos, a: &str, b: &str| p.play(Move { from: sq_of(a), to: sq_of(b), promo: 0 });
        let p = Pos::fen("4k3/8/8/3p4/8/3Q4/8/4K3 w - - 0 1").unwrap();
        assert!(hangs(&p, &m(&p, "d3", "e4")), "the queen walks into the pawn");
        assert!(!hangs(&p, &m(&p, "d3", "h3")));
        let p = Pos::fen("4k3/8/4p3/3n4/8/8/6B1/4K3 w - - 0 1").unwrap();
        assert!(!hangs(&p, &m(&p, "g2", "d5")), "bishop for knight is even");
    }
}
