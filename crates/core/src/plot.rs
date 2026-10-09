//! The Graph Lab: a function, sampled into a table, plotted by bouncing.
//!
//! The bed runs along the graph's x axis; the top of each bounce marks a
//! point, the feet's height standing for f(x). The planner is a behaviour
//! tree whose leaves press only keys a player could press: Space for so
//! many beats to push the bed (C to stop it), and an arrow for so many beats
//! to lean along it. It chooses those beats by imagining the bounce in the
//! core's own physics on a copy of the performer, and interpolating between
//! two imagined bounces that bracket the target (regula falsi).

use crate::balance::GRAVITY;
use crate::body::BodyDef;
use crate::expr::{self, Expr, ReadError, Undefined};
use crate::fx::{int, ratio, ONE};
use crate::performer::{Controls, Event, Performer, Phase, Posture};
use serde::Serialize;

/// The graph's x range covers this much of the bed either side of centre.
pub const BOARD_HALF: i32 = ratio(18, 10);
/// The lowest and highest marks, feet above the bed, metres.
pub const LOW_H: i32 = ratio(5, 10);
pub const HIGH_H: i32 = int(5);
pub const MIN_POINTS: usize = 2;
pub const MAX_POINTS: usize = 17;
/// The beats the planner may push the bed (Space) or stop it (C), and lean.
pub const PUSH_MAX: i32 = 14;
pub const STOP_MAX: i32 = 8;
pub const LEAN_MAX: i32 = 18;
/// A mark this close to its target height, metres, is good enough; a
/// bounce that cannot get within `REACH` is a set-up bounce instead.
pub const TOL_H: i32 = ratio(5, 100);
pub const TOL_X: i32 = ratio(6, 100);
pub const REACH: i32 = ratio(12, 100);
/// Snap for a dragged point, in the graph's units.
pub const SNAP: i64 = (ONE / 4) as i64;

const Q: i64 = ONE as i64;

/// The function as a table: x in, f(x) out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub formula: String,
    pub drawn: bool,
    pub x0: i64,
    pub x1: i64,
    pub ys: Vec<Result<i64, Undefined>>,
    pub lo: i64,
    pub hi: i64,
    expr: Option<Expr>,
}

fn floor_unit(v: i64) -> i64 {
    v.div_euclid(Q) * Q
}

fn ceil_unit(v: i64) -> i64 {
    -floor_unit(-v)
}

impl Board {
    pub fn new(formula: &str, x0: i64, x1: i64, n: usize) -> Result<Board, ReadError> {
        let mut b = Board {
            formula: String::new(),
            drawn: false,
            x0,
            x1,
            ys: Vec::new(),
            lo: 0,
            hi: Q,
            expr: None,
        };
        b.set_formula(formula, n)?;
        Ok(b)
    }

    pub fn n(&self) -> usize {
        self.ys.len()
    }

    pub fn x(&self, i: usize) -> i64 {
        let n = self.n().max(2) as i64;
        self.x0 + (self.x1 - self.x0) * i as i64 / (n - 1)
    }

    pub fn eval(&self, x: i64) -> Result<i64, Undefined> {
        match &self.expr {
            Some(e) => e.eval(x),
            None => Err("drawn"),
        }
    }

    pub fn set_formula(&mut self, src: &str, n: usize) -> Result<(), ReadError> {
        let e = expr::parse(src)?;
        self.expr = Some(e);
        self.formula = src.trim().to_string();
        self.drawn = false;
        self.resample(n);
        Ok(())
    }

