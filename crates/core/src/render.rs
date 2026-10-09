//! What the page draws: one 3×4 matrix, a colour and a glow per shape,
//! grouped by mesh so each mesh is one instanced draw (Iron Vector's
//! contract, kept exactly so its `gl.js` draws this game unchanged).

use crate::body::{self, BodyDef, POINTS};
use crate::fx::{self, deg, int, ONE};
use crate::geom::{v3, Affine, Mat4, V3};
use crate::mesh::Mesh;
use crate::tour::{rgb, Costume};

pub const INSTANCE_VALUES: usize = 16;
pub const DRAW_VALUES: usize = 5;
pub const BYTES_PER_VALUE: usize = 4;
pub const STATIC_BUFFER: u32 = 0;
pub const DYNAMIC_BUFFER: u32 = 1;

pub const FOV: i32 = deg(62);
/// The near and far planes. A near plane of 10 cm left too little depth
/// precision far off: when the camera rose with a high bounce, water, car
/// parks and the stage's apron lying a few centimetres over the ground
/// flickered through it (Sam, 2026-10-09). 40 cm in front of the camera is
/// nearer than anything it is ever asked to show.
pub const NEAR: i32 = ONE * 2 / 5;
pub const FAR: i32 = int(2400);

pub type Rgb = [i32; 3];

pub struct Batch {
    meshes: Vec<Vec<i32>>,
}

impl Default for Batch {
    fn default() -> Self {
        Batch::new()
    }
}

impl Batch {
    pub fn new() -> Batch {
        Batch {
            meshes: vec![Vec::new(); Mesh::ALL.len()],
        }
    }

    pub fn push(&mut self, mesh: Mesh, a: &Affine, rgb: Rgb, glow: i32) {
        self.meshes[mesh as usize].extend_from_slice(&[
            a.c0.x, a.c0.y, a.c0.z, a.t.x, a.c1.x, a.c1.y, a.c1.z, a.t.y, a.c2.x, a.c2.y, a.c2.z, a.t.z,
            rgb[0], rgb[1], rgb[2], glow,
        ]);
    }

    /// An axis-aligned shape: centre and full size.
    pub fn shape(&mut self, mesh: Mesh, at: V3, size: V3, rgb: Rgb) {
        self.push(mesh, &boxed(at, size), rgb, 0);
    }

    /// A shape standing on the ground at `(x, z)`, its foot at `y`.
    pub fn stand(&mut self, mesh: Mesh, x: i32, y: i32, z: i32, size: V3, rgb: Rgb) {
        self.shape(mesh, v3(x, y + size.y / 2, z), size, rgb);
    }

    /// A box from `a` to `b`, `w` across.
    pub fn rod(&mut self, mesh: Mesh, a: V3, b: V3, w: i32, rgb: Rgb, glow: i32) {
        if let Some(af) = segment(a, b, w, w) {
            self.push(mesh, &af, rgb, glow);
        }
    }

    pub fn count(&self) -> usize {
        self.meshes.iter().map(|m| m.len() / INSTANCE_VALUES).sum()
    }

    /// The instance values, and one draw per mesh that has any.
    pub fn finish(self, buffer: u32) -> (Vec<i32>, Vec<u32>) {
        let mut values = Vec::new();
        let mut draws = Vec::new();
        for mesh in Mesh::ALL {
            let v = &self.meshes[mesh as usize];
            if v.is_empty() {
                continue;
            }
            let offset = (values.len() * BYTES_PER_VALUE) as u32;
            let count = (v.len() / INSTANCE_VALUES) as u32;
            draws.extend_from_slice(&[buffer, mesh as u32, offset, count, 0]);
            values.extend_from_slice(v);
        }
        (values, draws)
    }
}

pub fn boxed(at: V3, size: V3) -> Affine {
    Affine::translate(at).then(&Affine::scale(size))
}

