//! Formulas people type, read and evaluated in Q16.16 (no floats).
//!
//! What it reads: numbers (`2`, `0.5`), `x`, `pi`, `e`; `+ - * / ^` with
//! the usual order (`-x^2` is `-(x^2)`, `2^3^2` is `2^(3^2)`); brackets
//! and `|x|`; the functions `sin cos tan abs sqrt floor round`; and the
//! way people write by hand: `2x`, `3(x+1)`, `(x-1)(x+1)`, `x²`, `x³`, `×`,
//! `÷`, `−`. Anything else is refused with the place the reading stopped.

use crate::fx::{self, ONE, TURN};

/// Why a formula could not be read: a copy key under `lab.expr`, and the
/// character it stopped at (counting from 1) or the word it did not know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadError {
    pub key: &'static str,
    pub at: usize,
    pub word: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Num(i64),
    X,
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Call(Func, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Abs,
    Sqrt,
    Floor,
    Round,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i64),
    Word(String),
    Op(char),
    Open,
    Close,
    Bar,
}

/// The largest value a formula may reach before it counts as off the chart.
const LIMIT: i64 = 1 << 46;
/// π and e in Q16.
const PI: i64 = 205_887;
const E: i64 = 178_145;

fn lex(src: &str) -> Result<Vec<(Tok, usize)>, ReadError> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let at = i + 1;
        match c {
            ' ' | '\t' => {}
            // a decimal point or, as French writes it, a decimal comma
            '0'..='9' | '.' | ',' => {
                let mut whole: i64 = 0;
                let mut frac: i64 = 0;
                let mut scale: i64 = 1;
                let mut dot = false;
                let mut digits = 0;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == ',') {
                    let d = chars[i];
                    if d == '.' || d == ',' {
                        if dot {
                            return Err(ReadError {
                                key: "unexpected",
                                at: i + 1,
                                word: ".".into(),
                            });
                        }
                        dot = true;
                    } else {
                        let v = d as i64 - '0' as i64;
                        digits += 1;
                        if digits > 9 {
                            return Err(ReadError {
                                key: "too_big",
                                at,
                                word: String::new(),
                            });
                        }
                        if dot {
                            if scale < 1_000_000 {
                                frac = frac * 10 + v;
                                scale *= 10;
                            }
                        } else {
                            whole = whole * 10 + v;
                        }
                    }
                    i += 1;
                }
                if digits == 0 {
                    return Err(ReadError {
                        key: "unexpected",
                        at,
                        word: ".".into(),
                    });
                }
                out.push((Tok::Num(whole * ONE as i64 + frac * ONE as i64 / scale), at));
                continue;
            }
            'a'..='z' | 'A'..='Z' | 'π' => {
                let mut w = String::new();
                while i < chars.len() && (chars[i].is_ascii_alphabetic() || chars[i] == 'π') {
                    w.push(chars[i].to_ascii_lowercase());
                    i += 1;
                }
                // `xx`, `pix` and the like are products of names people know
                let mut rest = w.as_str();
                while !rest.is_empty() {
                    let known = [
                        "sqrt", "floor", "round", "sin", "cos", "tan", "abs", "pi", "π", "x", "e",
                    ];
                    match known.iter().find(|k| rest.starts_with(*k)) {
                        Some(k) => {
                            out.push((Tok::Word(k.replace('π', "pi")), at));
                            rest = &rest[k.len()..];
                        }
                        None => {
                            return Err(ReadError {
                                key: "unknown",
                                at,
                                word: w.clone(),
                            })
                        }
                    }
                }
                continue;
            }
            '+' | '-' | '−' | '*' | '×' | '·' | '/' | '÷' | '^' => {
                let op = match c {
                    '−' => '-',
                    '×' | '·' => '*',
                    '÷' => '/',
                    o => o,
                };
                out.push((Tok::Op(op), at));
            }
            '²' | '³' => {
                out.push((Tok::Op('^'), at));
                out.push((Tok::Num(if c == '²' { 2 } else { 3 } * ONE as i64), at));
            }
            '(' | '[' => out.push((Tok::Open, at)),
            ')' | ']' => out.push((Tok::Close, at)),
            '|' => out.push((Tok::Bar, at)),
            _ => {
                return Err(ReadError {
                    key: "unexpected",
                    at,
                    word: c.to_string(),
                })
            }
        }
        i += 1;
    }
    Ok(out)
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    pos: usize,
    end: usize,
    /// Inside `|…|`, a bar closes rather than opens.
    bars: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|(t, _)| t)
    }
    fn at(&self) -> usize {
        self.toks.get(self.pos).map(|(_, a)| *a).unwrap_or(self.end)
    }
    fn fail(&self, key: &'static str) -> ReadError {
        let word = match self.peek() {
            Some(Tok::Op(c)) => c.to_string(),
            Some(Tok::Word(w)) => w.clone(),
            Some(Tok::Num(_)) => "number".into(),
            Some(Tok::Open) => "(".into(),
            Some(Tok::Close) => ")".into(),
            Some(Tok::Bar) => "|".into(),
            None => String::new(),
        };
        ReadError {
            key,
            at: self.at(),
            word,
        }
    }

    fn expr(&mut self) -> Result<Expr, ReadError> {
        let mut left = self.term()?;
        while let Some(Tok::Op(c)) = self.peek() {
            let c = *c;
            if c != '+' && c != '-' {
                break;
            }
            self.pos += 1;
            let right = self.term()?;
            left = if c == '+' {
                Expr::Add(Box::new(left), Box::new(right))
            } else {
                Expr::Sub(Box::new(left), Box::new(right))
            };
        }
        Ok(left)
    }

    /// Whether the next token can start a factor written straight after
    /// another: `2x`, `3(x+1)`, `x sin(x)`.
    fn starts_factor(&self) -> bool {
        match self.peek() {
            Some(Tok::Num(_)) | Some(Tok::Word(_)) | Some(Tok::Open) => true,
            Some(Tok::Bar) => self.bars == 0,
            _ => false,
        }
    }

    fn term(&mut self) -> Result<Expr, ReadError> {
        let mut left = self.unary()?;
        loop {
            match self.peek() {
                Some(Tok::Op('*')) => {
                    self.pos += 1;
                    let r = self.unary()?;
                    left = Expr::Mul(Box::new(left), Box::new(r));
                }
                Some(Tok::Op('/')) => {
                    self.pos += 1;
                    let r = self.unary()?;
                    left = Expr::Div(Box::new(left), Box::new(r));
                }
                _ if self.starts_factor() => {
                    let r = self.power()?;
                    left = Expr::Mul(Box::new(left), Box::new(r));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, ReadError> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.pos += 1;
                Ok(Expr::Neg(Box::new(self.unary()?)))
            }
            Some(Tok::Op('+')) => {
                self.pos += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Result<Expr, ReadError> {
        let base = self.primary()?;
        if let Some(Tok::Op('^')) = self.peek() {
            self.pos += 1;
            let exp = self.unary()?;
            return Ok(Expr::Pow(Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<Expr, ReadError> {
        let tok = self.peek().cloned();
        match tok {
            Some(Tok::Num(v)) => {
                self.pos += 1;
                Ok(Expr::Num(v))
            }
            Some(Tok::Word(w)) => {
                self.pos += 1;
                let f = match w.as_str() {
                    "x" => return Ok(Expr::X),
                    "pi" => return Ok(Expr::Num(PI)),
                    "e" => return Ok(Expr::Num(E)),
                    "sin" => Func::Sin,
                    "cos" => Func::Cos,
                    "tan" => Func::Tan,
                    "abs" => Func::Abs,
                    "sqrt" => Func::Sqrt,
                    "floor" => Func::Floor,
                    _ => Func::Round,
                };
                // a function's argument: brackets, or a single factor (`sin x`)
                let arg = if self.peek() == Some(&Tok::Open) {
                    self.primary()?
                } else {
                    self.power()?
                };
                Ok(Expr::Call(f, Box::new(arg)))
            }
            Some(Tok::Open) => {
                self.pos += 1;
                let saved = self.bars;
                self.bars = 0;
                let e = self.expr()?;
                self.bars = saved;
                if self.peek() != Some(&Tok::Close) {
                    return Err(self.fail("bracket"));
                }
                self.pos += 1;
                Ok(e)
            }
            Some(Tok::Bar) => {
                self.pos += 1;
                self.bars += 1;
                let e = self.expr()?;
                self.bars -= 1;
                if self.peek() != Some(&Tok::Bar) {
                    return Err(self.fail("bar"));
                }
                self.pos += 1;
                Ok(Expr::Call(Func::Abs, Box::new(e)))
            }
            None => Err(self.fail("end")),
            _ => Err(self.fail("unexpected")),
        }
    }
}

/// Reads a formula in x.
pub fn parse(src: &str) -> Result<Expr, ReadError> {
    let toks = lex(src)?;
    if toks.is_empty() {
        return Err(ReadError {
            key: "empty",
            at: 1,
            word: String::new(),
        });
    }
    let end = src.chars().count() + 1;
    let mut p = Parser {
        toks,
        pos: 0,
        end,
        bars: 0,
    };
    let e = p.expr()?;
    if p.pos < p.toks.len() {
        return Err(p.fail(if p.peek() == Some(&Tok::Close) {
            "bracket"
        } else {
            "unexpected"
        }));
    }
    Ok(e)
}

/// Why a formula has no value at some x: a copy key under `lab.undefined`.
pub type Undefined = &'static str;

fn check(v: i64) -> Result<i64, Undefined> {
    if v.abs() > LIMIT {
        Err("too_big")
    } else {
        Ok(v)
    }
}

fn mul(a: i64, b: i64) -> Result<i64, Undefined> {
    check(((a as i128 * b as i128) >> fx::FRAC_BITS) as i64)
}

/// Radians (Q16) to the core's angle units.
fn turns(r: i64) -> i32 {
    // 2π = 411 775 in Q16; reduce first so the product stays small
    let r = r.rem_euclid(411_775);
    (r * TURN as i64 / 411_775) as i32
}

impl Expr {
    /// The formula's value at `x`, both Q16.
    pub fn eval(&self, x: i64) -> Result<i64, Undefined> {
        let one = ONE as i64;
        match self {
            Expr::Num(v) => Ok(*v),
            Expr::X => Ok(x),
            Expr::Neg(a) => Ok(-a.eval(x)?),
            Expr::Add(a, b) => check(a.eval(x)? + b.eval(x)?),
            Expr::Sub(a, b) => check(a.eval(x)? - b.eval(x)?),
            Expr::Mul(a, b) => mul(a.eval(x)?, b.eval(x)?),
            Expr::Div(a, b) => {
                let d = b.eval(x)?;
                if d == 0 {
                    return Err("divide_by_zero");
                }
                check(
                    ((a.eval(x)? as i128) << fx::FRAC_BITS)
                        .checked_div(d as i128)
                        .unwrap_or(0) as i64,
                )
            }
            Expr::Pow(a, b) => {
                let base = a.eval(x)?;
                let p = b.eval(x)?;
                if p % one != 0 {
                    // a fractional power: only a half, as a square root
                    if p == one / 2 {
                        return Expr::Call(Func::Sqrt, Box::new(Expr::Num(base))).eval(x);
                    }
                    return Err("fraction_power");
                }
                let n = p / one;
                if n.abs() > 64 {
                    return Err("too_big");
                }
                let mut r = one;
                for _ in 0..n.abs() {
                    r = mul(r, base)?;
                }
                if n < 0 {
                    if r == 0 {
                        return Err("divide_by_zero");
                    }
                    r = check(
                        ((one as i128) << fx::FRAC_BITS)
                            .checked_div(r as i128)
                            .unwrap_or(0) as i64,
                    )?;
                }
                Ok(r)
            }
            Expr::Call(f, a) => {
                let v = a.eval(x)?;
                match f {
                    Func::Abs => Ok(v.abs()),
                    Func::Sin => Ok(fx::sin(turns(v)) as i64),
                    Func::Cos => Ok(fx::cos(turns(v)) as i64),
                    Func::Tan => {
                        let c = fx::cos(turns(v)) as i64;
                        if c.abs() < one / 1000 {
                            return Err("divide_by_zero");
                        }
                        check(((fx::sin(turns(v)) as i64) << fx::FRAC_BITS) / c)
                    }
                    Func::Sqrt => {
                        if v < 0 {
                            return Err("sqrt_negative");
                        }
                        Ok(fx::isqrt((v as u64) << fx::FRAC_BITS) as i64)
                    }
                    Func::Floor => Ok(v.div_euclid(one) * one),
                    Func::Round => Ok((v + one / 2).div_euclid(one) * one),
                }
            }
        }
    }
}

/// A number as people write it: at most two decimals, no trailing zeros.
pub fn show(v: i64) -> String {
    let one = ONE as i64;
    let hundredths = (v.abs() * 100 + one / 2) / one;
    let sign = if v < 0 && hundredths > 0 { "-" } else { "" };
    let (w, f) = (hundredths / 100, hundredths % 100);
    if f == 0 {
        format!("{sign}{w}")
    } else if f % 10 == 0 {
        format!("{sign}{w}.{}", f / 10)
    } else {
        format!("{sign}{w}.{f:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: i64) -> i64 {
        n * ONE as i64
    }

    fn at(src: &str, x: i64) -> i64 {
        parse(src)
            .unwrap_or_else(|e| panic!("{src}: {e:?}"))
            .eval(x)
            .unwrap()
    }

    #[test]
    fn it_reads_what_people_type() {
        // worked by hand at x = 3
        assert_eq!(at("x^2", q(3)), q(9));
        assert_eq!(at("2x+1", q(3)), q(7));
        assert_eq!(at("-x^2+4", q(3)), q(-5));
        assert_eq!(at("(x-1)(x+1)", q(3)), q(8));
        assert_eq!(at("x²", q(3)), q(9));
        assert_eq!(at("3(x−1)", q(3)), q(6));
        assert_eq!(at("|x-5|", q(3)), q(2));
        assert_eq!(at("abs(x - 5) + |x|", q(3)), q(5));
        assert_eq!(at("x/2", q(3)), q(3) / 2);
        assert_eq!(at("2^3^2", 0), q(512));
        assert_eq!(at("0.5x", q(3)), q(3) / 2);
        assert_eq!(at("0,5x", q(3)), q(3) / 2);
        assert_eq!(at("xx", q(3)), q(9));
        assert_eq!(at("floor(x/2)", q(3)), q(1));
        assert_eq!(at("sqrt(x+1)", q(3)), q(2));
        assert_eq!(at("x^-1", q(4)), q(1) / 4);
    }

    #[test]
    fn sines_are_read_in_radians() {
        // sin(π/2) = 1, cos(π) = -1, within a few Q16 steps
        assert!((at("sin(pi/2)", 0) - q(1)).abs() < 40);
        assert!((at("cos(pi)", 0) + q(1)).abs() < 40);
        assert!(at("sin(x)", 0).abs() < 4);
    }

    #[test]
    fn it_says_where_it_stopped() {
        assert_eq!(parse("2x+").unwrap_err().key, "end");
        let e = parse("2*(x+1").unwrap_err();
        assert_eq!(e.key, "bracket");
        let e = parse("x $ 2").unwrap_err();
        assert_eq!((e.key, e.at, e.word.as_str()), ("unexpected", 3, "$"));
        let e = parse("y^2").unwrap_err();
        assert_eq!((e.key, e.word.as_str()), ("unknown", "y"));
        assert_eq!(parse("  ").unwrap_err().key, "empty");
    }

    #[test]
    fn values_that_do_not_exist_say_why() {
        assert_eq!(parse("1/x").unwrap().eval(0), Err("divide_by_zero"));
        assert_eq!(parse("sqrt(x)").unwrap().eval(-q(1)), Err("sqrt_negative"));
        assert_eq!(parse("x^0.3").unwrap().eval(q(2)), Err("fraction_power"));
        assert_eq!(parse("x^40").unwrap().eval(q(100)), Err("too_big"));
    }

    #[test]
    fn numbers_are_shown_as_written() {
        assert_eq!(show(q(4)), "4");
        assert_eq!(show(q(9) / 4), "2.25");
        assert_eq!(show(-q(3) / 2), "-1.5");
        assert_eq!(show(0), "0");
    }
}
