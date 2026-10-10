//! The Juggling Lab: a pattern typed as numbers, checked, drawn as a beat
//! diagram, and juggled by the performer, each ball flying as high as its
//! number says. The rules are in siteswap.rs.

use crate::balance::GRAVITY;
use crate::body::{self, Pose};
use crate::expr;
use crate::fx::{ratio, HALF, ONE};
use crate::game::{juggle_spot, pct, Game};
use crate::geom::{v3, Affine, V3};
use crate::mesh::Mesh;
use crate::performer::{orientation, Performer, Phase};
use crate::render::{self, Batch};
use crate::siteswap::{self, Pattern, Problem, Throw};
use crate::tour::rgb;
use serde_json::{json, Value};

/// Ticks a beat: a three is up for 42 ticks and rises 0.6 m; a five 1.7 m.
pub const BEAT: i64 = 14;
const RUN_BEATS: usize = 2400;
const COLOURS: [&str; 10] = [
    "#ef476f", "#06d6a0", "#ffd166", "#118ab2", "#f77f00", "#8338ec", "#3a86ff", "#fb5607", "#2a9d8f",
    "#e63946",
];

pub struct JugLab {
    pub text: String,
    pub pattern: Option<Pattern>,
    pub balls: u32,
    pub problem: Option<Problem>,
    pub throws: Vec<Throw>,
    pub start: i64,
}

impl JugLab {
    pub fn new() -> JugLab {
        let mut j = JugLab {
            text: String::new(),
            pattern: None,
            balls: 0,
            problem: None,
            throws: Vec::new(),
            start: 0,
        };
        j.set("441");
        j
    }

    pub fn set(&mut self, src: &str) {
        self.text = src.trim().to_string();
        self.throws.clear();
        match siteswap::parse(src).and_then(|p| p.balls().map(|b| (p, b))) {
            Ok((p, b)) => {
                self.throws = siteswap::schedule(&p, b, RUN_BEATS);
                self.balls = b;
                self.pattern = Some(p);
                self.problem = None;
            }
            Err(e) => {
                self.pattern = siteswap::parse(src).ok();
                self.balls = 0;
                self.problem = Some(e);
            }
        }
    }

    /// Where each ball is at `tick`, given the two hands.
    pub fn balls_at(&self, tick: i64, hands: [V3; 2]) -> Vec<(usize, V3)> {
        let now = tick - self.start;
        if now < 0 {
            return Vec::new();
        }
        let mut last: Vec<Option<Throw>> = vec![None; self.balls as usize];
        for t in &self.throws {
            if (t.beat as i64) * BEAT > now {
                break;
            }
            if t.ball < last.len() {
                last[t.ball] = Some(*t);
            }
        }
        let mut out = Vec::new();
        for (ball, th) in last.iter().enumerate() {
            let Some(t) = th else { continue };
            let from = hands[siteswap::hand(t.beat)];
            let to = hands[siteswap::hand(t.beat + t.value as usize)];
            let flight = t.value as i64 * BEAT;
            let e = now - t.beat as i64 * BEAT;
            let p = if t.value <= 2 || e >= flight {
                // a 2 is held; a ball whose flight is over waits in the hand
                to
            } else {
                // up at g·T/2, so it comes back down after T
                let s = (e * ONE as i64 / flight) as i32;
                let along = from.lerp(to, s);
                let v0 = GRAVITY as i64 * flight / 2;
                let rise = v0 * e - GRAVITY as i64 * e * e / 2;
                along.add(v3(0, rise as i32, 0))
            };
            out.push((ball, p));
        }
        out
    }
}

impl Default for JugLab {
    fn default() -> Self {
        JugLab::new()
    }
}

impl Game {
    pub(crate) fn jug_reset(&mut self) {
        let mut p = Performer::new(&self.def, juggle_spot(), HALF);
        p.phase = Phase::Held;
        p.held_pose = Pose::Juggle;
        p.hold_frame = Some(Affine::translate(p.com).then(&orientation(HALF, 0, 0)));
        let frame = p.frame();
        p.body.place(&p.shape, &frame);
        p.body.prev = p.body.pos;
        self.perf = p;
        let tick = self.tick;
        if let Some(l) = self.lab.as_mut() {
            l.trail.clear();
            l.running = true;
            if let Some(j) = l.jug.as_mut() {
                j.start = tick + 30;
            }
        }
    }

    /// A new pattern, as typed.
    pub fn jug_pattern(&mut self, src: &str) {
        if let Some(j) = self.lab.as_mut().and_then(|l| l.jug.as_mut()) {
            j.set(src);
        }
        self.jug_reset();
    }

    pub(crate) fn jug_step(&mut self) {
        // the performer stands and juggles; nothing here waits on a key
        let _ = self.perf.step(&self.def, &Default::default());
        let Some(j) = self.lab.as_ref().and_then(|l| l.jug.as_ref()) else {
            return;
        };
        let now = self.tick - j.start;
        if now >= 0 && now % BEAT == 0 && j.problem.is_none() {
            let beat = (now / BEAT) as usize;
            if j.throws.iter().any(|t| t.beat == beat && t.value > 2) {
                self.sound("catch", 35);
            }
        }
    }

    fn hands(&self) -> [V3; 2] {
        [self.perf.body.pos[body::HAND_R], self.perf.body.pos[body::HAND_L]]
    }

    pub(crate) fn jug_draw(&self, b: &mut Batch) {
        let Some(j) = self.lab.as_ref().and_then(|l| l.jug.as_ref()) else {
            return;
        };
        for (ball, p) in j.balls_at(self.tick, self.hands()) {
            let c = rgb(COLOURS[ball % COLOURS.len()]);
            b.push(
                Mesh::Sphere,
                &render::boxed(p, v3(ratio(16, 100), ratio(16, 100), ratio(16, 100))),
                c,
                ONE / 5,
            );
        }
    }

