use std::path::PathBuf;
use std::time::Instant;

use assert_cmd::Command;
use serde_json::Value;

fn examples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn tmp() -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!(
        "toolbook-cli-{}-{n}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn toolbook(home: &PathBuf) -> Command {
    let mut c = Command::cargo_bin("toolbook").unwrap();
    c.env("TOOLBOOK_HOME", home).env_remove("TOOLBOOK_SECRET_KEY");
    c
}

fn json_out(c: &mut Command) -> (Value, i32) {
    let out = c.output().unwrap();
    let v: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
    (v, out.status.code().unwrap())
}

#[test]
fn run_scenario1_outputs_expiry() {
    let home = tmp();
    let (v, code) = json_out(toolbook(&home).arg("run").arg(examples().join("排查登录-401.md")).arg("--json"));
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["ok"], true);
    assert_eq!(v["output"]["name"], "expires");
    assert_eq!(v["output"]["value"]["unix"], 1761703600);
    assert_eq!(v["blocks"].as_array().unwrap().len(), 3);
}

#[test]
fn run_with_input_file_and_stdin() {
    let home = tmp();
    let csv = home.join("in.csv");
    std::fs::write(&csv, "姓名,手机,城市\n孙七,18800188000,北京\n").unwrap();
    let out = toolbook(&home)
        .args(["run"])
        .arg(examples().join("csv-to-sql.md"))
        .arg("--input")
        .arg(format!("rows=@{}", csv.display()))
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(out.status.success());
    assert_eq!(text.trim(), "INSERT INTO campaign_whitelist (name, phone, city) VALUES ('孙七', '18800188000', '北京');");

    let out = toolbook(&home)
        .arg("run")
        .arg(examples().join("csv-to-sql.md"))
        .args(["--input", "rows=@-"])
        .write_stdin("姓名,手机,城市\n周八,13900139000,杭州\n")
        .output()
        .unwrap();
    assert!(String::from_utf8(out.stdout).unwrap().contains("'周八'"));
}

