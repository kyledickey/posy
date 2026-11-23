//! Checking for a newer posy and swapping it in.
//!
//! Releases live on GitHub as one archive per kind of computer. Rather
//! than build networking into posy, this borrows `curl` and `tar`, which
//! every Mac and Linux machine already has, and Windows has too since
//! Windows 10.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use crate::{config, sha256};

pub const REPO: &str = "kyledickey/posy";
const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// How long to wait between quiet checks.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
const PINK: &str = "\x1b[38;2;246;122;172m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// The release archive built for this kind of computer.
fn target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-musl"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-musl"),
        ("linux", "arm") => Some("armv7-unknown-linux-musleabihf"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        ("windows", "aarch64") => Some("aarch64-pc-windows-msvc"),
        _ => None,
    }
}

/// The release file for a target: a zip on Windows, a tarball elsewhere.
fn archive_name(target: &str) -> String {
    if target.contains("windows") {
        format!("posy-{target}.zip")
    } else {
        format!("posy-{target}.tar.gz")
    }
}

/// Where curl can throw away what it downloads.
const NOWHERE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };

/// "1.2.3" (or "v1.2.3") as numbers, so versions compare properly.
fn parse(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.trim().trim_start_matches('v').split('.');
    let mut next = || parts.next()?.parse().ok();
    let version = (next()?, next()?, next()?);
    parts.next().is_none().then_some(version)
}

fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(l), Some(c)) if l > c)
}

fn curl(args: &[&str]) -> Command {
    let mut cmd = Command::new("curl");
    cmd.args(["--fail", "--silent", "--show-error", "--location"]);
    cmd.args(args);
    cmd
}

/// The newest released version, like "0.2.0".
///
/// GitHub redirects `/releases/latest` to the newest release's page, so
/// the address we end up at names the version. A private repository has
/// no such page for strangers; there, your `gh` login is used instead.
fn latest_version(timeout_secs: u32) -> Result<String> {
    let url = format!("https://github.com/{REPO}/releases/latest");
    let timeout = timeout_secs.to_string();
    let out = curl(&["--max-time", &timeout, "--output", NOWHERE])
        .args(["--write-out", "%{url_effective}", &url])
        .stderr(Stdio::null())
        .output()
        .context("couldn't run curl")?;
    let landed = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success()
        && let Some((_, tag)) = landed.rsplit_once("/tag/")
        && parse(tag).is_some()
    {
        return Ok(tag.trim_start_matches('v').to_string());
    }
    let out = Command::new("gh")
        .args(["release", "view", "--repo", REPO])
        .args(["--json", "tagName", "--jq", ".tagName"])
        .stderr(Stdio::null())
        .output();
    if let Ok(out) = out
        && out.status.success()
    {
        let tag = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if parse(&tag).is_some() {
            return Ok(tag.trim_start_matches('v').to_string());
        }
    }
    bail!("couldn't reach GitHub to look for a newer posy")
}

/// Fetches one file of a release into `dir`.
fn download(version: &str, name: &str, dir: &Path) -> Result<PathBuf> {
    let path = dir.join(name);
    let url = format!("https://github.com/{REPO}/releases/download/v{version}/{name}");
    let fetched = curl(&["--output"])
        .arg(&path)
        .arg(&url)
        .stderr(Stdio::null())
        .status()
        .context("couldn't run curl")?;
    if fetched.success() {
        return Ok(path);
    }
    // A private repository: fall back to the GitHub CLI, if it is set up.
    let tag = format!("v{version}");
    let fetched = Command::new("gh")
        .args(["release", "download", &tag, "--repo", REPO])
        .args(["--pattern", name, "--clobber", "--dir"])
        .arg(dir)
        .stderr(Stdio::null())
        .status();
    if matches!(fetched, Ok(status) if status.success()) {
        return Ok(path);
    }
    bail!("couldn't download {name} from {url}")
}

