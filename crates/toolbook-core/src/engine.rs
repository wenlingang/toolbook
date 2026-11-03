//! Running a note: blocks in order, each after the blocks it refers to.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::doc::{Block, Document};
use crate::fence::{Attr, VarRef};
use crate::path::get_path;
use crate::template::{render_template, template_refs};
use crate::tools::{ToolCtx, ToolError, tool};
use crate::value::Value;

/// Results kept between runs, by what produced them (tool, info string,
/// content and the inputs' own keys): a block whose inputs did not change
/// is not computed again, and a block run by hand (`http`) keeps its
/// result for the automatic runs after it.
#[derive(Debug, Clone, Default)]
pub struct Memo(Arc<Mutex<MemoInner>>);

#[derive(Debug, Default)]
struct MemoInner {
    entries: HashMap<u64, MemoEntry>,
    /// The latest run; entries remember the last run that used them.
    run: u64,
}

#[derive(Debug)]
struct MemoEntry {
    name: Option<String>,
    status: BlockStatus,
    notes: Vec<String>,
    used: u64,
}

/// Past this many results, a run keeps only what it used: typing makes a new
/// result per keystroke, and one can hold megabytes.
const MEMO_LIMIT: usize = 256;

impl Memo {
    /// How many results it keeps.
    pub fn len(&self) -> usize {
        self.0.lock().map(|m| m.entries.len()).unwrap_or(0)
    }

    /// Whether it keeps nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Forgets everything.
    pub fn clear(&self) {
        if let Ok(mut m) = self.0.lock() {
            m.entries.clear();
        }
    }

    fn begin_run(&self) {
        if let Ok(mut m) = self.0.lock() {
            m.run += 1;
        }
    }

    fn end_run(&self) {
        if let Ok(mut m) = self.0.lock()
            && m.entries.len() > MEMO_LIMIT
        {
            let run = m.run;
            m.entries.retain(|_, e| e.used == run);
        }
    }

    fn get(&self, key: u64) -> Option<(BlockStatus, Vec<String>)> {
        let mut m = self.0.lock().ok()?;
        let run = m.run;
        let e = m.entries.get_mut(&key)?;
        e.used = run;
        Some((e.status.clone(), e.notes.clone()))
    }

    fn put(&self, key: u64, name: Option<String>, status: BlockStatus, notes: Vec<String>) {
        if let Ok(mut m) = self.0.lock() {
            let used = m.run;
            m.entries.insert(key, MemoEntry { name, status, notes, used });
        }
    }

    /// Replaces the kept result of every block named `name` (tests).
    #[doc(hidden)]
    pub fn overwrite_all_for_test(&self, name: &str, value: Value) {
        if let Ok(mut m) = self.0.lock() {
            for e in m.entries.values_mut() {
                if e.name.as_deref() == Some(name) {
                    e.status = BlockStatus::Ok(value.clone());
                }
            }
        }
    }
}

/// Tools whose result depends on more than their inputs: the clock, files,
/// the environment.
const NOT_MEMOIZED: &[&str] = &["timestamp", "flow", "secret"];

fn block_key(info: &str, content: &str, deps: &[u64]) -> u64 {
    let mut h = DefaultHasher::new();
    info.hash(&mut h);
    content.hash(&mut h);
    deps.hash(&mut h);
    h.finish()
}

fn status_key(status: &BlockStatus) -> u64 {
    let mut h = DefaultHasher::new();
    format!("{status:?}").hash(&mut h);
    h.finish()
}

/// What a run is given from outside the note.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Values for `param` blocks (and `secret` blocks), by name.
    pub overrides: BTreeMap<String, String>,
    /// Whether tools that wait to be run by hand (`http`) run.
    pub run_manual: bool,
    /// The time "now", in Unix milliseconds.
    pub now_ms: i64,
    /// The environment variables the run may read.
    pub env: BTreeMap<String, String>,
    /// The library directory (`~/Toolbook`).
    pub library: PathBuf,
    /// Results kept between runs, when the caller keeps them.
    pub memo: Option<Memo>,
    /// Set by the caller to stop the run before its next block (a newer run
    /// replaces it).
    pub cancel: Option<Arc<AtomicBool>>,
}

