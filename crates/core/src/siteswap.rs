//! Juggling patterns as numbers (siteswap).
//!
//! A juggler throws on a steady beat, hands taking turns. Each digit says
//! how many beats later the ball thrown on that beat comes down, so a bigger
//! number is a higher throw: "3" is the three-ball cascade, "441" and "531"
//! are three-ball tricks, "2" holds a ball, "0" is an empty hand. A pattern
//! repeats. It can be juggled exactly when no two throws come down on the
//! same beat, and then the number of balls is the average of its digits.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    pub throws: Vec<u32>,
}

/// Why a pattern cannot be juggled, with what the page says about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Problem {
    Empty,
    NotDigit {
        at: usize,
        ch: String,
    },
    /// The digits do not average to a whole number.
    Average {
        sum: u32,
        n: usize,
    },
    /// Two throws come down on the same beat (counting from 1 for people).
    Collision {
        a: usize,
        b: usize,
        lands: usize,
    },
}

pub fn parse(src: &str) -> Result<Pattern, Problem> {
    let mut throws = Vec::new();
    for (i, c) in src.chars().enumerate() {
        match c {
            ' ' | ',' => {}
            '0'..='9' => throws.push(c as u32 - '0' as u32),
            _ => {
                return Err(Problem::NotDigit {
                    at: i + 1,
                    ch: c.to_string(),
                })
            }
        }
    }
    if throws.is_empty() {
        return Err(Problem::Empty);
    }
    Ok(Pattern { throws })
}

impl Pattern {
    pub fn period(&self) -> usize {
        self.throws.len()
    }

    pub fn sum(&self) -> u32 {
        self.throws.iter().sum()
    }

    /// The number of balls, or why there is no such number.
    pub fn balls(&self) -> Result<u32, Problem> {
        let n = self.period();
        let sum = self.sum();
        // the average first: a pattern that needs half a ball is no pattern
        if !(sum as usize).is_multiple_of(n) {
            return Err(Problem::Average { sum, n });
        }
        // where each throw lands in the cycle: beat i's ball comes down on i + t
        let mut landed: Vec<Option<usize>> = vec![None; n];
        for (i, &t) in self.throws.iter().enumerate() {
            let at = (i + t as usize) % n;
            if let Some(other) = landed[at] {
                return Err(Problem::Collision {
                    a: other + 1,
                    b: i + 1,
                    lands: i + t as usize + 1,
                });
            }
            landed[at] = Some(i);
        }
        Ok(sum / n as u32)
    }

    pub fn at(&self, beat: usize) -> u32 {
        self.throws[beat % self.period()]
    }
}

/// One throw in a run of the pattern: on which beat, how high, which ball.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Throw {
    pub beat: usize,
    pub value: u32,
    pub ball: usize,
}

/// The throws of `beats` beats of a valid pattern. Balls come out of the
/// pocket as they are first needed: a throw with no ball coming down to its
/// hand takes a new one, until all are in the air.
pub fn schedule(p: &Pattern, balls: u32, beats: usize) -> Vec<Throw> {
    let mut landing: Vec<Option<usize>> = vec![None; beats + 16];
    let mut out = Vec::new();
    let mut next_ball = 0usize;
    for beat in 0..beats {
        let t = p.at(beat);
        if t == 0 {
            continue;
        }
        let ball = match landing[beat].take() {
            Some(b) => b,
            None if (next_ball as u32) < balls => {
                next_ball += 1;
                next_ball - 1
            }
            None => continue,
        };
        out.push(Throw { beat, value: t, ball });
        if beat + (t as usize) < landing.len() {
            landing[beat + t as usize] = Some(ball);
        }
    }
    out
}

/// Which hand throws on a beat: they take turns, 0 then 1.
pub fn hand(beat: usize) -> usize {
    beat % 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn balls(s: &str) -> Result<u32, Problem> {
        parse(s).unwrap().balls()
    }

    #[test]
    fn the_number_of_balls_is_the_average_of_the_digits() {
        assert_eq!(balls("3"), Ok(3));
        assert_eq!(balls("441"), Ok(3));
        assert_eq!(balls("531"), Ok(3));
        assert_eq!(balls("51"), Ok(3));
        assert_eq!(balls("4"), Ok(4));
        assert_eq!(balls("720"), Ok(3));
        assert_eq!(balls("52512"), Ok(3));
    }

    #[test]
    fn a_pattern_that_cannot_be_juggled_says_why() {
        // 4 + 3 = 7 over 2 beats: 3.5 balls
        assert_eq!(balls("43"), Err(Problem::Average { sum: 7, n: 2 }));
        // 4 3 2: beat 1's 4 and beat 2's 3 both come down on beat 5
        assert_eq!(balls("432"), Err(Problem::Collision { a: 1, b: 2, lands: 5 }));
        assert_eq!(
            parse("4x1"),
            Err(Problem::NotDigit {
                at: 2,
                ch: "x".into()
            })
        );
        assert_eq!(parse(""), Err(Problem::Empty));
    }

    #[test]
    fn the_cascade_keeps_three_balls_in_turn() {
        let p = parse("3").unwrap();
        let s = schedule(&p, 3, 9);
        let order: Vec<usize> = s.iter().map(|t| t.ball).collect();
        assert_eq!(order, vec![0, 1, 2, 0, 1, 2, 0, 1, 2]);
        // and 441 sends the same ball up twice on its 4s before the 1
        let p = parse("441").unwrap();
        let s = schedule(&p, 3, 12);
        assert_eq!(s.iter().filter(|t| t.value == 1).count(), 4);
        assert!(s.iter().map(|t| t.ball).max().unwrap() <= 2);
    }
}
