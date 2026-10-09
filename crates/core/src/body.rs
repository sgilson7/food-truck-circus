//! The performer's body: Vagrancy's particles and sticks, in three dimensions.
//!
//! In control, the flight model places every particle from the pose and the
//! orientation (`Body::place`), and each particle's previous position is
//! where it was a tick ago, so the body always carries its own velocity. On
//! a crash the same particles go to the Verlet solver (`Body::fall`):
//! gravity, sticks relaxed in a fixed order, the ground, the bed and the
//! truck, exactly as a Vagrancy fighter goes limp. Nothing here reads input.

use crate::balance::{self, BED_Y, GRAVITY, ITERATIONS};
use crate::fx::{self, ONE};
use crate::geom::{v3, Affine, V3};
use crate::stage;
use serde::Deserialize;
use std::collections::BTreeMap;

pub const POINTS: usize = 15;

/// Particle indices, in the order of `points` in data/body.json.
pub const HEAD: usize = 0;
pub const NECK: usize = 1;
pub const SHOULDER_L: usize = 2;
pub const SHOULDER_R: usize = 3;
pub const HAND_L: usize = 6;
pub const HAND_R: usize = 7;
pub const PELVIS: usize = 8;
pub const HIP_L: usize = 9;
pub const HIP_R: usize = 10;
pub const FOOT_L: usize = 13;
pub const FOOT_R: usize = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pose {
    Stand = 0,
    Layout = 1,
    Tuck = 2,
    Pike = 3,
    Crouch = 4,
    Juggle = 5,
    Arch = 6,
}

impl Pose {
    pub const ALL: [Pose; 7] = [
        Pose::Stand,
        Pose::Layout,
        Pose::Tuck,
        Pose::Pike,
        Pose::Crouch,
        Pose::Juggle,
        Pose::Arch,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Pose::Stand => "stand",
            Pose::Layout => "layout",
            Pose::Tuck => "tuck",
            Pose::Pike => "pike",
            Pose::Crouch => "crouch",
            Pose::Juggle => "juggle",
            Pose::Arch => "arch",
        }
    }
}

pub type Shape = [V3; POINTS];

#[derive(Deserialize)]
struct BodyFile {
    points: Vec<String>,
    mass: BTreeMap<String, i32>,
    radius: BTreeMap<String, i32>,
    sticks: Vec<(String, String)>,
    bones: Vec<(String, String, String, i32)>,
    poses: BTreeMap<String, BTreeMap<String, [i32; 3]>>,
}

/// A limb drawn as a box between two particles, in a costume colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bone {
    pub a: usize,
    pub b: usize,
    pub paint: String,
    pub half_width: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyDef {
    pub mass: [i32; POINTS],
    pub radius: [i32; POINTS],
    /// Pairs and their rest lengths, from the stand pose.
    pub sticks: Vec<(usize, usize, i32)>,
    pub bones: Vec<Bone>,
    /// Each pose, centred on its own centre of mass, in metres.
    pub poses: Vec<Shape>,
    /// The flip inertia of the layout: the unit every other inertia is a share of.
    pub unit_inertia: i64,
}

pub fn cm(n: i32) -> i32 {
    n * ONE / 100
}