impl RunOptions {
    /// Options from this process: the clock, the environment, and the
    /// library at `TOOLBOOK_HOME` or `~/Toolbook`.
    pub fn from_process(run_manual: bool) -> RunOptions {
        let env: BTreeMap<String, String> = std::env::vars().collect();
        let library = env
            .get("TOOLBOOK_HOME")
            .map(PathBuf::from)
            .or_else(|| env.get("HOME").map(|h| Path::new(h).join("Toolbook")))
            .unwrap_or_else(|| PathBuf::from("Toolbook"));
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        RunOptions {
            overrides: BTreeMap::new(),
            run_manual,
            now_ms,
            env,
            library,
            memo: None,
            cancel: None,
        }
    }
}

/// How a block came out.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockStatus {
    /// Its value.
    Ok(Value),
    /// Why it failed.
    Error(String),
    /// A block it refers to did not succeed.
    Upstream {
        /// That block's index.
        block: usize,
        /// The name it was referred to by.
        name: String,
    },
    /// Its input is missing or not finished.
    Incomplete(String),
    /// It waits to be run by hand.
    NotRun,
}

/// A block's result.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockResult {
    /// The block's index (0-based).
    pub index: usize,
    /// Its name.
    pub name: Option<String>,
    /// Its tool.
    pub tool: String,
    /// How it came out.
    pub status: BlockStatus,
    /// Remarks the tool made.
    pub notes: Vec<String>,
    /// The values of secrets its result may carry from another note (a
    /// `flow` whose note reads one): never shown where it is shared.
    pub secrets: Vec<String>,
}

/// Every block's result, in order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunReport {
    /// The results.
    pub blocks: Vec<BlockResult>,
    /// Whether the run was stopped ([`RunOptions::cancel`]): `blocks` holds
    /// only the blocks before that.
    pub cancelled: bool,
}

impl RunReport {
    /// The block that went wrong first, following `Upstream` back from block `index`, and why.
    pub fn root_cause(&self, mut index: usize) -> (usize, String) {
        loop {
            match &self.blocks[index].status {
                BlockStatus::Upstream { block, .. } => index = *block,
                BlockStatus::Error(m) | BlockStatus::Incomplete(m) => return (index, m.clone()),
                BlockStatus::NotRun => return (index, "没有运行".into()),
                BlockStatus::Ok(_) => return (index, "没有输出".into()),
            }
        }
    }

    /// The block named `name`, or the last named block.
    pub fn output(&self, name: Option<&str>) -> Option<&BlockResult> {
        match name {
            Some(n) => self.blocks.iter().find(|b| b.name.as_deref() == Some(n)),
            None => self.blocks.iter().rev().find(|b| b.name.is_some()),
        }
    }
}

/// Runs a note.
pub fn run(doc: &Document, opts: &RunOptions, base_dir: &Path) -> RunReport {
    if let Some(memo) = &opts.memo {
        memo.begin_run();
    }
    let report = run_with_stack(doc, opts, base_dir, &[]);
    // A stopped run used only part of the note: keep what it did not reach.
    if let Some(memo) = &opts.memo
        && !report.cancelled
    {
        memo.end_run();
    }
    report
}

