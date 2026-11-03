//! `toolbook`: runs, describes and lists notes. Never asks anything: made
//! for scripts and for Claude / Codex as much as for people.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde_json::{Value as Json, json};
use toolbook_core::engine::{BlockResult, BlockStatus, RunOptions, RunReport};
use toolbook_core::{NoteSpec, describe_file, run_file};

#[derive(Parser)]
#[command(name = "toolbook", version, about = "运行、查看 Toolbook 笔记")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 运行一本笔记，输出一个块的结果（默认最后一个具名块）
    Run {
        /// 笔记路径（相对当前目录或笔记库）
        note: PathBuf,
        /// 输入：name=值，值以 @ 开头时读文件，@- 读标准输入
        #[arg(long = "input", value_name = "NAME=VALUE")]
        inputs: Vec<String>,
        /// 输出哪个块
        #[arg(long, value_name = "NAME")]
        output: Option<String>,
        /// 输出 JSON
        #[arg(long)]
        json: bool,
    },
    /// 列出笔记的输入、输出和说明
    Describe {
        /// 笔记路径
        note: PathBuf,
        /// 输出 JSON
        #[arg(long)]
        json: bool,
    },
    /// 列出笔记库里的所有笔记
    List {
        /// 输出 JSON
        #[arg(long)]
        json: bool,
    },
    /// 生成一本笔记的分享页（HTML，从输入到输出的每一步），或带结果的 Markdown
    Share {
        /// 笔记路径
        note: PathBuf,
        /// 写到这个文件（默认输出到标准输出）
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// 输出带结果的 Markdown，而不是 HTML
        #[arg(long)]
        markdown: bool,
        /// 输入：name=值（同 run）
        #[arg(long = "input", value_name = "NAME=VALUE")]
        inputs: Vec<String>,
    },
    /// 把一本笔记导出成 Claude skill
    ExportSkill {
        /// 笔记路径
        note: PathBuf,
        /// skill 目录（默认 ~/.claude/skills）
        #[arg(long)]
        dir: Option<PathBuf>,
        /// skill 名（小写字母、数字、连字符；默认由文件名生成）
        #[arg(long)]
        name: Option<String>,
    },
}

/// A usage problem: exit code 2.
struct Usage(String);

fn library() -> PathBuf {
    RunOptions::from_process(false).library
}

/// The note at `p`, as given or inside the library.
fn find_note(p: &Path) -> Result<PathBuf, Usage> {
    if p.is_file() {
        return Ok(p.to_path_buf());
    }
    let in_lib = library().join(p);
    if in_lib.is_file() {
        return Ok(in_lib);
    }
    Err(Usage(format!("找不到笔记 {}", p.display())))
}

fn read_value(v: &str) -> Result<String, Usage> {
    match v.strip_prefix('@') {
        Some("-") => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).map_err(|e| Usage(format!("读取标准输入失败：{e}")))?;
            Ok(s)
        }
        Some(path) => std::fs::read_to_string(path).map_err(|e| Usage(format!("读取 {path} 失败：{e}"))),
        None => Ok(v.to_owned()),
    }
}

/// The block that went wrong first, following `Upstream` back.
fn root_cause(report: &RunReport, mut index: usize) -> (usize, String) {
    loop {
        match &report.blocks[index].status {
            BlockStatus::Upstream { block, .. } => index = *block,
            BlockStatus::Error(m) | BlockStatus::Incomplete(m) => return (index, m.clone()),
            BlockStatus::NotRun => return (index, "没有运行".into()),
            BlockStatus::Ok(_) => return (index, "没有输出".into()),
        }
    }
}

fn block_json(b: &BlockResult) -> Json {
    let mut o = json!({ "index": b.index + 1, "name": b.name, "tool": b.tool });
    let (status, key, val) = match &b.status {
        BlockStatus::Ok(_) if b.tool == "secret" => ("ok", "value", json!("***")),
        BlockStatus::Ok(v) => ("ok", "value", v.to_json()),
        BlockStatus::Error(m) => ("error", "error", json!(m)),
        BlockStatus::Incomplete(m) => ("incomplete", "error", json!(m)),
        BlockStatus::Upstream { block, name } => ("upstream", "error", json!(format!("上游 [{}] ${name} 出错", block + 1))),
        BlockStatus::NotRun => ("not_run", "error", json!("没有运行")),
    };
    o["status"] = json!(status);
    o[key] = val;
    if !b.notes.is_empty() {
        o["notes"] = json!(b.notes);
    }
    o
}

fn label(b: &BlockResult) -> String {
    match &b.name {
        Some(n) => format!("第 {} 块（{n}，{}）", b.index + 1, b.tool),
        None => format!("第 {} 块（{}）", b.index + 1, b.tool),
    }
}

