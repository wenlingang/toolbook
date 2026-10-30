//! The values blocks produce.

use serde_json::{Map, Value as Json};

/// Rows of strings under named columns, as a CSV reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// The column names, from the header row.
    pub columns: Vec<String>,
    /// The rows, each as long as `columns`.
    pub rows: Vec<Vec<String>>,
}

/// What a block outputs.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Plain text.
    Text(String),
    /// A JSON value.
    Json(Json),
    /// A table.
    Table(Table),
}

const SUMMARY_CHARS: usize = 80;

impl Value {
    /// One line describing the value, at most 80 characters and an ellipsis.
    pub fn summary(&self) -> String {
        let line = match self {
            Value::Text(s) => s.lines().next().unwrap_or("").to_owned(),
            Value::Json(j) => j.to_string(),
            Value::Table(t) => {
                return format!("表格 · {} 行 × {} 列", t.rows.len(), t.columns.len());
            }
        };
        if line.chars().count() > SUMMARY_CHARS {
            let mut cut: String = line.chars().take(SUMMARY_CHARS).collect();
            cut.push('…');
            cut
        } else {
            line
        }
    }

    /// The value as JSON: text as a string, a table as an array of objects
    /// keyed by column.
    pub fn to_json(&self) -> Json {
        match self {
            Value::Text(s) => Json::String(s.clone()),
            Value::Json(j) => j.clone(),
            Value::Table(t) => Json::Array(
                t.rows
                    .iter()
                    .map(|row| {
                        let mut obj = Map::new();
                        for (col, cell) in t.columns.iter().zip(row) {
                            obj.insert(col.clone(), Json::String(cell.clone()));
                        }
                        Json::Object(obj)
                    })
                    .collect(),
            ),
        }
    }

    /// The text when the value is text.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }

    /// The value as a person reads it in a terminal: text as it is, JSON
    /// indented by two spaces, a table as CSV.
    pub fn render_plain(&self) -> String {
        match self {
            Value::Text(s) => s.clone(),
            Value::Json(j) => serde_json::to_string_pretty(j).unwrap_or_default(),
            Value::Table(t) => {
                let mut w = csv::Writer::from_writer(Vec::new());
                let _ = w.write_record(&t.columns);
                for row in &t.rows {
                    let _ = w.write_record(row);
                }
                String::from_utf8(w.into_inner().unwrap_or_default()).unwrap_or_default()
            }
        }
    }
}
