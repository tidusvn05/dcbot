use anyhow::{bail, Context, Result};
use console::style;
use rust_i18n::t;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_REPO: &str = "tidusvn05/dcbot";

fn repo() -> String {
    std::env::var("DCBOT_REPO").unwrap_or_else(|_| DEFAULT_REPO.to_string())
}

fn github_get(url: &str) -> Result<ureq::http::Response<ureq::Body>> {
    ureq::get(url)
        .header("User-Agent", concat!("dcbot/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| anyhow::anyhow!(t!("update.fetch_fail", err = e.to_string())))
}

/// Latest release tag (e.g. "v0.7.0") via the GitHub API.
fn latest_tag() -> Result<String> {
    let mut res = github_get(&format!(
        "https://api.github.com/repos/{}/releases/latest",
        repo()
    ))?;
    let body = res
        .body_mut()
        .read_to_string()
        .context("reading GitHub response")?;
    let v: serde_json::Value =
        serde_json::from_str(&body).context("unexpected response from GitHub")?;
    v.get("tag_name")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .context("GitHub response missing tag_name")
}

/// dcbot-{linux|macos}-{x86_64|aarch64} — matches release.yml matrix names.
fn target_name() -> Result<String> {
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "macos",
        o => bail!(t!(
            "update.unsupported",
            plat = format!("{o}-{}", std::env::consts::ARCH)
        )),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        a => bail!(t!(
            "update.unsupported",
            plat = format!("{}-{a}", std::env::consts::OS)
        )),
    };
    Ok(format!("dcbot-{os}-{arch}"))
}

/// "v0.7.0"/"0.7.0" → (0,7,0) for comparison. Non-numeric tail ignored.
fn semver(v: &str) -> (u64, u64, u64) {
    let v = v.trim().trim_start_matches('v');
    let mut it = v.split('.').map(|p| {
        p.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0)
    });
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

/// sha256sum -c (linux) / shasum -a 256 -c (macOS) — same tools install.sh uses.
/// Missing tool → warn and skip (checksum file is advisory there).
fn verify_checksum(dir: &Path, sha_file: &str) -> Result<()> {
    let check = if which::which("sha256sum").is_ok() {
        Command::new("sha256sum")
            .args(["-c", sha_file])
            .current_dir(dir)
            .output()
    } else if which::which("shasum").is_ok() {
        Command::new("shasum")
            .args(["-a", "256", "-c", sha_file])
            .current_dir(dir)
            .output()
    } else {
        println!("{}", style(t!("update.no_sha_tool")).yellow());
        return Ok(());
    };
    match check {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => bail!(
            "{}",
            t!(
                "update.bad_sha",
                err = String::from_utf8_lossy(&o.stderr).trim().to_string()
            )
        ),
        Err(e) => bail!(t!("update.bad_sha", err = e.to_string())),
    }
}

fn download(url: &str) -> Result<Vec<u8>> {
    let mut res = github_get(url)?;
    res.body_mut()
        .read_to_vec()
        .context("reading download body")
}

/// The binary to replace: the one currently running. Refuse cargo target/
/// dirs — those are dev builds, use `cargo install` or install.sh there.
fn current_binary() -> Result<PathBuf> {
    let exe = std::env::current_exe()?.canonicalize()?;
    if exe.components().any(|c| c.as_os_str() == "target") {
        bail!(t!("update.dev_binary", path = exe.display().to_string()));
    }
    Ok(exe)
}

pub fn run(version: Option<String>, check: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let pinned = version.is_some();
    let tag = match version {
        Some(v) => {
            let v = v.trim().to_string();
            if v.starts_with('v') {
                v
            } else {
                format!("v{v}")
            }
        }
        None => latest_tag()?,
    };
    let latest = tag.trim_start_matches('v');
    println!(
        "{}",
        t!("update.versions", cur = current.as_str(), new = latest)
    );
    if check {
        if semver(&current) >= semver(latest) {
            println!("{}", style(t!("update.uptodate")).green());
        } else {
            println!("{}", t!("update.available", tag = tag.as_str()));
        }
        return Ok(());
    }
    if !pinned && semver(&current) >= semver(latest) {
        println!("{}", style(t!("update.uptodate")).green());
        return Ok(());
    }

    let exe = current_binary()?;
    let target = target_name()?;
    let base = format!("https://github.com/{}/releases/download/{tag}", repo());
    let tmp = std::env::temp_dir().join(format!("dcbot-update-{}", std::process::id()));
    fs::create_dir_all(&tmp)?;
    let result = install(&tmp, &base, &target, &exe, latest);
    let _ = fs::remove_dir_all(&tmp);
    result
}

fn install(tmp: &Path, base: &str, target: &str, exe: &Path, expect: &str) -> Result<()> {
    let tgz = format!("{target}.tar.gz");
    println!("{}", t!("update.downloading", name = target));
    fs::write(tmp.join(&tgz), download(&format!("{base}/{tgz}"))?)?;
    fs::write(
        tmp.join(format!("{tgz}.sha256")),
        download(&format!("{base}/{tgz}.sha256"))?,
    )?;
    verify_checksum(tmp, &format!("{tgz}.sha256"))?;

    let st = Command::new("tar")
        .args(["-xzf", &tgz, "-C"])
        .arg(tmp)
        .current_dir(tmp)
        .status()?;
    if !st.success() {
        bail!(t!("update.extract_fail"));
    }
    let staged = tmp.join("dcbot");
    if !staged.exists() {
        bail!(t!("update.extract_fail"));
    }

    // Sanity-run the downloaded binary before swapping it in.
    let out = Command::new(&staged).arg("--version").output();
    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout);
            if !s.contains(expect) {
                bail!(t!("update.mismatch", got = s.trim().to_string()));
            }
        }
        _ => bail!(t!("update.wont_run")),
    }

    // Stage next to the target so rename stays on one filesystem.
    let next = exe.with_file_name("dcbot.next");
    fs::copy(&staged, &next).context("staging new binary")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&next, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&next, exe).context("replacing binary")?;
    println!(
        "{} {}",
        style("✓").green().bold(),
        t!(
            "update.done",
            ver = expect,
            path = exe.display().to_string()
        )
    );
    Ok(())
}
