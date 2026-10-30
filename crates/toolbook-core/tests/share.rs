use std::path::PathBuf;

use toolbook_core::doc::Document;
use toolbook_core::engine::{RunOptions, run};
use toolbook_core::share::{markdown_with_results, to_html};

fn example(name: &str) -> (Document, PathBuf) {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name);
    (Document::parse(&std::fs::read_to_string(&p).unwrap()), p)
}

#[test]
fn share_html_shows_the_flow() {
    let (doc, p) = example("csv-to-sql.md");
    let report = run(&doc, &RunOptions::default(), p.parent().unwrap());
    let html = to_html(&doc, &report, "csv-to-sql");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<title>白名单 CSV 转 SQL</title>"), "title from the first heading");
    assert!(html.contains("CSV → 表格"), "tools by their names");
    assert!(html.contains("$rows"), "where each step's data comes from");
    assert!(html.contains("<table") && html.contains("<td>张三</td>"), "tables as tables");
    assert!(html.contains("INSERT INTO campaign_whitelist"));
    assert!(html.contains("第 3 行：手机 不匹配"), "notes are shown");
    assert!(!html.contains("<script"), "a static page");
    assert!(!html.contains("http://") && !html.contains("https://"), "no outside resources");
}

#[test]
fn share_html_escapes_and_masks_secrets() {
    let doc = Document::parse("---\ntoolbook: 1\n---\n# t\n\n```text name=x\n<b>&</b>\n```\n```secret name=key\n```\n```js name=y\n$key.length\n```\n");
    let opts = RunOptions { env: [("TOOLBOOK_SECRET_KEY".to_owned(), "s3cr3t-value".to_owned())].into(), ..RunOptions::default() };
    let report = run(&doc, &opts, std::path::Path::new("."));
    let html = to_html(&doc, &report, "t");
    assert!(html.contains("&lt;b&gt;&amp;&lt;/b&gt;"));
    assert!(!html.contains("<b>&</b>"));
    assert!(!html.contains("s3cr3t-value"));
}

#[test]
fn markdown_with_results_puts_each_result_after_its_block() {
    let (doc, p) = example("排查登录-401.md");
    let report = run(&doc, &RunOptions::default(), p.parent().unwrap());
    let md = markdown_with_results(&doc, &report);
    let block = md.find("```timestamp name=expires").unwrap();
    let result = md[block..].find("\"unix\": 1761703600").expect("the result follows its block");
    assert!(result > 0);
    assert!(md.contains("> 结果"), "results are marked");
    assert!(!md.contains("toolbook: 1"), "front matter is left out of a shared copy");
}

#[test]
fn share_html_colors_json_and_marks_the_data_flow() {
    let (doc, p) = example("排查登录-401.md");
    let report = run(&doc, &RunOptions::default(), p.parent().unwrap());
    let html = to_html(&doc, &report, "排查登录-401");
    assert!(html.contains("<span class=\"k\">&quot;alg&quot;</span>"), "JSON keys are colored");
    assert!(html.contains("<span class=\"n\">1761703600</span>"), "numbers too");
    assert!(html.contains("<b>$header</b> → <b>$jwt</b>"), "a step says what it reads and what it gives");
}

#[test]
fn results_made_from_a_secret_are_not_shared() {
    let doc = Document::parse("---\ntoolbook: 1\n---\n# t\n\n```secret name=key\n```\n```js name=y\n$key + \"-x\"\n```\n```base64 name=z input=$y\n```\n```text name=plain\nhello\n```\n");
    let opts = RunOptions { env: [("TOOLBOOK_SECRET_KEY".to_owned(), "s3cr3t-value".to_owned())].into(), ..RunOptions::default() };
    let report = run(&doc, &opts, std::path::Path::new("."));
    let encoded = "czNjcjN0LXZhbHVlLXg=";
    for (what, out) in [("html", to_html(&doc, &report, "t")), ("markdown", markdown_with_results(&doc, &report))] {
        assert!(!out.contains("s3cr3t-value"), "{what}: a block that reads the secret");
        assert!(!out.contains(encoded), "{what}: a block further down");
        assert!(out.contains("用到了密钥"), "{what}: says why the result is left out");
        assert!(out.contains("hello"), "{what}: other results are shown");
    }
}

