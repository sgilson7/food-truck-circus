//! Naming a flight and grading its landing, from data/tricks.json.
//!
//! A flight is named by the first row that matches what it did. A landing's
//! grade is 100 less the degrees it was off the ideal and off a whole half
//! twist (`balance::SCORE_PER_DEGREE`); the word the crowd calls comes from
//! Slushline's thresholds.

use crate::balance::{SCORE_PER_DEGREE, SCORE_PER_TWIST_DEGREE};
use crate::body::cm;
use crate::performer::{Landing, Posture};
use crate::rng::Rng;
use crate::stage::Surface;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
struct Row {
    id: String,
    name: String,
    from: String,
    to: String,
    q: i32,
    twists: i32,
    pose: String,
    value: i32,
    #[serde(default)]
    min_cm: i32,
    #[serde(default)]
    how: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Judgement {
    at: i32,
    word: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Flair {
    pct: i32,
    words: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BookFile {
    judgements: Vec<Judgement>,
    flair: Flair,
    freestyle: String,
    roof: String,
    ground: String,
    trapeze: String,
    roof_how: String,
    tricks: Vec<Row>,
}

#[derive(Debug, Clone)]
pub struct TrickBook {
    f: BookFile,
}

/// A named flight: the row's id (or "freestyle"), the name shown, the points
/// before grading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub id: String,
    pub name: String,
    pub value: i32,
}

/// What the shape held most of a flight was.
pub fn dominant(l: &Landing) -> &'static str {
    let f = &l.flight;
    if f.tuck >= f.pike && f.tuck * 3 >= f.ticks && f.tuck > 0 {
        "tuck"
    } else if f.pike > f.tuck && f.pike * 3 >= f.ticks {
        "pike"
    } else {
        "straight"
    }
}

fn quarters_text(q: i32) -> String {
    let whole = q / 4;
    let part = ["", "¼", "½", "¾"][(q % 4) as usize];
    if whole == 0 && !part.is_empty() {
        part.to_string()
    } else {
        format!("{whole}{part}")
    }
}

impl TrickBook {
    pub fn parse(text: &str) -> Result<TrickBook, String> {
        let f: BookFile = serde_json::from_str(text).map_err(|e| format!("tricks.json: {e}"))?;
        if f.judgements.last().map(|j| j.at) != Some(0) {
            return Err("tricks.json: the last judgement must be at 0".into());
        }
        Ok(TrickBook { f })
    }

    /// The trick a landing completes, or `None` for a plain bounce.
    pub fn name(&self, l: &Landing) -> Option<Named> {
        let pose = dominant(l);
        let from = if l.flight.trapeze {
            Posture::Feet
        } else {
            l.flight.from
        };
        let row = self.f.tricks.iter().find(|r| {
            r.from == from.name()
                && r.to == l.to.name()
                && r.q == l.quarters
                && r.twists == l.half_twists
                && (r.pose == "any" || r.pose == pose)
                && l.height >= cm(r.min_cm)
        });
        let mut named = match row {
            Some(r) => Named {
                id: r.id.clone(),
                name: r.name.clone(),
                value: r.value,
            },
            None => {
                let q = l.quarters.abs();
                if q < 2 && l.half_twists == 0 {
                    // a straight bounce, or a jump too low to be a tuck jump
                    return None;
                }
                Named {
                    id: "freestyle".into(),
                    name: self
                        .f
                        .freestyle
                        .replace("{flips}", &quarters_text(q))
                        .replace("{twists}", &quarters_text(l.half_twists * 2)),
                    value: q * 7 + l.half_twists * 5,
                }
            }
        };
        if l.flight.trapeze && named.value > 0 {
            named.name = self.f.trapeze.replace("{trick}", &named.name);
            named.value += 10;
        }
        match l.surface {
            Surface::Roof if l.flight.from_surface != Surface::Roof => {
                named.name = self.f.roof.replace("{trick}", &named.name);
                named.value += 15;
            }
            Surface::Ground if l.flight.from_surface == Surface::Bed && named.value > 0 => {
                named.name = self.f.ground.replace("{trick}", &named.name);
            }
            _ => {}
        }
        Some(named)
    }

    /// The landing's grade out of 100.
    pub fn grade(&self, l: &Landing) -> i32 {
        let lost = l.off * SCORE_PER_DEGREE + l.twist_off * SCORE_PER_TWIST_DEGREE;
        (100 - lost / 100).clamp(0, 100)
    }

    /// The word for a grade, swapped for its flair word `pct` times in a
    /// hundred, drawn from the game's one Rng.
    pub fn word(&self, grade: i32, rng: &mut Rng) -> String {
        let w = self
            .f
            .judgements
            .iter()
            .find(|j| grade >= j.at)
            .map(|j| j.word.clone())
            .unwrap_or_default();
        self.flair(w, rng)
    }

    pub fn flair(&self, word: String, rng: &mut Rng) -> String {
        if rng.chance(self.f.flair.pct) {
            if let Some(f) = self.f.flair.words.get(&word) {
                return f.clone();
            }
        }
        word
    }

    pub fn top_word(&self) -> &str {
        &self.f.judgements[0].word
    }