impl BodyDef {
    pub fn parse(text: &str) -> Result<BodyDef, String> {
        let f: BodyFile = serde_json::from_str(text).map_err(|e| format!("body.json: {e}"))?;
        if f.points.len() != POINTS {
            return Err(format!(
                "body.json: {} points, the core expects {POINTS}",
                f.points.len()
            ));
        }
        let index = |n: &str| {
            f.points
                .iter()
                .position(|p| p == n)
                .ok_or_else(|| format!("body.json: no point named {n}"))
        };
        let mut mass = [0; POINTS];
        let mut radius = [0; POINTS];
        for (i, name) in f.points.iter().enumerate() {
            mass[i] = *f
                .mass
                .get(name)
                .ok_or_else(|| format!("body.json: no mass for {name}"))?;
            radius[i] = cm(*f
                .radius
                .get(name)
                .ok_or_else(|| format!("body.json: no radius for {name}"))?);
        }
        let mut poses = Vec::new();
        for pose in Pose::ALL {
            let p = f
                .poses
                .get(pose.name())
                .ok_or_else(|| format!("body.json: no pose {}", pose.name()))?;
            let mut s = [V3::ZERO; POINTS];
            for (i, name) in f.points.iter().enumerate() {
                let [x, y, z] = p
                    .get(name)
                    .ok_or_else(|| format!("body.json: pose {} lacks {name}", pose.name()))?;
                s[i] = v3(cm(*x), cm(*y), cm(*z));
            }
            poses.push(centred(&s, &mass));
        }
        let mut sticks = Vec::new();
        for (a, b) in &f.sticks {
            let (a, b) = (index(a)?, index(b)?);
            let stand = &poses[Pose::Stand as usize];
            sticks.push((a, b, stand[a].dist(stand[b])));
        }
        let mut bones = Vec::new();
        for (a, b, paint, w) in &f.bones {
            bones.push(Bone {
                a: index(a)?,
                b: index(b)?,
                paint: paint.clone(),
                half_width: cm(*w),
            });
        }
        let mut def = BodyDef {
            mass,
            radius,
            sticks,
            bones,
            poses,
            unit_inertia: 1,
        };
        def.unit_inertia = def.flip_inertia_raw(&def.poses[Pose::Layout as usize]);
        Ok(def)
    }

    pub fn pose(&self, p: Pose) -> &Shape {
        &self.poses[p as usize]
    }

    fn flip_inertia_raw(&self, s: &Shape) -> i64 {
        let mut sum = 0i64;
        for i in 0..POINTS {
            let (y, z) = (s[i].y as i64, s[i].z as i64);
            sum += self.mass[i] as i64 * ((y * y + z * z) >> fx::FRAC_BITS);
        }
        sum.max(1)
    }

    /// The inertia about the body's left-right axis, as a share of the
    /// layout's (Q16): a tuck is well under one, so it spins faster.
    pub fn flip_inertia(&self, s: &Shape) -> i32 {
        ((self.flip_inertia_raw(s) << fx::FRAC_BITS) / self.unit_inertia) as i32
    }

    /// The inertia about the long axis, as a share of the layout's flip inertia.
    pub fn twist_inertia(&self, s: &Shape) -> i32 {
        let mut sum = 0i64;
        for i in 0..POINTS {
            let (x, z) = (s[i].x as i64, s[i].z as i64);
            sum += self.mass[i] as i64 * ((x * x + z * z) >> fx::FRAC_BITS);
        }
        // Twisting reads arms and legs as close to the axis as a real body
        // holds them, and a body is never a line: a floor keeps a layout's
        // twist inertia from vanishing.
        let floor = self.unit_inertia / 12;
        (((sum.max(floor)) << fx::FRAC_BITS) / self.unit_inertia) as i32
    }

    pub fn total_mass(&self) -> i32 {
        self.mass.iter().sum()
    }
}

/// A shape moved so its centre of mass is the origin.
fn centred(s: &Shape, mass: &[i32; POINTS]) -> Shape {
    let total: i64 = mass.iter().map(|m| *m as i64).sum();
    let (mut x, mut y, mut z) = (0i64, 0i64, 0i64);
    for i in 0..POINTS {
        x += s[i].x as i64 * mass[i] as i64;
        y += s[i].y as i64 * mass[i] as i64;
        z += s[i].z as i64 * mass[i] as i64;
    }
    let c = v3((x / total) as i32, (y / total) as i32, (z / total) as i32);
    let mut out = *s;
    for p in out.iter_mut() {
        *p = p.sub(c);
    }
    out
}

/// Moves a shape toward a target by a share of the way.
pub fn blend(from: &Shape, to: &Shape, share: i32) -> Shape {
    let mut out = *from;
    for i in 0..POINTS {
        out[i] = from[i].lerp(to[i], share);
    }
    out
}

/// The particles of a body, now and a tick ago.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body {
    pub pos: Shape,
    pub prev: Shape,
}

