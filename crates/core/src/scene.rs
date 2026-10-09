//! The still part of a venue: the ground, the sky, the sun, the food truck,
//! the bed's frame, and the venue's buildings, trees and mountains, drawn from
//! the `props` of data/venues.json. Each prop kind is one rule below. Nothing
//! here moves; the page uploads it once a venue.

use crate::acts::{tower_base, trapeze_pivot, TABLE_HEIGHT};
use crate::balance::{
    BED_HALF_X, BED_HALF_Z, BED_Y, BED_Z, TRUCK_HALF_X, TRUCK_HALF_Z, TRUCK_ROOF, TRUCK_X, TRUCK_Z,
};
use crate::fx::{self, deg, int, ratio, ONE, TURN};
use crate::geom::{v3, Affine, V3};
use crate::mesh::Mesh;
use crate::render::{mix, segment, shade, Batch, Rgb};
use crate::rng::Rng;
use crate::tour::{rgb, Prop, Venue};

/// Which of the stage's fittings a mission needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fittings {
    pub trapeze: bool,
    pub tower: bool,
}

fn m(n: i32) -> i32 {
    int(n)
}

fn c(hex: &str) -> Rgb {
    rgb(hex)
}

const WHITE: &str = "#f7f4ec";

/// The sun's direction, from the venue's compass angle and height.
pub fn light_dir(v: &Venue) -> V3 {
    let yaw = deg(v.light[0]);
    let up = deg(v.light[1]);
    v3(
        fx::mul(fx::sin(yaw), fx::cos(up)),
        fx::sin(up),
        -fx::mul(fx::cos(yaw), fx::cos(up)),
    )
}

pub fn build(v: &Venue, fit: Fittings) -> Batch {
    let mut b = Batch::new();
    let mut rng = Rng::new(v.seed);
    ground_and_sky(&mut b, v);
    truck(&mut b);
    bed_frame(&mut b);
    if fit.trapeze {
        trapeze_rig(&mut b);
    }
    if fit.tower {
        table(&mut b);
    }
    for p in &v.props {
        prop(&mut b, &mut rng, p);
    }
    b
}

fn ground_and_sky(b: &mut Batch, v: &Venue) {
    let ground = c(&v.ground);
    b.push(
        Mesh::Ground,
        &crate::render::boxed(V3::ZERO, v3(m(4000), ONE, m(4000))),
        ground,
        0,
    );
    // a darker apron of trodden ground round the stage
    b.shape(
        Mesh::Cyl,
        v3(0, ONE / 40, int(1)),
        v3(m(26), ONE / 40, m(22)),
        shade(ground, 88),
    );
    // a band of the horizon's colour all round, far off, so the sky deepens
    // from the horizon up (the clear colour is the sky's top)
    let horizon = c(&v.sky);
    let ring = 24;
    for k in 0..ring {
        let a = TURN * k / ring;
        let at = v3(
            fx::mul(fx::sin(a), m(1500)),
            m(110),
            -fx::mul(fx::cos(a), m(1500)),
        );
        let face = Affine::translate(at)
            .then(&Affine::rot_y(-a))
            .then(&Affine::scale(v3(m(420), m(260), m(10))));
        b.push(Mesh::Cube, &face, horizon, ONE);
    }
    let sun = light_dir(v).scale(m(1300));
    let sun_colour = mix(horizon, c("#fff6dc"), ONE * 2 / 3);
    b.shape(Mesh::Sphere, sun, v3(m(70), m(70), m(70)), sun_colour);
    b.push(
        Mesh::Sphere,
        &crate::render::boxed(sun, v3(m(70), m(70), m(70))),
        sun_colour,
        ONE,
    );
}

