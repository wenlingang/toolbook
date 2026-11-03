use toolbook_app::share::{note_html, note_markdown};

fn note(body: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("toolbook-share-{}-{n}", std::process::id()));
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

#[test]
fn preview_page_takes_the_app_theme_colors() {
    use toolbook_app::share::with_theme;
    use toolbook_app::themes::builtin_theme;
    let p = note("---\ntoolbook: 1\n---\n# 主题\n\n```text name=a\nhi\n```\n");
    let html = note_html(&p).unwrap();
    let dracula = with_theme(&html, &builtin_theme("dracula").unwrap());
    // After the page's own colors, so it wins; both appearances, as the app.
    let at = dracula.find("--bg:#282a36").expect("dracula's background");
    assert!(at > dracula.find("--bg:#fff").unwrap());
    assert!(dracula.contains("@media (prefers-color-scheme:dark){:root{--bg:#282a36"));
    assert!(dracula.contains("--accent:#bd93f9"));
    let github = with_theme(&html, &builtin_theme("github-light").unwrap());
    assert!(github.contains(":root{--bg:#ffffff") && github.contains("--accent:#0969da"));
    // The exported page keeps its own colors.
    assert!(!html.contains("#282a36"));
}
