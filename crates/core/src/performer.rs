//! The performer in control: standing, bouncing, flying, and falling.
//!
//! The flight model carries a centre of mass, a velocity, a heading, a flip
//! angle and a twist angle, and the angular momentum behind each turn. The
//! shape the body holds sets its inertia, and the momentum is conserved, so a
//! tuck turns faster than a layout with nothing else said (PLAN.md, "The
//! body"). On the bed, the drive key pushes through the bed's rebound and
//! the flip and twist keys set the turn a takeoff carries. A landing is
//! classified here and named in `trick`; a bad one hands the particles to the
//! Verlet solver in `body`.

use crate::balance::*;
use crate::body::{self, Body, BodyDef, Pose, Shape};
use crate::fx::{self, deg, HALF, ONE, QUARTER, TURN};
use crate::geom::{facing, right_of, v3, Affine, V3};
use crate::stage::{self, Surface};

/// What the player is holding, as the core reads it. The page sets bits; the
/// game turns bits into this.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Controls {
    /// −1 front (W), +1 back (S).
    pub flip: i32,
    /// −1 left (A), +1 right (D).
    pub twist: i32,
    pub drive: bool,
    pub kill: bool,
    pub tuck: bool,
    pub pike: bool,
    /// −1 back, +1 forward (the arrow keys).
    pub travel: i32,
    /// −1 left, +1 right.
    pub turn: i32,
    /// Pressed this tick (not held): grab, release, place.
    pub act: bool,
    pub jump: bool,
}

/// How the body meets the bed: the four positions of trampolining.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Posture {
    Feet,
    Seat,
    Back,
    Front,
}

impl Posture {
    /// The flip angle that lies this way.
    pub fn angle(self) -> i32 {
        match self {
            Posture::Feet | Posture::Seat => 0,
            Posture::Back => QUARTER,
            Posture::Front => -QUARTER,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Posture::Feet => "feet",
            Posture::Seat => "seat",
            Posture::Back => "back",
            Posture::Front => "front",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Stand(Surface),
    Bed(Posture),
    Air,
    /// Limp, the particles in the solver, for so many ticks.
    Bail(i32),
    /// Getting up: the particles move from where they lay to the stand.
    Rise(i32),
    /// A mission's act holds the body (the trapeze, the tower, juggling).
    Held,
}

/// What one flight did, kept from takeoff to landing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flight {
    pub from: Posture,
    pub from_surface: Surface,
    pub flip0: i32,
    pub twist0: i32,
    pub base_y: i32,
    pub peak: i32,
    pub ticks: i32,
    /// Ticks spent in a tuck, a pike and straight.
    pub tuck: i32,
    pub pike: i32,
    pub straight: i32,
    /// A flight back to the feet from a drop, turned by the bed, not the player.
    pub returning: bool,
    pub trapeze: bool,
}

/// A landing, as the trick namer and the crowd need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Landing {
    pub flight: Flight,
    pub to: Posture,
    pub surface: Surface,
    /// Signed quarter turns of flip (positive is backward) and half turns of twist.
    pub quarters: i32,
    pub half_twists: i32,
    /// Degrees off the ideal landing, and off a whole half twist.
    pub off: i32,
    pub twist_off: i32,
    /// Height of the flight's peak above its takeoff, metres Q16.
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Landed(Landing),
    Crashed(Flight),
    /// The top of a flight: the lowest point of the body above the bed.
    Peak(i32),
    Takeoff,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Performer {
    pub com: V3,
    pub vel: V3,
    pub heading: i32,
    pub flip: i32,
    pub twist: i32,
    pub flip_l: i32,
    pub twist_l: i32,
    pub shape: Shape,
    pub body: Body,
    pub phase: Phase,
    pub flight: Flight,
    pub rose_from: Shape,
    /// The pose the body is held in by an act, and where the act holds it.
    pub held_pose: Pose,
    pub hold_frame: Option<Affine>,
    /// The Graph Lab's lean: the bed stops all sideways travel on landing,
    /// and each beat an arrow is held on the bed adds `LEAN_STEP` of speed,
    /// so how long the key is held sets how far a bounce carries. The
    /// shows keep the arrow's fixed travel.
    pub lean_build: bool,
}

/// The rotation of the body: heading, then flip about the body's left-right
/// axis, then twist about its long axis.
pub fn orientation(heading: i32, flip: i32, twist: i32) -> Affine {
    Affine::rot_y(heading)
        .then(&Affine::rot_x(flip))
        .then(&Affine::rot_y(twist))
}

fn round_div(a: i32, b: i32) -> i32 {
    let (a, b) = (a as i64, b as i64);
    (if a >= 0 {
        (a + b / 2) / b
    } else {
        -((-a + b / 2) / b)
    }) as i32
}

fn degrees(a: i32) -> i32 {
    (a.abs() as i64 * 360 / TURN as i64) as i32
}

impl Performer {
    pub fn new(def: &BodyDef, at: V3, heading: i32) -> Performer {
        let shape = *def.pose(Pose::Stand);
        let lowest = shape
            .iter()
            .zip(def.radius.iter())
            .map(|(p, r)| p.y - r)
            .min()
            .unwrap_or(0);
        let com = v3(at.x, at.y - lowest, at.z);
        let frame = Affine::translate(com).then(&orientation(heading, 0, 0));
        let flight = Flight {
            from: Posture::Feet,
            from_surface: Surface::Ground,
            flip0: 0,
            twist0: 0,
            base_y: com.y,
            peak: com.y,
            ticks: 0,
            tuck: 0,
            pike: 0,
            straight: 0,
            returning: false,
            trapeze: false,
        };
        Performer {
            com,
            vel: V3::ZERO,
            heading,
            flip: 0,
            twist: 0,
            flip_l: 0,
            twist_l: 0,
            shape,
            body: Body::at(&shape, &frame),
            phase: Phase::Stand(stage::surface_under(at.x, at.z, at.y)),
            flight,
            rose_from: shape,
            held_pose: Pose::Stand,
            hold_frame: None,
            lean_build: false,
        }
    }

