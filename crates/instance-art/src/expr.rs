//! Expressions for `field`: arithmetic on the pixel position and frame number.
//!
//! Variables: `x`, `y` (pixel centre in canvas pixels), `u`, `v` (0..1 across the
//! box), `w`, `h` (box size), `f` (frame number), `pi`, `tau`. Operators `+ - * / %`
//! and `^` (power), brackets, and functions: `sin cos tan asin acos atan atan2 abs
//! sqrt exp ln pow min max clamp floor ceil round fract sign mix smoothstep step
//! noise(x, y[, seed]) hash(x, y[, seed])`.

use crate::canvas::{hash, vnoise};

#[derive(Clone, Debug)]
pub enum Expr {
    Num(f64),
    Var(usize),
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
}

#[derive(Clone, Copy, Debug)]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Abs,
    Sqrt,
    Exp,
    Ln,
    Pow,
    Min,
    Max,
    Clamp,
    Floor,
    Ceil,
    Round,
    Fract,
    Sign,
    Mix,
    Smoothstep,
    Step,
    Noise,
    Hash,
}

/// Variable slots: x, y, u, v, w, h, f.
pub const VARS: [&str; 7] = ["x", "y", "u", "v", "w", "h", "f"];

fn func(name: &str) -> Option<(Func, usize, usize)> {
    use Func::*;
    Some(match name {
        "sin" => (Sin, 1, 1),
        "cos" => (Cos, 1, 1),
        "tan" => (Tan, 1, 1),
        "asin" => (Asin, 1, 1),
        "acos" => (Acos, 1, 1),
        "atan" => (Atan, 1, 1),
        "atan2" => (Atan2, 2, 2),
        "abs" => (Abs, 1, 1),
        "sqrt" => (Sqrt, 1, 1),
        "exp" => (Exp, 1, 1),
        "ln" => (Ln, 1, 1),
        "pow" => (Pow, 2, 2),
        "min" => (Min, 2, 8),
        "max" => (Max, 2, 8),
        "clamp" => (Clamp, 3, 3),
        "floor" => (Floor, 1, 1),
        "ceil" => (Ceil, 1, 1),
        "round" => (Round, 1, 1),
        "fract" => (Fract, 1, 1),
        "sign" => (Sign, 1, 1),
        "mix" => (Mix, 3, 3),
        "smoothstep" => (Smoothstep, 3, 3),
        "step" => (Step, 2, 2),
        "noise" => (Noise, 2, 3),
        "hash" => (Hash, 2, 3),
        _ => return None,
    })
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

pub fn parse(text: &str) -> Result<Expr, String> {
    let mut p = Parser { s: text.as_bytes(), i: 0 };
    let e = p.sum()?;
    p.ws();
    if p.i < p.s.len() {
        return Err(format!("unexpected `{}` at position {} of `{text}`", p.s[p.i] as char, p.i + 1));
    }
    Ok(e)
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn sum(&mut self) -> Result<Expr, String> {
        let mut e = self.product()?;
        while let Some(op @ (b'+' | b'-')) = self.peek() {
            self.i += 1;
            e = Expr::Bin(op as char, Box::new(e), Box::new(self.product()?));
        }
        Ok(e)
    }
    fn product(&mut self) -> Result<Expr, String> {
        let mut e = self.unary()?;
        while let Some(op @ (b'*' | b'/' | b'%')) = self.peek() {
            self.i += 1;
            e = Expr::Bin(op as char, Box::new(e), Box::new(self.unary()?));
        }
        Ok(e)
    }
    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat(b'-') {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.eat(b'+') {
            return self.unary();
        }
        self.power()
    }
    fn power(&mut self) -> Result<Expr, String> {
        let base = self.atom()?;
        if self.eat(b'^') {
            // right-associative, binds tighter than unary minus on its left
            return Ok(Expr::Bin('^', Box::new(base), Box::new(self.unary()?)));
        }
        Ok(base)
    }
    fn atom(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                let e = self.sum()?;
                if !self.eat(b')') {
                    return Err("missing `)`".into());
                }
                Ok(e)
            }
            Some(c) if c.is_ascii_digit() || c == b'.' => {
                let start = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.') {
                    self.i += 1;
                }
                let t = std::str::from_utf8(&self.s[start..self.i]).unwrap();
                t.parse().map(Expr::Num).map_err(|_| format!("`{t}` is not a number"))
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                let start = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_') {
                    self.i += 1;
                }
                let name = std::str::from_utf8(&self.s[start..self.i]).unwrap();
                if self.eat(b'(') {
                    let (f, lo, hi) = func(name).ok_or(format!("unknown function `{name}`"))?;
                    let mut args = Vec::new();
                    if !self.eat(b')') {
                        loop {
                            args.push(self.sum()?);
                            if self.eat(b')') {
                                break;
                            }
                            if !self.eat(b',') {
                                return Err(format!("expected `,` or `)` in `{name}(...)`"));
                            }
                        }
                    }
                    if args.len() < lo || args.len() > hi {
                        return Err(format!("`{name}` takes {lo}{} argument(s), not {}", if hi > lo { format!("..{hi}") } else { String::new() }, args.len()));
                    }
                    return Ok(Expr::Call(f, args));
                }
                match name {
                    "pi" => Ok(Expr::Num(std::f64::consts::PI)),
                    "tau" => Ok(Expr::Num(std::f64::consts::TAU)),
                    _ => VARS.iter().position(|v| *v == name).map(Expr::Var).ok_or(format!("unknown name `{name}` (variables: x y u v w h f pi tau)")),
                }
            }
            Some(c) => Err(format!("unexpected `{}`", c as char)),
            None => Err("the expression ends too early".into()),
        }
    }
}