    /// The word a grade earns, without flair (for mission results).
    pub fn plain_word(&self, grade: i32) -> String {
        self.f
            .judgements
            .iter()
            .find(|j| grade >= j.at)
            .map(|j| j.word.clone())
            .unwrap_or_default()
    }

    /// Every move once, in the file's order: id, name, how to do it, the
    /// position it starts and ends in, and its points.
    pub fn glossary(&self) -> Vec<(String, String, String, String, String, i32)> {
        let mut out: Vec<(String, String, String, String, String, i32)> = Vec::new();
        for r in &self.f.tricks {
            if r.value > 0 && !out.iter().any(|g| g.0 == r.id) {
                out.push((
                    r.id.clone(),
                    r.name.clone(),
                    r.how.clone(),
                    r.from.clone(),
                    r.to.clone(),
                    r.value,
                ));
            }
        }
        out
    }

    pub fn roof_how(&self) -> &str {
        &self.f.roof_how
    }

    pub fn names(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for r in &self.f.tricks {
            if !out.iter().any(|(id, _)| *id == r.id) {
                out.push((r.id.clone(), r.name.clone()));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::int;
    use crate::performer::Flight;

    fn book() -> TrickBook {
        TrickBook::parse(include_str!("../../../data/tricks.json")).unwrap()
    }

    fn landing(from: Posture, to: Posture, q: i32, halves: i32, tuck: i32) -> Landing {
        Landing {
            flight: Flight {
                from,
                from_surface: Surface::Bed,
                flip0: 0,
                twist0: 0,
                base_y: 0,
                peak: int(3),
                ticks: 100,
                tuck,
                pike: 0,
                straight: 100 - tuck,
                returning: false,
                trapeze: false,
            },
            to,
            surface: Surface::Bed,
            quarters: q,
            half_twists: halves,
            off: 0,
            twist_off: 0,
            height: int(3),
        }
    }

    #[test]
    fn flights_get_their_trampoline_names() {
        let b = book();
        let n = |l: Landing| b.name(&l).map(|n| n.id).unwrap_or_default();
        assert_eq!(n(landing(Posture::Feet, Posture::Feet, 4, 0, 80)), "back_tuck");
        assert_eq!(n(landing(Posture::Feet, Posture::Feet, 4, 0, 0)), "back_layout");
        assert_eq!(n(landing(Posture::Feet, Posture::Feet, -4, 1, 0)), "barani");
        assert_eq!(n(landing(Posture::Feet, Posture::Feet, -4, 3, 0)), "rudy");
        assert_eq!(n(landing(Posture::Feet, Posture::Feet, 8, 0, 70)), "double_back");
        assert_eq!(n(landing(Posture::Feet, Posture::Seat, 0, 0, 0)), "seat_drop");
        assert_eq!(n(landing(Posture::Seat, Posture::Seat, 0, 1, 0)), "swivel_hips");
        assert_eq!(n(landing(Posture::Back, Posture::Back, 0, 1, 0)), "cradle");
        assert_eq!(n(landing(Posture::Back, Posture::Feet, -1, 0, 0)), "to_feet");
        // a straight bounce is no trick
        assert!(b.name(&landing(Posture::Feet, Posture::Feet, 0, 0, 0)).is_none());
    }

    #[test]
    fn an_unlisted_flight_is_freestyle_with_its_turns_counted() {
        let b = book();
        let n = b.name(&landing(Posture::Feet, Posture::Feet, 6, 3, 0)).unwrap();
        assert_eq!(n.id, "freestyle");
        assert_eq!(n.name, "Freestyle: 1½ flips, 1½ twists");
        assert_eq!(n.value, 6 * 7 + 3 * 5);
    }

    #[test]
    fn a_roof_landing_says_so_and_scores_more() {
        let b = book();
        let mut l = landing(Posture::Feet, Posture::Feet, 4, 0, 80);
        l.surface = Surface::Roof;
        let n = b.name(&l).unwrap();
        assert_eq!(n.name, "Back tuck to the truck roof");
        assert_eq!(n.value, 30 + 15);
    }

    #[test]
    fn grades_fall_with_degrees_off_and_words_follow_slushline() {
        let b = book();
        let mut l = landing(Posture::Feet, Posture::Feet, 4, 0, 80);
        assert_eq!(b.grade(&l), 100);
        l.off = 10; // 10 x 1.1 = 11 points
        assert_eq!(b.grade(&l), 89);
        assert_eq!(b.plain_word(89), "excellent");
        assert_eq!(b.plain_word(87), "great");
        assert_eq!(b.plain_word(44), "miss");
        l.off = 200;
        assert_eq!(b.grade(&l), 0);
    }

    #[test]
    fn every_move_in_the_glossary_says_how_to_do_it() {
        let b = book();
        let g = b.glossary();
        assert!(g.len() >= 30);
        for (id, name, how, ..) in &g {
            assert!(how.len() > 20, "{id} ({name}) has no how-to");
        }
        assert!(
            !g.iter().any(|m| m.0 == "to_feet"),
            "a move between positions is not a trick"
        );
    }

    #[test]
    fn flair_swaps_about_thirty_in_a_hundred() {
        let b = book();
        let mut rng = Rng::new(7);
        let swapped = (0..1000).filter(|_| b.word(95, &mut rng) == "groovy").count();
        assert!((240..360).contains(&swapped), "{swapped}");
    }
}
