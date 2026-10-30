use std::path::Path;
use std::time::Instant;

use toolbook_core::doc::Document;
use toolbook_core::engine::{BlockStatus, RunOptions, run};

#[test]
#[ignore = "timing; run with --release --ignored"]
fn big_json_with_many_blocks_is_fast() {
    let spans: Vec<String> = (0..60_000)
        .map(|i| format!("{{\"service\":\"svc-{i}\",\"name\":\"op {i}\",\"start\":{i},\"duration\":{}}}", i % 300))
        .collect();
    let mut body = format!("---\ntoolbook: 1\n---\n```json name=trace param\n{{\"spans\":[{}]}}\n```\n", spans.join(","));
    for i in 0..15 {
        body.push_str(&format!("```filter name=f{i} input=$trace.spans where=\"duration > {}\"\n```\n", 100 + i * 10));
    }
    assert!(body.len() > 4_000_000);
    let doc = Document::parse(&body);
    let start = Instant::now();
    let r = run(&doc, &RunOptions::default(), Path::new("."));
    let took = start.elapsed();
    assert!(r.blocks.iter().all(|b| matches!(b.status, BlockStatus::Ok(_))));
    assert!(took.as_millis() < 1000, "took {took:?}");
}