    /// Standing on the bed's centre, as at the start and after a crash.
    pub fn on_bed(def: &BodyDef, heading: i32) -> Performer {
        let mut p = Performer::new(def, v3(0, BED_Y, BED_Z), heading);
        p.phase = Phase::Bed(Posture::Feet);
        p
    }

    pub fn frame(&self) -> Affine {
        match (self.phase, self.hold_frame) {
            (Phase::Held, Some(f)) => f,
            _ => Affine::translate(self.com).then(&orientation(self.heading, self.flip, self.twist)),
        }
    }

    /// How far below the centre of mass the body reaches in a pose turned
    /// by `flip`: the drop from the centre to whatever it stands on.
    pub fn reach(def: &BodyDef, pose: Pose, flip: i32) -> i32 {
        let r = Affine::rot_x(flip);
        let s = def.pose(pose);
        -(0..body::POINTS)
            .map(|i| r.apply_dir(s[i]).y - def.radius[i])
            .min()
            .unwrap_or(0)
    }

    fn lowest_in(&self, def: &BodyDef, shape: &Shape) -> i32 {
        let r = orientation(self.heading, self.flip, self.twist);
        (0..body::POINTS)
            .map(|i| self.com.y + r.apply_dir(shape[i]).y - def.radius[i])
            .min()
            .unwrap_or(self.com.y)
    }

    fn shape_toward(&mut self, def: &BodyDef, pose: Pose, rate: i32) {
        self.shape = body::blend(&self.shape, def.pose(pose), rate);
    }

