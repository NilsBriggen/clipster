//! The welcome: a setup and tour in its own Windows-98-style window, for new installs (the setup pages and
//! the tour) and, after an update or on "show me around", the tour alone. Like the Desk it's a view: it keeps
//! the form, draws the pages and hands back commands (the Try it buttons), and at the end what was chosen.
//! The page texts are `## welcome <page>` (plain, the same in both voices) and Clipster's line under his
//! picture is `## welcome_say <page>`.

use crate::games::win98;
use crate::lines;
use eframe::egui::{self, pos2, vec2, Align2, Color32, Id, Painter, Pos2, Rect, Sense, Stroke, Ui};

const W: f32 = 600.0;
const H: f32 = 460.0;

/// (id, chapter, heading, Try it buttons as (label, command)).
const PAGES: [(&str, &str, &str, &[(&str, &str)]); 15] = [
    ("hello", "Setting up", "Welcome to Clipster", &[]),
    ("name", "Setting up", "What should I call you?", &[]),
    ("persona", "Setting up", "Pick my personality", &[]),
    ("notice", "Setting up", "What may I comment on?", &[]),
    ("startup", "Setting up", "Starting up", &[]),
    ("calling", "Talking to me", "How to call me", &[("Try: help", "help")]),
    ("reminders", "Getting things done", "Reminders and to-dos", &[("Try: remind me in 1 minute", "remind me in 1 minute to stretch"), ("Try: my list", "todos")]),
    ("writing", "Getting things done", "Writing help", &[("Try: a letter", "letter")]),
    ("focus", "Focus and your day", "Focus, goals and limits", &[("Try: my time today", "timereport"), ("Try: my goals", "goals")]),
    ("day", "Focus and your day", "Your day and your week", &[("Try: plan my day", "planday")]),
    ("upkeep", "Your computer", "Looking after the computer", &[("Try: a checkup", "desk checkup"), ("Try: tidy plan", "tidy downloads")]),
    ("desk", "Clipster's Desk", "Clipster's Desk", &[("Try: open my Desk", "desk")]),
    ("fun", "Fun and you", "Fun", &[("Try: the arcade", "games")]),
    ("yours", "Fun and you", "Make me yours", &[("Try: what you know", "what do you know about me")]),
    ("done", "You're set", "You're all set", &[]),
];

/// The first page of the tour (after the setup pages).
pub const TOUR: usize = 5;

/// What the setup pages ask.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub name: String,
    pub unhinged: bool,
    pub clip: bool,
    pub programs: bool,
    pub windows: bool,
    pub checkin: bool,
    pub autostart: bool,
    pub updates: bool,
    pub sounds: bool,
    pub open_desk: bool,
}

pub enum Done {
    Finish(Setup),
    /// Cancelled: the personality it was opened with (picking one applies at once, so Cancel puts it back).
    Cancel(bool),
}

#[derive(Default)]
pub struct Welcome {
    open: bool,
    fresh: bool,
    page: usize,
    first: usize, // 0 = with the setup pages, TOUR = the tour alone
    setup: Option<Setup>,
    seed: usize,
    was_unhinged: bool,
}

impl Welcome {
    /// Open at `page` (0 = the whole setup, `TOUR` = just the tour) with the current settings filled in.
    pub fn open_at(&mut self, page: usize, setup: Setup) {
        *self = Welcome { open: true, fresh: true, page, first: page, was_unhinged: setup.unhinged, setup: Some(setup), seed: rand::random::<u8>() as usize };
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn set_page(&mut self, page: usize) {
        self.page = page.clamp(self.first, PAGES.len() - 1);
    }

    /// Every frame. `clip` draws Clipster with his top-left at the given point. Returns the commands to run
    /// (Try it, and the personality as soon as it's picked) and, when the window closes, how it ended.
    pub fn show(&mut self, ctx: &egui::Context, clip: &dyn Fn(&Painter, Pos2)) -> (Vec<String>, Option<Done>) {
        let mut out = vec![];
        let mut done = None;
        if !self.open {
            return (out, None);
        }
        let fresh = std::mem::take(&mut self.fresh);
        if ctx.embed_viewports() {
            egui::Area::new(Id::unique("clipster-welcome-embedded")).fixed_pos(pos2(0.0, 0.0)).show(ctx, |ui| done = self.window(ui, clip, false, &mut out));
        } else {
            let monitor = ctx.input(|i| i.viewport().monitor_size).unwrap_or(vec2(1920.0, 1080.0));
            let at = pos2(((monitor.x - W) / 2.0).max(0.0), ((monitor.y - H) / 2.0).max(0.0));
            let builder = egui::ViewportBuilder::default().with_title("Welcome to Clipster").with_inner_size(vec2(W, H)).with_decorations(false).with_resizable(false).with_position(at);
            ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("clipster-welcome"), builder, |ui, _| {
                if fresh {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                done = self.window(ui, clip, true, &mut out);
                if ui.input(|i| i.viewport().close_requested()) {
                    done = Some(Done::Cancel(self.was_unhinged));
                }
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(60)); // he moves
            });
        }
        if done.is_some() {
            self.open = false;
        }
        (out, done)
    }