    /// Samples the formula at `n` points, or, for a drawn function, reads
    /// the new points off the straight lines between the old ones.
    pub fn resample(&mut self, n: usize) {
        let n = n.clamp(MIN_POINTS, MAX_POINTS);
        if self.drawn || self.expr.is_none() {
            let old: Vec<(i64, i64)> = (0..self.n())
                .filter_map(|i| self.ys[i].ok().map(|y| (self.x(i), y)))
                .collect();
            let mut ys = Vec::new();
            for i in 0..n {
                let x = self.x0 + (self.x1 - self.x0) * i as i64 / (n as i64 - 1);
                ys.push(Ok(lerp_table(&old, x)));
            }
            self.ys = ys;
            return;
        }
        self.ys = (0..n).map(|_| Ok(0)).collect();
        for i in 0..n {
            let x = self.x(i);
            self.ys[i] = self.eval(x);
        }
        self.fit_range();
    }

    pub fn set_domain(&mut self, x0: i64, x1: i64) {
        if x1 > x0 && x1 - x0 <= 200 * Q {
            self.x0 = x0;
            self.x1 = x1;
            let n = self.n();
            self.resample(n);
        }
    }

    /// The y range drawn: whole units, always including zero.
    pub fn fit_range(&mut self) {
        let ys: Vec<i64> = self.ys.iter().filter_map(|y| y.as_ref().ok().copied()).collect();
        let lo = ys.iter().copied().min().unwrap_or(0).min(0);
        let hi = ys.iter().copied().max().unwrap_or(Q).max(0);
        self.lo = floor_unit(lo);
        self.hi = ceil_unit(hi).max(self.lo + Q);
    }

    /// Moves one point by hand; the function becomes a drawn one.
    pub fn drag(&mut self, i: usize, y: i64) {
        if i >= self.n() {
            return;
        }
        if !self.drawn {
            self.drawn = true;
            // a drawn function keeps room above and below to drag into
            self.lo = self.lo.min(-Q);
            self.hi = self.hi.max(4 * Q);
        }
        let snapped = (y + SNAP / 2).div_euclid(SNAP) * SNAP;
        self.ys[i] = Ok(snapped.clamp(self.lo, self.hi));
    }

    // ---- where things go ------------------------------------------------

    /// Metres along the bed for an x.
    pub fn bed_x(&self, x: i64) -> i32 {
        (-BOARD_HALF as i64 + (x - self.x0) * 2 * BOARD_HALF as i64 / (self.x1 - self.x0)) as i32
    }

    /// Metres of the feet above the bed for a y.
    pub fn height(&self, y: i64) -> i32 {
        (LOW_H as i64 + (y - self.lo) * (HIGH_H - LOW_H) as i64 / (self.hi - self.lo)) as i32
    }

    pub fn unit_x(&self, bed_x: i32) -> i64 {
        self.x0 + (bed_x as i64 + BOARD_HALF as i64) * (self.x1 - self.x0) / (2 * BOARD_HALF as i64)
    }

    pub fn unit_y(&self, h: i32) -> i64 {
        self.lo + (h - LOW_H) as i64 * (self.hi - self.lo) / (HIGH_H - LOW_H) as i64
    }

    /// The small graph's percentages (Q16): 8 % margins all round.
    pub fn gx(&self, x: i64) -> i32 {
        (8 * Q + 84 * Q * (x - self.x0) / (self.x1 - self.x0)) as i32
    }

    pub fn gy(&self, y: i64) -> i32 {
        (92 * Q - 84 * Q * (y - self.lo) / (self.hi - self.lo)) as i32
    }

    /// The y a pointer at `pct` (Q16 percent from the top) means.
    pub fn from_gy(&self, pct: i64) -> i64 {
        self.lo + (92 * Q - pct) * (self.hi - self.lo) / (84 * Q)
    }
}

/// Reads a value off the straight lines through a table's points.
pub fn lerp_table(pts: &[(i64, i64)], x: i64) -> i64 {
    match pts.len() {
        0 => 0,
        1 => pts[0].1,
        _ => {
            let k = pts.windows(2).position(|w| x <= w[1].0).unwrap_or(pts.len() - 2);
            let ((xa, ya), (xb, yb)) = (pts[k], pts[k + 1]);
            if xb == xa {
                ya
            } else {
                ya + (yb - ya) * (x - xa) / (xb - xa)
            }
        }
    }
}

