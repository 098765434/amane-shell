//! Penpot's token math: numbers with an optional `px`, `+ - * / % ^`,
//! parentheses and a fixed set of functions. References are substituted by
//! the resolver before evaluation, so this only sees literals.

/// A number and whether it carries `px`. `px` propagates through every
/// operation, as in Penpot (`{ref.space.8} * 2` is `16px`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Num {
    pub value: f64,
    pub px: bool,
}

/// Penpot rejects values outside JavaScript's safe-integer range.
const SAFE: f64 = 9_007_199_254_740_991.0;

pub fn eval(src: &str) -> Result<Num, String> {
    let tokens = lex(src)?;
    let mut parser = Parser { tokens, pos: 0 };
    let num = parser.expr()?;

    if parser.pos != parser.tokens.len() {
        return Err(format!(
            "unexpected {:?} in `{src}`",
            parser.tokens[parser.pos]
        ));
    }
    if !num.value.is_finite() {
        return Err(format!("`{src}` is not a finite number"));
    }
    if num.value.abs() > SAFE {
        return Err(format!("`{src}` is outside the safe-integer range"));
    }

    Ok(num)
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(Num),
    Ident(String),
    Op(char),
    Open,
    Close,
    Comma,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let value = text
                .parse::<f64>()
                .map_err(|_| format!("bad number `{text}`"))?;

            let unit_start = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            let unit: String = chars[unit_start..i].iter().collect();
            let px = match unit.as_str() {
                "" => false,
                "px" => true,
                other => return Err(format!("unit `{other}` is not supported (px or unitless)")),
            };

            out.push(Tok::Num(Num { value, px }));
        } else if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphanumeric() {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
        } else {
            out.push(match c {
                '+' | '-' | '*' | '/' | '%' | '^' => Tok::Op(c),
                '(' => Tok::Open,
                ')' => Tok::Close,
                ',' => Tok::Comma,
                other => return Err(format!("unexpected `{other}` in `{src}`")),
            });
            i += 1;
        }
    }

    Ok(out)
}

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let tok = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        tok
    }

    fn expect(&mut self, want: Tok) -> Result<(), String> {
        match self.next() {
            Some(tok) if tok == want => Ok(()),
            got => Err(format!("expected {want:?}, found {got:?}")),
        }
    }

    // expr := term (('+' | '-') term)*
    fn expr(&mut self) -> Result<Num, String> {
        let mut lhs = self.term()?;

        while let Some(Tok::Op(op @ ('+' | '-'))) = self.peek().cloned() {
            self.pos += 1;
            let rhs = self.term()?;
            let value = if op == '+' {
                lhs.value + rhs.value
            } else {
                lhs.value - rhs.value
            };
            lhs = Num {
                value,
                px: lhs.px || rhs.px,
            };
        }

        Ok(lhs)
    }

    // term := unary (('*' | '/' | '%') unary)*
    fn term(&mut self) -> Result<Num, String> {
        let mut lhs = self.unary()?;

        while let Some(Tok::Op(op @ ('*' | '/' | '%'))) = self.peek().cloned() {
            self.pos += 1;
            let rhs = self.unary()?;
            if op != '*' && rhs.value == 0.0 {
                return Err(String::from("division by zero"));
            }
            let value = match op {
                '*' => lhs.value * rhs.value,
                '/' => lhs.value / rhs.value,
                _ => lhs.value % rhs.value,
            };
            lhs = Num {
                value,
                px: lhs.px || rhs.px,
            };
        }

        Ok(lhs)
    }

    // unary := ('-' | '+') unary | power   (so -2^2 is -4)
    fn unary(&mut self) -> Result<Num, String> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.pos += 1;
                let n = self.unary()?;
                Ok(Num {
                    value: -n.value,
                    ..n
                })
            }
            Some(Tok::Op('+')) => {
                self.pos += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }

    // power := primary ('^' unary)?   (right-associative)
    fn power(&mut self) -> Result<Num, String> {
        let base = self.primary()?;

        if let Some(Tok::Op('^')) = self.peek() {
            self.pos += 1;
            let exp = self.unary()?;
            return Ok(Num {
                value: base.value.powf(exp.value),
                px: base.px || exp.px,
            });
        }

        Ok(base)
    }

    fn primary(&mut self) -> Result<Num, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(n),
            Some(Tok::Open) => {
                let n = self.expr()?;
                self.expect(Tok::Close)?;
                Ok(n)
            }
            Some(Tok::Ident(name)) => {
                self.expect(Tok::Open)?;
                let mut args = Vec::new();
                if self.peek() != Some(&Tok::Close) {
                    args.push(self.expr()?);
                    while self.peek() == Some(&Tok::Comma) {
                        self.pos += 1;
                        args.push(self.expr()?);
                    }
                }
                self.expect(Tok::Close)?;
                call(&name, &args)
            }
            got => Err(format!("expected a number, found {got:?}")),
        }
    }
}

