// Shared by the tool tests: run a note body, read block results.
#![allow(dead_code)]

use std::path::Path;

use toolbook_core::doc::Document;
use toolbook_core::engine::{BlockStatus, RunOptions, run};

pub fn run_body(body: &str, opts: &RunOptions) -> Vec<BlockStatus> {
    let doc = Document::parse(&format!("---\ntoolbook: 1\n---\n{body}"));
    run(&doc, opts, Path::new(".")).blocks.into_iter().map(|b| b.status).collect()
}

pub fn last(body: &str) -> BlockStatus {
    run_body(body, &RunOptions::default()).pop().unwrap()
}

pub fn b64url(s: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s)
}
