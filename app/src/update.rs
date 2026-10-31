//! Finding and fetching an update: the newest release's manifest
//! (`latest.json` on the GitHub release), and its disk image, checked against
//! the manifest's SHA-256. Installing it is `install`'s.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const REPO: &str = "https://github.com/wenlingang/toolbook";
pub const MANIFEST_URL: &str = "https://github.com/wenlingang/toolbook/releases/latest/download/latest.json";

/// Why an update could not be found or fetched.
#[derive(Debug)]
pub enum UpdateError {
    Network(String),
    Manifest(String),
    Checksum { expected: String, actual: String },
    Io(std::io::Error),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(e) => write!(f, "网络错误：{e}"),
            Self::Manifest(e) => write!(f, "更新信息不完整：{e}"),
            Self::Checksum { .. } => write!(f, "下载的安装包校验不通过，已删除"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<std::io::Error> for UpdateError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    dmg: String,
    sha256: String,
    /// Size of the disk image in bytes, for the download progress.
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    notes: String,
}

/// A release that can be downloaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// File name of the disk image, `Toolbook-<version>.dmg`.
    pub dmg: String,
    pub dmg_url: String,
    /// Lowercase hex SHA-256 of the disk image.
    pub sha256: String,
    pub size: Option<u64>,
    /// Release notes, plain text; may be empty.
    pub notes: String,
}

/// The manifest at `MANIFEST_URL`, read from `bytes`; the disk image is
/// fetched next to it (`base`: the release's download folder).
pub fn parse_manifest(bytes: &[u8], base: &str) -> Result<Release, UpdateError> {
    let bad = |m: String| UpdateError::Manifest(m);
    let m: Manifest = serde_json::from_slice(bytes).map_err(|e| bad(e.to_string()))?;
    let version = Version::parse(&m.version).map_err(|e| bad(format!("version: {e}")))?;
    if m.dmg != format!("Toolbook-{version}.dmg") {
        return Err(bad(format!("unexpected dmg name {:?}", m.dmg)));
    }
    let sha256 = m.sha256.to_ascii_lowercase();
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(bad(format!("sha256 is not 64 hex digits: {:?}", m.sha256)));
    }
    Ok(Release {
        dmg_url: format!("{}/{}", base.replace("{version}", &version.to_string()), m.dmg),
        version,
        dmg: m.dmg,
        sha256,
        size: m.size.filter(|&size| size > 0),
        notes: m.notes.trim().to_string(),
    })
}

/// `release` if it is newer than the running `current` version.
pub fn newer(current: &str, release: Release) -> Option<Release> {
    let current = Version::parse(current).ok()?;
    (release.version > current).then_some(release)
}

/// Fetches URLs. Blocking: called off the engine's thread.
pub trait Http: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, String>;
    /// Downloads `url` to `to`, calling `progress` with the bytes received so far now and then.
    fn download(&self, url: &str, to: &Path, progress: &dyn Fn(u64)) -> Result<(), String>;
}

/// How often a download reports its progress.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

/// `/usr/bin/curl`, on every Mac, so no HTTP or TLS stack is linked in.
/// HTTPS only, unless `local` (a test's own manifest, from a file).
pub struct Curl {
    pub local: bool,
}

impl Curl {
    fn command(&self, url: &str) -> Command {
        let mut command = Command::new("/usr/bin/curl");
        command
            .args(["--fail", "--silent", "--show-error", "--location"])
            .args(["--proto", if self.local { "=https,file" } else { "=https" }])
            .args(["--connect-timeout", "15"])
            .args(["--user-agent", concat!("Toolbook/", env!("CARGO_PKG_VERSION"))])
            .arg(url);
        command
    }

    fn run(mut command: Command) -> Result<Vec<u8>, String> {
        let out = command.output().map_err(|e| format!("curl: {e}"))?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }
}

impl Http for Curl {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        let mut command = self.command(url);
        command.args(["--max-time", "30"]);
        Self::run(command)
    }

    fn download(&self, url: &str, to: &Path, progress: &dyn Fn(u64)) -> Result<(), String> {
        let mut command = self.command(url);
        command
            .args(["--max-time", "900", "--output"])
            .arg(to)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        let mut child = command.spawn().map_err(|e| format!("curl: {e}"))?;
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| format!("curl: {e}"))? {
                break status;
            }
            if let Ok(meta) = std::fs::metadata(to) {
                progress(meta.len());
            }
            std::thread::sleep(PROGRESS_INTERVAL);
        };
        if status.success() {
            return Ok(());
        }
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stderr.take() {
            let _ = pipe.read_to_string(&mut stderr);
        }
        Err(stderr.trim().to_string())
    }
}

/// Where the manifest is, and where the disk images it names are: the
/// latest GitHub release, or `TOOLBOOK_UPDATE_MANIFEST` (a test's own,
/// with the images next to it).
pub fn source() -> (String, String, bool) {
    match std::env::var("TOOLBOOK_UPDATE_MANIFEST") {
        Ok(url) if !url.is_empty() => {
            let base = url.rsplit_once('/').map(|(b, _)| b.to_owned()).unwrap_or_default();
            (url, base, true)
        }
        _ => (MANIFEST_URL.into(), format!("{REPO}/releases/download/v{{version}}"), false),
    }
}

/// The newest release if it is newer than `current`.
pub fn check(http: &dyn Http, url: &str, base: &str, current: &str) -> Result<Option<Release>, UpdateError> {
    let bytes = http.get(url).map_err(UpdateError::Network)?;
    Ok(newer(current, parse_manifest(&bytes, base)?))
}

/// Downloads the release's disk image into `dir` and checks its SHA-256;
/// returns its path. `progress` gets the bytes received so far.
pub fn download(
    http: &dyn Http,
    release: &Release,
    dir: &Path,
    progress: &dyn Fn(u64),
) -> Result<PathBuf, UpdateError> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(&release.dmg);
    let partial = dir.join(format!("{}.part", release.dmg));
    let _ = std::fs::remove_file(&partial);
    http.download(&release.dmg_url, &partial, progress).map_err(UpdateError::Network)?;
    let actual = sha256_file(&partial)?;
    if actual != release.sha256 {
        let _ = std::fs::remove_file(&partial);
        return Err(UpdateError::Checksum { expected: release.sha256.clone(), actual });
    }
    std::fs::rename(&partial, &path)?;
    Ok(path)
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
