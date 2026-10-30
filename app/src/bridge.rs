//! A note ⇄ the rich editor's document. Tool blocks become code blocks
//! whose `language` holds the whole info string (`pick name=ts input=$jwt`),
//! which nsh's Markdown writer puts back after the fence as it is.

use nsh_base::rich_editor::extensions::starter;
use nsh_base::rich_editor::formats::{DocFormat, MarkdownFormat, MarkdownNames};
use nsh_base::rich_editor::model::{Fragment, Node, Schema};
use serde_json::{Value, json};
use toolbook_core::doc::{Document, Segment, is_tool};
use toolbook_core::fence::parse_info;

/// The editor's schema: the starter schema.
pub fn schema() -> Schema {
    Schema::new(starter::schema_spec()).expect("the starter schema is valid")
}

fn markdown() -> MarkdownFormat {
    MarkdownFormat::new(MarkdownNames::default())
}

/// The `language` of a code block holding Markdown the editor cannot show
/// as it is (a table, HTML, a footnote): kept and saved verbatim.
pub const RAW: &str = "toolbook-raw";

/// Prose split at blank lines into Markdown blocks; a fenced code block
/// stays whole.
fn chunks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in text.split('\n') {
        let t = line.trim_start();
        let run = |c: char| t.chars().take_while(|x| *x == c).count();
        match fence {
            Some((c, n)) => {
                cur.push(line);
                if run(c) >= n && t.trim_start_matches(c).trim().is_empty() {
                    fence = None;
                }
                continue;
            }
            None => {
                for c in ['`', '~'] {
                    if run(c) >= 3 {
                        fence = Some((c, run(c)));
                    }
                }
            }
        }
        if line.trim().is_empty() && fence.is_none() {
            if !cur.is_empty() {
                out.push(cur.join("\n"));
                cur.clear();
            }
        } else {
            cur.push(line);
        }
    }
    if !cur.is_empty() {
        out.push(cur.join("\n"));
    }
    out
}

/// Whether a code block whose `language` is `info` is a tool block.
pub fn block_is_tool(info: &str, toolbook: bool) -> bool {
    is_tool(&parse_info(info), toolbook)
}

/// The text after the front matter, when `text` opens with it.
fn after_front_matter(text: &str) -> &str {
    if !text.starts_with("---") {
        return text;
    }
    let mut offset = 0;
    for (i, line) in text.split_inclusive('\n').enumerate() {
        offset += line.len();
        if i > 0 && line.trim_end() == "---" {
            return &text[offset..];
        }
    }
    text
}

/// The note as the editor's document JSON.
pub fn to_editor_json(doc: &Document, schema: &Schema) -> Value {
    let md = markdown();
    let mut content: Vec<Value> = Vec::new();
    for (i, seg) in doc.segments.iter().enumerate() {
        match seg {
            Segment::Raw(text) => {
                let text = if i == 0 && doc.front_matter.is_some() {
                    after_front_matter(text)
                } else {
                    text
                };
                for chunk in chunks(text) {
                    // A block the editor writes back as it was is shown as
                    // rich text; any other is kept as its original text.
                    let parsed = md.parse(schema, &chunk).ok().filter(|node| {
                        md.serialize(node)
                            .map(|back| back.trim() == chunk.trim())
                            .unwrap_or(false)
                    });
                    match parsed.as_ref().and_then(|n| {
                        n.to_json()
                            .get("content")
                            .and_then(Value::as_array)
                            .cloned()
                    }) {
                        Some(children) => content.extend(children),
                        None => content.push(json!({
                            "type": "codeBlock",
                            "attrs": { "language": RAW },
                            "content": [{ "type": "text", "text": chunk }],
                        })),
                    }
                }
            }
            Segment::Block(b) => {
                let mut block =
                    json!({ "type": "codeBlock", "attrs": { "language": b.info_str() } });
                if !b.content.is_empty() {
                    block["content"] = json!([{ "type": "text", "text": b.content }]);
                }
                content.push(block);
            }
        }
    }
    if content.is_empty() {
        content.push(json!({ "type": "paragraph" }));
    }
    json!({ "type": "doc", "content": content })
}

fn language(node: &Node) -> Option<&str> {
    (node.ty().name() == "codeBlock")
        .then(|| node.attrs().get("language").and_then(Value::as_str))
        .flatten()
}

/// The editor's document as the note's Markdown, front matter first:
/// original text as it was, a secret block without its value, every
/// other block as nsh's Markdown writer writes it.
pub fn to_markdown(front_matter: Option<&str>, doc: &Node) -> String {
    let md = markdown();
    let pieces: Vec<String> = doc
        .content()
        .iter()
        .map(|child| match language(child) {
            Some(RAW) => child.text_content(),
            // A secret's value is never written; an empty block is two fence lines.
            Some(info) if parse_info(info).tool == "secret" || child.text_content().is_empty() => {
                format!("```{info}\n```")
            }
            _ => md
                .serialize_fragment(&Fragment::from(child.clone()))
                .trim_end()
                .to_owned(),
        })
        .filter(|p| !p.is_empty())
        .collect();
    let body = pieces.join("\n\n");
    let mut out = String::new();
    if let Some(fm) = front_matter {
        out.push_str("---\n");
        out.push_str(fm);
        if !fm.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("---\n\n");
    }
    out.push_str(body.trim_end());
    out.push('\n');
    out
}