fn cmd_run(note: &Path, inputs: &[String], output: Option<&str>, as_json: bool) -> Result<ExitCode, Usage> {
    let path = find_note(note)?;
    let spec = describe_file(&path).map_err(|e| Usage(format!("读取 {} 失败：{e}", path.display())))?;
    let mut opts = RunOptions::from_process(true);
    for i in inputs {
        let (k, v) = i.split_once('=').ok_or_else(|| Usage(format!("--input {i:?} 应写成 name=值")))?;
        if !spec.inputs.iter().any(|x| x.name == k) {
            let names: Vec<&str> = spec.inputs.iter().map(|x| x.name.as_str()).collect();
            return Err(Usage(format!(
                "笔记没有名为 {k} 的输入，可用：{}",
                if names.is_empty() { "（无）".to_owned() } else { names.join("、") }
            )));
        }
        opts.overrides.insert(k.to_owned(), read_value(v)?);
    }
    let report = run_file(&path, &opts).map_err(|e| Usage(format!("读取 {} 失败：{e}", path.display())))?;
    let Some(out) = report.output(output) else {
        return Err(Usage(match output {
            Some(o) => format!("笔记里没有名为 {o} 的块"),
            None => "笔记里没有具名块可作为输出".into(),
        }));
    };
    if out.tool == "secret" {
        return Err(Usage("密钥块不能作为输出".into()));
    }
    let blocks: Vec<Json> = report.blocks.iter().map(block_json).collect();
    if let BlockStatus::Ok(v) = &out.status {
        if as_json {
            let doc = json!({
                "ok": true,
                "output": { "block": out.index + 1, "name": out.name, "value": v.to_json() },
                "blocks": blocks,
            });
            println!("{doc}");
        } else {
            let text = v.render_plain();
            print!("{text}");
            if !text.ends_with('\n') {
                println!();
            }
        }
        return Ok(ExitCode::SUCCESS);
    }
    let (i, why) = root_cause(&report, out.index);
    let culprit = &report.blocks[i];
    eprintln!("{}：{why}", label(culprit));
    if as_json {
        let doc = json!({
            "ok": false,
            "error": { "block": i + 1, "name": culprit.name, "message": why },
            "blocks": blocks,
        });
        println!("{doc}");
    }
    Ok(ExitCode::from(1))
}

fn cmd_share(note: &Path, output: Option<&Path>, markdown: bool, inputs: &[String]) -> Result<ExitCode, Usage> {
    let path = find_note(note)?;
    let text = std::fs::read_to_string(&path).map_err(|e| Usage(format!("读取 {} 失败：{e}", path.display())))?;
    let doc = toolbook_core::doc::Document::parse(&text);
    let mut opts = RunOptions::from_process(true);
    for i in inputs {
        let (k, v) = i.split_once('=').ok_or_else(|| Usage(format!("--input {i:?} 应写成 name=值")))?;
        opts.overrides.insert(k.to_owned(), read_value(v)?);
    }
    let report = run_file(&path, &opts).map_err(|e| Usage(format!("运行失败：{e}")))?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let out = if markdown {
        toolbook_core::share::markdown_with_results(&doc, &report)
    } else {
        toolbook_core::share::to_html(&doc, &report, &stem)
    };
    match output {
        Some(file) => {
            std::fs::write(file, out).map_err(|e| Usage(format!("写入 {} 失败：{e}", file.display())))?;
            println!("{}", file.display());
        }
        None => print!("{out}"),
    }
    Ok(ExitCode::SUCCESS)
}

fn spec_json(spec: &NoteSpec) -> Json {
    json!({
        "description": spec.description,
        "inputs": spec.inputs.iter().map(|i| json!({
            "name": i.name, "tool": i.tool, "default": i.default, "secret": i.secret,
        })).collect::<Vec<_>>(),
        "outputs": spec.outputs,
    })
}

