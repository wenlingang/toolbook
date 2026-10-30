//! A note made to be shown: a self-contained HTML page of its flow, step
//! by step from input to output, and its Markdown with each block's result
//! after the block. Secrets are never shown.

use serde_json::Value as Json;

use crate::doc::{Block, Document, Segment, tool_label};
use crate::engine::{BlockResult, BlockStatus, RunReport};
use crate::template::template_refs;
use crate::value::{Table, Value};

/// Longest text a shared page shows of one output.
const MAX_CHARS: usize = 20_000;
/// Most rows a shared table shows.
const MAX_ROWS: usize = 200;

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

fn cut(s: &str) -> String {
    if s.chars().count() > MAX_CHARS {
        let mut c: String = s.chars().take(MAX_CHARS).collect();
        c.push_str("\n…（已截断）");
        c
    } else {
        s.to_owned()
    }
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

/// The prose of each raw segment, front matter left out.
fn prose(doc: &Document, i: usize, text: &str) -> String {
    if i == 0 && doc.front_matter.is_some() { after_front_matter(text).to_owned() } else { text.to_owned() }
}

fn title_of(doc: &Document, fallback: &str) -> String {
    for (i, seg) in doc.segments.iter().enumerate() {
        if let Segment::Raw(t) = seg {
            for line in prose(doc, i, t).lines() {
                if let Some(h) = line.strip_prefix("# ") {
                    return h.trim().to_owned();
                }
            }
        }
    }
    fallback.to_owned()
}

/// The earlier blocks a block reads, by name.
fn sources(b: &Block, earlier: &[String]) -> Vec<String> {
    let mut out: Vec<String> = b.info.refs().map(|r| r.name.clone()).collect();
    out.extend(template_refs(&b.content).into_iter().map(|r| r.name));
    if b.info.tool == "js" {
        for name in earlier {
            if b.content.contains(&format!("${name}")) {
                out.push(name.clone());
            }
        }
    }
    let mut seen = Vec::new();
    out.retain(|n| {
        let fresh = !seen.contains(n);
        seen.push(n.clone());
        fresh
    });
    out
}

fn table_html(t: &Table) -> String {
    let mut h = String::from("<div class=\"scroll\"><table><thead><tr>");
    for c in &t.columns {
        h.push_str(&format!("<th>{}</th>", esc(c)));
    }
    h.push_str("</tr></thead><tbody>");
    for row in t.rows.iter().take(MAX_ROWS) {
        h.push_str("<tr>");
        for cell in row {
            h.push_str(&format!("<td>{}</td>", esc(cell)));
        }
        h.push_str("</tr>");
    }
    h.push_str("</tbody></table></div>");
    if t.rows.len() > MAX_ROWS {
        h.push_str(&format!("<p class=\"muted\">还有 {} 行未显示</p>", t.rows.len() - MAX_ROWS));
    }
    h
}

/// An array of objects with the same keys, as a table.
fn json_table(j: &Json) -> Option<Table> {
    let items = j.as_array()?;
    let first = items.first()?.as_object()?;
    let columns: Vec<String> = first.keys().cloned().collect();
    let mut rows = Vec::new();
    for item in items {
        let o = item.as_object()?;
        if o.len() != columns.len() || !columns.iter().all(|c| o.contains_key(c)) {
            return None;
        }
        rows.push(
            columns
                .iter()
                .map(|c| match &o[c] {
                    Json::String(s) => s.clone(),
                    Json::Null => String::new(),
                    v => v.to_string(),
                })
                .collect(),
        );
    }
    Some(Table { columns, rows })
}

/// `text` as HTML, JSON colored by its tokens: keys, strings, numbers and
/// literals. Whatever is not JSON's goes through as it is.
fn json_html(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() * 2);
    let span = |out: &mut String, class: &str, s: &str| {
        out.push_str(&format!("<span class=\"{class}\">{}</span>", esc(s)));
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            i = (i + 1).min(chars.len());
            let token: String = chars[start..i].iter().collect();
            let key = chars[i..].iter().find(|c| !c.is_whitespace()) == Some(&':');
            span(&mut out, if key { "k" } else { "s" }, &token);
        } else if c == '-' || c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || "-+.eE".contains(chars[i])) {
                i += 1;
            }
            span(&mut out, "n", &chars[start..i].iter().collect::<String>());
        } else if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if matches!(word.as_str(), "true" | "false" | "null") {
                span(&mut out, "l", &word);
            } else {
                out.push_str(&esc(&word));
            }
        } else {
            out.push_str(&esc(&c.to_string()));
            i += 1;
        }
    }
    out
}

