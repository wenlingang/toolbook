use std::path::{Path, PathBuf};
use std::sync::Mutex;

use semver::Version;
use toolbook_app::install::{bundle_of, parse_requirement};
use toolbook_app::update::{Http, MANIFEST_URL, UpdateError, check, download, newer, parse_manifest};

const EMPTY_SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const BASE: &str = "https://github.com/wenlingang/toolbook/releases/download/v{version}";

fn manifest(version: &str, dmg: &str, sha: &str) -> Vec<u8> {
    format!(r#"{{"version":"{version}","dmg":"{dmg}","sha256":"{sha}","notes":" Fixes \n"}}"#).into_bytes()
}

#[test]
fn parse_manifest_builds_the_download_url() {
    let r = parse_manifest(&manifest("0.2.0", "Toolbook-0.2.0.dmg", EMPTY_SHA), BASE).unwrap();
    assert_eq!(r.version, Version::new(0, 2, 0));
    assert_eq!(r.dmg_url, "https://github.com/wenlingang/toolbook/releases/download/v0.2.0/Toolbook-0.2.0.dmg");
    assert_eq!(r.notes, "Fixes");
    assert_eq!(r.size, None);
}

#[test]
fn parse_manifest_reads_the_size_when_present() {
    let json = format!(r#"{{"version":"0.2.0","dmg":"Toolbook-0.2.0.dmg","sha256":"{EMPTY_SHA}","size":2048}}"#);
    assert_eq!(parse_manifest(json.as_bytes(), BASE).unwrap().size, Some(2048));
}

#[test]
fn parse_manifest_rejects_unexpected_names_and_checksums() {
    for (dmg, sha) in [
        ("../Toolbook-0.2.0.dmg", EMPTY_SHA),
        ("Toolbook-0.3.0.dmg", EMPTY_SHA),
        ("Toolbook-0.2.0.dmg", "abc"),
        ("Toolbook-0.2.0.dmg", &"z".repeat(64)),
    ] {
        assert!(parse_manifest(&manifest("0.2.0", dmg, sha), BASE).is_err(), "{dmg} {sha}");
    }
    assert!(parse_manifest(b"not json", BASE).is_err());
    assert!(parse_manifest(&manifest("v0.2", "Toolbook-v0.2.dmg", EMPTY_SHA), BASE).is_err());
}

#[test]
fn newer_compares_semantic_versions() {
    let r = parse_manifest(&manifest("0.10.0", "Toolbook-0.10.0.dmg", EMPTY_SHA), BASE).unwrap();
    assert!(newer("0.9.1", r.clone()).is_some());
    assert!(newer("0.10.0", r.clone()).is_none());
    assert!(newer("1.0.0", r).is_none());
}

/// Serves the manifest and "downloads" fixed bytes.
struct FakeHttp {
    manifest: Result<Vec<u8>, String>,
    file: Vec<u8>,
    urls: Mutex<Vec<String>>,
}

impl Http for FakeHttp {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        self.urls.lock().unwrap().push(url.to_string());
        self.manifest.clone()
    }
    fn download(&self, url: &str, to: &Path, progress: &dyn Fn(u64)) -> Result<(), String> {
        self.urls.lock().unwrap().push(url.to_string());
        progress(self.file.len() as u64);
        std::fs::write(to, &self.file).map_err(|e| e.to_string())
    }
}

fn fake(manifest: Result<Vec<u8>, String>, file: &[u8]) -> FakeHttp {
    FakeHttp { manifest, file: file.to_vec(), urls: Mutex::new(Vec::new()) }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("toolbook-update-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn check_reads_the_latest_manifest() {
    let http = fake(Ok(manifest("0.2.0", "Toolbook-0.2.0.dmg", EMPTY_SHA)), b"");
    let release = check(&http, MANIFEST_URL, BASE, "0.1.0").unwrap().unwrap();
    assert_eq!(release.version, Version::new(0, 2, 0));
    assert_eq!(*http.urls.lock().unwrap(), [MANIFEST_URL]);
    assert!(check(&http, MANIFEST_URL, BASE, "0.2.0").unwrap().is_none());
}

#[test]
fn check_reports_network_errors() {
    let http = fake(Err("404".into()), b"");
    assert!(matches!(check(&http, MANIFEST_URL, BASE, "0.1.0"), Err(UpdateError::Network(_))));
}

#[test]
fn download_keeps_a_file_with_the_expected_checksum() {
    let http = fake(Ok(Vec::new()), b"");
    let release = parse_manifest(&manifest("0.2.0", "Toolbook-0.2.0.dmg", EMPTY_SHA), BASE).unwrap();
    let dir = temp_dir("ok");
    let path = download(&http, &release, &dir, &|_| {}).unwrap();
    assert_eq!(path, dir.join("Toolbook-0.2.0.dmg"));
    assert!(path.exists());
    assert_eq!(http.urls.lock().unwrap()[0], release.dmg_url);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn download_deletes_a_file_with_the_wrong_checksum() {
    let http = fake(Ok(Vec::new()), b"tampered");
    let release = parse_manifest(&manifest("0.2.0", "Toolbook-0.2.0.dmg", EMPTY_SHA), BASE).unwrap();
    let dir = temp_dir("bad");
    let err = download(&http, &release, &dir, &|_| {}).unwrap_err();
    assert!(matches!(err, UpdateError::Checksum { .. }), "{err}");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_requirement_must_pin_a_certificate() {
    let signed = "Executable=/Applications/Toolbook.app/Contents/MacOS/Toolbook\ndesignated => identifier \"io.github.wenlingang.toolbook\" and certificate leaf = H\"264a\"\n";
    assert_eq!(
        parse_requirement(signed).as_deref(),
        Some("identifier \"io.github.wenlingang.toolbook\" and certificate leaf = H\"264a\"")
    );
    // Ad hoc: only a cdhash, which every build changes.
    assert_eq!(parse_requirement("designated => cdhash H\"8b7f\"\n"), None);
    assert_eq!(parse_requirement(""), None);
}

#[test]
fn bundle_of_finds_the_app_around_its_executable() {
    assert_eq!(
        bundle_of(Path::new("/Applications/Toolbook.app/Contents/MacOS/Toolbook")),
        Some(PathBuf::from("/Applications/Toolbook.app"))
    );
    assert_eq!(bundle_of(Path::new("/repo/target/debug/toolbook")), None);
    assert_eq!(bundle_of(Path::new("/x/Contents/MacOS/Toolbook")), None);
}

#[test]
fn a_test_manifest_names_images_next_to_it() {
    // SAFETY: the only test in this binary that touches the environment.
    unsafe { std::env::set_var("TOOLBOOK_UPDATE_MANIFEST", "file:///tmp/feed/latest.json") };
    let (url, base, local) = toolbook_app::update::source();
    unsafe { std::env::remove_var("TOOLBOOK_UPDATE_MANIFEST") };
    assert_eq!((url.as_str(), base.as_str(), local), ("file:///tmp/feed/latest.json", "file:///tmp/feed", true));
    let r = parse_manifest(&manifest("0.2.0", "Toolbook-0.2.0.dmg", EMPTY_SHA), &base).unwrap();
    assert_eq!(r.dmg_url, "file:///tmp/feed/Toolbook-0.2.0.dmg");
}
