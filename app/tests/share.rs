use toolbook_app::share::{note_html, note_markdown};

fn note(body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("toolbook-share-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("n.md");
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn shared_page_and_markdown_come_from_the_note() {
    let p = note(
        "---\ntoolbook: 1\n---\n# 验签\n\n```text name=body\nhello\n```\n```secret name=key\n```\n```hmac name=mac data=$body key=$key\n```\n",
    );
    toolbook_app::share::remember_secret_for_test(&p, "key", "s3cr3t");
    let html = note_html(&p).unwrap();
    assert!(html.contains("<title>验签</title>"));
    assert!(html.contains("HMAC 签名"));
    assert!(
        !html.contains("s3cr3t"),
        "a typed secret never reaches a shared page"
    );
    assert!(
        html.contains("&quot;hex&quot;"),
        "the HMAC ran with the typed secret"
    );
    let md = note_markdown(&p).unwrap();
    assert!(md.contains("> 结果") && !md.contains("s3cr3t"));
}
