//! A note: Markdown kept as written, with its tool blocks picked out.
//!
//! The source is split into segments whose concatenation is the source,
//! byte for byte; only an edited block is written anew.

use crate::fence::{FenceInfo, parse_info};

/// The tools v1 knows (spec §6).
pub const TOOL_NAMES: &[&str] = &[
    "text", "http", "json", "pick", "filter", "map", "csv", "to-json", "to-sql", "base64", "url",
    "jwt-decode", "hash", "hmac", "timestamp", "regex", "diff", "js", "flow", "secret",
];

/// The tools as people call them, for menus and shared pages.
pub const TOOL_LABELS: &[(&str, &str)] = &[
    ("text", "文本"),
    ("http", "HTTP 请求"),
    ("json", "JSON 格式化"),
    ("pick", "取字段"),
    ("filter", "筛选"),
    ("map", "字段映射"),
    ("csv", "CSV → 表格"),
    ("to-json", "表格 → JSON"),
    ("to-sql", "生成 SQL"),
    ("base64", "Base64"),
    ("url", "URL 编解码"),
    ("jwt-decode", "JWT 解码"),
    ("hash", "Hash"),
    ("hmac", "HMAC 签名"),
    ("timestamp", "时间戳"),
    ("regex", "正则"),
    ("diff", "Diff 对比"),
    ("js", "JS 表达式"),
    ("flow", "调用工具流"),
    ("secret", "密钥"),
];

/// What people call `tool` (the tool's own name when it has no label).
pub fn tool_label(tool: &str) -> &str {
    TOOL_LABELS.iter().find(|(t, _)| *t == tool).map(|(_, l)| *l).unwrap_or(tool)
}

/// A tool block: a fenced code block whose info names a tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The parsed info string.
    pub info: FenceInfo,
    /// The text between the fences, without the last line break.
    pub content: String,
    /// The whole block as written, fences included.
    pub raw: String,
    /// The opening fence's characters (```` ``` ````, `~~~~`).
    pub fence: String,
    open_line: String,
    close_line: String,
}

impl Block {
    /// The info string as written after the opening fence.
    pub fn info_str(&self) -> &str {
        let ch = self.fence.chars().next().unwrap_or('`');
        strip_eol(&self.open_line).trim_start().trim_start_matches(ch).trim()
    }

    fn line_end(&self) -> &str {
        if self.open_line.ends_with("\r\n") { "\r\n" } else { "\n" }
    }

    fn rebuild(&mut self) {
        let eol = self.line_end().to_owned();
        let ch = self.fence.chars().next().unwrap_or('`');
        // A longer fence when the content holds a line that would close it.
        let longest = self
            .content
            .lines()
            .map(|l| l.trim_start().chars().take_while(|c| *c == ch).count())
            .max()
            .unwrap_or(0);
        if longest >= self.fence.chars().count() {
            let fence: String = std::iter::repeat_n(ch, longest + 1).collect();
            let info_part = self.open_line.trim_start().trim_start_matches(ch).to_owned();
            self.open_line = format!("{fence}{info_part}");
            let close_eol = if self.close_line.ends_with('\n') { eol.as_str() } else { "" };
            self.close_line = format!("{fence}{close_eol}");
            self.fence = fence;
        }
        let mut raw = self.open_line.clone();
        if !self.content.is_empty() {
            raw.push_str(&self.content);
            raw.push_str(&eol);
        }
        raw.push_str(&self.close_line);
        self.raw = raw;
    }
}

/// A run of the source: text kept as written, or a tool block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Text as written (prose, front matter, code that is not a tool).
    Raw(String),
    /// A tool block.
    Block(Block),
}

/// A parsed note.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    /// The front matter's text, between its `---` lines.
    pub front_matter: Option<String>,
    /// Whether the front matter says `toolbook: 1`.
    pub is_toolbook: bool,
    /// The source, in order.
    pub segments: Vec<Segment>,
}

