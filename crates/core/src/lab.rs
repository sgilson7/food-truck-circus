//! The Graph Lab's mode of the game: a function on a board over the bed, a
//! performer who plots it by bouncing (plot.rs plans; this file runs it),
//! and everything the lab page shows: the small graph, the function
//! machine's table, the planner's tree and its thinking, in words.

use crate::balance::{BED_Y, BED_Z};
use crate::expr::{self, ReadError};
use crate::fx::{int, ratio, ONE, QUARTER};
use crate::game::{fill, pct, Game, Mode};
use crate::geom::{v3, V3};
use crate::mesh::Mesh;
use crate::performer::{Controls, Event, Performer};
use crate::plot::{self, Board, Mark, Planner, BOARD_HALF, HIGH_H, LOW_H, MAX_POINTS, MIN_POINTS, TREE};
use crate::render::{self, Batch};
use crate::scene::Fittings;
use crate::tour::rgb;
use serde_json::{json, Value};

const Q: i64 = ONE as i64;
/// The lab opens on x² from −2 to 2 at nine points, at Woodcrest Farm.
const START_FORMULA: &str = "x^2";
const START_VENUE: &str = "woodcrest";
const TRAIL_MAX: usize = 900;

pub struct LabState {
    pub board: Board,
    pub planner: Planner,
    pub auto: bool,
    pub running: bool,
    /// The player's own marks, in "You try".
    pub free: Vec<Mark>,
    pub contacts: u32,
    pub trail: Vec<V3>,
    /// Slow motion: one tick for every this many.
    pub scale: i64,
    pub error: String,
    pub finished: bool,
    pub summary: Option<(i32, i64)>,
    /// How the marks are joined into the performer's version of f.
    pub smooth: bool,
    /// Which lab: 0 plotting, 1 popcorn under a flight, 2 juggling patterns.
    pub kind: u32,
    pub fly: Option<crate::labfly::FlyLab>,
    pub jug: Option<crate::labjuggle::JugLab>,
}

fn grade_of(off_m: i32) -> i32 {
    // three points a centimetre
    (100 - off_m.abs() * 300 / ONE).clamp(0, 100)
}

impl Game {
    pub fn lab_open(&mut self) {
        let board = Board::new(START_FORMULA, -2 * Q, 2 * Q, 9).expect("the lab's first formula reads");
        self.lab = Some(LabState {
            board,
            planner: Planner::new(),
            auto: true,
            running: false,
            free: Vec::new(),
            contacts: 0,
            trail: Vec::new(),
            scale: 1,
            error: String::new(),
            finished: false,
            summary: None,
            smooth: false,
            kind: 0,
            fly: None,
            jug: None,
        });
        let v = self.tour.venue_index(START_VENUE);
        self.set_stage(v, Fittings::default());
        self.act = crate::game::Act::None;
        self.mode = Mode::Lab;
        self.cam_yaw = 0;
        self.cam_pitch = 0;
        self.lab_reset();
        self.snap_camera();
    }

    pub fn time_scale(&self) -> i64 {
        match (&self.lab, self.mode) {
            (Some(l), Mode::Lab) => l.scale.max(1),
            _ => 1,
        }
    }

    /// Switches between the labs: 0 plotting, 1 popcorn, 2 juggling.
    pub fn lab_kind(&mut self, kind: u32) {
        let Some(l) = self.lab.as_mut() else { return };
        l.kind = kind.min(2);
        l.running = false;
        if l.kind == 1 && l.fly.is_none() {
            l.fly = Some(crate::labfly::FlyLab::new());
        }
        if l.kind == 2 && l.jug.is_none() {
            l.jug = Some(crate::labjuggle::JugLab::new());
        }
        self.lab_reset();
        self.snap_camera();
    }

