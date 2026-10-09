//! The door the shim calls through. Build a game from the data files; each
//! display frame hand it the clock, the held keys and the mouse movement;
//! read back what to draw, the HUD and the tour map.
//!
//! The game is in one mode at a time: the tour map (the venue behind it, a
//! performer bouncing on her own), a show in play, paused, or the result.
//! The simulation runs at a fixed 60 ticks a second; `advance` runs as many
//! ticks as the elapsed time owes, and the performer is drawn between the
//! last two ticks.

use crate::acts::{
    self, Cloth, Juggle, JuggleEvent, Popcorn, Tower, TowerEvent, TowerPhase, Trapeze, CLOTH_X, CLOTH_Z,
};
use crate::balance::{self, BED_HALF_Z, BED_Y, BED_Z, CAM_PITCH, CAM_YAW, FLIP_MAX, TICKS_PER_SECOND};
use crate::body::{self, BodyDef, Pose, POINTS};
use crate::crowd::Crowd;
use crate::fx::{self, deg, int, ratio, HALF, ONE, QUARTER};
use crate::geom::{facing, v3, Affine, V3};
use crate::mesh::{self, Mesh};
use crate::mission::{Outcome, Run, Signal};
use crate::performer::{orientation, Controls, Event, Performer, Phase, Posture};
use crate::render::{self, Batch, Camera, Paint};
use crate::rng::Rng;
use crate::scene::{self, Fittings};
use crate::stage::{self, Surface};
use crate::tour::{rgb, Goal, Progress, Tour};
use crate::trick::TrickBook;
use serde::Serialize;
use serde_json::{json, Value};

/// The actions a key or button can hold, and the bit each sets.
pub const ACTIONS: &[(&str, u32)] = &[
    ("front", 1 << 0),
    ("back", 1 << 1),
    ("left", 1 << 2),
    ("right", 1 << 3),
    ("drive", 1 << 4),
    ("kill", 1 << 5),
    ("tuck", 1 << 6),
    ("pike", 1 << 7),
    ("up", 1 << 8),
    ("down", 1 << 9),
    ("turn_l", 1 << 10),
    ("turn_r", 1 << 11),
    ("act", 1 << 12),
    ("restart", 1 << 13),
    // set while the mouse is dragged over the view
    ("drag", 1 << 14),
];

fn bit(name: &str) -> u32 {
    ACTIONS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, b)| *b)
        .unwrap_or(0)
}

/// Angle units of camera turn per hundredth of a pixel dragged.
const LOOK_PER_HUNDREDTH: i32 = 2;
const TICK_UNITS: i64 = 1_000_000;
pub const MAX_TICKS_PER_FRAME: u32 = 6;
const MAX_FRAME_US: i64 = 250_000;
const MAX_PIXELS: i64 = 2560 * 1440;
/// How long a finished show runs on before the result shows.
const ENCORE_TICKS: i32 = 100;
/// How long the shouting lasts.
const SHOUT_TICKS: i64 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Map,
    Play,
    Paused,
    Result,
    /// The Graph Lab (lab.rs).
    Lab,
}

