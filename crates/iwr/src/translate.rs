//! Offline translation for codecasts played in another language (`iwr translate`,
//! `/api/translate`, `iwr export --lang`).
//!
//! Argos Translate models run in a Python sidecar (`ctranslate2` + `sentencepiece`) set up on first
//! use under `~/.cache/iwr/translate` (`IWR_TRANSLATE` overrides), one model per language,
//! downloaded when first asked for. Every translation is cached there (`<lang>.json`), so a
//! codecast is translated once and only edited text goes to the model again. Nothing is written
//! next to the codecast.

use crate::codecast;
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

const SIDECAR: &str = include_str!("translate_sidecar.py");

/// One translation at a time: the cache file is rewritten after each.
static LOCK: Mutex<()> = Mutex::new(());

/// Translate English texts (protected by `iwr_core::i18n::texts`) into `lang`, cached;
/// `fresh` translates them again (and caches the new translations).
pub fn translate(lang: &str, texts: &[String], fresh: bool) -> Result<Vec<String>> {
    if iwr_core::i18n::lang(lang).is_none() || lang == "en" {
        bail!("no translation to {:?}", lang);
    }
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let home = home()?;
    let cache_path = home.join(format!("{}.json", lang));
    let mut cache: HashMap<String, String> = std::fs::read_to_string(&cache_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let mut todo: Vec<String> = texts.iter().filter(|t| fresh || !cache.contains_key(*t)).cloned().collect();
    todo.sort();
    todo.dedup();
    if !todo.is_empty() {
        let py = setup(&home)?;
        let job = serde_json::json!({ "home": home, "lang": lang, "texts": todo });
        let mut child = Command::new(py).arg(home.join("translate_sidecar.py")).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().context("starting the translation sidecar")?;
        child.stdin.take().context("sidecar stdin")?.write_all(job.to_string().as_bytes())?;
        let out = child.wait_with_output()?;
        if !out.status.success() {
            bail!("translation sidecar failed ({})", out.status);
        }
        let done: Vec<String> = serde_json::from_slice(&out.stdout).context("reading the sidecar's result")?;
        cache.extend(todo.into_iter().zip(done));
        std::fs::write(&cache_path, serde_json::to_string(&cache)?)?;
    }
    Ok(texts.iter().map(|t| cache.get(t).cloned().unwrap_or_else(|| t.clone())).collect())
}

/// Translate a codecast (tours expanded, glossary questions included) and return the source →
/// translation map the player looks pieces up in.
pub fn codecast_map(script: &Path, project: &iwr_core::Project, lang: &str) -> Result<HashMap<String, String>> {
    let loaded = codecast::load(script)?;
    let mut s = iwr_core::script::parse(&loaded.text);
    iwr_core::script::expand_tours(project, &mut s);
    let mut g = vec![iwr_core::glossary::builtin()];
    if let Some(own) = &loaded.glossary {
        g.push(iwr_core::glossary::parse(own));
    }
    let texts = iwr_core::i18n::texts(&s, &iwr_core::glossary::merge(&g), &iwr_core::i18n::names(project));
    let done = translate(lang, &texts, false)?;
    Ok(texts.into_iter().zip(done).collect())
}

/// `iwr translate`: translate a codecast and print it, to read the result before playing it.
pub fn run(script: &Path, path: &Path, lang: &str) -> Result<()> {
    let p = iwr_core::analyze_path(path)?;
    let map = codecast_map(script, &p, lang)?;
    let loaded = codecast::load(script)?;
    let mut s = iwr_core::script::parse(&loaded.text);
    iwr_core::script::expand_tours(&p, &mut s);
    iwr_core::i18n::apply(&mut s, &mut [], &iwr_core::i18n::names(&p), &|t| map.get(t).cloned());
    print!("{}", iwr_core::script::to_text(&s));
    eprintln!("{} piece(s) translated to {}; cached in {}", map.len(), lang, home()?.display());
    Ok(())
}

fn home() -> Result<PathBuf> {
    let home = match std::env::var_os("IWR_TRANSLATE") {
        Some(h) => PathBuf::from(h),
        None => std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache"))).context("no HOME")?.join("iwr/translate"),
    };
    std::fs::create_dir_all(&home)?;
    Ok(home)
}

/// The sidecar's Python, in a venv with `ctranslate2` and `sentencepiece`.
fn setup(home: &Path) -> Result<PathBuf> {
    let run = |cmd: &mut Command, what: &str| -> Result<()> {
        eprintln!("{}…", what);
        let st = cmd.stderr(Stdio::inherit()).stdout(Stdio::null()).status().with_context(|| what.to_string())?;
        if !st.success() {
            bail!("{} failed ({})", what, st);
        }
        Ok(())
    };
    let py = home.join("venv/bin/python");
    if !py.is_file() {
        run(Command::new("python3").args(["-m", "venv"]).arg(home.join("venv")), "creating a Python venv for translation")?;
    }
    if !Command::new(&py).args(["-c", "import ctranslate2, sentencepiece"]).stderr(Stdio::null()).status()?.success() {
        run(Command::new(home.join("venv/bin/pip")).args(["install", "-q", "ctranslate2", "sentencepiece"]), "installing ctranslate2 and sentencepiece")?;
    }
    std::fs::write(home.join("translate_sidecar.py"), SIDECAR)?;
    Ok(py)
}
