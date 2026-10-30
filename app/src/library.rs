//! The library: the notes under `~/Toolbook/` (or `$TOOLBOOK_HOME`).

use std::path::{Path, PathBuf};

use nsh_core::element::move_to_trash;
use nsh_core::prelude::*;
use toolbook_core::engine::RunOptions;

/// The example notes a new library starts with: folder, file, text.
const SEEDS: &[(&str, &str, &str)] = &[
    (
        "日志排查",
        "排查登录-401.md",
        include_str!("../../examples/排查登录-401.md"),
    ),
    (
        "日志排查",
        "trace-慢请求.md",
        include_str!("../../examples/trace-慢请求.md"),
    ),
    (
        "日志排查",
        "nginx-慢请求统计.md",
        include_str!("../../examples/nginx-慢请求统计.md"),
    ),
    (
        "数据转换",
        "csv-to-sql.md",
        include_str!("../../examples/csv-to-sql.md"),
    ),
    (
        "数据转换",
        "csv-批量更新.md",
        include_str!("../../examples/csv-批量更新.md"),
    ),
    (
        "数据转换",
        "配置对比.md",
        include_str!("../../examples/配置对比.md"),
    ),
    (
        "签名",
        "webhook-验签.md",
        include_str!("../../examples/webhook-验签.md"),
    ),
    (
        "签名",
        "api-签名.md",
        include_str!("../../examples/api-签名.md"),
    ),
    (
        "签名",
        "安装包校验.md",
        include_str!("../../examples/安装包校验.md"),
    ),
    (
        "编码解码",
        "k8s-secret-解码.md",
        include_str!("../../examples/k8s-secret-解码.md"),
    ),
    (
        "编码解码",
        "回调地址解码.md",
        include_str!("../../examples/回调地址解码.md"),
    ),
    (
        "时间",
        "时区换算.md",
        include_str!("../../examples/时区换算.md"),
    ),
    (
        "接口请求",
        "github-仓库信息.md",
        include_str!("../../examples/github-仓库信息.md"),
    ),
];

/// What a new note starts with: a short demo of the basics.
const NEW_NOTE: &str = include_str!("new_note.md");

/// A note in the library.
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct NoteItem {
    /// Its file.
    pub path: String,
    /// Its file name without `.md`.
    pub name: String,
    /// Its folder inside the library (`""` at the top).
    pub dir: String,
}

fn root_dir() -> PathBuf {
    RunOptions::from_process(false).library
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<NoteItem>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(kind) = e.file_type() else { continue };
        let p = e.path();
        if kind.is_dir() {
            walk(root, &p, out);
        } else if kind.is_file() && p.extension().is_some_and(|x| x == "md") {
            let rel_dir = p
                .parent()
                .and_then(|d| d.strip_prefix(root).ok())
                .map(|d| d.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push(NoteItem {
                path: p.to_string_lossy().into_owned(),
                name: p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                dir: rel_dir,
            });
        }
    }
}

/// The library's own settings (`.toolbook.json`): the categories' order.
fn settings_path() -> PathBuf {
    root_dir().join(".toolbook.json")
}