/// The `$name` identifiers in JS code that name earlier blocks.
fn js_refs(content: &str, defined: &HashMap<String, usize>) -> Vec<VarRef> {
    let mut out: Vec<VarRef> = Vec::new();
    let bytes: Vec<char> = content.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '$' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_alphanumeric() || bytes[end] == '_') {
                end += 1;
            }
            let name: String = bytes[start..end].iter().collect();
            if defined.contains_key(&name) && !out.iter().any(|r| r.name == name) {
                out.push(VarRef { name, path: Vec::new() });
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

/// Runs a note inside the notes in `stack` (a `flow` calling it).
pub fn run_with_stack(
    doc: &Document,
    opts: &RunOptions,
    base_dir: &Path,
    stack: &[PathBuf],
) -> RunReport {
    let mut report = RunReport::default();
    let mut defined: HashMap<String, usize> = HashMap::new();
    let mut vars: BTreeMap<String, Value> = BTreeMap::new();
    let mut keys: Vec<u64> = Vec::new();

    for (index, block) in doc.blocks() {
        if opts.cancel.as_ref().is_some_and(|c| c.load(Ordering::Relaxed)) {
            report.cancelled = true;
            break;
        }
        let (status, notes, secrets, key) = run_block(block, opts, base_dir, stack, &defined, &vars, &report, &keys);
        keys.push(key);
        if let Some(name) = &block.info.name
            && !defined.contains_key(name)
        {
            defined.insert(name.clone(), index);
            if let BlockStatus::Ok(v) = &status {
                vars.insert(name.clone(), v.clone());
            }
        }
        report.blocks.push(BlockResult {
            index,
            name: block.info.name.clone(),
            tool: block.info.tool.clone(),
            status,
            notes,
            secrets,
        });
    }
    report
}

#[allow(clippy::too_many_arguments)]
fn run_block(
    block: &Block,
    opts: &RunOptions,
    base_dir: &Path,
    stack: &[PathBuf],
    defined: &HashMap<String, usize>,
    vars: &BTreeMap<String, Value>,
    report: &RunReport,
    keys: &[u64],
) -> (BlockStatus, Vec<String>, Vec<String>, u64) {
    let (status, notes, secrets, key) = run_block_inner(block, opts, base_dir, stack, defined, vars, report, keys);
    // A block computed without a key is keyed by its result.
    let key = key.unwrap_or_else(|| status_key(&status));
    (status, notes, secrets, key)
}

#[allow(clippy::too_many_arguments)]
fn run_block_inner(
    block: &Block,
    opts: &RunOptions,
    base_dir: &Path,
    stack: &[PathBuf],
    defined: &HashMap<String, usize>,
    vars: &BTreeMap<String, Value>,
    report: &RunReport,
    keys: &[u64],
) -> (BlockStatus, Vec<String>, Vec<String>, Option<u64>) {
    let info = &block.info;
    if let Some(name) = &info.name
        && let Some(first) = defined.get(name)
    {
        return (
            BlockStatus::Error(format!("块名 {name} 重复（第 {} 块已使用）", first + 1)),
            Vec::new(),
            Vec::new(),
            None,
        );
    }
    if let Some(e) = info.errors.first() {
        return (BlockStatus::Error(e.clone()), Vec::new(), Vec::new(), None);
    }
    let Some(t) = tool(&info.tool) else {
        return (BlockStatus::Error(format!("未知工具 {}", info.tool)), Vec::new(), Vec::new(), None);
    };

    let mut content = block.content.clone();
    if info.has_flag("param")
        && let Some(v) = info.name.as_ref().and_then(|n| opts.overrides.get(n))
    {
        content = v.clone();
    }
    if let Err(e) = t.check(info, &content) {
        let status = match e {
            ToolError::Incomplete(m) => BlockStatus::Incomplete(m),
            ToolError::Failed(m) => BlockStatus::Error(m),
        };
        return (status, Vec::new(), Vec::new(), None);
    }

    let mut deps: Vec<VarRef> = info.refs().cloned().collect();
    if t.template() {
        deps.extend(template_refs(&content));
    }
    if info.tool == "js" {
        deps.extend(js_refs(&content, defined));
    }
    for r in &deps {
        let Some(&at) = defined.get(&r.name) else {
            return (
                BlockStatus::Error(format!("引用了 ${}，但它没有在前面定义", r.name)),
                Vec::new(),
                Vec::new(),
                None,
            );
        };
        // A request not run yet: this block waits for ⌘↵ with it.
        if report.blocks[at].status == BlockStatus::NotRun {
            return (BlockStatus::NotRun, Vec::new(), Vec::new(), None);
        }
        if !matches!(report.blocks[at].status, BlockStatus::Ok(_)) {
            return (
                BlockStatus::Upstream { block: at, name: r.name.clone() },
                Vec::new(),
                Vec::new(),
                None,
            );
        }
    }
    // js that reads the clock or draws random numbers gives a new result each run.
    let volatile = NOT_MEMOIZED.contains(&info.tool.as_str())
        || info.tool == "js" && (content.contains("Date") || content.contains("Math.random"));
    let memoized = opts.memo.as_ref().filter(|_| !volatile);
    let dep_keys: Vec<u64> = deps.iter().filter_map(|r| defined.get(&r.name).map(|&i| keys[i])).collect();
    let key = block_key(block.info_str(), &content, &dep_keys);
    // A block run by hand runs every time it is asked to (⌘↵ again, a retry).
    let asked_by_hand = t.manual() && opts.run_manual;
    if let Some(memo) = memoized
        && !asked_by_hand
        && let Some((status, notes)) = memo.get(key)
        && status != BlockStatus::NotRun
    {
        return (status, notes, Vec::new(), Some(key));
    }
    if t.manual() && !opts.run_manual {
        if let Some(memo) = memoized {
            memo.put(key, info.name.clone(), BlockStatus::NotRun, Vec::new());
        }
        return (BlockStatus::NotRun, Vec::new(), Vec::new(), Some(key));
    }

    let lookup = |r: &VarRef| -> Result<Value, String> {
        let v = vars
            .get(&r.name)
            .ok_or_else(|| format!("引用了 {}，但它没有在前面定义", r.display()))?;
        get_path(v, &r.path)
    };
    let mut attrs = BTreeMap::new();
    for (k, a) in &info.attrs {
        let v = match a {
            Attr::Lit(s) => Value::Text(s.clone()),
            Attr::Ref(r) => match lookup(r) {
                Ok(v) => v,
                Err(e) => return (BlockStatus::Error(e), Vec::new(), Vec::new(), None),
            },
        };
        attrs.insert(k.clone(), v);
    }
    if t.template() {
        match render_template(&content, &lookup) {
            Ok(c) => content = c,
            Err(e) => return (BlockStatus::Error(e), Vec::new(), Vec::new(), None),
        }
    }
    let primary = match attrs.get("input") {
        Some(v) => Some(v.clone()),
        None if !content.trim().is_empty() => Some(Value::Text(content.clone())),
        None => None,
    };

    // Only `js` reads earlier blocks by name, and only those it mentions.
    let js_vars: BTreeMap<String, Value> = if info.tool == "js" {
        deps.iter().filter_map(|r| vars.get(&r.name).map(|v| (r.name.clone(), v.clone()))).collect()
    } else {
        BTreeMap::new()
    };
    let ctx = ToolCtx {
        block,
        content,
        primary,
        attrs,
        vars: js_vars,
        opts,
        base_dir,
        stack,
        notes: RefCell::new(Vec::new()),
        secrets: RefCell::new(Vec::new()),
    };
    let status = match t.run(&ctx) {
        Ok(v) => BlockStatus::Ok(v),
        Err(ToolError::Incomplete(m)) => BlockStatus::Incomplete(m),
        Err(ToolError::Failed(m)) => BlockStatus::Error(m),
    };
    let notes = ctx.notes.into_inner();
    let secrets = ctx.secrets.into_inner();
    if let Some(memo) = memoized {
        memo.put(key, info.name.clone(), status.clone(), notes.clone());
        return (status, notes, secrets, Some(key));
    }
    // Not kept (the clock, files): keyed by the result, so what follows
    // is computed again only when it changed.
    (status, notes, secrets, None)
}