#[derive(Debug, Clone, Serialize)]
pub struct Callout {
    pub seq: u32,
    /// The word's id (excellent, oh_no …): the sound and the style.
    pub word: String,
    /// What is shown big.
    pub shown: String,
    /// The line under it: the trick's name.
    pub text: String,
    #[serde(skip)]
    pub tick: i64,
    #[serde(skip)]
    pub shouters: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sfx {
    pub seq: u32,
    pub kind: &'static str,
    /// Loudness, 0 to 100.
    pub level: i32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResultInfo {
    passed: bool,
    grade: i32,
    word: String,
    shown: String,
    unlocked: String,
    next: Option<usize>,
}

pub(crate) enum Act {
    None,
    Popcorn(Popcorn),
    Tower(Tower),
    /// The trapeze, and ticks since the performer last let go.
    Trapeze(Trapeze, i32),
    Juggle(Juggle),
}

pub struct Game {
    pub(crate) tour: Tour,
    pub(crate) def: BodyDef,
    pub(crate) book: TrickBook,
    pub(crate) copy: Value,
    pub(crate) progress: Progress,
    pub(crate) mode: Mode,
    pub(crate) selected: usize,
    pub(crate) fit: Fittings,
    pub(crate) venue: usize,
    pub(crate) perf: Performer,
    pub(crate) run: Run,
    pub(crate) act: Act,
    pub(crate) cloth: Cloth,
    pub(crate) crowd: Crowd,
    pub(crate) rng: Rng,
    pub(crate) tick: i64,
    pub(crate) acc: i64,
    pub(crate) last_us: Option<i64>,
    pub(crate) tick_bits: u32,
    pub(crate) latched: u32,
    pub(crate) frame_bits: u32,
    pub(crate) cam_yaw: i32,
    pub(crate) cam_pitch: i32,
    pub(crate) cam: Camera,
    pub(crate) cam_target: V3,
    pub(crate) orbit: i32,
    pub(crate) alpha: i32,
    pub(crate) callouts: Vec<Callout>,
    pub(crate) sfx: Vec<Sfx>,
    pub(crate) seq: u32,
    pub(crate) scene: (Vec<i32>, Vec<u32>),
    pub(crate) scene_version: u32,
    pub(crate) aspect: i32,
    pub(crate) css: (i32, i32),
    pub(crate) pixels: (i32, i32),
    pub(crate) end_in: i32,
    pub(crate) result: Option<ResultInfo>,
    pub(crate) peak: i32,
    pub(crate) demo_n: i32,
    pub(crate) demo_peak: i32,
    pub(crate) frame: (Vec<i32>, Vec<u32>),
    pub(crate) lab: Option<crate::lab::LabState>,
}

pub(crate) fn fill(template: &str, pairs: &[(&str, String)]) -> String {
    let mut s = template.to_string();
    for (k, v) in pairs {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

/// A Q16 percentage as CSS writes it, to two places.
pub fn pct(q: i32) -> String {
    let hundredths = (q as i64 * 100 / ONE as i64) as i32;
    format!(
        "{}{}.{:02}",
        if hundredths < 0 { "-" } else { "" },
        hundredths.abs() / 100,
        hundredths.abs() % 100
    )
}

/// Metres with one decimal.
pub fn metres(q: i32) -> String {
    let tenths = (q.max(0) as i64 * 10 / ONE as i64) as i32;
    format!("{}.{}", tenths / 10, tenths % 10)
}

pub fn feet(q: i32) -> String {
    format!("{}", (q.max(0) as i64 * 3281 / 1000 / ONE as i64) as i32)
}

impl Game {
    pub fn new(
        body: &str,
        tricks: &str,
        venues: &str,
        missions: &str,
        costumes: &str,
        copy: &str,
        saved: &str,
    ) -> Result<Game, String> {
        let tour = Tour::parse(venues, missions, costumes)?;
        let def = BodyDef::parse(body)?;
        let book = TrickBook::parse(tricks)?;
        let copy: Value = serde_json::from_str(copy).map_err(|e| format!("copy.json: {e}"))?;
        let mut progress = Progress::load(saved);
        if progress.costume.is_empty() || !tour.costumes.iter().any(|c| c.id == progress.costume) {
            progress.costume = tour.costumes[0].id.clone();
        }
        let selected = (0..tour.missions.len())
            .find(|&i| progress.open(&tour, i) && !progress.passed(&tour.missions[i].id))
            .unwrap_or(0);
        let perf = Performer::on_bed(&def, 0);
        let run = Run::new(tour.missions[selected].goal.clone());
        let crowd = Crowd::new(&tour.venues[0]);
        let mut g = Game {
            tour,
            def,
            book,
            copy,
            progress,
            mode: Mode::Map,
            selected,
            fit: Fittings::default(),
            venue: 0,
            perf,
            run,
            act: Act::None,
            cloth: Cloth::default(),
            crowd,
            rng: Rng::new(2026),
            tick: 0,
            acc: 0,
            last_us: None,
            tick_bits: 0,
            latched: 0,
            frame_bits: 0,
            cam_yaw: 0,
            cam_pitch: 0,
            cam: Camera {
                eye: v3(0, int(6), int(18)),
                target: v3(0, int(2), 0),
            },
            cam_target: v3(0, int(2), 0),
            orbit: 0,
            alpha: 0,
            callouts: Vec::new(),
            sfx: Vec::new(),
            seq: 0,
            scene: (Vec::new(), Vec::new()),
            scene_version: 0,
            aspect: ONE * 16 / 9,
            css: (1280, 720),
            pixels: (1280, 720),
            end_in: 0,
            result: None,
            peak: 0,
            demo_n: 0,
            demo_peak: 0,
            frame: (Vec::new(), Vec::new()),
            lab: None,
        };
        g.select(selected);
        Ok(g)
    }

    pub(crate) fn copy_str(&self, path: &[&str]) -> String {
        let mut v = &self.copy;
        for p in path {
            v = &v[*p];
        }
        v.as_str().unwrap_or("").to_string()
    }

    // ---- the page's calls -------------------------------------------------

    pub fn resize(&mut self, css_w: i32, css_h: i32, dpr_pct: i32) -> [i32; 2] {
        let (cw, ch) = (css_w.max(1), css_h.max(1));
        let mut w = cw as i64 * dpr_pct.clamp(50, 300) as i64 / 100;
        let mut h = ch as i64 * dpr_pct.clamp(50, 300) as i64 / 100;
        if w * h > MAX_PIXELS {
            let k = fx::isqrt((MAX_PIXELS * 1_000_000 / (w * h)) as u64) as i64;
            w = w * k / 1000;
            h = h * k / 1000;
        }
        self.css = (cw, ch);
        self.pixels = (w.max(1) as i32, h.max(1) as i32);
        self.aspect = (cw as i64 * ONE as i64 / ch as i64) as i32;
        [self.pixels.0, self.pixels.1]
    }

    /// One display frame: the clock in microseconds, the held actions, the
    /// mouse movement since the last frame in hundredths of a CSS pixel.
    pub fn advance(&mut self, now_us: i64, bits: u32, dx: i32, dy: i32) {
        let dt = match self.last_us {
            Some(last) => (now_us - last).clamp(0, MAX_FRAME_US),
            None => 0,
        };
        self.last_us = Some(now_us);
        self.latched |= bits & !self.frame_bits;
        self.frame_bits = bits;
        if bits & bit("drag") != 0 {
            self.cam_yaw -= dx * LOOK_PER_HUNDREDTH;
            self.cam_pitch = (self.cam_pitch - dy * LOOK_PER_HUNDREDTH).clamp(-deg(30), deg(40));
        }
        if self.mode == Mode::Paused {
            return;
        }
        self.acc += dt * TICKS_PER_SECOND as i64 / self.time_scale();
        let mut n = 0;
        while self.acc >= TICK_UNITS && n < MAX_TICKS_PER_FRAME {
            let held = bits | self.latched;
            self.latched = 0;
            self.step(held);
            self.acc -= TICK_UNITS;
            n += 1;
        }
        if n == MAX_TICKS_PER_FRAME {
            self.acc = 0;
        }
        self.alpha = (self.acc * ONE as i64 / TICK_UNITS) as i32;
    }

    pub fn select(&mut self, i: usize) {
        let i = i.min(self.tour.missions.len() - 1);
        self.selected = i;
        let venue = self.tour.venue_index(&self.tour.missions[i].venue);
        self.set_stage(venue, Fittings::default());
        self.mode = Mode::Map;
        self.act = Act::None;
        self.perf = Performer::on_bed(&self.def, 0);
        self.result = None;
        self.run = Run::new(self.tour.missions[i].goal.clone());
    }

    pub(crate) fn set_stage(&mut self, venue: usize, fit: Fittings) {
        if venue != self.venue || fit != self.fit || self.scene_version == 0 {
            self.venue = venue;
            self.fit = fit;
            self.scene = scene::build(&self.tour.venues[venue], fit).finish(render::STATIC_BUFFER);
            self.crowd = Crowd::new(&self.tour.venues[venue]);
            // the people who were shouting belong to the last venue's crowd
            for c in self.callouts.iter_mut() {
                c.shouters.clear();
            }
            self.scene_version += 1;
        }
    }

    /// Starts the selected show, if it is open.
    pub fn start(&mut self) {
        let i = self.selected;
        if !self.progress.open(&self.tour, i) {
            return;
        }
        let goal = self.tour.missions[i].goal.clone();
        let fit = Fittings {
            trapeze: matches!(goal, Goal::Trapeze { .. }),
            tower: matches!(goal, Goal::Tower { .. }),
        };
        let venue = self.tour.venue_index(&self.tour.missions[i].venue);
        self.set_stage(venue, fit);
        self.act = match &goal {
            Goal::Popcorn { .. } => Act::Popcorn(Popcorn::default()),
            Goal::Tower { .. } => Act::Tower(Tower::new()),
            Goal::Trapeze { .. } => Act::Trapeze(Trapeze::new(), 0),
            Goal::Juggle { balls, .. } => Act::Juggle(Juggle::new(*balls as usize)),
            _ => Act::None,
        };
        self.perf = match &self.act {
            Act::Juggle(_) => Performer::new(&self.def, juggle_spot(), HALF),
            _ => Performer::on_bed(&self.def, 0),
        };
        self.run = Run::new(goal);
        self.cloth = Cloth::default();
        self.mode = Mode::Play;
        self.end_in = 0;
        self.result = None;
        self.peak = 0;
        self.cam_yaw = 0;
        self.cam_pitch = 0;
        self.hold_act();
        let frame = self.perf.frame();
        self.perf.body.place(&self.perf.shape, &frame);
        self.perf.body.prev = self.perf.body.pos;
        self.snap_camera();
    }

    pub fn pause(&mut self) {
        if self.mode == Mode::Play {
            self.mode = Mode::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.mode == Mode::Paused {
            self.mode = Mode::Play;
            self.last_us = None;
        }
    }

    pub fn to_map(&mut self) {
        let i = self.selected;
        self.select(i);
    }

    pub fn wear(&mut self, i: usize) {
        if let Some(c) = self.tour.costumes.get(i) {
            if self.progress.unlocked(&self.tour, &c.id) {
                self.progress.costume = c.id.clone();
            }
        }
    }

    pub fn saved(&self) -> String {
        self.progress.save()
    }

    pub fn reset(&mut self) {
        self.progress = Progress::load("");
        self.progress.costume = self.tour.costumes[0].id.clone();
        self.select(0);
    }

    /// Marks every show passed, for trying things out.
    pub fn open_everything(&mut self) {
        for m in &self.tour.missions {
            if !self.progress.passed(&m.id) {
                self.progress.record(&m.id, 0);
            }
        }
    }

    // ---- the simulation ---------------------------------------------------

    fn controls(&self, bits: u32, edges: u32) -> Controls {
        let on = |n: &str| bits & bit(n) != 0;
        let tap = |n: &str| edges & bit(n) != 0;
        let one = |a: bool, b: bool| a as i32 - b as i32;
        let standing = matches!(self.perf.phase, Phase::Stand(_));
        Controls {
            flip: if standing { 0 } else { one(on("back"), on("front")) },
            twist: if standing { 0 } else { one(on("right"), on("left")) },
            drive: on("drive"),
            kill: on("kill"),
            tuck: on("tuck"),
            pike: on("pike"),
            travel: one(on("up"), on("down")) + if standing { one(on("front"), on("back")) } else { 0 },
            turn: one(on("turn_r"), on("turn_l")) + if standing { one(on("right"), on("left")) } else { 0 },
            act: tap("act") || tap("drive"),
            jump: tap("drive"),
        }
    }

    /// A bot for the map screen: bounce to about three metres and throw a
    /// back tuck every third bounce, opening out at three quarters.
    fn demo(&self) -> Controls {
        let p = &self.perf;
        match p.phase {
            Phase::Bed(Posture::Feet) => Controls {
                drive: self.demo_peak < int(3),
                flip: (p.vel.y >= 0 && self.demo_peak > ratio(26, 10) && self.demo_n % 3 == 2) as i32,
                ..Controls::default()
            },
            Phase::Air => Controls {
                tuck: (p.flip - p.flight.flip0).abs() < 3 * QUARTER && p.flip_l != 0,
                ..Controls::default()
            },
            _ => Controls::default(),
        }
    }

    pub(crate) fn say(&mut self, word: &str, text: String, shout: usize) {
        self.seq += 1;
        let until = self.tick + SHOUT_TICKS;
        let shouters = self.crowd.shouters(&mut self.rng, shout, until);
        let shown = self.copy_str(&["judge", word]);
        self.callouts.push(Callout {
            seq: self.seq,
            word: word.to_string(),
            shown,
            text,
            tick: self.tick,
            shouters,
        });
        if self.callouts.len() > 6 {
            self.callouts.remove(0);
        }
    }

    pub(crate) fn sound(&mut self, kind: &'static str, level: i32) {
        self.seq += 1;
        self.sfx.push(Sfx {
            seq: self.seq,
            kind,
            level: level.clamp(0, 100),
        });
        if self.sfx.len() > 12 {
            self.sfx.remove(0);
        }
    }

    /// Puts the performer where an act holds her this tick.
    fn hold_act(&mut self) {
        let def = &self.def;
        match &self.act {
            Act::Tower(t) => {
                if matches!(t.phase, TowerPhase::Fallen(_)) || self.perf.bailed() {
                    return;
                }
                let [tx, tz] = t.tilt();
                let lean = Affine::rot_z(-tx).then(&Affine::rot_x(tz));
                let top = acts::tower_base().add(lean.apply_dir(v3(0, t.height(), 0)));
                let hand = matches!(t.phase, TowerPhase::Handstand(_) | TowerPhase::Done);
                let (pose, flip) = if hand {
                    (Pose::Layout, HALF)
                } else {
                    (Pose::Stand, 0)
                };
                let up = Performer::reach(def, pose, flip);
                let com = top.add(lean.apply_dir(v3(0, up, 0)));
                self.perf.phase = Phase::Held;
                self.perf.held_pose = pose;
                self.perf.com = com;
                self.perf.hold_frame =
                    Some(Affine::translate(com).then(&lean).then(&orientation(0, flip, 0)));
            }
            Act::Trapeze(t, _) if t.holding => {
                let com = t.com();
                let pose = if self.tick_bits & bit("front") != 0 {
                    Pose::Pike
                } else if self.tick_bits & bit("back") != 0 {
                    Pose::Arch
                } else {
                    Pose::Layout
                };
                self.perf.phase = Phase::Held;
                self.perf.held_pose = pose;
                self.perf.com = com;
                self.perf.heading = -QUARTER;
                self.perf.flip = t.angle();
                self.perf.twist = 0;
                self.perf.hold_frame =
                    Some(Affine::translate(com).then(&orientation(-QUARTER, t.angle(), 0)));
            }
            Act::Juggle(_) => {
                let com = self.perf.com;
                self.perf.phase = Phase::Held;
                self.perf.held_pose = Pose::Juggle;
                self.perf.hold_frame = Some(Affine::translate(com).then(&orientation(HALF, 0, 0)));
            }
            _ => {}
        }
    }

    pub(crate) fn step(&mut self, bits: u32) {
        if self.mode == Mode::Lab {
            self.lab_step(bits);
            self.tick += 1;
            self.follow();
            return;
        }
        let edges = bits & !self.tick_bits;
        self.tick_bits = bits;
        let playing = self.mode == Mode::Play;
        if playing && edges & bit("restart") != 0 {
            self.start();
            return;
        }
        let c = if playing && self.end_in == 0 {
            self.controls(bits, edges)
        } else if self.mode == Mode::Map {
            self.demo()
        } else {
            Controls::default()
        };
        let mut signals = Vec::new();
        let on = |n: &str| bits & bit(n) != 0;
        let one = |a: bool, b: bool| a as i32 - b as i32;

        // the acts that hold the performer move first
        match &mut self.act {
            Act::Tower(t) if playing && !self.perf.bailed() => {
                let push = [
                    one(on("right") || on("turn_r"), on("left") || on("turn_l")),
                    one(on("back") || on("down"), on("front") || on("up")),
                ];
                let (chairs, hold) = match &self.run.goal {
                    Goal::Tower { chairs, hold } => (*chairs, hold * TICKS_PER_SECOND),
                    _ => (4, 120),
                };
                let act = c.act && self.end_in == 0;
                let mut fell = false;
                for e in t.step(&mut self.rng, push, act, chairs, hold) {
                    match e {
                        TowerEvent::Placed(g) => signals.push(Signal::ChairPlaced(g)),
                        TowerEvent::Handstand => signals.push(Signal::Handstand),
                        TowerEvent::Held => signals.push(Signal::TowerHeld(t.grade())),
                        TowerEvent::Fell => {
                            signals.push(Signal::TowerFell);
                            fell = true;
                        }
                    }
                }
                if fell {
                    let mut ev = Vec::new();
                    self.perf.vel = v3(t.spin[0] / acts::FINE / 4, 0, t.spin[1] / acts::FINE / 4);
                    self.perf.crash(&self.def, &mut ev);
                }
            }
            Act::Tower(t) => {
                // the tower goes on falling after the show ends
                t.step(&mut self.rng, [0, 0], false, 99, 99);
            }
            Act::Trapeze(t, since) => {
                if t.holding {
                    let pump = one(on("front"), on("back"));
                    t.step(pump);
                    if c.act && playing {
                        t.holding = false;
                        *since = 0;
                        let spin = t.spin() + c.flip * FLIP_MAX / 3;
                        let (vel, com) = (t.velocity(), t.com());
                        self.perf.phase = Phase::Air;
                        self.perf.com = com;
                        self.perf.hold_frame = None;
                        self.perf.release(vel, spin);
                    }
                } else {
                    t.step(0);
                    *since += 1;
                    let resting = !matches!(self.perf.phase, Phase::Air);
                    if *since > 150 && resting && self.end_in == 0 {
                        *t = Trapeze::new();
                    }
                }
            }
            _ => {}
        }
        self.hold_act();

        let events = self.perf.step(&self.def, &c);
        let mut cheer = Vec::new();
        for e in events {
            match e {
                Event::Landed(l) => {
                    let named = self.book.name(&l);
                    let grade = self.book.grade(&l);
                    if l.surface == Surface::Bed {
                        self.sound("boing", 40 + l.height * 40 / int(7));
                    } else {
                        self.sound("thud", 60);
                    }
                    signals.push(Signal::Landed {
                        trick: named.as_ref().map(|n| (n.id.clone(), n.value)),
                        grade,
                        surface: l.surface,
                        trapeze: l.flight.trapeze,
                        feet: l.to == Posture::Feet,
                    });
                    if let Some(n) = named.filter(|n| n.value > 0) {
                        cheer.push((grade, n.name, n.value));
                    } else if l.flight.trapeze && l.to == Posture::Feet {
                        cheer.push((grade, String::new(), 1));
                    }
                }
                Event::Crashed(_) => {
                    signals.push(Signal::Crashed);
                    self.sound("crash", 80);
                    let w = self.book.flair("oh_no".into(), &mut self.rng);
                    self.say(&w, String::new(), 4);
                    self.crowd.cheer(ONE / 3);
                }
                Event::Peak(h) => {
                    signals.push(Signal::Peak(h));
                    self.peak = self.peak.max(h);
                    if self.mode == Mode::Map {
                        self.demo_peak = h;
                        self.demo_n += 1;
                    }
                    if let Goal::Height { metres: m, .. } = self.run.goal {
                        if playing && h >= int(m) {
                            let t = fill(&self.copy_str(&["height_called"]), &[("metres", metres(h))]);
                            let w = self.book.word(80 + (h - int(m)) * 20 / ONE, &mut self.rng);
                            self.say(&w, t, 2);
                        }
                    }
                }
                Event::Takeoff => {
                    if matches!(self.perf.flight.from_surface, Surface::Bed) {
                        self.sound("spring", 30);
                    }
                }
            }
        }
        for (grade, name, value) in cheer {
            if playing {
                let w = self.book.word(grade, &mut self.rng);
                self.say(&w, name, 2 + (value / 20) as usize);
            }
            self.crowd.cheer(ONE * grade / 100 / 2 + value * ONE / 200);
        }

        // the acts that run beside the performer
        match &mut self.act {
            Act::Popcorn(p) => {
                let n = p.step(&mut self.rng, &self.perf.body.pos);
                if n > 0 && playing {
                    signals.push(Signal::Popcorn(n));
                    self.seq += 1;
                    self.sfx.push(Sfx {
                        seq: self.seq,
                        kind: "pop",
                        level: 60,
                    });
                }
            }
            Act::Juggle(j) if playing => {
                // A is the hand on the screen's left: the performer faces the
                // crowd, so that is her right hand
                let press = [edges & bit("left") != 0, edges & bit("right") != 0];
                let mut said = Vec::new();
                for e in j.step(press) {
                    match e {
                        JuggleEvent::Caught(g) => {
                            signals.push(Signal::Caught(g));
                            said.push((Some(g), j.streak));
                        }
                        JuggleEvent::Dropped => {
                            signals.push(Signal::Dropped);
                            said.push((None, 0));
                        }
                    }
                }
                for (g, streak) in said {
                    match g {
                        Some(g) => {
                            self.sound("catch", 50);
                            if streak % 5 == 0 || g >= 94 {
                                let w = self.book.word(g, &mut self.rng);
                                self.say(&w, String::new(), 1);
                            }
                            self.crowd.cheer(ONE / 30);
                        }
                        None => {
                            self.sound("drop", 60);
                            let w = self.book.flair("oh_no".into(), &mut self.rng);
                            self.say(&w, String::new(), 3);
                        }
                    }
                }
            }
            _ => {}
        }

        // tower callouts and sounds
        for s in &signals {
            match s {
                Signal::ChairPlaced(g) => {
                    self.sound("chair", 50);
                    let w = self.book.word(*g, &mut self.rng);
                    self.say(&w, String::new(), 2);
                    self.crowd.cheer(ONE / 6);
                }
                Signal::TowerFell => {
                    self.sound("crash", 100);
                    let w = self.book.flair("oh_no".into(), &mut self.rng);
                    self.say(&w, String::new(), 6);
                }
                _ => {}
            }
        }

        // the bed's cloth, pressed where the performer meets it
        let lowest = self.perf.body.lowest(&self.def);
        let centre = self.perf.body.centre(&self.def);
        let press = if lowest < BED_Y + ONE / 20 && stage::over_bed(centre.x, centre.z) {
            Some((centre.x, centre.z, (BED_Y - lowest).max(0)))
        } else {
            None
        };
        self.cloth.step(press);
        self.crowd.tick();

        if playing {
            self.run.tick();
            for s in &signals {
                self.run.hear(s);
            }
            if self.run.outcome.is_some() && self.end_in == 0 {
                self.end_in = ENCORE_TICKS;
                let o = self.run.outcome.unwrap();
                if o.passed {
                    self.crowd.cheer(ONE);
                    self.sound("applause", 100);
                }
            }
            if self.end_in > 0 {
                self.end_in -= 1;
                if self.end_in == 0 {
                    self.finish();
                }
            }
        }
        self.tick += 1;
        self.follow();
    }

    fn finish(&mut self) {
        let o: Outcome = self.run.outcome.unwrap_or(Outcome {
            passed: false,
            grade: 0,
        });
        let m = &self.tour.missions[self.selected];
        let id = m.id.clone();
        let unlock = m.unlock.clone();
        let had = !unlock.is_empty() && self.progress.unlocked(&self.tour, &unlock);
        if o.passed {
            self.progress.record(&id, o.grade);
        }
        let unlocked = if o.passed && !unlock.is_empty() && !had {
            self.tour.costume(&unlock).name.clone()
        } else {
            String::new()
        };
        let word = if o.passed {
            self.book.plain_word(o.grade)
        } else {
            self.book.flair("oh_no".into(), &mut self.rng)
        };
        let next = (0..self.tour.missions.len()).find(|&i| {
            i != self.selected
                && self.progress.open(&self.tour, i)
                && !self.progress.passed(&self.tour.missions[i].id)
        });
        let shown = self.copy_str(&["judge", &word]);
        self.say(&word, String::new(), 8);
        self.result = Some(ResultInfo {
            passed: o.passed,
            grade: o.grade,
            word,
            shown,
            unlocked,
            next,
        });
        self.mode = Mode::Result;
    }

    // ---- the camera -------------------------------------------------------

    fn camera_goal(&self) -> (V3, i32, i32, i32) {
        if self.mode == Mode::Lab {
            return (
                v3(0, ratio(29, 10), BED_Z),
                self.cam_yaw,
                -deg(4) + self.cam_pitch,
                ratio(105, 10),
            );
        }
        if self.mode == Mode::Map {
            let a = self.orbit;
            let target = v3(0, ratio(28, 10), -int(1));
            return (target, a + self.cam_yaw, -deg(12) + self.cam_pitch, int(19));
        }
        let p = self.perf.body.centre(&self.def);
        let yaw = self.cam_yaw;
        let pitch = CAM_PITCH + self.cam_pitch;
        match &self.act {
            Act::Tower(t) => {
                let base = acts::tower_base();
                let up = (t.height() + int(2)) * 6 / 10;
                (
                    base.add(v3(-int(1), up, 0)),
                    yaw - deg(20),
                    pitch,
                    int(12) + t.height() / 2,
                )
            }
            Act::Trapeze(..) => (
                v3(0, ratio(38, 10), BED_Z),
                yaw - deg(28),
                pitch + deg(4),
                int(12),
            ),
            Act::Juggle(_) => (
                v3(p.x, ratio(17, 10), p.z),
                yaw - deg(14),
                pitch + deg(6),
                ratio(58, 10),
            ),
            _ => {
                // keep a high bounce in frame: the target rises with the body
                let ty = ratio(16, 10) + fx::mul(p.y - ratio(16, 10), ONE * 3 / 4);
                (
                    v3(fx::mul(p.x, ONE * 7 / 10), ty, fx::mul(p.z, ONE * 7 / 10)),
                    yaw + CAM_YAW,
                    pitch,
                    balance::CAM_DISTANCE + (p.y - BED_Y).max(0) / 5,
                )
            }
        }
    }

    pub(crate) fn follow(&mut self) {
        if self.mode == Mode::Map || self.mode == Mode::Result {
            self.orbit += deg(1) / 6;
        }
        let (target, yaw, pitch, dist) = self.camera_goal();
        self.cam_target = self.cam_target.lerp(target, balance::CAM_FOLLOW);
        let eye = self.cam_target.sub(facing(yaw, pitch).scale(dist));
        let eye = v3(eye.x, eye.y.max(ratio(5, 10)), eye.z);
        self.cam = Camera {
            eye: self.cam.eye.lerp(eye, balance::CAM_FOLLOW * 2),
            target: self.cam_target,
        };
    }

    pub(crate) fn snap_camera(&mut self) {
        let (target, yaw, pitch, dist) = self.camera_goal();
        self.cam_target = target;
        self.cam = Camera {
            eye: target.sub(facing(yaw, pitch).scale(dist)),
            target,
        };
    }

    // ---- what the page draws ----------------------------------------------

    pub fn scene_version(&self) -> u32 {
        self.scene_version
    }

    pub fn static_instances(&self) -> Vec<i32> {
        self.scene.0.clone()
    }

    pub fn static_draws(&self) -> Vec<u32> {
        self.scene.1.clone()
    }

    pub fn view(&self) -> Vec<i32> {
        self.cam.values(self.aspect)
    }

    fn dynamic(&self) -> Batch {
        let mut b = Batch::new();
        // the performer, between the last two ticks
        let mut pos = [V3::ZERO; POINTS];
        for (i, p) in pos.iter_mut().enumerate() {
            *p = self.perf.body.prev[i].lerp(self.perf.body.pos[i], self.alpha);
        }
        let paint = Paint::of(self.tour.costume(&self.progress.costume));
        render::performer(&mut b, &self.def, &pos, &paint);
        // a shadow on whatever is under her
        let centre = self.perf.body.centre(&self.def);
        let floor = stage::floor_under(centre.x, centre.z, self.perf.body.lowest(&self.def));
        let high = (centre.y - floor).clamp(0, int(10));
        let size = ratio(7, 10) - high / 25;
        let shadow_y = if floor == BED_Y {
            floor - self.cloth_depth_at(centre) + ONE / 50
        } else {
            // over the stage's raised apron (scene.rs), not into it
            floor + ONE / 15
        };
        b.shape(
            Mesh::Cyl,
            v3(centre.x, shadow_y, centre.z),
            v3(size, ONE / 100, size),
            rgb("#141414"),
        );
        self.draw_cloth(&mut b);
        self.crowd.draw(&mut b, self.tick, self.mode == Mode::Lab);
        if self.mode == Mode::Lab {
            self.lab_draw(&mut b);
        }
        match &self.act {
            Act::Popcorn(p) => {
                for k in &p.kernels {
                    b.push(
                        Mesh::Octa,
                        &render::boxed(k.pos, v3(ONE / 7, ONE / 7, ONE / 7)),
                        rgb("#fff3c4"),
                        ONE / 3,
                    );
                }
            }
            Act::Tower(t) => self.draw_tower(&mut b, t),
            Act::Trapeze(t, _) => {
                let p = acts::trapeze_pivot();
                let bar = t.bar();
                let half = ratio(5, 10);
                for s in [-1, 1] {
                    b.rod(
                        Mesh::Cyl,
                        p.add(v3(0, 0, s * half)),
                        bar.add(v3(0, 0, s * half)),
                        ONE / 30,
                        rgb("#e5e7eb"),
                        0,
                    );
                }
                b.rod(
                    Mesh::Cyl,
                    bar.add(v3(0, 0, -half)),
                    bar.add(v3(0, 0, half)),
                    ONE / 14,
                    rgb("#c0392b"),
                    0,
                );
            }
            Act::Juggle(j) => {
                let hands = [pos[body::HAND_R], pos[body::HAND_L]];
                let colours = [rgb("#ef476f"), rgb("#06d6a0"), rgb("#ffd166")];
                // a ring under each hand, lit while a ball is due there
                for (k, h) in hands.iter().enumerate() {
                    let ring = h.sub(v3(0, ratio(16, 100), 0));
                    let (colour, glow) = if j.hot(k) {
                        (rgb("#ffd166"), ONE)
                    } else {
                        (rgb("#9ca3af"), 0)
                    };
                    b.push(
                        Mesh::Cyl,
                        &render::boxed(ring, v3(ratio(3, 10), ONE / 40, ratio(3, 10))),
                        colour,
                        glow,
                    );
                }
                for (i, ball) in j.balls.iter().enumerate() {
                    let at = j.ball_at(ball, hands);
                    b.push(
                        Mesh::Sphere,
                        &render::boxed(at, v3(ratio(17, 100), ratio(17, 100), ratio(17, 100))),
                        colours[i % 3],
                        ONE / 5,
                    );
                }
            }
            Act::None => {}
        }
        b
    }

    fn cloth_depth_at(&self, p: V3) -> i32 {
        let i = ((p.x + balance::BED_HALF_X) * (CLOTH_X as i32 - 1) / (2 * balance::BED_HALF_X))
            .clamp(0, CLOTH_X as i32 - 1) as usize;
        let k = ((p.z - BED_Z + BED_HALF_Z) * (CLOTH_Z as i32 - 1) / (2 * BED_HALF_Z))
            .clamp(0, CLOTH_Z as i32 - 1) as usize;
        -self.cloth.h[k * CLOTH_X + i]
    }

    fn draw_cloth(&self, b: &mut Batch) {
        let dark = rgb("#22252b");
        let line = rgb("#e2e8f0");
        for k in 0..CLOTH_Z - 1 {
            for i in 0..CLOTH_X - 1 {
                let (a, bx, cz, d) = (
                    self.cloth.at(i, k),
                    self.cloth.at(i + 1, k),
                    self.cloth.at(i, k + 1),
                    self.cloth.at(i + 1, k + 1),
                );
                let centre = v3((a.x + d.x) / 2, (a.y + bx.y + cz.y + d.y) / 4, (a.z + d.z) / 2);
                let ex = bx.lerp(d, ONE / 2).sub(a.lerp(cz, ONE / 2));
                let ez = cz.lerp(d, ONE / 2).sub(a.lerp(bx, ONE / 2));
                let n = ez.cross(ex).norm().scale(ONE / 40);
                let af = Affine {
                    c0: ex,
                    c1: n,
                    c2: ez,
                    t: centre,
                };
                // the bed's centre cross is marked, as on a real bed
                let mark =
                    (i == CLOTH_X / 2 - 1 || i == CLOTH_X / 2) && (k == CLOTH_Z / 2 - 1 || k == CLOTH_Z / 2);
                b.push(
                    Mesh::Cube,
                    &af,
                    if mark {
                        render::mix(dark, line, ONE / 5)
                    } else {
                        dark
                    },
                    0,
                );
            }
        }
    }

    fn draw_tower(&self, b: &mut Batch, t: &Tower) {
        let base = acts::tower_base();
        let [tx, tz] = t.tilt();
        let lean = Affine::rot_z(-tx).then(&Affine::rot_x(tz));
        let at = |p: V3| base.add(lean.apply_dir(p));
        let wood = rgb("#c08552");
        let red = rgb("#d62828");
        let lifting = match t.phase {
            TowerPhase::Lifting(k) => Some(k),
            _ => None,
        };
        let mut chairs = t.chairs;
        if lifting.is_some() {
            chairs += 1;
        }
        for k in 0..chairs {
            let y0 = acts::TABLE_HEIGHT + k * acts::CHAIR_HEIGHT;
            let turn = if k % 2 == 0 { 0 } else { QUARTER };
            let colour = if k % 3 == 1 { red } else { wood };
            let rise = match lifting {
                Some(s) if k == chairs - 1 => (45 - s) * (int(1) / 12),
                _ => 0,
            };
            let frame = Affine::translate(at(v3(0, y0, 0)))
                .then(&lean)
                .then(&Affine::rot_y(turn + k * deg(7)));
            let local = |p: V3| frame.apply(p.add(v3(0, -rise, 0)));
            let seat = local(v3(0, ratio(3, 10), 0));
            let r = Affine {
                t: seat,
                ..frame.then(&Affine::scale(v3(ratio(45, 100), ONE / 20, ratio(45, 100))))
            };
            b.push(Mesh::Cube, &r, colour, 0);
            for (sx, sz) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                let foot = local(v3(sx * ratio(2, 10), 0, sz * ratio(2, 10)));
                let top = local(v3(sx * ratio(2, 10), ratio(3, 10), sz * ratio(2, 10)));
                b.rod(Mesh::Cube, foot, top, ONE / 25, render::shade(colour, 80), 0);
            }
            let back0 = local(v3(0, ratio(3, 10), -ratio(2, 10)));
            let back1 = local(v3(0, ratio(62, 100), -ratio(22, 100)));
            if let Some(af) = render::segment(back0, back1, ratio(42, 100), ONE / 25) {
                b.push(Mesh::Cube, &af, colour, 0);
            }
        }
    }

    /// Builds this frame's moving things, for `instances` and `draws`.
    pub fn build_frame(&mut self) {
        let (values, draws) = self.dynamic().finish(render::DYNAMIC_BUFFER);
        let mut all = self.scene.1.clone();
        all.extend(draws);
        self.frame = (values, all);
    }

    pub fn instances(&self) -> Vec<i32> {
        self.frame.0.clone()
    }

    pub fn draws(&self) -> Vec<u32> {
        self.frame.1.clone()
    }

    /// Both at once: the values and the draws of one frame's moving things.
    pub fn frame_draws(&self) -> (Vec<i32>, Vec<u32>) {
        self.dynamic().finish(render::DYNAMIC_BUFFER)
    }

    pub fn scene_uniforms(&self) -> Vec<i32> {
        let v = &self.tour.venues[self.venue];
        let l = scene::light_dir(v);
        let mut u = vec![l.x, l.y, l.z];
        u.extend(rgb(&v.sky_top));
        u.extend(rgb(&v.fog));
        u.extend([int(70), int(1700)]);
        u.extend([0, 0, 0]);
        u.push(int(10));
        u
    }

    pub fn clear_color(&self) -> Vec<i32> {
        rgb(&self.tour.venues[self.venue].sky_top).to_vec()
    }

    // ---- the HUD ----------------------------------------------------------

    fn goal_text(&self, goal: &Goal) -> String {
        let g = |k: &str| self.copy_str(&["goals", k]);
        match goal {
            Goal::Height { metres: m, times } => fill(
                &g("height"),
                &[("metres", m.to_string()), ("times", times.to_string())],
            ),
            Goal::Tricks { tricks } => {
                let names: Vec<String> = tricks
                    .iter()
                    .map(|id| {
                        self.book
                            .names()
                            .into_iter()
                            .find(|(i, _)| i == id)
                            .map(|(_, n)| n)
                            .unwrap_or_default()
                    })
                    .collect();
                fill(&g("tricks"), &[("list", names.join(", "))])
            }
            Goal::Routine { count, score } => fill(
                &g("routine"),
                &[("count", count.to_string()), ("score", score.to_string())],
            ),
            Goal::Roof { trick } => g(if *trick { "roof_trick" } else { "roof" }),
            Goal::Popcorn { count, seconds } => fill(
                &g("popcorn"),
                &[("count", count.to_string()), ("seconds", seconds.to_string())],
            ),
            Goal::Tower { chairs, hold } => fill(
                &g("tower"),
                &[("chairs", chairs.to_string()), ("hold", hold.to_string())],
            ),
            Goal::Trapeze { trick } if trick.is_empty() => g("trapeze"),
            Goal::Trapeze { trick } => {
                let name = self
                    .book
                    .names()
                    .into_iter()
                    .find(|(i, _)| i == trick)
                    .map(|(_, n)| n)
                    .unwrap_or_default();
                fill(&g("trapeze_trick"), &[("trick", name)])
            }
            Goal::Juggle { balls, catches } => fill(
                &g("juggle"),
                &[("catches", catches.to_string()), ("balls", balls.to_string())],
            ),
        }
    }

    fn progress_text(&self) -> String {
        let r = &self.run;
        let base = fill(
            &self.copy_str(&["progress"]),
            &[("done", r.count.to_string()), ("target", r.target().to_string())],
        );
        match &r.goal {
            Goal::Routine { score, .. } => format!(
                "{base} · {}",
                fill(
                    &self.copy_str(&["routine_points"]),
                    &[
                        ("points", r.chain_score.to_string()),
                        ("target", score.to_string())
                    ]
                )
            ),
            Goal::Tower { .. } => {
                if let Act::Tower(t) = &self.act {
                    let height = t.height() + balance::TRUCK_ROOF;
                    format!(
                        "{base} · {} · {}",
                        fill(&self.copy_str(&["tower_height"]), &[("feet", feet(height))]),
                        fill(
                            &self.copy_str(&["tower_lean"]),
                            &[("degrees", t.lean_degrees().to_string())]
                        )
                    )
                } else {
                    base
                }
            }
            Goal::Juggle { .. } => {
                if let Act::Juggle(j) = &self.act {
                    format!(
                        "{base} · {}",
                        fill(
                            &self.copy_str(&["juggle_streak"]),
                            &[("streak", j.streak.to_string())]
                        )
                    )
                } else {
                    base
                }
            }
            _ => base,
        }
    }

    fn hint(&self) -> String {
        match &self.act {
            Act::Tower(t) => match t.phase {
                TowerPhase::Stacking => {
                    let target = match &self.run.goal {
                        Goal::Tower { chairs, .. } => *chairs,
                        _ => 0,
                    };
                    self.copy_str(&[if t.chairs >= target {
                        "start_handstand"
                    } else {
                        "add_chair"
                    }])
                }
                TowerPhase::Handstand(_) => self.copy_str(&["handstand"]),
                _ => String::new(),
            },
            Act::Trapeze(t, _) => self.copy_str(&[if t.holding { "release" } else { "climb_back" }]),
            Act::Juggle(j) => self.copy_str(&[if j.started { "juggle_hint" } else { "juggle_start" }]),
            _ => self.copy_str(&["restart_hint"]),
        }
    }

    pub fn hud_json(&self) -> String {
        let m = &self.tour.missions[self.selected];
        let v = self.tour.venue(&m.venue);
        let h = (self.perf.body.lowest(&self.def) - BED_Y).max(0);
        let bubbles: Vec<Value> = self
            .callouts
            .iter()
            .filter(|c| self.tick - c.tick < SHOUT_TICKS)
            .flat_map(|c| {
                c.shouters
                    .iter()
                    .filter(|&&i| i < self.crowd.people.len())
                    .filter_map(move |&i| {
                        self.cam
                            .screen(self.aspect, self.crowd.head(i))
                            .filter(|(x, y)| *x > 0 && *x < 100 * ONE && *y > 0 && *y < 100 * ONE)
                            .map(|(x, y)| json!({"x": pct(x), "y": pct(y), "text": c.shown, "seq": c.seq}))
                    })
            })
            .collect();
        json!({
            "mode": self.mode,
            "mission": m.name,
            "venue": v.name,
            "city": v.city,
            "when": v.when,
            "objective": self.goal_text(&m.goal),
            "progress": self.progress_text(),
            "timer": self.run.seconds_left().map(|s| fill(&self.copy_str(&["time_left"]), &[("seconds", s.to_string())])),
            "height": fill(&self.copy_str(&["height_now"]), &[("metres", metres(h)), ("feet", feet(h))]),
            "peak": fill(&self.copy_str(&["peak"]), &[("metres", metres(self.peak))]),
            "hint": self.hint(),
            "callouts": self.callouts,
            "sfx": self.sfx,
            "bubbles": bubbles,
            "result": self.result,
            "hands": self.hand_badges(),
            "art": postcard(&v.id),
            "next_art": self.result.as_ref().and_then(|r| r.next).map(|i| postcard(&self.tour.missions[i].venue)),
            "next_venue": self.result.as_ref().and_then(|r| r.next).map(|i| {
                let nv = self.tour.venue(&self.tour.missions[i].venue);
                format!("{} · {}", nv.name, nv.city)
            }),
            "ending": self.end_in > 0,
        })
        .to_string()
    }

    /// Juggling's key badges: under each hand on screen, lit when a ball
    /// is arriving there.
    fn hand_badges(&self) -> Vec<Value> {
        let Act::Juggle(j) = &self.act else {
            return Vec::new();
        };
        let hands = [self.perf.body.pos[body::HAND_R], self.perf.body.pos[body::HAND_L]];
        let mut out = Vec::new();
        for (k, at) in hands.iter().enumerate() {
            let below = at.sub(v3(0, ratio(28, 100), 0));
            if let Some((x, y)) = self.cam.screen(self.aspect, below) {
                out.push(json!({
                    "x": pct(x),
                    "y": pct(y),
                    "key": self.copy["hand_keys"][k],
                    "hot": j.hot(k),
                }));
            }
        }
        out
    }

    /// The glossary of moves: each trick, how to do it, and its points.
    pub fn glossary_json(&self) -> String {
        let pos = |p: &str| self.copy_str(&["positions", p]);
        let moves: Vec<Value> = self
            .book
            .glossary()
            .into_iter()
            .map(|(id, name, how, from, to, value)| {
                json!({
                    "id": id,
                    "name": name,
                    "how": how,
                    "from_to": fill(&self.copy_str(&["glossary_from_to"]), &[("from", pos(&from)), ("to", pos(&to))]),
                    "points": fill(&self.copy_str(&["glossary_points"]), &[("points", value.to_string())]),
                })
            })
            .collect();
        json!({"moves": moves, "roof": self.book.roof_how()}).to_string()
    }

    pub fn map_json(&self) -> String {
        let layout = self.tour.layout();
        let missions: Vec<Value> = self
            .tour
            .missions
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let v = self.tour.venue(&m.venue);
                let best = self.progress.best.get(&m.id).copied();
                let leg = self.tour.legs.iter().position(|l| l.id == v.leg).unwrap_or(0);
                json!({
                    "i": i,
                    "name": m.name,
                    "venue": v.name,
                    "city": v.city,
                    "when": v.when,
                    "leg": leg,
                    "level": self.tour.level(i),
                    "x": pct(layout[i].0),
                    "y": pct(layout[i].1),
                    "open": self.progress.open(&self.tour, i),
                    "passed": best.is_some(),
                    "best": best.map(|g| self.copy_str(&["judge", &self.book.plain_word(g)])),
                    "best_word": best.map(|g| self.book.plain_word(g)),
                    "goal": self.goal_text(&m.goal),
                    "brief": m.brief,
                    "unlock": if m.unlock.is_empty() { String::new() } else { self.tour.costume(&m.unlock).name.clone() },
                    "art": postcard(&v.id),
                })
            })
            .collect();
        let costumes: Vec<Value> = self
            .tour
            .costumes
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let by = self
                    .tour
                    .missions
                    .iter()
                    .find(|m| m.unlock == c.id)
                    .map(|m| m.name.clone())
                    .unwrap_or_default();
                json!({
                    "i": i,
                    "name": c.name,
                    "unlocked": self.progress.unlocked(&self.tour, &c.id),
                    "worn": c.id == self.progress.costume,
                    "by": by,
                    "colours": [c.shirt, c.sleeve, c.pants, c.hat_colour],
                })
            })
            .collect();
        let edges: Vec<Value> = self
            .tour
            .edges()
            .iter()
            .map(|&(a, b)| json!({"x1": pct(layout[a].0), "y1": pct(layout[a].1), "x2": pct(layout[b].0), "y2": pct(layout[b].1), "open": self.progress.passed(&self.tour.missions[a].id)}))
            .collect();
        let legs: Vec<Value> = self
            .tour
            .legs
            .iter()
            .map(|l| json!({"name": l.name, "note": l.note}))
            .collect();
        json!({
            "selected": self.selected,
            "missions": missions,
            "edges": edges,
            "legs": legs,
            "costumes": costumes,
            "passed": self.progress.best.len(),
            "total": self.tour.missions.len(),
        })
        .to_string()
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn performer(&self) -> &Performer {
        &self.perf
    }

    pub fn progress(&self) -> &Progress {
        &self.progress
    }
}

/// The postcard of a venue (tools/postcards.py draws them).
pub fn postcard(venue_id: &str) -> String {
    format!("art/venue/{venue_id}.png")
}

/// Where the juggler stands: on the grass between the bed and the crowd.
pub fn juggle_spot() -> V3 {
    v3(0, 0, BED_Z + BED_HALF_Z + ratio(22, 10))
}

/// White noise for the page's sounds, from the core's one source of chance
/// (the page may not draw random numbers itself).
pub fn noise(n: u32, seed: u32) -> Vec<i32> {
    let mut rng = Rng::new(seed as u64);
    (0..n.min(1 << 20)).map(|_| rng.range(-ONE, ONE)).collect()
}

pub fn mesh_vertices(id: u32) -> Vec<i32> {
    Mesh::ALL
        .get(id as usize)
        .map(|m| mesh::vertices(*m))
        .unwrap_or_default()
}

/// Every number the page needs, from where it is decided.
pub fn numbers() -> String {
    let vertex_bytes = mesh::VERTEX_VALUES * render::BYTES_PER_VALUE;
    let instance_bytes = render::INSTANCE_VALUES * render::BYTES_PER_VALUE;
    let vec4 = 4 * render::BYTES_PER_VALUE;
    json!({
        "frac_bits": fx::FRAC_BITS,
        "ticks_per_second": TICKS_PER_SECOND,
        "actions": ACTIONS.iter().map(|(n, b)| (n.to_string(), json!(b))).collect::<serde_json::Map<_, _>>(),
        "meshes": Mesh::ALL.iter().map(|m| json!({
            "id": *m as u32,
            "vertices": mesh::vertices(*m).len() / mesh::VERTEX_VALUES,
        })).collect::<Vec<_>>(),
        "vertex": {
            "stride": vertex_bytes,
            "attributes": [["aPos", 3, 0], ["aNormal", 3, 3 * render::BYTES_PER_VALUE]],
        },
        "instance": {
            "stride": instance_bytes,
            "attributes": [["iA", 4, 0], ["iB", 4, vec4], ["iC", 4, 2 * vec4], ["iD", 4, 3 * vec4]],
        },
        "draw_values": render::DRAW_VALUES,
        "buffers": { "static": render::STATIC_BUFFER, "dynamic": render::DYNAMIC_BUFFER },
        "uniforms": [["uLightDir", 3], ["uSky", 3], ["uFog", 3], ["uFogRange", 2], ["uGridColor", 3], ["uGridStep", 1]],
        "view_values": 16,
        "percent": 100,
    })
    .to_string()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn game_with(saved: &str) -> Game {
        Game::new(
            include_str!("../../../data/body.json"),
            include_str!("../../../data/tricks.json"),
            include_str!("../../../data/venues.json"),
            include_str!("../../../data/missions.json"),
            include_str!("../../../data/costumes.json"),
            include_str!("../../../data/copy.json"),
            saved,
        )
        .unwrap()
    }

    fn frames(g: &mut Game, bits: u32, n: i64, at: &mut i64) {
        for _ in 0..n {
            *at += 16_667;
            g.advance(*at, bits, 0, 0);
        }
    }

    #[test]
    fn the_game_opens_on_the_map_at_the_first_show() {
        let g = game_with("");
        assert_eq!(g.mode(), Mode::Map);
        assert_eq!(g.selected, 0);
        let map: Value = serde_json::from_str(&g.map_json()).unwrap();
        assert_eq!(map["missions"][0]["open"], true);
        assert_eq!(map["missions"][1]["open"], false);
        assert_eq!(map["missions"][0]["venue"], "RTP 800 Building");
    }

    #[test]
    fn the_map_bot_bounces_without_crashing_for_a_minute() {
        let mut g = game_with("");
        let mut t = 0;
        frames(&mut g, 0, 3600, &mut t);
        assert!(g.demo_n > 5, "{}", g.demo_n);
        assert!(!g.perf.bailed());
    }

    #[test]
    fn holding_space_in_the_first_show_passes_it() {
        let mut g = game_with("");
        g.start();
        assert_eq!(g.mode(), Mode::Play);
        let mut t = 0;
        frames(&mut g, bit("drive"), 1800, &mut t);
        frames(&mut g, 0, 200, &mut t);
        assert_eq!(g.mode(), Mode::Result);
        let hud: Value = serde_json::from_str(&g.hud_json()).unwrap();
        assert_eq!(hud["result"]["passed"], true);
        assert!(g.progress().passed("m01"));
        // the next show is open now
        assert!(g.progress().open(&g.tour, 1));
    }

    #[test]
    fn a_locked_show_does_not_start() {
        let mut g = game_with("");
        g.select(5);
        g.start();
        assert_eq!(g.mode(), Mode::Map);
    }

    #[test]
    fn every_show_starts_and_runs_ten_seconds_of_random_keys() {
        let mut g = game_with("");
        g.open_everything();
        let mut rng = Rng::new(9);
        for i in 0..g.tour.missions.len() {
            g.select(i);
            g.start();
            let mut t = 0;
            for _ in 0..600 {
                t += 16_667;
                let bits = rng.next() as u32 & 0x1fff & !bit("restart");
                g.advance(t, bits, 0, 0);
            }
            let hud: Value = serde_json::from_str(&g.hud_json()).unwrap();
            assert!(hud["objective"].as_str().unwrap().len() > 5);
            assert!(!g.frame_draws().1.is_empty());
        }
    }

    #[test]
    fn the_draw_lists_name_only_meshes_that_exist() {
        let g = game_with("");
        let (values, draws) = g.frame_draws();
        let mut total = 0;
        for d in draws.chunks(render::DRAW_VALUES) {
            assert!((d[1] as usize) < Mesh::ALL.len());
            total += d[3] as usize;
        }
        assert_eq!(total * render::INSTANCE_VALUES, values.len());
    }

    #[test]
    fn percentages_are_written_for_css() {
        assert_eq!(pct(ONE * 75 / 2), "37.50");
        assert_eq!(pct(-ONE / 4), "-0.25");
    }

    #[test]
    fn metres_and_feet_read_as_people_write_them() {
        assert_eq!(metres(int(3) + ONE / 4), "3.2");
        assert_eq!(feet(int(10)), "32");
    }
}

#[cfg(test)]
mod page_walk {
    use super::tests::game_with;
    use super::*;