    /// One tick. Returns what happened that the mission and the crowd hear.
    pub fn step(&mut self, def: &BodyDef, c: &Controls) -> Vec<Event> {
        let mut events = Vec::new();
        match self.phase {
            Phase::Stand(s) => self.stand(def, c, s, &mut events),
            Phase::Bed(p) => self.bounce(def, c, p, &mut events),
            Phase::Air => self.fly(def, c, &mut events),
            Phase::Bail(t) => {
                self.body.fall(def);
                self.phase = if t + 1 >= BAIL_TICKS {
                    self.rose_from = self.body.pos;
                    Phase::Rise(0)
                } else {
                    Phase::Bail(t + 1)
                };
                return events;
            }
            Phase::Rise(t) => {
                let fresh = Performer::on_bed(def, self.heading);
                if t + 1 >= RISE_TICKS {
                    *self = fresh;
                } else {
                    let share = (t + 1) * ONE / RISE_TICKS;
                    self.body.prev = self.body.pos;
                    for i in 0..body::POINTS {
                        self.body.pos[i] = self.rose_from[i].lerp(fresh.body.pos[i], share);
                    }
                    self.phase = Phase::Rise(t + 1);
                }
                return events;
            }
            Phase::Held => {
                self.shape_toward(def, self.held_pose, SHAPE_RATE);
            }
        }
        let frame = self.frame();
        self.body.place(&self.shape, &frame);
        events
    }

    fn stand(&mut self, def: &BodyDef, c: &Controls, s: Surface, events: &mut Vec<Event>) {
        self.heading += c.turn * -TURN_RATE;
        self.flip = 0;
        self.twist = 0;
        self.vel = V3::ZERO;
        let pose = if c.jump { Pose::Crouch } else { Pose::Stand };
        self.shape_toward(def, pose, SHAPE_RATE);
        let step = facing(self.heading, 0).scale(WALK * c.travel);
        self.com = self.com.add(step);
        // Snap the feet to the floor under them.
        let floor = stage::floor_under(self.com.x, self.com.z, stage::height(s));
        let lowest = self.lowest_in(def, &self.shape);
        self.com.y += floor - lowest;
        let under = stage::surface_under(self.com.x, self.com.z, stage::height(s));
        if s == Surface::Roof && under != Surface::Roof {
            // walked off the edge of the roof
            self.take_off(Posture::Feet, Surface::Roof, events);
            return;
        }
        if under == Surface::Bed {
            self.phase = Phase::Bed(Posture::Feet);
            return;
        }
        if let Some(out) = stage::out_of_truck(self.com, ONE / 4) {
            self.com.x = out.x;
            self.com.z = out.z;
        }
        if c.jump {
            self.vel = v3(0, JUMP, 0).add(facing(self.heading, 0).scale(TRAVEL * c.travel));
            self.flip_l = c.flip * FLIP_MAX * 2 / 3;
            self.twist_l = c.twist * TWIST_MAX / 2;
            self.take_off(Posture::Feet, s, events);
        }
    }

    fn take_off(&mut self, from: Posture, surface: Surface, events: &mut Vec<Event>) {
        self.phase = Phase::Air;
        self.flight = Flight {
            from,
            from_surface: surface,
            flip0: self.flip,
            twist0: self.twist,
            base_y: self.com.y,
            peak: self.com.y,
            ticks: 0,
            tuck: 0,
            pike: 0,
            straight: 0,
            returning: false,
            trapeze: false,
        };
        events.push(Event::Takeoff);
    }

    /// Leaves an act (the trapeze) into a free flight.
    pub fn release(&mut self, vel: V3, flip_l: i32) {
        let mut ev = Vec::new();
        self.vel = vel;
        self.flip_l = flip_l;
        self.twist_l = 0;
        self.take_off(Posture::Feet, Surface::Ground, &mut ev);
        self.flight.trapeze = true;
        self.flight.base_y = BED_Y;
    }

