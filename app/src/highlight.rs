//! Colors for the Markdown view: headings, fences, a tool block's tool,
//! settings and references. Columns count UTF-16 units, as the editor's do.
//! Each class names a theme token (`text-syntax-variable`), so the colors
//! follow the theme.

use nsh_base::{EditorDecoration, EditorPosition, EditorRange};

/// A colored run of one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// The line, from 0.
    pub line: u32,
    /// Its first column.
    pub start: u32,
    /// The column after it.
    pub end: u32,
    /// Its Tailwind class.
    pub class: &'static str,
}

const KEYWORD: &str = "text-syntax-keyword";
const STRING: &str = "text-syntax-string";
const NUMBER: &str = "text-syntax-number";
const VARIABLE: &str = "text-syntax-variable";
const COMMENT: &str = "text-syntax-comment";
const TOOL: &str = "text-syntax-tool";
const ATTR: &str = "text-syntax-attr";

fn u16len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// An opening or closing fence: its indent, character and length.
fn fence(line: &str) -> Option<(usize, char, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let ch = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = rest.chars().take_while(|c| *c == ch).count();
    (n >= 3).then_some((indent, ch, n))
}

/// The words of an info string with their byte offsets; quotes kept.
fn words(info: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    let mut quoted = false;
    for (i, c) in info.char_indices() {
        match c {
            '"' => {
                quoted = !quoted;
                start.get_or_insert(i);
            }
            c if c.is_whitespace() && !quoted => {
                if let Some(s) = start.take() {
                    out.push((s, &info[s..i]));
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(s) = start {
        out.push((s, &info[s..]));
    }
    out
}

fn push(out: &mut Vec<Span>, line: u32, text: &str, from: usize, to: usize, class: &'static str) {
    if to > from {
        out.push(Span {
            line,
            start: u16len(&text[..from]),
            end: u16len(&text[..to]),
            class,
        });
    }
}

fn info_spans(out: &mut Vec<Span>, line: u32, text: &str, at: usize) {
    for (k, (off, word)) in words(&text[at..]).into_iter().enumerate() {
        let off = at + off;
        if k == 0 {
            push(out, line, text, off, off + word.len(), TOOL);
            continue;
        }
        let Some(eq) = word.find('=') else {
            push(out, line, text, off, off + word.len(), ATTR);
            continue;
        };
        push(out, line, text, off, off + eq + 1, ATTR);
        let v = &word[eq + 1..];
        let v_at = off + eq + 1;
        let class = if v.starts_with('"') {
            STRING
        } else if v.starts_with('$') && !v.starts_with("$.") && !v.starts_with("$[") {
            VARIABLE
        } else if v.parse::<f64>().is_ok() {
            NUMBER
        } else {
            STRING
        };
        push(out, line, text, v_at, v_at + v.len(), class);
    }
}

fn template_spans(out: &mut Vec<Span>, line: u32, text: &str) {
    let mut from = 0;
    while let Some(s) = text[from..].find("{{") {
        let s = from + s;
        let Some(e) = text[s..].find("}}") else { break };
        let e = s + e + 2;
        push(out, line, text, s, e, VARIABLE);
        from = e;
    }
}

/// The colored runs of `src`.
pub fn spans(src: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut open: Option<(char, usize)> = None;
    for (i, raw) in src.split('\n').enumerate() {
        let line = i as u32;
        let text = raw.strip_suffix('\r').unwrap_or(raw);
        match (open, fence(text)) {
            (Some((ch, n)), Some((ind, c, m)))
                if c == ch && m >= n && text[ind + m..].trim().is_empty() =>
            {
                push(&mut out, line, text, ind, ind + m, COMMENT);
                open = None;
            }
            (Some(_), _) => template_spans(&mut out, line, text),
            (None, Some((ind, c, m))) => {
                push(&mut out, line, text, ind, ind + m, COMMENT);
                info_spans(&mut out, line, text, ind + m);
                open = Some((c, m));
            }
            (None, None) if text.starts_with('#') => {
                push(&mut out, line, text, 0, text.len(), KEYWORD)
            }
            (None, None) => {}
        }
    }
    out
}

/// [`spans`] as the editor's decorations.
pub fn decorations(src: &str) -> Vec<EditorDecoration> {
    spans(src)
        .into_iter()
        .map(|s| {
            let mut d = EditorDecoration::default();
            d.range = EditorRange::new(
                EditorPosition::new(s.line, s.start),
                EditorPosition::new(s.line, s.end),
            );
            d.class = Some(s.class.into());
            d
        })
        .collect()
}