fn saved_order() -> Vec<String> {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("dir_order").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn save_order(order: &[String]) -> Result<(), String> {
    std::fs::create_dir_all(root_dir()).map_err(|e| format!("保存分类顺序失败：{e}"))?;
    let json = serde_json::json!({ "dir_order": order });
    std::fs::write(
        settings_path(),
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    )
    .map_err(|e| format!("保存分类顺序失败：{e}"))
}

/// A folder or file name: no path separators, no `..`, not hidden.
fn plain_name(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('.') && !s.contains(['/', '\\']) && s != ".."
}

/// The notes in the library.
#[module]
pub struct Library;

#[module]
impl Library {
    /// The library's folder.
    pub fn root() -> String {
        root_dir().to_string_lossy().into_owned()
    }

    /// Every note, sorted by folder then name (symbolic links are not followed).
    pub fn list() -> Vec<NoteItem> {
        let root = root_dir();
        let mut out = Vec::new();
        walk(&root, &root, &mut out);
        out.sort_by(|a, b| {
            (a.dir.as_str(), a.name.as_str()).cmp(&(b.dir.as_str(), b.name.as_str()))
        });
        out
    }

    /// The categories: the library's folders, empty ones included, sorted.
    pub fn dirs() -> Vec<String> {
        let root = root_dir();
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(&root) else {
            return out;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.') && e.file_type().is_ok_and(|t| t.is_dir()) {
                out.push(name);
            }
        }
        // In the saved order; categories not in it (new ones) after, by name.
        let order = saved_order();
        out.sort_by(|a, b| {
            let pa = order.iter().position(|o| o == a).unwrap_or(usize::MAX);
            let pb = order.iter().position(|o| o == b).unwrap_or(usize::MAX);
            pa.cmp(&pb).then_with(|| a.cmp(b))
        });
        out
    }

    /// Keeps the categories in `order` (as dragged in the sidebar).
    pub fn set_dir_order(order: Vec<String>) -> Result<(), String> {
        save_order(&order)
    }

    /// Creates the category `name` (a folder in the library); its path.
    pub fn create_dir(name: String) -> Result<String, String> {
        let name = name.trim().to_owned();
        if !plain_name(&name) {
            return Err("分类名不能为空，也不能包含 / 或以 . 开头".into());
        }
        let path = root_dir().join(&name);
        if path.exists() {
            return Err(format!("已经有分类 {name}"));
        }
        std::fs::create_dir_all(&path).map_err(|e| format!("创建分类失败：{e}"))?;
        Ok(path.to_string_lossy().into_owned())
    }

    /// Creates an empty note `name` in folder `dir` (`""` for the top); its path.
    pub fn create(dir: String, name: String) -> Result<String, String> {
        let name = name.trim().trim_end_matches(".md").to_owned();
        if !plain_name(&name) || (!dir.is_empty() && !dir.split('/').all(plain_name)) {
            return Err("名字不能为空，也不能包含 / 或以 . 开头".into());
        }
        let folder = if dir.is_empty() {
            root_dir()
        } else {
            root_dir().join(&dir)
        };
        std::fs::create_dir_all(&folder).map_err(|e| format!("创建文件夹失败：{e}"))?;
        let path = folder.join(format!("{name}.md"));
        if path.exists() {
            return Err(format!("已经有一本叫 {name} 的笔记"));
        }
        std::fs::write(&path, NEW_NOTE.replace("{{name}}", &name))
            .map_err(|e| format!("写入失败：{e}"))?;
        Ok(path.to_string_lossy().into_owned())
    }

    /// Renames the note at `path` to `name` in its category; its new path.
    pub fn rename(path: String, name: String) -> Result<String, String> {
        let from = PathBuf::from(&path);
        let name = name.trim().trim_end_matches(".md").to_owned();
        if !plain_name(&name) {
            return Err("名字不能为空，也不能包含 / 或以 . 开头".into());
        }
        let to = from.with_file_name(format!("{name}.md"));
        if to == from {
            return Ok(path);
        }
        if to.exists() {
            return Err(format!("这个分类里已经有 {name}"));
        }
        std::fs::rename(&from, &to).map_err(|e| format!("重命名失败：{e}"))?;
        Ok(to.to_string_lossy().into_owned())
    }

    /// Moves the note at `path` into category `dir` (`""` for the top); its new path.
    pub fn move_to(path: String, dir: String) -> Result<String, String> {
        let from = PathBuf::from(&path);
        if !dir.is_empty() && !plain_name(&dir) {
            return Err("分类名不能包含 / 或以 . 开头".into());
        }
        let folder = if dir.is_empty() {
            root_dir()
        } else {
            root_dir().join(&dir)
        };
        let file = from.file_name().ok_or("不是一本笔记")?;
        let to = folder.join(file);
        if to == from {
            return Ok(path);
        }
        if to.exists() {
            return Err(format!(
                "{} 里已经有同名笔记",
                if dir.is_empty() { "笔记" } else { &dir }
            ));
        }
        std::fs::create_dir_all(&folder).map_err(|e| format!("创建分类失败：{e}"))?;
        std::fs::rename(&from, &to).map_err(|e| format!("移动失败：{e}"))?;
        Ok(to.to_string_lossy().into_owned())
    }

    /// Renames category `from` to `to`; its new path.
    pub fn rename_dir(from: String, to: String) -> Result<String, String> {
        let to = to.trim().to_owned();
        if !plain_name(&from) || !plain_name(&to) {
            return Err("分类名不能为空，也不能包含 / 或以 . 开头".into());
        }
        let target = root_dir().join(&to);
        if target.exists() {
            return Err(format!("已经有分类 {to}"));
        }
        std::fs::rename(root_dir().join(&from), &target)
            .map_err(|e| format!("重命名分类失败：{e}"))?;
        // It keeps its place in the order.
        let mut order = saved_order();
        if let Some(o) = order.iter_mut().find(|o| **o == from) {
            *o = to.clone();
            save_order(&order)?;
        }
        Ok(target.to_string_lossy().into_owned())
    }

    /// Moves the note or category at `path` to the Trash (it can be put back).
    pub async fn trash(path: String) -> Result<(), String> {
        let p = PathBuf::from(&path);
        if !p.starts_with(root_dir()) {
            return Err("只能删除笔记库里的东西".into());
        }
        move_to_trash(p).await.map(|_| ())
    }

    /// Puts the example notes in a library that has no notes (a new one, one
    /// made by hand, one emptied); whether it did.
    pub fn ensure_seeded() -> bool {
        let root = root_dir();
        let mut notes = Vec::new();
        walk(&root, &root, &mut notes);
        if !notes.is_empty() {
            return false;
        }
        for (dir, file, text) in SEEDS {
            let folder = root.join(dir);
            let path = folder.join(file);
            if std::fs::create_dir_all(&folder).is_ok() && !path.exists() {
                let _ = std::fs::write(path, text);
            }
        }
        true
    }
}
