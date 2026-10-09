//! The Popcorn Lab: a jump off a tall ledge onto the bed, the flight drawn
//! as a curve, and the popcorn dropped under it counting the area beneath.
//!
//! Every free flight is a parabola: x moves on at a steady speed while y
//! gains and loses height under gravity, so the path is
//! y = y0 + (vy / vx)(x − x0) − g / (2 vx²) (x − x0)². The lab writes each
//! arc's equation from its takeoff. As the performer flies, at one place in
//! each strip of width Δx they let go of a column of popcorn as tall as they
//! are high, which falls and piles on the ground in front of the bed: the
//! piles are a Riemann sum, and counting kernels counts area.

use crate::balance::{BED_HALF_Z, BED_Z, GRAVITY, TICKS_PER_SECOND};
use crate::body::cm;
use crate::expr::{self, Expr};
use crate::fx::{self, deg, int, ratio, ONE};
use crate::geom::{v3, V3};

/// The ledge: a tall platform to the left of the bed; its edge, and the
/// heights it can be set to.
pub const LEDGE_X: i32 = -ratio(46, 10);
pub const LEDGE_MIN: i32 = int(3);
pub const LEDGE_MAX: i32 = int(8);
/// Where the drawing stops on the right: past the bed, over the grass.
pub const END_X: i32 = ratio(75, 10);
/// One kernel stands for a square this many metres on a side.
pub const KERNEL: i32 = ONE / 4;
/// The piles sit on the ground in front of the bed, toward the crowd.
pub fn tray_z() -> i32 {
    BED_Z + BED_HALF_Z + ratio(9, 10)
}
/// Popcorn falls lighter than a body: half of gravity.
const KERNEL_G: i32 = GRAVITY / 2;
const Q: i64 = ONE as i64;

/// Where in each strip the popcorn is let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Left,
    Middle,
    Right,
}

impl Rule {
    pub fn from_index(i: u32) -> Rule {
        match i {
            0 => Rule::Left,
            2 => Rule::Right,
            _ => Rule::Middle,
        }
    }
    pub fn index(self) -> u32 {
        self as u32
    }
}

/// A jump: speed in metres a second, angle above level in degrees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Jump {
    pub ledge: i32,
    pub speed: i32,
    pub angle: i32,
}

impl Jump {
    /// The takeoff velocity, metres a tick.
    pub fn velocity(&self) -> V3 {
        let a = deg(self.angle);
        let per_tick = self.speed / TICKS_PER_SECOND;
        v3(fx::mul(per_tick, fx::cos(a)), fx::mul(per_tick, fx::sin(a)), 0)
    }
}

/// The strips the ground is cut into, and where each one's popcorn is let go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strips {
    pub width: i32,
    pub rule: Rule,
}

impl Strips {
    pub fn count(&self) -> usize {
        ((END_X - LEDGE_X) / self.width.max(ONE / 16)) as usize
    }
    pub fn left(&self, i: usize) -> i32 {
        LEDGE_X + self.width * i as i32
    }
    pub fn release_x(&self, i: usize) -> i32 {
        match self.rule {
            Rule::Left => self.left(i),
            Rule::Middle => self.left(i) + self.width / 2,
            Rule::Right => self.left(i) + self.width,
        }
    }
    /// Kernels across one strip.
    pub fn columns(&self) -> i32 {
        (self.width / KERNEL).max(1)
    }
}

/// One kernel in the air or on its pile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kernel {
    pub from: V3,
    pub to: V3,
    pub released: i64,
    pub fall: i32,
}

impl Kernel {
    /// Where it is at a tick: falling straight down in height while it drifts
    /// to its place on the pile, then resting there.
    pub fn at(&self, tick: i64) -> V3 {
        let t = (tick - self.released).max(0) as i32;
        if t >= self.fall {
            return self.to;
        }
        let share = t * ONE / self.fall.max(1);
        let drop = (KERNEL_G as i64 * t as i64 * t as i64 / 2).min(int(100) as i64) as i32;
        let x = fx::lerp(self.from.x, self.to.x, share);
        let z = fx::lerp(self.from.z, self.to.z, share);
        v3(x, (self.from.y - drop).max(self.to.y), z)
    }
}

/// A pile, as it was released: the height the performer was at, and how
/// many kernels that makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pile {
    pub strip: usize,
    pub height: i32,
    pub kernels: i32,
}