fn truck(b: &mut Batch) {
    let body = c("#f4c430");
    let stripe = c("#2a9d8f");
    let dark = c("#26262b");
    let (x0, x1) = (TRUCK_X - TRUCK_HALF_X, TRUCK_X + TRUCK_HALF_X);
    let (z0, z1) = (TRUCK_Z - TRUCK_HALF_Z, TRUCK_Z + TRUCK_HALF_Z);
    let floor = ratio(6, 10);
    // the box, a little short of the roof line so the roof deck reads
    b.shape(
        Mesh::Cube,
        v3(TRUCK_X, (floor + TRUCK_ROOF) / 2, TRUCK_Z),
        v3(x1 - x0, TRUCK_ROOF - floor, z1 - z0),
        body,
    );
    // the roof deck the performers stand on, with a rail of lights round it
    b.shape(
        Mesh::Cube,
        v3(TRUCK_X, TRUCK_ROOF - ONE / 40, TRUCK_Z),
        v3(x1 - x0 + ONE / 10, ONE / 20, z1 - z0 + ONE / 10),
        c("#d9d4c7"),
    );
    // a stripe along both sides
    for z in [z0 - ONE / 100, z1 + ONE / 100] {
        b.shape(
            Mesh::Cube,
            v3(TRUCK_X, ratio(11, 10), z),
            v3(x1 - x0, ratio(3, 10), ONE / 50),
            stripe,
        );
    }
    // chassis and wheels
    b.shape(
        Mesh::Cube,
        v3(TRUCK_X, ratio(4, 10), TRUCK_Z),
        v3(x1 - x0 - ONE, ratio(3, 10), z1 - z0 - ONE / 2),
        dark,
    );
    for wx in [x0 + ratio(13, 10), x1 - ratio(13, 10)] {
        for wz in [z0 + ratio(1, 10), z1 - ratio(1, 10)] {
            let at = v3(wx, ratio(45, 100), wz);
            if let Some(af) = segment(
                at.sub(v3(0, 0, ratio(15, 100))),
                at.add(v3(0, 0, ratio(15, 100))),
                ratio(9, 10),
                ratio(9, 10),
            ) {
                b.push(Mesh::Cyl, &af, c("#18181b"), 0);
            }
        }
    }
    // windscreen at the front (+x end)
    b.shape(
        Mesh::Cube,
        v3(x1 + ONE / 100, ratio(23, 10), TRUCK_Z),
        v3(ONE / 40, ratio(9, 10), z1 - z0 - ratio(3, 10)),
        c("#9fc5d8"),
    );
    b.push(
        Mesh::Cube,
        &crate::render::boxed(
            v3(x1 + ONE / 50, ratio(23, 10), TRUCK_Z),
            v3(ONE / 60, ratio(8, 10), z1 - z0 - ratio(4, 10)),
        ),
        c("#bfe3f2"),
        ONE / 5,
    );
    // headlights
    for z in [z0 + ratio(3, 10), z1 - ratio(3, 10)] {
        b.push(
            Mesh::Sphere,
            &crate::render::boxed(v3(x1 + ONE / 50, ratio(9, 10), z), v3(ONE / 5, ONE / 5, ONE / 5)),
            c("#fff7d6"),
            ONE,
        );
    }
    // the serving window, facing the bed, lit warm from inside
    let wz = z1 + ONE / 100;
    b.push(
        Mesh::Cube,
        &crate::render::boxed(
            v3(TRUCK_X - ratio(6, 10), ratio(19, 10), wz),
            v3(ratio(34, 10), ratio(10, 10), ONE / 50),
        ),
        c("#ffb35c"),
        ONE * 2 / 5,
    );
    // the counter under it
    b.shape(
        Mesh::Cube,
        v3(TRUCK_X - ratio(6, 10), ratio(14, 10), wz + ratio(15, 100)),
        v3(ratio(36, 10), ONE / 12, ratio(3, 10)),
        c("#8d6e63"),
    );
    // the awning: striped slats leaning out over the window
    for k in 0..8 {
        let x = TRUCK_X - ratio(23, 10) + k * ratio(45, 100);
        let colour = if k % 2 == 0 { c("#e63946") } else { c(WHITE) };
        let top = v3(x, ratio(265, 100), wz);
        let lip = v3(x, ratio(235, 100), wz + ratio(9, 10));
        if let Some(af) = segment(top, lip, ratio(45, 100), ONE / 30) {
            b.push(Mesh::Cube, &af, colour, 0);
        }
    }
    // the popcorn machine in the window: a red cabinet, its glass lit
    let s = crate::acts::spout();
    b.shape(
        Mesh::Cube,
        v3(s.x, ratio(17, 10), s.z - ratio(2, 10)),
        v3(ratio(6, 10), ratio(3, 10), ratio(4, 10)),
        c("#c1121f"),
    );
    b.push(
        Mesh::Cube,
        &crate::render::boxed(
            v3(s.x, ratio(21, 10), s.z - ratio(2, 10)),
            v3(ratio(55, 100), ratio(45, 100), ratio(35, 100)),
        ),
        c("#ffe08a"),
        ONE / 2,
    );
    // a menu board beside the window: dark, with coloured lines
    b.shape(
        Mesh::Cube,
        v3(TRUCK_X + ratio(19, 10), ratio(20, 10), wz),
        v3(ratio(12, 10), ratio(10, 10), ONE / 40),
        c("#2b2d42"),
    );
    for k in 0..4 {
        let colour = [c("#ffd166"), c("#06d6a0"), c("#ef476f"), c("#f8f9fa")][k as usize];
        b.push(
            Mesh::Cube,
            &crate::render::boxed(
                v3(
                    TRUCK_X + ratio(19, 10),
                    ratio(235, 100) - k * ratio(2, 10),
                    wz + ONE / 50,
                ),
                v3(ratio(9, 10), ONE / 25, ONE / 60),
            ),
            colour,
            ONE / 3,
        );
    }
    // rail posts and bulbs round the roof
    let mut bulb = 0;
    let mut x = x0;
    while x <= x1 {
        for z in [z0, z1] {
            b.push(
                Mesh::Sphere,
                &crate::render::boxed(v3(x, TRUCK_ROOF + ONE / 20, z), v3(ONE / 9, ONE / 9, ONE / 9)),
                [c("#ffd166"), c("#ef476f"), c("#06d6a0")][bulb % 3],
                ONE,
            );
            bulb += 1;
        }
        x += ratio(6, 10);
    }
}

fn bed_frame(b: &mut Batch) {
    let pad = c("#1d4ed8");
    let steel = c("#9ca3af");
    let (hx, hz) = (BED_HALF_X, BED_HALF_Z);
    let w = ratio(35, 100);
    let y = BED_Y - ONE / 20;
    // pads along the four sides
    b.shape(
        Mesh::Cube,
        v3(0, y, BED_Z - hz - w / 2),
        v3(2 * hx + 2 * w, ONE / 8, w),
        pad,
    );
    b.shape(
        Mesh::Cube,
        v3(0, y, BED_Z + hz + w / 2),
        v3(2 * hx + 2 * w, ONE / 8, w),
        pad,
    );
    b.shape(Mesh::Cube, v3(-hx - w / 2, y, BED_Z), v3(w, ONE / 8, 2 * hz), pad);
    b.shape(Mesh::Cube, v3(hx + w / 2, y, BED_Z), v3(w, ONE / 8, 2 * hz), pad);
    // legs
    for sx in [-1, 1] {
        for sz in [-1, 1] {
            let at = v3(sx * (hx + w / 2), 0, BED_Z + sz * (hz + w / 2));
            b.rod(Mesh::Cyl, at, at.add(v3(0, y, 0)), ONE / 12, steel, 0);
        }
    }
    // the shadow under the bed
    b.shape(
        Mesh::Cube,
        v3(0, ONE / 25, BED_Z),
        v3(2 * hx, ONE / 50, 2 * hz),
        c("#1f2a1c"),
    );
}