#[test]
fn share_html_runs_no_script_from_links() {
    let doc = Document::parse("---\ntoolbook: 1\n---\n# t\n\n[a](javascript:alert(1)) [b](JavaScript:alert(2)) ![c](data:text/html,x) [d](https://example.com/x) [e](#top) [f](other.md)\n");
    let report = run(&doc, &RunOptions::default(), std::path::Path::new("."));
    let html = to_html(&doc, &report, "t");
    assert!(!html.to_lowercase().contains("javascript:"), "script links are dropped");
    assert!(!html.contains("data:text/html"), "so are data links");
    assert!(html.contains("href=\"https://example.com/x\"") && html.contains("href=\"#top\"") && html.contains("href=\"other.md\""), "the rest stay");
    assert!(html.contains("script-src 'none'"), "and the page says it runs no script");
}

#[test]
fn an_hmac_made_with_a_secret_is_shared() {
    let doc = Document::parse("---\ntoolbook: 1\n---\n# t\n\n```text name=body\nhello\n```\n```secret name=key\n```\n```hmac name=mac data=$body key=$key\n```\n");
    let opts = RunOptions { env: [("TOOLBOOK_SECRET_KEY".to_owned(), "s3cr3t-value".to_owned())].into(), ..RunOptions::default() };
    let report = run(&doc, &opts, std::path::Path::new("."));
    let html = to_html(&doc, &report, "t");
    assert!(html.contains("&quot;hex&quot;") && !html.contains("用到了密钥"), "a signature gives nothing of the key back");
    assert!(!html.contains("s3cr3t-value"));
}

#[test]
fn every_example_runs_without_errors() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let opts = RunOptions {
        env: [("TOOLBOOK_SECRET_KEY", "whsec_demo"), ("TOOLBOOK_SECRET_APP_SECRET", "demo-secret")]
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .into(),
        ..RunOptions::default()
    };
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let doc = Document::parse(&std::fs::read_to_string(&path).unwrap());
        let report = run(&doc, &opts, &dir);
        for (i, b) in report.blocks.iter().enumerate() {
            // HTTP blocks wait for ⌘↵, and what reads them waits with them.
            let waits = matches!(&b.status, toolbook_core::engine::BlockStatus::NotRun);
            assert!(
                matches!(&b.status, toolbook_core::engine::BlockStatus::Ok(_)) || waits,
                "{} 第 {} 块：{:?}",
                path.display(),
                i + 1,
                b.status
            );
        }
        seen += 1;
    }
    assert!(seen >= 13, "{seen}");
}

#[test]
fn a_flow_that_reads_a_secret_is_not_shared() {
    let dir = std::env::temp_dir().join(format!("toolbook-flow-secret-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("sub.md"), "---\ntoolbook: 1\n---\n```secret name=key\n```\n```js name=out\n\"token:\" + $key\n```\n").unwrap();
    let doc = Document::parse("---\ntoolbook: 1\n---\n# t\n\n```flow name=f src=sub.md\n```\n```js name=g\n$f.toUpperCase()\n```\n");
    let opts = RunOptions { env: [("TOOLBOOK_SECRET_KEY".to_owned(), "s3cr3t-value".to_owned())].into(), ..RunOptions::default() };
    let report = run(&doc, &opts, &dir);
    assert!(matches!(report.blocks[0].status, toolbook_core::engine::BlockStatus::Ok(_)), "{:?}", report.blocks[0].status);
    for (what, out) in [("html", to_html(&doc, &report, "t")), ("markdown", markdown_with_results(&doc, &report))] {
        assert!(!out.contains("s3cr3t-value"), "{what}: what the flow gave");
        assert!(!out.contains("S3CR3T-VALUE"), "{what}: what reads the flow");
        assert!(out.contains("用到了密钥"), "{what}");
    }
}