pub fn eval(e: &Expr, vars: &[f64; 7]) -> f64 {
    match e {
        Expr::Num(n) => *n,
        Expr::Var(i) => vars[*i],
        Expr::Neg(a) => -eval(a, vars),
        Expr::Bin(op, a, b) => {
            let (a, b) = (eval(a, vars), eval(b, vars));
            match op {
                '+' => a + b,
                '-' => a - b,
                '*' => a * b,
                '/' => a / b,
                '%' => a.rem_euclid(b),
                _ => a.powf(b),
            }
        }
        Expr::Call(f, args) => {
            let a: Vec<f64> = args.iter().map(|x| eval(x, vars)).collect();
            use Func::*;
            match f {
                Sin => a[0].sin(),
                Cos => a[0].cos(),
                Tan => a[0].tan(),
                Asin => a[0].asin(),
                Acos => a[0].acos(),
                Atan => a[0].atan(),
                Atan2 => a[0].atan2(a[1]),
                Abs => a[0].abs(),
                Sqrt => a[0].max(0.0).sqrt(),
                Exp => a[0].exp(),
                Ln => a[0].ln(),
                Pow => a[0].powf(a[1]),
                Min => a.iter().copied().fold(f64::INFINITY, f64::min),
                Max => a.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                Clamp => a[0].clamp(a[1].min(a[2]), a[2].max(a[1])),
                Floor => a[0].floor(),
                Ceil => a[0].ceil(),
                Round => a[0].round(),
                Fract => a[0] - a[0].floor(),
                Sign => a[0].signum(),
                Mix => a[0] + (a[1] - a[0]) * a[2],
                Smoothstep => {
                    let t = ((a[2] - a[0]) / (a[1] - a[0])).clamp(0.0, 1.0);
                    t * t * (3.0 - 2.0 * t)
                }
                Step => {
                    if a[1] >= a[0] {
                        1.0
                    } else {
                        0.0
                    }
                }
                Noise => vnoise(a[0], a[1], a.get(2).copied().unwrap_or(1.0) as u32, 0, 0),
                Hash => hash(a[0].floor() as i32, a[1].floor() as i32, a.get(2).copied().unwrap_or(1.0) as u32),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> f64 {
        eval(&parse(s).unwrap(), &[3.0, 4.0, 0.5, 0.25, 10.0, 20.0, 2.0])
    }

    #[test]
    fn arithmetic_and_functions() {
        assert_eq!(ev("1 + 2 * 3"), 7.0);
        assert_eq!(ev("(1 + 2) * 3"), 9.0);
        assert_eq!(ev("-2 ^ 2"), -4.0);
        assert_eq!(ev("2 ^ 3 ^ 2"), 512.0);
        assert_eq!(ev("x * y + f"), 14.0);
        assert_eq!(ev("max(u, v, 0.1)"), 0.5);
        assert_eq!(ev("mix(0, 10, u)"), 5.0);
        assert_eq!(ev("-7 % 3"), 2.0);
        assert!((ev("sin(pi / 2)") - 1.0).abs() < 1e-12);
        assert!(parse("sin(1, 2)").is_err());
        assert!(parse("foo + 1").is_err());
        assert!(parse("(1 + 2").is_err());
    }
}
