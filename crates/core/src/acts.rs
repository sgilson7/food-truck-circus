//! The show's other acts and the bed's cloth: the membrane, the popcorn
//! machine, the tower of tables and chairs, the trapeze and juggling. Each is
//! a small state machine stepped once a tick by the game; none reads input
//! except through the `Controls` the game hands it.

use crate::balance::{
    BED_HALF_X, BED_HALF_Z, BED_Y, BED_Z, GRAVITY, TRUCK_HALF_X, TRUCK_HALF_Z, TRUCK_ROOF, TRUCK_X, TRUCK_Z,
};
use crate::body::{cm, Shape, POINTS};
use crate::fx::{self, deg, int, ratio, ONE, TURN};
use crate::geom::{v3, V3};
use crate::rng::Rng;

// ---- the bed's cloth -------------------------------------------------------

pub const CLOTH_X: usize = 13;
pub const CLOTH_Z: usize = 7;

/// The bed's cloth: a grid of particles that move only up and down, each
/// held to its neighbours by springs and to the frame at the edges. The
/// performer's contact presses a dent into it; the springs carry the wave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cloth {
    pub h: Vec<i32>,
    prev: Vec<i32>,
}

impl Default for Cloth {
    fn default() -> Self {
        Cloth {
            h: vec![0; CLOTH_X * CLOTH_Z],
            prev: vec![0; CLOTH_X * CLOTH_Z],
        }
    }
}

impl Cloth {
    pub fn node(i: usize, k: usize) -> (i32, i32) {
        let x = -BED_HALF_X + (2 * BED_HALF_X) * i as i32 / (CLOTH_X as i32 - 1);
        let z = BED_Z - BED_HALF_Z + (2 * BED_HALF_Z) * k as i32 / (CLOTH_Z as i32 - 1);
        (x, z)
    }

    /// One tick; `press` is a point on the bed and how deep it is pressed.
    pub fn step(&mut self, press: Option<(i32, i32, i32)>) {
        let n = CLOTH_X * CLOTH_Z;
        for j in 0..n {
            let v = self.h[j] - self.prev[j];
            self.prev[j] = self.h[j];
            self.h[j] += v - v / 12;
        }
        for _ in 0..3 {
            for k in 1..CLOTH_Z - 1 {
                for i in 1..CLOTH_X - 1 {
                    let j = k * CLOTH_X + i;
                    let avg = (self.h[j - 1] + self.h[j + 1] + self.h[j - CLOTH_X] + self.h[j + CLOTH_X]) / 4;
                    self.h[j] += (avg - self.h[j]) / 3;
                }
            }
        }
        if let Some((px, pz, depth)) = press {
            let reach = ratio(13, 10);
            for k in 1..CLOTH_Z - 1 {
                for i in 1..CLOTH_X - 1 {
                    let (x, z) = Cloth::node(i, k);
                    let d = v3(x - px, 0, z - pz).len();
                    if d < reach {
                        let want = -fx::mul(depth, ONE - fx::div(d, reach));
                        let j = k * CLOTH_X + i;
                        self.h[j] = self.h[j].min(want);
                    }
                }
            }
        }
    }

    pub fn at(&self, i: usize, k: usize) -> V3 {
        let (x, z) = Cloth::node(i, k);
        v3(x, BED_Y + self.h[k * CLOTH_X + i], z)
    }
}

// ---- popcorn -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kernel {
    pub pos: V3,
    pub vel: V3,
    /// Ticks lying still on a floor, after which it is swept away.
    pub rest: i32,
}

/// The popcorn machine in the truck's window overflows in bursts (the show's
/// gag); a kernel touched by any part of the performer is caught.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Popcorn {
    pub kernels: Vec<Kernel>,
    pub caught: i32,
    next_burst: i32,
}

pub const KERNEL_REACH: i32 = ratio(45, 100);
const KERNEL_LIFE: i32 = 240;

/// Where the machine's spout is: the truck's serving window, over the bed.
pub fn spout() -> V3 {
    v3(TRUCK_X + int(1), ratio(22, 10), TRUCK_Z + TRUCK_HALF_Z)
}

