//! `iwr serve`: analyze the project, serve the UI + JSON API, re-analyze on file changes.

use anyhow::{Context, Result};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path as AxPath, State},
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use iwr_core::Project;
use notify::{RecursiveMode, Watcher};
use rust_embed::RustEmbed;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

#[derive(RustEmbed)]
#[folder = "../../target/dx/iwr-ui/release/web/public"]
struct Assets;

struct Shared {
    project: RwLock<Arc<Project>>,
    version: RwLock<u64>,
    path: PathBuf,
    script: Option<PathBuf>,
    audio: Option<PathBuf>,
    /// the Generate page's recording (one at a time), and when it started
    voice_job: Mutex<(iwr_core::voice::VoiceJob, Option<Instant>)>,
    voice_cancel: AtomicBool,
}

type AppState = Arc<Shared>;

fn analyze(path: &Path) -> Result<Project> {
    let started = Instant::now();
    let p = iwr_core::analyze_path(path).with_context(|| format!("analyzing {}", path.display()))?;
    eprintln!(
        "analyzed {}: {} files, {} items, {} calls in {:.0?}",
        p.name,
        p.files.len(),
        p.items.len(),
        p.calls.len(),
        started.elapsed()
    );
    for d in &p.diagnostics {
        eprintln!("  warning: {}", d);
    }
    Ok(p)
}

pub fn serve(path: PathBuf, port: u16, open_browser: bool, script: Option<PathBuf>, audio: Option<PathBuf>) -> Result<()> {
    let project = analyze(&path)?;
    // a `codecast/` directory or `codecast.md` next to the project is picked up automatically
    let script = script.or_else(|| crate::codecast::locate(&path));
    if let Some(s) = &script {
        let loaded = crate::codecast::load(s)?;
        let sc = iwr_core::script::parse(&loaded.text);
        for b in iwr_core::script::check(&project, &sc) {
            eprintln!("script: {}", b);
        }
        if let Some(g) = &loaded.glossary {
            for b in crate::codecast::check_glossary(&project, g) {
                eprintln!("script: {}", b);
            }
        }
        for a in crate::codecast::audio_files(&sc) {
            if !loaded.dir.join(&a).is_file() {
                eprintln!("script: audio file not found: {}", loaded.dir.join(&a).display());
            }
        }
        for stem in &loaded.stale {
            eprintln!("script: {}: text changed since its recording, spoken by TTS (re-record: iwr voice {}{})", stem, s.display(), crate::codecast::lang_arg(stem));
        }
        for b in crate::codecast::check_config(s) {
            eprintln!("script: {}", b);
        }
        eprintln!("script {} ({}): {} part(s), {} cue(s)", sc.title, s.display(), sc.parts.len(), sc.cue_count());
    }
    let shared: AppState = Arc::new(Shared { project: RwLock::new(Arc::new(project)), version: RwLock::new(1), path: path.clone(), script, audio, voice_job: Default::default(), voice_cancel: AtomicBool::new(false) });
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        // file watcher → re-analyze (debounced)
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let watch_root = shared.path.canonicalize().unwrap_or(shared.path.clone());
        let watch_root = if watch_root.is_file() { watch_root.parent().unwrap().to_path_buf() } else { watch_root };
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                // reading the sources (our own re-analysis included) emits access events: ignore them
                if ev.kind.is_access() {
                    return;
                }
                if ev.paths.iter().any(|p| p.extension().map(|e| e == "rs").unwrap_or(false) || p.file_name().map(|f| f == "Cargo.toml").unwrap_or(false)) {
                    let _ = tx.send(());
                }
            }
        })?;
        watcher.watch(&watch_root, RecursiveMode::Recursive)?;
        {
            let shared = shared.clone();
            tokio::spawn(async move {
                loop {
                    if rx.recv().await.is_none() {
                        break;
                    }
                    // debounce
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    while rx.try_recv().is_ok() {}
                    let path = shared.path.clone();
                    match tokio::task::spawn_blocking(move || analyze(&path)).await {
                        Ok(Ok(p)) => {
                            *shared.project.write().unwrap() = Arc::new(p);
                            *shared.version.write().unwrap() += 1;
                        }
                        Ok(Err(e)) => eprintln!("re-analysis failed: {e:#}"),
                        Err(e) => eprintln!("re-analysis panicked: {e}"),
                    }
                }
            });
        }

        let app = Router::new()
            .route("/api/project", get(api_project))
            .route("/api/version", get(api_version))
            .route("/api/script", get(api_script))
            .route("/api/glossary", get(api_glossary))
            .route("/api/audio", get(api_audio))
            .route("/api/config", get(api_config).post(api_save_config))
            .route("/api/font/{name}", post(api_save_font).layer(DefaultBodyLimit::max(MAX_FONT)))
            .route("/api/codecast/{*name}", get(api_codecast_file))
            .route("/api/translate/{lang}", post(api_translate))
            .route("/api/voice", get(api_voice_status).post(api_voice_start))
            .route("/api/voice/job", get(api_voice_job))
            .route("/api/voice/cancel", post(api_voice_cancel))
            .route("/api/voice/refresh", post(api_voice_refresh))
            .route("/api/voice/clean", post(api_voice_clean))
            .route("/api/voice/preview", post(api_voice_preview))
            .fallback(static_handler)
            .layer(tower_http::cors::CorsLayer::permissive())
            .with_state(shared);
        let addr = format!("127.0.0.1:{port}");
        let listener = tokio::net::TcpListener::bind(&addr).await.with_context(|| format!("binding {addr}"))?;
        let url = format!("http://{addr}");
        eprintln!("IwannatRust serving {} at {}", path.display(), url);
        if Assets::get("index.html").is_none() {
            eprintln!("warning: web UI not built. Run `dx build --release` in crates/iwr-ui (or ./build.sh) and rebuild iwr.");
        }
        if open_browser {
            let _ = open::that(&url);
        }
        axum::serve(listener, app).await?;
        Ok::<(), anyhow::Error>(())
    })
}