/// `posy update`: looks for a newer version and, unless only checking,
/// replaces this program with it.
pub fn run(check_only: bool) -> Result<()> {
    let latest = latest_version(15)?;
    if !is_newer(&latest, CURRENT) {
        println!("{PINK}♥{RESET} posy {CURRENT} is the newest there is");
        return Ok(());
    }
    if check_only {
        println!("{PINK}✿{RESET} posy {latest} is out (you have {CURRENT}): run `posy update`");
        return Ok(());
    }
    let Some(target) = target() else {
        bail!(
            "there is no ready-made posy for this kind of computer; \
             build it with `cargo install --git https://github.com/{REPO}`"
        );
    };
    let exe = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .context("couldn't work out where posy is installed")?;
    let folder = exe.parent().context("posy has no folder")?;

    println!("{PINK}✿{RESET} updating posy {CURRENT} → {latest}");
    let dir = std::env::temp_dir().join(format!("posy-update-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    let result = install(&latest, target, &dir, &exe, folder);
    let _ = fs::remove_dir_all(&dir);
    result?;
    println!("{PINK}♥{RESET} posy is now {latest}");
    Ok(())
}

fn install(version: &str, target: &str, dir: &Path, exe: &Path, folder: &Path) -> Result<()> {
    let archive_name = archive_name(target);
    let archive = download(version, &archive_name, dir)?;
    let sums = fs::read_to_string(download(version, "checksums.txt", dir)?)?;
    let expected = sums
        .lines()
        .filter_map(|line| line.split_once(char::is_whitespace))
        .find(|(_, name)| name.trim().trim_start_matches('*') == archive_name)
        .map(|(sum, _)| sum.to_lowercase())
        .context("the release has no checksum for this download")?;
    if sha256::hex(&fs::read(&archive)?) != expected {
        bail!("the download doesn't match its checksum; nothing was changed");
    }

    // Windows' own tar reads zips as well as tarballs.
    let unpacked = Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(dir)
        .status()
        .context("couldn't run tar")?;
    let fresh = dir.join(format!("posy{}", std::env::consts::EXE_SUFFIX));
    if !unpacked.success() || !fresh.is_file() {
        bail!("couldn't unpack the download");
    }

    // Put the new one next to the old, then swap it in with a rename,
    // which either fully happens or doesn't: posy is never left half
    // written.
    let staged = folder.join(".posy-new");
    fs::copy(&fresh, &staged).with_context(|| {
        format!(
            "couldn't write to {} (if posy lives somewhere protected, \
             run the update with sudo)",
            folder.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))?;
    }
    // Windows won't let a running program be replaced, but it will let
    // it be renamed: move this one aside first. The old copy is still
    // running, so it can only be tidied away by the next update.
    #[cfg(windows)]
    {
        let old = folder.join(".posy-old.exe");
        let _ = fs::remove_file(&old);
        fs::rename(exe, &old).with_context(|| format!("couldn't move {} aside", exe.display()))?;
        if let Err(err) = fs::rename(&staged, exe) {
            let _ = fs::rename(&old, exe);
            return Err(err).with_context(|| format!("couldn't replace {}", exe.display()));
        }
    }
    #[cfg(not(windows))]
    fs::rename(&staged, exe).with_context(|| format!("couldn't replace {}", exe.display()))?;
    Ok(())
}

/// Where the time and result of the last quiet check are kept.
fn stamp_path() -> Option<PathBuf> {
    Some(config::config_dir()?.join("update-check"))
}

/// Looks for a newer version in the background, at most once a day, and
/// never gets in the way: no network, no answer. The answer, if there is
/// one by the time posy closes, is a line to print.
pub fn check_quietly() -> Receiver<String> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let Some(stamp) = stamp_path() else { return };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        // The stamp is "when we looked" and "what was newest then".
        let known = fs::read_to_string(&stamp).ok().and_then(|text| {
            let (when, version) = text.trim().split_once(' ')?;
            Some((when.parse::<u64>().ok()?, version.to_string()))
        });
        let latest = match known {
            Some((when, version)) if now.saturating_sub(when) < CHECK_EVERY.as_secs() => version,
            _ => {
                let Ok(version) = latest_version(4) else {
                    return;
                };
                if let Some(folder) = stamp.parent() {
                    let _ = fs::create_dir_all(folder);
                }
                let _ = fs::write(&stamp, format!("{now} {version}\n"));
                version
            }
        };
        if is_newer(&latest, CURRENT) {
            let _ = send.send(format!(
                "{DIM}posy {latest} is out · run `posy update` to get it{RESET}"
            ));
        }
    });
    receive
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_as_numbers() {
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("v1.0.0", "0.9.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("nonsense", "0.1.0"));
        assert!(!is_newer("1.2", "0.1.0"));
    }

    #[test]
    fn this_computer_has_a_release() {
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            assert!(target().is_some());
        }
    }
}
