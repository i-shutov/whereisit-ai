use crate::{Error, Result};
use rusqlite::types::Value;

// Video is an indexed filename-extension heuristic, NOT proof of file content.
const VIDEO_EXTENSION_SQL: &str = concat!(
    "ext IN ('mkv','mp4','avi','mov','m4v','webm',",
    "'mpg','mpeg','wmv','ts','m2ts','vob','flv')"
);

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
#[derive(PartialEq, Eq)]
enum PredicateKind {
    Other,
    ExtensionMatch,
    ExtensionExclusion,
}

#[derive(Clone, Debug)]
enum Expr {
    True,
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Contains(Field, String, Vec<char>, Vec<usize>),
    Glob(Field, String, Vec<char>),
    Extension(String),
    Size(&'static str, i64),
    KindFile,
    Video,
    NameExact(String),
    Category(MetadataCategoryPredicate),
    Branch(i64),
}

#[derive(Clone, Copy, Debug)]
enum Field {
    Text,
    Name,
    Path,
    Description,
    Tag,
}

/// Category operation to resolve against the catalog's category metadata.
#[derive(Clone, Debug)]
pub enum MetadataCategoryPredicate {
    PrimaryId(i64),
    ExactId(i64),
    Id(i64),
    Name(String),
}

/// Borrowed metadata needed to evaluate a query without materializing `Entry`s.
pub struct MetadataRecord<'a> {
    pub id: i64,
    pub name: &'a str,
    pub kind: &'a str,
    pub size: u64,
    pub extension: &'a str,
    pub description: &'a str,
    pub tags: &'a str,
}

/// Catalog-specific data resolved only when a query needs it.
pub trait MetadataQueryContext {
    fn relative_path(&self) -> &str;
    /// Match a path substring without requiring callers to materialize the full path.
    /// Contexts with a structured path representation can override this; the
    /// default preserves the existing Unicode-aware evaluator behavior.
    fn relative_path_contains(&self, matcher: &MetadataContainsMatcher<'_>) -> bool {
        matcher.matches(self.relative_path())
    }
    /// Match a path substring after an unqualified text query missed the
    /// row's name, description, and tags. Structured contexts may override
    /// this to scan path components without materializing the full path.
    fn relative_path_contains_after_name_miss(
        &self,
        matcher: &MetadataContainsMatcher<'_>,
    ) -> bool {
        self.relative_path_contains(matcher)
    }
    fn is_branch_descendant(&self, anchor_id: i64) -> bool;
    fn category_matches(&self, predicate: &MetadataCategoryPredicate) -> bool;
}

/// Reusable substring matcher passed to metadata contexts for path matching.
/// It keeps the compiled query's Unicode lowercase semantics while allowing
/// structured contexts to scan path components without joining them first.
pub struct MetadataContainsMatcher<'a> {
    needle: &'a str,
    target: &'a [char],
    failure: &'a [usize],
}

impl MetadataContainsMatcher<'_> {
    /// Normalized needle shared by equivalent substring terms in this query.
    pub fn normalized_needle(&self) -> &str {
        self.needle
    }

    /// Match text using the same lowercase and Unicode rules as query evaluation.
    pub fn matches(&self, text: &str) -> bool {
        lower_contains(text, self.needle, self.target, self.failure)
    }

    /// True when matching can be performed independently on slash-separated
    /// components, with ASCII case folding.
    pub fn is_ascii_component_needle(&self) -> bool {
        self.needle.is_ascii() && !self.needle.contains('/')
    }
}

/// Parsed Atlas query that can be evaluated against borrowed in-memory metadata.
pub struct MetadataQuery {
    expr: Expr,
}

/// Reusable matcher for SQLite-compatible GLOB over already normalized text.
pub struct MetadataGlobPattern {
    pattern: Vec<char>,
}

impl MetadataGlobPattern {
    /// Compile a lowercase GLOB pattern once for repeated row checks.
    pub fn new(lowered_pattern: &str) -> Self {
        Self {
            pattern: lowered_pattern.chars().collect(),
        }
    }

    /// Match a lowercase value using SQLite GLOB behavior.
    pub fn is_match(&self, lowered_value: &str) -> bool {
        glob_match(lowered_value, &self.pattern)
    }
}

/// Parameterized DuckDB predicate for the normalized `ctf_rows` table.
/// Wildcard queries use `atlas_glob(value, pattern)` and require a compatible UDF.
pub struct DuckDbPredicate {
    pub sql: String,
    pub args: Vec<Value>,
    pub requires_glob_udf: bool,
}

impl MetadataQuery {
    pub fn compile_metadata(input: &str) -> Result<Self> {
        let (predicate, _) = compile_predicate(input)?;
        Ok(Self {
            expr: predicate.expr,
        })
    }

    pub fn matches<C: MetadataQueryContext>(
        &self,
        record: &MetadataRecord<'_>,
        context: &C,
    ) -> bool {
        eval(&self.expr, record, context)
    }

    pub fn needs_relative_path(&self) -> bool {
        needs_path(&self.expr)
    }

    pub fn needs_extension(&self) -> bool {
        needs_extension(&self.expr)
    }

    /// Emit a parameterized predicate for CTF rows, whose category assignments
    /// are empty. Bind `args` in order. Register SQLite-compatible `atlas_glob`
    /// when `requires_glob_udf` is true.
    pub fn duckdb_predicate(&self) -> DuckDbPredicate {
        let mut args = Vec::new();
        let mut requires_glob_udf = false;
        let sql = emit_duckdb(&self.expr, &mut args, &mut requires_glob_udf);
        DuckDbPredicate {
            sql,
            args,
            requires_glob_udf,
        }
    }
}