impl Popcorn {
    pub fn step(&mut self, rng: &mut Rng, body: &Shape) -> i32 {
        self.next_burst -= 1;
        if self.next_burst <= 0 {
            self.next_burst = rng.range(70, 130);
            let s = spout();
            for _ in 0..rng.range(6, 10) {
                self.kernels.push(Kernel {
                    pos: s,
                    vel: v3(
                        rng.range(-ratio(4, 100), ratio(4, 100)),
                        rng.range(ratio(10, 100), ratio(17, 100)),
                        rng.range(ratio(1, 100), ratio(4, 100)),
                    ),
                    rest: 0,
                });
            }
        }
        let mut caught = 0;
        let light = GRAVITY / 3;
        self.kernels.retain_mut(|k| {
            if body.iter().take(POINTS).any(|p| p.dist(k.pos) < KERNEL_REACH) {
                caught += 1;
                return false;
            }
            k.vel.y -= light;
            k.vel = k.vel.sub(k.vel.scale(ratio(2, 100)));
            k.pos = k.pos.add(k.vel);
            let on_bed = k.pos.x.abs() <= BED_HALF_X && (k.pos.z - BED_Z).abs() <= BED_HALF_Z;
            let floor = if on_bed { BED_Y } else { 0 };
            if k.pos.y < floor + cm(3) {
                k.pos.y = floor + cm(3);
                k.vel = V3::ZERO;
                k.rest += 1;
            }
            k.rest < KERNEL_LIFE
        });
        self.caught += caught;
        caught
    }
}

// ---- the tower ---------------------------------------------------------------

pub const TABLE_HEIGHT: i32 = ratio(8, 10);
pub const CHAIR_HEIGHT: i32 = ratio(55, 100);
/// The tower stands on the truck's roof, toward one end.
pub fn tower_base() -> V3 {
    v3(TRUCK_X + TRUCK_HALF_X - int(1), TRUCK_ROOF, TRUCK_Z)
}
/// Lean, spin and gusts are kept in 1/256ths of an angle unit, so a slow
/// lean still moves.
pub const FINE: i32 = 256;
/// Past this lean the tower goes over.
pub const TOWER_LIMIT: i32 = deg(14) * FINE;
const LIFT_TICKS: i32 = 45;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TowerPhase {
    Stacking,
    /// A chair on its way up, ticks so far.
    Lifting(i32),
    Handstand(i32),
    Fallen(i32),
    Done,
}

/// The tower of a table and chairs on the truck's roof: an inverted
/// pendulum that leans further the further it leans, nudged by gusts, held
/// up by the player shifting weight (the arrow or WASD keys push the top the
/// way they point).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tower {
    pub chairs: i32,
    pub lean: [i32; 2],
    pub spin: [i32; 2],
    gust: [i32; 2],
    gust_left: i32,
    pub phase: TowerPhase,
}

pub enum TowerEvent {
    Placed(i32),
    Handstand,
    Held,
    Fell,
}

impl Tower {
    pub fn new() -> Tower {
        Tower {
            chairs: 1,
            lean: [0; 2],
            spin: [0; 2],
            gust: [0; 2],
            gust_left: 0,
            phase: TowerPhase::Stacking,
        }
    }

    /// Height of the top seat above the roof.
    pub fn height(&self) -> i32 {
        TABLE_HEIGHT + self.chairs * CHAIR_HEIGHT
    }

    /// Lean in degrees, the larger of the two axes.
    pub fn lean_degrees(&self) -> i32 {
        let a = self.lean[0].abs().max(self.lean[1].abs()) / FINE;
        a * 360 / TURN
    }

