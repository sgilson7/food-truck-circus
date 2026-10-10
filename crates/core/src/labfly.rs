//! The Popcorn Lab's running: the performer on the ledge, the jump, the
//! bounce onward, the popcorn let go strip by strip, and what the page shows.
//! The mathematics is in fly.rs; this file runs it in the game.

use crate::balance::{BED_Z, LEAN_STEP};
use crate::body::{self, Pose};
use crate::expr::{self, Expr};
use crate::fly::{self, Arc, Jump, Kernel, Pile, Rule, Strips, END_X, KERNEL, LEDGE_MAX, LEDGE_MIN, LEDGE_X};
use crate::fx::{int, ratio, ONE, QUARTER};
use crate::game::{pct, Game};
use crate::geom::{v3, Affine, V3};
use crate::mesh::Mesh;
use crate::performer::{orientation, Event, Performer, Phase};
use crate::plot::{Beats, Choice};
use crate::render::{self, Batch};
use crate::stage::Surface;
use crate::tour::rgb;
use serde_json::{json, Value};

const Q: i64 = ONE as i64;
/// The small graph's window, metres: from just left of the ledge to the
/// end of the drawing, and from the ground to above the highest jump.
const GX0: i32 = LEDGE_X - ONE;
const GY1: i32 = int(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlyPhase {
    Ready,
    Flying,
    Done,
}

pub struct FlyLab {
    pub jump: Jump,
    pub strips: Strips,
    pub phase: FlyPhase,
    pub piles: Vec<Pile>,
    pub kernels: Vec<Kernel>,
    pub next_strip: usize,
    /// The pen (feet, as if standing) every tick, metres above the ground.
    pub path: Vec<(i32, i32)>,
    pub arcs: Vec<Arc>,
    pub target: Option<(String, Expr)>,
    pub log: Vec<fly::Line>,
    pub choice: Choice,
    pub beats: Beats,
    pub error: String,
    pub done: Option<(i64, i64, i64)>,
}

impl FlyLab {
    pub fn new() -> FlyLab {
        FlyLab {
            jump: Jump {
                ledge: int(6),
                speed: int(4),
                angle: 25,
            },
            strips: Strips {
                width: ONE / 2,
                rule: Rule::Middle,
            },
            phase: FlyPhase::Ready,
            piles: Vec::new(),
            kernels: Vec::new(),
            next_strip: 0,
            path: Vec::new(),
            arcs: Vec::new(),
            target: None,
            log: Vec::new(),
            choice: Choice::default(),
            beats: Beats::default(),
            error: String::new(),
            done: None,
        }
    }
}

impl Default for FlyLab {
    fn default() -> Self {
        FlyLab::new()
    }
}

fn gx(x: i32) -> String {
    pct(((8 * Q + 84 * Q * (x - GX0) as i64 / (END_X - GX0) as i64) * Q / Q) as i32)
}

fn gy(y: i32) -> String {
    pct((92 * Q - 84 * Q * y.clamp(0, GY1) as i64 / GY1 as i64) as i32)
}

impl Game {
    fn reach(&self) -> i32 {
        Performer::reach(&self.def, Pose::Stand, 0)
    }

    /// The performer standing at the ledge's edge, facing the bed.
    pub(crate) fn fly_reset(&mut self) {
        let reach = self.reach();
        let Some(f) = self.lab.as_mut().and_then(|l| l.fly.as_mut()) else {
            return;
        };
        let ledge = f.jump.ledge;
        f.phase = FlyPhase::Ready;
        f.piles.clear();
        f.kernels.clear();
        f.next_strip = 0;
        f.path.clear();
        f.arcs.clear();
        f.done = None;
        let mut p = Performer::new(&self.def, v3(LEDGE_X - ratio(3, 10), ledge, BED_Z), -QUARTER);
        p.lean_build = true;
        p.phase = Phase::Held;
        p.held_pose = Pose::Stand;
        let com = v3(LEDGE_X - ratio(3, 10), ledge + reach, BED_Z);
        p.com = com;
        p.hold_frame = Some(Affine::translate(com).then(&orientation(-QUARTER, 0, 0)));
        let frame = p.frame();
        p.body.place(&p.shape, &frame);
        p.body.prev = p.body.pos;
        self.perf = p;
        if let Some(l) = self.lab.as_mut() {
            l.trail.clear();
        }
    }

    /// The jump's three numbers, as typed or slid: ledge height and speed
    /// in metres (a second), angle in degrees.
    pub fn fly_set(&mut self, ledge: &str, speed: &str, angle: &str) -> String {
        let read = |s: &str| expr::parse(s).ok().and_then(|e| e.eval(0).ok());
        let msg = match (read(ledge), read(speed), read(angle)) {
            (Some(h), Some(v), Some(a)) => {
                if let Some(f) = self.lab.as_mut().and_then(|l| l.fly.as_mut()) {
                    f.jump = Jump {
                        ledge: (h as i32).clamp(LEDGE_MIN, LEDGE_MAX),
                        speed: (v as i32).clamp(ONE / 2, int(12)),
                        angle: ((a / Q) as i32).clamp(-30, 80),
                    };
                }
                String::new()
            }
            _ => self.copy_str(&["lab", "fly_numbers"]),
        };
        self.fly_reset();
        msg
    }

    /// Strip width (in sixteenths of a metre) and where in each strip the
    /// popcorn is let go (0 left, 1 middle, 2 right).
    pub fn fly_strips(&mut self, sixteenths: u32, rule: u32) {
        if let Some(f) = self.lab.as_mut().and_then(|l| l.fly.as_mut()) {
            f.strips = Strips {
                width: (sixteenths.clamp(2, 32) as i32) * ONE / 16,
                rule: Rule::from_index(rule),
            };
        }
        self.fly_reset();
    }

    /// Leaves the ledge.
    pub fn fly_jump(&mut self) {
        self.fly_reset();
        let reach = self.reach();
        let Some(f) = self.lab.as_mut().and_then(|l| l.fly.as_mut()) else {
            return;
        };
        f.phase = FlyPhase::Flying;
        let vel = f.jump.velocity();
        let start = (LEDGE_X, f.jump.ledge);
        f.path.push(start);
        if let Some(a) = fly::arc(start.0, start.1, vel) {
            f.arcs.push(a);
        }
        self.perf.com = v3(LEDGE_X, f.jump.ledge + reach, BED_Z);
        self.perf.launch(vel);
        if let Some(l) = self.lab.as_mut() {
            l.running = true;
        }
        self.sound("spring", 40);
    }

    /// Finds the jump whose flight best fits a typed curve, and sets it.
    pub fn fly_match(&mut self, src: &str) -> String {
        let e = match expr::parse(src) {
            Ok(e) => e,
            Err(err) => {
                let m = self.fill(
                    &self.copy_str(&["lab", "expr", err.key]),
                    &[("at", err.at.to_string()), ("word", err.word.clone())],
                );
                return m;
            }
        };
        let start = match e.eval(LEDGE_X as i64) {
            Ok(v) => v as i32,
            Err(_) => return self.copy_str(&["lab", "fly_no_start"]),
        };
        let ledge = start.clamp(LEDGE_MIN, LEDGE_MAX);
        let reach = self.reach();
        let mut log = Vec::new();
        let best = fly::fit(&e, ledge, 0, crate::balance::BED_Y, &mut log);
        let _ = reach;
        if let Some(f) = self.lab.as_mut().and_then(|l| l.fly.as_mut()) {
            f.jump = best;
            f.target = Some((src.trim().to_string(), e));
            f.log = log;
        }
        self.fly_reset();
        if start != ledge {
            return self.fill(
                &self.copy_str(&["lab", "fly_ledge_moved"]),
                &[("h", expr::show(ledge as i64))],
            );
        }
        String::new()
    }

    pub fn fly_clear_target(&mut self) {
        if let Some(f) = self.lab.as_mut().and_then(|l| l.fly.as_mut()) {
            f.target = None;
            f.log.clear();
        }
    }

    pub(crate) fn fly_step(&mut self) {
        let reach = self.reach();
        let tick = self.tick;
        let flying = self
            .lab
            .as_ref()
            .and_then(|l| l.fly.as_ref())
            .map(|f| f.phase == FlyPhase::Flying)
            .unwrap_or(false);
        // the keys: on the bed, lean on to keep the speed across the bed
        let c = {
            let Some(f) = self.lab.as_mut().and_then(|l| l.fly.as_mut()) else {
                return;
            };
            if flying && matches!(self.perf.phase, Phase::Bed(_)) {
                let k = f.choice.keys(&self.perf, &f.beats);
                f.beats.count(&self.perf);
                k
            } else {
                Default::default()
            }
        };
        let events = self.perf.step(&self.def, &c);
        let pen = (self.perf.com.x, self.perf.com.y - reach);
        let hand = self.perf.body.pos[body::HAND_R];
        let mut pops = 0;
        let mut finished = false;
        {
            let Some(f) = self.lab.as_mut().and_then(|l| l.fly.as_mut()) else {
                return;
            };
            for e in &events {
                match e {
                    Event::Landed(l) if flying => {
                        if l.surface == Surface::Bed {
                            // the bed throws you on: lean as fast as you came
                            let vx = f.path.windows(2).last().map(|w| w[1].0 - w[0].0).unwrap_or(0);
                            let lean = ((vx + LEAN_STEP / 2) / LEAN_STEP).clamp(0, 18);
                            f.choice = Choice {
                                lean,
                                ..Choice::default()
                            };
                            f.beats = Beats::default();
                        } else {
                            finished = true;
                        }
                    }
                    Event::Crashed(_) if flying => finished = true,
                    Event::Takeoff if flying && !f.path.is_empty() => {
                        if let Some(a) = fly::arc(pen.0, pen.1, self.perf.vel) {
                            f.arcs.push(a);
                        }
                    }
                    _ => {}
                }
            }
            if flying {
                if pen.0 > f.path.last().map(|p| p.0).unwrap_or(i32::MIN) {
                    f.path.push(pen);
                }
                // popcorn for every strip the pen has passed its release point in
                while f.next_strip < f.strips.count()
                    && pen.0 >= f.strips.release_x(f.next_strip)
                    && pen.0 <= END_X
                {
                    let i = f.next_strip;
                    let height = pen.1.max(0);
                    let pile = Pile {
                        strip: i,
                        height,
                        kernels: fly::kernels_for(height, f.strips.width),
                    };
                    f.kernels.extend(fly::pour(&f.strips, &pile, hand, tick));
                    f.piles.push(pile);
                    f.next_strip += 1;
                    pops += 1;
                }
                if pen.0 > END_X + ONE || finished {
                    f.phase = FlyPhase::Done;
                    let last = f.path.last().map(|p| p.0).unwrap_or(LEDGE_X).min(END_X);
                    let fine = fly::area_under(&f.path, LEDGE_X, last);
                    let riemann: i64 = f
                        .piles
                        .iter()
                        .map(|p| p.height as i64 * f.strips.width as i64 / Q)
                        .sum();
                    let count: i64 = f.piles.iter().map(|p| p.kernels as i64).sum();
                    let popcorn = count * KERNEL as i64 * KERNEL as i64 / Q;
                    f.done = Some((popcorn, riemann, fine));
                }
            }
        }
        if pops > 0 {
            self.sound("pop", 70);
        }
        if let Some(l) = self.lab.as_mut() {
            if flying && self.tick % 2 == 0 {
                l.trail.push(v3(pen.0, pen.1, BED_Z));
            }
        }
        let done_now = self
            .lab
            .as_ref()
            .and_then(|l| l.fly.as_ref())
            .map(|f| f.phase == FlyPhase::Done && flying)
            .unwrap_or(false);
        if done_now {
            if let Some(l) = self.lab.as_mut() {
                l.running = false;
            }
            let line = self.copy_str(&["lab", "fly_done_call"]);
            self.say("excellent", line, 6);
            self.crowd.cheer(ONE);
            self.sound("applause", 90);
        }
    }

    pub(crate) fn fly_draw(&self, b: &mut Batch) {
        let Some(f) = self.lab.as_ref().and_then(|l| l.fly.as_ref()) else {
            return;
        };
        // the ledge: a scaffold tower with a deck, and the mat past the bed
        let steel = rgb("#d1d5db");
        let deck = rgb("#a47148");
        let (x0, x1) = (LEDGE_X - ratio(16, 10), LEDGE_X);
        let h = f.jump.ledge;
        for (x, z) in [
            (x0, BED_Z - ONE / 2),
            (x1, BED_Z - ONE / 2),
            (x0, BED_Z + ONE / 2),
            (x1, BED_Z + ONE / 2),
        ] {
            b.rod(Mesh::Cyl, v3(x, 0, z), v3(x, h, z), ONE / 10, steel, 0);
        }
        let mut y = ONE;
        while y < h {
            b.rod(
                Mesh::Cube,
                v3(x0, y, BED_Z + ONE / 2),
                v3(x1, y, BED_Z + ONE / 2),
                ONE / 20,
                steel,
                0,
            );
            y += ONE;
        }
        b.shape(
            Mesh::Cube,
            v3((x0 + x1) / 2, h - ONE / 20, BED_Z),
            v3(x1 - x0 + ONE / 5, ONE / 10, int(1) + ONE / 5),
            deck,
        );
        b.shape(
            Mesh::Cube,
            v3(ratio(55, 10), ratio(15, 100), BED_Z),
            v3(int(4), ratio(3, 10), ratio(26, 10)),
            rgb("#3a86ff"),
        );
        // strip lines on the ground in front, and the piles' tray
        let tz = fly::tray_z();
        b.shape(
            Mesh::Cube,
            v3((LEDGE_X + END_X) / 2, ONE / 100, tz),
            v3(END_X - LEDGE_X, ONE / 50, ratio(4, 10)),
            rgb("#f1faee"),
        );
        for i in 0..=f.strips.count() {
            let x = f.strips.left(i);
            b.rod(
                Mesh::Cube,
                v3(x, 0, tz - ratio(2, 10)),
                v3(x, ratio(15, 100), tz - ratio(2, 10)),
                ONE / 50,
                rgb("#264653"),
                0,
            );
        }
        // the target curve, if matching one
        if let Some((_, e)) = &f.target {
            let mut prev: Option<V3> = None;
            for k in 0..=96 {
                let x = LEDGE_X + (END_X - LEDGE_X) * k / 96;
                let p = e.eval(x as i64).ok().map(|y| v3(x, y as i32, BED_Z - ONE / 20));
                if let (Some(a), Some(c)) = (prev, p) {
                    if a.y >= 0 && c.y >= 0 {
                        b.rod(Mesh::Cube, a, c, ratio(4, 100), rgb("#2ec4b6"), ONE * 2 / 3);
                    }
                }
                prev = p;
            }
        }
        // the popcorn
        let tick = self.tick;
        for k in &f.kernels {
            if tick >= k.released {
                b.push(
                    Mesh::Octa,
                    &render::boxed(k.at(tick), v3(KERNEL * 9 / 10, KERNEL * 9 / 10, KERNEL * 9 / 10)),
                    rgb("#fff3c4"),
                    ONE / 4,
                );
            }
        }
        // the bucket in the hand
        let hand = self.perf.body.pos[body::HAND_R];
        b.shape(
            Mesh::Cyl,
            hand.sub(v3(0, ratio(12, 100), 0)),
            v3(ratio(22, 100), ratio(26, 100), ratio(22, 100)),
            rgb("#e63946"),
        );
    }

    pub(crate) fn fly_json(&self) -> Value {
        let Some(f) = self.lab.as_ref().and_then(|l| l.fly.as_ref()) else {
            return Value::Null;
        };
        let lab = |k: &str| self.copy_str(&["lab", k]);
        let path = f
            .path
            .iter()
            .map(|(x, y)| format!("{},{}", gx(*x), gy(*y)))
            .collect::<Vec<_>>()
            .join(" ");
        let bars: Vec<Value> = f
            .piles
            .iter()
            .map(|p| {
                let x = f.strips.left(p.strip);
                // the pile's height: its rows of kernels
                let cols = f.strips.columns();
                let shown = (p.kernels + cols - 1) / cols * KERNEL;
                json!({"x": gx(x), "y": gy(shown), "w": pct((84 * Q * f.strips.width as i64 / (END_X - GX0) as i64) as i32), "h": pct((84 * Q * shown.clamp(0, GY1) as i64 / GY1 as i64) as i32)})
            })
            .collect();
        let target = f.target.as_ref().map(|(_, e)| {
            (0..=80)
                .filter_map(|k| {
                    let x = LEDGE_X + (END_X - LEDGE_X) * k / 80;
                    e.eval(x as i64)
                        .ok()
                        .filter(|y| *y >= 0 && *y <= GY1 as i64)
                        .map(|y| format!("{},{}", gx(x), gy(y as i32)))
                })
                .collect::<Vec<_>>()
                .join(" ")
        });
        let arcs: Vec<Value> = f
            .arcs
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let (standard, top) = fly::equation(a);
                json!({"n": i + 1, "standard": self.local(standard), "top": self.local(top)})
            })
            .collect();
        let ticks_x: Vec<Value> = (((GX0 / ONE) - 1)..=(END_X / ONE))
            .map(int)
            .filter(|x| *x >= GX0 && *x <= END_X)
            .map(|x| json!({"at": gx(x), "label": self.num(x as i64)}))
            .collect();
        let ticks_y: Vec<Value> = (0..=10)
            .step_by(2)
            .map(|m| json!({"at": gy(int(m)), "label": m.to_string()}))
            .collect();
        let count: i64 = f.piles.iter().map(|p| p.kernels as i64).sum();
        let summary = f.done.map(|(pop, riemann, fine)| {
            self.fill(
                &lab("fly_summary"),
                &[
                    ("kernels", count.to_string()),
                    ("popcorn", expr::show(pop)),
                    ("riemann", expr::show(riemann)),
                    ("fine", expr::show(fine)),
                    ("width", expr::show(f.strips.width as i64)),
                ],
            )
        });
        let log: Vec<String> = f
            .log
            .iter()
            .rev()
            .take(12)
            .rev()
            .map(|(k, vals)| {
                self.fill(
                    &self.copy_str(&["lab", "fly_log", k]),
                    &vals.iter().map(|(a, b)| (*a, b.clone())).collect::<Vec<_>>(),
                )
            })
            .collect();
        json!({
            // the sliders' values (always with a point), and as shown
            "ledge": expr::show(f.jump.ledge as i64),
            "speed": expr::show(f.jump.speed as i64),
            "ledge_shown": self.num(f.jump.ledge as i64),
            "speed_shown": self.num(f.jump.speed as i64),
            "angle": f.jump.angle.to_string(),
            "strip": (f.strips.width * 16 / ONE),
            "rule": f.strips.rule.index(),
            "phase": match f.phase { FlyPhase::Ready => "ready", FlyPhase::Flying => "flying", FlyPhase::Done => "done" },
            "kernels": count,
            "kernel_area": lab("fly_kernel"),
            "arcs": arcs,
            "summary": summary,
            "target": f.target.as_ref().map(|(s, _)| s.clone()),
            "log": log,
            "error": f.error,
            "graph": {
                "path": path,
                "bars": bars,
                "target": target,
                "x_axis": gy(0),
                "ticks_x": ticks_x,
                "ticks_y": ticks_y,
                "ledge": {"x": gx(LEDGE_X - ratio(16, 10)), "w": pct((84 * Q * ratio(16, 10) as i64 / (END_X - GX0) as i64) as i32), "y": gy(f.jump.ledge), "h": pct((84 * Q * f.jump.ledge as i64 / GY1 as i64) as i32)},
                "bed": {"x": gx(-crate::balance::BED_HALF_X), "w": pct((84 * Q * 2 * crate::balance::BED_HALF_X as i64 / (END_X - GX0) as i64) as i32), "y": gy(crate::balance::BED_Y)},
            },
            "version": f.path.len() / 4 + f.piles.len() * 1000 + f.arcs.len() * 100_000 + f.log.len() * 7,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::game::tests::game_with;
    use serde_json::Value;

    #[test]
    fn a_jump_off_the_ledge_drops_popcorn_that_adds_up_to_the_area_under_it() {
        let mut g = game_with("");
        g.lab_open();
        g.lab_kind(1);
        g.fly_set("6", "4", "25");
        g.fly_jump();
        let mut t = 0;
        for _ in 0..900 {
            t += 16_667;
            g.advance(t, 0, 0, 0);
        }
        let j: Value = serde_json::from_str(&g.lab_json()).unwrap();
        let f = &j["fly"];
        assert_eq!(f["phase"], "done", "{f}");
        // two arcs: off the ledge, then off the bed
        assert!(f["arcs"].as_array().unwrap().len() >= 2, "{}", f["arcs"]);
        let kernels = f["kernels"].as_i64().unwrap();
        assert!(kernels > 100, "{kernels}");
        // the popcorn, the Riemann sum and the fine area agree to within a
        // strip's worth (each 0.5 m strip is at most 0.5 × 8 m² off)
        let text = f["summary"].as_str().unwrap();
        assert!(text.contains(&kernels.to_string()), "{text}");
        let (pop, riemann, fine) = g.lab.as_ref().unwrap().fly.as_ref().unwrap().done.unwrap();
        let q = super::Q;
        // rounding each pile to whole kernels: at most half a kernel (1/32 m²)
        // a strip, over about 24 strips
        assert!((pop - riemann).abs() <= q, "popcorn {pop} vs Riemann {riemann}");
        // the middle rule on half-metre strips is close to the fine area
        assert!(
            (riemann - fine).abs() <= q * 3 / 2,
            "Riemann {riemann} vs fine {fine}"
        );
        assert!(fine > 20 * q, "{fine}");
    }

    #[test]
    fn matching_a_curve_sets_a_jump_that_draws_it() {
        let mut g = game_with("");
        g.lab_open();
        g.lab_kind(1);
        // a curve a jump really draws: 6 m, level-ish, falling to the bed
        let msg = g.fly_match("6 - 0.2(x + 4.6)^2");
        assert_eq!(msg, "");
        let j: Value = serde_json::from_str(&g.lab_json()).unwrap();
        assert_eq!(j["fly"]["ledge"], "6");
        assert!(!j["fly"]["log"].as_array().unwrap().is_empty());
    }
}
