//! The tour: venues, missions and costumes from data/, the Hasse diagram of
//! the missions, and the player's progress through them.

use crate::fx::ONE;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Leg {
    pub id: String,
    pub name: String,
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Crowd {
    pub count: i32,
    pub seats: String,
    pub colours: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Prop {
    pub kind: String,
    #[serde(default)]
    pub at: [i32; 2],
    #[serde(default)]
    pub size: [i32; 3],
    #[serde(default)]
    pub area: [i32; 4],
    #[serde(default)]
    pub from: [i32; 2],
    #[serde(default)]
    pub to: [i32; 2],
    #[serde(default)]
    pub count: i32,
    #[serde(default)]
    pub height: Heights,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub distance: i32,
    #[serde(default)]
    pub colour: String,
    #[serde(default)]
    pub colours: Vec<String>,
    #[serde(default)]
    pub windows: String,
    #[serde(default)]
    pub trim: String,
    #[serde(default)]
    pub bands: i32,
    #[serde(default)]
    pub roof: String,
    #[serde(default)]
    pub tree: String,
    #[serde(default)]
    pub snow: bool,
}

/// A height, or a range of heights to draw from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Heights(pub i32, pub i32);

impl<'de> Deserialize<'de> for Heights {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Heights, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum H {
            One(i32),
            Two([i32; 2]),
        }
        Ok(match H::deserialize(d)? {
            H::One(h) => Heights(h, h),
            H::Two([a, b]) => Heights(a, b),
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Venue {
    pub id: String,
    pub leg: String,
    pub name: String,
    pub city: String,
    pub when: String,
    pub sky: String,
    pub sky_top: String,
    pub fog: String,
    pub ground: String,
    /// The sun: compass angle and height above the horizon, degrees.
    pub light: [i32; 2],
    pub seed: u64,
    pub crowd: Crowd,
    pub props: Vec<Prop>,
}

#[derive(Debug, Clone, Deserialize)]
struct VenueFile {
    legs: Vec<Leg>,
    venues: Vec<Venue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Goal {
    Height { metres: i32, times: i32 },
    Tricks { tricks: Vec<String> },
    Routine { count: i32, score: i32 },
    Roof { trick: bool },
    Popcorn { count: i32, seconds: i32 },
    Tower { chairs: i32, hold: i32 },
    Trapeze { trick: String },
    Juggle { balls: i32, catches: i32 },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Mission {
    pub id: String,
    pub venue: String,
    pub name: String,
    pub after: Vec<String>,
    #[serde(default)]
    pub unlock: String,
    pub goal: Goal,
    pub brief: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MissionFile {
    missions: Vec<Mission>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Costume {
    pub id: String,
    pub name: String,
    pub skin: String,
    pub shirt: String,
    pub sleeve: String,
    pub pants: String,
    pub shoes: String,
    pub hat: String,
    pub hat_colour: String,
    pub accent: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CostumeFile {
    costumes: Vec<Costume>,
}

pub fn hex(s: &str) -> Result<[i32; 3], String> {
    let t = s.trim_start_matches('#');
    if t.len() != 6 {
        return Err(format!("not a colour: {s}"));
    }
    let byte = |i: usize| i32::from_str_radix(&t[i..i + 2], 16).map_err(|_| format!("not a colour: {s}"));
    Ok([byte(0)?, byte(2)?, byte(4)?])
}

/// A colour as Q16 channels.
pub fn rgb(s: &str) -> [i32; 3] {
    hex(s).unwrap_or([128, 128, 128]).map(|c| c * ONE / 255)
}

#[derive(Debug, Clone)]
pub struct Tour {
    pub legs: Vec<Leg>,
    pub venues: Vec<Venue>,
    pub missions: Vec<Mission>,
    pub costumes: Vec<Costume>,
    /// Each mission's index list of prerequisites.
    pub after: Vec<Vec<usize>>,
}

impl Tour {
    pub fn parse(venues: &str, missions: &str, costumes: &str) -> Result<Tour, String> {
        let v: VenueFile = serde_json::from_str(venues).map_err(|e| format!("venues.json: {e}"))?;
        let m: MissionFile = serde_json::from_str(missions).map_err(|e| format!("missions.json: {e}"))?;
        let c: CostumeFile = serde_json::from_str(costumes).map_err(|e| format!("costumes.json: {e}"))?;
        let mut after = Vec::new();
        for mi in &m.missions {
            if !v.venues.iter().any(|x| x.id == mi.venue) {
                return Err(format!(
                    "missions.json: {} is at unknown venue {}",
                    mi.id, mi.venue
                ));
            }
            if !mi.unlock.is_empty() && !c.costumes.iter().any(|x| x.id == mi.unlock) {
                return Err(format!(
                    "missions.json: {} unlocks unknown costume {}",
                    mi.id, mi.unlock
                ));
            }
            let mut a = Vec::new();
            for p in &mi.after {
                let j = m
                    .missions
                    .iter()
                    .position(|x| x.id == *p)
                    .ok_or_else(|| format!("missions.json: {} comes after unknown {p}", mi.id))?;
                a.push(j);
            }
            after.push(a);
        }
        for ve in &v.venues {
            if !v.legs.iter().any(|l| l.id == ve.leg) {
                return Err(format!("venues.json: {} is on unknown leg {}", ve.id, ve.leg));
            }
            for col in [&ve.sky, &ve.sky_top, &ve.fog, &ve.ground] {
                hex(col)?;
            }
        }
        let t = Tour {
            legs: v.legs,
            venues: v.venues,
            missions: m.missions,
            costumes: c.costumes,
            after,
        };
        // every chain of prerequisites ends: the diagram has no cycle
        for i in 0..t.missions.len() {
            t.level_checked(i, 0)?;
        }
        Ok(t)
    }

    fn level_checked(&self, i: usize, depth: usize) -> Result<usize, String> {
        if depth > self.missions.len() {
            return Err(format!("missions.json: {} is in a cycle", self.missions[i].id));
        }
        let mut l = 0;
        for &j in &self.after[i] {
            l = l.max(self.level_checked(j, depth + 1)? + 1);
        }
        Ok(l)
    }

    /// A mission's level in the Hasse diagram: its longest chain of prerequisites.
    pub fn level(&self, i: usize) -> usize {
        self.after[i]
            .iter()
            .map(|&j| self.level(j) + 1)
            .max()
            .unwrap_or(0)
    }

    /// The edges of the Hasse diagram: the cover relation, so a prerequisite
    /// already implied by another is not drawn.
    pub fn edges(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for i in 0..self.missions.len() {
            for &j in &self.after[i] {
                let implied = self.after[i].iter().any(|&k| k != j && self.reaches(k, j));
                if !implied {
                    out.push((j, i));
                }
            }
        }
        out
    }

    /// Whether `from` lies somewhere down the prerequisites of `of`.
    fn reaches(&self, of: usize, from: usize) -> bool {
        self.after[of].iter().any(|&k| k == from || self.reaches(k, from))
    }

    /// Where each mission sits on the map, as percentages (Q16) across and
    /// down: levels left to right, each level's missions ordered by the mean
    /// place of what they come after, then by the tour's order.
    pub fn layout(&self) -> Vec<(i32, i32)> {
        let n = self.missions.len();
        let levels: Vec<usize> = (0..n).map(|i| self.level(i)).collect();
        let top = levels.iter().copied().max().unwrap_or(0);
        let mut row = vec![0i64; n];
        let mut place = vec![(0, 0); n];
        for l in 0..=top {
            let mut members: Vec<usize> = (0..n).filter(|&i| levels[i] == l).collect();
            let key = |i: usize| -> i64 {
                if self.after[i].is_empty() {
                    i as i64 * ONE as i64
                } else {
                    self.after[i].iter().map(|&j| row[j]).sum::<i64>() / self.after[i].len() as i64
                }
            };
            members.sort_by_key(|&i| (key(i), i));
            let count = members.len() as i64;
            for (k, &i) in members.iter().enumerate() {
                row[i] = ((k as i64 * 2 + 1) * ONE as i64 * 50) / count;
                let x = ((l as i64 * 2 + 1) * ONE as i64 * 50) / (top as i64 + 1);
                place[i] = (x as i32, row[i] as i32);
            }
        }
        place
    }

    pub fn venue(&self, id: &str) -> &Venue {
        self.venues.iter().find(|v| v.id == id).unwrap_or(&self.venues[0])
    }

    pub fn venue_index(&self, id: &str) -> usize {
        self.venues.iter().position(|v| v.id == id).unwrap_or(0)
    }

    pub fn costume(&self, id: &str) -> &Costume {
        self.costumes
            .iter()
            .find(|c| c.id == id)
            .unwrap_or(&self.costumes[0])
    }
}

/// What the player has done: the best grade on each mission passed, and the
/// costume worn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub version: u32,
    pub best: BTreeMap<String, i32>,
    pub costume: String,
}

pub const SAVE_VERSION: u32 = 1;

impl Progress {
    pub fn load(text: &str) -> Progress {
        serde_json::from_str::<Progress>(text)
            .ok()
            .filter(|p| p.version == SAVE_VERSION)
            .unwrap_or(Progress {
                version: SAVE_VERSION,
                best: BTreeMap::new(),
                costume: String::new(),
            })
    }

    pub fn save(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn passed(&self, id: &str) -> bool {
        self.best.contains_key(id)
    }

    pub fn open(&self, tour: &Tour, i: usize) -> bool {
        tour.after[i].iter().all(|&j| self.passed(&tour.missions[j].id))
    }

    pub fn unlocked(&self, tour: &Tour, costume: &str) -> bool {
        let earned_by: Vec<&Mission> = tour.missions.iter().filter(|m| m.unlock == costume).collect();
        earned_by.is_empty() || earned_by.iter().any(|m| self.passed(&m.id))
    }

    pub fn record(&mut self, id: &str, grade: i32) {
        let e = self.best.entry(id.to_string()).or_insert(grade);
        *e = (*e).max(grade);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn tour() -> Tour {
        Tour::parse(
            include_str!("../../../data/venues.json"),
            include_str!("../../../data/missions.json"),
            include_str!("../../../data/costumes.json"),
        )
        .unwrap()
    }

    #[test]
    fn the_tour_starts_in_durham_at_the_rtp_800_building() {
        let t = tour();
        assert_eq!(t.venues[0].id, "rtp");
        assert_eq!(t.legs[0].id, "durham");
        assert_eq!(t.missions[0].venue, "rtp");
        assert_eq!(t.level(0), 0);
    }

    #[test]
    fn levels_count_the_longest_chain() {
        let t = tour();
        let at = |id: &str| t.missions.iter().position(|m| m.id == id).unwrap();
        // m07 comes after m05, which comes after m03, after m01: level 3
        assert_eq!(t.level(at("m07")), 3);
        assert_eq!(t.level(at("m03")), 1);
        // the finale is the one mission at the top level
        let top = (0..t.missions.len()).map(|i| t.level(i)).max().unwrap();
        let tops: Vec<_> = (0..t.missions.len()).filter(|&i| t.level(i) == top).collect();
        assert_eq!(tops, vec![at("m26")]);
    }

    #[test]
    fn the_diagram_draws_covers_only() {
        let t = tour();
        let at = |id: &str| t.missions.iter().position(|m| m.id == id).unwrap();
        // m16 comes after m05 and m08; both cover it (neither reaches the other)
        let e = t.edges();
        assert!(e.contains(&(at("m05"), at("m16"))));
        assert!(e.contains(&(at("m08"), at("m16"))));
        // m07 after m02, m03 and m05: m03 is below m05, so it is not drawn
        assert!(!e.contains(&(at("m03"), at("m07"))));
        assert!(e.contains(&(at("m05"), at("m07"))));
    }

    #[test]
    fn a_mission_opens_when_everything_before_it_is_passed() {
        let t = tour();
        let mut p = Progress::load("");
        assert!(p.open(&t, 0));
        assert!(!p.open(&t, 1));
        p.record("m01", 70);
        assert!(p.open(&t, 1));
        p.record("m01", 60);
        assert_eq!(p.best["m01"], 70);
        assert!(p.unlocked(&t, "cook"));
        assert!(!p.unlocked(&t, "striped"));
        p.record("m03", 90);
        assert!(p.unlocked(&t, "striped"));
        let back = Progress::load(&p.save());
        assert_eq!(back, p);
    }

    #[test]
    fn every_venue_is_used_and_the_layout_stays_on_the_map() {
        let t = tour();
        for v in &t.venues {
            assert!(
                t.missions.iter().any(|m| m.venue == v.id),
                "{} has no mission",
                v.id
            );
        }
        for (x, y) in t.layout() {
            assert!(x > 0 && x < 100 * ONE && y > 0 && y < 100 * ONE);
        }
    }
}

#[cfg(test)]
mod counts {
    use super::tests::tour;

    /// The README's counts come from the data, not from a hand.
    #[test]
    fn the_readme_counts_the_shows_and_venues_in_the_data() {
        let t = tour();
        let readme = include_str!("../../../README.md");
        let line = format!("{} shows at {} venues", t.missions.len(), t.venues.len());
        assert!(readme.contains(&line), "README should say {line:?}");
        assert_eq!(readme.contains("**Costumes.** Twelve"), t.costumes.len() == 12);
    }
}
