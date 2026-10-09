//! The crowd: people on blankets, in lawn chairs or standing, in rows round
//! the stage, as the listings say to come (blankets or lawn chairs). They
//! bob, and throw their arms up when something lands well; a few of them
//! shout the word.

use crate::fx::{self, deg, int, ratio, ONE, TURN};
use crate::geom::{v3, V3};
use crate::mesh::Mesh;
use crate::render::{shade, Batch, Rgb};
use crate::rng::Rng;
use crate::tour::{rgb, Venue};

const SKIN: [&str; 7] = [
    "#f1c27d", "#e0ac69", "#c68642", "#8d5524", "#ffdbac", "#a1665e", "#5c3b28",
];
const HAIR: [&str; 6] = ["#2b1b0e", "#4a2f1b", "#111111", "#c9a15b", "#7a4b2a", "#a3a3a3"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub at: V3,
    pub yaw: i32,
    pub seated: bool,
    pub chair: bool,
    pub blanket: Option<Rgb>,
    pub shirt: Rgb,
    pub pants: Rgb,
    pub skin: Rgb,
    pub hair: Rgb,
    pub phase: i32,
    pub size: i32,
    /// Shouting until this tick.
    pub shout: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crowd {
    pub people: Vec<Person>,
    /// How excited, 0 to ONE; it rises with a good landing and fades.
    pub excite: i32,
}

impl Crowd {
    pub fn new(v: &Venue) -> Crowd {
        let mut rng = Rng::new(v.seed ^ 0x00c0_ffee);
        let standing = v.crowd.seats == "standing";
        let chairs = v.crowd.seats == "lawn_chairs";
        let spacing = if standing { ratio(85, 100) } else { ratio(12, 10) };
        let mut people = Vec::new();
        let mut row = 0;
        while (people.len() as i32) < v.crowd.count && row < 12 {
            let r = int(9) + row * if standing { ratio(9, 10) } else { ratio(15, 10) };
            let span = deg(58);
            let seats = (fx::mul(r, 2 * span) * 6 / TURN * ONE / spacing / ONE).max(1);
            let arc = (r as i64 * 2 * span as i64 * 6283 / 1000 / TURN as i64) as i32;
            let seats = seats.max(arc / spacing);
            for k in 0..seats {
                if (people.len() as i32) >= v.crowd.count {
                    break;
                }
                if rng.chance(12) {
                    continue;
                }
                let a = -span + 2 * span * k / seats.max(1) + rng.range(-deg(1), deg(1));
                let at = v3(fx::mul(fx::sin(a), r), 0, int(1) + fx::mul(fx::cos(a), r));
                let pick =
                    |rng: &mut Rng, list: &[String]| rgb(&list[rng.range(0, list.len() as i32 - 1) as usize]);
                let shirt = pick(&mut rng, &v.crowd.colours);
                let pants = shade(pick(&mut rng, &v.crowd.colours), 60);
                let blanket = if !standing && !chairs {
                    Some(pick(&mut rng, &v.crowd.colours))
                } else {
                    None
                };
                people.push(Person {
                    at,
                    // facing the stage
                    yaw: -a,
                    seated: !standing,
                    chair: chairs,
                    blanket,
                    shirt,
                    pants,
                    skin: rgb(SKIN[rng.range(0, SKIN.len() as i32 - 1) as usize]),
                    hair: rgb(HAIR[rng.range(0, HAIR.len() as i32 - 1) as usize]),
                    phase: rng.range(0, TURN - 1),
                    size: rng.range(85, 105),
                    shout: 0,
                });
            }
            row += 1;
        }
        Crowd {
            people,
            excite: ONE / 6,
        }
    }

    pub fn cheer(&mut self, amount: i32) {
        self.excite = (self.excite + amount).min(ONE);
    }

    pub fn tick(&mut self) {
        self.excite = (self.excite - ONE / 400).max(ONE / 8);
    }

    /// Picks people to shout until `until`; returns their indices.
    pub fn shouters(&mut self, rng: &mut Rng, n: usize, until: i64) -> Vec<usize> {
        let mut out = Vec::new();
        if self.people.is_empty() {
            return out;
        }
        for _ in 0..n {
            let i = rng.range(0, self.people.len() as i32 - 1) as usize;
            if !out.contains(&i) {
                self.people[i].shout = until;
                out.push(i);
            }
        }
        out
    }

    pub fn head(&self, i: usize) -> V3 {
        let p = &self.people[i];
        let h = if p.seated {
            ratio(105, 100)
        } else {
            ratio(165, 100)
        };
        p.at.add(v3(0, h * p.size / 100 + ratio(3, 10), 0))
    }

    pub fn draw(&self, b: &mut Batch, tick: i64) {
        let t = (tick as i32).wrapping_mul(deg(9)) & (TURN - 1);
        for p in &self.people {
            let fwd = v3(-fx::sin(p.yaw), 0, -fx::cos(p.yaw));
            let side = v3(fx::cos(p.yaw), 0, -fx::sin(p.yaw));
            let s = p.size;
            let sz = |n: i32| n * s / 100;
            let wave = fx::sin(t + p.phase);
            let hype = self.excite + if p.shout > tick { ONE / 2 } else { 0 };
            let bob = fx::mul(fx::mul(wave.abs(), hype.min(ONE)), ratio(12, 100));
            let base = p.at;
            if let Some(bl) = p.blanket {
                b.shape(
                    Mesh::Cube,
                    base.add(fwd.scale(ratio(2, 10))).add(v3(0, ONE / 100, 0)),
                    v3(ratio(14, 10), ONE / 50, ratio(14, 10)),
                    bl,
                );
            }
            let hip_y = if p.seated {
                if p.chair {
                    ratio(45, 100)
                } else {
                    ratio(12, 100)
                }
            } else {
                ratio(90, 100)
            };
            let hip = base.add(v3(0, sz(hip_y) + if p.seated { bob / 3 } else { bob }, 0));
            if p.chair {
                b.shape(
                    Mesh::Cube,
                    base.add(v3(0, ratio(38, 100), 0)),
                    v3(ratio(55, 100), ONE / 20, ratio(55, 100)),
                    rgb("#3b82c4"),
                );
                b.rod(
                    Mesh::Cube,
                    base.sub(fwd.scale(ratio(25, 100))).add(v3(0, ratio(38, 100), 0)),
                    base.sub(fwd.scale(ratio(35, 100))).add(v3(0, ratio(95, 100), 0)),
                    ratio(5, 10),
                    rgb("#3b82c4"),
                    0,
                );
            }
            // legs: forward along the ground when seated, down when standing
            for k in [-1, 1] {
                let h = hip.add(side.scale(k * ratio(1, 10)));
                let foot = if p.seated {
                    if p.chair {
                        base.add(fwd.scale(ratio(45, 100)))
                            .add(side.scale(k * ratio(12, 100)))
                    } else {
                        h.add(fwd.scale(sz(ratio(7, 10))))
                            .add(v3(0, -h.y + ratio(8, 100), 0))
                    }
                } else {
                    base.add(side.scale(k * ratio(12, 100)))
                };
                b.rod(Mesh::Cyl, h, foot, ratio(15, 100), p.pants, 0);
            }
            let chest = hip.add(v3(0, sz(ratio(58, 100)), 0));
            if let Some(af) = crate::render::segment(hip, chest, ratio(40, 100) * s / 100, ratio(24, 100)) {
                b.push(Mesh::Cube, &af, p.shirt, 0);
            }
            let head = chest.add(v3(0, sz(ratio(25, 100)), 0));
            b.shape(
                Mesh::Sphere,
                head,
                v3(sz(ratio(24, 100)), sz(ratio(27, 100)), sz(ratio(24, 100))),
                p.skin,
            );
            b.shape(
                Mesh::Sphere,
                head.add(v3(0, ratio(6, 100), 0)).sub(fwd.scale(ratio(3, 100))),
                v3(sz(ratio(25, 100)), sz(ratio(18, 100)), sz(ratio(25, 100))),
                p.hair,
            );
            // arms: up when cheering hard or shouting, down otherwise
            let up = hype > ONE * 3 / 5 && (p.phase & 3) != 0 || p.shout > tick;
            for k in [-1, 1] {
                let sh = chest
                    .add(side.scale(k * sz(ratio(22, 100))))
                    .sub(v3(0, ratio(5, 100), 0));
                let hand = if up {
                    sh.add(v3(0, sz(ratio(55, 100)) + bob, 0))
                        .add(side.scale(k * ratio(15, 100)))
                } else {
                    sh.add(v3(0, -sz(ratio(45, 100)), 0))
                        .add(fwd.scale(ratio(12, 100)))
                };
                b.rod(Mesh::Cyl, sh, hand, ratio(10, 100), p.shirt, 0);
                b.shape(
                    Mesh::Sphere,
                    hand,
                    v3(ratio(9, 100), ratio(9, 100), ratio(9, 100)),
                    p.skin,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tour::Tour;

    #[test]
    fn every_venue_seats_its_crowd_clear_of_the_stage() {
        let t = Tour::parse(
            include_str!("../../../data/venues.json"),
            include_str!("../../../data/missions.json"),
            include_str!("../../../data/costumes.json"),
        )
        .unwrap();
        for v in &t.venues {
            let c = Crowd::new(v);
            assert_eq!(c.people.len() as i32, v.crowd.count, "{}", v.id);
            for p in &c.people {
                // nobody sits on the bed or in front of the truck
                assert!(p.at.z > int(5), "{}: {:?}", v.id, p.at);
            }
        }
    }
}