/// `text` as HTML, each `$name` of an earlier block marked: where the
/// data comes in.
fn refs_html(text: &str, names: &[String]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        out.push_str(&esc(&rest[..at]));
        let after = &rest[at + 1..];
        let len = after.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(after.len());
        if len > 0 && names.iter().any(|n| n == &after[..len]) {
            out.push_str(&format!("<span class=\"ref\">${}</span>", esc(&after[..len])));
        } else {
            out.push_str(&esc(&rest[at..at + 1 + len]));
        }
        rest = &after[len..];
    }
    out.push_str(&esc(rest));
    out
}

/// A block's input or a text result: JSON colored when it is JSON.
fn code_html(text: &str, names: &[String]) -> String {
    let text = cut(text);
    let is_json = serde_json::from_str::<Json>(&text).is_ok_and(|j| j.is_object() || j.is_array());
    if is_json { json_html(&text) } else { refs_html(&text, names) }
}

fn output_html(r: &BlockResult) -> String {
    match &r.status {
        BlockStatus::Ok(_) if r.tool == "secret" => "<p class=\"quiet\">已读取密钥（不显示）</p>".into(),
        BlockStatus::Ok(Value::Text(t)) => format!("<pre class=\"out\">{}</pre>", code_html(t, &[])),
        BlockStatus::Ok(Value::Table(t)) => table_html(t),
        BlockStatus::Ok(Value::Json(j)) => match json_table(j) {
            Some(t) => table_html(&t),
            None => format!(
                "<pre class=\"out\">{}</pre>",
                json_html(&cut(&serde_json::to_string_pretty(j).unwrap_or_default()))
            ),
        },
        BlockStatus::Error(m) => format!("<p class=\"error\">出错：{}</p>", esc(m)),
        BlockStatus::Upstream { block, name } => {
            format!("<p class=\"error\">上游第 {} 步 ${} 出错</p>", block + 1, esc(name))
        }
        BlockStatus::Incomplete(m) => format!("<p class=\"quiet\">{}</p>", esc(m)),
        BlockStatus::NotRun => "<p class=\"quiet\">未运行（HTTP 请求需要手动运行）</p>".into(),
    }
}

