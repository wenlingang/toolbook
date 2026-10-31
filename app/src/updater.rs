//! In-app updates for the TSX shell, which owns the menu item, the timer and
//! the dialogs: find a newer release, download and install it, relaunch.

use std::path::PathBuf;
use std::sync::Mutex;

use nsh_core::element::blocking;
use nsh_core::prelude::*;

use crate::install::{self, InstallError};
use crate::update::{self, Curl, Release};

/// The release the last check found, for `install`.
static FOUND: Mutex<Option<Release>> = Mutex::new(None);
/// What `install` is doing now, for the window's status line.
static STATUS: Mutex<String> = Mutex::new(String::new());

fn set_status(s: String) {
    *STATUS.lock().unwrap() = s;
}

/// A newer release.
#[derive(Debug, Clone, PartialEq, JsObject)]
pub struct UpdateInfo {
    /// Its version, `0.2.0`.
    pub version: String,
    /// Its release notes, plain text; may be empty.
    pub notes: String,
}

/// "正在下载 45%" when the size is known, "正在下载 12.3 MB" otherwise.
fn downloading(bytes: u64, size: Option<u64>) -> String {
    match size {
        Some(size) => format!("正在下载更新 {}%", bytes.min(size) * 100 / size),
        None => format!("正在下载更新 {:.1} MB", bytes as f64 / 1_000_000.0),
    }
}

/// The running app's bundle; none when not run from one.
fn running_app() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|exe| install::bundle_of(&exe))
}

/// In-app updates.
#[module]
pub struct Updater;

#[module]
impl Updater {
    /// This build's version.
    pub fn current() -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    /// The newest release when it is newer than this build, else nothing.
    pub async fn check() -> Result<Option<UpdateInfo>, String> {
        let (url, base, local) = update::source();
        let found = blocking(move || update::check(&Curl { local }, &url, &base, env!("CARGO_PKG_VERSION")))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let info = found.as_ref().map(|r| UpdateInfo { version: r.version.to_string(), notes: r.notes.clone() });
        *FOUND.lock().unwrap() = found;
        Ok(info)
    }

    /// Downloads the release the last check found and installs it over the
    /// running app (which keeps running until `relaunch`).
    pub async fn install() -> Result<(), String> {
        let release = FOUND.lock().unwrap().clone().ok_or("没有可安装的更新")?;
        let local = update::source().2;
        let result = blocking(move || {
            let app = running_app().ok_or(InstallError::NotInApp.to_string())?;
            let dir = std::env::temp_dir().join("toolbook-update");
            set_status(downloading(0, release.size));
            let package = update::download(&Curl { local }, &release, &dir, &|bytes| {
                set_status(downloading(bytes, release.size))
            })
            .map_err(|e| e.to_string())?;
            set_status("正在安装更新…".into());
            let installed = install::install(&package, &app, &release.version.to_string());
            let _ = std::fs::remove_dir_all(&dir);
            installed.map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);
        set_status(String::new());
        result
    }

    /// What an install is doing now; empty when none is running.
    pub fn status() -> String {
        STATUS.lock().unwrap().clone()
    }

    /// Opens the installed app once this one has quit, and quits.
    pub fn relaunch() -> Result<(), String> {
        let app = running_app().ok_or(InstallError::NotInApp.to_string())?;
        install::relaunch(&app).map_err(|e| e.to_string())?;
        install::quit();
        Ok(())
    }
}
