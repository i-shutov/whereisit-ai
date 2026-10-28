use crate::{Error, Result};
use rusqlite::types::Value;

#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    L,
    R,
    And,
    Or,
    Not,
}
fn lex(s: &str) -> Result<Vec<Token>> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '(' {
            chars.next();
            out.push(Token::L);
            continue;
        }
        if c == ')' {
            chars.next();
            out.push(Token::R);
            continue;
        }
        let mut w = String::new();
        let mut quoted = false;
        let mut had_quote = false;
        while let Some(&c) = chars.peek() {
            if !quoted && (c.is_whitespace() || c == '(' || c == ')') {
                break;
            }
            chars.next();
            if c == '"' {
                quoted = !quoted;
                had_quote = true;
            } else if c == '\\' && quoted {
                let next = chars
                    .next()
                    .ok_or_else(|| Error::new("invalid_query", "Incomplete escape"))?;
                w.push(next);
            } else {
                w.push(c);
            }
        }
        if quoted {
            return Err(Error::new("invalid_query", "Unclosed quote"));
        }
        if w.is_empty() {
            return Err(Error::new("invalid_query", "Empty term"));
        }
        out.push(if had_quote {
            Token::Word(w)
        } else {
            match w.to_ascii_uppercase().as_str() {
                "AND" => Token::And,
                "OR" => Token::Or,
                "NOT" => Token::Not,
                _ => Token::Word(w),
            }
        });
    }
    Ok(out)
}
struct Parser {
    t: Vec<Token>,
    i: usize,
    args: Vec<Value>,
}
impl Parser {
    fn expr(&mut self) -> Result<String> {
        let mut x = self.and()?;
        while self.t.get(self.i) == Some(&Token::Or) {
            self.i += 1;
            let y = self.and()?;
            x = format!("({x} OR {y})")
        }
        Ok(x)
    }
    fn and(&mut self) -> Result<String> {
        let mut x = self.unary()?;
        loop {
            let explicit = self.t.get(self.i) == Some(&Token::And);
            let implicit = matches!(
                self.t.get(self.i),
                Some(Token::Word(_) | Token::L | Token::Not)
            );
            if !explicit && !implicit {
                break;
            }
            if explicit {
                self.i += 1
            }
            let y = self.unary()?;
            x = format!("({x} AND {y})");
        }
        Ok(x)
    }
    fn unary(&mut self) -> Result<String> {
        if self.t.get(self.i) == Some(&Token::Not) {
            self.i += 1;
            return Ok(format!("(NOT {})", self.unary()?));
        }
        if self.t.get(self.i) == Some(&Token::L) {
            self.i += 1;
            let x = self.expr()?;
            if self.t.get(self.i) != Some(&Token::R) {
                return Err(Error::new("invalid_query", "Missing closing parenthesis"));
            }
            self.i += 1;
            return Ok(format!("({x})"));
        }
        let w = match self.t.get(self.i) {
            Some(Token::Word(w)) => w.clone(),
            _ => return Err(Error::new("invalid_query", "Expected a search term")),
        };
        self.i += 1;
        self.term(&w)
    }
    fn term(&mut self, w: &str) -> Result<String> {
        let (field, value) = match w.split_once(':') {
            Some((a, b)) => (a.to_ascii_lowercase(), b),
            None => ("text".into(), w),
        };
        if value.is_empty() {
            return Err(Error::new("invalid_query", "Empty field value"));
        }
        let v = value.to_lowercase();
        match field.as_str() {
            "size" => {
                let (op, num) = if let Some(x) = v.strip_prefix(">=") {
                    (">=", x)
                } else if let Some(x) = v.strip_prefix("<=") {
                    ("<=", x)
                } else if let Some(x) = v.strip_prefix('>') {
                    (">", x)
                } else if let Some(x) = v.strip_prefix('<') {
                    ("<", x)
                } else if let Some(x) = v.strip_prefix('=') {
                    ("=", x)
                } else {
                    ("=", v.as_str())
                };
                let (op, size) = parse_size(op, num)?;
                self.args.push(Value::Integer(size));
                Ok(format!("size {op} ?"))
            }
            "ext" => {
                self.args
                    .push(Value::Text(v.trim_start_matches('.').into()));
                Ok("ext = ?".into())
            }
            "category" => {
                self.args.push(Value::Text(v));
                Ok("id IN (WITH RECURSIVE marked(id,volume_id) AS (SELECT e.id,e.volume_id FROM categories c JOIN entry_categories ec ON ec.category_id=c.id JOIN entries e ON e.id=ec.entry_id WHERE c.name_norm=? UNION SELECT child.id,child.volume_id FROM marked m JOIN entries child ON child.volume_id=m.volume_id AND child.parent_id=m.id) SELECT id FROM marked)".into())
            }
            "text" | "name" | "path" | "description" | "tag" => {
                let col = match field.as_str() {
                    "name" => "name_norm",
                    "path" => "unicode_lower(relative_path)",
                    "description" => "unicode_lower(description)",
                    "tag" => "unicode_lower(tags)",
                    _ => "search_text",
                };
                let wildcard = v.contains(['*', '?', '[']);
                let literal = if wildcard {
                    longest_glob_literal(&v)
                } else {
                    v.clone()
                };
                let condition = if wildcard {
                    format!("{col} GLOB ?")
                } else {
                    format!("instr({col},?)>0")
                };
                if literal.chars().count() >= 3 {
                    self.args.push(Value::Text(trigram_query(&literal)));
                    self.args.push(Value::Text(v));
                    Ok(format!("(id IN (SELECT rowid FROM entries_trigram WHERE entries_trigram MATCH ?) AND {condition})"))
                } else {
                    self.args.push(Value::Text(v));
                    Ok(condition)
                }
            }
            _ => Err(Error::new(
                "invalid_query",
                format!("Unknown field: {field}"),
            )),
        }
    }
}
pub fn compile(s: &str) -> Result<(String, Vec<Value>)> {
    if s.len() > 4096 || s.contains('\0') {
        return Err(Error::new(
            "invalid_query",
            "Query is too long or contains NUL",
        ));
    }
    let t = lex(s)?;
    if t.len() > 128 {
        return Err(Error::new("invalid_query", "Too many query tokens"));
    }
    let mut depth = 0usize;
    let mut negations = 0usize;
    for token in &t {
        match token {
            Token::L => {
                depth += 1;
                if depth > 32 {
                    return Err(Error::new("invalid_query", "Query nesting is too deep"));
                }
            }
            Token::R => {
                depth = depth.saturating_sub(1);
            }
            Token::Not => {
                negations += 1;
                if negations > 32 {
                    return Err(Error::new(
                        "invalid_query",
                        "Too many consecutive NOT operators",
                    ));
                }
            }
            _ => {
                negations = 0;
            }
        }
    }
    if t.is_empty() {
        return Ok(("1".into(), vec![]));
    }
    let mut p = Parser {
        t,
        i: 0,
        args: vec![],
    };
    let sql = p.expr()?;
    if p.i != p.t.len() {
        return Err(Error::new("invalid_query", "Unexpected token"));
    }
    Ok((sql, p.args))
}

