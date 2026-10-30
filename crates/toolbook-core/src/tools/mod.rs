//! The built-in tools and what they run with.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::doc::Block;
use crate::engine::RunOptions;
use crate::value::Value;

mod base64;
mod csv;
mod diff;
mod flow;
mod http;
mod js;
mod regex;
mod timestamp;
mod filter;
mod json;
mod map;
mod pick;
mod rows;
mod to_json;
mod to_sql;
mod hash;
mod hmac;
mod jwt;
mod secret;
mod text;
mod url;

/// Why a tool gave no value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    /// The input is missing or not finished yet (half a Base64 string).
    Incomplete(String),
    /// The input is wrong.
    Failed(String),
}

/// What a tool runs with.
pub struct ToolCtx<'a> {
    /// The block.
    pub block: &'a Block,
    /// The content the tool reads: the block's, a `param` override, or a
    /// rendered template.
    pub content: String,
    /// `input=` when given, else the content when not empty.
    pub primary: Option<Value>,
    /// Every attribute, resolved (literals as text).
    pub attrs: BTreeMap<String, Value>,
    /// For `js`: the earlier named blocks its code mentions.
    pub vars: BTreeMap<String, Value>,
    /// The run's options.
    pub opts: &'a RunOptions,
    /// The directory of the note being run.
    pub base_dir: &'a Path,
    /// The notes being run, outermost first (for `flow`).
    pub stack: &'a [PathBuf],
    pub(crate) notes: RefCell<Vec<String>>,
    /// Secrets' values the result carries from elsewhere (`flow`).
    pub(crate) secrets: RefCell<Vec<String>>,
}

impl ToolCtx<'_> {
    /// Adds a remark to the block's result (rows dropped, and the like).
    pub fn note(&self, s: String) {
        self.notes.borrow_mut().push(s);
    }

    /// The attribute as text: text as written, JSON compact.
    pub fn attr_text(&self, key: &str) -> Option<String> {
        self.attrs.get(key).map(|v| match v {
            Value::Text(s) => s.clone(),
            other => other.to_json().to_string(),
        })
    }

    /// The primary input, or "输入为空".
    pub fn require_primary(&self) -> Result<&Value, ToolError> {
        self.primary
            .as_ref()
            .ok_or_else(|| ToolError::Incomplete("输入为空".into()))
    }

    /// The primary input as text (JSON compact).
    pub fn primary_text(&self) -> Result<String, ToolError> {
        Ok(match self.require_primary()? {
            Value::Text(s) => s.clone(),
            other => other.to_json().to_string(),
        })
    }
}

/// A built-in tool.
pub trait Tool: Send + Sync {
    /// Computes the block's value.
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError>;
    /// Whether the tool waits to be run by hand in the app (`http`).
    fn manual(&self) -> bool {
        false
    }
    /// Whether `{{ name }}` in its content is replaced.
    fn template(&self) -> bool {
        false
    }
}

/// The tool named `name`.
pub fn tool(name: &str) -> Option<&'static dyn Tool> {
    Some(match name {
        "text" => &text::Text,
        "secret" => &secret::Secret,
        "base64" => &base64::Base64,
        "url" => &url::Url,
        "jwt-decode" => &jwt::JwtDecode,
        "hash" => &hash::Hash,
        "hmac" => &hmac::HmacTool,
        "json" => &json::JsonTool,
        "pick" => &pick::Pick,
        "filter" => &filter::Filter,
        "map" => &map::MapTool,
        "csv" => &csv::Csv,
        "to-json" => &to_json::ToJson,
        "to-sql" => &to_sql::ToSql,
        "timestamp" => &timestamp::Timestamp,
        "regex" => &regex::RegexTool,
        "diff" => &diff::Diff,
        "js" => &js::Js,
        "http" => &http::Http,
        "flow" => &flow::Flow,
        _ => return None,
    })
}