async fn api_project(State(s): State<AppState>) -> Response {
    let p = s.project.read().unwrap().clone();
    // serialize on a blocking thread: projects can be large
    let body = tokio::task::spawn_blocking(move || serde_json::to_vec(&*p)).await.unwrap().unwrap();
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

async fn api_version(State(s): State<AppState>) -> Json<u64> {
    Json(*s.version.read().unwrap())
}

/// The codecast script as text (re-read on every request so edits show up on reload).
/// A `codecast/` directory is assembled into one text.
async fn api_script(State(s): State<AppState>) -> Response {
    match &s.script {
        Some(p) => match crate::codecast::load(p) {
            Ok(l) => ([(header::CONTENT_TYPE, "text/markdown; charset=utf-8")], l.text).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// The project's own glossary (`glossary.md` next to the script), re-read on every request.
async fn api_glossary(State(s): State<AppState>) -> Response {
    let Some(p) = &s.script else { return StatusCode::NOT_FOUND.into_response() };
    match std::fs::read_to_string(crate::codecast::glossary_path(p)) {
        Ok(t) => ([(header::CONTENT_TYPE, "text/markdown; charset=utf-8")], t).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// The codecast's `codecast.toml` as JSON, re-read on every request.
async fn api_config(State(s): State<AppState>) -> Response {
    let Some(p) = &s.script else { return StatusCode::NOT_FOUND.into_response() };
    match crate::codecast::read_config(p) {
        Ok(Some(c)) => Json(c).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            eprintln!("{e:#}");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Largest font file the Settings page may upload.
const MAX_FONT: usize = 10 << 20;

/// Is a write request from the page this server serves? The API allows any origin to read, so
/// writes check that `Origin` (sent by browsers on every cross-site POST) is this server, reached
/// by a loopback name (not a rebinding DNS name).
fn same_origin(headers: &HeaderMap) -> bool {
    let host = headers.get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or_default();
    let name = host.rsplit_once(':').map(|(n, _)| n).unwrap_or(host);
    if !["127.0.0.1", "localhost", "[::1]"].contains(&name) {
        return false;
    }
    match headers.get(header::ORIGIN).and_then(|o| o.to_str().ok()) {
        Some(o) => o == format!("http://{host}"),
        None => true,
    }
}

/// Save the Settings page's design as `codecast.toml` next to the script.
async fn api_save_config(State(s): State<AppState>, headers: HeaderMap, Json(c): Json<iwr_core::config::Config>) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(p) = &s.script else { return (StatusCode::CONFLICT, "no codecast next to the project: create codecast.md or codecast/ first").into_response() };
    let bad = c.problems();
    if !bad.is_empty() {
        return (StatusCode::BAD_REQUEST, bad.join("\n")).into_response();
    }
    let path = crate::codecast::script_dir(p).join(iwr_core::config::FILE);
    match std::fs::write(&path, c.to_toml()) {
        Ok(()) => {
            eprintln!("saved {}", path.display());
            path.display().to_string().into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}: {e}", path.display())).into_response(),
    }
}

/// Store an uploaded font as `fonts/NAME` in the codecast directory; answers that relative path.
async fn api_save_font(State(s): State<AppState>, headers: HeaderMap, AxPath(name): AxPath<String>, body: Bytes) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(p) = &s.script else { return (StatusCode::CONFLICT, "no codecast next to the project").into_response() };
    let rel = format!("fonts/{name}");
    if name.contains('/') || !crate::codecast::is_safe_asset_name(&rel) || !iwr_core::config::is_font_file(&name) {
        return (StatusCode::BAD_REQUEST, "not a font file name (ttf, otf, woff, woff2)").into_response();
    }
    let dir = crate::codecast::script_dir(p).join("fonts");
    let res = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(&name), &body));
    match res {
        Ok(()) => rel.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// An audio file named by an `audio:` line, or a font of `codecast.toml`, relative to the script's directory.
async fn api_codecast_file(State(s): State<AppState>, AxPath(name): AxPath<String>) -> Response {
    let Some(script) = &s.script else { return StatusCode::NOT_FOUND.into_response() };
    if !crate::codecast::is_safe_asset_name(&name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match std::fs::read(crate::codecast::script_dir(script).join(&name)) {
        Ok(b) => {
            let mime = mime_guess::from_path(&name).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref().to_string())], b).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Translate the texts the player sends (English, code in backticks) with the offline model;
/// the first request for a language sets the model up, which takes a while.
async fn api_translate(AxPath(lang): AxPath<String>, Json(texts): Json<Vec<String>>) -> Response {
    match tokio::task::spawn_blocking(move || crate::translate::translate(&lang, &texts)).await {
        Ok(Ok(done)) => Json(done).into_response(),
        Ok(Err(e)) => {
            eprintln!("translate: {e:#}");
            (StatusCode::SERVICE_UNAVAILABLE, format!("{e:#}")).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Every part of the codecast and its recordings (`iwr voice`), for the Generate page.
async fn api_voice_status(State(s): State<AppState>) -> Response {
    let script = s.script.clone();
    match tokio::task::spawn_blocking(move || crate::voice::status(script.as_deref())).await {
        Ok(st) => Json(st).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Start recording the codecast in the background; `GET /api/voice/job` follows it.
async fn api_voice_start(State(s): State<AppState>, headers: HeaderMap, Json(req): Json<iwr_core::voice::VoiceRequest>) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(dir) = s.script.clone().filter(|d| d.is_dir()) else { return (StatusCode::CONFLICT, "recordings are made for a codecast/ directory").into_response() };
    if iwr_core::voice::voice_lang(&req.lang).is_none() || !iwr_core::voice::VOICES.contains(&req.voice.as_str()) {
        return (StatusCode::BAD_REQUEST, "unknown voice or language").into_response();
    }
    let opts = crate::voice::Opts { voice: req.voice.clone(), speed: req.speed.clamp(0.5, 2.0), lang: req.lang.clone(), path: Some(s.path.clone()), gap: req.gap.clamp(0.0, 3.0), level: req.level.clamp(0.0, 1.0), force: req.force, only: req.parts.clone() };
    spawn_voice_job(s, dir, vec![opts], vec![], Some(req))
}

/// Record the out-of-date recordings again, each with the voice and delivery it was made with,
/// and delete the files of parts that are gone.
async fn api_voice_refresh(State(s): State<AppState>, headers: HeaderMap, Json(req): Json<iwr_core::voice::CleanRequest>) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(dir) = s.script.clone().filter(|d| d.is_dir()) else { return (StatusCode::CONFLICT, "recordings are made for a codecast/ directory").into_response() };
    let st = crate::voice::status(Some(&dir));
    let opts = st
        .stale
        .iter()
        .filter(|g| req.groups.is_empty() || req.groups.contains(&g.key()))
        .map(|g| crate::voice::Opts { voice: g.voice.clone(), speed: g.speed, lang: g.arg.clone(), path: Some(s.path.clone()), gap: g.gap, level: g.level, force: false, only: g.parts.clone() })
        .collect();
    spawn_voice_job(s, dir, opts, st.orphans, None)
}

/// Delete out-of-date recordings (the parts are spoken by TTS until recorded again) and the
/// files of parts that are gone; answers how many files went.
async fn api_voice_clean(State(s): State<AppState>, headers: HeaderMap, Json(req): Json<iwr_core::voice::CleanRequest>) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(dir) = s.script.clone().filter(|d| d.is_dir()) else { return (StatusCode::CONFLICT, "recordings are made for a codecast/ directory").into_response() };
    if s.voice_job.lock().unwrap().0.running {
        return (StatusCode::CONFLICT, "a recording is running").into_response();
    }
    let st = crate::voice::status(Some(&dir));
    let mut names = st.orphans;
    names.extend(st.stale.into_iter().filter(|g| req.groups.is_empty() || req.groups.contains(&g.key())).flat_map(|g| g.files));
    match crate::voice::delete(&dir, &names) {
        Ok(n) => {
            eprintln!("voice: deleted {n} out-of-date file(s)");
            Json(n).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
    }
}

/// Run recordings one after the other in the background (one job at a time), then delete
/// `delete` (file names in `dir`); `GET /api/voice/job` follows it.
fn spawn_voice_job(s: AppState, dir: PathBuf, runs: Vec<crate::voice::Opts>, delete: Vec<String>, request: Option<iwr_core::voice::VoiceRequest>) -> Response {
    use crate::voice::Event;
    {
        let mut job = s.voice_job.lock().unwrap();
        if job.0.running {
            return (StatusCode::CONFLICT, "a recording is already running").into_response();
        }
        let id = job.0.id + 1;
        *job = (iwr_core::voice::VoiceJob { id, running: true, stage: "starting".into(), request, ..Default::default() }, Some(Instant::now()));
    }
    s.voice_cancel.store(false, Ordering::Relaxed);
    let shared = s.clone();
    tokio::task::spawn_blocking(move || {
        let mut res = Ok(());
        for o in &runs {
            // each run's parts come after the earlier runs'
            let mut offset = 0;
            res = crate::voice::record(&dir, o, &shared.voice_cancel, &mut |e| {
                let mut job = shared.voice_job.lock().unwrap();
                let j = &mut job.0;
                match e {
                    Event::Stage(t) => j.stage = t,
                    Event::Plan(p) => {
                        offset = j.parts.len();
                        j.parts.extend(p.into_iter().map(|(stem, cues)| iwr_core::voice::JobPart { stem, cues, ..Default::default() }));
                    }
                    Event::Cue { part, done } => {
                        if let Some(p) = j.parts.get_mut(offset + part) {
                            p.done = done;
                        }
                    }
                    Event::Part { part, duration, kb } => {
                        if let Some(p) = j.parts.get_mut(offset + part) {
                            p.done = p.cues;
                            p.duration = Some(duration);
                            p.kb = kb;
                        }
                    }
                    Event::Log(l) => {
                        eprintln!("voice: {}", l);
                        j.log.push(l);
                        let n = j.log.len();
                        j.log.drain(..n.saturating_sub(200));
                    }
                }
            });
            if res.is_err() {
                break;
            }
        }
        if res.is_ok() && !delete.is_empty() {
            res = crate::voice::delete(&dir, &delete).map(|n| {
                shared.voice_job.lock().unwrap().0.log.push(format!("deleted {n} file(s) of parts that are gone"));
            });
        }
        let mut job = shared.voice_job.lock().unwrap();
        job.0.running = false;
        job.0.elapsed = job.1.map(|t| t.elapsed().as_secs_f64()).unwrap_or_default();
        job.1 = None;
        match res {
            Ok(()) => job.0.stage = "done".into(),
            Err(e) => {
                eprintln!("voice: {e:#}");
                job.0.stage = if e.to_string() == "cancelled" { "cancelled".into() } else { "failed".into() };
                job.0.error = Some(format!("{e:#}"));
            }
        }
    });
    StatusCode::ACCEPTED.into_response()
}

/// The running (or last) recording's progress.
async fn api_voice_job(State(s): State<AppState>) -> Json<iwr_core::voice::VoiceJob> {
    let job = s.voice_job.lock().unwrap();
    let mut j = job.0.clone();
    if let Some(t) = job.1 {
        j.elapsed = t.elapsed().as_secs_f64();
    }
    Json(j)
}

async fn api_voice_cancel(State(s): State<AppState>, headers: HeaderMap) -> StatusCode {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN;
    }
    s.voice_cancel.store(true, Ordering::Relaxed);
    StatusCode::NO_CONTENT
}

/// A voice saying a sample sentence (Ogg Opus); sets Kokoro up first if needed.
async fn api_voice_preview(headers: HeaderMap, Json(req): Json<iwr_core::voice::VoiceRequest>) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match tokio::task::spawn_blocking(move || crate::voice::preview(&req.voice, req.speed.clamp(0.5, 2.0), &req.lang)).await {
        Ok(Ok(b)) => ([(header::CONTENT_TYPE, "audio/ogg")], b).into_response(),
        Ok(Err(e)) => (StatusCode::SERVICE_UNAVAILABLE, format!("{e:#}")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn api_audio(State(s): State<AppState>) -> Response {
    match &s.audio {
        Some(p) => match std::fs::read(p) {
            Ok(b) => {
                let mime = mime_guess::from_path(p).first_or_octet_stream();
                ([(header::CONTENT_TYPE, mime.as_ref().to_string())], b).into_response()
            }
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref().to_string())], content.data.into_owned()).into_response()
        }
        None => {
            if path == "index.html" {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "The IwannatRust web UI is not built into this binary. Run ./build.sh (needs the dioxus CLI `dx`) and rebuild.",
                )
                    .into_response()
            } else {
                // SPA fallback
                match Assets::get("index.html") {
                    Some(c) => ([(header::CONTENT_TYPE, "text/html")], c.data.into_owned()).into_response(),
                    None => StatusCode::NOT_FOUND.into_response(),
                }
            }
        }
    }
}

/// Write the UI assets plus `project.json` into `out` so the visualization can be
/// served as static files (e.g. inside a documentation site).
pub fn export(path: PathBuf, out: PathBuf, script: Option<PathBuf>, audio: Option<PathBuf>, langs: &[String]) -> Result<()> {
    let project = analyze(&path)?;
    let script = script.or_else(|| crate::codecast::locate(&path));
    if Assets::get("index.html").is_none() {
        anyhow::bail!("web UI not built into this binary; run ./build.sh first");
    }
    std::fs::create_dir_all(&out)?;
    for name in Assets::iter() {
        let f = Assets::get(&name).unwrap();
        let dest = out.join(name.as_ref());
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut data = f.data.into_owned();
        if name.ends_with(".html") || name.ends_with(".js") {
            // make asset links relative so the site works from any sub-path (fetches resolve against the page URL)
            let text = String::from_utf8_lossy(&data).replace("\"/./assets/", "\"./assets/").replace("'/./assets/", "'./assets/");
            data = text.into_bytes();
        }
        std::fs::write(&dest, data)?;
    }
    std::fs::write(out.join("project.json"), serde_json::to_vec(&project)?)?;
    if let Some(s) = &script {
        let loaded = crate::codecast::load(s)?;
        let sc = iwr_core::script::parse(&loaded.text);
        for b in iwr_core::script::check(&project, &sc) {
            eprintln!("script: {}", b);
        }
        std::fs::write(out.join("codecast.md"), &loaded.text)?;
        if let Some(g) = &loaded.glossary {
            std::fs::write(out.join("glossary.md"), g)?;
        }
        // recordings named by `audio:` lines go to out/codecast/
        for a in crate::codecast::audio_files(&sc) {
            let src = loaded.dir.join(&a);
            if src.is_file() && crate::codecast::is_safe_audio_name(&a) {
                std::fs::create_dir_all(out.join("codecast"))?;
                std::fs::copy(&src, out.join("codecast").join(&a))?;
            } else {
                eprintln!("script: audio file not found or not allowed: {}", src.display());
            }
        }
        // the look and defaults: out/codecast.json, fonts beside the recordings in out/codecast/
        for b in crate::codecast::check_config(s) {
            eprintln!("script: {}", b);
        }
        if let Some(c) = crate::codecast::read_config(s)? {
            std::fs::write(out.join("codecast.json"), serde_json::to_vec(&c)?)?;
            for f in c.font_files() {
                let src = loaded.dir.join(&f);
                if src.is_file() {
                    let dest = out.join("codecast").join(&f);
                    std::fs::create_dir_all(dest.parent().unwrap())?;
                    std::fs::copy(&src, dest)?;
                }
            }
        }
        // translations the player looks up when another language is picked: out/translations/<lang>.json
        for lang in langs {
            let map = crate::translate::codecast_map(s, &project, lang)?;
            std::fs::create_dir_all(out.join("translations"))?;
            std::fs::write(out.join("translations").join(format!("{}.json", lang)), serde_json::to_vec(&map)?)?;
            eprintln!("translated the codecast to {} ({} pieces)", lang, map.len());
        }
    }
    if let Some(a) = &audio {
        let ext = a.extension().and_then(|e| e.to_str()).unwrap_or("mp3");
        std::fs::copy(a, out.join(format!("audio.{}", ext)))?;
        std::fs::write(out.join("audio.txt"), format!("audio.{}", ext))?;
    }
    eprintln!("exported {} to {}", project.name, out.display());
    Ok(())
}