    fn bounce(&mut self, def: &BodyDef, c: &Controls, posture: Posture, events: &mut Vec<Event>) {
        let lowest = self.lowest_in(def, &self.shape);
        let depth = (BED_Y - lowest).clamp(0, BED_MAX_DEPTH);
        let pushing = self.vel.y >= 0;
        let mut ay = fx::mul(BED_K, depth) - GRAVITY;
        // the apex this rebound is heading for, if the bed let go now
        let stored = fx::mul(self.vel.y, self.vel.y) + fx::mul(BED_K, fx::mul(depth, depth));
        let apex = self.com.y - BED_Y + fx::div(stored, 2 * GRAVITY);
        if pushing {
            if c.drive && apex < MAX_BOUNCE {
                ay += DRIVE_ACCEL;
            } else if !c.drive {
                self.vel.y -= fx::mul(self.vel.y, BED_DAMP);
            }
        }
        if posture == Posture::Feet {
            self.flip_l = (self.flip_l + c.flip * FLIP_BUILD).clamp(-FLIP_MAX, FLIP_MAX);
            self.twist_l = (self.twist_l + c.twist * TWIST_BUILD).clamp(-TWIST_MAX, TWIST_MAX);
        }
        if c.kill {
            self.vel.y -= fx::mul(self.vel.y, KILL_DAMP);
            if !self.lean_build {
                self.vel.x = 0;
                self.vel.z = 0;
            }
        }
        self.vel.y += ay;
        if self.lean_build {
            let lean = facing(self.heading, 0).scale(LEAN_STEP * c.travel);
            self.vel.x += lean.x;
            self.vel.z += lean.z;
        } else if posture == Posture::Feet && depth > 0 {
            // On the bed the turn keys turn the performer; the arrows set travel.
            self.heading += c.turn * -TURN_RATE;
            let want = facing(self.heading, 0).scale(TRAVEL * c.travel);
            self.vel.x = want.x;
            self.vel.z = want.z;
        }
        self.com = self.com.add(self.vel);
        // The body settles into the position it landed in.
        let target = posture.angle();
        self.flip = fx::approach(self.flip, target, deg(6));
        self.twist = fx::approach(self.twist, 0, deg(6));
        let pose = match posture {
            Posture::Feet => {
                if c.kill {
                    Pose::Crouch
                } else {
                    Pose::Stand
                }
            }
            Posture::Seat => Pose::Pike,
            Posture::Back | Posture::Front => Pose::Layout,
        };
        self.shape_toward(def, pose, SHAPE_RATE);
        let still_in = self.lowest_in(def, &self.shape) < BED_Y;
        if !still_in && self.vel.y > 0 {
            self.leave_bed(def, c, posture, events);
        }
        // Walked or bounced off the side of the bed onto the ground.
        if !stage::over_bed(self.com.x, self.com.z) && self.phase == Phase::Bed(posture) {
            self.take_off(posture, Surface::Bed, events);
        }
    }

    fn leave_bed(&mut self, def: &BodyDef, c: &Controls, posture: Posture, events: &mut Vec<Event>) {
        self.take_off(posture, Surface::Bed, events);
        if posture == Posture::Feet {
            return;
        }
        // From a drop the bed throws the body back to its feet, unless a
        // twist key asks for a half twist into the same drop (swivel hips,
        // a cradle).
        let ticks = (fx::div(2 * self.vel.y, GRAVITY) >> fx::FRAC_BITS).max(8);
        let twist_i = def.twist_inertia(def.pose(Pose::Layout));
        if c.twist != 0 {
            self.twist_l = (fx::mul(HALF / ticks, twist_i)) * c.twist;
            self.flip_l = 0;
        } else {
            let to_feet = -posture.angle();
            let stand_i = def.flip_inertia(def.pose(Pose::Stand));
            self.flip_l = fx::mul(to_feet / ticks, stand_i);
            self.twist_l = 0;
            self.flight.returning = true;
        }
    }