fn call(name: &str, args: &[Num]) -> Result<Num, String> {
    let px = args.iter().any(|a| a.px);
    let v: Vec<f64> = args.iter().map(|a| a.value).collect();

    let arity = |n: usize| {
        if v.len() == n {
            Ok(())
        } else {
            Err(format!("{name}() takes {n} argument(s), got {}", v.len()))
        }
    };

    let value = match name {
        "abs" => arity(1).map(|_| v[0].abs())?,
        "ceil" => arity(1).map(|_| v[0].ceil())?,
        "floor" => arity(1).map(|_| v[0].floor())?,
        "round" => arity(1).map(|_| v[0].round())?,
        "sqrt" => arity(1).map(|_| v[0].sqrt())?,
        "exp" => arity(1).map(|_| v[0].exp())?,
        "sin" => arity(1).map(|_| v[0].sin())?,
        "cos" => arity(1).map(|_| v[0].cos())?,
        "tan" => arity(1).map(|_| v[0].tan())?,
        "asin" => arity(1).map(|_| v[0].asin())?,
        "acos" => arity(1).map(|_| v[0].acos())?,
        "atan" => arity(1).map(|_| v[0].atan())?,
        "atan2" => arity(2).map(|_| v[0].atan2(v[1]))?,
        "pow" => arity(2).map(|_| v[0].powf(v[1]))?,
        "log" => match v.len() {
            1 => v[0].ln(),
            2 => v[0].log(v[1]),
            n => return Err(format!("log() takes 1 or 2 arguments, got {n}")),
        },
        "roundTo" => arity(2).map(|_| {
            let scale = 10f64.powi(v[1] as i32);
            (v[0] * scale).round() / scale
        })?,
        "max" | "min" if !v.is_empty() => {
            let fold = if name == "max" { f64::max } else { f64::min };
            v.iter().copied().reduce(fold).unwrap_or(0.0)
        }
        other => return Err(format!("unknown function {other}()")),
    };

    Ok(Num { value, px })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(src: &str) -> f64 {
        eval(src).unwrap_or_else(|e| panic!("{src}: {e}")).value
    }

    #[test]
    fn precedence() {
        assert_eq!(n("2 + 3 * 4"), 14.0);
        assert_eq!(n("(2 + 3) * 4"), 20.0);
        assert_eq!(n("10 - 4 - 3"), 3.0);
        assert_eq!(n("2 ^ 3 ^ 2"), 512.0);
        assert_eq!(n("-2 ^ 2"), -4.0);
        assert_eq!(n("2 * -3"), -6.0);
        assert_eq!(n("7 % 4"), 3.0);
        assert_eq!(n("8 / 2 * 3"), 12.0);
    }

    #[test]
    fn every_function() {
        assert_eq!(n("abs(-3)"), 3.0);
        assert_eq!(n("ceil(1.2)"), 2.0);
        assert_eq!(n("floor(1.8)"), 1.0);
        assert_eq!(n("round(10.5)"), 11.0);
        assert_eq!(n("round(5.25)"), 5.0);
        assert_eq!(n("max(1, 7, 3)"), 7.0);
        assert_eq!(n("min(4, 2)"), 2.0);
        assert_eq!(n("sqrt(16)"), 4.0);
        assert_eq!(n("pow(2, 10)"), 1024.0);
        assert_eq!(n("log(exp(2))"), 2.0);
        assert_eq!(n("log(8, 2)"), 3.0);
        assert_eq!(n("sin(0)"), 0.0);
        assert_eq!(n("cos(0)"), 1.0);
        assert_eq!(n("tan(0)"), 0.0);
        assert_eq!(n("asin(0)"), 0.0);
        assert_eq!(n("acos(1)"), 0.0);
        assert_eq!(n("atan(0)"), 0.0);
        assert_eq!(n("atan2(0, 1)"), 0.0);
        assert_eq!(n("roundTo(2.71828, 2)"), 2.72);
    }

    #[test]
    fn px_propagates() {
        assert_eq!(
            eval("8px * 2"),
            Ok(Num {
                value: 16.0,
                px: true
            })
        );
        assert_eq!(
            eval("round(24 * 0.875)"),
            Ok(Num {
                value: 21.0,
                px: false
            })
        );
        assert_eq!(
            eval("round(40px * 0.875) - 10"),
            Ok(Num {
                value: 25.0,
                px: true
            })
        );
    }

    #[test]
    fn errors() {
        assert!(eval("1 / 0").is_err());
        assert!(eval("1 % 0").is_err());
        assert!(eval("1rem").is_err());
        assert!(eval("2 +").is_err());
        assert!(eval("nope(1)").is_err());
        assert!(eval("(1").is_err());
        assert!(eval("1 2").is_err());
        assert!(eval("9007199254740992 * 2").is_err());
    }
}