const STYLE: &str = r#"
:root{--bg:#fff;--fg:#1b2130;--muted:#677184;--rule:#e4e7ee;--accent:#3b5bff;--rail:#c9d2ff;--in:#f4f6fa;--panel:#171c28;--panel-fg:#d9dfeb;--k:#9db4ff;--s:#a8d5a2;--n:#f3b86b;--l:#ee8f8f;--error:#d92d20}
@media (prefers-color-scheme:dark){:root{--bg:#0e121a;--fg:#e1e6ef;--muted:#8792a6;--rule:#232b3a;--accent:#7f96ff;--rail:#2f3a66;--in:#151b26;--panel:#0a0d14;--panel-fg:#d9dfeb;--error:#ff6b6b}}
*{box-sizing:border-box}
html{background:var(--bg)}
body{margin:0;color:var(--fg);font:16px/1.8 -apple-system,"PingFang SC","Hiragino Sans GB","Microsoft YaHei",sans-serif;-webkit-font-smoothing:antialiased}
main{max-width:760px;margin:0 auto;padding:56px 24px 72px}
h1{font-size:32px;line-height:1.25;font-weight:700;letter-spacing:-.01em;margin:0 0 16px}
h2{font-size:22px;line-height:1.35;margin:40px 0 8px}
h3{font-size:18px;margin:28px 0 6px}
p{margin:10px 0}
ul,ol{padding-left:22px}
a{color:var(--accent)}
code,pre{font-family:"SF Mono",Menlo,monospace;font-size:13px;line-height:1.65}
p code,li code{background:var(--in);padding:1px 5px;border-radius:4px}
pre{margin:0;padding:14px 16px;border-radius:8px;overflow-x:auto;white-space:pre-wrap;word-break:break-all}
.step{position:relative;padding-left:44px;margin:28px 0 0}
.step::before{content:"";position:absolute;left:13px;top:30px;bottom:-4px;width:2px;background:var(--rail)}
.step:has(+ .step)::before{bottom:-30px}
.node{position:absolute;left:0;top:2px;width:28px;height:28px;border-radius:50%;background:var(--accent);color:var(--bg);font:600 13px/28px -apple-system,sans-serif;text-align:center}
.step header{display:flex;flex-wrap:wrap;align-items:baseline;gap:4px 12px;min-height:32px;margin-bottom:10px}
.tool{font-weight:600;font-size:16px}
.io{font-family:"SF Mono",Menlo,monospace;font-size:13px;color:var(--muted)}
.ref,.io b{color:var(--accent);font-weight:600}
pre.in{background:var(--in);color:var(--fg);margin-bottom:10px}
pre.in .k{color:#3550d9}pre.in .s{color:#2f7d32}pre.in .n{color:#b35c00}pre.in .l{color:#c2410c}
@media (prefers-color-scheme:dark){pre.in .k{color:var(--k)}pre.in .s{color:var(--s)}pre.in .n{color:var(--n)}pre.in .l{color:var(--l)}}
pre.out{background:var(--panel);color:var(--panel-fg)}
pre.out .k{color:var(--k)}pre.out .s{color:var(--s)}pre.out .n{color:var(--n)}pre.out .l{color:var(--l)}
.scroll{overflow-x:auto;border-top:2px solid var(--fg)}
table{border-collapse:collapse;font-size:14px;width:100%}
th,td{border-bottom:1px solid var(--rule);padding:7px 12px 7px 0;text-align:left;white-space:nowrap}
th{font-weight:600}
td{font-variant-numeric:tabular-nums}
.quiet,.muted{color:var(--muted)}
.error{color:var(--error)}
ul.notes{margin:10px 0 0;padding-left:18px;color:var(--muted);font-size:14px}
footer{margin-top:56px;padding-top:16px;border-top:1px solid var(--rule);color:var(--muted);font-size:13px}
"#;

/// `url` when following it opens a page or a mail and runs nothing (http,
/// https, mailto, a place in the page, a relative path); `#` otherwise.
fn safe_url(url: pulldown_cmark::CowStr<'_>) -> pulldown_cmark::CowStr<'_> {
    let scheme = url.split_once(':').map(|(s, _)| s).filter(|s| !s.contains(['/', '?', '#']));
    match scheme.map(str::to_ascii_lowercase).as_deref() {
        None | Some("http" | "https" | "mailto") => url,
        Some(_) => "#".into(),
    }
}

/// What a shared copy keeps back of the note's secrets.
struct Secrets {
    /// For each block: whether it reads a secret, directly or through the
    /// blocks it reads (a secret block itself shows no value anyway). A
    /// hash or an HMAC gives nothing of its input back: shown, and what
    /// reads it is too.
    hidden: Vec<bool>,
    /// The secrets' values, blanked out wherever they still turn up (an
    /// error message quoting its input).
    values: Vec<String>,
}

const HIDDEN: &str = "用到了密钥，结果不显示";

impl Secrets {
    fn of(doc: &Document, report: &RunReport) -> Self {
        let mut names: Vec<String> = Vec::new();
        let mut tainted: Vec<String> = Vec::new();
        let mut hidden = Vec::new();
        let mut values = Vec::new();
        for ((_, b), r) in doc.blocks().zip(&report.blocks) {
            let secret = b.info.tool == "secret";
            let one_way = matches!(b.info.tool.as_str(), "hash" | "hmac");
            // A flow whose note read a secret carries it in.
            let carries = !r.secrets.is_empty();
            values.extend(r.secrets.iter().cloned());
            let reads = carries || (!one_way && sources(b, &names).iter().any(|n| tainted.contains(n)));
            hidden.push(reads && !secret);
            if secret && let BlockStatus::Ok(Value::Text(v)) = &r.status && !v.is_empty() {
                values.push(v.clone());
            }
            if let Some(n) = &b.info.name {
                if secret || reads {
                    tainted.push(n.clone());
                }
                names.push(n.clone());
            }
        }
        Self { hidden, values }
    }

    fn hides(&self, step: usize) -> bool {
        self.hidden.get(step).copied().unwrap_or(false)
    }

    /// `out` with every secret's value, as written and as HTML, blanked out.
    fn scrub(&self, mut out: String) -> String {
        for v in &self.values {
            out = out.replace(v.as_str(), "••••••").replace(&esc(v), "••••••");
        }
        out
    }
}

/// The note as a page to share: its prose, then each tool block as a step
/// on a line that joins the steps one after another, showing what it reads,
/// what it was given and what it gave.
pub fn to_html(doc: &Document, report: &RunReport, fallback_title: &str) -> String {
    let title = title_of(doc, fallback_title);
    let secrets = Secrets::of(doc, report);
    let mut body = String::new();
    let mut names: Vec<String> = Vec::new();
    let mut steps = 0;
    for (i, seg) in doc.segments.iter().enumerate() {
        match seg {
            Segment::Raw(text) => {
                let md = prose(doc, i, text);
                let parser = pulldown_cmark::Parser::new_ext(&md, pulldown_cmark::Options::ENABLE_TABLES);
                let parser = parser.map(|e| match e {
                    pulldown_cmark::Event::Html(h) | pulldown_cmark::Event::InlineHtml(h) => {
                        pulldown_cmark::Event::Text(h)
                    }
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { link_type, dest_url, title, id }) => {
                        pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { link_type, dest_url: safe_url(dest_url), title, id })
                    }
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::Image { link_type, dest_url, title, id }) => {
                        pulldown_cmark::Event::Start(pulldown_cmark::Tag::Image { link_type, dest_url: safe_url(dest_url), title, id })
                    }
                    e => e,
                });
                pulldown_cmark::html::push_html(&mut body, parser);
            }
            Segment::Block(b) => {
                let Some(r) = report.blocks.get(steps) else { continue };
                steps += 1;
                let from = sources(b, &names);
                let vars = |list: &[String]| list.iter().map(|n| format!("<b>${}</b>", esc(n))).collect::<Vec<_>>().join("、");
                body.push_str(&format!(
                    "<section class=\"step\"><span class=\"node\">{steps}</span><header><span class=\"tool\">{}</span>",
                    esc(tool_label(&b.info.tool))
                ));
                let out = b.info.name.as_ref().map(|n| format!("<b>${}</b>", esc(n)));
                match (from.is_empty(), &out) {
                    (false, Some(o)) => body.push_str(&format!("<span class=\"io\">{} → {o}</span>", vars(&from))),
                    (false, None) => body.push_str(&format!("<span class=\"io\">{} →</span>", vars(&from))),
                    (true, Some(o)) => body.push_str(&format!("<span class=\"io\">→ {o}</span>")),
                    (true, None) => {}
                }
                body.push_str("</header>");
                if b.info.tool != "secret" && !b.content.trim().is_empty() {
                    body.push_str(&format!("<pre class=\"in\">{}</pre>", code_html(&b.content, &names)));
                }
                if secrets.hides(steps - 1) {
                    body.push_str(&format!("<p class=\"quiet\">{HIDDEN}</p>"));
                } else {
                    body.push_str(&output_html(r));
                }
                if !r.notes.is_empty() && !secrets.hides(steps - 1) {
                    body.push_str("<ul class=\"notes\">");
                    for n in &r.notes {
                        body.push_str(&format!("<li>{}</li>", esc(n)));
                    }
                    body.push_str("</ul>");
                }
                body.push_str("</section>");
                if let Some(n) = &b.info.name {
                    names.push(n.clone());
                }
            }
        }
    }
    secrets.scrub(format!(
        "<!doctype html>\n<html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src http: https:\"><title>{}</title><style>{STYLE}</style></head><body><main>{body}<footer>由 Toolbook 生成，共 {steps} 步</footer></main></body></html>\n",
        esc(&title)
    ))
}

fn result_markdown(r: &BlockResult) -> String {
    let fenced = |lang: &str, s: &str| format!("> 结果\n\n```{lang}\n{}\n```\n", cut(s).trim_end());
    match &r.status {
        BlockStatus::Ok(_) if r.tool == "secret" => "> 结果：已读取密钥（不显示）\n".into(),
        BlockStatus::Ok(Value::Text(t)) => fenced("text", t),
        BlockStatus::Ok(Value::Json(j)) => fenced("json", &serde_json::to_string_pretty(j).unwrap_or_default()),
        BlockStatus::Ok(v @ Value::Table(_)) => fenced("csv", &v.render_plain()),
        BlockStatus::Error(m) => format!("> 出错：{m}\n"),
        BlockStatus::Upstream { block, name } => format!("> 上游 [{}] ${name} 出错\n", block + 1),
        BlockStatus::Incomplete(m) => format!("> {m}\n"),
        BlockStatus::NotRun => "> 未运行（HTTP 请求需要手动运行）\n".into(),
    }
}

/// The note's Markdown without its front matter, each tool block followed
/// by its result: a copy to paste where Toolbook is not.
pub fn markdown_with_results(doc: &Document, report: &RunReport) -> String {
    let secrets = Secrets::of(doc, report);
    let mut out = String::new();
    let mut steps = 0;
    for (i, seg) in doc.segments.iter().enumerate() {
        match seg {
            Segment::Raw(text) => out.push_str(&prose(doc, i, text)),
            Segment::Block(b) => {
                out.push_str(&b.raw);
                if !b.raw.ends_with('\n') {
                    out.push('\n');
                }
                if let Some(r) = report.blocks.get(steps) {
                    out.push('\n');
                    if secrets.hides(steps) {
                        out.push_str(&format!("> 结果：{HIDDEN}\n"));
                    } else {
                        out.push_str(&result_markdown(r));
                    }
                }
                steps += 1;
            }
        }
    }
    secrets.scrub(out.trim_start().to_owned())
}