fn emit_duckdb(expr: &Expr, args: &mut Vec<Value>, requires_glob_udf: &mut bool) -> String {
    match expr {
        Expr::True => "TRUE".into(),
        Expr::And(a, b) => format!(
            "({} AND {})",
            emit_duckdb(a, args, requires_glob_udf),
            emit_duckdb(b, args, requires_glob_udf)
        ),
        Expr::Or(a, b) => format!(
            "({} OR {})",
            emit_duckdb(a, args, requires_glob_udf),
            emit_duckdb(b, args, requires_glob_udf)
        ),
        Expr::Not(inner) => format!("(NOT {})", emit_duckdb(inner, args, requires_glob_udf)),
        Expr::Contains(field, value, _, _) => {
            args.push(Value::Text(value.clone()));
            format!("strpos({}, ?) > 0", duckdb_field(*field))
        }
        Expr::Glob(field, value, _) => {
            *requires_glob_udf = true;
            args.push(Value::Text(value.clone()));
            format!("atlas_glob({}, ?)", duckdb_field(*field))
        }
        Expr::Extension(value) => {
            args.push(Value::Text(value.clone()));
            "ext = ?".into()
        }
        Expr::Size(op, size) => {
            args.push(Value::Integer(*size));
            format!("size {op} ?")
        }
        Expr::KindFile => "kind = 'file'".into(),
        Expr::Video => {
            "ext IN ('mkv','mp4','avi','mov','m4v','webm','mpg','mpeg','wmv','ts','m2ts','vob','flv')".into()
        }
        Expr::NameExact(value) => {
            args.push(Value::Text(value.clone()));
            "name_norm = ?".into()
        }
        Expr::Category(MetadataCategoryPredicate::PrimaryId(0)) => "kind = 'file'".into(),
        // CTF import deliberately has no category assignments.
        Expr::Category(_) => "FALSE".into(),
        Expr::Branch(anchor) => {
            args.push(Value::Integer(*anchor));
            "id IN (WITH RECURSIVE branch(id, volume_id) AS (SELECT child.id, child.volume_id FROM ctf_rows anchor JOIN ctf_rows child ON child.parent_id = anchor.id AND child.volume_id = anchor.volume_id WHERE anchor.id = ? UNION SELECT child.id, child.volume_id FROM ctf_rows child JOIN branch parent ON child.parent_id = parent.id AND child.volume_id = parent.volume_id) SELECT id FROM branch)".into()
        }
    }
}

fn duckdb_field(field: Field) -> &'static str {
    match field {
        Field::Text => "(name_norm || chr(10) || path_norm || chr(10) || description_norm || chr(10) || tags_norm)",
        Field::Name => "name_norm",
        Field::Path => "path_norm",
        Field::Description => "description_norm",
        Field::Tag => "tags_norm",
    }
}

fn needs_path(expr: &Expr) -> bool {
    match expr {
        Expr::Contains(Field::Path | Field::Text, _, _, _)
        | Expr::Glob(Field::Path | Field::Text, _, _) => true,
        Expr::And(a, b) | Expr::Or(a, b) => needs_path(a) || needs_path(b),
        Expr::Not(a) => needs_path(a),
        _ => false,
    }
}

fn needs_extension(expr: &Expr) -> bool {
    match expr {
        Expr::Extension(_) | Expr::Video => true,
        Expr::And(a, b) | Expr::Or(a, b) => needs_extension(a) || needs_extension(b),
        Expr::Not(a) => needs_extension(a),
        _ => false,
    }
}

fn field_kind(field: &str) -> Field {
    match field {
        "name" => Field::Name,
        "path" => Field::Path,
        "description" => Field::Description,
        "tag" => Field::Tag,
        _ => Field::Text,
    }
}

fn contains_expr(field: Field, value: String) -> Expr {
    let target: Vec<char> = value.chars().collect();
    let mut failure = vec![0; target.len()];
    let mut matched = 0;
    for i in 1..target.len() {
        while matched > 0 && target[i] != target[matched] {
            matched = failure[matched - 1];
        }
        if target[i] == target[matched] {
            matched += 1;
        }
        failure[i] = matched;
    }
    Expr::Contains(field, value, target, failure)
}

fn glob_expr(field: Field, value: String) -> Expr {
    let pattern = value.chars().collect();
    Expr::Glob(field, value, pattern)
}

fn eval<C: MetadataQueryContext>(expr: &Expr, row: &MetadataRecord<'_>, ctx: &C) -> bool {
    match expr {
        Expr::True => true,
        Expr::And(a, b) => eval(a, row, ctx) && eval(b, row, ctx),
        Expr::Or(a, b) => eval(a, row, ctx) || eval(b, row, ctx),
        Expr::Not(a) => !eval(a, row, ctx),
        Expr::Extension(v) => lower_eq(row.extension, v),
        Expr::Size(op, n) => compare_size(row.size, op, *n),
        Expr::KindFile => row.kind == "file",
        Expr::Video => [
            "mkv", "mp4", "avi", "mov", "m4v", "webm", "mpg", "mpeg", "wmv", "ts", "m2ts", "vob",
            "flv",
        ]
        .iter()
        .any(|ext| lower_eq(row.extension, ext)),
        Expr::NameExact(v) => lower_eq(row.name, v),
        Expr::Category(
            MetadataCategoryPredicate::PrimaryId(_) | MetadataCategoryPredicate::ExactId(_),
        ) if row.kind != "file" => false,
        Expr::Category(p) => ctx.category_matches(p),
        Expr::Branch(id) => ctx.is_branch_descendant(*id),
        Expr::Contains(field, needle, target, failure) => match field {
            Field::Text if !target.contains(&'\n') => {
                // A term without a separator cannot straddle search_text fields.
                // Check the already available strings first, then request the path.
                lower_contains(row.name, needle, target, failure)
                    || lower_contains(row.description, needle, target, failure)
                    || lower_contains(row.tags, needle, target, failure)
                    || ctx.relative_path_contains_after_name_miss(&MetadataContainsMatcher {
                        needle,
                        target,
                        failure,
                    })
            }
            Field::Text => lower_contains_parts(
                &[row.name, ctx.relative_path(), row.description, row.tags],
                needle,
                target,
                failure,
            ),
            Field::Name => lower_contains(row.name, needle, target, failure),
            Field::Path => ctx.relative_path_contains(&MetadataContainsMatcher {
                needle,
                target,
                failure,
            }),
            Field::Description => lower_contains(row.description, needle, target, failure),
            Field::Tag => lower_contains(row.tags, needle, target, failure),
        },
        Expr::Glob(field, _pattern_text, pattern) => {
            let val = match field {
                Field::Text => {
                    return glob_match(
                        &format!(
                            "{}\n{}\n{}\n{}",
                            row.name,
                            ctx.relative_path(),
                            row.description,
                            row.tags
                        )
                        .to_lowercase(),
                        pattern,
                    )
                }
                Field::Name => row.name.to_lowercase(),
                Field::Path => ctx.relative_path().to_lowercase(),
                Field::Description => row.description.to_lowercase(),
                Field::Tag => row.tags.to_lowercase(),
            };
            glob_match(&val, pattern)
        }
    }
}

