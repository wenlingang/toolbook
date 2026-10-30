//! A tool block's fence info string: ```` ```pick name=ts input=$jwt.payload.exp path="a b" ````.

/// One step of a path into a value: an object key or an array index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSeg {
    /// An object key.
    Key(String),
    /// An array index.
    Index(usize),
}

/// A reference to an earlier block's output, with a path into it: `$jwt.payload.exp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarRef {
    /// The block's name.
    pub name: String,
    /// The path into its output; empty for the whole output.
    pub path: Vec<PathSeg>,
}

impl VarRef {
    /// `$name.path` as written.
    pub fn display(&self) -> String {
        let mut s = format!("${}", self.name);
        s.push_str(&display_path(&self.path));
        s
    }
}

/// A path as written after a name: `.a.b[0]`.
pub fn display_path(path: &[PathSeg]) -> String {
    let mut s = String::new();
    for seg in path {
        match seg {
            PathSeg::Key(k) => {
                s.push('.');
                s.push_str(k);
            }
            PathSeg::Index(i) => s.push_str(&format!("[{i}]")),
        }
    }
    s
}

/// An attribute's value: a literal, or a reference to an earlier block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attr {
    /// A literal string.
    Lit(String),
    /// `$name.path`.
    Ref(VarRef),
}

/// A parsed fence info string.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FenceInfo {
    /// The tool, the first word.
    pub tool: String,
    /// `name=…`, taken out of the attributes.
    pub name: Option<String>,
    /// Words without `=` (`param`, `static`), in order.
    pub flags: Vec<String>,
    /// `key=value` pairs other than `name`, in order.
    pub attrs: Vec<(String, Attr)>,
    /// What could not be read (a malformed reference).
    pub errors: Vec<String>,
}

impl FenceInfo {
    /// The attribute `key`, if present.
    pub fn attr(&self, key: &str) -> Option<&Attr> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Whether the flag is present.
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }

    /// The references among the attributes.
    pub fn refs(&self) -> impl Iterator<Item = &VarRef> {
        self.attrs.iter().filter_map(|(_, a)| match a {
            Attr::Ref(r) => Some(r),
            Attr::Lit(_) => None,
        })
    }
}

/// Splits on whitespace; `"…"` keeps spaces, `\"` inside quotes is a quote.
/// Each word says whether it had quotes (a quoted value is a literal).
fn words(info: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    let mut chars = info.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any || !cur.is_empty() {
                    out.push((std::mem::take(&mut cur), any));
                    any = false;
                }
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push((cur, any));
    }
    out
}

/// Parses a path written after a name: `a.b[0].c` (a leading `.` is allowed).
/// A malformed path (`a[1`, `a[0]b`, `a..b`) is an error naming it.
pub fn parse_path(s: &str) -> Result<Vec<PathSeg>, String> {
    let bad = || format!("路径 {s} 写法有误，应如 a.b[0].c");
    let mut out = Vec::new();
    let body = s.strip_prefix('.').unwrap_or(s);
    if body.is_empty() {
        return Ok(out);
    }
    for part in body.split('.') {
        let (key, mut rest) = match part.find('[') {
            Some(i) => (&part[..i], &part[i..]),
            None => (part, ""),
        };
        if key.contains(']') || (key.is_empty() && rest.is_empty()) {
            return Err(bad());
        }
        if !key.is_empty() {
            out.push(PathSeg::Key(key.to_owned()));
        }
        while !rest.is_empty() {
            let inner_end = rest.strip_prefix('[').and_then(|r| r.find(']')).ok_or_else(bad)?;
            let inner = &rest[1..=inner_end];
            match inner.parse::<usize>() {
                Ok(n) => out.push(PathSeg::Index(n)),
                Err(_) if inner.len() >= 2 && inner.starts_with('"') && inner.ends_with('"') => {
                    out.push(PathSeg::Key(inner[1..inner.len() - 1].to_owned()))
                }
                Err(_) => return Err(bad()),
            }
            rest = &rest[inner_end + 2..];
        }
    }
    Ok(out)
}

/// Parses `$name.path` (without checking that it starts with `$`).
pub fn parse_ref(s: &str) -> Result<VarRef, String> {
    let body = s.strip_prefix('$').unwrap_or(s);
    let end = body.find(['.', '[']).unwrap_or(body.len());
    let path = parse_path(&body[end..]).map_err(|_| format!("引用 ${body} 写法有误，应如 $name.a.b[0]"))?;
    Ok(VarRef { name: body[..end].to_owned(), path })
}

/// Parses an info string.
pub fn parse_info(info: &str) -> FenceInfo {
    let mut words = words(info).into_iter();
    let mut out = FenceInfo {
        tool: words.next().map(|(w, _)| w).unwrap_or_default(),
        ..FenceInfo::default()
    };
    for (w, quoted) in words {
        match w.split_once('=') {
            Some((k, v)) if !k.is_empty() => {
                if k == "name" {
                    out.name = Some(v.to_owned());
                } else if !quoted && v.starts_with('$') && v.len() > 1 && !v.starts_with("$.") && !v.starts_with("$[") {
                    match parse_ref(v) {
                        Ok(r) => out.attrs.push((k.to_owned(), Attr::Ref(r))),
                        Err(e) => {
                            out.errors.push(e);
                            out.attrs.push((k.to_owned(), Attr::Lit(v.to_owned())));
                        }
                    }
                } else {
                    out.attrs.push((k.to_owned(), Attr::Lit(v.to_owned())));
                }
            }
            _ => out.flags.push(w),
        }
    }
    out
}

fn quote(v: &str) -> String {
    if v.is_empty() || v.chars().any(|c| c.is_whitespace() || c == '"') {
        format!("\"{}\"", v.replace('"', "\\\""))
    } else {
        v.to_owned()
    }
}

/// Writes an info string back: tool, name, attributes, flags.
pub fn format_info(info: &FenceInfo) -> String {
    let mut parts = vec![info.tool.clone()];
    if let Some(name) = &info.name {
        parts.push(format!("name={}", quote(name)));
    }
    for (k, v) in &info.attrs {
        let v = match v {
            Attr::Lit(s) if s.starts_with('$') => format!("\"{}\"", s.replace('"', "\\\"")),
            Attr::Lit(s) => quote(s),
            Attr::Ref(r) => r.display(),
        };
        parts.push(format!("{k}={v}"));
    }
    parts.extend(info.flags.iter().cloned());
    parts.join(" ")
}
