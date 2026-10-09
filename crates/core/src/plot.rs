//! The Graph Lab: a function, sampled into a table, plotted by bouncing.
//!
//! The bed runs along the graph's x axis; the top of each bounce marks a
//! point, the feet's height standing for f(x). The planner is a behaviour
//! tree whose leaves press only keys a player could press: Space for so
//! many beats to push the bed (C to stop it), and an arrow for so many beats
//! to lean along it. It chooses those beats by imagining the bounce in the
//! core's own physics on a copy of the performer, and interpolating between
//! two imagined bounces that bracket the target (regula falsi).

use crate::balance::{BED_Y, GRAVITY};
use crate::body::BodyDef;
use crate::body::Pose;
use crate::expr::{self, Expr, ReadError, Undefined};
use crate::fx::{self, int, ratio, ONE, QUARTER};
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
/// Marks this high or higher get a somersault on the way; lower ones a tuck jump.
pub const TRICK_FLIP_H: i32 = ratio(22, 10);
pub const TRICK_JUMP_H: i32 = ratio(9, 10);

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
    Trick,
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
    (Node::Trick, 3, "action"),
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
    /// Beats of S (positive) or W (negative) from the start of the contact,
    /// to set a somersault turning; then tuck, kicking out at three quarters.
    pub flip: i32,
    pub tuck: bool,
    /// The somersault in a pike rather than a tuck (released at three
    /// quarters too: a pike held to the landing is a seat drop).
    pub pike: bool,
    /// Knees to chest at the top of a straight bounce.
    pub tuck_jump: bool,
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
            flip: if b.contact < self.flip.abs() {
                self.flip.signum()
            } else {
                0
            },
            ..Controls::default()
        }
    }

    /// The keys in the air: the tuck, held until three quarters of the turn.
    pub fn air_keys(&self, p: &Performer) -> Controls {
        let turned = (p.flip - p.flight.flip0).abs();
        let spinning = p.flip_l != 0 && turned < 3 * QUARTER;
        Controls {
            tuck: (self.tuck && !self.pike && spinning) || (self.tuck_jump && p.vel.y.abs() < GRAVITY * 12),
            pike: self.tuck && self.pike && spinning,
            ..Controls::default()
        }
    }
}

/// The pen's height at the top of a bounce: where the feet would be if the
/// performer were standing straight, so a somersault does not move the mark.
pub fn pen_height(def: &BodyDef, p: &Performer) -> i32 {
    p.com.y - BED_Y - Performer::reach(def, Pose::Stand, 0)
}

