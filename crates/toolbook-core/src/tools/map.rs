//! `map rename="旧:新,…" keep="a,b"`: renames and keeps fields.

use serde_json::Map;

use super::rows::{records, same_kind};
use super::{Tool, ToolCtx, ToolError};
use crate::value::{Table, Value};

pub(super) struct MapTool;

fn list(s: Option<String>) -> Vec<String> {
    s.map(|s| s.split(',').map(|p| p.trim().to_owned()).filter(|p| !p.is_empty()).collect())
        .unwrap_or_default()
}

impl Tool for MapTool {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let input = ctx.require_primary()?;
        let renames: Vec<(String, String)> = list(ctx.attr_text("rename"))
            .iter()
            .map(|p| match p.split_once(':') {
                Some((a, b)) => Ok((a.trim().to_owned(), b.trim().to_owned())),
                None => Err(ToolError::Failed(format!("rename 里的 {p:?} 应写成 旧:新"))),
            })
            .collect::<Result<_, _>>()?;
        let keep = list(ctx.attr_text("keep"));
        let rename = |k: &str| {
            renames.iter().find(|(a, _)| a == k).map(|(_, b)| b.clone()).unwrap_or_else(|| k.to_owned())
        };
        let rows = records(input)?
            .into_iter()
            .map(|r| {
                let mut out = Map::new();
                for (k, v) in r {
                    let k = rename(&k);
                    if keep.is_empty() || keep.contains(&k) {
                        out.insert(k, v);
                    }
                }
                out
            })
            .collect();
        let columns = match input {
            Value::Table(Table { columns, .. }) => Some(
                columns.iter().map(|c| rename(c)).filter(|c| keep.is_empty() || keep.contains(c)).collect(),
            ),
            _ => None,
        };
        Ok(same_kind(input, rows, columns))
    }
}