    fn fly(&mut self, def: &BodyDef, c: &Controls, events: &mut Vec<Event>) {
        let was_rising = self.vel.y > 0;
        self.vel.y -= GRAVITY;
        self.vel = self.vel.clamp_len(SPEED_CAP);
        self.com = self.com.add(self.vel);
        if was_rising && self.vel.y <= 0 {
            events.push(Event::Peak(self.lowest_in(def, &self.shape) - BED_Y));
        }
        let f = &mut self.flight;
        f.ticks += 1;
        f.peak = f.peak.max(self.com.y);
        let pose = if f.returning {
            match f.from {
                Posture::Back | Posture::Front => Pose::Layout,
                _ => Pose::Stand,
            }
        } else if c.tuck {
            f.tuck += 1;
            Pose::Tuck
        } else if c.pike {
            f.pike += 1;
            Pose::Pike
        } else {
            f.straight += 1;
            Pose::Layout
        };
        self.shape_toward(def, pose, SHAPE_RATE);
        if !self.flight.returning {
            self.flip_l += c.flip * AIR_FLIP;
            self.twist_l += c.twist * AIR_TWIST;
        }
        let flip_i = def.flip_inertia(&self.shape);
        let twist_i = def.twist_inertia(&self.shape);
        self.flip += fx::div(self.flip_l, flip_i);
        self.twist += fx::div(self.twist_l, twist_i);

        // Bumping into the truck's side ends the flight in a heap.
        let r = orientation(self.heading, self.flip, self.twist);
        for i in 0..body::POINTS {
            let p = self.com.add(r.apply_dir(self.shape[i]));
            if stage::out_of_truck(p, def.radius[i]).is_some() {
                self.crash(def, events);
                return;
            }
        }

        let lowest = self.lowest_in(def, &self.shape);
        let prev_low = self.body.lowest(def);
        if self.vel.y > 0 {
            return;
        }
        let surface = stage::surface_under(self.com.x, self.com.z, prev_low);
        let floor = stage::height(surface);
        if lowest >= floor {
            return;
        }
        match self.classify(surface, c) {
            Some(landing) => {
                self.land(def, landing, events);
            }
            None => self.crash(def, events),
        }
    }

    /// What this contact would be: a landing in one of the four positions
    /// (only on the feet off the bed), or `None` for a crash.
    pub fn classify(&self, surface: Surface, c: &Controls) -> Option<Landing> {
        let f = fx::wrap(self.flip);
        let posture = if f.abs() <= FEET_LIMIT {
            if surface == Surface::Bed && c.pike && !self.flight.returning {
                Posture::Seat
            } else {
                Posture::Feet
            }
        } else if surface == Surface::Bed && fx::wrap(f - QUARTER).abs() <= DROP_LIMIT {
            Posture::Back
        } else if surface == Surface::Bed && fx::wrap(f + QUARTER).abs() <= DROP_LIMIT {
            Posture::Front
        } else {
            return None;
        };
        let off = degrees(fx::wrap(f - posture.angle()));
        let total = self.flip - self.flight.flip0;
        let land_angle = self.flip - f + posture.angle();
        let quarters = round_div(land_angle - self.flight.flip0, QUARTER);
        let _ = total;
        let half_twists = round_div(self.twist - self.flight.twist0, HALF);
        let twist_off = degrees(self.twist - round_div(self.twist, HALF) * HALF);
        Some(Landing {
            flight: self.flight,
            to: posture,
            surface,
            quarters,
            half_twists: half_twists.abs(),
            off,
            twist_off,
            height: self.flight.peak - self.flight.base_y,
        })
    }

    fn land(&mut self, def: &BodyDef, l: Landing, events: &mut Vec<Event>) {
        // Fold whole turns of flip and half turns of twist away: a half twist
        // turns the heading round.
        let halves = round_div(self.twist, HALF);
        self.twist -= halves * HALF;
        if halves % 2 != 0 {
            self.heading += HALF;
            // facing the other way, a flip angle reads the other way round
            self.flip = -self.flip;
        }
        let turns = round_div(self.flip - l.to.angle(), TURN);
        self.flip -= turns * TURN;
        self.flip_l = 0;
        self.twist_l = 0;
        match l.surface {
            Surface::Bed => {
                self.phase = Phase::Bed(l.to);
                if self.lean_build {
                    self.vel.x = 0;
                    self.vel.z = 0;
                }
            }
            s => {
                self.phase = Phase::Stand(s);
                self.vel = V3::ZERO;
                self.flip = 0;
                self.twist = 0;
                let floor = stage::height(s);
                let lowest = self.lowest_in(def, &self.shape);
                self.com.y += floor - lowest;
            }
        }
        events.push(Event::Landed(l));
    }

    pub fn crash(&mut self, def: &BodyDef, events: &mut Vec<Event>) {
        let frame = self.frame();
        self.body.place(&self.shape, &frame);
        // The body keeps the speed it had; a spin shows in how the
        // particles moved this tick.
        let spin = self.vel;
        for i in 0..body::POINTS {
            self.body.prev[i] = self.body.pos[i].sub(spin);
        }
        let _ = def;
        self.phase = Phase::Bail(0);
        self.flip_l = 0;
        self.twist_l = 0;
        events.push(Event::Crashed(self.flight));
    }

