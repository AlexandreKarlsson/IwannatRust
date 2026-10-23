//! Offline translation for codecasts played in another language (`iwr translate`,
//! `/api/translate`, `iwr export --lang`).
//!
//! Argos Translate models run in a Python sidecar (`ctranslate2` + `sentencepiece`) set up on first
//! use under `~/.cache/iwr/translate` (`IWR_TRANSLATE` overrides), one model per language,
//! downloaded when first asked for. Every translation is cached there (`<lang>.json`), so a
//! codecast is translated once and only edited text goes to the model again. Nothing is written
//! next to the codecast.
//!
//! The model's output is rough. An author can translate the pieces with another tool instead:
//! `iwr translate --pieces` prints them as JSON, and the filled-in file saved as
//! `translations/<lang>.json` next to the codecast wins over the model, piece by piece.

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

/// The author's own translations of a codecast: `translations/<lang>.json` in `dir` (the
/// codecast directory), English piece → translation; empty values are left to the model.
pub fn own_path(dir: &Path, lang: &str) -> PathBuf {
    dir.join("translations").join(format!("{}.json", lang))
}

fn own(dir: Option<&Path>, lang: &str) -> Result<HashMap<String, String>> {
    let Some(path) = dir.map(|d| own_path(d, lang)).filter(|p| p.is_file()) else { return Ok(HashMap::new()) };
    let map: HashMap<String, String> = serde_json::from_str(&std::fs::read_to_string(&path)?).with_context(|| format!("reading {}", path.display()))?;
    Ok(map.into_iter().filter(|(_, v)| !v.trim().is_empty()).collect())
}

/// Translate English texts (protected by `iwr_core::i18n::texts`) into `lang`: the author's own
/// translation from `dir` when it has the piece, else the model's, cached; `fresh` asks the
/// model again (and caches the new translations).
pub fn translate(lang: &str, texts: &[String], fresh: bool, dir: Option<&Path>) -> Result<Vec<String>> {
    if iwr_core::i18n::lang(lang).is_none() || lang == "en" {
        bail!("no translation to {:?}", lang);
    }
    let own = own(dir, lang)?;
    let rest: Vec<String> = texts.iter().filter(|t| !own.contains_key(*t)).cloned().collect();
    let done = if rest.is_empty() { vec![] } else { machine(lang, &rest, fresh)? };
    let machine: HashMap<&String, String> = rest.iter().zip(done).collect();
    Ok(texts.iter().map(|t| own.get(t).or_else(|| machine.get(t)).cloned().unwrap_or_else(|| t.clone())).collect())
}

/// The offline model's translations, cached.
fn machine(lang: &str, texts: &[String], fresh: bool) -> Result<Vec<String>> {
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
    let texts = pieces(script, project)?;
    let done = translate(lang, &texts, false, Some(&codecast::script_dir(script)))?;
    Ok(texts.into_iter().zip(done).collect())
}

/// Every piece of a codecast the player translates (tours expanded, glossary questions included).
fn pieces(script: &Path, project: &iwr_core::Project) -> Result<Vec<String>> {
    let loaded = codecast::load(script)?;
    let mut s = iwr_core::script::parse(&loaded.text);
    iwr_core::script::expand_tours(project, &mut s);
    let mut g = vec![iwr_core::glossary::builtin()];
    if let Some(own) = &loaded.glossary {
        g.push(iwr_core::glossary::parse(own));
    }
    Ok(iwr_core::i18n::texts(&s, &iwr_core::glossary::merge(&g), &iwr_core::i18n::names(project)))
}

/// `iwr translate --pieces`: the pieces as a JSON object to translate with another tool, valued
/// with the author's translations so far (empty when there is none). No model is used.
pub fn print_pieces(script: &Path, path: &Path, lang: &str) -> Result<()> {
    if iwr_core::i18n::lang(lang).is_none() || lang == "en" {
        bail!("no translation to {:?}", lang);
    }
    let p = iwr_core::analyze_path(path)?;
    let dir = codecast::script_dir(script);
    let own = own(Some(&dir), lang)?;
    // written by hand to keep the reading order (a translator wants the context)
    let mut seen = std::collections::HashSet::new();
    let mut lines = Vec::new();
    for t in pieces(script, &p)?.iter().filter(|t| seen.insert(t.to_string())) {
        lines.push(format!("  {}: {}", serde_json::to_string(t)?, serde_json::to_string(own.get(t).map_or("", String::as_str))?));
    }
    println!("{{\n{}\n}}", lines.join(",\n"));
    eprintln!("{} piece(s); translate the values, keep `code` as is, save as {}", lines.len(), own_path(&dir, lang).display());
    Ok(())
}

/// `iwr translate`: translate a codecast and print it, to read the result before playing it.
pub fn run(script: &Path, path: &Path, lang: &str) -> Result<()> {
    let p = iwr_core::analyze_path(path)?;
    let map = codecast_map(script, &p, lang)?;
    let own = own_path(&codecast::script_dir(script), lang);
    let loaded = codecast::load(script)?;
    let mut s = iwr_core::script::parse(&loaded.text);
    iwr_core::script::expand_tours(&p, &mut s);
    iwr_core::i18n::apply(&mut s, &mut [], &iwr_core::i18n::names(&p), &|t| map.get(t).cloned());
    print!("{}", iwr_core::script::to_text(&s));
    eprintln!("{} piece(s) translated to {}; cached in {}", map.len(), lang, home()?.display());
    if !own.is_file() {
        eprintln!("machine translation is rough: for a better one, translate `iwr translate --pieces` with another tool into {}", own.display());
    }
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