    pub(crate) fn lab_camera(&self) -> (V3, i32, i32, i32) {
        let kind = self.lab.as_ref().map(|l| l.kind).unwrap_or(0);
        let (yaw, pitch) = (self.cam_yaw, self.cam_pitch);
        match kind {
            1 => (
                v3(ratio(12, 10), ratio(37, 10), BED_Z + ONE / 2),
                yaw,
                -crate::fx::deg(6) + pitch,
                int(16),
            ),
            2 => {
                let p = crate::game::juggle_spot();
                (
                    v3(p.x, ratio(21, 10), p.z),
                    yaw - crate::fx::deg(8),
                    crate::fx::deg(3) + pitch,
                    ratio(68, 10),
                )
            }
            _ => (
                v3(0, ratio(34, 10), BED_Z),
                yaw,
                -crate::fx::deg(4) + pitch,
                int(12),
            ),
        }
    }

    /// The performer back at the start of the current lab.
    pub fn lab_reset(&mut self) {
        match self.lab.as_ref().map(|l| l.kind) {
            Some(1) => {
                self.fly_reset();
                if let Some(l) = self.lab.as_mut() {
                    l.running = false;
                }
                return;
            }
            Some(2) => {
                self.jug_reset();
                return;
            }
            _ => {}
        }
        let Some(l) = self.lab.as_mut() else { return };
        let mut p = Performer::on_bed(&self.def, -QUARTER);
        p.lean_build = true;
        p.com.x = l.board.bed_x(l.board.x0);
        let frame = p.frame();
        p.body.place(&p.shape, &frame);
        p.body.prev = p.body.pos;
        self.perf = p;
        l.planner = Planner::new();
        l.free.clear();
        l.trail.clear();
        l.contacts = 0;
        l.running = false;
        l.finished = false;
        l.summary = None;
    }

    fn read_error(&self, e: &ReadError) -> String {
        fill(
            &self.copy_str(&["lab", "expr", e.key]),
            &[("at", e.at.to_string()), ("word", e.word.clone())],
        )
    }

    /// Reads a formula; returns what went wrong, or nothing.
    pub fn lab_formula(&mut self, src: &str) -> String {
        let Some(l) = self.lab.as_mut() else {
            return String::new();
        };
        let n = l.board.n();
        let r = l.board.set_formula(src, n);
        let msg = match r {
            Ok(()) => String::new(),
            Err(e) => self.read_error(&e),
        };
        if let Some(l) = self.lab.as_mut() {
            l.error = msg.clone();
        }
        self.lab_reset();
        msg
    }

    /// Sets the x range from two typed numbers (`-2`, `pi`, `3/2`).
    pub fn lab_domain(&mut self, from: &str, to: &str) -> String {
        let read = |s: &str| expr::parse(s).map(|e| e.eval(0));
        let msg = match (read(from), read(to)) {
            (Ok(Ok(a)), Ok(Ok(b))) if b > a => {
                if let Some(l) = self.lab.as_mut() {
                    l.board.set_domain(a, b);
                }
                String::new()
            }
            (Ok(Ok(_)), Ok(Ok(_))) => self.copy_str(&["lab", "domain_order"]),
            (Err(e), _) | (_, Err(e)) => self.read_error(&e),
            _ => self.copy_str(&["lab", "domain_order"]),
        };
        if let Some(l) = self.lab.as_mut() {
            l.error = msg.clone();
        }
        self.lab_reset();
        msg
    }

    pub fn lab_points(&mut self, n: usize) {
        if let Some(l) = self.lab.as_mut() {
            l.board.resample(n);
        }
        self.lab_reset();
    }

    /// A point of the small graph dragged to `y_px` of `h_px` from its top.
    pub fn lab_drag(&mut self, i: usize, y_px: i32, h_px: i32) {
        if let Some(l) = self.lab.as_mut() {
            let at = y_px as i64 * 100 * Q / h_px.max(1) as i64;
            let y = l.board.from_gy(at);
            l.board.drag(i, y);
        }
        self.lab_reset();
    }

    pub fn lab_run(&mut self, auto: bool) {
        self.lab_reset();
        if let Some(l) = self.lab.as_mut() {
            l.auto = auto;
            l.running = true;
        }
    }

    /// Joins the marks with straight lines, or with a smooth curve.
    pub fn lab_join(&mut self, smooth: bool) {
        if let Some(l) = self.lab.as_mut() {
            l.smooth = smooth;
        }
    }

