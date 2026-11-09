//! gettext's plural rules: the C expression of a catalog's
//! `Plural-Forms` header (`nplurals=2; plural=(n > 1);`), read and
//! evaluated for a count. The subset gettext itself takes: `n`, whole
//! numbers, `!`, `* / %`, `+ -`, `< <= > >=`, `== !=`, `&&`, `||`,
//! `?:`, and parentheses. Shared by the build script, which refuses a
//! catalog whose rule does not read, and the window, which evaluates it.

/// A rule, read.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    N,
    Num(i64),
    Not(Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>),
    Choose(Box<Expr>, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Mul,
    Div,
    Rem,
    Add,
    Sub,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

impl Expr {
    /// The rule's value for a count of `n`: the form's index.
    pub fn eval(&self, n: u64) -> i64 {
        match self {
            Expr::N => i64::try_from(n).unwrap_or(i64::MAX),
            Expr::Num(v) => *v,
            Expr::Not(e) => i64::from(e.eval(n) == 0),
            Expr::Choose(c, a, b) => {
                if c.eval(n) != 0 {
                    a.eval(n)
                } else {
                    b.eval(n)
                }
            }
            Expr::Binary(op, a, b) => {
                let (a, b) = (a.eval(n), b.eval(n));
                match op {
                    Op::Mul => a.wrapping_mul(b),
                    Op::Div => a.checked_div(b).unwrap_or(0),
                    Op::Rem => a.checked_rem(b).unwrap_or(0),
                    Op::Add => a.wrapping_add(b),
                    Op::Sub => a.wrapping_sub(b),
                    Op::Lt => i64::from(a < b),
                    Op::Le => i64::from(a <= b),
                    Op::Gt => i64::from(a > b),
                    Op::Ge => i64::from(a >= b),
                    Op::Eq => i64::from(a == b),
                    Op::Ne => i64::from(a != b),
                    Op::And => i64::from(a != 0 && b != 0),
                    Op::Or => i64::from(a != 0 || b != 0),
                }
            }
        }
    }
}

/// The header's value, `nplurals=N; plural=EXPR;`: the number of forms
/// and the rule.
pub fn forms(header: &str) -> Result<(usize, Expr), String> {
    let mut count = None;
    let mut rule = None;
    for part in header.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key.trim() {
            "nplurals" => {
                count = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|e| format!("nplurals {value:?}: {e}"))?,
                );
            }
            "plural" => rule = Some(parse(value)?),
            _ => {}
        }
    }
    match (count, rule) {
        (Some(c), Some(r)) if c > 0 => Ok((c, r)),
        _ => Err(format!("{header:?} is not nplurals=N; plural=EXPR;")),
    }
}

/// Read a rule's expression.
pub fn parse(src: &str) -> Result<Expr, String> {
    let tokens = tokens(src)?;
    let mut at = 0;
    let expr = choose(&tokens, &mut at)?;
    if at != tokens.len() {
        return Err(format!("{src:?}: {:?} left over", &tokens[at..]));
    }
    Ok(expr)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    N,
    Num(i64),
    Op(&'static str),
}

fn tokens(src: &str) -> Result<Vec<Token>, String> {
    const OPS: [&str; 19] = [
        "<=", ">=", "==", "!=", "&&", "||", "<", ">", "!", "*", "/", "%", "+", "-", "?", ":", "(",
        ")", "=",
    ];
    let mut out = Vec::new();
    let mut rest = src.trim();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('n') {
            out.push(Token::N);
            rest = r;
        } else if rest.starts_with(|c: char| c.is_ascii_digit()) {
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            let value = rest[..end]
                .parse::<i64>()
                .map_err(|e| format!("{src:?}: {e}"))?;
            out.push(Token::Num(value));
            rest = &rest[end..];
        } else if let Some(op) = OPS.iter().copied().find(|op| rest.starts_with(op)) {
            if op == "=" {
                return Err(format!("{src:?}: a lone '='"));
            }
            out.push(Token::Op(op));
            rest = &rest[op.len()..];
        } else {
            return Err(format!("{src:?}: unexpected {:?}", rest.chars().next()));
        }
        rest = rest.trim_start();
    }
    Ok(out)
}

/// The levels, loosest first, each with its operators.
const LEVELS: [&[(&str, Op)]; 6] = [
    &[("||", Op::Or)],
    &[("&&", Op::And)],
    &[("==", Op::Eq), ("!=", Op::Ne)],
    &[("<", Op::Lt), ("<=", Op::Le), (">", Op::Gt), (">=", Op::Ge)],
    &[("+", Op::Add), ("-", Op::Sub)],
    &[("*", Op::Mul), ("/", Op::Div), ("%", Op::Rem)],
];

fn choose(t: &[Token], at: &mut usize) -> Result<Expr, String> {
    let condition = level(t, at, 0)?;
    if t.get(*at) != Some(&Token::Op("?")) {
        return Ok(condition);
    }
    *at += 1;
    let yes = choose(t, at)?;
    if t.get(*at) != Some(&Token::Op(":")) {
        return Err("a '?' without its ':'".into());
    }
    *at += 1;
    let no = choose(t, at)?;
    Ok(Expr::Choose(
        Box::new(condition),
        Box::new(yes),
        Box::new(no),
    ))
}

fn level(t: &[Token], at: &mut usize, depth: usize) -> Result<Expr, String> {
    if depth == LEVELS.len() {
        return unary(t, at);
    }
    let mut left = level(t, at, depth + 1)?;
    while let Some(Token::Op(op)) = t.get(*at) {
        let Some((_, op)) = LEVELS[depth].iter().find(|(s, _)| s == op) else {
            break;
        };
        *at += 1;
        let right = level(t, at, depth + 1)?;
        left = Expr::Binary(*op, Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn unary(t: &[Token], at: &mut usize) -> Result<Expr, String> {
    let token = t.get(*at).cloned().ok_or("the rule ends early")?;
    *at += 1;
    match token {
        Token::N => Ok(Expr::N),
        Token::Num(v) => Ok(Expr::Num(v)),
        Token::Op("!") => Ok(Expr::Not(Box::new(unary(t, at)?))),
        Token::Op("(") => {
            let inner = choose(t, at)?;
            if t.get(*at) != Some(&Token::Op(")")) {
                return Err("a '(' without its ')'".into());
            }
            *at += 1;
            Ok(inner)
        }
        Token::Op(op) => Err(format!("unexpected {op:?}")),
    }
}
