//! Runs a note again as it changes, in the background, for both views.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nsh_core::prelude::*;
use toolbook_core::doc::Document;
use toolbook_core::engine::{Memo, RunOptions, RunReport, run};

/// Runs the note after `delay`, the latest request only. A newer request
/// stops the automatic run before it; a run asked for by hand (⌘↵) is not
/// stopped, and when typing replaced it before it finished, the note runs
/// once more so its results (kept in the memo) show.
#[derive(Clone)]
pub(crate) struct Runner {
    path: PathBuf,
    base_dir: PathBuf,
    memo: Memo,
    generation: Rc<Cell<u64>>,
    /// The text of the latest request.
    latest: Rc<RefCell<String>>,
    /// Stops the automatic run in progress.
    running: Rc<RefCell<Option<Arc<AtomicBool>>>>,
    on_report: Rc<dyn Fn(RunReport)>,
}

impl Runner {
    pub(crate) fn new(path: &Path, on_report: impl Fn(RunReport) + 'static) -> Runner {
        Runner {
            path: path.to_path_buf(),
            base_dir: path.parent().map(Path::to_path_buf).unwrap_or_default(),
            memo: crate::memos::memo_for(path),
            generation: Rc::new(Cell::new(0)),
            latest: Rc::new(RefCell::new(String::new())),
            running: Rc::new(RefCell::new(None)),
            on_report: Rc::new(on_report),
        }
    }

    pub(crate) fn request(&self, markdown: String, delay: Duration, run_manual: bool) {
        let my = self.generation.get() + 1;
        self.generation.set(my);
        *self.latest.borrow_mut() = markdown.clone();
        let this = self.clone();
        spawn(async move {
            if !delay.is_zero() {
                timer(delay).await;
            }
            if this.generation.get() != my {
                return;
            }
            // This run replaces the one in progress.
            if let Some(old) = this.running.borrow_mut().take() {
                old.store(true, Ordering::Relaxed);
            }
            let cancel = (!run_manual).then(|| Arc::new(AtomicBool::new(false)));
            *this.running.borrow_mut() = cancel.clone();
            let memo = this.memo.clone();
            let base_dir = this.base_dir.clone();
            let secrets = crate::secrets::overrides(&this.path);
            let job = background(async move {
                let mut opts = RunOptions::from_process(run_manual);
                opts.memo = Some(memo);
                opts.overrides = secrets;
                opts.cancel = cancel;
                run(&Document::parse(&markdown), &opts, &base_dir)
            });
            let Ok(report) = job.await else { return };
            if this.generation.get() == my {
                if !report.cancelled {
                    (this.on_report)(report);
                }
            } else if run_manual {
                // Typed over while the request was out: show it with the
                // latest text.
                let latest = this.latest.borrow().clone();
                this.request(latest, Duration::ZERO, false);
            }
        });
    }
}