    pub fn lab_speed(&mut self, scale: u32) {
        if let Some(l) = self.lab.as_mut() {
            l.scale = (scale as i64).clamp(1, 8);
        }
    }

    pub fn lab_venue(&mut self, i: usize) {
        if i < self.tour.venues.len() {
            self.set_stage(i, Fittings::default());
        }
    }

    pub(crate) fn lab_step(&mut self, bits: u32) {
        let kind = self.lab.as_ref().map(|l| l.kind).unwrap_or(0);
        if kind != 0 {
            if kind == 1 {
                self.fly_step();
            } else {
                self.jug_step();
            }
            let lowest = self.perf.body.lowest(&self.def);
            let centre = self.perf.body.centre(&self.def);
            let press = if lowest < BED_Y + ONE / 20 && crate::stage::over_bed(centre.x, centre.z) {
                Some((centre.x, centre.z, (BED_Y - lowest).max(0)))
            } else {
                None
            };
            self.cloth.step(press);
            self.crowd.tick();
            return;
        }
        let _ = bits;
        let on = |n: &str| {
            bits & crate::game::ACTIONS
                .iter()
                .find(|(k, _)| *k == n)
                .map(|(_, b)| *b)
                .unwrap_or(0)
                != 0
        };
        let Some(l) = self.lab.as_mut() else { return };
        let c = if l.running && l.auto && !l.finished {
            l.planner.keys(&self.def, &l.board, &self.perf, l.contacts)
        } else if l.running && !l.auto {
            let one = |a: bool, b: bool| a as i32 - b as i32;
            Controls {
                drive: on("drive"),
                kill: on("kill"),
                travel: one(on("turn_r") || on("up"), on("turn_l") || on("down")),
                ..Controls::default()
            }
        } else {
            Controls::default()
        };
        let events = self.perf.step(&self.def, &c);
        let mut said: Vec<(i32, String)> = Vec::new();
        let mut tricks: Vec<(i32, String)> = Vec::new();
        let l = self.lab.as_mut().expect("lab");
        for e in events {
            match e {
                Event::Landed(land) => {
                    l.contacts += 1;
                    // the crowd names the tricks done on the way
                    if l.running {
                        if let Some(n) = self.book.name(&land).filter(|n| n.value > 0) {
                            tricks.push((self.book.grade(&land), n.name));
                        }
                    }
                }
                Event::Peak(_) if l.running => {
                    let h = plot::pen_height(&self.def, &self.perf);
                    let x = self.perf.com.x;
                    let mark = if l.auto {
                        l.planner.peak(&l.board, x, h)
                    } else {
                        let m = Mark {
                            i: None,
                            x: l.board.unit_x(x),
                            y: l.board.unit_y(h),
                        };
                        l.free.push(m.clone());
                        Some(m)
                    };
                    if let Some(m) = mark {
                        let col = nearest(&l.board, m.x);
                        let want = l.board.ys[col].as_ref().ok().copied();
                        let off = want
                            .map(|w| l.board.height(m.y) - l.board.height(w))
                            .unwrap_or(int(1));
                        said.push((
                            grade_of(off),
                            format!("({}, {})", expr::show(m.x), expr::show(m.y)),
                        ));
                    }
                }
                _ => {}
            }
        }
        if l.running && self.tick % 2 == 0 {
            let low = self.perf.body.lowest(&self.def);
            l.trail.push(v3(self.perf.com.x, low, BED_Z));
            if l.trail.len() > TRAIL_MAX {
                l.trail.remove(0);
            }
        }
        let mut finished_now = None;
        if l.running && l.auto && l.planner.done && !l.finished {
            l.finished = true;
            let offs: Vec<i32> = l
                .planner
                .marks
                .iter()
                .filter_map(|m| {
                    let i = m.i?;
                    let w = l.board.ys[i].as_ref().ok()?;
                    Some(l.board.height(m.y) - l.board.height(*w))
                })
                .collect();
            let n = offs.len().max(1) as i32;
            let mean_m = offs.iter().map(|o| o.abs()).sum::<i32>() / n;
            let mean_units = l.planner.marks.iter().filter_map(|m| {
                let w = l.board.ys[m.i?].as_ref().ok()?;
                Some((m.y - w).abs())
            });
            let total: i64 = mean_units.sum();
            let grade = grade_of(mean_m);
            l.summary = Some((mean_m, total / n as i64));
            finished_now = Some(grade);
        }
        for (g, text) in said {
            let w = self.book.word(g, &mut self.rng);
            self.say(&w, text, 2);
            self.crowd.cheer(ONE * g / 300);
            self.sound("boing", 40);
        }
        for (g, name) in tricks {
            let w = self.book.word(g, &mut self.rng);
            self.say(&w, name, 3);
        }
        if let Some(g) = finished_now {
            let w = self.book.plain_word(g);
            let line = self.copy_str(&["lab", "done_call"]);
            self.say(&w, line, 8);
            self.crowd.cheer(ONE);
            self.sound("applause", 100);
        }
        let lowest = self.perf.body.lowest(&self.def);
        let centre = self.perf.body.centre(&self.def);
        let press = if lowest < BED_Y + ONE / 20 && crate::stage::over_bed(centre.x, centre.z) {
            Some((centre.x, centre.z, (BED_Y - lowest).max(0)))
        } else {
            None
        };
        self.cloth.step(press);
        self.crowd.tick();
    }

