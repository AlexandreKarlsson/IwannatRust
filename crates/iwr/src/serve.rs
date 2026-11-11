//! `codecast serve`: analyze the project, serve the UI + JSON API, re-analyze on file changes.

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
use std::future::IntoFuture;
use std::time::{Duration, Instant};

#[derive(RustEmbed)]
#[folder = "../../target/dx/iwr-ui/release/web/public"]
struct Assets;

struct Shared {
    /// The served project; an empty one until the first analysis is done (`version` is `0`
    /// then, and `/api/project` answers 503)
    project: RwLock<Arc<Project>>,
    version: RwLock<u64>,
    /// The highest version served before the root last changed: the next one is above it, so a
    /// page that missed the `0` in between still sees a new version
    past_version: std::sync::atomic::AtomicU64,
    /// How far the running (or last) analysis is, and when it started: what `/api/status` answers
    status: Mutex<(iwr_core::progress::Status, Option<Instant>)>,
    /// What is served now; the cast map swaps it (`POST /api/root`)
    root: RwLock<Root>,
    /// What `codecast serve` was started with: jumping back to its folder restores `--script` / `--audio`
    first_root: Root,
    /// Tells the re-analysis task that a source changed
    changes: tokio::sync::mpsc::UnboundedSender<()>,
    /// The watch on the served folder; replaced when the root changes
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
    /// the Generate page's recording (one at a time), and when it started
    voice_job: Mutex<(iwr_core::voice::VoiceJob, Option<Instant>)>,
    voice_cancel: AtomicBool,
}

/// A served project: its folder (or file), its codecast and the `--audio` file, if any.
#[derive(Clone)]
struct Root {
    path: PathBuf,
    script: Option<PathBuf>,
    audio: Option<PathBuf>,
    /// The topic that plays (a sub-folder of the `codecast/` directory); `None`: the general
    /// codecast
    topic: Option<String>,
}

impl Shared {
    fn path(&self) -> PathBuf {
        self.root.read().unwrap().path.clone()
    }

    /// The codecast that plays: the topic's directory when a topic is picked.
    fn script(&self) -> Option<PathBuf> {
        let root = self.root.read().unwrap();
        match &root.topic {
            Some(t) => root.script.as_ref().map(|s| s.join(t)),
            None => root.script.clone(),
        }
    }

    /// The project's codecast, whatever topic plays.
    fn cast(&self) -> Option<PathBuf> {
        self.root.read().unwrap().script.clone()
    }

    /// The topic that plays; empty for the general codecast.
    fn topic(&self) -> String {
        self.root.read().unwrap().topic.clone().unwrap_or_default()
    }

    /// The `--audio` recording: it is the general codecast's.
    fn audio(&self) -> Option<PathBuf> {
        let root = self.root.read().unwrap();
        if root.topic.is_some() { None } else { root.audio.clone() }
    }
}

type AppState = Arc<Shared>;

/// Watch the served folder: an edited source wakes the re-analysis task. Replaces the watch
/// on the folder served before.
fn watch_root(shared: &AppState) {
    let tx = shared.changes.clone();
    let made = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            // reading the sources (our own re-analysis included) emits access events: ignore them
            if ev.kind.is_access() {
                return;
            }
            if ev.paths.iter().any(|p| p.extension().map(|e| e == "rs").unwrap_or(false) || p.file_name().map(|f| f == "Cargo.toml").unwrap_or(false)) {
                let _ = tx.send(());
            }
        }
    });
    let mut slot = shared.watcher.lock().unwrap();
    // the old watch goes first: its folders do not count against the new one's
    *slot = None;
    match made {
        Ok(mut w) => {
            watch_sources(&mut w, &crate::codecast::project_dir(&shared.path()));
            *slot = Some(w);
        }
        Err(e) => eprintln!("no file watch ({e}); edits need a restart"),
    }
}