/// A unit shape stretched from `a` to `b`, `w` by `d` across; its long axis
/// is the shape's y, so a cylinder runs along the segment.
pub fn segment(a: V3, b: V3, w: i32, d: i32) -> Option<Affine> {
    let dir = b.sub(a);
    let len = dir.len();
    if len == 0 {
        return None;
    }
    let y = dir.scale(fx::div(ONE, len));
    let helper = if y.y.abs() > ONE - ONE / 8 {
        v3(ONE, 0, 0)
    } else {
        V3::UP
    };
    let x = helper.cross(y).norm();
    let z = x.cross(y);
    Some(Affine {
        c0: x.scale(w),
        c1: y.scale(len),
        c2: z.scale(d),
        t: a.lerp(b, ONE / 2),
    })
}

pub fn shade(c: Rgb, pct: i32) -> Rgb {
    c.map(|v| (v * pct / 100).min(ONE))
}

pub fn mix(a: Rgb, b: Rgb, t: i32) -> Rgb {
    [
        fx::lerp(a[0], b[0], t),
        fx::lerp(a[1], b[1], t),
        fx::lerp(a[2], b[2], t),
    ]
}

/// A camera: where it is and where it looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Camera {
    pub eye: V3,
    pub target: V3,
}

impl Camera {
    pub fn matrix(&self, aspect: i32) -> Mat4 {
        let fwd = self.target.sub(self.eye).norm();
        Mat4::perspective(FOV, aspect, NEAR, FAR).mul(&Mat4::look(self.eye, fwd, V3::UP))
    }

    /// The view values the page uploads: the matrix, then the eye.
    pub fn values(&self, aspect: i32) -> Vec<i32> {
        let mut v = self.matrix(aspect).0.to_vec();
        v.extend([self.eye.x, self.eye.y, self.eye.z]);
        v
    }

    /// Where a world point lands on screen, as percentages (Q16) from the
    /// left and the top; `None` behind the camera.
    pub fn screen(&self, aspect: i32, p: V3) -> Option<(i32, i32)> {
        let [x, y, _, w] = self.matrix(aspect).project(p);
        if w <= ONE / 100 {
            return None;
        }
        let nx = fx::div(x, w);
        let ny = fx::div(y, w);
        Some(((nx + ONE) * 50, (ONE - ny) * 50))
    }
}

/// The costume's colours, ready to draw.
pub struct Paint {
    pub skin: Rgb,
    pub shirt: Rgb,
    pub sleeve: Rgb,
    pub pants: Rgb,
    pub shoes: Rgb,
    pub hat: Rgb,
    pub accent: Rgb,
    pub hat_kind: String,
}

impl Paint {
    pub fn of(c: &Costume) -> Paint {
        Paint {
            skin: rgb(&c.skin),
            shirt: rgb(&c.shirt),
            sleeve: rgb(&c.sleeve),
            pants: rgb(&c.pants),
            shoes: rgb(&c.shoes),
            hat: rgb(&c.hat_colour),
            accent: rgb(&c.accent),
            hat_kind: c.hat.clone(),
        }
    }

    fn named(&self, name: &str) -> Rgb {
        match name {
            "skin" => self.skin,
            "sleeve" => self.sleeve,
            "pants" => self.pants,
            "shoes" => self.shoes,
            _ => self.shirt,
        }
    }
}

