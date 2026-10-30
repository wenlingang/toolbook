//! `{{ name.path }}` in the content of template blocks (`http`, `to-sql`).

use crate::fence::{VarRef, parse_ref};
use crate::value::Value;

/// The pieces of a template: literal text and references.
enum Piece<'a> {
    Lit(&'a str),
    Ref(VarRef),
}

const LITERAL_OPEN: &str = "{{\"{{\"}}";

fn pieces(s: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        if rest[start..].starts_with(LITERAL_OPEN) {
            out.push(Piece::Lit(&rest[..start]));
            out.push(Piece::Lit("{{"));
            rest = &rest[start + LITERAL_OPEN.len()..];
            continue;
        }
        let Some(end) = rest[start..].find("}}") else { break };
        let inner = rest[start + 2..start + end].trim();
        match parse_ref(inner) {
            Ok(r) if !inner.is_empty() && !inner.contains(char::is_whitespace) => {
                out.push(Piece::Lit(&rest[..start]));
                out.push(Piece::Ref(r));
            }
            _ => out.push(Piece::Lit(&rest[..start + end + 2])),
        }
        rest = &rest[start + end + 2..];
    }
    out.push(Piece::Lit(rest));
    out
}

/// The references a template makes, in order.
pub fn template_refs(s: &str) -> Vec<VarRef> {
    pieces(s)
        .into_iter()
        .filter_map(|p| match p {
            Piece::Ref(r) => Some(r),
            Piece::Lit(_) => None,
        })
        .collect()
}

/// Replaces each reference with its value: text as written, JSON compact.
pub fn render_template(
    s: &str,
    lookup: &dyn Fn(&VarRef) -> Result<Value, String>,
) -> Result<String, String> {
    let mut out = String::new();
    for p in pieces(s) {
        match p {
            Piece::Lit(l) => out.push_str(l),
            Piece::Ref(r) => match lookup(&r)? {
                Value::Text(t) => out.push_str(&t),
                other => out.push_str(&other.to_json().to_string()),
            },
        }
    }
    Ok(out)
}