fn trapeze_rig(b: &mut Batch) {
    let p = trapeze_pivot();
    let steel = c("#d1d5db");
    let half = ratio(22, 10);
    for sz in [-1, 1] {
        let top = v3(p.x, p.y, p.z + sz * half);
        // the legs splay away from the bed, so from the crowd each side
        // reads as one slim post rather than an A across the view
        for sx in [-1, 1] {
            let foot = v3(p.x + sx * ratio(4, 10), 0, p.z + sz * (half + ratio(17, 10)));
            b.rod(Mesh::Cyl, foot, top, ONE / 10, steel, 0);
        }
    }
    b.rod(
        Mesh::Cyl,
        v3(p.x, p.y, p.z - half),
        v3(p.x, p.y, p.z + half),
        ONE / 8,
        steel,
        0,
    );
    // the catcher's net-like apron at the far end is the bed itself
}

fn table(b: &mut Batch) {
    let base = tower_base();
    let wood = c("#a47148");
    b.shape(
        Mesh::Cube,
        base.add(v3(0, TABLE_HEIGHT, 0)),
        v3(ratio(12, 10), ONE / 12, ratio(8, 10)),
        wood,
    );
    for sx in [-1, 1] {
        for sz in [-1, 1] {
            let at = base.add(v3(sx * ratio(5, 10), 0, sz * ratio(3, 10)));
            b.rod(
                Mesh::Cube,
                at,
                at.add(v3(0, TABLE_HEIGHT, 0)),
                ONE / 14,
                shade(wood, 80),
                0,
            );
        }
    }
}

fn in_stage(x: i32, z: i32) -> bool {
    // the truck, the bed and the crowd's lawn stay clear of scenery
    x.abs() < m(18) && z > m(-9) && z < m(20)
}

fn area_point(rng: &mut Rng, a: [i32; 4]) -> (i32, i32) {
    let x = rng.range(a[0] * 10, a[2] * 10) * ONE / 10;
    let z = rng.range(a[1] * 10, a[3] * 10) * ONE / 10;
    (x, z)
}

fn colour_from(rng: &mut Rng, list: &[String], fallback: &str) -> Rgb {
    if list.is_empty() {
        c(fallback)
    } else {
        c(&list[rng.range(0, list.len() as i32 - 1) as usize])
    }
}

