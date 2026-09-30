//! `iwr serve`: analyze the project, serve the UI + JSON API, re-analyze on file changes.

use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use iwr_core::Project;
use notify::{RecursiveMode, Watcher};
use rust_embed::RustEmbed;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
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
    // a `codecast.txt` next to the project is picked up automatically
    let script = script.or_else(|| {
        let p = if path.is_file() { path.parent().map(|d| d.join("codecast.txt")) } else { Some(path.join("codecast.txt")) };
        p.filter(|p| p.exists())
    });
    if let Some(s) = &script {
        let text = std::fs::read_to_string(s).with_context(|| format!("reading script {}", s.display()))?;
        let sc = iwr_core::script::parse(&text);
        for b in iwr_core::script::check(&project, &sc) {
            eprintln!("script: {}", b);
        }
        eprintln!("script {}: {} part(s), {} cue(s)", sc.title, sc.parts.len(), sc.cue_count());
    }
    let shared: AppState = Arc::new(Shared { project: RwLock::new(Arc::new(project)), version: RwLock::new(1), path: path.clone(), script, audio });
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        // file watcher → re-analyze (debounced)
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let watch_root = shared.path.canonicalize().unwrap_or(shared.path.clone());
        let watch_root = if watch_root.is_file() { watch_root.parent().unwrap().to_path_buf() } else { watch_root };
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
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
            .route("/api/audio", get(api_audio))
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
async fn api_script(State(s): State<AppState>) -> Response {
    match &s.script {
        Some(p) => match std::fs::read_to_string(p) {
            Ok(t) => ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], t).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
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
pub fn export(path: PathBuf, out: PathBuf, script: Option<PathBuf>, audio: Option<PathBuf>) -> Result<()> {
    let project = analyze(&path)?;
    let script = script.or_else(|| Some(path.join("codecast.txt")).filter(|p| p.exists()));
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
        let text = std::fs::read_to_string(s)?;
        let sc = iwr_core::script::parse(&text);
        for b in iwr_core::script::check(&project, &sc) {
            eprintln!("script: {}", b);
        }
        std::fs::write(out.join("script.txt"), text)?;
    }
    if let Some(a) = &audio {
        let ext = a.extension().and_then(|e| e.to_str()).unwrap_or("mp3");
        std::fs::copy(a, out.join(format!("audio.{}", ext)))?;
        std::fs::write(out.join("audio.txt"), format!("audio.{}", ext))?;
    }
    eprintln!("exported {} to {}", project.name, out.display());
    Ok(())
}