fn lower_eq(value: &str, lowered: &str) -> bool {
    if value.is_ascii() && lowered.is_ascii() {
        value.eq_ignore_ascii_case(lowered)
    } else {
        value.to_lowercase() == lowered
    }
}

fn lower_contains(value: &str, needle: &str, target: &[char], failure: &[usize]) -> bool {
    lower_contains_parts(&[value], needle, target, failure)
}

fn lower_contains_parts(parts: &[&str], needle: &str, target: &[char], failure: &[usize]) -> bool {
    if target.is_empty() {
        return true;
    }
    let needle_is_ascii = needle.is_ascii();
    if !needle_is_ascii && parts.iter().all(|part| part.is_ascii()) {
        return false;
    }
    if !needle.contains('\n') && needle_is_ascii && parts.iter().all(|part| part.is_ascii()) {
        let pattern = needle.as_bytes();
        return parts.iter().any(|part| {
            part.as_bytes()
                .windows(pattern.len())
                .any(|window| window.eq_ignore_ascii_case(pattern))
        });
    }
    // Rust's str lowercase includes contextual final sigma; char lowercase does
    // not. Preserve exact SQLite unicode_lower semantics on those rare records.
    if parts.iter().any(|part| part.contains('Σ')) {
        let mut lowered = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                lowered.push('\n');
            }
            lowered.push_str(&part.to_lowercase());
        }
        let needle: String = target.iter().collect();
        return lowered.contains(&needle);
    }
    let mut matched = 0usize;
    for ch in parts.iter().enumerate().flat_map(|(i, p)| {
        let separator = (i > 0).then_some('\n').into_iter();
        separator.chain(p.chars())
    }) {
        for c in ch.to_lowercase() {
            while matched > 0 && c != target[matched] {
                matched = failure[matched - 1];
            }
            if c == target[matched] {
                matched += 1;
                if matched == target.len() {
                    return true;
                }
            }
        }
    }
    false
}

fn compare_size(size: u64, op: &str, n: i64) -> bool {
    let n = n as u64;
    match op {
        ">" => size > n,
        ">=" => size >= n,
        "<" => size < n,
        "<=" => size <= n,
        _ => size == n,
    }
}