/// Watch the project for edits. The whole tree first; when the OS runs out of watches (a big
/// workspace: every `target/` folder counts), only the folders that can hold sources, each on
/// its own; and when even that fails, serve without reloading rather than not at all.
fn watch_sources(watcher: &mut impl Watcher, root: &Path) {
    if watcher.watch(root, RecursiveMode::Recursive).is_ok() {
        return;
    }
    let _ = watcher.unwatch(root);
    // Build output, git objects, dependencies and hidden folders hold no source to re-read
    let skip = |p: &Path| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.') || n == "target" || n == "node_modules")
    };
    let mut stack = vec![root.to_path_buf()];
    let mut watched = 0usize;
    while let Some(dir) = stack.pop() {
        // Out of watches even here: keep what is watched, say so once
        if let Err(e) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
            eprintln!("file watch stopped after {watched} folders ({e}); edits elsewhere need a restart");
            return;
        }
        watched += 1;
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            if entry.file_type().is_ok_and(|t| t.is_dir()) && !skip(&p) {
                stack.push(p);
            }
        }
    }
    eprintln!("watching {watched} source folders (too many folders to watch the whole tree)");
}

fn analyze(path: &Path) -> Result<Project> {
    analyze_with(path, &mut |_| {})
}

/// [`analyze`] that reports how far it is to `report` (the loading page follows it).
fn analyze_with(path: &Path, report: &mut dyn FnMut(&iwr_core::progress::Progress)) -> Result<Project> {
    let started = Instant::now();
    let p = iwr_core::analyze_path_with(path, report).with_context(|| format!("analyzing {}", path.display()))?;
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

/// Say on stderr what is wrong with the codecast next to a freshly analyzed project.
fn check_script(project: &Project, script: Option<&Path>) -> Result<()> {
    if let Some(s) = script {
        let loaded = crate::codecast::load(s)?;
        let sc = iwr_core::script::parse(&loaded.text);
        for b in iwr_core::script::check(project, &sc) {
            eprintln!("script: {}", b);
        }
        if let Some(g) = &loaded.glossary {
            for b in crate::codecast::check_glossary(project, g) {
                eprintln!("script: {}", b);
            }
        }
        for a in crate::codecast::audio_files(&sc) {
            if !loaded.dir.join(&a).is_file() {
                eprintln!("script: audio file not found: {}", loaded.dir.join(&a).display());
            }
        }
        for stem in &loaded.stale {
            eprintln!("script: {}: text changed since its recording, spoken by TTS (re-record: codecast voice {}{})", stem, s.display(), crate::codecast::lang_arg(stem));
        }
        for b in crate::codecast::check_config(s) {
            eprintln!("script: {}", b);
        }
        eprintln!("script {} ({}): {} part(s), {} cue(s)", sc.title, s.display(), sc.parts.len(), sc.cue_count());
    }
    Ok(())
}

/// Analyze the project on a blocking thread, the loading page following through `/api/status`;
/// on success it becomes the served project (`version` bumped). The first analysis's error is
/// fatal (the server stops), a re-analysis's is reported and the old project stays.
async fn reanalyze(shared: &AppState, first: bool) -> Result<()> {
    {
        let mut st = shared.status.lock().unwrap();
        st.0.busy = true;
        st.0.error = None;
        st.0.progress = Default::default();
        st.1 = Some(Instant::now());
    }
    let path = shared.path();
    let for_report = shared.clone();
    let result = tokio::task::spawn_blocking(move || {
        analyze_with(&path, &mut |p| {
            let mut st = for_report.status.lock().unwrap();
            st.0.progress = p.clone();
        })
    })
    .await;
    let outcome = match result {
        Ok(Ok(p)) => {
            if let Err(e) = check_script(&p, shared.script().as_deref()) {
                if first {
                    return Err(e);
                }
                eprintln!("script: {e:#}");
            }
            let name = p.name.clone();
            *shared.project.write().unwrap() = Arc::new(p);
            {
                let mut v = shared.version.write().unwrap();
                *v = (*v).max(shared.past_version.load(Ordering::Relaxed)) + 1;
            }
            let mut st = shared.status.lock().unwrap();
            st.0.name = name;
            st.0.ready = true;
            st.0.progress.stage = iwr_core::progress::Stage::Done;
            Ok(())
        }
        Ok(Err(e)) => Err(e),
        Err(e) => Err(anyhow::anyhow!("analysis panicked: {e}")),
    };
    if let Err(e) = &outcome {
        let mut st = shared.status.lock().unwrap();
        st.0.progress.stage = iwr_core::progress::Stage::Failed;
        st.0.error = Some(format!("{e:#}"));
    }
    shared.status.lock().unwrap().0.busy = false;
    outcome
}

pub fn serve(path: PathBuf, port: u16, open_browser: bool, script: Option<PathBuf>, audio: Option<PathBuf>) -> Result<()> {
    // a `codecast/` directory or `codecast.md` next to the project is picked up automatically
    let script = script.or_else(|| crate::codecast::locate(&path));
    let name = path.canonicalize().ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).unwrap_or_default();
    let status = iwr_core::progress::Status { name, path: path.display().to_string(), ..Default::default() };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let root = Root { path: path.clone(), script, audio, topic: None };
    let shared: AppState = Arc::new(Shared { project: RwLock::new(Arc::new(Project::default())), version: RwLock::new(0), past_version: Default::default(), status: Mutex::new((status, None)), root: RwLock::new(root.clone()), first_root: root, changes: tx, watcher: Mutex::new(None), voice_job: Default::default(), voice_cancel: AtomicBool::new(false) });
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        // file watcher → re-analyze (debounced)
        watch_root(&shared);
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
                    if let Err(e) = reanalyze(&shared, false).await {
                        eprintln!("re-analysis failed: {e:#}");
                    }
                }
            });
        }

        let app = Router::new()
            .route("/api/project", get(api_project))
            .route("/api/version", get(api_version))
            .route("/api/status", get(api_status))
            .route("/api/casts", get(api_casts))
            .route("/api/root", post(api_set_root))
            .route("/api/topics", get(api_topics))
            .route("/api/topic", post(api_set_topic))
            .route("/api/script", get(api_script))
            .route("/api/glossary", get(api_glossary))
            .route("/api/audio", get(api_audio))
            .route("/api/config", get(api_config).post(api_save_config))
            .route("/api/font/{name}", post(api_save_font).layer(DefaultBodyLimit::max(MAX_FONT)))
            .route("/api/codecast/{*name}", get(api_codecast_file))
            .route("/api/translate/{lang}", post(api_translate))
            .route("/api/subtitles/{lang}", post(api_subtitles))
            .route("/api/voice", get(api_voice_status).post(api_voice_start))
            .route("/api/voice/job", get(api_voice_job))
            .route("/api/voice/cancel", post(api_voice_cancel))
            .route("/api/voice/refresh", post(api_voice_refresh))
            .route("/api/voice/clean", post(api_voice_clean))
            .route("/api/voice/complete", post(api_voice_complete))
            .route("/api/voice/drop", post(api_voice_drop))
            .route("/api/voice/preview", post(api_voice_preview))
            .fallback(static_handler)
            .layer(tower_http::cors::CorsLayer::permissive())
            .with_state(shared.clone());
        let addr = format!("127.0.0.1:{port}");
        let listener = tokio::net::TcpListener::bind(&addr).await.with_context(|| format!("binding {addr}"))?;
        let url = format!("http://{addr}");
        eprintln!("IwannatRust serving {} at {}", path.display(), url);
        if Assets::get("index.html").is_none() {
            eprintln!("warning: web UI not built. Run `dx build --release` in crates/iwr-ui (or ./build.sh) and rebuild codecast.");
        }
        if open_browser {
            let _ = open::that(&url);
        }
        // the page is up at once: it shows the analysis going while the first one runs
        let first = {
            let shared = shared.clone();
            async move { reanalyze(&shared, true).await }
        };
        let server = axum::serve(listener, app).into_future();
        tokio::pin!(server);
        tokio::select! {
            r = &mut server => r?,
            r = first => {
                // a failed first analysis stops the server; a good one keeps serving
                r?;
                server.await?;
            }
        }
        Ok::<(), anyhow::Error>(())
    })
}

