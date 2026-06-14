//! Query-DSL (v0.5.0, spec §3.8) — a pure parser + predicate (no Bevy) that
//! replaces the substring filter. Implicit-AND terms, leading `-` negates,
//! `key:value` predicates over `type`/`kind`/`host`/`sev`/`name`/`path`/`deg`/
//! `recent`, and bare words (substring on the node label). Malformed input
//! returns a `QueryError` (the filter UI renders it as a red chip).

/// Comparison operator (degree filter only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Gt,
    Lt,
    Ge,
    Le,
}

impl Op {
    fn test(self, a: u32, b: u32) -> bool {
        match self {
            Op::Gt => a > b,
            Op::Lt => a < b,
            Op::Ge => a >= b,
            Op::Le => a <= b,
        }
    }
}

/// A single predicate.
#[derive(Debug, Clone, PartialEq)]
pub enum Pred {
    Kind(String),
    Sev(String),
    Name(String),
    Path(String),
    Host(String),
    Deg(Op, u32),
    /// Active/glowing within the window (seconds). The duration is parsed; the
    /// recency signal available is "currently glowing" (see `NodeView::recent`).
    Recent(u64),
    /// Bare word → substring on the node label.
    Word(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    pub negate: bool,
    pub pred: Pred,
}

/// Parsed query — an implicit-AND of terms.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Query {
    pub terms: Vec<Term>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    pub message: String,
}

fn err(message: String) -> QueryError {
    QueryError { message }
}

/// The fields a predicate reads for one node (built by the caller from graph
/// state). Borrowed strings — no allocation per node.
#[derive(Debug, Clone, Default)]
pub struct NodeView<'a> {
    /// `process` | `file` | `user` | `socket` | `host` | `alert`.
    pub kind: &'a str,
    pub label: &'a str,
    pub name: &'a str,
    pub path: &'a str,
    pub host: &'a str,
    /// `low` | `med` | `high` | "".
    pub severity: &'a str,
    pub degree: u32,
    /// Active/glowing within the recency window.
    pub recent: bool,
}

const KINDS: [&str; 6] = ["process", "file", "user", "socket", "host", "alert"];

fn norm_sev(s: &str) -> Option<String> {
    match s.to_lowercase().as_str() {
        "low" => Some("low".into()),
        "med" | "medium" => Some("med".into()),
        "high" | "critical" => Some("high".into()),
        _ => None,
    }
}

