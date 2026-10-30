//! Secrets typed in the app: kept in memory for the window's life, never
//! written to a note (v1 is unsigned, so not in the keychain either).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Each note's typed secrets, by block name.
type Store = HashMap<PathBuf, BTreeMap<String, String>>;

thread_local! {
    static SECRETS: RefCell<Store> = RefCell::new(HashMap::new());
}

/// Keeps `value` for the secret block `name` of the note at `path`.
pub(crate) fn set(path: &Path, name: &str, value: &str) {
    SECRETS.with_borrow_mut(|s| {
        s.entry(path.to_path_buf())
            .or_default()
            .insert(name.to_owned(), value.to_owned());
    });
}

/// The secret typed for block `name` of the note at `path`.
pub(crate) fn get(path: &Path, name: &str) -> Option<String> {
    SECRETS.with_borrow(|s| s.get(path).and_then(|m| m.get(name).cloned()))
}

/// Every secret typed for the note at `path`, as run overrides.
pub(crate) fn overrides(path: &Path) -> BTreeMap<String, String> {
    SECRETS.with_borrow(|s| s.get(path).cloned().unwrap_or_default())
}
