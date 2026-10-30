//! Toolbook engine: parses notes, runs their tool blocks in order.

pub mod describe;
pub mod doc;
pub mod engine;
pub mod fence;
pub mod path;
pub mod share;
pub mod template;
pub mod tools;
pub mod value;

pub use describe::{InputSpec, NoteSpec, describe};
pub use value::{Table, Value};

use std::path::Path;

use engine::{RunOptions, RunReport};

/// Reads and runs the note at `path`.
pub fn run_file(path: &Path, opts: &RunOptions) -> std::io::Result<RunReport> {
    let text = std::fs::read_to_string(path)?;
    let doc = doc::Document::parse(&text);
    let canon = path.canonicalize()?;
    let base = canon.parent().unwrap_or(Path::new(".")).to_path_buf();
    Ok(engine::run_with_stack(&doc, opts, &base, &[canon]))
}

/// Reads and describes the note at `path`.
pub fn describe_file(path: &Path) -> std::io::Result<NoteSpec> {
    Ok(describe(&doc::Document::parse(&std::fs::read_to_string(path)?)))
}