/// The kernels a strip's pile gets for a height: the strip's area in kernel
/// squares, rounded.
pub fn kernels_for(height: i32, width: i32) -> i32 {
    let area = height.max(0) as i64 * width as i64;
    let square = KERNEL as i64 * KERNEL as i64;
    ((area + square / 2) / square) as i32
}

/// The kernels of a pile, each with its place: columns across the strip,
/// rows up from the ground.
pub fn pour(strips: &Strips, pile: &Pile, from: V3, tick: i64) -> Vec<Kernel> {
    let cols = strips.columns();
    let cell = strips.width / cols;
    let x0 = strips.left(pile.strip);
    let z = tray_z();
    (0..pile.kernels)
        .map(|k| {
            let (col, row) = (k % cols, k / cols);
            let to = v3(x0 + cell * col + cell / 2, KERNEL * row + KERNEL / 2, z);
            let h = (from.y - to.y).max(cm(10));
            // the fall takes √(2h/g) ticks
            let fall = fx::isqrt((2 * h as i64 / KERNEL_G as i64) as u64) as i32 + 1;
            Kernel {
                from,
                to,
                released: tick + (k / cols) as i64 / 3,
                fall,
            }
        })
        .collect()
}

/// An arc's equation, from where it left the ground or the bed and how fast:
/// the three coefficients of y = y0 + b(x − x0) − a(x − x0)², and the top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arc {
    pub x0: i32,
    pub y0: i32,
    pub b: i64,
    pub a: i64,
    pub top_x: i32,
    pub top_y: i32,
}

/// The arc a takeoff draws. Velocities are metres a tick; the equation is in
/// metres, whatever the clock.
pub fn arc(x0: i32, y0: i32, vel: V3) -> Option<Arc> {
    if vel.x <= 0 {
        return None;
    }
    // The game steps speed before place each tick, so after n ticks the
    // height is y0 + n(vy − g/2) − g n²/2: the integrator's own parabola,
    // which the equation matches exactly (a smooth throw would use vy).
    let (vx, vy) = (vel.x as i64, vel.y as i64 - GRAVITY as i64 / 2);
    // b = vy / vx; a = g / (2 vx²), the tick cancels out of both
    let b = vy * Q / vx;
    let a = GRAVITY as i64 * Q * Q / (2 * vx * vx);
    // top where the slope b − 2a(x − x0) is zero
    let dx = if a > 0 { b * Q / (2 * a) } else { 0 };
    let top_y = y0 as i64 + b * dx / Q - a * dx / Q * dx / Q;
    Some(Arc {
        x0,
        y0,
        b,
        a,
        top_x: x0 + dx as i32,
        top_y: top_y as i32,
    })
}

/// "6 + 0.42(x + 4.6) − 0.31(x + 4.6)²", as the lab writes an arc.
pub fn equation(a: &Arc) -> (String, String) {
    let shift = |x: i32| {
        if x < 0 {
            format!("x + {}", expr::show(-(x as i64)))
        } else if x == 0 {
            "x".to_string()
        } else {
            format!("x − {}", expr::show(x as i64))
        }
    };
    let sign = |v: i64| if v < 0 { "−" } else { "+" };
    let s = shift(a.x0);
    let standard = format!(
        "y = {} {} {}({}) − {}({})²",
        expr::show(a.y0 as i64),
        sign(a.b),
        expr::show(a.b.abs()),
        s,
        expr::show(a.a),
        s
    );
    let top = format!(
        "y = {} − {}({})²",
        expr::show(a.top_y as i64),
        expr::show(a.a),
        shift(a.top_x)
    );
    (standard, top)
}

/// The area under a path of points (x rising), by trapezoids: the fine
/// answer the popcorn and the Riemann sum are measured against.
pub fn area_under(path: &[(i32, i32)], from: i32, to: i32) -> i64 {
    let mut sum = 0i64;
    for w in path.windows(2) {
        let ((xa, ya), (xb, yb)) = (w[0], w[1]);
        let (lo, hi) = (xa.max(from), xb.min(to));
        if hi <= lo || xb <= xa {
            continue;
        }
        let y_at = |x: i32| ya as i64 + (yb - ya) as i64 * (x - xa) as i64 / (xb - xa) as i64;
        sum += (y_at(lo) + y_at(hi)).max(0) * (hi - lo) as i64 / 2;
    }
    sum / Q
}