// SQLite GLOB wildcard semantics: * matches any sequence, ? one Unicode scalar,
// and bracket classes support ranges and leading ^ negation.
fn glob_match(value: &str, p: &[char]) -> bool {
    fn atom(p: &[char], j: usize, value: char) -> Option<(bool, usize)> {
        if p[j] == '?' {
            return Some((true, j + 1));
        }
        if p[j] != '[' {
            return Some((value == p[j], j + 1));
        }
        let mut k = j + 1;
        let neg = k < p.len() && p[k] == '^';
        if neg {
            k += 1;
        }
        let mut hit = false;
        let mut any = false;
        while k < p.len() {
            if p[k] == ']' && any {
                break;
            }
            let start = p[k];
            any = true;
            if k + 2 < p.len() && p[k + 1] == '-' && p[k + 2] != ']' {
                hit |= value >= start && value <= p[k + 2];
                k += 3;
            } else {
                hit |= value == start;
                k += 1;
            }
        }
        if k == p.len() {
            return Some((false, j + 1));
        }
        Some((hit != neg, k + 1))
    }
    // SQLite's GLOB implementation treats an embedded U+0000 as the end of
    // the UTF-8 string, even when the bound TEXT value has a larger byte len.
    let value = value.split('\0').next().unwrap_or("");
    let mut vi = 0;
    let mut pi = 0;
    let mut star_pattern = None;
    let mut star_value = 0;
    while vi < value.len() {
        let ch = value[vi..].chars().next().unwrap();
        if pi < p.len() && p[pi] == '*' {
            star_pattern = Some(pi + 1);
            pi += 1;
            star_value = vi;
            continue;
        }
        if pi < p.len() {
            let (hit, next) = atom(p, pi, ch).unwrap();
            if hit {
                vi += ch.len_utf8();
                pi = next;
                continue;
            }
        }
        if let Some(after_star) = star_pattern {
            if star_value >= value.len() {
                return false;
            }
            let c = value[star_value..].chars().next().unwrap();
            star_value += c.len_utf8();
            vi = star_value;
            pi = after_star;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

pub(crate) struct Predicate {
    pub(crate) sql: String,
    kind: PredicateKind,
    expr: Expr,
}

impl Predicate {
    fn other(sql: String) -> Self {
        Self {
            sql,
            kind: PredicateKind::Other,
            expr: Expr::True,
        }
    }

    fn with_expr(sql: String, expr: Expr) -> Self {
        Self {
            sql,
            kind: PredicateKind::Other,
            expr,
        }
    }

    pub(crate) fn within_volume(self) -> Self {
        Self::other(format!("{} AND volume_id = ?", self.sql))
    }

    pub(crate) fn count_sql(&self, table: &str) -> String {
        if self.kind == PredicateKind::ExtensionExclusion {
            // Every supported schema declares ext NOT NULL. The unfiltered
            // count can use SQLite's b-tree Count opcode, and equality can
            // seek entries_ext. Both counts read the same statement snapshot.
            format!(
                "SELECT (SELECT COUNT(*) FROM {table}) - \
                 (SELECT COUNT(*) FROM {table} WHERE ext = ?)"
            )
        } else {
            format!("SELECT COUNT(*) FROM {table} WHERE {}", self.sql)
        }
    }
}

struct Parser {
    t: Vec<Token>,
    i: usize,
    args: Vec<Value>,
}
impl Parser {
    fn expr(&mut self) -> Result<Predicate> {
        let mut x = self.and()?;
        while self.t.get(self.i) == Some(&Token::Or) {
            self.i += 1;
            let y = self.and()?;
            let expr = Expr::Or(Box::new(x.expr), Box::new(y.expr));
            x = Predicate::with_expr(format!("({} OR {})", x.sql, y.sql), expr);
        }
        Ok(x)
    }
    fn and(&mut self) -> Result<Predicate> {
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
            let expr = Expr::And(Box::new(x.expr), Box::new(y.expr));
            x = Predicate::with_expr(format!("({} AND {})", x.sql, y.sql), expr);
        }
        Ok(x)
    }
    fn unary(&mut self) -> Result<Predicate> {
        if self.t.get(self.i) == Some(&Token::Not) {
            self.i += 1;
            let inner = self.unary()?;
            return Ok(Predicate {
                sql: format!("(NOT {})", inner.sql),
                expr: Expr::Not(Box::new(inner.expr)),
                kind: if inner.kind == PredicateKind::ExtensionMatch {
                    PredicateKind::ExtensionExclusion
                } else {
                    PredicateKind::Other
                },
            });
        }
        if self.t.get(self.i) == Some(&Token::L) {
            self.i += 1;
            let x = self.expr()?;
            if self.t.get(self.i) != Some(&Token::R) {
                return Err(Error::new("invalid_query", "Missing closing parenthesis"));
            }
            self.i += 1;
            return Ok(Predicate {
                sql: format!("({})", x.sql),
                kind: x.kind,
                expr: x.expr,
            });
        }
        let w = match self.t.get(self.i) {
            Some(Token::Word(w)) => w.clone(),
            _ => return Err(Error::new("invalid_query", "Expected a search term")),
        };
        self.i += 1;
        self.term(&w)
    }
    fn term(&mut self, w: &str) -> Result<Predicate> {
        let (field, value) = match w.split_once(':') {
            Some((a, b)) => (a.to_ascii_lowercase(), b),
            None => ("text".into(), w),
        };
        if value.is_empty() && field != "ext" {
            return Err(Error::new("invalid_query", "Empty field value"));
        }
        let v = value.to_lowercase();
        if field == "ext" {
            self.args
                .push(Value::Text(v.trim_start_matches('.').into()));
            return Ok(Predicate {
                sql: "ext = ?".into(),
                kind: PredicateKind::ExtensionMatch,
                expr: Expr::Extension(v.trim_start_matches('.').to_owned()),
            });
        }
        let sql = match field.as_str() {
            "primary_category_id" => {
                let id = value
                    .parse::<i64>()
                    .ok()
                    .filter(|id| *id >= 0)
                    .ok_or_else(|| {
                        Error::new(
                            "invalid_query",
                            "primary_category_id requires a non-negative integer",
                        )
                    })?;
                self.args.push(Value::Integer(id));
                Ok(("id IN (SELECT e.id FROM entries e WHERE e.kind='file' AND COALESCE((SELECT MIN(category_id) FROM entry_categories WHERE entry_id=e.id),0)=?)".into(), Expr::Category(MetadataCategoryPredicate::PrimaryId(id))))
            }
            "category_exact_id" => {
                let id = value
                    .parse::<i64>()
                    .ok()
                    .filter(|id| *id > 0)
                    .ok_or_else(|| {
                        Error::new(
                            "invalid_query",
                            "category_exact_id requires a positive integer",
                        )
                    })?;
                self.args.push(Value::Integer(id));
                Ok(("id IN (SELECT e.id FROM entries e WHERE e.kind='file' AND EXISTS(SELECT 1 FROM entry_categories ec WHERE ec.entry_id=e.id AND ec.category_id=?))".into(), Expr::Category(MetadataCategoryPredicate::ExactId(id))))
            }
            "branch_id" => {
                let id = value
                    .parse::<i64>()
                    .ok()
                    .filter(|id| *id > 0)
                    .ok_or_else(|| {
                        Error::new("invalid_query", "branch_id requires a positive integer")
                    })?;
                self.args.push(Value::Integer(id));
                Ok(("id IN (WITH RECURSIVE branch(id,volume_id) AS (SELECT child.id,child.volume_id FROM entries anchor JOIN entries child ON child.parent_id=anchor.id AND child.volume_id=anchor.volume_id WHERE anchor.id=? UNION SELECT child.id,child.volume_id FROM entries child JOIN branch parent ON child.parent_id=parent.id AND child.volume_id=parent.volume_id) SELECT id FROM branch)".into(), Expr::Branch(id)))
            }
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
                Ok((format!("size {op} ?"), Expr::Size(op, size)))
            }
            "name_exact" => {
                // Case-folded literal equality: any recorded size, any disk.
                // Bound parameter, never filename interpolation or hash inference.
                self.args.push(Value::Text(v.clone()));
                Ok(("name_norm = ?".into(), Expr::NameExact(v)))
            }
            "kind" if v == "file" => Ok(("kind = 'file'".into(), Expr::KindFile)),
            "kind" => Err(Error::new(
                "invalid_query",
                "Unsupported kind filter; kind:file selects recorded files only",
            )),
            "type" if v == "video" => Ok((VIDEO_EXTENSION_SQL.into(), Expr::Video)),
            "type" => Err(Error::new(
                "invalid_query",
                "Unsupported type filter; type:video checks filename extensions only",
            )),
            "category_id" => {
                // Stable identity across repeated Unicode display names.
                // Parents include descendant categories and their marked files.
                let id = v.parse::<i64>().ok().filter(|id| *id > 0).ok_or_else(|| {
                    Error::new("invalid_query", "category_id requires a positive integer")
                })?;
                self.args.push(Value::Integer(id));
                Ok(("id IN (WITH RECURSIVE category_tree(id) AS (SELECT id FROM categories WHERE id=? UNION SELECT child.id FROM categories child JOIN category_tree parent ON child.parent_id=parent.id), marked(id,volume_id) AS (SELECT e.id,e.volume_id FROM category_tree c JOIN entry_categories ec ON ec.category_id=c.id JOIN entries e ON e.id=ec.entry_id UNION SELECT child.id,child.volume_id FROM marked m JOIN entries child ON child.volume_id=m.volume_id AND child.parent_id=m.id) SELECT id FROM marked)".into(), Expr::Category(MetadataCategoryPredicate::Id(id))))
            }
            "category" => {
                self.args.push(Value::Text(v.clone()));
                // Legacy name search intentionally unions same-named categories.
                // Like category_id, it includes category descendants and files
                // under explicitly assigned folders.
                Ok(("id IN (WITH RECURSIVE category_tree(id) AS (SELECT id FROM categories WHERE name_norm=? UNION SELECT child.id FROM categories child JOIN category_tree parent ON child.parent_id=parent.id), marked(id,volume_id) AS (SELECT e.id,e.volume_id FROM category_tree c JOIN entry_categories ec ON ec.category_id=c.id JOIN entries e ON e.id=ec.entry_id UNION SELECT child.id,child.volume_id FROM marked m JOIN entries child ON child.volume_id=m.volume_id AND child.parent_id=m.id) SELECT id FROM marked)".into(), Expr::Category(MetadataCategoryPredicate::Name(v.clone()))))
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
                    self.args.push(Value::Text(v.clone()));
                    Ok((format!("(id IN (SELECT rowid FROM entries_trigram WHERE entries_trigram MATCH ?) AND {condition})"), if wildcard { glob_expr(field_kind(&field), v) } else { contains_expr(field_kind(&field), v) }))
                } else {
                    self.args.push(Value::Text(v.clone()));
                    Ok((
                        condition,
                        if wildcard {
                            glob_expr(field_kind(&field), v)
                        } else {
                            contains_expr(field_kind(&field), v)
                        },
                    ))
                }
            }
            _ => Err(Error::new(
                "invalid_query",
                format!("Unknown field: {field}"),
            )),
        }?;
        Ok(Predicate::with_expr(sql.0, sql.1))
    }
}
pub fn compile(s: &str) -> Result<(String, Vec<Value>)> {
    let (predicate, args) = compile_predicate(s)?;
    Ok((predicate.sql, args))
}

