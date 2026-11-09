//! The Week tab: the last seven days and the week in review.

#![allow(unused_imports)]
use super::*;

impl Desk {
/// Last week's time per day, stacked as work, fun and the rest, with focus minutes under each day; on the
/// right, the week in review.
pub(super) fn week(&mut self, ui: &mut Ui, c_all: Rect, view: &View, look: Look, out: &mut Vec<String>) {
    let p = ui.painter().clone();
    let side = Rect::from_min_max(pos2(c_all.max.x - 220.0, c_all.min.y), c_all.max);
    let c = Rect::from_min_max(c_all.min, pos2(side.min.x - 12.0, c_all.max.y));
    look.group(&p, side, "This week");
    let w = crate::plan::week(view.kind_time, view.focus_log, &self.files.todos, view.checkins, chrono::Local::now().date_naive());
    let mut y = side.min.y + 32.0;
    for (i, line) in crate::plan::week_lines(&w).into_iter().enumerate() {
        let g = p.layout(line, if i == 0 { look.bold(12.0) } else { look.font(12.0) }, look.text(), side.width() - 24.0);
        let h = g.size().y;
        p.galley(pos2(side.min.x + 12.0, y), g, look.text());
        y += h + 8.0;
    }
    if look.button(ui, Rect::from_min_size(pos2(side.min.x + 12.0, side.max.y - look.ctl() - 12.0), vec2(side.width() - 24.0, look.ctl())), Id::unique("desk-review-copy"), "Copy as text", true).clicked() {
        out.push("review copy".into());
    }
    let chart = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - 40.0));
    look.group(&p, chart, "The last seven days");
    let groups = look.groups();
    let days: Vec<(String, [u64; 3], u64)> = (0..7)
        .rev()
        .map(|d| {
            let date = chrono::Local::now().date_naive() - chrono::Days::new(d as u64); // whole days, also on a daylight-saving night
            let key = date.format("%Y-%m-%d").to_string();
            let mut g = [0u64; 3];
            for (kind, secs) in view.kind_time.get(&key).into_iter().flatten() {
                g[brain::kind_group(kind)] += secs;
            }
            let focus: u64 = view.focus_log.iter().filter(|s| s.0 == key).map(|s| s.2 / 60).sum();
            (date.format("%a").to_string(), g, focus)
        })
        .collect();
    let max = days.iter().map(|d| d.1.iter().sum::<u64>()).max().unwrap_or(0).max(3600) as f32;
    let base = chart.max.y - 48.0;
    let tall = base - chart.min.y - 68.0; // leaves room for the total above the tallest column
    let slot = (c.width() - 4.0) / 7.0;
    p.hline(chart.min.x + 20.0..=chart.max.x - 20.0, base, Stroke::new(1.0, look.rule()));
    for (i, (name, g, focus)) in days.iter().enumerate() {
        let x = c.min.x + 8.0 + slot * (i as f32 + 0.5);
        let mut top = base;
        for (j, secs) in g.iter().enumerate() {
            let h = (*secs as f32 / max) * tall;
            if h > 0.5 {
                look.column(&p, Rect::from_min_max(pos2(x - 15.0, top - h), pos2(x + 15.0, top)), groups[j].1);
                top -= h;
            }
        }
        let total: u64 = g.iter().sum();
        if total > 0 {
            p.text(pos2(x, top - 4.0), Align2::CENTER_BOTTOM, brain::fmt_dur(total), look.font(11.0), look.dim());
        }
        p.text(pos2(x, base + 16.0), Align2::CENTER_CENTER, name, look.font(12.0), look.text());
        if *focus > 0 {
            focus_mark(&p, pos2(x - 15.0, base + 34.5), look);
            p.text(pos2(x - 7.0, base + 34.5), Align2::LEFT_CENTER, format!("{focus}m"), look.font(11.5), look.dim());
        }
    }
    // the legend
    let y = c.max.y - 17.0;
    let mut x = c.min.x + 12.0;
    for (name, color) in groups {
        look.column(&p, Rect::from_center_size(pos2(x + 5.0, y), vec2(10.0, 10.0)), color);
        x += 16.0 + p.text(pos2(x + 16.0, y), Align2::LEFT_CENTER, name, look.font(11.5), look.text()).width() + 20.0;
    }
    let label = p.text(pos2(c.max.x - 12.0, y), Align2::RIGHT_CENTER, "focus", look.font(11.5), look.dim());
    focus_mark(&p, pos2(label.min.x - 10.0, y), look);
}
}