    /// One tick. `push` is the player's weight shift (x, z), each −1, 0 or 1;
    /// `act` adds a chair, or starts the handstand once `target` are up.
    pub fn step(
        &mut self,
        rng: &mut Rng,
        push: [i32; 2],
        act: bool,
        target: i32,
        hold: i32,
    ) -> Vec<TowerEvent> {
        let mut ev = Vec::new();
        if let TowerPhase::Fallen(t) = self.phase {
            // over it goes, faster the further
            for a in 0..2 {
                let s = self.lean[a].signum().max(if a == 0 { 1 } else { 0 });
                self.spin[a] += s * deg(1) * FINE / 6;
                self.lean[a] = (self.lean[a] + self.spin[a]).clamp(-deg(85) * FINE, deg(85) * FINE);
            }
            self.phase = TowerPhase::Fallen(t + 1);
            return ev;
        }
        if self.phase == TowerPhase::Done {
            return ev;
        }
        self.gust_left -= 1;
        if self.gust_left <= 0 {
            self.gust_left = rng.range(50, 110);
            let g = 18 + self.chairs * 2;
            self.gust = [rng.range(-g, g), rng.range(-g, g)];
        }
        let hand = matches!(self.phase, TowerPhase::Handstand(_));
        // λ² of an inverted pendulum, g over the height, made gentler; a
        // handstand is twice as touchy.
        let height = self.height() + int(1);
        let mut lam2 = fx::div(GRAVITY, height) * 2 / 5;
        if hand {
            lam2 *= 2;
        }
        let control = 150;
        for a in 0..2 {
            let fall = fx::mul(self.lean[a], lam2);
            self.spin[a] += fall + self.gust[a] + push[a] * control;
            self.spin[a] -= self.spin[a] / 40;
            self.lean[a] += self.spin[a];
        }
        if self.lean[0].abs() > TOWER_LIMIT || self.lean[1].abs() > TOWER_LIMIT {
            self.phase = TowerPhase::Fallen(0);
            ev.push(TowerEvent::Fell);
            return ev;
        }
        match self.phase {
            TowerPhase::Stacking => {
                if act && self.chairs < target {
                    self.phase = TowerPhase::Lifting(0);
                } else if act && self.chairs >= target {
                    self.phase = TowerPhase::Handstand(0);
                    ev.push(TowerEvent::Handstand);
                }
            }
            TowerPhase::Lifting(t) => {
                if t + 1 >= LIFT_TICKS {
                    self.chairs += 1;
                    self.phase = TowerPhase::Stacking;
                    // the new chair's weight sets the tower swaying a little
                    self.spin[0] += rng.range(-40, 40);
                    self.spin[1] += rng.range(-40, 40);
                    ev.push(TowerEvent::Placed(self.grade()));
                } else {
                    self.phase = TowerPhase::Lifting(t + 1);
                }
            }
            TowerPhase::Handstand(t) => {
                if t + 1 >= hold {
                    self.phase = TowerPhase::Done;
                    ev.push(TowerEvent::Held);
                } else {
                    self.phase = TowerPhase::Handstand(t + 1);
                }
            }
            _ => {}
        }
        ev
    }

    /// How steady the tower is now, out of 100: six points a degree of lean.
    pub fn grade(&self) -> i32 {
        (100 - self.lean_degrees() * 6).clamp(0, 100)
    }

    /// The lean as two angles in angle units, for drawing.
    pub fn tilt(&self) -> [i32; 2] {
        [self.lean[0] / FINE, self.lean[1] / FINE]
    }
}

impl Default for Tower {
    fn default() -> Self {
        Tower::new()
    }
}

// ---- the trapeze ---------------------------------------------------------------

/// The trapeze hangs over the bed and swings along it (x).
pub fn trapeze_pivot() -> V3 {
    v3(-ratio(9, 10), ratio(78, 10), BED_Z)
}
pub const ROPE: i32 = ratio(34, 10);
/// From the hands to the centre of mass, in the layout (arms overhead).
pub const HANG: i32 = ratio(105, 100);
/// 2π in Q16, as `fx` keeps it.
const TWO_PI: i64 = 411_775;
const PUMP: i32 = 7;
/// The highest swing pumping can reach, radians Q16 (about 75°).
const SWING_MAX: i32 = ratio(131, 100);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trapeze {
    /// Swing angle and its rate, in radians (Q16) and radians a tick.
    pub phi: i32,
    pub omega: i32,
    pub holding: bool,
}

impl Trapeze {
    /// Hanging, pulled back as if from the platform.
    pub fn new() -> Trapeze {
        Trapeze {
            phi: -ratio(6, 10),
            omega: 0,
            holding: true,
        }
    }

    pub fn angle(&self) -> i32 {
        (self.phi as i64 * TURN as i64 / TWO_PI) as i32
    }

    pub fn length(&self) -> i32 {
        ROPE + HANG
    }

    /// Where the bar is.
    pub fn bar(&self) -> V3 {
        let a = self.angle();
        trapeze_pivot().add(v3(fx::mul(ROPE, fx::sin(a)), -fx::mul(ROPE, fx::cos(a)), 0))
    }