fn strip_eol(line: &str) -> &str {
    line.strip_suffix('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .unwrap_or(line)
}

/// An opening fence: its characters and the info string after them.
fn open_fence(line: &str) -> Option<(char, usize, &str)> {
    let body = strip_eol(line);
    let indent = body.len() - body.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let body = &body[indent..];
    let ch = body.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let n = body.chars().take_while(|c| *c == ch).count();
    if n < 3 {
        return None;
    }
    let info = body[n..].trim();
    if ch == '`' && info.contains('`') {
        return None;
    }
    Some((ch, n, info))
}

fn closes(line: &str, ch: char, n: usize) -> bool {
    let body = strip_eol(line);
    let indent = body.len() - body.trim_start_matches(' ').len();
    if indent > 3 {
        return false;
    }
    let body = &body[indent..];
    let m = body.chars().take_while(|c| *c == ch).count();
    m >= n && body[m..].trim().is_empty()
}

/// Whether a fence with `info` is a tool block, in a note marked `toolbook: 1` or not (spec §7).
pub fn is_tool(info: &FenceInfo, toolbook: bool) -> bool {
    if info.has_flag("static") || info.tool.is_empty() {
        return false;
    }
    let marked = info.name.is_some() || info.has_flag("param") || info.refs().next().is_some();
    if toolbook {
        TOOL_NAMES.contains(&info.tool.as_str()) || marked
    } else {
        marked
    }
}

impl Document {
    /// Parses a note.
    pub fn parse(src: &str) -> Document {
        let lines: Vec<&str> = src.split_inclusive('\n').collect();
        let mut doc = Document::default();
        let mut raw = String::new();
        let mut i = 0;

        if lines.first().is_some_and(|l| strip_eol(l) == "---")
            && let Some(end) = lines.iter().skip(1).position(|l| strip_eol(l) == "---")
        {
            let end = end + 1;
            let fm: String = lines[1..end].concat();
            doc.is_toolbook = fm.lines().any(|l| {
                let l = l.trim();
                l == "toolbook: 1" || l == "toolbook: true"
            });
            doc.front_matter = Some(fm);
            for l in &lines[..=end] {
                raw.push_str(l);
            }
            i = end + 1;
        }

        while i < lines.len() {
            let line = lines[i];
            let Some((ch, n, info_str)) = open_fence(line) else {
                raw.push_str(line);
                i += 1;
                continue;
            };
            let Some(close) = (i + 1..lines.len()).find(|&j| closes(lines[j], ch, n)) else {
                // Unclosed: the rest of the file is text.
                for l in &lines[i..] {
                    raw.push_str(l);
                }
                break;
            };
            let block_text: String = lines[i..=close].concat();
            let info = parse_info(info_str);
            if is_tool(&info, doc.is_toolbook) {
                if !raw.is_empty() {
                    doc.segments.push(Segment::Raw(std::mem::take(&mut raw)));
                }
                let inner: String = lines[i + 1..close].concat();
                let content = inner
                    .strip_suffix('\n')
                    .map(|c| c.strip_suffix('\r').unwrap_or(c))
                    .unwrap_or(&inner)
                    .to_owned();
                doc.segments.push(Segment::Block(Block {
                    info,
                    content,
                    raw: block_text,
                    fence: std::iter::repeat_n(ch, n).collect(),
                    open_line: line.to_owned(),
                    close_line: lines[close].to_owned(),
                }));
            } else {
                raw.push_str(&block_text);
            }
            i = close + 1;
        }
        if !raw.is_empty() {
            doc.segments.push(Segment::Raw(raw));
        }
        doc
    }

    /// The note as Markdown: the source as written, edited blocks anew.
    pub fn serialize(&self) -> String {
        self.segments
            .iter()
            .map(|s| match s {
                Segment::Raw(r) => r.as_str(),
                Segment::Block(b) => b.raw.as_str(),
            })
            .collect()
    }

    /// The tool blocks with their index (0-based, in order).
    pub fn blocks(&self) -> impl Iterator<Item = (usize, &Block)> {
        self.segments
            .iter()
            .filter_map(|s| match s {
                Segment::Block(b) => Some(b),
                Segment::Raw(_) => None,
            })
            .enumerate()
    }

    /// Replaces block `idx`'s content; the rest of the source stays as written.
    pub fn set_block_content(&mut self, idx: usize, content: &str) {
        if let Some(b) = self
            .segments
            .iter_mut()
            .filter_map(|s| match s {
                Segment::Block(b) => Some(b),
                Segment::Raw(_) => None,
            })
            .nth(idx)
        {
            b.content = content.to_owned();
            b.rebuild();
        }
    }

    /// The first paragraph of prose (not a heading, not code), as plain text.
    pub fn description(&self) -> Option<String> {
        let mut para: Vec<String> = Vec::new();
        let mut fence: Option<(char, usize)> = None;
        let mut skip_front = self.front_matter.is_some();
        for seg in &self.segments {
            let Segment::Raw(text) = seg else {
                if !para.is_empty() {
                    break;
                }
                continue;
            };
            let mut lines = text.split_inclusive('\n').peekable();
            if skip_front {
                // The front matter opens the first raw segment.
                lines.next();
                for l in lines.by_ref() {
                    if strip_eol(l) == "---" {
                        break;
                    }
                }
                skip_front = false;
            }
            for l in lines {
                if let Some((ch, n)) = fence {
                    if closes(l, ch, n) {
                        fence = None;
                    }
                    continue;
                }
                if let Some((ch, n, _)) = open_fence(l) {
                    if !para.is_empty() {
                        return Some(para.join(" "));
                    }
                    fence = Some((ch, n));
                    continue;
                }
                let t = strip_eol(l).trim();
                if t.is_empty() {
                    if !para.is_empty() {
                        return Some(para.join(" "));
                    }
                } else if !t.starts_with('#') {
                    para.push(t.to_owned());
                }
            }
        }
        (!para.is_empty()).then(|| para.join(" "))
    }
}