    /// The page's own sequence: resize, then every frame advance, the HUD
    /// and a frame build, through each kind of show.
    #[test]
    fn the_page_walk_through_every_act_does_not_panic() {
        let mut g = game_with("");
        g.resize(1400, 860, 200);
        g.open_everything();
        let mut t = 0i64;
        let mut run = |g: &mut Game, bits: u32, frames: i32| {
            for _ in 0..frames {
                t += 16_667;
                g.advance(t, bits, 0, 0);
                let _ = g.hud_json();
                g.build_frame();
                let _ = g.view();
            }
        };
        // a crash at a big venue, then straight to a smaller crowd
        g.select(25);
        g.start();
        g.say("oh_no", String::new(), 40);
        let shouting = g.callouts.last().unwrap().shouters.clone();
        assert!(shouting.iter().any(|&i| i >= 46), "{shouting:?}");
        g.select(14);
        g.start();
        let _ = g.hud_json();
        for (mission, keys) in [
            (2, vec![("drive", 360), ("back", 15), ("", 120)]),
            (8, vec![("", 90), ("drive", 2), ("", 600)]),
            (11, vec![("front", 90), ("drive", 2), ("", 600)]),
            (9, vec![("", 600), ("left", 2), ("", 20), ("right", 2)]),
            (3, vec![("drive", 1800)]),
        ] {
            g.select(mission);
            g.start();
            let _ = g.map_json();
            for (k, n) in keys {
                run(&mut g, bit(k), n);
            }
        }
    }
}

#[cfg(test)]
mod reachable {
    use super::tests::game_with;
    use super::*;