/// How a choice's flight ends: the turn of the flip by the landing, and
/// whether it came down on the feet.
pub fn imagine_landing(def: &BodyDef, p: &Performer, c: Choice) -> Option<(i32, bool)> {
    let mut q = p.clone();
    let mut beats = Beats::default();
    let mut flew = false;
    for _ in 0..600 {
        let keys = match q.phase {
            Phase::Bed(_) => {
                let k = c.keys(&q, &beats);
                beats.count(&q);
                k
            }
            Phase::Air => {
                flew = true;
                c.air_keys(&q)
            }
            _ => Controls::default(),
        };
        for e in q.step(def, &keys) {
            match e {
                Event::Landed(l) if flew => return Some((l.quarters * QUARTER, l.to == Posture::Feet)),
                Event::Crashed(_) => return Some((0, false)),
                _ => {}
            }
        }
    }
    None
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
        let keys = match q.phase {
            Phase::Bed(_) => {
                let k = c.keys(&q, &beats);
                beats.count(&q);
                k
            }
            Phase::Air => c.air_keys(&q),
            _ => Controls::default(),
        };
        for e in q.step(def, &keys) {
            match e {
                Event::Peak(_) => return Some((q.com.x, pen_height(def, &q))),
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
        if p.phase == Phase::Air {
            return self.choice.air_keys(p);
        }
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
                ..Choice::default()
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
                    ..Choice::default()
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
                ..Choice::default()
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
                    ..Choice::default()
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
        let mut choice = Choice {
            push,
            lean: bestw.0,
            marking: true,
            ..Choice::default()
        };
        self.choose_trick(def, p, want_h, &mut choice);
        self.lit.push(Node::Mark);
        self.say(
            "go",
            vec![("push", push.to_string()), ("lean", bestw.0.to_string())],
        );
        self.choice = choice;
    }

    /// A trick for the marking flight, if it is high enough for one: a
    /// somersault, its flip beats found by imagining the landing and
    /// interpolating toward one whole turn, or a tuck jump.
    fn choose_trick(&mut self, def: &BodyDef, p: &Performer, want_h: i32, choice: &mut Choice) {
        self.lit.push(Node::Trick);
        if want_h >= TRICK_FLIP_H {
            // back somersaults, tucked and piked in turn (a front one off a
            // forward lean over-rotates in imagination: SECOND-ORDER L6)
            let dir = 1;
            let pike = self.marks.len() % 2 == 1;
            let key = "S";
            let target = dir * fx::TURN;
            let mut tried: Vec<(i32, i32, bool)> = Vec::new();
            let base = *choice;
            let mut try_flip = |me: &mut Planner, f: i32| -> (i32, bool) {
                if let Some(&(_, t, ok)) = tried.iter().find(|(b, _, _)| *b == f) {
                    return (t, ok);
                }
                let c = Choice {
                    flip: dir * f,
                    tuck: true,
                    pike,
                    ..base
                };
                let (turn, ok) = imagine_landing(def, p, c).unwrap_or((0, false));
                let deg = (turn.abs() as i64 * 360 / fx::TURN as i64).to_string();
                me.say(
                    "imagine_flip",
                    vec![("key", key.into()), ("beats", f.to_string()), ("deg", deg)],
                );
                tried.push((f, turn, ok));
                (turn, ok)
            };
            let (mut a, mut b) = (4, 18);
            let (mut ta, _) = try_flip(self, a);
            let (mut tb, _) = try_flip(self, b);
            for _ in 0..4 {
                if b - a <= 1 || tb == ta {
                    break;
                }
                let f = (a + ((b - a) as i64 * (target - ta) as i64 / (tb - ta) as i64) as i32)
                    .clamp(a + 1, b - 1);
                self.say("flip_between", vec![("beats", f.to_string())]);
                let (t, _) = try_flip(self, f);
                if t == target {
                    break;
                }
                if (t - target) * dir < 0 {
                    a = f;
                    ta = t;
                } else {
                    b = f;
                    tb = t;
                }
            }
            let best = tried
                .iter()
                .filter(|(_, t, ok)| *ok && *t == target)
                .min_by_key(|(f, _, _)| *f)
                .copied();
            if let Some((f, _, _)) = best {
                choice.flip = dir * f;
                choice.tuck = true;
                choice.pike = pike;
                let name = if pike { "back pike" } else { "back tuck" };
                self.say("trick", vec![("trick", name.into())]);
                return;
            }
        }
        if want_h >= TRICK_JUMP_H {
            let c = Choice {
                tuck_jump: true,
                ..*choice
            };
            if let Some((_, true)) = imagine_landing(def, p, c) {
                choice.tuck_jump = true;
                self.say("trick", vec![("trick", "tuck jump".into())]);
            }
        }
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
                    Event::Peak(_) => {
                        let h = pen_height(&d, &p);
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
        // the high marks get somersaults on the way, and still land
        let tricks: Vec<&Line> = planner.log.iter().filter(|l| l.key == "trick").collect();
        let flips: Vec<&Line> = planner
            .log
            .iter()
            .filter(|l| l.key == "imagine_flip" || l.key == "read")
            .collect();
        assert!(tricks.len() >= 4, "{:?} {:?}", tricks, flips);
        assert!(
            tricks
                .iter()
                .any(|l| l.vals[0].1 == "back tuck" || l.vals[0].1 == "back pike"),
            "{:?}",
            tricks
        );
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