    pub fn bailed(&self) -> bool {
        matches!(self.phase, Phase::Bail(_) | Phase::Rise(_))
    }

    /// Height of the centre of mass above the bed, for the HUD.
    pub fn height(&self) -> i32 {
        self.com.y - BED_Y
    }

    pub fn right(&self) -> V3 {
        right_of(self.heading)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::int;

    pub fn def() -> BodyDef {
        BodyDef::parse(include_str!("../../../data/body.json")).unwrap()
    }

    fn run(p: &mut Performer, d: &BodyDef, c: Controls, ticks: i32) -> Vec<Event> {
        let mut all = Vec::new();
        for _ in 0..ticks {
            all.extend(p.step(d, &c));
        }
        all
    }

    fn drive() -> Controls {
        Controls {
            drive: true,
            ..Controls::default()
        }
    }

    #[test]
    fn a_tuck_has_less_than_half_the_flip_inertia_of_a_layout() {
        let d = def();
        let tuck = d.flip_inertia(d.pose(Pose::Tuck));
        let layout = d.flip_inertia(d.pose(Pose::Layout));
        assert_eq!(layout, ONE);
        assert!(tuck < ONE / 2, "tuck inertia {tuck}");
        assert!(tuck > ONE / 8, "tuck inertia {tuck}");
    }

    #[test]
    fn standing_on_the_bed_sinks_it_by_gravity_over_the_spring() {
        // g / k = 0.002716 / 0.0305 = 0.089 m
        let d = def();
        let mut p = Performer::on_bed(&d, 0);
        run(&mut p, &d, Controls::default(), 600);
        let sag = BED_Y - p.lowest_in(&d, &p.shape);
        let want = fx::div(GRAVITY, BED_K);
        assert!((sag - want).abs() < int(1) / 50, "sag {sag} want {want}");
        assert_eq!(p.phase, Phase::Bed(Posture::Feet));
    }

    #[test]
    fn holding_the_drive_key_bounces_higher_each_time_up_to_the_cap() {
        let d = def();
        let mut p = Performer::on_bed(&d, 0);
        let mut peaks = Vec::new();
        for _ in 0..3000 {
            for e in p.step(&d, &drive()) {
                if let Event::Peak(h) = e {
                    peaks.push(h);
                }
            }
        }
        assert!(peaks.len() > 6, "{peaks:?}");
        assert!(peaks[1] > peaks[0] && peaks[3] > peaks[2], "{peaks:?}");
        let top = *peaks.iter().max().unwrap();
        assert!(top <= MAX_BOUNCE + int(1), "top {top}");
        assert!(top > int(5), "top {top}");
    }

    #[test]
    fn letting_go_of_the_drive_key_dies_the_bounce_down() {
        let d = def();
        let mut p = Performer::on_bed(&d, 0);
        run(&mut p, &d, drive(), 600);
        let mut last = 0;
        for e in run(&mut p, &d, Controls::default(), 900) {
            if let Event::Peak(h) = e {
                last = h;
            }
        }
        let mut later = 0;
        for e in run(&mut p, &d, Controls::default(), 900) {
            if let Event::Peak(h) = e {
                later = h;
            }
        }
        assert!(later < last, "{later} !< {last}");
    }

    /// Bounces with the drive key until a peak is at least `h`, then lands.
    fn bounce_to(p: &mut Performer, d: &BodyDef, h: i32) {
        for _ in 0..4000 {
            let ev = p.step(d, &drive());
            if p.phase == Phase::Bed(Posture::Feet) && p.vel.y < 0 && p.flight.peak - BED_Y > h {
                return;
            }
            let _ = ev;
        }
        panic!("never reached {h}");
    }

    fn first_landing(p: &mut Performer, d: &BodyDef, c: Controls, takeoff: Controls) -> Option<Landing> {
        // through the push with `takeoff` held, then `c` in the air
        for _ in 0..600 {
            let pushing = p.vel.y >= 0;
            // kick out of the tuck once three quarters of the turn is done
            let kicked = (p.flip - p.flight.flip0).abs() > 3 * QUARTER;
            let held = if p.phase == Phase::Air && kicked {
                Controls::default()
            } else if p.phase == Phase::Air {
                c
            } else if pushing {
                takeoff
            } else {
                drive()
            };
            for e in p.step(d, &held) {
                match e {
                    Event::Landed(l) if l.flight.ticks > 0 && p.flight.ticks > 0 => return Some(l),
                    Event::Crashed(_) => return None,
                    _ => {}
                }
            }
        }
        None
    }

    #[test]
    fn a_back_tuck_turns_once_backward_and_lands_on_the_feet() {
        let d = def();
        let mut p = Performer::on_bed(&d, 0);
        bounce_to(&mut p, &d, int(3));
        // wait for the bottom, then push with S and tuck in the air
        let takeoff = Controls {
            drive: true,
            flip: 1,
            ..Controls::default()
        };
        let air = Controls {
            tuck: true,
            ..Controls::default()
        };
        let l = first_landing(&mut p, &d, air, takeoff).expect("landed");
        assert_eq!(l.quarters, 4, "{l:?}");
        assert_eq!(l.to, Posture::Feet);
        assert!(l.off < 40, "{l:?}");
    }

    #[test]
    fn the_same_takeoff_in_a_layout_turns_less_than_in_a_tuck() {
        let d = def();
        let takeoff = Controls {
            drive: true,
            flip: 1,
            ..Controls::default()
        };
        let mut a = Performer::on_bed(&d, 0);
        bounce_to(&mut a, &d, int(3));
        let mut b = a.clone();
        // the same takeoff, then a tuck in one and a layout in the other,
        // for the same thirty ticks of flight
        let tick = |p: &mut Performer, air: Controls| {
            for _ in 0..200 {
                let c = if p.phase == Phase::Air { air } else { takeoff };
                p.step(&d, &c);
                if p.phase == Phase::Air && p.flight.ticks == 30 {
                    return p.flip - p.flight.flip0;
                }
            }
            panic!("no flight");
        };
        let tucked = tick(
            &mut a,
            Controls {
                tuck: true,
                ..Controls::default()
            },
        );
        let laid = tick(&mut b, Controls::default());
        assert!(tucked > laid * 3 / 2, "tuck {tucked} layout {laid}");
    }

    #[test]
    fn landing_on_the_head_is_a_crash_and_the_body_goes_limp() {
        let d = def();
        let mut p = Performer::on_bed(&d, 0);
        bounce_to(&mut p, &d, int(2));
        let takeoff = Controls {
            drive: true,
            flip: 1,
            ..Controls::default()
        };
        // a half turn, straight: lands on the head
        let mut crashed = false;
        for _ in 0..600 {
            let c = if p.phase == Phase::Air {
                Controls::default()
            } else {
                takeoff
            };
            for e in p.step(&d, &c) {
                if let Event::Crashed(_) = e {
                    crashed = true;
                }
            }
            if crashed {
                break;
            }
            if let Phase::Air = p.phase {
                // hold the turn near half a turn at landing time
                if fx::wrap(p.flip).abs() > HALF - deg(30) {
                    p.flip_l = 0;
                }
            }
        }
        assert!(crashed);
        assert!(p.bailed());
        // the ragdoll falls, then the performer is back on the bed
        run(&mut p, &d, Controls::default(), BAIL_TICKS + RISE_TICKS + 2);
        assert_eq!(p.phase, Phase::Bed(Posture::Feet));
    }

    #[test]
    fn the_ragdoll_keeps_its_sticks_near_their_lengths_and_comes_to_rest() {
        let d = def();
        let mut p = Performer::on_bed(&d, 0);
        p.com.y += int(3);
        let frame = p.frame();
        p.body.place(&p.shape, &frame);
        p.body.prev = p.body.pos;
        for _ in 0..240 {
            p.body.fall(&d);
        }
        for &(a, b, len) in &d.sticks {
            let now = p.body.pos[a].dist(p.body.pos[b]);
            assert!((now - len).abs() < len / 5, "{a}-{b}: {now} vs {len}");
        }
        assert!(p.body.lowest(&d) >= -int(1) / 50);
    }
}