    fn window(&mut self, ui: &mut Ui, clip: &dyn Fn(&Painter, Pos2), drag: bool, out: &mut Vec<String>) -> Option<Done> {
        let (win, _) = ui.allocate_exact_size(vec2(W, H), Sense::hover());
        let p = ui.painter().clone();
        let setup = self.setup.get_or_insert_with(|| Setup { name: String::new(), unhinged: true, clip: true, programs: true, windows: true, checkin: true, autostart: false, updates: true, sounds: true, open_desk: true });
        win98::panel(&p, win, win98::FACE, true);
        let bar = Rect::from_min_size(win.min + vec2(4.0, 4.0), vec2(W - 8.0, 22.0));
        if win98::title_bar(ui, bar, "Welcome to Clipster", drag) {
            return Some(Done::Cancel(self.was_unhinged));
        }
        let (id, chapter, heading, tries) = PAGES[self.page];
        // the left panel: Clipster, what he says, the chapters
        let side = Rect::from_min_max(pos2(win.min.x + 10.0, bar.max.y + 10.0), pos2(win.min.x + 180.0, win.max.y - 56.0));
        win98::gradient(&p, side, win98::NAVY, win98::SKY);
        win98::bevel(&p, side, false);
        clip(&p, pos2(side.center().x - 40.0, side.min.y + 18.0));
        let says = lines::all(&format!("welcome_say {id}"));
        let line = says.get((self.seed + self.page) % says.len().max(1)).cloned().unwrap_or_default();
        // at most three lines, so a long name can't push the chapters out of the panel
        let mut job = egui::text::LayoutJob::simple(line.replace("{name}", if setup.name.is_empty() { "friend" } else { &setup.name }), win98::font(11.5), win98::TEXT, side.width() - 24.0);
        job.wrap.max_rows = 3;
        job.wrap.overflow_character = Some('…');
        let g = p.layout_job(job);
        let note = Rect::from_min_size(pos2(side.min.x + 8.0, side.min.y + 158.0), vec2(side.width() - 16.0, g.size().y + 10.0));
        p.rect_filled(note, 0.0, Color32::from_rgb(255, 255, 225)); // the classic yellow tip
        p.rect_stroke(note, 0.0, Stroke::new(1.0, win98::DARK), egui::StrokeKind::Inside);
        p.galley(note.min + vec2(4.0, 5.0), g, win98::TEXT);
        let mut chapters: Vec<(&str, usize)> = vec![];
        for (i, page) in PAGES.iter().enumerate().skip(self.first) {
            if chapters.last().is_none_or(|c| c.0 != page.1) {
                chapters.push((page.1, i));
            }
        }
        let mut y = note.max.y + 12.0;
        for (name, first) in chapters {
            let r = Rect::from_min_size(pos2(side.min.x + 10.0, y), vec2(side.width() - 20.0, 18.0));
            let resp = ui.interact(r, Id::unique(("welcome-chapter", name)), Sense::click());
            let here = name == chapter;
            let text = if here { format!("▸ {name}") } else { format!("   {name}") };
            p.text(r.left_center(), Align2::LEFT_CENTER, text, if here { win98::bold(12.0) } else { win98::font(12.0) }, if resp.hovered() { Color32::from_rgb(255, 255, 160) } else { Color32::WHITE });
            if resp.clicked() {
                self.page = first;
            }
            y += 18.0;
        }
        // the page
        let (x0, x1) = (side.max.x + 16.0, win.max.x - 16.0);
        let mut y = side.min.y + 4.0;
        p.text(pos2(x0, y), Align2::LEFT_TOP, heading, win98::bold(17.0), win98::TEXT);
        y += 32.0;
        let body = lines::all(&format!("welcome {id}")).first().cloned().unwrap_or_default().replace("Meta+Shift+C", crate::sys::DESK_HOTKEY).replace("Meta+C", crate::sys::HOTKEY);
        let g = p.layout(body, win98::font(12.5), win98::TEXT, x1 - x0);
        let h = g.size().y;
        p.galley(pos2(x0, y), g, win98::TEXT);
        y += h + 14.0;
        let check = |ui: &mut Ui, y: &mut f32, label: &str, on: &mut bool, key: &str| {
            let r = Rect::from_min_size(pos2(x0, *y), vec2(13.0, 13.0));
            let row = Rect::from_min_size(pos2(x0, *y - 3.0), vec2(x1 - x0, 19.0));
            if ui.interact(row, Id::unique(("welcome-check", key)), Sense::click()).clicked() {
                *on = !*on;
            }
            ui.painter().rect_filled(r, 0.0, Color32::WHITE);
            win98::bevel(ui.painter(), r, false);
            if *on {
                ui.painter().add(egui::Shape::line(vec![r.min + vec2(3.0, 6.5), r.min + vec2(5.5, 9.5), r.min + vec2(10.0, 3.5)], Stroke::new(2.0, win98::DARK)));
            }
            ui.painter().text(pos2(x0 + 20.0, r.center().y), Align2::LEFT_CENTER, label, win98::font(12.5), win98::TEXT);
            *y += 24.0;
        };
        match id {
            "name" => {
                let r = Rect::from_min_size(pos2(x0, y), vec2(240.0, 24.0));
                p.rect_filled(r, 0.0, Color32::WHITE);
                win98::bevel(&p, r, false);
                let edit = egui::TextEdit::singleline(&mut setup.name).id(Id::unique("welcome-name")).frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(6, 4))).font(win98::font(13.0)).text_color(win98::TEXT).hint_text("your name").char_limit(30);
                let resp = ui.put(r, edit);
                if self.page != self.first && !resp.has_focus() && ui.memory(|m| m.focused().is_none()) {
                    resp.request_focus();
                }
            }
            "persona" => {
                for (unhinged, label, about) in [(false, "Boring", "A regular sarcastic paperclip. Helpful, dry, brief."), (true, "Unhinged", "Thinks he's the god of your computer, trapped since 1997. Moods, memories, prophecies. Same help, more drama.")] {
                    let row = Rect::from_min_size(pos2(x0, y - 2.0), vec2(x1 - x0, 44.0));
                    let c = pos2(x0 + 6.0, y + 7.0);
                    p.circle(c, 6.0, Color32::WHITE, Stroke::new(1.0, win98::SHADOW));
                    p.circle_stroke(c, 5.0, Stroke::new(1.0, win98::DARK));
                    if setup.unhinged == unhinged {
                        p.circle_filled(c, 2.5, win98::DARK);
                    }
                    p.text(pos2(x0 + 20.0, y + 7.0), Align2::LEFT_CENTER, label, win98::bold(12.5), win98::TEXT);
                    let g = p.layout(about.to_string(), win98::font(12.0), win98::GRAY_TEXT, x1 - x0 - 20.0);
                    p.galley(pos2(x0 + 20.0, y + 16.0), g, win98::GRAY_TEXT);
                    if ui.interact(row, Id::unique(("welcome-persona", label)), Sense::click()).clicked() && setup.unhinged != unhinged {
                        setup.unhinged = unhinged;
                        out.push(format!("pref persona {}", if unhinged { "unhinged" } else { "boring" })); // his voice changes right away
                    }
                    y += 52.0;
                }
            }
            "notice" => {
                check(ui, &mut y, "Things you copy (I offer to clean links, format JSON…)", &mut setup.clip, "clip");
                check(ui, &mut y, "Programs you open", &mut setup.programs, "programs");
                check(ui, &mut y, "What you're working on (from window titles)", &mut setup.windows, "windows");
                check(ui, &mut y, "Ask how you're doing once a day", &mut setup.checkin, "checkin");
            }
            "startup" => {
                check(ui, &mut y, "Start Clipster when I log in", &mut setup.autostart, "autostart");
                check(ui, &mut y, "Tell me when a new version is out", &mut setup.updates, "updates");
                check(ui, &mut y, "Play sounds", &mut setup.sounds, "sounds");
            }
            "desk" => desk_sketch(&p, Rect::from_min_size(pos2(x0, y), vec2(x1 - x0, 64.0))),
            "done" => check(ui, &mut y, "Open my Desk now", &mut setup.open_desk, "desk"),
            _ => {}
        }
        // Try it: runs the example for real, so he answers in his balloon
        let mut tx = x0;
        for (i, (label, cmd)) in tries.iter().enumerate() {
            let w = p.layout_no_wrap(label.to_string(), win98::font(12.5), win98::TEXT).size().x + 24.0;
            if win98::button(ui, Rect::from_min_size(pos2(tx, side.max.y - 26.0), vec2(w, 24.0)), Id::unique(("welcome-try", id, i)), label, true).clicked() {
                out.push(cmd.to_string());
            }
            tx += w + 8.0;
        }
        // the bottom row
        win98::separator(&p, win.max.y - 46.0, win.min.x + 10.0, win.max.x - 10.0);
        let by = win.max.y - 36.0;
        p.text(pos2(win.min.x + 14.0, by + 12.0), Align2::LEFT_CENTER, format!("Step {} of {}", self.page - self.first + 1, PAGES.len() - self.first), win98::font(12.0), win98::GRAY_TEXT);
        let last = self.page == PAGES.len() - 1;
        let btn = |ui: &mut Ui, x: f32, label: &str, enabled: bool| win98::button(ui, Rect::from_min_size(pos2(x, by), vec2(86.0, 24.0)), Id::unique(("welcome-nav", label)), label, enabled).clicked();
        if btn(ui, win.max.x - 290.0, "< Back", self.page > self.first) {
            self.page -= 1;
        }
        let next = btn(ui, win.max.x - 198.0, if last { "Finish" } else { "Next >" }, true) || ui.input(|i| i.key_pressed(egui::Key::Enter)) && id != "name";
        if btn(ui, win.max.x - 100.0, "Cancel", true) || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            return Some(Done::Cancel(self.was_unhinged));
        }
        if next || (id == "name" && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
            if last {
                return Some(Done::Finish(setup.clone()));
            }
            self.page += 1;
        }
        None
    }
}