/// Tokenize on whitespace, keeping `"double quoted"` spans together.
fn tokenize(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for c in input.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                cur.push(c);
            }
            c if c.is_whitespace() && !in_quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn strip_quotes(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

fn parse_deg(v: &str) -> Result<Pred, QueryError> {
    let (op, rest) = if let Some(r) = v.strip_prefix(">=") {
        (Op::Ge, r)
    } else if let Some(r) = v.strip_prefix("<=") {
        (Op::Le, r)
    } else if let Some(r) = v.strip_prefix('>') {
        (Op::Gt, r)
    } else if let Some(r) = v.strip_prefix('<') {
        (Op::Lt, r)
    } else {
        return Err(err("deg needs an operator (>, <, >=, <=)".into()));
    };
    let n: u32 = rest
        .trim()
        .parse()
        .map_err(|_| err(format!("deg value '{rest}' is not a number")))?;
    Ok(Pred::Deg(op, n))
}

fn parse_recent(v: &str) -> Result<Pred, QueryError> {
    let v = v.trim();
    let split = v.find(|c: char| !c.is_ascii_digit()).unwrap_or(v.len());
    let (num, unit) = v.split_at(split);
    let n: u64 = num
        .parse()
        .map_err(|_| err(format!("recent value '{v}' is not a number+unit")))?;
    let secs = match unit {
        "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        "d" => n * 86_400,
        "" => return Err(err("recent needs a unit (s, m, h, d)".into())),
        u => return Err(err(format!("recent unit '{u}' is invalid"))),
    };
    Ok(Pred::Recent(secs))
}

fn parse_term(body: &str) -> Result<Pred, QueryError> {
    if let Some((key, raw)) = body.split_once(':') {
        let value = strip_quotes(raw);
        match key {
            "type" | "kind" => {
                let mut v = value.to_lowercase();
                if v == "remotehost" {
                    v = "host".into();
                }
                if KINDS.contains(&v.as_str()) {
                    Ok(Pred::Kind(v))
                } else {
                    Err(err(format!("unknown type '{value}'")))
                }
            }
            "sev" => {
                let v =
                    norm_sev(&value).ok_or_else(|| err(format!("unknown severity '{value}'")))?;
                Ok(Pred::Sev(v))
            }
            "name" => Ok(Pred::Name(value.to_lowercase())),
            "path" => Ok(Pred::Path(value.to_lowercase())),
            "host" => Ok(Pred::Host(value.to_lowercase())),
            "deg" => parse_deg(&value),
            "recent" => parse_recent(&value),
            other => Err(err(format!("unknown key '{other}'"))),
        }
    } else if body.is_empty() {
        Err(err("empty term".into()))
    } else {
        Ok(Pred::Word(body.to_lowercase()))
    }
}

/// Parse a query string. `Ok(empty query)` for blank input (matches everything).
pub fn parse_query(input: &str) -> Result<Query, QueryError> {
    let mut terms = Vec::new();
    for tok in tokenize(input) {
        let (negate, body) = match tok.strip_prefix('-') {
            Some(rest) if !rest.is_empty() => (true, rest.to_string()),
            _ => (false, tok),
        };
        terms.push(Term {
            negate,
            pred: parse_term(&body)?,
        });
    }
    Ok(Query { terms })
}

impl Pred {
    fn eval(&self, n: &NodeView) -> bool {
        match self {
            Pred::Kind(k) => n.kind.eq_ignore_ascii_case(k),
            Pred::Sev(s) => norm_sev(n.severity).as_deref() == Some(s.as_str()),
            Pred::Name(s) => n.name.to_lowercase().contains(s),
            Pred::Path(s) => n.path.to_lowercase().contains(s),
            Pred::Host(s) => n.host.to_lowercase().contains(s),
            Pred::Deg(op, v) => op.test(n.degree, *v),
            Pred::Recent(_secs) => n.recent,
            Pred::Word(s) => n.label.to_lowercase().contains(s),
        }
    }
}

impl Query {
    /// Evaluate the AND-of-terms predicate (empty query → matches all).
    pub fn matches(&self, n: &NodeView) -> bool {
        self.terms.iter().all(|t| t.negate ^ t.pred.eval(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Query {
        parse_query(s).expect("valid query")
    }

    #[test]
    fn parses_valid_forms() {
        assert_eq!(
            p("type:process").terms[0].pred,
            Pred::Kind("process".into())
        );
        assert_eq!(
            p("kind:remotehost").terms[0].pred,
            Pred::Kind("host".into())
        );
        assert_eq!(p("sev:medium").terms[0].pred, Pred::Sev("med".into()));
        assert_eq!(p("deg:>5").terms[0].pred, Pred::Deg(Op::Gt, 5));
        assert_eq!(p("deg:<=3").terms[0].pred, Pred::Deg(Op::Le, 3));
        assert_eq!(p("recent:5m").terms[0].pred, Pred::Recent(300));
        assert_eq!(p("recent:2h").terms[0].pred, Pred::Recent(7200));
        assert_eq!(p("ssh").terms[0].pred, Pred::Word("ssh".into()));
        assert_eq!(p("name:\"a b\"").terms[0].pred, Pred::Name("a b".into()));
    }

    #[test]
    fn negation_and_implicit_and() {
        let q = p("type:process -name:bash deg:>2");
        assert_eq!(q.terms.len(), 3);
        assert!(q.terms[1].negate);
        assert_eq!(q.terms[1].pred, Pred::Name("bash".into()));
    }

    #[test]
    fn malformed_inputs_error() {
        assert!(parse_query("type:bogus").is_err());
        assert!(parse_query("deg:abc").is_err());
        assert!(parse_query("deg:5").is_err(), "deg needs an operator");
        assert!(parse_query("recent:5x").is_err());
        assert!(parse_query("recent:5").is_err(), "recent needs a unit");
        assert!(parse_query("foo:bar").is_err(), "unknown key");
        assert!(parse_query("sev:urgent").is_err());
    }

    fn view<'a>(kind: &'a str, label: &'a str, degree: u32) -> NodeView<'a> {
        NodeView {
            kind,
            label,
            degree,
            ..Default::default()
        }
    }

    #[test]
    fn predicate_hits_and_misses() {
        let proc = NodeView {
            kind: "process",
            label: "sshd (12)",
            name: "sshd",
            degree: 5,
            severity: "",
            recent: true,
            ..Default::default()
        };
        assert!(p("type:process").matches(&proc));
        assert!(!p("type:file").matches(&proc));
        assert!(p("deg:>3").matches(&proc));
        assert!(!p("deg:>9").matches(&proc));
        assert!(p("ssh").matches(&proc), "bare word substring on label");
        assert!(p("-type:file").matches(&proc), "negation");
        assert!(!p("-type:process").matches(&proc));
        assert!(p("recent:10m").matches(&proc), "glowing → recent");
        // AND: both must hold.
        assert!(p("type:process deg:>3").matches(&proc));
        assert!(!p("type:process deg:>9").matches(&proc));
    }

    #[test]
    fn alert_severity_predicate() {
        let alert = NodeView {
            kind: "alert",
            label: "⚠ portscan",
            severity: "high",
            ..Default::default()
        };
        assert!(p("type:alert sev:high").matches(&alert));
        assert!(!p("sev:low").matches(&alert));
    }

    #[test]
    fn empty_query_matches_all() {
        let q = parse_query("   ").expect("blank ok");
        assert!(q.terms.is_empty());
        assert!(q.matches(&view("file", "x", 0)));
    }
}
