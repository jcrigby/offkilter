//! A small expression language for dimensions.
//!
//! Grammar: numbers, `+ - * / ^`, parentheses, unary minus, variable
//! references written `#name` (or a bare identifier), the constant `pi`,
//! and the functions `sin cos tan asin acos atan sqrt abs floor ceil round
//! min max`. Trigonometric functions take and return degrees, matching the
//! rest of the document. Lengths are model units (mm).

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
    Comma,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || chars[i] == 'e'
                    || chars[i] == 'E'
                    || ((chars[i] == '-' || chars[i] == '+') && matches!(chars[i - 1], 'e' | 'E')))
            {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            out.push(Tok::Num(
                text.parse().map_err(|_| format!("bad number '{text}'"))?,
            ));
        } else if c == '#' || c.is_alphabetic() || c == '_' {
            let start = if c == '#' { i + 1 } else { i };
            i = start;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            if i == start {
                return Err("expected a name after '#'".into());
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
        } else if "+-*/^".contains(c) {
            out.push(Tok::Op(c));
            i += 1;
        } else if c == '(' {
            out.push(Tok::LParen);
            i += 1;
        } else if c == ')' {
            out.push(Tok::RParen);
            i += 1;
        } else if c == ',' {
            out.push(Tok::Comma);
            i += 1;
        } else {
            return Err(format!("unexpected character '{c}'"));
        }
    }
    Ok(out)
}

struct Parser<'a> {
    toks: Vec<Tok>,
    pos: usize,
    vars: &'a BTreeMap<String, f64>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn expr(&mut self) -> Result<f64, String> {
        let mut v = self.term()?;
        while let Some(Tok::Op(c)) = self.peek() {
            let c = *c;
            if c != '+' && c != '-' {
                break;
            }
            self.next();
            let r = self.term()?;
            v = if c == '+' { v + r } else { v - r };
        }
        Ok(v)
    }

    fn term(&mut self) -> Result<f64, String> {
        let mut v = self.power()?;
        while let Some(Tok::Op(c)) = self.peek() {
            let c = *c;
            if c != '*' && c != '/' {
                break;
            }
            self.next();
            let r = self.power()?;
            if c == '/' && r == 0.0 {
                return Err("division by zero".into());
            }
            v = if c == '*' { v * r } else { v / r };
        }
        Ok(v)
    }

    fn power(&mut self) -> Result<f64, String> {
        let base = self.unary()?;
        if let Some(Tok::Op('^')) = self.peek() {
            self.next();
            let exp = self.power()?; // right associative
            return Ok(base.powf(exp));
        }
        Ok(base)
    }

    fn unary(&mut self) -> Result<f64, String> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.next();
                Ok(-self.unary()?)
            }
            Some(Tok::Op('+')) => {
                self.next();
                self.unary()
            }
            _ => self.atom(),
        }
    }

    fn atom(&mut self) -> Result<f64, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(n),
            Some(Tok::LParen) => {
                let v = self.expr()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(v),
                    _ => Err("expected ')'".into()),
                }
            }
            Some(Tok::Ident(name)) => {
                if let Some(Tok::LParen) = self.peek() {
                    self.next();
                    let mut args = Vec::new();
                    if let Some(Tok::RParen) = self.peek() {
                        self.next();
                    } else {
                        loop {
                            args.push(self.expr()?);
                            match self.next() {
                                Some(Tok::Comma) => continue,
                                Some(Tok::RParen) => break,
                                _ => return Err(format!("expected ',' or ')' in call to {name}")),
                            }
                        }
                    }
                    return call(&name, &args);
                }
                if name == "pi" {
                    return Ok(std::f64::consts::PI);
                }
                self.vars
                    .get(&name)
                    .copied()
                    .ok_or_else(|| format!("unknown variable #{name}"))
            }
            Some(t) => Err(format!("unexpected token {t:?}")),
            None => Err("unexpected end of expression".into()),
        }
    }
}

fn call(name: &str, args: &[f64]) -> Result<f64, String> {
    let one = |f: fn(f64) -> f64| -> Result<f64, String> {
        match args {
            [x] => Ok(f(*x)),
            _ => Err(format!("{name} takes one argument")),
        }
    };
    match name {
        "sin" => one(|x| x.to_radians().sin()),
        "cos" => one(|x| x.to_radians().cos()),
        "tan" => one(|x| x.to_radians().tan()),
        "asin" => one(|x| x.asin().to_degrees()),
        "acos" => one(|x| x.acos().to_degrees()),
        "atan" => one(|x| x.atan().to_degrees()),
        "sqrt" => one(f64::sqrt),
        "abs" => one(f64::abs),
        "floor" => one(f64::floor),
        "ceil" => one(f64::ceil),
        "round" => one(f64::round),
        "min" if !args.is_empty() => Ok(args.iter().copied().fold(f64::INFINITY, f64::min)),
        "max" if !args.is_empty() => Ok(args.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        _ => Err(format!("unknown function {name}")),
    }
}

/// Evaluates an expression against a variable table.
pub fn evaluate(src: &str, vars: &BTreeMap<String, f64>) -> Result<f64, String> {
    let toks = tokenize(src)?;
    if toks.is_empty() {
        return Err("empty expression".into());
    }
    let mut p = Parser { toks, pos: 0, vars };
    let v = p.expr()?;
    if p.pos != p.toks.len() {
        return Err("unexpected trailing input".into());
    }
    if !v.is_finite() {
        return Err("expression is not a finite number".into());
    }
    Ok(v)
}

/// Whether a string is just a number (no expression needed).
pub fn is_plain_number(src: &str) -> bool {
    src.trim().parse::<f64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> BTreeMap<String, f64> {
        let mut m = BTreeMap::new();
        m.insert("width".into(), 60.0);
        m.insert("n".into(), 3.0);
        m
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(evaluate("1 + 2 * 3", &env()).unwrap(), 7.0);
        assert_eq!(evaluate("(1 + 2) * 3", &env()).unwrap(), 9.0);
        assert_eq!(evaluate("2 ^ 3 ^ 2", &env()).unwrap(), 512.0);
        assert_eq!(evaluate("-2 ^ 2", &env()).unwrap(), 4.0);
        assert_eq!(evaluate("10 / 4", &env()).unwrap(), 2.5);
        assert_eq!(evaluate("1.5e1", &env()).unwrap(), 15.0);
    }

    #[test]
    fn variables_and_functions() {
        assert_eq!(evaluate("#width / 2", &env()).unwrap(), 30.0);
        assert_eq!(evaluate("width - n", &env()).unwrap(), 57.0);
        assert!((evaluate("sin(30)", &env()).unwrap() - 0.5).abs() < 1e-12);
        assert_eq!(evaluate("max(1, #n, 2)", &env()).unwrap(), 3.0);
        assert_eq!(evaluate("sqrt(16) + abs(-1)", &env()).unwrap(), 5.0);
        assert!((evaluate("2 * pi", &env()).unwrap() - std::f64::consts::TAU).abs() < 1e-12);
    }

    #[test]
    fn errors() {
        assert!(evaluate("#missing + 1", &env()).is_err());
        assert!(evaluate("1 +", &env()).is_err());
        assert!(evaluate("(1 + 2", &env()).is_err());
        assert!(evaluate("1 / 0", &env()).is_err());
        assert!(evaluate("foo(1)", &env()).is_err());
        assert!(evaluate("", &env()).is_err());
    }
}