    fn tick(g: &mut Game, bits: u32) {
        g.step(bits);
    }

    #[test]
    fn a_player_who_bounces_high_and_travels_lands_on_the_roof() {
        // a player tries the travel key at a few heights; at least some land
        let mut found = Vec::new();
        for quarter in 12..26 {
            let mut g = game_with("");
            g.open_everything();
            g.select(4);
            g.start();
            let mut landed_on = None;
            for _ in 0..4000 {
                let p = &g.perf;
                let high = p.flight.peak - BED_Y > int(quarter) / 4;
                let bits = if matches!(p.phase, Phase::Bed(_)) && high {
                    bit("drive") | bit("up")
                } else {
                    bit("drive")
                };
                tick(&mut g, bits);
                if let Phase::Stand(s) = g.perf.phase {
                    landed_on = Some(s);
                    break;
                }
                if g.perf.bailed() {
                    break;
                }
            }
            found.push((quarter, landed_on));
        }
        assert!(
            found.iter().filter(|(_, s)| *s == Some(Surface::Roof)).count() >= 2,
            "{found:?}"
        );
    }

    #[test]
    fn letting_go_at_the_front_of_the_swing_lands_on_the_bed() {
        let mut found = Vec::new();
        for wait in (60..400).step_by(10) {
            let mut g = game_with("");
            g.open_everything();
            g.select(11);
            g.start();
            for k in 0..wait {
                let pump = if let Act::Trapeze(t, _) = &g.act {
                    if t.omega > 0 {
                        bit("front")
                    } else {
                        bit("back")
                    }
                } else {
                    0
                };
                let _ = k;
                tick(&mut g, pump);
            }
            tick(&mut g, bit("act"));
            let mut out = "none";
            for _ in 0..400 {
                tick(&mut g, 0);
                match g.perf.phase {
                    Phase::Bed(Posture::Feet) => {
                        out = "bed";
                        break;
                    }
                    Phase::Bail(_) => {
                        out = "crash";
                        break;
                    }
                    Phase::Stand(_) => {
                        out = "ground";
                        break;
                    }
                    _ => {}
                }
            }
            found.push((wait, out));
        }
        assert!(
            found.iter().filter(|(_, o)| *o == "bed").count() >= 3,
            "{found:?}"
        );
    }
}