// ---- matching a curve -----------------------------------------------------------

/// The path a jump would draw from the ledge until it comes down to the
/// bed's height: (x, height of the feet) every tick. `reach` is how far the
/// feet hang below the centre of mass.
pub fn imagine_flight(j: &Jump, reach: i32, stop_y: i32) -> Vec<(i32, i32)> {
    let mut pos = v3(LEDGE_X, j.ledge + reach, 0);
    let mut vel = j.velocity();
    let mut out = vec![(pos.x, pos.y - reach)];
    for _ in 0..600 {
        vel.y -= GRAVITY;
        pos = pos.add(vel);
        out.push((pos.x, pos.y - reach));
        if vel.y < 0 && pos.y - reach <= stop_y {
            break;
        }
    }
    out
}

/// How far a path is from a function, as the mean of the squared gaps at a
/// dozen places along it (metres², Q16). `None` if f has no value there.
pub fn misfit(path: &[(i32, i32)], f: &Expr) -> Option<i64> {
    let n = path.len();
    if n < 4 {
        return None;
    }
    let mut sum = 0i64;
    let samples = 12;
    for k in 0..samples {
        let (x, y) = path[(n - 1) * k / (samples - 1)];
        let want = f.eval(x as i64).ok()?;
        let d = y as i64 - want;
        sum += d * d / Q;
    }
    Some(sum / samples as i64)
}

/// One line of the matcher's thinking: a copy key under `lab.fly_log` and
/// its values.
pub type Line = (&'static str, Vec<(&'static str, String)>);

/// The jump whose flight fits `f` best, found the way the plotter finds a
/// push: imagine a few, then interpolate. For each speed, the best angle by
/// fitting a parabola through three misfits and trying its lowest point
/// (successive parabolic interpolation: a parabola to find the best
/// parabola); then the same across speeds.
pub fn fit(f: &Expr, ledge: i32, reach: i32, stop_y: i32, log: &mut Vec<Line>) -> Jump {
    let score = |speed: i32, angle: i32| -> i64 {
        let j = Jump { ledge, speed, angle };
        misfit(&imagine_flight(&j, reach, stop_y), f).unwrap_or(i64::MAX / 4)
    };
    // the best angle for one speed: a coarse sweep, then parabolic steps
    let best_angle = |speed: i32, log: &mut Vec<Line>, say: bool| -> (i32, i64) {
        let mut best = (0, i64::MAX);
        let mut a = -20;
        while a <= 70 {
            let s = score(speed, a);
            if s < best.1 {
                best = (a, s);
            }
            a += 15;
        }
        let mut step = 15;
        let mut centre = best;
        for _ in 0..5 {
            let (l, r) = (centre.0 - step, centre.0 + step);
            let (sl, sr) = (score(speed, l), score(speed, r));
            // the parabola through (l, sl), (c, sc), (r, sr): its lowest point
            let den = sl - 2 * centre.1 + sr;
            let next = if den > 0 {
                centre.0 + ((step as i64 * (sl - sr)) / (2 * den)) as i32
            } else if sl < sr {
                l
            } else {
                r
            };
            let next = next.clamp(-30, 80);
            let sn = score(speed, next);
            if say {
                log.push((
                    "parabola",
                    vec![
                        ("l", l.to_string()),
                        ("r", r.to_string()),
                        ("next", next.to_string()),
                    ],
                ));
            }
            if sn < centre.1 {
                centre = (next, sn);
            }
            step = (step / 2).max(1);
        }
        centre
    };
    let mut best = (int(5), 0, i64::MAX);
    let mut speed = int(2);
    while speed <= int(10) {
        let (a, s) = best_angle(speed, log, false);
        log.push((
            "try_speed",
            vec![
                ("speed", expr::show(speed as i64)),
                ("angle", a.to_string()),
                ("off", expr::show(isqrt_q(s))),
            ],
        ));
        if s < best.2 {
            best = (speed, a, s);
        }
        speed += int(1);
    }
    // between the neighbouring speeds, the parabola's lowest point again
    let (sp, _, _) = best;
    for half in [ONE / 2, -ONE / 2, ONE / 4, -ONE / 4] {
        let s2 = (sp + half).clamp(int(1), int(12));
        let (a2, sc) = best_angle(s2, log, false);
        if sc < best.2 {
            best = (s2, a2, sc);
        }
    }
    let (speed, angle, s) = best;
    let _ = best_angle(speed, log, true);
    log.push((
        "fit",
        vec![
            ("speed", expr::show(speed as i64)),
            ("angle", angle.to_string()),
            ("off", expr::show(isqrt_q(s))),
        ],
    ));
    Jump { ledge, speed, angle }
}