    pub(crate) fn lab_draw(&self, b: &mut Batch) {
        let Some(l) = &self.lab else { return };
        if l.kind != 0 {
            for (k, p) in l.trail.iter().enumerate() {
                if k % 2 == 0 {
                    b.push(
                        Mesh::Sphere,
                        &render::boxed(*p, v3(ratio(6, 100), ratio(6, 100), ratio(6, 100))),
                        rgb("#ffb4a2"),
                        ONE / 2,
                    );
                }
            }
            if l.kind == 1 {
                self.fly_draw(b);
            } else {
                self.jug_draw(b);
            }
            return;
        }
        let board = &l.board;
        let z = BED_Z - ratio(3, 100);
        let at = |x: i64, y: i64| v3(board.bed_x(x), BED_Y + board.height(y), z);
        let faint = rgb("#f1faee");
        let left = -BOARD_HALF - ratio(2, 10);
        let right = BOARD_HALF + ratio(2, 10);
        // whole-number heights, the zero line brightest
        let span = (board.hi - board.lo) / Q;
        let step = if span > 12 { (span / 6).max(1) } else { 1 };
        let mut y = board.lo;
        while y <= board.hi {
            let h = BED_Y + board.height(y);
            let (w, glow) = if y == 0 {
                (ratio(4, 100), ONE)
            } else {
                (ratio(15, 1000), ONE / 3)
            };
            b.rod(Mesh::Cube, v3(left, h, z), v3(right, h, z), w, faint, glow);
            y += step * Q;
        }
        // a column for each x the machine is asked about
        for i in 0..board.n() {
            let x = board.bed_x(board.x(i));
            b.rod(
                Mesh::Cube,
                v3(x, BED_Y + LOW_H - ratio(2, 10), z),
                v3(x, BED_Y + HIGH_H + ratio(2, 10), z),
                ratio(1, 100),
                faint,
                ONE / 4,
            );
            if let Ok(y) = board.ys[i] {
                b.push(
                    Mesh::Octa,
                    &render::boxed(
                        at(board.x(i), y),
                        v3(ratio(14, 100), ratio(14, 100), ratio(14, 100)),
                    ),
                    rgb("#2a9d8f"),
                    ONE / 2,
                );
            }
        }
        // the function itself, as a bright line
        let curve = curve_points(board, 96);
        for seg in curve {
            for w in seg.windows(2) {
                b.rod(
                    Mesh::Cube,
                    at(w[0].0, w[0].1),
                    at(w[1].0, w[1].1),
                    ratio(4, 100),
                    rgb("#2ec4b6"),
                    ONE * 2 / 3,
                );
            }
        }
        // the feet's path: every flight a parabola of its own
        for (k, p) in l.trail.iter().enumerate() {
            if k % 2 == 0 {
                b.push(
                    Mesh::Sphere,
                    &render::boxed(*p, v3(ratio(5, 100), ratio(5, 100), ratio(5, 100))),
                    rgb("#ffb4a2"),
                    ONE / 3,
                );
            }
        }
        // the performer's version of f: the marks joined up
        let marks = if l.auto { &l.planner.marks } else { &l.free };
        let joined = interpolate(marks, l.smooth, 24);
        for w in joined.windows(2) {
            b.rod(
                Mesh::Cube,
                at(w[0].0, w[0].1),
                at(w[1].0, w[1].1),
                ratio(3, 100),
                rgb("#f77f00"),
                ONE * 2 / 3,
            );
        }
        for m in marks {
            b.push(
                Mesh::Sphere,
                &render::boxed(at(m.x, m.y), v3(ratio(2, 10), ratio(2, 10), ratio(2, 10))),
                rgb("#ffd166"),
                ONE,
            );
        }
    }