fn prop(b: &mut Batch, rng: &mut Rng, p: &Prop) {
    let (ax, az) = (m(p.at[0]), m(p.at[1]));
    match p.kind.as_str() {
        "building" => building(b, ax, az, p.size, c(&p.colour), &p.windows, p.bands, &p.roof),
        "containers" => {
            for k in 0..p.count {
                let (row, level) = (k % 5, k / 5);
                let colour = colour_from(rng, &p.colours, WHITE);
                let x = ax + m(row * 3) - m(6);
                b.stand(
                    Mesh::Cube,
                    x,
                    level * ratio(26, 10),
                    az + m(level),
                    v3(ratio(25, 10), ratio(26, 10), m(6)),
                    colour,
                );
                // corrugation: a darker stripe
                b.stand(
                    Mesh::Cube,
                    x,
                    level * ratio(26, 10) + ratio(11, 10),
                    az + m(level) + m(3),
                    v3(ratio(25, 10), ratio(2, 10), ONE / 40),
                    shade(colour, 70),
                );
            }
        }
        "lot" => {
            let a = p.area;
            let (x0, z0, x1, z1) = (m(a[0]), m(a[1]), m(a[2]), m(a[3]));
            b.shape(
                Mesh::Cube,
                v3((x0 + x1) / 2, ONE / 20, (z0 + z1) / 2),
                v3(x1 - x0, ONE / 20, z1 - z0),
                c(&p.colour),
            );
            let mut x = x0 + m(1);
            while x < x1 {
                b.shape(
                    Mesh::Cube,
                    v3(x, ONE / 12, (z0 + z1) / 2),
                    v3(ONE / 10, ONE / 50, (z1 - z0) * 3 / 4),
                    c(WHITE),
                );
                x += m(3);
            }
        }
        "trees" => {
            let Some(h) = (if p.height.1 > 0 { Some(p.height) } else { None }) else {
                return;
            };
            for _ in 0..p.count {
                let (x, z) = area_point(rng, p.area);
                if in_stage(x, z) {
                    continue;
                }
                let tall = rng.range(h.0 * 10, h.1 * 10) * ONE / 10;
                let crown = colour_from(rng, &p.colours, "#3f7a35");
                tree(b, rng, &p.tree, x, z, tall, crown);
            }
        }
        "lights" => {
            let (a, z0) = (v3(m(p.from[0]), 0, m(p.from[1])), m(p.from[1]));
            let e = v3(m(p.to[0]), 0, m(p.to[1]));
            let top = ratio(48, 10);
            for post in [a, e, a.lerp(e, ONE / 2)] {
                b.rod(
                    Mesh::Cyl,
                    post,
                    post.add(v3(0, top, 0)),
                    ONE / 10,
                    c("#4b3b2f"),
                    0,
                );
            }
            let bulbs = 28;
            for k in 0..=bulbs {
                let t = k * ONE / bulbs;
                let along = a.lerp(e, t);
                // a sag in each half
                let half = (t * 2) % ONE;
                let sag = fx::mul(fx::mul(half, ONE - half), m(2));
                let at = v3(along.x, top - sag, z0 + (e.z - a.z) * k / bulbs);
                let colour = colour_from(rng, &p.colours, "#fff1c1");
                b.push(
                    Mesh::Sphere,
                    &crate::render::boxed(at, v3(ONE / 7, ONE / 7, ONE / 7)),
                    colour,
                    ONE,
                );
            }
        }
        "stack" => {
            let h = m(p.height.0);
            b.stand(Mesh::Cyl, ax, 0, az, v3(m(3), h, m(3)), c(&p.colour));
            b.stand(
                Mesh::Cyl,
                ax,
                h - m(2),
                az,
                v3(ratio(34, 10), m(1), ratio(34, 10)),
                shade(c(&p.colour), 70),
            );
            b.stand(
                Mesh::Cyl,
                ax,
                0,
                az,
                v3(m(5), m(3), m(5)),
                shade(c(&p.colour), 85),
            );
        }
        "water_tower" => {
            let h = m(p.height.0);
            let col = c(&p.colour);
            for (sx, sz) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                b.rod(
                    Mesh::Cyl,
                    v3(ax + sx * m(3), 0, az + sz * m(3)),
                    v3(ax + sx * m(2), h - m(6), az + sz * m(2)),
                    ONE / 4,
                    shade(col, 70),
                    0,
                );
            }
            b.stand(Mesh::Cyl, ax, h - m(7), az, v3(m(7), m(6), m(7)), col);
            b.stand(
                Mesh::Cone,
                ax,
                h - m(1),
                az,
                v3(ratio(75, 10), m(3), ratio(75, 10)),
                shade(col, 80),
            );
        }
        "barn" => {
            let col = c(&p.colour);
            b.stand(Mesh::Cube, ax, 0, az, v3(m(14), m(8), m(12)), col);
            gable(b, v3(ax, m(8), az), m(14), m(12), m(4), shade(col, 60));
            // white trim: the doors' X
            let face = az + m(6) + ONE / 50;
            for (a, e) in [((-3, 0), (3, 6)), ((3, 0), (-3, 6))] {
                b.rod(
                    Mesh::Cube,
                    v3(ax + m(a.0), m(a.1), face),
                    v3(ax + m(e.0), m(e.1), face),
                    ONE / 4,
                    c(WHITE),
                    0,
                );
            }
            b.shape(
                Mesh::Cube,
                v3(ax, m(3), face),
                v3(m(6), m(6), ONE / 60),
                shade(col, 85),
            );
        }
        "silo" => {
            let h = m(p.height.0);
            b.stand(Mesh::Cyl, ax, 0, az, v3(m(5), h, m(5)), c(&p.colour));
            b.shape(
                Mesh::Sphere,
                v3(ax, h, az),
                v3(m(5), m(4), m(5)),
                shade(c(&p.colour), 90),
            );
        }
        "fence" => {
            let (a, e) = (v3(m(p.from[0]), 0, m(p.from[1])), v3(m(p.to[0]), 0, m(p.to[1])));
            let posts = (e.sub(a).len() / m(3)).max(1);
            for k in 0..=posts {
                let at = a.lerp(e, k * ONE / posts);
                if in_stage(at.x, at.z) && at.x.abs() < m(10) {
                    continue;
                }
                b.rod(
                    Mesh::Cube,
                    at,
                    at.add(v3(0, ratio(12, 10), 0)),
                    ONE / 8,
                    c(&p.colour),
                    0,
                );
            }
            for y in [ratio(5, 10), ratio(10, 10)] {
                for (s, f) in [(a, v3(-m(10), 0, a.z)), (v3(m(10), 0, a.z), e)] {
                    b.rod(
                        Mesh::Cube,
                        s.add(v3(0, y, 0)),
                        f.add(v3(0, y, 0)),
                        ONE / 12,
                        c(&p.colour),
                        0,
                    );
                }
            }
        }
        "hills" => {
            for k in 0..p.count {
                let a = -deg(70) + deg(140) * k / (p.count - 1).max(1) + rng.range(-deg(6), deg(6));
                let d = m(p.distance) + rng.range(0, m(80));
                let (x, z) = (fx::mul(fx::sin(a), d), -fx::mul(fx::cos(a), d));
                let w = m(rng.range(120, 220));
                let h = m(rng.range(25, 50));
                b.shape(
                    Mesh::Sphere,
                    v3(x, 0, z),
                    v3(w, h * 2, w * 2 / 3),
                    shade(c(&p.colour), rng.range(85, 105)),
                );
            }
        }
        "bales" => {
            for _ in 0..p.count {
                let (x, z) = area_point(rng, p.area);
                let at = v3(x, ratio(7, 10), z);
                b.rod(
                    Mesh::Cyl,
                    at.sub(v3(ratio(7, 10), 0, 0)),
                    at.add(v3(ratio(7, 10), 0, 0)),
                    ratio(14, 10),
                    c("#d9b45b"),
                    0,
                );
            }
        }
        "mural" => {
            let [w, h, d] = p.size;
            b.stand(Mesh::Cube, ax, 0, az, v3(m(w), m(h), m(d)), c("#c9c3b8"));
            let n = p.colours.len().max(1) as i32;
            let stripe = m(w) / (n * 2);
            for k in 0..n * 2 {
                let colour = c(&p.colours[(k % n) as usize]);
                let x = ax - m(w) / 2 + stripe * k + stripe / 2;
                let tall = m(h) * rng.range(55, 95) / 100;
                b.push(
                    Mesh::Cube,
                    &crate::render::boxed(
                        v3(x, tall / 2, az + m(d) / 2 + ONE / 50),
                        v3(stripe, tall, ONE / 30),
                    ),
                    colour,
                    ONE / 8,
                );
                // a ring painted on each stripe
                b.shape(
                    Mesh::Cyl,
                    v3(x, tall + stripe / 2, az + m(d) / 2 + ONE / 40),
                    v3(stripe, ONE / 30, stripe),
                    colour,
                );
            }
        }
        "piles" => {
            for _ in 0..p.count {
                let (x, z) = area_point(rng, p.area);
                let colour = colour_from(rng, &p.colours, WHITE);
                b.shape(
                    Mesh::Octa,
                    v3(x, ratio(6, 10), z),
                    v3(m(2), ratio(15, 10), m(2)),
                    colour,
                );
                b.shape(
                    Mesh::Sphere,
                    v3(x + ratio(6, 10), ratio(4, 10), z),
                    v3(m(1), ratio(8, 10), m(1)),
                    shade(colour, 80),
                );
            }
        }
        "lamps" => {
            for _ in 0..p.count {
                let (x, z) = area_point(rng, p.area);
                if x.abs() < m(10) && z > m(-7) && z < m(17) {
                    continue;
                }
                b.rod(
                    Mesh::Cyl,
                    v3(x, 0, z),
                    v3(x, ratio(42, 10), z),
                    ONE / 10,
                    c("#2f3640"),
                    0,
                );
                b.push(
                    Mesh::Sphere,
                    &crate::render::boxed(v3(x, ratio(44, 10), z), v3(ONE / 2, ONE / 2, ONE / 2)),
                    c("#fff2c6"),
                    ONE,
                );
            }
        }
        "street" => {
            let n = p.count.max(1);
            let width = m(9);
            for k in 0..n {
                let x = ax + (k - n / 2) * width;
                let h = m(rng.range(7, 13));
                let colour = colour_from(rng, &p.colours, "#9b4a35");
                let size = [9, h / ONE, 10];
                building(b, x, az, size, colour, "#ffe1a8", 2, "flat");
                // a striped awning at the shopfront
                let face = if az > 0 { az - m(5) } else { az + m(5) };
                b.shape(
                    Mesh::Cube,
                    v3(x, ratio(32, 10), face),
                    v3(width - m(2), ONE / 10, m(1)),
                    colour_from(
                        rng,
                        &["#e63946".into(), "#2a9d8f".into(), "#264653".into()],
                        WHITE,
                    ),
                );
            }
        }
        "gothic" => {
            let [w, h, d] = p.size;
            let col = c(&p.colour);
            let trim = c(&p.trim);
            building(b, ax, az, [w, h, d], col, "#ffe2a6", 0, "flat");
            // towers with spires at the ends and the middle
            for k in [-1, 0, 1] {
                let x = ax + k * (m(w) / 2 - m(2));
                let th = m(h) + m(if k == 0 { 9 } else { 5 });
                b.stand(
                    Mesh::Cube,
                    x,
                    0,
                    az + m(d) / 2 - m(2),
                    v3(m(4), th, m(4)),
                    shade(col, 92),
                );
                b.stand(
                    Mesh::Cone,
                    x,
                    th,
                    az + m(d) / 2 - m(2),
                    v3(m(4), m(5), m(4)),
                    c("#4b5563"),
                );
                b.stand(
                    Mesh::Cube,
                    x,
                    th - m(1),
                    az + m(d) / 2 - m(2),
                    v3(ratio(42, 10), ONE / 2, ratio(42, 10)),
                    trim,
                );
            }
            // tall pointed windows
            let face = az + m(d) / 2 + ONE / 50;
            let mut x = ax - m(w) / 2 + m(4);
            while x < ax + m(w) / 2 - m(3) {
                b.push(
                    Mesh::Cube,
                    &crate::render::boxed(v3(x, m(h) / 2, face), v3(m(1), m(h) / 2, ONE / 30)),
                    c("#ffe2a6"),
                    ONE / 3,
                );
                b.push(
                    Mesh::Wedge,
                    &crate::render::boxed(v3(x, m(h) * 3 / 4 + m(1) / 2, face), v3(m(1), m(1), ONE / 30)),
                    c("#ffe2a6"),
                    ONE / 3,
                );
                x += m(3);
            }
            b.shape(
                Mesh::Cube,
                v3(ax, m(h), az),
                v3(m(w) + ONE / 4, ONE / 2, m(d) + ONE / 4),
                trim,
            );
        }
        "water" => {
            let a = p.area;
            let (x0, z0, x1, z1) = (m(a[0]), m(a[1]), m(a[2]), m(a[3]));
            b.push(
                Mesh::Cube,
                &crate::render::boxed(
                    v3((x0 + x1) / 2, ONE / 8, (z0 + z1) / 2),
                    v3(x1 - x0, ONE / 8, z1 - z0),
                ),
                c(&p.colour),
                ONE / 10,
            );
        }
        "arches" => {
            let col = c(&p.colour);
            let n = p.count.max(1);
            let (w, h) = (m(p.width), m(p.height.0));
            let total = w * n + m(2) * (n + 1);
            b.stand(Mesh::Cube, ax, 0, az, v3(total, h + m(4), m(14)), col);
            let face = az + m(7) + ONE / 50;
            for k in 0..n {
                let x = ax - total / 2 + m(2) + w / 2 + k * (w + m(2));
                b.push(
                    Mesh::Cube,
                    &crate::render::boxed(v3(x, h / 2, face), v3(w, h, ONE / 30)),
                    c("#f3c46b"),
                    ONE / 2,
                );
                b.shape(Mesh::Cyl, v3(x, h, face), v3(w, ONE / 30, w / 2), col);
            }
            // the reflecting pool's edge and plaza paving
            b.shape(
                Mesh::Cube,
                v3(ax, ONE / 20, az + m(12)),
                v3(total + m(10), ONE / 20, m(10)),
                shade(col, 95),
            );
        }
        "skyline" => {
            for k in 0..p.count {
                let a = -deg(60) + deg(120) * k / p.count.max(1) + rng.range(-deg(3), deg(3));
                let d = m(p.distance) + rng.range(0, m(90));
                let (x, z) = (fx::mul(fx::sin(a), d), -fx::mul(fx::cos(a), d));
                let h = m(rng.range(40, 140));
                let w = m(rng.range(14, 30));
                let colour = colour_from(rng, &p.colours, "#6b7280");
                b.stand(Mesh::Cube, x, 0, z, v3(w, h, w), colour);
                let mut y = m(6);
                while y < h - m(3) {
                    b.push(
                        Mesh::Cube,
                        &crate::render::boxed(v3(x, y, z + w / 2 + ONE / 20), v3(w - m(2), ONE, ONE / 20)),
                        c("#fde9b0"),
                        ONE / 3,
                    );
                    y += m(7);
                }
            }
        }
        "mansion" => {
            let col = c(&p.colour);
            let trim = c(&p.trim);
            b.stand(Mesh::Cube, ax, 0, az, v3(m(40), m(12), m(16)), col);
            b.stand(Mesh::Cube, ax, m(12), az, v3(m(14), m(8), m(12)), col);
            b.stand(Mesh::Cube, ax, m(20), az, v3(m(7), m(8), m(7)), shade(col, 95));
            b.stand(Mesh::Cone, ax, m(28), az, v3(m(8), m(4), m(8)), c("#7d5a50"));
            for y in [m(4), m(8), m(12), m(20)] {
                b.shape(Mesh::Cube, v3(ax, y, az), v3(m(41), ONE / 3, m(17)), trim);
            }
            let face = az + m(8) + ONE / 50;
            let mut x = ax - m(17);
            while x <= ax + m(17) {
                b.push(
                    Mesh::Cube,
                    &crate::render::boxed(v3(x, m(6), face), v3(ratio(15, 10), m(3), ONE / 30)),
                    c("#ffe1b0"),
                    ONE / 3,
                );
                x += m(4);
            }
            // the terrace down to the bay
            b.stand(
                Mesh::Cube,
                ax,
                0,
                az + m(12),
                v3(m(44), ONE / 2, m(8)),
                shade(trim, 90),
            );
        }
        "deco" => {
            let n = p.count.max(1);
            for k in 0..n {
                let x = ax + (k - n / 2) * m(13);
                let h = m(rng.range(10, 17));
                let colour = c(&p.colours[(k as usize) % p.colours.len().max(1)]);
                b.stand(Mesh::Cube, x, 0, az, v3(m(11), h, m(10)), colour);
                // rounded corners and a stepped crown
                for sx in [-1, 1] {
                    b.stand(Mesh::Cyl, x + sx * m(5), 0, az + m(4), v3(m(2), h, m(2)), colour);
                }
                b.stand(Mesh::Cube, x, h, az, v3(m(5), m(3), m(5)), shade(colour, 95));
                for y in [m(4), m(8), h - m(1)] {
                    b.shape(
                        Mesh::Cube,
                        v3(x, y, az + m(5) + ONE / 50),
                        v3(m(11), ONE / 4, ONE / 30),
                        c(WHITE),
                    );
                }
                b.push(
                    Mesh::Cube,
                    &crate::render::boxed(
                        v3(x, h / 2, az + m(5) + ONE / 30),
                        v3(ONE / 2, h - m(3), ONE / 30),
                    ),
                    c("#7df9ff"),
                    ONE / 2,
                );
            }
        }
        "mountains" => {
            let snow = p.snow;
            for k in 0..p.count {
                let a = -deg(65) + deg(130) * k / (p.count - 1).max(1) + rng.range(-deg(5), deg(5));
                let d = m(p.distance) + rng.range(0, m(120));
                let (x, z) = (fx::mul(fx::sin(a), d), -fx::mul(fx::cos(a), d));
                let h = m(p.height.0) / 100 * rng.range(60, 110);
                let w = h / 100 * rng.range(200, 320);
                let colour = shade(c(&p.colour), rng.range(85, 108));
                b.stand(Mesh::Cone, x, 0, z, v3(w, h, w * 2 / 3), colour);
                if snow {
                    b.stand(
                        Mesh::Cone,
                        x,
                        h * 7 / 10,
                        z,
                        v3(w * 3 / 10, h * 3 / 10, w * 2 / 10),
                        c("#f8fafc"),
                    );
                }
            }
        }
        "peak" => {
            let h = m(p.height.0);
            b.stand(Mesh::Cone, ax, 0, az, v3(h * 3, h, h * 2), c(&p.colour));
            b.stand(
                Mesh::Cone,
                ax,
                h * 6 / 10,
                az,
                v3(h * 12 / 10, h * 4 / 10, h * 8 / 10),
                c("#f8fafc"),
            );
        }
        "temple" => {
            let col = c(&p.colour);
            b.stand(
                Mesh::Cube,
                ax,
                0,
                az,
                v3(m(24), ONE * 3 / 2, m(16)),
                shade(col, 90),
            );
            b.stand(
                Mesh::Cube,
                ax,
                ONE * 3 / 2,
                az - m(2),
                v3(m(22), m(9), m(10)),
                col,
            );
            for k in 0..6 {
                let x = ax - m(9) + k * m(18) / 5;
                b.stand(Mesh::Cyl, x, ONE * 3 / 2, az + m(5), v3(ONE, m(8), ONE), c(WHITE));
            }
            b.stand(
                Mesh::Cube,
                ax,
                ONE * 3 / 2 + m(8),
                az + m(1),
                v3(m(22), ONE, m(10)),
                col,
            );
            gable(
                b,
                v3(ax, ONE * 5 / 2 + m(8), az + m(1)),
                m(22),
                m(10),
                m(3),
                shade(col, 95),
            );
        }
        "flag" => {
            b.rod(Mesh::Cyl, v3(ax, 0, az), v3(ax, m(3), az), ONE / 20, c(WHITE), 0);
            b.shape(
                Mesh::Cube,
                v3(ax + ratio(4, 10), ratio(27, 10), az),
                v3(ratio(8, 10), ratio(5, 10), ONE / 40),
                c("#e11d48"),
            );
        }
        "adobe" => {
            let n = p.count.max(1);
            for k in 0..n {
                let x = ax + (k - n / 2) * m(11);
                let h = m(rng.range(4, 7));
                let colour = colour_from(rng, &p.colours, "#d6a77a");
                b.stand(
                    Mesh::Cube,
                    x,
                    0,
                    az + m(rng.range(-2, 2)),
                    v3(m(10), h, m(9)),
                    colour,
                );
                for j in 0..5 {
                    let vx = x - m(4) + j * m(2);
                    b.rod(
                        Mesh::Cyl,
                        v3(vx, h - ONE / 2, az + m(4)),
                        v3(vx, h - ONE / 2, az + m(5)),
                        ONE / 4,
                        c("#6b4423"),
                        0,
                    );
                }
                b.push(
                    Mesh::Cube,
                    &crate::render::boxed(v3(x, ratio(15, 10), az + ratio(46, 10)), v3(m(2), m(2), ONE / 20)),
                    c("#ffcf8a"),
                    ONE / 2,
                );
            }
        }
        "playground" => {
            // a slide, a swing frame, a climbing dome, in bright colours
            b.stand(
                Mesh::Cube,
                ax,
                0,
                az,
                v3(ratio(16, 10), m(2), ratio(16, 10)),
                c("#ef4444"),
            );
            b.rod(
                Mesh::Cube,
                v3(ax + ONE, m(2), az),
                v3(ax + m(4), ONE / 4, az),
                ONE * 2 / 3,
                c("#facc15"),
                0,
            );
            for sx in [-1, 1] {
                b.rod(
                    Mesh::Cyl,
                    v3(ax - m(5) + sx * m(2), 0, az - m(1)),
                    v3(ax - m(5), ratio(25, 10), az - m(1)),
                    ONE / 8,
                    c("#3b82f6"),
                    0,
                );
                b.rod(
                    Mesh::Cyl,
                    v3(ax - m(5) + sx * m(2), 0, az + m(1)),
                    v3(ax - m(5), ratio(25, 10), az + m(1)),
                    ONE / 8,
                    c("#3b82f6"),
                    0,
                );
            }
            b.rod(
                Mesh::Cyl,
                v3(ax - m(5), ratio(25, 10), az - m(1)),
                v3(ax - m(5), ratio(25, 10), az + m(1)),
                ONE / 8,
                c("#3b82f6"),
                0,
            );
            b.shape(
                Mesh::Sphere,
                v3(ax + m(2), 0, az - m(4)),
                v3(m(3), m(4), m(3)),
                c("#22c55e"),
            );
        }
        "victorians" => {
            let n = p.count.max(1);
            for k in 0..n {
                let x = ax + (k - n / 2) * m(7);
                let h = m(rng.range(9, 12));
                let colour = c(&p.colours[(k as usize) % p.colours.len().max(1)]);
                let z = az + m(rng.range(-1, 1)) + m(rng.range(0, 3) * 4);
                let y0 = m(rng.range(0, 3) * 2);
                b.stand(Mesh::Cube, x, y0, z, v3(m(6), h, m(10)), colour);
                gable(b, v3(x, y0 + h, z), m(6), m(10), m(3), shade(colour, 75));
                b.stand(
                    Mesh::Cube,
                    x - m(1),
                    y0 + m(3),
                    z + m(5),
                    v3(m(2), m(5), m(1)),
                    c(WHITE),
                );
                b.push(
                    Mesh::Cube,
                    &crate::render::boxed(
                        v3(x + ratio(15, 10), y0 + m(6), z + m(5) + ONE / 30),
                        v3(ONE * 3 / 2, m(2), ONE / 30),
                    ),
                    c("#ffe7b3"),
                    ONE / 3,
                );
            }
        }
        "tents" => {
            for _ in 0..p.count {
                let (x, z) = area_point(rng, p.area);
                if in_stage(x, z) && x.abs() < m(10) {
                    continue;
                }
                let colour = colour_from(rng, &p.colours, WHITE);
                for (sx, sz) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                    b.rod(
                        Mesh::Cyl,
                        v3(x + sx * ratio(14, 10), 0, z + sz * ratio(14, 10)),
                        v3(x + sx * ratio(14, 10), ratio(22, 10), z + sz * ratio(14, 10)),
                        ONE / 14,
                        c("#9ca3af"),
                        0,
                    );
                }
                b.stand(
                    Mesh::Cone,
                    x,
                    ratio(22, 10),
                    z,
                    v3(ratio(42, 10), ratio(12, 10), ratio(42, 10)),
                    colour,
                );
                b.stand(
                    Mesh::Cube,
                    x,
                    0,
                    z,
                    v3(m(2), ratio(9, 10), ratio(8, 10)),
                    shade(colour, 70),
                );
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn building(b: &mut Batch, x: i32, z: i32, size: [i32; 3], col: Rgb, windows: &str, bands: i32, roof: &str) {
    let [w, h, d] = size;
    let (w, h, d) = (m(w), m(h), m(d));
    b.stand(Mesh::Cube, x, 0, z, v3(w, h, d), col);
    // the face toward the stage carries the windows: lit bands at dusk
    let toward = if z < 0 { 1 } else { -1 };
    let face = z + toward * (d / 2 + ONE / 50);
    if !windows.is_empty() {
        let lit = c(windows);
        for k in 0..bands {
            let y = h * (k * 2 + 1) / (bands * 2 + 1) + h / (bands * 4 + 2);
            let mut wx = x - w / 2 + m(2);
            while wx < x + w / 2 - m(1) {
                b.push(
                    Mesh::Cube,
                    &crate::render::boxed(v3(wx, y, face), v3(ratio(14, 10), h / (bands * 3), ONE / 30)),
                    lit,
                    ONE * 2 / 5,
                );
                wx += m(3);
            }
        }
    }
    match roof {
        "gable" => gable(b, v3(x, h, z), w, d, m(3), shade(col, 70)),
        "saw" => {
            let teeth = (w / m(6)).max(1);
            for k in 0..teeth {
                let tx = x - w / 2 + w * (2 * k + 1) / (2 * teeth);
                b.push(
                    Mesh::Wedge,
                    &crate::render::boxed(v3(tx, h + m(1), z), v3(w / teeth, m(2), d)),
                    shade(col, 75),
                    0,
                );
            }
        }
        _ => {
            b.shape(
                Mesh::Cube,
                v3(x, h, z),
                v3(w + ONE / 3, ONE / 3, d + ONE / 3),
                shade(col, 80),
            );
        }
    }
}

/// A pitched roof: two wedges back to back, ridge along x.
fn gable(b: &mut Batch, top_centre: V3, w: i32, d: i32, rise: i32, col: Rgb) {
    for side in [-1, 1] {
        // the wedge is full at +z and slopes to -z; turn it to face out each side
        let at = top_centre.add(v3(0, rise / 2, side * d / 4));
        let turn = Affine::rot_y(if side > 0 { 0 } else { fx::HALF });
        let af = Affine::translate(at)
            .then(&turn)
            .then(&Affine::scale(v3(w, rise, d / 2)));
        // the wedge's tall edge is its back (+z): flip so the ridge is in the middle
        let af = af.then(&Affine::rot_y(fx::HALF));
        b.push(Mesh::Wedge, &af, col, 0);
    }
}

fn tree(b: &mut Batch, rng: &mut Rng, kind: &str, x: i32, z: i32, h: i32, crown: Rgb) {
    let bark = c("#5b4636");
    match kind {
        "pine" => {
            b.stand(Mesh::Cyl, x, 0, z, v3(h / 18, h / 3, h / 18), bark);
            let green = mix(c("#1f4d2b"), crown, ONE / 4);
            for k in 0..3 {
                let base = h / 5 + k * h / 4;
                let w = h * (5 - k) / 12;
                b.stand(Mesh::Cone, x, base, z, v3(w, h / 2, w), shade(green, 90 + k * 6));
            }
        }
        "palm" => {
            let lean = rng.range(-m(1), m(1));
            let top = v3(x + lean, h, z);
            b.rod(Mesh::Cyl, v3(x, 0, z), top, h / 22, c("#8b6b4a"), 0);
            for k in 0..7 {
                let a = TURN * k / 7 + rng.range(0, deg(20));
                let tip = top.add(v3(
                    fx::mul(fx::sin(a), h / 3),
                    -h / 10,
                    fx::mul(fx::cos(a), h / 3),
                ));
                if let Some(af) = segment(top, tip, h / 9, h / 40) {
                    b.push(Mesh::Octa, &af, mix(c("#2f7d32"), crown, ONE / 3), 0);
                }
            }
            b.shape(Mesh::Sphere, top, v3(h / 12, h / 12, h / 12), c("#6b4f2a"));
        }
        "saguaro" => {
            let green = c("#4d7c3a");
            let w = h / 7;
            b.stand(Mesh::Cyl, x, 0, z, v3(w, h, w), green);
            b.shape(Mesh::Sphere, v3(x, h, z), v3(w, w, w), green);
            for side in [-1, 1] {
                if rng.chance(75) {
                    let y = h * rng.range(35, 55) / 100;
                    let out = v3(x + side * h / 4, y, z);
                    b.rod(Mesh::Cyl, v3(x, y, z), out, w * 3 / 4, green, 0);
                    let up = out.add(v3(0, h / 3, 0));
                    b.rod(Mesh::Cyl, out, up, w * 3 / 4, green, 0);
                    b.shape(Mesh::Sphere, up, v3(w * 3 / 4, w * 3 / 4, w * 3 / 4), green);
                }
            }
        }
        "banyan" => {
            for k in 0..5 {
                let dx = (k - 2) * h / 6;
                b.stand(
                    Mesh::Cyl,
                    x + dx,
                    0,
                    z + rng.range(-m(1), m(1)),
                    v3(h / 12, h * 2 / 3, h / 12),
                    bark,
                );
            }
            b.shape(
                Mesh::Sphere,
                v3(x, h * 3 / 4, z),
                v3(h * 2, h * 2 / 3, h * 3 / 2),
                shade(c("#3e7b35"), 95),
            );
        }
        "paloverde" => {
            b.stand(Mesh::Cyl, x, 0, z, v3(h / 14, h / 2, h / 14), c("#9acd32"));
            b.shape(Mesh::Sphere, v3(x, h * 2 / 3, z), v3(h, h / 2, h), c("#a3c94a"));
        }
        _ => {
            // oak, plane and the rest: a trunk and a round crown or two
            let light = if kind == "plane" { 112 } else { 100 };
            b.stand(Mesh::Cyl, x, 0, z, v3(h / 14, h / 2, h / 14), bark);
            b.shape(
                Mesh::Sphere,
                v3(x, h * 2 / 3, z),
                v3(h * 2 / 3, h / 2, h * 2 / 3),
                shade(crown, light),
            );
            b.shape(
                Mesh::Sphere,
                v3(x + h / 6, h * 4 / 5, z - h / 8),
                v3(h / 2, h * 2 / 5, h / 2),
                shade(crown, light + 8),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tour::Tour;

    #[test]
    fn every_venue_builds_a_scene_of_reasonable_size() {
        let t = Tour::parse(
            include_str!("../../../data/venues.json"),
            include_str!("../../../data/missions.json"),
            include_str!("../../../data/costumes.json"),
        )
        .unwrap();
        for v in &t.venues {
            let n = build(
                v,
                Fittings {
                    trapeze: true,
                    tower: true,
                },
            )
            .count();
            assert!(n > 150 && n < 6000, "{}: {n} shapes", v.id);
        }
    }

    #[test]
    fn the_same_venue_builds_the_same_scene() {
        let t = Tour::parse(
            include_str!("../../../data/venues.json"),
            include_str!("../../../data/missions.json"),
            include_str!("../../../data/costumes.json"),
        )
        .unwrap();
        let a = build(&t.venues[2], Fittings::default()).finish(0);
        let b = build(&t.venues[2], Fittings::default()).finish(0);
        assert_eq!(a, b);
    }
}