    /// Where the performer's centre of mass hangs.
    pub fn com(&self) -> V3 {
        let a = self.angle();
        let l = self.length();
        trapeze_pivot().add(v3(fx::mul(l, fx::sin(a)), -fx::mul(l, fx::cos(a)), 0))
    }

    /// The centre of mass's velocity, metres a tick.
    pub fn velocity(&self) -> V3 {
        let a = self.angle();
        let s = fx::mul(self.omega, self.length());
        v3(fx::mul(s, fx::cos(a)), fx::mul(s, fx::sin(a)), 0)
    }

    /// The swing's rate as flip momentum: turn units a tick.
    pub fn spin(&self) -> i32 {
        (self.omega as i64 * TURN as i64 / TWO_PI) as i32
    }

    /// One tick of the pendulum. Pumping with the swing (W while it swings
    /// forward, S while it swings back) adds energy, as Vagrancy's motors do.
    pub fn step(&mut self, pump: i32) {
        let g_over_l = fx::div(GRAVITY, self.length());
        let a = self.angle();
        self.omega -= fx::mul(g_over_l, fx::sin(a));
        if pump != 0 && pump == self.omega.signum() && self.phi.abs() < SWING_MAX {
            self.omega += pump * PUMP;
        }
        self.omega -= self.omega / 2000;
        self.phi += self.omega;
    }
}

impl Default for Trapeze {
    fn default() -> Self {
        Trapeze::new()
    }
}

// ---- juggling ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ball {
    /// The hand it left (0 left, 1 right) and the tick it left.
    pub from: usize,
    pub thrown: i32,
    /// Dropped: it falls from where it was missed.
    pub dropped: Option<i32>,
}

pub const CATCH_WINDOW: i32 = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Juggle {
    pub balls: Vec<Ball>,
    pub flight: i32,
    pub tick: i32,
    pub catches: i32,
    pub streak: i32,
}

pub enum JuggleEvent {
    Caught(i32),
    Dropped,
}

impl Juggle {
    pub fn new(balls: usize) -> Juggle {
        let flight = if balls >= 3 { 66 } else { 80 };
        let gap = flight / balls as i32;
        let start = 100;
        Juggle {
            balls: (0..balls)
                .map(|i| Ball {
                    from: (i + 1) % 2,
                    thrown: start + gap * i as i32 - flight,
                    dropped: None,
                })
                .collect(),
            flight,
            tick: 0,
            catches: 0,
            streak: 0,
        }
    }

    /// When a ball in the air reaches its hand.
    fn arrives(&self, b: &Ball) -> i32 {
        b.thrown + self.flight
    }

    /// One tick; `press` holds whether each hand was pressed this tick.
    pub fn step(&mut self, press: [bool; 2]) -> Vec<JuggleEvent> {
        let mut ev = Vec::new();
        self.tick += 1;
        let now = self.tick;
        for hand in 0..2 {
            if !press[hand] {
                continue;
            }
            // the ball in the air nearest its arrival at this hand
            let flight = self.flight;
            let best = self
                .balls
                .iter_mut()
                .filter(|b| b.dropped.is_none() && 1 - b.from == hand)
                .min_by_key(|b| (b.thrown + flight - now).abs());
            if let Some(b) = best {
                let off = (b.thrown + flight - now).abs();
                if off <= CATCH_WINDOW {
                    b.from = hand;
                    b.thrown = now;
                    self.catches += 1;
                    self.streak += 1;
                    ev.push(JuggleEvent::Caught((100 - off * 6).clamp(0, 100)));
                }
            }
        }
        for i in 0..self.balls.len() {
            let b = self.balls[i];
            if b.dropped.is_none() && now > self.arrives(&b) + CATCH_WINDOW {
                self.balls[i].dropped = Some(now);
                self.streak = 0;
                ev.push(JuggleEvent::Dropped);
            }
            if let Some(t) = b.dropped {
                // picked up again after a second, and thrown back in
                if now - t > 60 {
                    self.balls[i] = Ball {
                        from: b.from,
                        thrown: now,
                        dropped: None,
                    };
                }
            }
        }
        ev
    }