    /// Everything the lab page shows, as JSON.
    pub fn lab_json(&self) -> String {
        let Some(l) = &self.lab else { return "null".into() };
        let board = &l.board;
        let lab = |k: &str| self.copy_str(&["lab", k]);
        let gx = |x: i64| pct(board.gx(x));
        let gy = |y: i64| pct(board.gy(y));
        let curve: Vec<String> = curve_points(board, 160)
            .iter()
            .map(|seg| {
                seg.iter()
                    .map(|(x, y)| format!("{},{}", gx(*x), gy(*y)))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        let samples: Vec<Value> = (0..board.n())
            .filter_map(|i| {
                let y = board.ys[i].as_ref().ok()?;
                Some(json!({"i": i, "x": gx(board.x(i)), "y": gy(*y), "label": format!("f({}) = {}", expr::show(board.x(i)), expr::show(*y))}))
            })
            .collect();
        let marks: Vec<Value> = if l.auto { &l.planner.marks } else { &l.free }
            .iter()
            .filter(|m| m.x >= board.x0 - Q && m.x <= board.x1 + Q)
            .map(|m| json!({"x": gx(m.x.clamp(board.x0, board.x1)), "y": gy(m.y.clamp(board.lo, board.hi))}))
            .collect();
        let pen = {
            let x = board.unit_x(self.perf.com.x).clamp(board.x0, board.x1);
            let y = board
                .unit_y(self.perf.body.lowest(&self.def) - BED_Y)
                .clamp(board.lo, board.hi);
            json!({"x": gx(x), "y": gy(y)})
        };
        let ticks = |lo: i64, hi: i64| -> Vec<i64> {
            let span = ((hi - lo) / Q).max(1);
            let step = if span > 12 { (span / 6).max(1) } else { 1 } * Q;
            let mut v = Vec::new();
            let mut t = lo.div_euclid(step) * step;
            while t <= hi {
                if t >= lo {
                    v.push(t);
                }
                t += step;
            }
            v
        };
        let tx: Vec<Value> = ticks(board.x0, board.x1)
            .iter()
            .map(|t| json!({"at": gx(*t), "label": expr::show(*t)}))
            .collect();
        let ty: Vec<Value> = ticks(board.lo, board.hi)
            .iter()
            .map(|t| json!({"at": gy(*t), "label": expr::show(*t)}))
            .collect();
        let y_axis = (board.x0 <= 0 && board.x1 >= 0).then(|| gx(0));

        // the function machine's table, with the change from row to row
        let marks_for = |i: usize| -> Option<&Mark> {
            let ms = if l.auto { &l.planner.marks } else { &l.free };
            if l.auto {
                ms.iter().find(|m| m.i == Some(i))
            } else {
                ms.iter()
                    .filter(|m| nearest(board, m.x) == i)
                    .min_by_key(|m| (m.x - board.x(i)).abs())
            }
        };
        let mut rows = Vec::new();
        for i in 0..board.n() {
            let y = board.ys[i].as_ref().ok().copied();
            let prev = if i > 0 {
                board.ys[i - 1].as_ref().ok().copied()
            } else {
                None
            };
            let prev2 = if i > 1 {
                board.ys[i - 2].as_ref().ok().copied()
            } else {
                None
            };
            let change = match (y, prev) {
                (Some(a), Some(b)) => expr::show(a - b),
                _ => String::new(),
            };
            let change2 = match (y, prev, prev2) {
                (Some(a), Some(b), Some(c)) => expr::show((a - b) - (b - c)),
                _ => String::new(),
            };
            let m = marks_for(i);
            rows.push(json!({
                "x": expr::show(board.x(i)),
                "y": match &board.ys[i] { Ok(v) => expr::show(*v), Err(why) => self.copy_str(&["lab", "undefined", why]) },
                "marked": m.map(|m| expr::show(m.y)).unwrap_or_default(),
                "off": match (m, y) { (Some(m), Some(y)) => expr::show((m.y - y).abs()), _ => String::new() },
                "change": change,
                "change2": change2,
                "next": l.auto && l.running && i == l.planner.next,
            }));
        }
        let tree: Vec<Value> = TREE
            .iter()
            .map(|(node, depth, kind)| {
                let id = serde_json::to_value(node)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default();
                json!({
                    "depth": depth,
                    "kind": kind,
                    "label": self.copy_str(&["lab", "tree", &id]),
                    "lit": l.running && l.auto && l.planner.lit.contains(node),
                })
            })
            .collect();
        let log: Vec<String> = l
            .planner
            .log
            .iter()
            .rev()
            .take(14)
            .rev()
            .map(|line| {
                fill(
                    &self.copy_str(&["lab", "log", line.key]),
                    &line.vals.iter().map(|(k, v)| (*k, v.clone())).collect::<Vec<_>>(),
                )
            })
            .collect();
        let marks_now = if l.auto { &l.planner.marks } else { &l.free };
        let joined = interpolate(marks_now, l.smooth, 24);
        let interp = if joined.len() >= 2 {
            let line: Vec<String> = joined
                .iter()
                .map(|(x, y)| format!("{},{}", gx(*x), gy((*y).clamp(board.lo - Q, board.hi + Q))))
                .collect();
            let gap = gap_of(board, &joined);
            json!({
                "curve": line.join(" "),
                "gap": gap.polygon.iter().map(|(x, y)| format!("{},{}", gx(*x), gy((*y).clamp(board.lo - Q, board.hi + Q)))).collect::<Vec<_>>().join(" "),
                "text": fill(
                    &lab(if l.smooth { "error_smooth" } else { "error_lines" }),
                    &[("max", expr::show(gap.max)), ("at", expr::show(gap.at)), ("mean", expr::show(gap.mean))],
                ),
            })
        } else {
            Value::Null
        };
        let summary = l.summary.map(|(cm_off, units)| {
            fill(
                &lab("summary"),
                &[
                    ("cm", (cm_off * 100 / ONE).to_string()),
                    ("units", expr::show(units)),
                    ("n", l.planner.marks.len().to_string()),
                ],
            )
        });
        json!({
            "kind": l.kind,
            "fly": self.fly_json(),
            "jug": self.jug_json(),
            "formula": board.formula,
            "drawn": board.drawn,
            "error": l.error,
            "from": expr::show(board.x0),
            "to": expr::show(board.x1),
            "n": board.n(),
            "counts": (MIN_POINTS..=MAX_POINTS).collect::<Vec<_>>(),
            "running": l.running,
            "auto": l.auto,
            "finished": l.finished,
            "summary": summary,
            "scale": l.scale,
            "venue": self.venue,
            "venues": self.tour.venues.iter().map(|v| format!("{} · {}", v.name, v.city)).collect::<Vec<_>>(),
            "interp": interp,
            "smooth": l.smooth,
            "graph": {
                "curve": curve,
                "samples": samples,
                "marks": marks,
                "pen": pen,
                "x_axis": gy(0),
                "y_axis": y_axis,
                "ticks_x": tx,
                "ticks_y": ty,
            },
            "rows": rows,
            "tree": tree,
            "log": log,
            "version": l.planner.log.len() + l.planner.marks.len() * 1000 + l.free.len() * 100_000,
        })
        .to_string()
    }
}

/// The marks joined up, left to right: straight lines from mark to mark, or
/// a smooth curve through them (Catmull-Rom: each piece bends toward the
/// marks either side of it), `steps` points a piece.
pub fn interpolate(marks: &[Mark], smooth: bool, steps: i64) -> Vec<(i64, i64)> {
    let mut pts: Vec<(i64, i64)> = marks.iter().map(|m| (m.x, m.y)).collect();
    pts.sort_by_key(|p| p.0);
    pts.dedup_by_key(|p| p.0);
    if pts.len() < 2 {
        return pts;
    }
    if !smooth {
        return pts;
    }
    let n = pts.len();
    let mut out = Vec::new();
    for i in 0..n - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let (p1, p2) = (pts[i], pts[i + 1]);
        let p3 = pts[(i + 2).min(n - 1)];
        for k in 0..steps {
            let t = k * Q / steps;
            let t2 = t * t / Q;
            let t3 = t2 * t / Q;
            let cr = |a: i64, b: i64, c: i64, d: i64| -> i64 {
                (2 * b
                    + (c - a) * t / Q
                    + (2 * a - 5 * b + 4 * c - d) * t2 / Q
                    + (3 * b - a - 3 * c + d) * t3 / Q)
                    / 2
            };
            out.push((cr(p0.0, p1.0, p2.0, p3.0), cr(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out.push(pts[n - 1]);
    out
}

/// How far the joined-up marks are from f, between the first mark and the
/// last: the largest gap and where, the mean gap, and the region between
/// the two curves as a polygon (f along the top, the join back along the
/// bottom, whichever is which).
pub struct Gap {
    pub max: i64,
    pub at: i64,
    pub mean: i64,
    pub polygon: Vec<(i64, i64)>,
}

pub fn gap_of(board: &Board, joined: &[(i64, i64)]) -> Gap {
    let truth = |x: i64| -> Option<i64> {
        if board.drawn {
            let pts: Vec<(i64, i64)> = (0..board.n())
                .filter_map(|i| board.ys[i].as_ref().ok().map(|y| (board.x(i), *y)))
                .collect();
            Some(plot::lerp_table(&pts, x))
        } else {
            board.eval(x).ok()
        }
    };
    let (x0, x1) = (joined[0].0, joined[joined.len() - 1].0);
    let steps = 120;
    let mut gap = Gap {
        max: 0,
        at: x0,
        mean: 0,
        polygon: Vec::new(),
    };
    let mut back = Vec::new();
    let mut count = 0i64;
    for k in 0..=steps {
        let x = x0 + (x1 - x0) * k / steps;
        let mine = plot::lerp_table(joined, x);
        if let Some(f) = truth(x) {
            let d = (mine - f).abs();
            gap.mean += d;
            count += 1;
            if d > gap.max {
                gap.max = d;
                gap.at = x;
            }
            gap.polygon.push((x, f));
            back.push((x, mine));
        }
    }
    gap.mean /= count.max(1);
    back.reverse();
    gap.polygon.extend(back);
    gap
}

/// The column nearest a graph x.
fn nearest(board: &Board, x: i64) -> usize {
    (0..board.n())
        .min_by_key(|&i| (board.x(i) - x).abs())
        .unwrap_or(0)
}

/// The function's line through its range, in pieces that break where it
/// has no value or leaves the board: a formula sampled finely, a drawn
/// function through its points.
pub fn curve_points(board: &Board, steps: i64) -> Vec<Vec<(i64, i64)>> {
    let mut segs: Vec<Vec<(i64, i64)>> = Vec::new();
    let mut cur: Vec<(i64, i64)> = Vec::new();
    let inside = |y: i64| y >= board.lo - Q / 4 && y <= board.hi + Q / 4;
    let push = |p: Option<(i64, i64)>, segs: &mut Vec<Vec<(i64, i64)>>, cur: &mut Vec<(i64, i64)>| match p {
        Some(p) if inside(p.1) => cur.push(p),
        _ => {
            if cur.len() > 1 {
                segs.push(std::mem::take(cur));
            } else {
                cur.clear();
            }
        }
    };
    if board.drawn {
        for i in 0..board.n() {
            let p = board.ys[i].as_ref().ok().map(|y| (board.x(i), *y));
            push(p, &mut segs, &mut cur);
        }
    } else {
        for k in 0..=steps {
            let x = board.x0 + (board.x1 - board.x0) * k / steps;
            let p = board.eval(x).ok().map(|y| (x, y));
            push(p, &mut segs, &mut cur);
        }
    }
    push(None, &mut segs, &mut cur);
    let _ = plot::SNAP;
    segs
}

#[cfg(test)]
mod tests {
    use crate::game::tests::game_with;
    use crate::game::Mode;
    use serde_json::Value;

    #[test]
    fn the_lab_opens_on_x_squared_and_plots_it_when_asked() {
        let mut g = game_with("");
        g.resize(1400, 860, 100);
        g.lab_open();
        assert_eq!(g.mode(), Mode::Lab);
        let j: Value = serde_json::from_str(&g.lab_json()).unwrap();
        assert_eq!(j["formula"], "x^2");
        assert_eq!(j["rows"].as_array().unwrap().len(), 9);
        // x² at -1.5 is 2.25; its change from 4 is -1.75; the change of the
        // change is the same 0.5 on every row, the mark of a parabola
        assert_eq!(j["rows"][1]["y"], "2.25");
        assert_eq!(j["rows"][1]["change"], "-1.75");
        assert_eq!(j["rows"][2]["change2"], "0.5");
        assert_eq!(j["rows"][7]["change2"], "0.5");
        g.lab_run(true);
        let mut t = 0;
        for _ in 0..3600 {
            t += 16_667;
            g.advance(t, 0, 0, 0);
        }
        let j: Value = serde_json::from_str(&g.lab_json()).unwrap();
        assert_eq!(j["finished"], true);
        assert!(j["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| !r["marked"].as_str().unwrap().is_empty()));
        assert!(j["summary"].as_str().unwrap().len() > 10);
    }

    #[test]
    fn a_formula_the_lab_cannot_read_says_where() {
        let mut g = game_with("");
        g.lab_open();
        let msg = g.lab_formula("2x+");
        assert!(!msg.is_empty());
        assert_eq!(g.lab_formula("2x+1"), "");
        assert_eq!(g.lab_domain("3", "1"), g.copy_str(&["lab", "domain_order"]));
        assert_eq!(g.lab_domain("-pi", "pi"), "");
    }

    #[test]
    fn straight_lines_through_three_points_of_x_squared_miss_by_a_known_amount() {
        use super::{gap_of, interpolate};
        use crate::plot::{Board, Mark};
        let q = super::Q;
        // marks exactly on x² at -2, 0 and 2: straight lines from (−2, 4) to
        // (0, 0) are y = −2x, and x² − (−2x) is largest at x = −1, where it
        // is 1; the mean gap over [−2, 2] is 2/3 of that
        let board = Board::new("x^2", -2 * q, 2 * q, 3).unwrap();
        let marks: Vec<Mark> = [(-2, 4), (0, 0), (2, 4)]
            .iter()
            .map(|&(x, y)| Mark {
                i: None,
                x: x * q,
                y: y * q,
            })
            .collect();
        let g = gap_of(&board, &interpolate(&marks, false, 24));
        assert!((g.max - q).abs() < q / 100, "{}", g.max);
        assert_eq!(g.at.abs(), q);
        assert!((g.mean - q * 2 / 3).abs() < q / 50, "{}", g.mean);
        // a smooth curve through the same marks bends toward x² and misses less
        let s = gap_of(&board, &interpolate(&marks, true, 24));
        assert!(s.mean < g.mean, "{} vs {}", s.mean, g.mean);
    }

    #[test]
    fn dragging_a_point_draws_the_function_by_hand() {
        let mut g = game_with("");
        g.lab_open();
        // the top of the small graph is 8 % down; drag point 4 there
        g.lab_drag(4, 8, 100);
        let j: Value = serde_json::from_str(&g.lab_json()).unwrap();
        assert_eq!(j["drawn"], true);
        assert_eq!(j["rows"][4]["y"], "4");
    }
}