impl Body {
    pub fn at(shape: &Shape, frame: &Affine) -> Body {
        let mut pos = [V3::ZERO; POINTS];
        for i in 0..POINTS {
            pos[i] = frame.apply(shape[i]);
        }
        Body { pos, prev: pos }
    }

    /// In control: every particle where the pose and the frame put it.
    pub fn place(&mut self, shape: &Shape, frame: &Affine) {
        self.prev = self.pos;
        for i in 0..POINTS {
            self.pos[i] = frame.apply(shape[i]);
        }
    }

    pub fn lowest(&self, def: &BodyDef) -> i32 {
        (0..POINTS)
            .map(|i| self.pos[i].y - def.radius[i])
            .min()
            .unwrap_or(0)
    }

    pub fn centre(&self, def: &BodyDef) -> V3 {
        let total = def.total_mass() as i64;
        let (mut x, mut y, mut z) = (0i64, 0i64, 0i64);
        for i in 0..POINTS {
            x += self.pos[i].x as i64 * def.mass[i] as i64;
            y += self.pos[i].y as i64 * def.mass[i] as i64;
            z += self.pos[i].z as i64 * def.mass[i] as i64;
        }
        v3((x / total) as i32, (y / total) as i32, (z / total) as i32)
    }

    /// Velocity of the centre of mass, metres a tick.
    pub fn velocity(&self, def: &BodyDef) -> V3 {
        let then = Body {
            pos: self.prev,
            prev: self.prev,
        };
        self.centre(def).sub(then.centre(def))
    }

    /// One tick limp: Verlet integration, then the sticks and the stage,
    /// relaxed in a fixed order (Vagrancy's `integrate` and `relax`).
    pub fn fall(&mut self, def: &BodyDef) {
        for i in 0..POINTS {
            let v = self.pos[i].sub(self.prev[i]);
            let v = v.sub(v.scale(balance::DRAG)).clamp_len(balance::SPEED_CAP);
            self.prev[i] = self.pos[i];
            self.pos[i] = self.pos[i].add(v).sub(v3(0, GRAVITY, 0));
        }
        for _ in 0..ITERATIONS {
            for &(a, b, len) in &def.sticks {
                self.stick(def, a, b, len);
            }
            for i in 0..POINTS {
                self.collide(def, i);
            }
        }
    }

    /// Holds two particles at their rest length, each moved by the other's
    /// share of the mass, so the pair's momentum is unchanged.
    fn stick(&mut self, def: &BodyDef, a: usize, b: usize, len: i32) {
        let d = self.pos[b].sub(self.pos[a]);
        let now = d.len();
        if now == 0 {
            return;
        }
        let err = now - len;
        let (ma, mb) = (def.mass[a] as i64, def.mass[b] as i64);
        let shift = d.scale(fx::div(err, now));
        let share_a = ((mb << fx::FRAC_BITS) / (ma + mb)) as i32;
        let pa = shift.scale(share_a);
        let pb = shift.sub(pa);
        self.pos[a] = self.pos[a].add(pa);
        self.pos[b] = self.pos[b].sub(pb);
    }

    fn collide(&mut self, def: &BodyDef, i: usize) {
        let r = def.radius[i];
        let p = self.pos[i];
        let floor = stage::floor_under(p.x, p.z, self.prev[i].y - r);
        if p.y - r < floor {
            let bed = floor == BED_Y;
            self.pos[i].y = floor + r;
            let vy = p.y - self.prev[i].y;
            // The bed gives a little back; the ground and the roof hold.
            self.prev[i].y = if bed {
                self.pos[i].y + vy / 3
            } else {
                self.pos[i].y
            };
            let keep = balance::GRIP;
            self.prev[i].x = p.x - fx::mul(p.x - self.prev[i].x, keep);
            self.prev[i].z = p.z - fx::mul(p.z - self.prev[i].z, keep);
        }
        // the truck's sides
        if let Some(out) = stage::out_of_truck(self.pos[i], r) {
            self.pos[i] = out;
        }
    }
}