async fn api_project(State(s): State<AppState>) -> Response {
    if *s.version.read().unwrap() == 0 {
        // still analyzing: the page shows `/api/status` meanwhile
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let p = s.project.read().unwrap().clone();
    // serialize on a blocking thread: projects can be large
    let body = tokio::task::spawn_blocking(move || serde_json::to_vec(&*p)).await.unwrap().unwrap();
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

async fn api_version(State(s): State<AppState>) -> Json<u64> {
    Json(*s.version.read().unwrap())
}

/// How far the running (or last) analysis is: what the loading page shows.
async fn api_status(State(s): State<AppState>) -> Json<iwr_core::progress::Status> {
    let st = s.status.lock().unwrap();
    let mut status = st.0.clone();
    if let Some(t) = st.1 {
        status.elapsed_ms = t.elapsed().as_millis() as u64;
    }
    Json(status)
}

/// The codecasts around the served project, as a folder tree: what the cast map draws.
async fn api_casts(State(s): State<AppState>) -> Response {
    let (path, topic) = (s.path(), s.topic());
    match tokio::task::spawn_blocking(move || crate::codecast::discover(&path, &topic)).await {
        Ok(map) => Json(map).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// What `POST /api/root` takes: a folder of the cast map, as its path from the map's top.
#[derive(serde::Deserialize)]
struct RootRequest {
    path: String,
    /// A topic of that folder's codecast to play, instead of its general codecast
    #[serde(default)]
    topic: Option<String>,
}

/// The topics of the served codecast and the one that plays: what the plan page offers.
async fn api_topics(State(s): State<AppState>) -> Response {
    let (cast, topic) = (s.cast(), s.topic());
    let list = match cast {
        Some(c) => tokio::task::spawn_blocking(move || crate::codecast::topic_list(&c, &topic)).await.unwrap_or_default(),
        None => Default::default(),
    };
    Json(list).into_response()
}

/// What `POST /api/topic` takes: a topic's folder name, or `""` for the general codecast.
#[derive(serde::Deserialize)]
struct TopicRequest {
    dir: String,
}

/// Play another topic of the served codecast (same project: nothing is analyzed again). The
/// version goes up, so the other open tabs see the change and follow.
async fn api_set_topic(State(s): State<AppState>, headers: HeaderMap, Json(req): Json<TopicRequest>) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match set_topic(&s, &req.dir) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(r) => r.into_response(),
    }
}

/// Make `dir` the topic that plays (`""`: the general codecast). Only a topic the codecast
/// has, looked up by name (never joined as given); refused while a recording runs.
fn set_topic(s: &AppState, dir: &str) -> Result<(), (StatusCode, &'static str)> {
    if s.voice_job.lock().unwrap().0.running {
        return Err((StatusCode::CONFLICT, "recording"));
    }
    let topic = if dir.is_empty() {
        None
    } else {
        let known = s.cast().map(|c| crate::codecast::topics(&c)).unwrap_or_default();
        match known.into_iter().find(|t| t.dir == dir) {
            Some(t) => Some(t.dir),
            None => return Err((StatusCode::NOT_FOUND, "no such topic")),
        }
    };
    if s.root.read().unwrap().topic == topic {
        return Ok(());
    }
    s.root.write().unwrap().topic = topic;
    eprintln!("now playing {}", s.script().map(|p| p.display().to_string()).unwrap_or_default());
    let mut v = s.version.write().unwrap();
    if *v != 0 {
        *v += 1;
        drop(v);
        if let Err(e) = check_script(&s.project.read().unwrap().clone(), s.script().as_deref()) {
            eprintln!("script: {e:#}");
        }
    }
    Ok(())
}

/// Serve the codecast of another folder of the cast map: the folder becomes the project, it is
/// analyzed from scratch (`version` back to `0`, the loading page follows) and its own codecast
/// is the script. Only folders the scan found a codecast in; refused while a recording or an
/// analysis runs.
async fn api_set_root(State(s): State<AppState>, headers: HeaderMap, Json(req): Json<RootRequest>) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if s.voice_job.lock().unwrap().0.running {
        return (StatusCode::CONFLICT, "recording").into_response();
    }
    let (path, topic) = (s.path(), s.topic());
    let Ok(map) = tokio::task::spawn_blocking(move || crate::codecast::discover(&path, &topic)).await else { return StatusCode::INTERNAL_SERVER_ERROR.into_response() };
    // the path is looked up in the scan, never joined as given: no way out of the tree
    let Some(node) = map.nodes.iter().find(|n| n.path == req.path && n.topic == req.topic && n.cast.is_some()) else { return (StatusCode::NOT_FOUND, "no codecast in that folder").into_response() };
    // the served folder: only the topic changes, nothing to analyze
    if map.nodes.iter().any(|n| n.path == req.path && n.topic.is_none() && n.current) {
        return match set_topic(&s, req.topic.as_deref().unwrap_or_default()) {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(r) => r.into_response(),
        };
    }
    let dir = node.path.split('/').filter(|p| !p.is_empty()).fold(PathBuf::from(&map.top), |d, p| d.join(p));
    let mut root = if dir == crate::codecast::project_dir(&s.first_root.path) {
        s.first_root.clone()
    } else {
        Root { script: crate::codecast::locate(&dir), path: dir.clone(), audio: None, topic: None }
    };
    root.topic = node.topic.clone();
    {
        let mut st = s.status.lock().unwrap();
        if st.0.busy {
            return (StatusCode::CONFLICT, "analyzing").into_response();
        }
        // taken here, not in `reanalyze`, so nothing else starts in between
        st.0 = iwr_core::progress::Status { name: node.name.clone(), path: dir.display().to_string(), busy: true, ..Default::default() };
        st.1 = Some(Instant::now());
        *s.root.write().unwrap() = root;
        let mut v = s.version.write().unwrap();
        s.past_version.fetch_max(*v, Ordering::Relaxed);
        *v = 0;
        drop(v);
        *s.project.write().unwrap() = Arc::new(Project::default());
    }
    eprintln!("now serving {}", dir.display());
    watch_root(&s);
    tokio::spawn(async move {
        if let Err(e) = reanalyze(&s, false).await {
            eprintln!("analysis failed: {e:#}");
        }
    });
    StatusCode::ACCEPTED.into_response()
}

/// The codecast script as text (re-read on every request so edits show up on reload).
/// A `codecast/` directory is assembled into one text.
async fn api_script(State(s): State<AppState>) -> Response {
    match &s.script() {
        Some(p) => match crate::codecast::load(p) {
            Ok(l) => ([(header::CONTENT_TYPE, "text/markdown; charset=utf-8")], l.text).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// The project's own glossary (`glossary.md` next to the script), re-read on every request.
async fn api_glossary(State(s): State<AppState>) -> Response {
    let Some(p) = s.script() else { return StatusCode::NOT_FOUND.into_response() };
    match std::fs::read_to_string(crate::codecast::glossary_path(&p)) {
        Ok(t) => ([(header::CONTENT_TYPE, "text/markdown; charset=utf-8")], t).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// The codecast's `codecast.toml` as JSON, re-read on every request.
async fn api_config(State(s): State<AppState>) -> Response {
    let Some(p) = s.script() else { return StatusCode::NOT_FOUND.into_response() };
    match crate::codecast::read_config(&p) {
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
    let Some(p) = s.script() else { return (StatusCode::CONFLICT, "no codecast next to the project: create codecast.md or codecast/ first").into_response() };
    let bad = c.problems();
    if !bad.is_empty() {
        return (StatusCode::BAD_REQUEST, bad.join("\n")).into_response();
    }
    let path = crate::codecast::config_dir(&p).join(iwr_core::config::FILE);
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
    let Some(p) = s.script() else { return (StatusCode::CONFLICT, "no codecast next to the project").into_response() };
    let rel = format!("fonts/{name}");
    if name.contains('/') || !crate::codecast::is_safe_asset_name(&rel) || !iwr_core::config::is_font_file(&name) {
        return (StatusCode::BAD_REQUEST, "not a font file name (ttf, otf, woff, woff2)").into_response();
    }
    let dir = crate::codecast::config_dir(&p).join("fonts");
    let res = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(&name), &body));
    match res {
        Ok(()) => rel.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// An audio file named by an `audio:` line, or a font of `codecast.toml`, relative to the script's directory.
async fn api_codecast_file(State(s): State<AppState>, AxPath(name): AxPath<String>) -> Response {
    let Some(script) = s.script() else { return StatusCode::NOT_FOUND.into_response() };
    if !crate::codecast::is_safe_asset_name(&name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match std::fs::read(crate::codecast::asset_path(&script, &name)) {
        Ok(b) => {
            let mime = mime_guess::from_path(&name).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref().to_string())], b).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Translate the texts the player sends (English, code in backticks): the author's own
/// translations next to the codecast, else the offline model; the first request for a language sets the model up, which takes a while.
async fn api_translate(State(s): State<AppState>, AxPath(lang): AxPath<String>, Json(texts): Json<Vec<String>>) -> Response {
    let dir = s.script().as_deref().map(crate::codecast::script_dir);
    match tokio::task::spawn_blocking(move || crate::translate::translate(&lang, &texts, false, dir.as_deref())).await {
        Ok(Ok(done)) => Json(done).into_response(),
        Ok(Err(e)) => {
            eprintln!("translate: {e:#}");
            (StatusCode::SERVICE_UNAVAILABLE, format!("{e:#}")).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// The translations of the texts the player sends that already exist (the codecast's
/// `translations/<lang>.json`, then the cache), `null` for the others; never runs the model.
async fn api_subtitles(State(s): State<AppState>, AxPath(lang): AxPath<String>, Json(texts): Json<Vec<String>>) -> Response {
    let dir = s.script().as_deref().map(crate::codecast::script_dir);
    match tokio::task::spawn_blocking(move || crate::translate::known(&lang, &texts, dir.as_deref())).await {
        Ok(Ok(done)) => Json(done).into_response(),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, format!("{e:#}")).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Every part of the codecast and its recordings (`codecast voice`), for the Generate page.
async fn api_voice_status(State(s): State<AppState>) -> Response {
    let script = s.script();
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
    let Some(dir) = s.script().filter(|d| d.is_dir()) else { return (StatusCode::CONFLICT, "recordings are made for a codecast/ directory").into_response() };
    if iwr_core::voice::voice_lang(&req.lang).is_none() || !iwr_core::voice::VOICES.contains(&req.voice.as_str()) {
        return (StatusCode::BAD_REQUEST, "unknown voice or language").into_response();
    }
    let opts = crate::voice::Opts { voice: req.voice.clone(), voice2: req.voice2.clone(), speed: req.speed.clamp(0.5, 2.0), lang: req.lang.clone(), path: Some(s.path()), gap: req.gap.clamp(0.0, 3.0), level: req.level.clamp(0.0, 1.0), force: req.force, only: req.parts.clone(), retranslate: req.retranslate, fresh: req.fresh, written: false };
    spawn_voice_job(s, dir, vec![opts], vec![], Some(req))
}

/// Record the out-of-date recordings again, each with the voice and delivery it was made with,
/// and delete the files of parts that are gone.
async fn api_voice_refresh(State(s): State<AppState>, headers: HeaderMap, Json(req): Json<iwr_core::voice::CleanRequest>) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(dir) = s.script().filter(|d| d.is_dir()) else { return (StatusCode::CONFLICT, "recordings are made for a codecast/ directory").into_response() };
    let st = crate::voice::status(Some(&dir));
    let opts = st
        .stale
        .iter()
        .filter(|g| req.groups.is_empty() || req.groups.contains(&g.key()))
        .map(|g| crate::voice::Opts { voice: g.voice.clone(), voice2: None, speed: g.speed, lang: g.arg.clone(), path: Some(s.path()), gap: g.gap, level: g.level, force: false, only: g.parts.clone(), retranslate: false, fresh: false, written: false })
        .collect();
    spawn_voice_job(s, dir, opts, st.orphans, None)
}

/// Delete out-of-date recordings (the parts are spoken by TTS until recorded again) and the
/// files of parts that are gone; answers how many files went.
async fn api_voice_clean(State(s): State<AppState>, headers: HeaderMap, Json(req): Json<iwr_core::voice::CleanRequest>) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(dir) = s.script().filter(|d| d.is_dir()) else { return (StatusCode::CONFLICT, "recordings are made for a codecast/ directory").into_response() };
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

/// Record what each asked voice set lacks (its changed and missing parts), with the voice and
/// delivery it was made with.
async fn api_voice_complete(State(s): State<AppState>, headers: HeaderMap, Json(req): Json<iwr_core::voice::CleanRequest>) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(dir) = s.script().filter(|d| d.is_dir()) else { return (StatusCode::CONFLICT, "recordings are made for a codecast/ directory").into_response() };
    let st = crate::voice::status(Some(&dir));
    let opts: Vec<_> = st
        .sets
        .iter()
        .filter(|g| req.groups.is_empty() || req.groups.contains(&g.key()))
        .filter(|g| !g.todo().is_empty())
        .map(|g| crate::voice::Opts { voice: g.voice.clone(), voice2: None, speed: g.speed, lang: g.arg.clone(), path: Some(s.path()), gap: g.gap, level: g.level, force: false, only: g.todo(), retranslate: false, fresh: false, written: false })
        .collect();
    if opts.is_empty() {
        return (StatusCode::CONFLICT, "nothing to record: those voices are complete").into_response();
    }
    spawn_voice_job(s, dir, opts, vec![], None)
}

/// Delete every recording of the asked voice sets; answers how many files went.
async fn api_voice_drop(State(s): State<AppState>, headers: HeaderMap, Json(req): Json<iwr_core::voice::CleanRequest>) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(dir) = s.script().filter(|d| d.is_dir()) else { return (StatusCode::CONFLICT, "recordings are made for a codecast/ directory").into_response() };
    if req.groups.is_empty() {
        return (StatusCode::BAD_REQUEST, "name the voices to delete").into_response();
    }
    if s.voice_job.lock().unwrap().0.running {
        return (StatusCode::CONFLICT, "a recording is running").into_response();
    }
    let st = crate::voice::status(Some(&dir));
    let names: Vec<String> = st.sets.into_iter().filter(|g| req.groups.contains(&g.key())).flat_map(|g| g.files).collect();
    match crate::voice::delete(&dir, &names) {
        Ok(n) => {
            eprintln!("voice: deleted {n} recording file(s)");
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
    match &s.audio() {
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
        if crate::codecast::has_general(s) {
            export_cast(s, &out, &project, langs)?;
        }
        // each topic is a codecast of its own under out/topics/<dir>/, listed in out/topics.json
        let topics = crate::codecast::topic_list(s, "");
        if !topics.topics.is_empty() {
            for t in &topics.topics {
                export_cast(&s.join(&t.dir), &out.join("topics").join(&t.dir), &project, langs)?;
            }
            std::fs::write(out.join("topics.json"), serde_json::to_vec(&topics)?)?;
            eprintln!("exported {} topic(s)", topics.topics.len());
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

/// Write one codecast (the general one, or a topic) into `out`: `codecast.md`, `glossary.md`,
/// `codecast.json`, recordings and fonts in `codecast/`, translations in `translations/`.
fn export_cast(s: &Path, out: &Path, project: &Project, langs: &[String]) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let loaded = crate::codecast::load(s)?;
    let sc = iwr_core::script::parse(&loaded.text);
    for b in iwr_core::script::check(project, &sc) {
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
            let src = crate::codecast::config_dir(s).join(&f);
            if src.is_file() {
                let dest = out.join("codecast").join(&f);
                std::fs::create_dir_all(dest.parent().unwrap())?;
                std::fs::copy(&src, dest)?;
            }
        }
    }
    // translations the player looks up when another language is picked: out/translations/<lang>.json
    for lang in langs {
        let map = crate::translate::codecast_map(s, project, lang)?;
        std::fs::create_dir_all(out.join("translations"))?;
        std::fs::write(out.join("translations").join(format!("{}.json", lang)), serde_json::to_vec(&map)?)?;
        eprintln!("translated the codecast to {} ({} pieces)", lang, map.len());
    }
    Ok(())
}