// ---- the planner -----------------------------------------------------------

/// The tree's nodes, in the order they are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Node {
    Plot,
    NextPoint,
    PointsLeft,
    ReadMachine,
    ImaginePush,
    OneBounce,
    CloseEnough,
    ImagineLean,
    Mark,
    SetUp,
    Bow,
}

/// Each node: its depth in the tree and its kind (selector, sequence,
/// condition, action), for drawing.
pub const TREE: &[(Node, u8, &str)] = &[
    (Node::Plot, 0, "selector"),
    (Node::NextPoint, 1, "sequence"),
    (Node::PointsLeft, 2, "condition"),
    (Node::ReadMachine, 2, "action"),
    (Node::ImaginePush, 2, "action"),
    (Node::OneBounce, 2, "selector"),
    (Node::CloseEnough, 3, "condition"),
    (Node::ImagineLean, 3, "action"),
    (Node::Mark, 3, "action"),
    (Node::SetUp, 3, "action"),
    (Node::Bow, 1, "action"),
];

/// One line of the planner's thinking: a copy key under `lab.log` and the
/// values to fill it with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    pub key: &'static str,
    pub vals: Vec<(&'static str, String)>,
}

/// The keys for one contact with the bed: push (positive) or stop
/// (negative) for so many beats, lean forward (positive) or back for so many.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Choice {
    pub push: i32,
    pub lean: i32,
    pub marking: bool,
}

/// Beats counted through one contact, so a choice becomes keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Beats {
    pub push: i32,
    pub contact: i32,
}

impl Choice {
    /// The keys this tick.
    pub fn keys(&self, p: &Performer, b: &Beats) -> Controls {
        let pushing = p.vel.y >= 0;
        Controls {
            drive: pushing && self.push > 0 && b.push < self.push,
            kill: pushing && self.push < 0 && b.push < -self.push,
            travel: if b.contact < self.lean.abs() {
                self.lean.signum()
            } else {
                0
            },
            ..Controls::default()
        }
    }
}

impl Beats {
    pub fn count(&mut self, p: &Performer) {
        self.contact += 1;
        if p.vel.y >= 0 {
            self.push += 1;
        }
    }
}

