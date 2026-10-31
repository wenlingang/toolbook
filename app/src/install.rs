//! Installing an update: mount the downloaded disk image, copy the app next
//! to the running one, check its signature against the running app's
//! designated requirement, swap the bundles, then relaunch.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const APP_NAME: &str = "Toolbook.app";
/// Bundles the installer puts next to the app while it works
/// (`<prefix>update-<pid>.app`, …).
const LEFTOVER_PREFIX: &str = ".Toolbook-";

/// Why a downloaded update could not be installed.
#[derive(Debug)]
pub enum InstallError {
    /// Not running from an app bundle (`nsh run`).
    NotInApp,
    /// macOS runs the app from a read-only random path (App Translocation):
    /// it was opened where it was downloaded instead of from Applications.
    Translocated(PathBuf),
    /// The running app's signature pins no certificate (ad hoc or none), so
    /// there is nothing to check the update against.
    Unsigned(String),
    Package(String),
    /// The update is not signed by the running app's certificate.
    Signature(String),
    Replace { path: PathBuf, detail: String },
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInApp => write!(f, "Toolbook 不是从应用程序包里运行的（开发版），不能自动更新"),
            Self::Translocated(_) => write!(f, "请先把 Toolbook 拖进「应用程序」文件夹再打开，之后才能自动更新"),
            Self::Unsigned(_) => write!(f, "这个 Toolbook 没有正式签名（自己构建的版本），不能自动更新，请从发布页下载"),
            Self::Package(e) => write!(f, "安装包有问题：{e}"),
            Self::Signature(_) => write!(f, "安装包的签名和当前应用不一致，已拒绝安装"),
            Self::Replace { path, detail } => write!(f, "无法替换 {}：{detail}", path.display()),
        }
    }
}

/// The `.app` bundle around the executable `exe`
/// (`…/Toolbook.app/Contents/MacOS/Toolbook`).
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    let is_app = app.extension().is_some_and(|e| e == "app");
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && is_app).then(|| app.to_path_buf())
}

/// Installs the app inside the disk image `package` over the bundle `app`,
/// after checking that it is signed by the same certificate as `app` and is
/// `version` (so an old, validly signed build cannot pass for a newer one).
/// Blocking.
pub fn install(package: &Path, app: &Path, version: &str) -> Result<(), InstallError> {
    if app.components().any(|c| c.as_os_str() == "AppTranslocation") {
        return Err(InstallError::Translocated(app.to_path_buf()));
    }
    let requirement = designated_requirement(app)?;
    let parent = app
        .parent()
        .ok_or_else(|| InstallError::Package(format!("{} has no parent folder", app.display())))?;
    remove_leftovers(parent);
    let pid = std::process::id();
    let staged = parent.join(format!("{LEFTOVER_PREFIX}update-{pid}.app"));
    let old = parent.join(format!("{LEFTOVER_PREFIX}old-{pid}.app"));

    let result = copy_out_of(package, &staged)
        .and_then(|()| verify(&staged, &requirement))
        .and_then(|()| check_version(&staged, version))
        .and_then(|()| swap(app, &staged, &old));
    let _ = std::fs::remove_dir_all(&staged);
    result
}

/// Deletes bundles left by an install that was stopped half-way.
fn remove_leftovers(parent: &Path) {
    let Ok(entries) = std::fs::read_dir(parent) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(LEFTOVER_PREFIX) && name.ends_with(".app") {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// The running app's designated requirement, e.g.
/// `identifier "io.github.wenlingang.toolbook" and certificate leaf = H"…"`.
fn designated_requirement(app: &Path) -> Result<String, InstallError> {
    let out = run(Command::new("/usr/bin/codesign").arg("-d").arg("-r-").arg(app)).map_err(InstallError::Unsigned)?;
    // codesign prints the requirement on stdout, other details on stderr.
    let text = String::from_utf8_lossy(&out.stdout);
    parse_requirement(&text).ok_or_else(|| InstallError::Unsigned(text.trim().to_string()))
}

/// The `designated => …` line, when it pins a certificate (an ad hoc
/// signature only pins a cdhash, which every build changes).
pub fn parse_requirement(text: &str) -> Option<String> {
    let requirement = text.lines().find_map(|line| line.trim().strip_prefix("designated => "))?;
    requirement.contains("certificate ").then(|| requirement.to_string())
}

fn copy_out_of(package: &Path, staged: &Path) -> Result<(), InstallError> {
    let mount = std::env::temp_dir().join(format!("toolbook-mount-{}", std::process::id()));
    std::fs::create_dir_all(&mount).map_err(|e| InstallError::Package(e.to_string()))?;
    let copied = run(Command::new("/usr/bin/hdiutil")
        .args(["attach", "-quiet", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"])
        .arg(&mount)
        .arg(package))
    .and_then(|_| {
        let copied = run(Command::new("/usr/bin/ditto").arg(mount.join(APP_NAME)).arg(staged));
        let _ = run(Command::new("/usr/bin/hdiutil").args(["detach", "-quiet", "-force"]).arg(&mount));
        copied
    });
    let _ = std::fs::remove_dir(&mount);
    copied.map(|_| ()).map_err(InstallError::Package)
}

fn verify(staged: &Path, requirement: &str) -> Result<(), InstallError> {
    run(Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(format!("-R={requirement}"))
        .arg(staged))
    .map(|_| ())
    .map_err(InstallError::Signature)
}

fn check_version(staged: &Path, expected: &str) -> Result<(), InstallError> {
    let out = run(Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleShortVersionString"])
        .arg(staged.join("Contents/Info.plist")))
    .map_err(InstallError::Package)?;
    let found = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if found == expected {
        Ok(())
    } else {
        Err(InstallError::Package(format!("安装包里是 {found}，应为 {expected}")))
    }
}

/// Moves the running app aside, the new one into its place, then deletes
/// the old one. The running process keeps working: its open files stay.
fn swap(app: &Path, staged: &Path, old: &Path) -> Result<(), InstallError> {
    let replace_error = |e: std::io::Error| InstallError::Replace { path: app.to_path_buf(), detail: e.to_string() };
    std::fs::rename(app, old).map_err(replace_error)?;
    if let Err(e) = std::fs::rename(staged, app) {
        let _ = std::fs::rename(old, app);
        return Err(replace_error(e));
    }
    let _ = std::fs::remove_dir_all(old);
    Ok(())
}

/// Runs a command; on failure, its stderr (or why it did not start).
fn run(command: &mut Command) -> Result<Output, String> {
    let out = command.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(out)
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if stderr.is_empty() { out.status.to_string() } else { stderr })
    }
}

/// Opens `app` once this process has exited (`open` would otherwise only
/// bring this one to the front).
pub fn relaunch(app: &Path) -> std::io::Result<()> {
    Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"while kill -0 "$0" 2>/dev/null; do sleep 0.2; done; exec /usr/bin/open "$1""#)
        .arg(std::process::id().to_string())
        .arg(app)
        .spawn()
        .map(|_| ())
}

/// Quits the app as ⌘Q does (AppKit's `terminate:`), from the main thread;
/// elsewhere the process just exits.
pub fn quit() {
    match objc2::MainThreadMarker::new() {
        Some(mtm) => objc2_app_kit::NSApplication::sharedApplication(mtm).terminate(None),
        None => std::process::exit(0),
    }
}