/// √ of a Q16 square, in Q16: how far off, from how far off squared.
fn isqrt_q(v: i64) -> i64 {
    fx::isqrt((v.max(0) as u64) << fx::FRAC_BITS) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_jump_draws_the_parabola_its_speed_says() {
        // 6 m/s level from 6 m: b = 0, a = g / (2 vx²) = 9.81 / 72 = 0.136
        let j = Jump {
            ledge: int(6),
            speed: int(6),
            angle: 0,
        };
        let a = arc(LEDGE_X, int(6), j.velocity()).unwrap();
        // b is −g/(2vx) from the tick's half step: −0.0136, nearly level
        assert!(a.b < 0 && a.b > -Q / 50, "{}", a.b);
        assert!((a.a - (Q * 136 / 1000)).abs() < Q / 200, "{}", a.a);
        assert!((a.top_x - LEDGE_X).abs() < ONE / 10, "{}", a.top_x);
        let (standard, top) = equation(&a);
        assert!(standard.starts_with("y = 6 "), "{standard}");
        assert!(top.contains("(x + 4.6"), "{top}");
    }

    #[test]
    fn the_equation_predicts_the_flight_it_came_from() {
        let j = Jump {
            ledge: int(5),
            speed: int(5),
            angle: 35,
        };
        let a = arc(LEDGE_X, int(5), j.velocity()).unwrap();
        let path = imagine_flight(&j, 0, int(1));
        for &(x, y) in path.iter().step_by(7) {
            let d = (x - a.x0) as i64;
            let want = a.y0 as i64 + a.b * d / Q - a.a * d / Q * d / Q;
            // the equation is the integrator's own: within a centimetre
            assert!((y as i64 - want).abs() < Q / 100, "at x {x}: {y} vs {want}");
        }
    }

    #[test]
    fn popcorn_piles_add_up_to_the_strip_areas() {
        // a strip 0.5 m wide at 3 m high is 1.5 m², 24 kernels of 1/16 m²
        assert_eq!(kernels_for(int(3), ONE / 2), 24);
        let strips = Strips {
            width: ONE / 2,
            rule: Rule::Middle,
        };
        assert_eq!(strips.columns(), 2);
        let pile = Pile {
            strip: 3,
            height: int(3),
            kernels: 24,
        };
        let ks = pour(&strips, &pile, v3(0, int(6), 0), 0);
        assert_eq!(ks.len(), 24);
        // the top kernel sits in row 11: 12 rows of 0.25 m make the 3 m pile
        let top = ks.iter().map(|k| k.to.y).max().unwrap();
        assert_eq!(top, KERNEL * 11 + KERNEL / 2);
        // each lands where it is going
        let k = ks[0];
        assert_eq!(k.at(10_000), k.to);
    }

    #[test]
    fn the_area_under_a_straight_ramp_is_a_triangle() {
        // from (0, 0) to (4, 2): 4 m²
        let path: Vec<(i32, i32)> = (0..=40).map(|i| (int(i) / 10, int(i) / 20)).collect();
        assert!((area_under(&path, 0, int(4)) - Q * 4).abs() <= 2);
    }

    #[test]
    fn the_matcher_finds_the_jump_that_drew_a_curve() {
        // draw a curve with a known jump, then ask the matcher for it back
        let real = Jump {
            ledge: int(6),
            speed: int(4),
            angle: 20,
        };
        let a = arc(LEDGE_X, int(6), real.velocity()).unwrap();
        let src = format!(
            "6 + {}(x + 4.6) - {}(x + 4.6)^2",
            expr::show(a.b),
            expr::show(a.a)
        );
        let f = expr::parse(&src).unwrap();
        let mut log = Vec::new();
        let got = fit(&f, int(6), 0, int(1), &mut log);
        assert!((got.speed - real.speed).abs() <= ONE / 2, "{:?}", got);
        assert!((got.angle - real.angle).abs() <= 6, "{:?}", got);
        assert!(log.iter().any(|(k, _)| *k == "parabola"));
    }
}
