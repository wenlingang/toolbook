//! `timestamp tz=Asia/Shanghai`: seconds, milliseconds or a date, in several forms.

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::json;

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct Timestamp;

const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// "3 小时 20 分钟前" / "2 天 1 小时后": the two largest units.
fn relative(diff_ms: i64) -> String {
    let secs = diff_ms.unsigned_abs() / 1000;
    let parts = [
        (secs / 86_400, "天"),
        (secs % 86_400 / 3600, "小时"),
        (secs % 3600 / 60, "分钟"),
        (secs % 60, "秒"),
    ];
    let Some(first) = parts.iter().position(|(n, _)| *n > 0) else {
        return "刚刚".into();
    };
    let mut words: Vec<String> = vec![format!("{} {}", parts[first].0, parts[first].1)];
    if let Some((n, unit)) = parts.get(first + 1)
        && *n > 0
    {
        words.push(format!("{n} {unit}"));
    }
    format!("{}{}", words.join(" "), if diff_ms >= 0 { "前" } else { "后" })
}

impl Tool for Timestamp {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let text = ctx.primary_text()?;
        let text = text.trim().trim_matches('"');
        let tz_name = ctx.attr_text("tz").unwrap_or_else(|| "Asia/Shanghai".into());
        let tz: Tz = tz_name
            .parse()
            .map_err(|_| ToolError::Failed(format!("不认识的时区 {tz_name}，例如 Asia/Shanghai、UTC")))?;
        let ms: i64 = if let Ok(n) = text.parse::<i64>() {
            if n.unsigned_abs() > 1_000_000_000_000 { n } else { n * 1000 }
        } else if let Ok(d) = DateTime::parse_from_rfc3339(text) {
            d.timestamp_millis()
        } else if let Ok(naive) = NaiveDateTime::parse_from_str(text, FORMAT) {
            tz.from_local_datetime(&naive)
                .single()
                .ok_or_else(|| ToolError::Failed(format!("{text} 在 {tz_name} 不是唯一的时刻")))?
                .timestamp_millis()
        } else {
            return Err(ToolError::Failed(format!(
                "看不懂 {text:?}：请输入秒或毫秒时间戳，或 2025-10-29T02:06:40Z、2025-10-29 10:06:40"
            )));
        };
        let utc = Utc
            .timestamp_millis_opt(ms)
            .single()
            .ok_or_else(|| ToolError::Failed("时间戳超出范围".into()))?;
        Ok(Value::Json(json!({
            "unix": ms.div_euclid(1000),
            "unix_ms": ms,
            "utc": utc.format(FORMAT).to_string(),
            "local": utc.with_timezone(&tz).format(FORMAT).to_string(),
            "tz": tz_name,
            "relative": relative(ctx.opts.now_ms - ms),
        })))
    }
}