#[test]
fn output_flag_picks_block() {
    let home = tmp();
    let out = toolbook(&home)
        .arg("run")
        .arg(examples().join("csv-to-sql.md"))
        .args(["--output", "json", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["output"]["value"].as_array().unwrap().len(), 3);
}

#[test]
fn failure_exits_1_with_block_info() {
    let home = tmp();
    let out = toolbook(&home)
        .arg("run")
        .arg(examples().join("排查登录-401.md"))
        .args(["--input", "header=garbage"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("第 2 块") && err.contains("JWT"), "{err}");

    let (v, code) = json_out(toolbook(&home).arg("run").arg(examples().join("排查登录-401.md")).args(["--input", "header=garbage", "--json"]));
    assert_eq!(code, 1);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["block"], 2);
}

#[test]
fn secret_from_input_and_missing_file_exit_2() {
    let home = tmp();
    let (v, code) = json_out(toolbook(&home).arg("run").arg(examples().join("webhook-验签.md")).args(["--input", "key=whsec_demo_9f3a1c", "--json"]));
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["output"]["value"], serde_json::json!({"hex": false, "base64": true}));
    let out = toolbook(&home).args(["run", "/nope/missing.md"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn list_and_describe_json() {
    let home = tmp();
    std::fs::create_dir_all(home.join("数据转换")).unwrap();
    std::fs::create_dir_all(home.join(".themes")).unwrap();
    std::fs::copy(examples().join("csv-to-sql.md"), home.join("数据转换/csv-to-sql.md")).unwrap();
    std::fs::write(home.join(".themes/x.md"), "hidden").unwrap();
    let (v, code) = json_out(toolbook(&home).args(["list", "--json"]));
    assert_eq!(code, 0);
    let items = v.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["path"], "数据转换/csv-to-sql.md");
    assert_eq!(items[0]["inputs"], serde_json::json!(["rows"]));

    let (v, _) = json_out(toolbook(&home).args(["describe", "数据转换/csv-to-sql.md", "--json"]));
    assert_eq!(v["inputs"][0]["name"], "rows");
    assert_eq!(v["outputs"], serde_json::json!(["rows", "clean", "json", "sql"]));
    assert!(v["description"].as_str().unwrap().contains("白名单"));
}

#[test]
fn export_skill_writes_skill_md() {
    let home = tmp();
    let dir = home.join("skills");
    let out = toolbook(&home)
        .arg("export-skill")
        .arg(examples().join("csv-to-sql.md"))
        .arg("--dir")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let skill = std::fs::read_to_string(dir.join("csv-to-sql/SKILL.md")).unwrap();
    assert!(skill.starts_with("---\nname: csv-to-sql\n"));
    assert!(skill.contains("toolbook run"));
    assert!(skill.contains("--input rows="));
}

#[test]
#[ignore = "timing; run with --release --ignored"]
fn run_is_fast() {
    let home = tmp();
    let n = 10;
    let start = Instant::now();
    for _ in 0..n {
        toolbook(&home).arg("run").arg(examples().join("排查登录-401.md")).assert().success();
    }
    let avg = start.elapsed() / n;
    assert!(avg.as_millis() < 50, "average {avg:?}");
}

#[test]
fn unknown_input_name_is_usage_error() {
    let home = tmp();
    let out = toolbook(&home)
        .arg("run")
        .arg(examples().join("排查登录-401.md"))
        .args(["--input", "heder=x"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("heder") && err.contains("header"), "{err}");
}

#[test]
fn json_output_never_prints_secrets() {
    let home = tmp();
    let out = toolbook(&home)
        .env("TOOLBOOK_SECRET_KEY", "supersecret-value")
        .arg("run")
        .arg(examples().join("webhook-验签.md"))
        .arg("--json")
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(!stdout.contains("supersecret-value"), "{stdout}");
    let out = toolbook(&home)
        .env("TOOLBOOK_SECRET_KEY", "supersecret-value")
        .arg("run")
        .arg(examples().join("webhook-验签.md"))
        .args(["--output", "key"])
        .output()
        .unwrap();
    assert!(!String::from_utf8(out.stdout).unwrap().contains("supersecret-value"));
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn run_does_not_wait_on_open_stdin() {
    use std::process::{Command as Std, Stdio};
    let home = tmp();
    let bin = assert_cmd::cargo::cargo_bin("toolbook");
    let mut child = Std::new(bin)
        .env("TOOLBOOK_HOME", &home)
        .arg("run")
        .arg(examples().join("trace-慢请求.md"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let _keep_open = child.stdin.take();
    let start = Instant::now();
    while start.elapsed().as_secs() < 5 {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let _ = child.kill();
    panic!("toolbook run waited on stdin");
}

#[test]
fn export_skill_slugifies_and_escapes() {
    let home = tmp();
    let dir = home.join("skills");
    let note = home.join("有空格 的笔记.md");
    std::fs::write(&note, "---\ntoolbook: 1\n---\n说明: 带冒号\n\n```text name=s param\nx\n```\n").unwrap();
    let out = toolbook(&home).arg("export-skill").arg(&note).arg("--dir").arg(&dir).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let made: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(made.len(), 1);
    let name = &made[0];
    assert!(name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'), "{name}");
    let skill = std::fs::read_to_string(dir.join(name).join("SKILL.md")).unwrap();
    assert!(skill.contains("description: \"说明: 带冒号"), "{skill}");
    assert!(skill.contains("toolbook run '"), "{skill}");
    let out = toolbook(&home).arg("export-skill").arg(&note).args(["--name", "my-flow", "--dir"]).arg(&dir).output().unwrap();
    assert!(out.status.success());
    assert!(dir.join("my-flow/SKILL.md").is_file());
}

#[test]
fn share_writes_html_or_markdown() {
    let home = tmp();
    let out_file = home.join("share.html");
    let out = toolbook(&home).arg("share").arg(examples().join("csv-to-sql.md")).arg("-o").arg(&out_file).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let html = std::fs::read_to_string(&out_file).unwrap();
    assert!(html.starts_with("<!doctype html>") && html.contains("INSERT INTO campaign_whitelist"));
    let out = toolbook(&home).arg("share").arg(examples().join("排查登录-401.md")).arg("--markdown").output().unwrap();
    let md = String::from_utf8(out.stdout).unwrap();
    assert!(md.contains("> 结果") && md.contains("1761703600"));
}

#[test]
fn export_skill_quotes_input_names_in_its_command() {
    // An agent runs the skill's command as written: a block name must not
    // be able to add a command of its own.
    let home = tmp();
    let dir = home.join("skills");
    let note = home.join("n.md");
    std::fs::write(&note, "---\ntoolbook: 1\n---\n```text name=a;touch$(x) param\nx\n```\n").unwrap();
    let out = toolbook(&home).arg("export-skill").arg(&note).arg("--dir").arg(&dir).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let skill = std::fs::read_to_string(dir.join("n/SKILL.md")).unwrap();
    assert!(skill.contains(" --input 'a;touch$(x)=…'"), "{skill}");
    assert!(!skill.contains(" --input a;touch"), "{skill}");
}