fn cmd_describe(note: &Path, as_json: bool) -> Result<ExitCode, Usage> {
    let path = find_note(note)?;
    let spec = describe_file(&path).map_err(|e| Usage(format!("读取失败：{e}")))?;
    if as_json {
        println!("{}", spec_json(&spec));
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(d) = &spec.description {
        println!("{d}\n");
    }
    println!("输入：");
    for i in &spec.inputs {
        let kind = if i.secret { "（密钥，环境变量 TOOLBOOK_SECRET_*）" } else { "" };
        println!("  {} [{}]{kind}", i.name, i.tool);
    }
    println!("输出：{}", spec.outputs.join("、"));
    Ok(ExitCode::SUCCESS)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
}

fn cmd_list(as_json: bool) -> Result<ExitCode, Usage> {
    let lib = library();
    let mut files = Vec::new();
    walk(&lib, &mut files);
    let mut items = Vec::new();
    for f in files {
        let Ok(spec) = describe_file(&f) else { continue };
        let rel = f.strip_prefix(&lib).unwrap_or(&f).to_string_lossy().into_owned();
        items.push((rel, spec));
    }
    if as_json {
        let arr: Vec<Json> = items
            .iter()
            .map(|(p, s)| json!({
                "path": p,
                "description": s.description,
                "inputs": s.inputs.iter().map(|i| i.name.clone()).collect::<Vec<_>>(),
            }))
            .collect();
        println!("{}", Json::Array(arr));
    } else {
        for (p, s) in &items {
            println!("{p}\t{}", s.description.as_deref().unwrap_or(""));
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// A skill name from a file name: lowercase ASCII letters, digits and `-`;
/// `toolbook-` in front when the file name had other characters.
fn slug(stem: &str) -> String {
    let mut out = String::new();
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_owned();
    if out.is_empty() {
        "toolbook-note".into()
    } else if stem.chars().any(|c| !c.is_ascii()) {
        format!("toolbook-{out}")
    } else {
        out
    }
}

/// `s` in single quotes for a POSIX shell.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// `--input name=value` as a shell word: as it is when the name is a plain
/// word (letters, digits, `_`, `-`, or not ASCII, as Chinese names are),
/// quoted otherwise, so a block's name cannot add a command of its own.
fn input_arg(name: &str, value: &str) -> String {
    let arg = format!("{name}={value}");
    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || !c.is_ascii()) {
        arg
    } else {
        shell_quote(&arg)
    }
}

fn cmd_export_skill(note: &Path, dir: Option<PathBuf>, name: Option<String>) -> Result<ExitCode, Usage> {
    let path = find_note(note)?;
    let spec = describe_file(&path).map_err(|e| Usage(format!("读取失败：{e}")))?;
    let title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = match name {
        Some(n) if !n.is_empty() && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') => n,
        Some(n) => return Err(Usage(format!("--name {n:?} 只能用小写字母、数字和连字符"))),
        None => slug(&title),
    };
    let dir = match dir {
        Some(d) => d,
        None => {
            let home = std::env::var("HOME").map_err(|_| Usage("没有 HOME，请用 --dir 指定目录".into()))?;
            Path::new(&home).join(".claude/skills")
        }
    };
    let abs = path.canonicalize().unwrap_or(path.clone());
    let desc = spec.description.clone().unwrap_or_else(|| format!("Toolbook 工具流 {title}"));
    let front_desc = serde_json::to_string(&format!("{}。通过 toolbook run 执行这本 Toolbook 笔记。", desc.trim_end_matches('。')))
        .unwrap_or_default();
    let mut md = format!(
        "---\nname: {stem}\ndescription: {front_desc}\n---\n\n# {title}\n\n{desc}\n\n## 运行\n\n```sh\ntoolbook run {} --json",
        shell_quote(&abs.to_string_lossy())
    );
    for i in spec.inputs.iter().filter(|i| !i.secret) {
        md.push_str(&format!(" --input {}", input_arg(&i.name, "…")));
    }
    md.push_str("\n```\n\n## 输入\n\n");
    for i in &spec.inputs {
        if i.secret {
            md.push_str(&format!("- `{}`：密钥，从环境变量 `TOOLBOOK_SECRET_{}` 读取，或 `--input {}`\n", i.name, i.name.to_uppercase().replace('-', "_"), input_arg(&i.name, "…")));
        } else {
            md.push_str(&format!("- `{}`（{}）：值前加 `@` 表示读文件，如 `--input {}`\n", i.name, i.tool, input_arg(&i.name, "@data.csv")));
        }
    }
    md.push_str(&format!(
        "\n## 输出\n\n默认输出最后一个具名块；可用 `--output` 选择：{}。\n\n退出码 0 成功；1 某个块出错（stderr 和 JSON 里的 `error.block` 指出是第几块）；2 参数或文件错误。\n",
        spec.outputs.join("、")
    ));
    let target = dir.join(&stem);
    std::fs::create_dir_all(&target).map_err(|e| Usage(format!("创建 {} 失败：{e}", target.display())))?;
    std::fs::write(target.join("SKILL.md"), md).map_err(|e| Usage(format!("写入失败：{e}")))?;
    println!("{}", target.join("SKILL.md").display());
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    // Like other Unix tools, end quietly when the reader goes away (`toolbook run … | head`)
    // instead of panicking on the failed write; Rust ignores SIGPIPE by default.
    #[cfg(unix)]
    // SAFETY: called first thing in main, before any other thread exists.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run { note, inputs, output, json } => cmd_run(&note, &inputs, output.as_deref(), json),
        Command::Describe { note, json } => cmd_describe(&note, json),
        Command::List { json } => cmd_list(json),
        Command::Share { note, output, markdown, inputs } => cmd_share(&note, output.as_deref(), markdown, &inputs),
        Command::ExportSkill { note, dir, name } => cmd_export_skill(&note, dir, name),
    };
    result.unwrap_or_else(|Usage(m)| {
        eprintln!("toolbook: {m}");
        ExitCode::from(2)
    })
}