/// The performer: bones as boxes between particles, a torso spanning the
/// shoulders and hips, a head, a hat.
pub fn performer(b: &mut Batch, def: &BodyDef, pos: &[V3; POINTS], paint: &Paint) {
    for bone in &def.bones {
        b.rod(
            Mesh::Cyl,
            pos[bone.a],
            pos[bone.b],
            bone.half_width * 2,
            paint.named(&bone.paint),
            0,
        );
    }
    // hands and feet
    for (i, c) in [(body::HAND_L, paint.skin), (body::HAND_R, paint.skin)] {
        b.shape(Mesh::Sphere, pos[i], v3(int(1) / 9, int(1) / 9, int(1) / 9), c);
    }
    for i in [body::FOOT_L, body::FOOT_R] {
        b.shape(
            Mesh::Sphere,
            pos[i],
            v3(int(1) / 8, int(1) / 10, int(1) / 7),
            paint.shoes,
        );
    }
    // the torso: across from shoulder to shoulder, up from the hips to the neck
    let sh = pos[body::SHOULDER_L].lerp(pos[body::SHOULDER_R], ONE / 2);
    let hip = pos[body::HIP_L].lerp(pos[body::HIP_R], ONE / 2);
    let across = pos[body::SHOULDER_R].sub(pos[body::SHOULDER_L]);
    let up = sh.sub(hip);
    let x = across.norm();
    let y = up.norm();
    let z = x.cross(y).norm();
    let len = up.len();
    let torso = Affine {
        c0: x.scale(int(1) * 36 / 100),
        c1: y.scale(len + int(1) / 10),
        c2: z.scale(int(1) * 21 / 100),
        t: sh.lerp(hip, ONE / 2),
    };
    b.push(Mesh::Cube, &torso, paint.shirt, 0);
    // a belt in the accent colour
    let belt = Affine {
        c0: x.scale(int(1) * 37 / 100),
        c1: y.scale(int(1) / 14),
        c2: z.scale(int(1) * 22 / 100),
        t: hip.add(y.scale(int(1) / 14)),
    };
    b.push(Mesh::Cube, &belt, paint.accent, 0);
    // the head and the hat sit along the neck
    let head = pos[body::HEAD];
    let neck_up = head.sub(pos[body::NECK]).norm();
    b.shape(
        Mesh::Sphere,
        head,
        v3(int(1) * 25 / 100, int(1) * 28 / 100, int(1) * 25 / 100),
        paint.skin,
    );
    let hat_at = |k: i32| head.add(neck_up.scale(int(1) * k / 100));
    let cap = |b: &mut Batch, from: i32, to: i32, w: i32, mesh: Mesh, c: Rgb| {
        if let Some(af) = segment(hat_at(from), hat_at(to), w, w) {
            b.push(mesh, &af, c, 0);
        }
    };
    match paint.hat_kind.as_str() {
        "chef" => {
            cap(b, 8, 18, int(1) * 26 / 100, Mesh::Cyl, paint.hat);
            cap(b, 16, 34, int(1) * 30 / 100, Mesh::Sphere, paint.hat);
        }
        "top" => {
            cap(b, 8, 11, int(1) * 40 / 100, Mesh::Cyl, paint.hat);
            cap(b, 10, 36, int(1) * 24 / 100, Mesh::Cyl, paint.hat);
            cap(b, 12, 16, int(1) * 25 / 100, Mesh::Cyl, paint.accent);
        }
        "cap" => {
            cap(b, 4, 16, int(1) * 27 / 100, Mesh::Sphere, paint.hat);
        }
        "beanie" => {
            cap(b, 2, 20, int(1) * 27 / 100, Mesh::Sphere, paint.hat);
            cap(b, 18, 24, int(1) * 8 / 100, Mesh::Sphere, paint.accent);
        }
        "crown" => {
            cap(b, 9, 20, int(1) * 22 / 100, Mesh::Cyl, paint.hat);
            cap(b, 18, 26, int(1) * 6 / 100, Mesh::Octa, paint.accent);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_runs_from_one_end_to_the_other() {
        let a = v3(int(1), int(2), int(3));
        let b = v3(int(1), int(5), int(3));
        let af = segment(a, b, ONE / 10, ONE / 10).unwrap();
        // the unit shape's top and bottom (y = ±0.5) land on the ends
        let top = af.apply(v3(0, ONE / 2, 0));
        let bottom = af.apply(v3(0, -ONE / 2, 0));
        assert!(top.dist(b) < 8 && bottom.dist(a) < 8, "{top:?} {bottom:?}");
    }

    #[test]
    fn the_point_the_camera_looks_at_is_the_middle_of_the_screen() {
        let c = Camera {
            eye: v3(0, int(2), int(10)),
            target: v3(int(1), int(1), 0),
        };
        let (x, y) = c.screen(ONE, c.target).unwrap();
        assert!(
            (x - 50 * ONE).abs() < ONE / 4 && (y - 50 * ONE).abs() < ONE / 4,
            "{x} {y}"
        );
        // behind the camera there is nothing to show
        assert!(c.screen(ONE, v3(0, int(2), int(20))).is_none());
    }
}