/// Where the next top would be: the performer copied and bounced forward
/// with a choice of keys. `None` if the copy never leaves the bed.
pub fn imagine(def: &BodyDef, p: &Performer, c: Choice) -> Option<(i32, i32)> {
    let mut q = p.clone();
    let mut beats = Beats::default();
    for _ in 0..480 {
        let keys = if matches!(q.phase, Phase::Bed(_)) {
            let k = c.keys(&q, &beats);
            beats.count(&q);
            k
        } else {
            Controls::default()
        };
        for e in q.step(def, &keys) {
            match e {
                Event::Peak(h) => return Some((q.com.x, h)),
                Event::Crashed(_) => return None,
                _ => {}
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    pub i: Option<usize>,
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planner {
    pub next: usize,
    pub choice: Choice,
    pub beats: Beats,
    /// The contact the current choice was made for.
    pub decided: u32,
    pub lit: Vec<Node>,
    pub log: Vec<Line>,
    pub marks: Vec<Mark>,
    pub bowing: bool,
    pub done: bool,
}

impl Planner {
    pub fn new() -> Planner {
        Planner {
            next: 0,
            choice: Choice::default(),
            beats: Beats::default(),
            decided: u32::MAX,
            lit: vec![Node::Plot],
            log: Vec::new(),
            marks: Vec::new(),
            bowing: false,
            done: false,
        }
    }

    fn say(&mut self, key: &'static str, vals: Vec<(&'static str, String)>) {
        self.log.push(Line { key, vals });
        if self.log.len() > 300 {
            self.log.remove(0);
        }
    }

    /// The keys this tick. `contact` counts landings on the bed so far.
    pub fn keys(&mut self, def: &BodyDef, board: &Board, p: &Performer, contact: u32) -> Controls {
        if !matches!(p.phase, Phase::Bed(Posture::Feet)) || self.done {
            return Controls::default();
        }
        // a choice that left the performer standing still is made again
        if self.decided != contact || self.beats.contact > 90 {
            self.decided = contact;
            self.beats = Beats::default();
            self.decide(def, board, p);
        }
        let k = self.choice.keys(p, &self.beats);
        self.beats.count(p);
        k
    }

    fn decide(&mut self, def: &BodyDef, board: &Board, p: &Performer) {
        // skip the points the machine has no value for
        while self.next < board.n() {
            if let Err(why) = board.ys[self.next] {
                let x = expr::show(board.x(self.next));
                self.say("no_value", vec![("x", x), ("why", why.to_string())]);
                self.next += 1;
            } else {
                break;
            }
        }
        if self.next >= board.n() {
            self.lit = vec![Node::Plot, Node::Bow];
            if !self.bowing {
                self.bowing = true;
                self.say("bow", vec![]);
            }
            self.choice = Choice {
                push: -STOP_MAX,
                lean: 0,
                marking: false,
            };
            // standing still on the bed: the routine is over
            if p.vel.y.abs() < GRAVITY * 2 && p.height() < int(1) + ratio(5, 10) {
                self.done = true;
            }
            return;
        }
        let i = self.next;
        let y = board.ys[i].unwrap_or(0);
        let want_h = board.height(y);
        let want_x = board.bed_x(board.x(i));
        self.lit = vec![Node::Plot, Node::NextPoint, Node::PointsLeft, Node::ReadMachine];
        self.say("read", vec![("x", expr::show(board.x(i))), ("y", expr::show(y))]);

        // how hard to push: imagine, then interpolate
        self.lit.push(Node::ImaginePush);
        let unit = |h: i32| expr::show(board.unit_y(h));
        let mut tried: Vec<(i32, i32)> = Vec::new();
        let mut try_push = |me: &mut Planner, u: i32| -> i32 {
            if let Some(&(_, h)) = tried.iter().find(|(v, _)| *v == u) {
                return h;
            }
            let h = imagine(
                def,
                p,
                Choice {
                    push: u,
                    lean: 0,
                    marking: false,
                },
            )
            .map(|(_, h)| h)
            .unwrap_or(0);
            let key = if u >= 0 { "imagine_push" } else { "imagine_stop" };
            me.say(key, vec![("beats", u.abs().to_string()), ("y", unit(h))]);
            tried.push((u, h));
            h
        };
        let h0 = try_push(self, 0);
        let (mut a, mut ha, mut b, mut hb) = if h0 < want_h {
            (0, h0, PUSH_MAX, try_push(self, PUSH_MAX))
        } else {
            (-STOP_MAX, try_push(self, -STOP_MAX), 0, h0)
        };
        let mut best = if (h0 - want_h).abs() <= (ha - want_h).abs().min((hb - want_h).abs()) {
            (0, h0)
        } else if (ha - want_h).abs() < (hb - want_h).abs() {
            (a, ha)
        } else {
            (b, hb)
        };
        let reachable = ha <= want_h + REACH && hb >= want_h - REACH;
        if reachable {
            for _ in 0..6 {
                if (best.1 - want_h).abs() <= TOL_H || b - a <= 1 {
                    break;
                }
                // where the target sits on the line between the two
                let span = (hb - ha).max(1) as i64;
                let along = ((want_h - ha) as i64 * 100 / span).clamp(0, 100);
                let mut u = a + ((b - a) as i64 * (want_h - ha) as i64 / span) as i32;
                u = u.clamp(a + 1, b - 1);
                self.say(
                    "between",
                    vec![
                        ("y", unit(want_h)),
                        ("a", a.to_string()),
                        ("ya", unit(ha)),
                        ("b", b.to_string()),
                        ("yb", unit(hb)),
                        ("pct", along.to_string()),
                        ("u", u.to_string()),
                    ],
                );
                let h = try_push(self, u);
                if (h - want_h).abs() < (best.1 - want_h).abs() {
                    best = (u, h);
                }
                if h < want_h {
                    a = u;
                    ha = h;
                } else {
                    b = u;
                    hb = h;
                }
            }
        }
        let push = best.0;
        self.lit.push(Node::OneBounce);
        if !reachable || (best.1 - want_h).abs() > REACH {
            // too far for one bounce: a set-up bounce in place toward it
            self.lit.push(Node::SetUp);
            self.say("set_up", vec![("y", unit(want_h))]);
            self.choice = Choice {
                push,
                lean: 0,
                marking: false,
            };
            return;
        }
        self.lit.push(Node::CloseEnough);

        // how far to lean so the top lands over the column
        self.lit.push(Node::ImagineLean);
        let ux = |x: i32| expr::show(board.unit_x(x));
        let mut lean_tried: Vec<(i32, i32)> = Vec::new();
        let mut try_lean = |me: &mut Planner, w: i32| -> i32 {
            if let Some(&(_, x)) = lean_tried.iter().find(|(v, _)| *v == w) {
                return x;
            }
            let x = imagine(
                def,
                p,
                Choice {
                    push,
                    lean: w,
                    marking: true,
                },
            )
            .map(|(x, _)| x)
            .unwrap_or(p.com.x);
            me.say("imagine_lean", vec![("beats", w.to_string()), ("x", ux(x))]);
            lean_tried.push((w, x));
            x
        };
        let x0 = try_lean(self, 0);
        let mut bestw = (0, x0);
        if (x0 - want_x).abs() > TOL_X {
            let (mut a, mut xa, mut b, mut xb) = if x0 < want_x {
                (0, x0, LEAN_MAX, try_lean(self, LEAN_MAX))
            } else {
                (-LEAN_MAX, try_lean(self, -LEAN_MAX), 0, x0)
            };
            for (w, x) in [(a, xa), (b, xb)] {
                if (x - want_x).abs() < (bestw.1 - want_x).abs() {
                    bestw = (w, x);
                }
            }
            for _ in 0..6 {
                if (bestw.1 - want_x).abs() <= TOL_X || b - a <= 1 {
                    break;
                }
                let span = (xb - xa).max(1) as i64;
                let w = (a + ((b - a) as i64 * (want_x - xa) as i64 / span) as i32).clamp(a + 1, b - 1);
                self.say("lean_between", vec![("x", ux(want_x)), ("w", w.to_string())]);
                let x = try_lean(self, w);
                if (x - want_x).abs() < (bestw.1 - want_x).abs() {
                    bestw = (w, x);
                }
                if x < want_x {
                    a = w;
                    xa = x;
                } else {
                    b = w;
                    xb = x;
                }
            }
        }
        self.lit.push(Node::Mark);
        self.say(
            "go",
            vec![("push", push.to_string()), ("lean", bestw.0.to_string())],
        );
        self.choice = Choice {
            push,
            lean: bestw.0,
            marking: true,
        };
    }

    /// The top of a bounce: a mark, if this bounce was meant to make one.
    pub fn peak(&mut self, board: &Board, x: i32, h: i32) -> Option<Mark> {
        if !self.choice.marking || self.next >= board.n() {
            return None;
        }
        let i = self.next;
        let m = Mark {
            i: Some(i),
            x: board.unit_x(x),
            y: board.unit_y(h),
        };
        let want = board.ys[i].unwrap_or(0);
        self.say(
            "marked",
            vec![
                ("x", expr::show(m.x)),
                ("y", expr::show(m.y)),
                ("fx", expr::show(board.x(i))),
                ("fy", expr::show(want)),
                ("off", expr::show((m.y - want).abs())),
            ],
        );
        self.marks.push(m.clone());
        self.next += 1;
        self.choice.marking = false;
        Some(m)
    }
}

impl Default for Planner {
    fn default() -> Self {
        Planner::new()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::fx::HALF;
    use crate::fx::QUARTER;

    fn def() -> BodyDef {
        BodyDef::parse(include_str!("../../../data/body.json")).unwrap()
    }

    fn q(n: i64) -> i64 {
        n * Q
    }

    /// Runs the planner on a board until it bows; returns its marks.
    pub(crate) fn plot(board: &Board) -> (Planner, i32) {
        let d = def();
        let mut p = Performer::on_bed(&d, -QUARTER);
        p.lean_build = true;
        p.com.x = board.bed_x(board.x0);
        let frame = p.frame();
        p.body.place(&p.shape, &frame);
        p.body.prev = p.body.pos;
        let mut planner = Planner::new();
        let mut contact = 0;
        let mut ticks = 0;
        while !planner.done && ticks < 60 * 120 {
            let keys = planner.keys(&d, board, &p, contact);
            for e in p.step(&d, &keys) {
                match e {
                    Event::Landed(_) => contact += 1,
                    Event::Peak(h) => {
                        planner.peak(board, p.com.x, h);
                    }
                    _ => {}
                }
            }
            ticks += 1;
        }
        (planner, ticks)
    }

    #[test]
    fn a_formula_becomes_a_table_and_a_range_with_zero_in_it() {
        let b = Board::new("x^2", -q(2), q(2), 9).unwrap();
        assert_eq!(b.n(), 9);
        assert_eq!(b.x(1), -q(3) / 2);
        assert_eq!(b.ys[1], Ok(q(9) / 4));
        assert_eq!((b.lo, b.hi), (0, q(4)));
        // the ends of the domain land on the ends of the board
        assert_eq!(b.bed_x(-q(2)), -BOARD_HALF);
        assert_eq!(b.bed_x(q(2)), BOARD_HALF);
        assert_eq!(b.height(0), LOW_H);
        assert_eq!(b.height(q(4)), HIGH_H);
        let s = Board::new("sin(x)", -q(3), q(3), 7).unwrap();
        assert_eq!((s.lo, s.hi), (-q(1), q(1)));
    }

    #[test]
    fn a_drawn_function_reads_new_points_off_its_lines() {
        let mut b = Board::new("x", 0, q(4), 3).unwrap();
        b.drag(1, q(3) + 1000);
        assert_eq!(b.ys[1], Ok(q(3)));
        assert!(b.drawn);
        b.resample(5);
        // between (0,0) and (2,3) the line at x = 1 is 1.5
        assert_eq!(b.ys[1], Ok(q(3) / 2));
        assert_eq!(b.ys[2], Ok(q(3)));
    }

    #[test]
    fn the_planner_plots_x_squared_within_a_few_centimetres_at_every_point() {
        let board = Board::new("x^2", -q(2), q(2), 9).unwrap();
        let (planner, ticks) = plot(&board);
        assert!(
            planner.done,
            "never finished in {ticks} ticks: {:?}",
            planner.marks
        );
        assert_eq!(planner.marks.len(), 9);
        for m in &planner.marks {
            let i = m.i.unwrap();
            let want_h = board.height(board.ys[i].unwrap());
            let got_h = board.height(m.y);
            assert!(
                (got_h - want_h).abs() <= REACH,
                "point {i}: {} m off",
                (got_h - want_h) * 100 / ONE
            );
            let want_x = board.bed_x(board.x(i));
            let got_x = board.bed_x(m.x);
            assert!(
                (got_x - want_x).abs() <= TOL_X * 2,
                "point {i}: x {} cm off",
                (got_x - want_x) * 100 / ONE
            );
        }
        let _ = HALF;
    }

    #[test]
    fn a_point_with_no_value_is_skipped_and_said_so() {
        let board = Board::new("1/x", -q(2), q(2), 5).unwrap();
        assert_eq!(board.ys[2], Err("divide_by_zero"));
        let (planner, _) = plot(&board);
        assert!(planner.log.iter().any(|l| l.key == "no_value"));
        assert_eq!(planner.marks.len(), 4);
    }
}