    /// Where a ball is, given where the two hands are.
    pub fn ball_at(&self, b: &Ball, hands: [V3; 2]) -> V3 {
        let to = 1 - b.from;
        if let Some(t) = b.dropped {
            // it fell past the hand and keeps falling to the ground
            let since = self.tick - t;
            let y = hands[to].y - GRAVITY * since * since / 2;
            return v3(hands[to].x, y.max(cm(4)), hands[to].z + cm(10));
        }
        let s = ((self.tick - b.thrown) * ONE / self.flight).clamp(0, ONE + ONE / 8);
        let p = hands[b.from].lerp(hands[to], s);
        let lift = fx::mul(int(4) * 9 / 10, fx::mul(s, ONE - s));
        p.add(v3(0, lift, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cloth_dents_where_it_is_pressed_and_springs_back() {
        let mut c = Cloth::default();
        for _ in 0..10 {
            c.step(Some((0, BED_Z, cm(50))));
        }
        let mid = c.at(CLOTH_X / 2, CLOTH_Z / 2).y;
        assert!(mid <= BED_Y - cm(45), "{mid}");
        // the frame never moves
        assert_eq!(c.at(0, 0).y, BED_Y);
        for _ in 0..600 {
            c.step(None);
        }
        assert!((c.at(CLOTH_X / 2, CLOTH_Z / 2).y - BED_Y).abs() < cm(1));
    }

    #[test]
    fn popcorn_comes_out_of_the_window_and_is_caught_by_a_hand_in_its_way() {
        let mut p = Popcorn::default();
        let mut rng = Rng::new(3);
        let far = [v3(int(50), 0, 0); POINTS];
        p.step(&mut rng, &far);
        assert!(p.kernels.len() >= 6);
        assert_eq!(p.kernels[0].pos, spout().add(p.kernels[0].vel));
        let mut near = far;
        near[0] = p.kernels[0].pos;
        assert!(p.step(&mut rng, &near) >= 1);
        assert!(p.caught >= 1);
    }

    #[test]
    fn a_tower_left_alone_falls_and_one_held_level_stands() {
        let mut rng = Rng::new(1);
        let mut t = Tower::new();
        let mut fell = false;
        for _ in 0..2000 {
            for e in t.step(&mut rng, [0, 0], false, 4, 120) {
                if let TowerEvent::Fell = e {
                    fell = true;
                }
            }
        }
        assert!(fell);
        // a player who pushes against the lean every tick keeps it up
        let mut rng = Rng::new(1);
        let mut t = Tower::new();
        for _ in 0..2000 {
            let push = [
                -t.lean[0].signum() - t.spin[0].signum(),
                -t.lean[1].signum() - t.spin[1].signum(),
            ];
            let push = [push[0].clamp(-1, 1), push[1].clamp(-1, 1)];
            t.step(&mut rng, push, false, 4, 120);
        }
        assert!(!matches!(t.phase, TowerPhase::Fallen(_)), "{t:?}");
    }

    #[test]
    fn a_trapeze_pumped_with_the_swing_swings_higher() {
        let mut free = Trapeze::new();
        let mut pumped = Trapeze::new();
        let mut top_free = 0;
        let mut top_pumped = 0;
        for _ in 0..1200 {
            free.step(0);
            let w = pumped.omega.signum();
            pumped.step(w);
            top_free = top_free.max(free.phi.abs());
            top_pumped = top_pumped.max(pumped.phi.abs());
        }
        assert!(top_pumped > top_free + ratio(3, 10), "{top_pumped} vs {top_free}");
        // pumping against the swing takes energy away instead of adding it
        let mut against = Trapeze::new();
        for _ in 0..1200 {
            let w = -against.omega.signum();
            against.step(w);
        }
        assert!(against.phi.abs() <= top_free);
    }

    #[test]
    fn a_juggler_who_presses_on_time_never_drops() {
        let mut j = Juggle::new(3);
        let mut drops = 0;
        for _ in 0..1200 {
            let now = j.tick + 1;
            let mut press = [false; 2];
            for b in &j.balls {
                if b.dropped.is_none() && b.thrown + j.flight == now {
                    press[1 - b.from] = true;
                }
            }
            for e in j.step(press) {
                if let JuggleEvent::Dropped = e {
                    drops += 1;
                }
            }
        }
        assert_eq!(drops, 0);
        assert!(j.catches > 40, "{}", j.catches);
        // and one who never presses drops every ball
        let mut j = Juggle::new(3);
        let mut drops = 0;
        for _ in 0..160 {
            drops += j.step([false; 2]).len();
        }
        assert_eq!(drops, 3);
    }
}