// detail=none stores row IDs only. Intersect a few three-character terms, then
// always apply the exact predicate to eliminate candidates with reordered grams.
fn trigram_query(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let positions = [0, (chars.len() - 3) / 2, chars.len() - 3];
    positions
        .into_iter()
        .map(|p| {
            let gram: String = chars[p..p + 3].iter().collect();
            format!("\"{}\"", gram.replace('"', "\"\""))
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}
fn longest_glob_literal(s: &str) -> String {
    let mut best = String::new();
    let mut part = String::new();
    let mut in_class = false;
    for c in s.chars() {
        match c {
            '[' if !in_class => {
                if part.chars().count() > best.chars().count() {
                    best = part.clone()
                }
                part.clear();
                in_class = true;
            }
            ']' if in_class => in_class = false,
            '*' | '?' if !in_class => {
                if part.chars().count() > best.chars().count() {
                    best = part.clone()
                }
                part.clear();
            }
            _ if !in_class => part.push(c),
            _ => {}
        }
    }
    if part.chars().count() > best.chars().count() {
        part
    } else {
        best
    }
}
fn parse_size(op: &str, num: &str) -> Result<(&'static str, i64)> {
    let pos = num
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(num.len());
    let number = &num[..pos];
    let unit = &num[pos..];
    let factor: u128 = match unit {
        "" | "b" => 1,
        "kb" => 1000,
        "mb" => 1_000_000,
        "gb" => 1_000_000_000,
        "kib" => 1024,
        "mib" => 1_048_576,
        "gib" => 1_073_741_824,
        "tib" => 1_099_511_627_776,
        _ => return Err(Error::new("invalid_query", "Invalid size unit")),
    };
    let mut parts = number.split('.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next().unwrap_or("");
    if whole.is_empty()
        || parts.next().is_some()
        || (number.contains('.') && fraction.is_empty())
        || fraction.len() > 18
    {
        return Err(Error::new("invalid_query", "Invalid size"));
    }
    let scale = 10u128.pow(fraction.len() as u32);
    let w: u128 = whole
        .parse()
        .map_err(|_| Error::new("invalid_query", "Invalid size"))?;
    let f: u128 = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse()
            .map_err(|_| Error::new("invalid_query", "Invalid size"))?
    };
    let numerator = w
        .checked_mul(scale)
        .and_then(|x| x.checked_add(f))
        .and_then(|x| x.checked_mul(factor))
        .ok_or_else(|| Error::new("invalid_query", "Size too large"))?;
    let floor = numerator / scale;
    let remainder = numerator % scale;
    if floor > i64::MAX as u128 || (remainder > 0 && floor == i64::MAX as u128) {
        return Err(Error::new("invalid_query", "Size too large"));
    }
    let (new_op, n) = match (op, remainder > 0) {
        (">", true) => (">=", floor + 1),
        (">=", true) => (">=", floor + 1),
        ("<", true) => ("<=", floor),
        ("<=", true) => ("<=", floor),
        ("=", true) => return Ok(("<", 0)),
        (">", false) => (">", floor),
        (">=", false) => (">=", floor),
        ("<", false) => ("<", floor),
        ("<=", false) => ("<=", floor),
        _ => ("=", floor),
    };
    Ok((new_op, n as i64))
}

/// This is a physical-plan hint, never a semantic rewrite. Unknown fields are
/// still rejected by compile(). Unqualified/free-text terms include payload.
pub fn needs_payload(input: &str) -> Result<bool> {
    Ok(lex(input)?.iter().any(|token| match token {
        Token::Word(word) => match word.split_once(':') {
            None => true,
            Some((field, _)) => matches!(
                field.to_ascii_lowercase().as_str(),
                "text" | "description" | "tag"
            ),
        },
        _ => false,
    }))
}