/// A little drawing of the Desk's tabs for its page.
fn desk_sketch(p: &Painter, r: Rect) {
    let w = Rect::from_min_size(r.min, vec2(r.width().min(400.0), r.height()));
    win98::panel(p, w, win98::FACE, true);
    let bar = Rect::from_min_size(w.min + vec2(3.0, 3.0), vec2(w.width() - 6.0, 14.0));
    win98::gradient(p, bar, win98::NAVY, win98::SKY);
    p.text(pos2(bar.min.x + 4.0, bar.center().y), Align2::LEFT_CENTER, "Clipster's Desk", win98::bold(10.0), Color32::WHITE);
    let mut x = w.min.x + 6.0;
    for (i, t) in crate::desk::TABS.iter().enumerate() {
        let tw = p.layout_no_wrap(t.to_string(), win98::font(10.0), win98::TEXT).size().x + 8.0;
        if x + tw > w.max.x - 6.0 {
            break;
        }
        let tab = Rect::from_min_size(pos2(x, bar.max.y + 6.0), vec2(tw, 15.0));
        win98::thin(p, tab, true);
        p.text(tab.center(), Align2::CENTER_CENTER, *t, if i == 0 { win98::bold(10.0) } else { win98::font(10.0) }, win98::TEXT);
        x += tw;
    }
    let page = Rect::from_min_max(pos2(w.min.x + 6.0, bar.max.y + 21.0), pos2(w.max.x - 6.0, w.max.y - 6.0));
    win98::bevel(p, page, true);
    for (i, frac) in [0.8, 0.5, 0.3].iter().enumerate() {
        let y = page.min.y + 5.0 + i as f32 * 7.0;
        if y + 6.0 > page.max.y {
            break; // a short sketch shows fewer bars, not bars over its edge
        }
        p.rect_filled(Rect::from_min_size(pos2(page.min.x + 6.0, y), vec2(page.width() * 0.4 * frac, 4.0)), 0.0, win98::NAVY);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_page_has_its_words() {
        for (id, ..) in super::PAGES {
            assert!(!crate::lines::all(&format!("welcome {id}")).is_empty(), "no text for page {id}");
            assert!(!crate::lines::all(&format!("welcome_say {id}")).is_empty(), "nothing for him to say on page {id}");
        }
        assert_eq!(super::PAGES[super::TOUR].0, "calling", "the tour starts with how to call me");
    }
}