    pub(crate) fn jug_json(&self) -> Value {
        let Some(j) = self.lab.as_ref().and_then(|l| l.jug.as_ref()) else {
            return Value::Null;
        };
        let lab = |k: &str| self.copy_str(&["lab", k]);
        let explain = match &j.problem {
            None => {
                let p = j.pattern.as_ref().expect("a valid pattern");
                let digits: Vec<String> = p.throws.iter().map(|t| t.to_string()).collect();
                self.fill(
                    &lab("jug_ok"),
                    &[
                        ("digits", digits.join(" + ")),
                        ("sum", p.sum().to_string()),
                        ("n", p.period().to_string()),
                        ("balls", j.balls.to_string()),
                    ],
                )
            }
            Some(Problem::Empty) => lab("jug_empty"),
            Some(Problem::NotDigit { at, ch }) => {
                self.fill(&lab("jug_digit"), &[("at", at.to_string()), ("ch", ch.clone())])
            }
            Some(Problem::Average { sum, n }) => self.fill(
                &lab("jug_average"),
                &[
                    ("sum", sum.to_string()),
                    ("n", n.to_string()),
                    ("avg", expr::show(*sum as i64 * ONE as i64 / *n as i64)),
                ],
            ),
            Some(Problem::Collision { a, b, lands }) => self.fill(
                &lab("jug_collision"),
                &[
                    ("a", a.to_string()),
                    ("b", b.to_string()),
                    ("lands", lands.to_string()),
                ],
            ),
        };
        // the beat diagram: beats along a line, an arc from each throw to
        // where it lands, as high as the throw, coloured by ball
        let mut beats = Vec::new();
        let mut arcs = Vec::new();
        if let Some(p) = &j.pattern {
            let shown = (p.period() * 3).clamp(6, 18);
            let x = |b: usize| pct((6 * ONE as i64 + 88 * ONE as i64 * b as i64 / (shown + 9) as i64) as i32);
            let line = 82;
            for b in 0..shown {
                beats.push(json!({"x": x(b), "label": p.at(b).to_string(), "hand": siteswap::hand(b)}));
            }
            let collide = match &j.problem {
                Some(Problem::Collision { lands, .. }) => Some(*lands - 1),
                _ => None,
            };
            let throws: Vec<Throw> = if j.problem.is_none() {
                j.throws.iter().filter(|t| t.beat < shown).copied().collect()
            } else {
                (0..shown)
                    .map(|b| Throw {
                        beat: b,
                        value: p.at(b),
                        ball: b % p.period(),
                    })
                    .collect()
            };
            for t in throws.iter().filter(|t| t.value > 0) {
                let (a, e) = (t.beat, t.beat + t.value as usize);
                let mid_x = x(a);
                let end_x = x(e);
                let height = (line - 15 * t.value as i64).max(2);
                let bad =
                    collide.map(|c| e % p.period() == c % p.period()).unwrap_or(false) && j.problem.is_some();
                arcs.push(json!({
                    "d": format!("M {mid_x} {line} Q {} {height} {end_x} {line}", pct((6 * ONE as i64 + 88 * ONE as i64 * (a + e) as i64 / 2 / (shown + 9) as i64) as i32)),
                    "colour": if bad { "#e63946".to_string() } else { COLOURS[t.ball % COLOURS.len()].to_string() },
                    "bad": bad,
                }));
            }
        }
        json!({
            "text": j.text,
            "balls": j.balls,
            "valid": j.problem.is_none(),
            "explain": explain,
            "beats": beats,
            "arcs": arcs,
            "presets": self.copy["lab"]["jug_presets"],
            "version": j.text.len() * 100 + j.balls as usize,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::game::tests::game_with;
    use serde_json::Value;

    #[test]
    fn the_lab_juggles_a_valid_pattern_and_explains_a_broken_one() {
        let mut g = game_with("");
        g.lab_open();
        g.lab_kind(2);
        g.jug_pattern("531");
        let j: Value = serde_json::from_str(&g.lab_json()).unwrap();
        assert_eq!(j["jug"]["balls"], 3);
        assert!(j["jug"]["explain"].as_str().unwrap().contains("5 + 3 + 1"));
        let mut t = 0;
        for _ in 0..240 {
            t += 16_667;
            g.advance(t, 0, 0, 0);
        }
        assert!(!g.frame_draws().1.is_empty());
        g.jug_pattern("432");
        let j: Value = serde_json::from_str(&g.lab_json()).unwrap();
        assert_eq!(j["jug"]["valid"], false);
        let why = j["jug"]["explain"].as_str().unwrap();
        assert!(
            why.contains('1') && why.contains('2') && why.contains('5'),
            "{why}"
        );
    }

    #[test]
    fn a_five_goes_higher_than_a_three() {
        use super::JugLab;
        use crate::geom::v3;
        let mut j = JugLab::new();
        j.set("53");
        j.start = 0;
        let hands = [v3(0, 65536, 0), v3(20000, 65536, 0)];
        // half way through the first throw (a 5, 70 ticks), and the second (a 3)
        let five = j.balls_at(35, hands).iter().map(|(_, p)| p.y).max().unwrap();
        let three = j
            .balls_at(14 + 21, hands)
            .iter()
            .filter(|(b, _)| *b == 1)
            .map(|(_, p)| p.y)
            .max()
            .unwrap();
        assert!(five > three, "{five} vs {three}");
    }
}