pub(crate) fn compile_predicate(s: &str) -> Result<(Predicate, Vec<Value>)> {
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
        return Ok((Predicate::other("1".into()), vec![]));
    }
    let mut p = Parser {
        t,
        i: 0,
        args: vec![],
    };
    let predicate = p.expr()?;
    if p.i != p.t.len() {
        return Err(Error::new("invalid_query", "Unexpected token"));
    }
    Ok((predicate, p.args))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Catalog;
    use rusqlite::params_from_iter;

    #[test]
    fn glob_match_agrees_with_sqlite_on_bracket_and_wildcard_edges() {
        fn strings(alphabet: &[char], max_len: usize) -> Vec<String> {
            let mut out = vec![String::new()];
            let mut layer = vec![String::new()];
            for _ in 0..max_len {
                let mut next = Vec::new();
                for prefix in &layer {
                    for ch in alphabet {
                        let mut s = prefix.clone();
                        s.push(*ch);
                        next.push(s);
                    }
                }
                out.extend(next.iter().cloned());
                layer = next;
            }
            out
        }
        let catalog = Catalog::open(":memory:").unwrap();
        let mut stmt = catalog
            .conn()
            .prepare("SELECT unicode_lower(?1) GLOB unicode_lower(?2)")
            .unwrap();
        let mut values = strings(&['a', ']', '['], 3);
        values.extend(["é".into(), "İ".into(), "Σ".into(), "a\0b".into()]);
        let mut patterns = strings(&['a', ']', '[', '^', '-', '*', '?'], 3);
        patterns.extend([
            "[]]".into(),
            "[a]".into(),
            "[^a]".into(),
            "[a-z]".into(),
            "[unclosed".into(),
            "[a-]".into(),
            "[-a]".into(),
        ]);
        for value in &values {
            for pattern in &patterns {
                let expected: bool = stmt.query_row((value, pattern), |row| row.get(0)).unwrap();
                let actual = glob_match(
                    &value.to_lowercase(),
                    &pattern.to_lowercase().chars().collect::<Vec<_>>(),
                );
                assert_eq!(actual, expected, "value={value:?}, pattern={pattern:?}");
            }
        }
    }

    #[test]
    fn duckdb_predicate_emits_ast_fields_and_parameters_in_order() {
        let query = MetadataQuery::compile_metadata(
            "name:Résumé OR (path:資料 AND size:>2mb) AND branch_id:17",
        )
        .unwrap();
        let emitted = query.duckdb_predicate();
        assert_eq!(
            emitted.sql,
            "(strpos(name_norm, ?) > 0 OR ((strpos(path_norm, ?) > 0 AND size > ?) AND id IN (WITH RECURSIVE branch(id, volume_id) AS (SELECT child.id, child.volume_id FROM ctf_rows anchor JOIN ctf_rows child ON child.parent_id = anchor.id AND child.volume_id = anchor.volume_id WHERE anchor.id = ? UNION SELECT child.id, child.volume_id FROM ctf_rows child JOIN branch parent ON child.parent_id = parent.id AND child.volume_id = parent.volume_id) SELECT id FROM branch)))"
        );
        assert_eq!(
            emitted.args,
            vec![
                Value::Text("résumé".into()),
                Value::Text("資料".into()),
                Value::Integer(2_000_000),
                Value::Integer(17),
            ]
        );
        assert!(!emitted.requires_glob_udf);

        let glob = MetadataQuery::compile_metadata("text:movie*")
            .unwrap()
            .duckdb_predicate();
        assert!(glob.requires_glob_udf);
        assert!(glob.sql.starts_with("atlas_glob((name_norm || chr(10)"));
        assert_eq!(glob.args, vec![Value::Text("movie*".into())]);
    }

    #[test]
    fn duckdb_ctf_category_predicates_reflect_empty_assignments() {
        let zero = MetadataQuery::compile_metadata("primary_category_id:0")
            .unwrap()
            .duckdb_predicate();
        assert_eq!(zero.sql, "kind = 'file'");
        assert!(zero.args.is_empty());

        let positive = MetadataQuery::compile_metadata("NOT category_id:3")
            .unwrap()
            .duckdb_predicate();
        assert_eq!(positive.sql, "(NOT FALSE)");
        assert!(positive.args.is_empty());
    }

    struct EntryContext<'a>(&'a crate::Entry);
    impl MetadataQueryContext for EntryContext<'_> {
        fn relative_path(&self) -> &str {
            &self.0.relative_path
        }
        fn is_branch_descendant(&self, anchor_id: i64) -> bool {
            // The parity fixture is shallow; production contexts resolve ancestry lazily.
            self.0.parent_id == Some(anchor_id)
        }
        fn category_matches(&self, predicate: &MetadataCategoryPredicate) -> bool {
            matches!(predicate, MetadataCategoryPredicate::PrimaryId(0)) && self.0.kind == "file"
        }
    }

    struct ComponentPathContext<'a>(&'a [&'a str]);
    impl MetadataQueryContext for ComponentPathContext<'_> {
        fn relative_path(&self) -> &str {
            panic!("component path match should not materialize the path")
        }
        fn relative_path_contains(&self, matcher: &MetadataContainsMatcher<'_>) -> bool {
            assert!(matcher.is_ascii_component_needle());
            self.0.iter().any(|component| matcher.matches(component))
        }
        fn is_branch_descendant(&self, _: i64) -> bool {
            false
        }
        fn category_matches(&self, _: &MetadataCategoryPredicate) -> bool {
            false
        }
    }

    struct HookContext<'a> {
        path: &'a str,
        ordinary_calls: std::cell::Cell<usize>,
        after_miss_calls: std::cell::Cell<usize>,
    }
    impl MetadataQueryContext for HookContext<'_> {
        fn relative_path(&self) -> &str {
            self.path
        }
        fn relative_path_contains(&self, matcher: &MetadataContainsMatcher<'_>) -> bool {
            self.ordinary_calls.set(self.ordinary_calls.get() + 1);
            matcher.matches(self.path)
        }
        fn relative_path_contains_after_name_miss(
            &self,
            matcher: &MetadataContainsMatcher<'_>,
        ) -> bool {
            self.after_miss_calls.set(self.after_miss_calls.get() + 1);
            matcher.matches(self.path)
        }
        fn is_branch_descendant(&self, _: i64) -> bool {
            false
        }
        fn category_matches(&self, _: &MetadataCategoryPredicate) -> bool {
            false
        }
    }

    fn hook_context(path: &str) -> HookContext<'_> {
        HookContext {
            path,
            ordinary_calls: std::cell::Cell::new(0),
            after_miss_calls: std::cell::Cell::new(0),
        }
    }

    #[test]
    fn text_path_hook_runs_only_after_leaf_misses_and_keeps_newline_and_path_dispatch() {
        let row = MetadataRecord {
            id: 1,
            name: "needle.bin",
            kind: "file",
            size: 0,
            extension: "bin",
            description: "",
            tags: "",
        };
        let ctx = hook_context("Folder/needle.bin");
        assert!(MetadataQuery::compile_metadata("needle")
            .unwrap()
            .matches(&row, &ctx));
        assert_eq!(
            ctx.after_miss_calls.get(),
            0,
            "name hit short-circuits path lookup"
        );

        let miss_row = MetadataRecord {
            name: "other.bin",
            ..row
        };
        assert!(MetadataQuery::compile_metadata("needle")
            .unwrap()
            .matches(&miss_row, &ctx));
        assert_eq!(ctx.after_miss_calls.get(), 1);
        assert_eq!(ctx.ordinary_calls.get(), 0);

        // Slash-containing text retains full-path substring behavior.
        assert!(MetadataQuery::compile_metadata("Folder/needle")
            .unwrap()
            .matches(&miss_row, &ctx));
        assert_eq!(ctx.after_miss_calls.get(), 2);

        // Newline-bearing text uses the joined fields path and does not use either hook.
        assert!(
            MetadataQuery::compile_metadata("text:\"other.bin\nFolder\"")
                .unwrap()
                .matches(&miss_row, &ctx)
        );
        assert_eq!(ctx.after_miss_calls.get(), 2);
        assert_eq!(ctx.ordinary_calls.get(), 0);

        assert!(MetadataQuery::compile_metadata("path:needle")
            .unwrap()
            .matches(&miss_row, &ctx));
        assert_eq!(ctx.ordinary_calls.get(), 1, "Path keeps the original hook");
    }

    #[test]
    fn lower_contains_parts_fast_rejection_and_unicode_matcher_parity() {
        let ascii_query = MetadataQuery::compile_metadata("text:résumé").unwrap();
        let Expr::Contains(_, needle, target, failure) = &ascii_query.expr else {
            panic!("contains expected")
        };
        assert!(!lower_contains_parts(
            &["plain ASCII", "more ASCII"],
            needle,
            target,
            failure
        ));

        for (query, haystack) in [("ος", "ΟΣ"), ("i̇", "İ"), ("k", "K")] {
            let compiled = MetadataQuery::compile_metadata(&format!("path:{query}")).unwrap();
            let Expr::Contains(_, needle, target, failure) = &compiled.expr else {
                panic!("contains expected")
            };
            let matcher = MetadataContainsMatcher {
                needle,
                target,
                failure,
            };
            assert_eq!(
                matcher.matches(haystack),
                haystack.to_lowercase().contains(needle),
                "{query:?} in {haystack:?}"
            );
        }
    }

    #[test]
    fn path_contains_context_hook_avoids_materializing_ascii_component_paths() {
        let components = ["Root", "Media", "Feature.MKV"];
        let context = ComponentPathContext(&components);
        let record = MetadataRecord {
            id: 1,
            name: "unrelated.bin",
            kind: "file",
            size: 0,
            extension: "bin",
            description: "",
            tags: "",
        };
        for query in ["path:feature", "feature"] {
            assert!(MetadataQuery::compile_metadata(query)
                .unwrap()
                .matches(&record, &context));
        }
        assert!(!MetadataQuery::compile_metadata("path:absent")
            .unwrap()
            .matches(&record, &context));

        let matching_name = MetadataRecord {
            name: "Feature.MKV",
            ..record
        };
        assert!(MetadataQuery::compile_metadata("feature")
            .unwrap()
            .matches(&matching_name, &context));
    }

    #[test]
    fn contains_matcher_preserves_unicode_semantics_and_component_eligibility() {
        let query = MetadataQuery::compile_metadata("path:ος").unwrap();
        let Expr::Contains(_, needle, target, failure) = &query.expr else {
            panic!("expected contains expression");
        };
        let matcher = MetadataContainsMatcher {
            needle,
            target,
            failure,
        };
        assert!(!matcher.is_ascii_component_needle());
        // The dot is case-ignorable and the following T keeps uppercase Σ
        // non-final, so use a component ending at Σ for the final-sigma case.
        assert!(matcher.matches("Unicode/ΟΣ"));

        let query = MetadataQuery::compile_metadata("path:disk").unwrap();
        let Expr::Contains(_, needle, target, failure) = &query.expr else {
            panic!("expected contains expression");
        };
        assert!(MetadataContainsMatcher {
            needle,
            target,
            failure,
        }
        .is_ascii_component_needle());

        let query = MetadataQuery::compile_metadata("path:folder/name").unwrap();
        let Expr::Contains(_, needle, target, failure) = &query.expr else {
            panic!("expected contains expression");
        };
        assert!(!MetadataContainsMatcher {
            needle,
            target,
            failure,
        }
        .is_ascii_component_needle());
    }

    fn metadata_record(entry: &crate::Entry) -> MetadataRecord<'_> {
        let ext = std::path::Path::new(&entry.name)
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("");
        MetadataRecord {
            id: entry.id,
            name: &entry.name,
            kind: &entry.kind,
            size: entry.size,
            extension: ext,
            description: &entry.description,
            tags: &entry.tags,
        }
    }

    #[test]
    fn in_memory_evaluator_matches_sql_for_shared_query_grammar() {
        let dir = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::open(dir.path().join("metadata-query.sqlite")).unwrap();
        catalog
            .bulk_import("Disk", "offline", |w| {
                for (name, path, kind, size, description, tags) in [
                    (
                        "résumé.TXT",
                        "資料/résumé.TXT",
                        "file",
                        12,
                        "Notes about Café",
                        "Personal",
                    ),
                    (
                        "movie.MKV",
                        "Media/movie.MKV",
                        "file",
                        2_000_000,
                        "Feature film",
                        "Video",
                    ),
                    (
                        "archive.zip",
                        "Media/archive.zip",
                        "file",
                        90,
                        "Bundle",
                        "backup",
                    ),
                    ("Café", "資料/Café", "directory", 0, "Folder", ""),
                    (
                        "a[b].dat",
                        "Meta/a[b].dat",
                        "file",
                        33,
                        "literal bracket",
                        "",
                    ),
                    (
                        "İstanbul.TXT",
                        "Unicode/İstanbul.TXT",
                        "file",
                        34,
                        "dotted I",
                        "",
                    ),
                    ("Σίγμα.bin", "Unicode/Σίγμα.bin", "file", 35, "sigma", ""),
                    (
                        "ΟΣ.TXT",
                        "Unicode/ΟΣ.TXT",
                        "file",
                        37,
                        "greek final sigma",
                        "",
                    ),
                    ("a\"b.txt", "Meta/a\"b.txt", "file", 36, "quoted", ""),
                ] {
                    w.insert(&crate::NewEntry {
                        parent_id: None,
                        name: name.into(),
                        relative_path: path.into(),
                        kind: kind.into(),
                        size,
                        modified_unix: None,
                        description: description.into(),
                        tags: tags.into(),
                    })?;
                }
                Ok(())
            })
            .unwrap();
        let all = catalog.search("", None, 0, 100).unwrap().items;
        for query in [
            "",
            "name:résumé",
            "path:資料",
            "description:café",
            "tag:personal",
            "ext:MKV",
            "size:>1mb",
            "kind:file",
            "type:video",
            "(ext:txt OR ext:mkv) AND NOT size:<100",
            "text:café",
            "name:*.mkv",
            "text:feature*",
            "name:*[a-z]*",
            "name:[^m]*",
            "name:[abc].dat",
            "name:[unclosed",
            "name:résumé?",
            "name_exact:İSTANBUL.TXT",
            "name:ος",
            "name_exact:ΟΣ.TXT",
            "text:\"résumé.TXT\n資料\"",
            "name:\"a\\\"b.txt\"",
            "primary_category_id:0",
            "(name:résumé OR name:movie) archive",
        ] {
            let sql_page = catalog.search(query, None, 0, 100).unwrap();
            let compiled = MetadataQuery::compile_metadata(query).unwrap();
            let expected = sql_page.items.iter().map(|e| e.id).collect::<Vec<_>>();
            let mut actual = Vec::new();
            for entry in &all {
                let record = metadata_record(entry);
                if compiled.matches(&record, &EntryContext(entry)) {
                    actual.push(entry.id);
                }
            }
            assert_eq!(actual, expected, "query={query}");
        }
        for bad in ["unknown:x", "size:nope", "(name:x", "name:\"unterminated"] {
            assert!(MetadataQuery::compile_metadata(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn branch_id_searches_only_same_volume_descendants_excluding_anchor() {
        let dir = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::open(dir.path().join("branch.sqlite")).unwrap();
        let mut branch = 0;
        catalog
            .bulk_import("Disk", "offline", |w| {
                branch = w.insert(&crate::NewEntry {
                    parent_id: None,
                    name: "資料".into(),
                    relative_path: "資料".into(),
                    kind: "directory".into(),
                    size: 0,
                    modified_unix: None,
                    description: String::new(),
                    tags: String::new(),
                })?;
                let nested = w.insert(&crate::NewEntry {
                    parent_id: Some(branch),
                    name: "nested".into(),
                    relative_path: "資料/nested".into(),
                    kind: "directory".into(),
                    size: 0,
                    modified_unix: None,
                    description: String::new(),
                    tags: String::new(),
                })?;
                w.insert(&crate::NewEntry {
                    parent_id: Some(nested),
                    name: "résumé.TXT".into(),
                    relative_path: "資料/nested/résumé.TXT".into(),
                    kind: "file".into(),
                    size: 12,
                    modified_unix: None,
                    description: String::new(),
                    tags: String::new(),
                })?;
                w.insert(&crate::NewEntry {
                    parent_id: Some(branch),
                    name: "near.txt".into(),
                    relative_path: "資料/near.txt".into(),
                    kind: "file".into(),
                    size: 8,
                    modified_unix: None,
                    description: String::new(),
                    tags: String::new(),
                })?;
                w.insert(&crate::NewEntry {
                    parent_id: None,
                    name: "outside.txt".into(),
                    relative_path: "outside.txt".into(),
                    kind: "file".into(),
                    size: 20,
                    modified_unix: None,
                    description: String::new(),
                    tags: String::new(),
                })?;
                Ok(())
            })
            .unwrap();
        let query = format!("branch_id:{branch} AND ext:txt");
        let result = catalog.search(&query, None, 0, 20).unwrap();
        assert_eq!(result.total, 2);
        assert!(result.items.iter().any(|e| e.name == "résumé.TXT"));
        let only_nested = catalog
            .search(
                &format!("branch_id:{branch} AND name_exact:résumé.TXT"),
                None,
                0,
                20,
            )
            .unwrap();
        assert_eq!(only_nested.total, 1);
        assert_eq!(only_nested.items[0].relative_path, "資料/nested/résumé.TXT");
        assert_eq!(
            catalog
                .search(
                    &format!("branch_id:{branch} AND name_exact:資料"),
                    None,
                    0,
                    20
                )
                .unwrap()
                .total,
            0,
            "anchor folder must not match its own branch"
        );
        for invalid in ["0", "-4", "nope"] {
            assert!(compile(&format!("branch_id:{invalid}")).is_err());
        }
        drop(catalog);
    }

    #[test]
    fn category_id_predicates_match_primary_and_exact_assignment() {
        let dir = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::open(dir.path().join("categories.sqlite")).unwrap();
        let mut files = Vec::new();
        catalog
            .bulk_import("Disk", "offline", |w| {
                for name in ["multi.bin", "second.bin", "uncategorized.bin", "noext"] {
                    files.push(w.insert(&crate::NewEntry {
                        parent_id: None,
                        name: name.into(),
                        relative_path: name.into(),
                        kind: "file".into(),
                        size: 1,
                        modified_unix: None,
                        description: String::new(),
                        tags: String::new(),
                    })?);
                }
                Ok(())
            })
            .unwrap();
        let duplicate_a = catalog.create_category("Movies", None).unwrap();
        let duplicate_b = catalog.create_category("Movies", None).unwrap();
        catalog.assign_category(files[0], duplicate_a).unwrap();
        catalog.assign_category(files[0], duplicate_b).unwrap();
        catalog.assign_category(files[1], duplicate_b).unwrap();

        let primary_a = catalog
            .search(&format!("primary_category_id:{duplicate_a}"), None, 0, 10)
            .unwrap();
        assert_eq!(
            primary_a.items.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![files[0]]
        );
        let primary_b = catalog
            .search(&format!("primary_category_id:{duplicate_b}"), None, 0, 10)
            .unwrap();
        assert_eq!(
            primary_b.items.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![files[1]]
        );
        let exact_b = catalog
            .search(&format!("category_exact_id:{duplicate_b}"), None, 0, 10)
            .unwrap();
        assert_eq!(exact_b.total, 2);
        let uncategorized = catalog
            .search("primary_category_id:0", None, 0, 10)
            .unwrap();
        assert_eq!(uncategorized.total, 2);
        assert!(compile("primary_category_id:-1").is_err());
        assert!(compile("category_exact_id:0").is_err());

        let empty_ext = catalog.search("ext:\"\"", None, 0, 10).unwrap();
        assert_eq!(empty_ext.total, 1);
        assert!(compile("name:\"\"").is_err());
    }

    #[test]
    fn counted_extension_exclusion_uses_btree_count_and_index_seek() {
        let catalog = Catalog::open(":memory:").unwrap();
        for (query, scoped, use_complement) in [
            ("NOT ext:tmp", false, true),
            ("(NOT (ExT:\".ФЛАК\"))", false, true),
            ("NOT ext:tmp", true, false),
            ("NOT NOT ext:tmp", false, false),
            ("NOT ext:tmp AND ext:mkv", false, false),
            ("NOT ext:tmp OR category:Media", false, false),
            ("NOT (ext:tmp OR ext:mkv)", false, false),
        ] {
            let (mut predicate, mut args) = compile_predicate(query).unwrap();
            if scoped {
                predicate = predicate.within_volume();
                args.push(Value::Integer(1));
            }
            let sql = predicate.count_sql(catalog.query_table(query).unwrap());
            let opcodes: Vec<String> = catalog
                .conn()
                .prepare(&format!("EXPLAIN {sql}"))
                .unwrap()
                .query_map(params_from_iter(&args), |row| row.get(1))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            assert_eq!(
                opcodes.iter().any(|op| op == "Count"),
                use_complement,
                "{query}, scoped={scoped}"
            );
            if use_complement {
                let plan: Vec<String> = catalog
                    .conn()
                    .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                    .unwrap()
                    .query_map(params_from_iter(&args), |row| row.get(3))
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                assert!(
                    plan.iter()
                        .any(|step| step.contains("SEARCH") && step.contains("entries_ext")),
                    "{query}: {plan:?}"
                );
            }
        }
    }
}
