//! Each note's kept results, shared by its two views, so a request run in
//! one shows in the other without being sent again.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use toolbook_core::engine::Memo;

thread_local! {
    static MEMOS: RefCell<HashMap<PathBuf, Memo>> = RefCell::new(HashMap::new());
}

/// The kept results of the note at `path`.
pub(crate) fn memo_for(path: &Path) -> Memo {
    MEMOS.with_borrow_mut(|m| m.entry(path.to_path_buf()).or_default().clone())
}
