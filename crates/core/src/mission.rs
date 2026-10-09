//! A mission in play: what counts toward its goal, when it is passed or
//! failed, and the grade it ends with.

use crate::balance::TICKS_PER_SECOND;
use crate::body::cm;
use crate::stage::Surface;
use crate::tour::Goal;
use serde::Serialize;

/// Something that happened, as a mission hears it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// A landing: the trick's id and points (none for a plain bounce), the
    /// grade, the surface, and whether the flight left the trapeze.
    Landed {
        trick: Option<(String, i32)>,
        grade: i32,
        surface: Surface,
        trapeze: bool,
        feet: bool,
    },
    Crashed,
    /// The top of a bounce, the feet's height above the bed.
    Peak(i32),
    Popcorn(i32),
    ChairPlaced(i32),
    Handstand,
    TowerHeld(i32),
    TowerFell,
    Caught(i32),
    Dropped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Outcome {
    pub passed: bool,
    pub grade: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub goal: Goal,
    pub ticks: i32,
    /// How far along: heights reached, popcorn caught, catches, the chain.
    pub count: i32,
    pub grades: Vec<i32>,
    /// For a trick list, which are done.
    pub done: Vec<bool>,
    pub chain_score: i32,
    pub outcome: Option<Outcome>,
}

fn mean(v: &[i32]) -> i32 {
    if v.is_empty() {
        100
    } else {
        v.iter().sum::<i32>() / v.len() as i32
    }
}

impl Run {
    pub fn new(goal: Goal) -> Run {
        let done = match &goal {
            Goal::Tricks { tricks } => vec![false; tricks.len()],
            _ => Vec::new(),
        };
        Run {
            goal,
            ticks: 0,
            count: 0,
            grades: Vec::new(),
            done,
            chain_score: 0,
            outcome: None,
        }
    }

    /// The number the goal counts to, for the HUD.
    pub fn target(&self) -> i32 {
        match &self.goal {
            Goal::Height { times, .. } => *times,
            Goal::Tricks { tricks } => tricks.len() as i32,
            Goal::Routine { count, .. } => *count,
            Goal::Roof { .. } | Goal::Trapeze { .. } => 1,
            Goal::Popcorn { count, .. } => *count,
            Goal::Tower { chairs, .. } => *chairs,
            Goal::Juggle { catches, .. } => *catches,
        }
    }

    /// Seconds left on a timed goal.
    pub fn seconds_left(&self) -> Option<i32> {
        match &self.goal {
            Goal::Popcorn { seconds, .. } => {
                Some((seconds * TICKS_PER_SECOND - self.ticks).max(0) / TICKS_PER_SECOND)
            }
            _ => None,
        }
    }

    fn finish(&mut self, passed: bool, grade: i32) {
        if self.outcome.is_none() {
            self.outcome = Some(Outcome {
                passed,
                grade: grade.clamp(0, 100),
            });
        }
    }

    pub fn tick(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        self.ticks += 1;
        if let Goal::Popcorn { seconds, .. } = self.goal {
            if self.ticks >= seconds * TICKS_PER_SECOND {
                self.finish(false, 0);
            }
        }
    }

