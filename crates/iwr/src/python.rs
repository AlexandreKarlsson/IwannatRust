//! The Python environments of the sidecars (Kokoro voice, Argos translation), the same on Linux,
//! macOS and Windows.
//!
//! Each sidecar has its own directory under the user's cache (`~/.cache/codecast`,
//! `~/Library/Caches/codecast`, `%LOCALAPPDATA%\codecast`) with a venv in it. The venv is made by
//! [uv](https://docs.astral.sh/uv/): the one on `PATH`, or else one downloaded into the cache on
//! first use, which also fetches a Python when the machine has none. Nothing has to be installed
//! beforehand and nothing outside the cache is touched.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// uv version installed into the cache when there is none on `PATH`.
const UV_VERSION: &str = "0.13.6";
/// Python the venvs are made with (wheels exist for onnxruntime and ctranslate2 on every OS).
const PYTHON: &str = "3.12";

/// A sidecar's directory: `$env` when set (the older `IWR_…` name too), else `<cache>/codecast/<name>`.
pub fn home(env: &str, name: &str) -> Result<PathBuf> {
    let old = env.replacen("CODECAST_", "IWR_", 1);
    if let Some(h) = std::env::var_os(env).or_else(|| std::env::var_os(old)) {
        return Ok(PathBuf::from(h));
    }
    Ok(cache()?.join(name))
}

/// `<cache>/codecast`, moved from `<cache>/iwr` (the CLI's former name) so models are not downloaded again.
fn cache() -> Result<PathBuf> {
    let base = dirs::cache_dir().context("no cache directory (HOME / LOCALAPPDATA unset)")?;
    let dir = base.join("codecast");
    let old = base.join("iwr");
    if !dir.exists() && old.is_dir() {
        std::fs::rename(&old, &dir).ok();
    }
    Ok(dir)
}

/// The venv's Python: `venv/bin/python`, or `venv\Scripts\python.exe` on Windows.
pub fn python(home: &Path) -> PathBuf {
    if cfg!(windows) {
        home.join("venv").join("Scripts").join("python.exe")
    } else {
        home.join("venv").join("bin").join("python")
    }
}

/// Make `home`'s venv if needed and install the `packages` missing from it (`import_check` is the
/// Python statement that fails when they are). `run` runs a step, naming it. Answers the venv's Python.
pub fn venv(home: &Path, packages: &[&str], import_check: &str, run: &mut dyn FnMut(&mut Command, &str) -> Result<()>) -> Result<PathBuf> {
    let py = python(home);
    let installed = |py: &Path| Command::new(py).args(["-c", import_check]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
    if py.is_file() && installed(&py) {
        return Ok(py);
    }
    let uv = uv(run)?;
    if !py.is_file() {
        run(Command::new(&uv).args(["venv", "-q", "--python", PYTHON]).arg(home.join("venv")), "creating a Python environment")?;
    }
    run(Command::new(&uv).args(["pip", "install", "-q", "--python"]).arg(&py).args(packages), &format!("installing {}", packages.join(", ")))?;
    Ok(py)
}

/// uv: the one on `PATH`, else `<cache>/codecast/bin/uv`, installed there by uv's own installer.
fn uv(run: &mut dyn FnMut(&mut Command, &str) -> Result<()>) -> Result<PathBuf> {
    if Command::new("uv").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success()) {
        return Ok("uv".into());
    }
    let bin = cache()?.join("bin");
    let uv = bin.join(if cfg!(windows) { "uv.exe" } else { "uv" });
    if uv.is_file() {
        return Ok(uv);
    }
    std::fs::create_dir_all(&bin)?;
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("powershell");
        c.args(["-NoProfile", "-ExecutionPolicy", "ByPass", "-Command", &format!("irm https://astral.sh/uv/{}/install.ps1 | iex", UV_VERSION)]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", &format!("curl -LsSf https://astral.sh/uv/{}/install.sh | sh", UV_VERSION)]);
        c
    };
    // Flat install into our directory, no shell profile or PATH change.
    cmd.env("UV_UNMANAGED_INSTALL", &bin).env("UV_NO_MODIFY_PATH", "1").stdout(Stdio::null());
    run(&mut cmd, &format!("downloading uv {} (it sets up Python)", UV_VERSION))?;
    if !uv.is_file() {
        bail!("uv was not installed in {}: install it yourself (https://docs.astral.sh/uv/) and run again", bin.display());
    }
    Ok(uv)
}