    pub fn hear(&mut self, s: &Signal) {
        if self.outcome.is_some() {
            return;
        }
        match (&self.goal.clone(), s) {
            (Goal::Height { metres, times }, Signal::Peak(h)) if *h >= cm(metres * 100) => {
                self.count += 1;
                if self.count >= *times {
                    self.finish(true, 100);
                }
            }
            (
                Goal::Tricks { tricks },
                Signal::Landed {
                    trick: Some((id, _)),
                    grade,
                    ..
                },
            ) => {
                if let Some(k) = (0..tricks.len()).find(|&k| !self.done[k] && tricks[k] == *id) {
                    self.done[k] = true;
                    self.count += 1;
                    self.grades.push(*grade);
                    if self.done.iter().all(|d| *d) {
                        let g = mean(&self.grades);
                        self.finish(true, g);
                    }
                }
            }
            (
                Goal::Routine { count, score },
                Signal::Landed {
                    trick: Some((_, value)),
                    grade,
                    ..
                },
            ) if *value > 0 => {
                self.count += 1;
                self.grades.push(*grade);
                self.chain_score += value * grade / 100;
                if self.count >= *count && self.chain_score >= *score {
                    let g = mean(&self.grades);
                    self.finish(true, g);
                }
            }
            (Goal::Routine { .. }, Signal::Crashed) => {
                self.count = 0;
                self.chain_score = 0;
                self.grades.clear();
            }
            (
                Goal::Roof { trick },
                Signal::Landed {
                    trick: t,
                    grade,
                    surface: Surface::Roof,
                    ..
                },
            ) if (!trick || t.as_ref().map(|(_, v)| *v > 0).unwrap_or(false)) => {
                self.count = 1;
                self.finish(true, *grade);
            }
            (Goal::Popcorn { count, seconds }, Signal::Popcorn(n)) => {
                self.count += n;
                if self.count >= *count {
                    let limit = seconds * TICKS_PER_SECOND;
                    let left = (limit - self.ticks).max(0);
                    self.finish(true, 60 + 40 * left / limit);
                }
            }
            (Goal::Tower { .. }, Signal::ChairPlaced(g)) => {
                self.count += 1;
                self.grades.push(*g);
            }
            (Goal::Tower { .. }, Signal::TowerHeld(g)) => {
                self.grades.push(*g);
                let g = mean(&self.grades);
                self.finish(true, g);
            }
            (Goal::Tower { .. }, Signal::TowerFell) => self.finish(false, 0),
            (
                Goal::Trapeze { trick },
                Signal::Landed {
                    trick: t,
                    grade,
                    trapeze: true,
                    feet,
                    ..
                },
            ) => {
                let ok = if trick.is_empty() {
                    *feet
                } else {
                    t.as_ref().map(|(id, _)| id == trick).unwrap_or(false)
                };
                if ok {
                    self.count = 1;
                    self.finish(true, *grade);
                }
            }
            (Goal::Juggle { catches, .. }, Signal::Caught(g)) => {
                self.count += 1;
                self.grades.push(*g);
                if self.count >= *catches {
                    let g = mean(&self.grades);
                    self.finish(true, g);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn landed(id: &str, value: i32, grade: i32) -> Signal {
        Signal::Landed {
            trick: Some((id.into(), value)),
            grade,
            surface: Surface::Bed,
            trapeze: false,
            feet: true,
        }
    }

    #[test]
    fn a_trick_list_passes_when_every_trick_is_done_and_grades_their_mean() {
        let mut r = Run::new(Goal::Tricks {
            tricks: vec!["seat_drop".into(), "seat_drop".into()],
        });
        r.hear(&landed("seat_drop", 8, 90));
        assert!(r.outcome.is_none());
        r.hear(&landed("back_tuck", 30, 100));
        assert!(r.outcome.is_none());
        r.hear(&landed("seat_drop", 8, 70));
        assert_eq!(
            r.outcome,
            Some(Outcome {
                passed: true,
                grade: 80
            })
        );
    }

    #[test]
    fn a_crash_starts_a_routine_over() {
        let mut r = Run::new(Goal::Routine { count: 3, score: 50 });
        r.hear(&landed("back_tuck", 30, 100));
        r.hear(&landed("back_tuck", 30, 100));
        r.hear(&Signal::Crashed);
        assert_eq!(r.count, 0);
        r.hear(&landed("back_tuck", 30, 100));
        r.hear(&landed("to_feet", 0, 100));
        r.hear(&landed("seat_drop", 8, 50));
        assert!(r.outcome.is_none());
        r.hear(&landed("back_tuck", 30, 80));
        // 30 + 4 + 24 = 58 points over three tricks
        assert_eq!(r.chain_score, 58);
        assert_eq!(r.outcome.map(|o| o.passed), Some(true));
    }

    #[test]
    fn popcorn_runs_out_of_time() {
        let mut r = Run::new(Goal::Popcorn { count: 5, seconds: 1 });
        r.hear(&Signal::Popcorn(4));
        for _ in 0..TICKS_PER_SECOND {
            r.tick();
        }
        assert_eq!(
            r.outcome,
            Some(Outcome {
                passed: false,
                grade: 0
            })
        );
        // caught at the half: 60 + 20
        let mut r = Run::new(Goal::Popcorn { count: 5, seconds: 2 });
        for _ in 0..TICKS_PER_SECOND {
            r.tick();
        }
        r.hear(&Signal::Popcorn(5));
        assert_eq!(
            r.outcome,
            Some(Outcome {
                passed: true,
                grade: 80
            })
        );
    }

    #[test]
    fn heights_count_only_peaks_that_reach_the_mark() {
        let mut r = Run::new(Goal::Height { metres: 3, times: 2 });
        r.hear(&Signal::Peak(cm(290)));
        r.hear(&Signal::Peak(cm(310)));
        assert!(r.outcome.is_none());
        r.hear(&Signal::Peak(cm(300)));
        assert!(r.outcome.unwrap().passed);
    }

    #[test]
    fn a_trapeze_goal_needs_the_flight_to_leave_the_trapeze() {
        let mut r = Run::new(Goal::Trapeze {
            trick: "back_tuck".into(),
        });
        r.hear(&landed("back_tuck", 30, 90));
        assert!(r.outcome.is_none());
        r.hear(&Signal::Landed {
            trick: Some(("back_tuck".into(), 40)),
            grade: 90,
            surface: Surface::Bed,
            trapeze: true,
            feet: true,
        });
        assert!(r.outcome.unwrap().passed);
    }
}
