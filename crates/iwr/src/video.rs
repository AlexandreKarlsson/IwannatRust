//! `codecast video`: film each recorded part of a codecast as an MP4 (H.264 + AAC).
//!
//! The player runs in its render mode (`/?render=N&voice=V`: the stage, the subtitles if
//! asked for, no controls, the theme and fonts of `codecast.toml`) inside a headless Chromium (`chromium-headless-shell`, the only build that lets a
//! client draw frames itself). Time is virtual: for every frame the renderer sets the player's
//! clock (`window.__codecastTime(t)`), lets the page's timers and animations run exactly one
//! frame (`Emulation.setVirtualTimePolicy`) and asks for that frame and its picture
//! (`HeadlessExperimental.beginFrame`). Nothing depends on how fast the machine is: a frame
//! shows what the player shows at its time, and the picture lines up with the recording that
//! ffmpeg lays under it.
//!
//! Chromium and ffmpeg on `PATH` film natively (Linux only: frame times are read on the clock
//! Chromium uses). Without them, [`in_docker`] runs this same command in the `codecast-video`
//! image (`docker/video/Dockerfile`: Chromium, ffmpeg and fonts), built from this repository.

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};
use tokio_tungstenite::tungstenite::Message;

/// What to film and how (`codecast video` flags).
pub struct Opts {
    /// the project the codecast explains (served to the player)
    pub path: PathBuf,
    /// recorded voice to film (`""`: the one that recorded the most parts)
    pub voice: String,
    /// language of the recordings and captions (`en`, or one `codecast voice --lang` recorded)
    pub lang: String,
    /// language of the subtitles under the picture (`""`: none, the stage fills the video)
    pub sub: String,
    /// part numbers to film, 1-based (empty: all)
    pub parts: Vec<usize>,
    pub fps: u32,
    /// output size in pixels
    pub width: u32,
    pub height: u32,
    /// device pixel ratio: the page is laid out at `width / scale` × `height / scale`, so text
    /// is larger in the video than in a browser of that size
    pub scale: f64,
    /// x264 quality (lower: better, bigger)
    pub crf: u32,
    /// where the MP4s go (default: next to the recordings)
    pub out: Option<PathBuf>,
    /// film parts whose video is newer than its recording and text too
    pub force: bool,
    /// Chromium binary (default: `chromium-headless-shell` / `chrome-headless-shell` on `PATH`)
    pub chrome: Option<String>,
    /// seconds held on the last picture after the recording ends
    pub tail: f64,
}

/// One part to film: its number in the script, recording and output.
struct Job {
    /// 1-based, as in `?render=N`
    n: usize,
    name: String,
    audio: PathBuf,
    duration: f64,
    out: PathBuf,
}

/// Film the parts of the codecast at `script` (a `codecast/` directory or a single file).
pub fn run(script: &Path, o: &Opts) -> Result<()> {
    let loaded = crate::codecast::load(script)?;
    let mut s = iwr_core::script::parse(&loaded.text);
    let lang = o.lang.as_str();
    let voices = iwr_core::i18n::recorded_voices(&s, lang);
    // `--voice am_michael` also names a pair recorded with it (`am_michael+af_heart`: `sp:` lines)
    let pair = format!("{}+", o.voice);
    let voice = match o.voice.as_str() {
        "" => voices.first().map(|(v, _)| v.clone()).unwrap_or_default(),
        v => voices.iter().map(|(n, _)| n).find(|n| *n == v).or_else(|| voices.iter().map(|(n, _)| n).find(|n| n.starts_with(&pair))).cloned().unwrap_or_else(|| v.to_string()),
    };
    if !o.voice.is_empty() && !voices.iter().any(|(v, _)| *v == voice) {
        let have: Vec<_> = voices.iter().map(|(v, n)| format!("{v} ({n} parts)")).collect();
        bail!("no {lang} recording by voice {voice}; recorded: {}", if have.is_empty() { "none".into() } else { have.join(", ") });
    }
    if voices.is_empty() {
        bail!("no {lang} recording in {} (record one: codecast voice {} --lang {lang})", script.display(), script.display());
    }
    iwr_core::i18n::use_voices(&mut s, lang, &voice);
    let out_dir = o.out.clone().unwrap_or_else(|| loaded.dir.clone());
    std::fs::create_dir_all(&out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let mut jobs = vec![];
    for (i, p) in s.parts.iter().enumerate() {
        let n = i + 1;
        if !o.parts.is_empty() && !o.parts.contains(&n) {
            continue;
        }
        let Some(a) = &p.audio else {
            let stale = loaded.stale.iter().any(|st| st.starts_with(&format!("{:02}-", n)));
            eprintln!("part {n} {}: skipped, {}", p.name, if stale { "its recording is stale (text changed): run `codecast voice` again" } else { "no recording" });
            continue;
        };
        let audio = loaded.dir.join(a);
        let stem = Path::new(a).file_stem().and_then(|x| x.to_str()).unwrap_or("part").to_string();
        let out = out_dir.join(if o.sub.is_empty() { format!("{stem}.mp4") } else { format!("{stem}.sub-{}.mp4", o.sub) });
        let times = audio.with_extension("times");
        let duration = read_duration(&times).or_else(|| probe_duration(&audio)).ok_or_else(|| anyhow!("{}: duration unknown (no `# duration` in {} and ffprobe failed)", a, times.display()))?;
        // the part's text file: `01-welcome.am_michael` → `01-welcome.md`
        let md = loaded.dir.join(format!("{}.md", stem.split('.').next().unwrap_or(&stem)));
        if !o.force && is_newer(&out, &[&audio, &times, &md]) {
            eprintln!("part {n} {}: {} is up to date (--force films it again)", p.name, out.display());
            continue;
        }
        jobs.push(Job { n, name: p.name.clone(), audio, duration, out });
    }
    if jobs.is_empty() {
        eprintln!("nothing to film");
        return Ok(());
    }
    which("ffmpeg").ok_or_else(|| anyhow!("ffmpeg not found on PATH (without --chrome, codecast video films in Docker when ffmpeg is missing)"))?;

    let server = Server::start(&o.path, script)?;
    let chrome = Chrome::start(o)?;
    let rt = tokio::runtime::Runtime::new()?;
    let total = jobs.len();
    for (k, j) in jobs.iter().enumerate() {
        eprintln!("[{}/{total}] part {} {} — {:.0} s → {}", k + 1, j.n, j.name, j.duration, j.out.display());
        let url = format!("{}/?render={}&voice={}&lang={}&sub={}", server.url, j.n, voice, lang, o.sub);
        rt.block_on(film(&chrome.ws, &url, j, o)).with_context(|| format!("filming part {} ({})", j.n, j.name))?;
    }
    drop(chrome);
    drop(server);
    Ok(())
}

/// `# duration 86.45` in a `.times` file written by `codecast voice`.
fn read_duration(times: &Path) -> Option<f64> {
    let text = std::fs::read_to_string(times).ok()?;
    text.lines().find_map(|l| l.strip_prefix("# duration ")).and_then(|d| d.trim().parse().ok())
}

fn probe_duration(audio: &Path) -> Option<f64> {
    let out = Command::new("ffprobe").args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"]).arg(audio).output().ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// Is `out` newer than every one of `inputs` that exists?
fn is_newer(out: &Path, inputs: &[&Path]) -> bool {
    let m = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let Some(t) = m(out) else { return false };
    inputs.iter().filter_map(|p| m(p)).all(|i: SystemTime| i <= t)
}

/// The Docker image that films when this machine cannot.
const IMAGE: &str = "codecast-video";

/// Set inside the image: the recordings were made on the host, so nothing is recorded there.
pub const IN_DOCKER: &str = "CODECAST_VIDEO_IN_DOCKER";

/// Chromium (or `--chrome`) and ffmpeg are here, so the parts are filmed on this machine.
pub fn native(o: &Opts) -> bool {
    let chrome = o.chrome.is_some() || ["chromium-headless-shell", "chrome-headless-shell"].into_iter().any(|b| which(b).is_some());
    chrome && which("ffmpeg").is_some()
}

/// Film in the `codecast-video` image: build it from this repository (cached after the first
/// time), mount the project, the codecast and the output, and run `codecast video` there with
/// the same options, as the calling user so the MP4s are theirs.
pub fn in_docker(script: &Path, o: &Opts) -> Result<()> {
    which("docker").ok_or_else(|| anyhow!("filming needs chromium-headless-shell and ffmpeg on PATH, or Docker to run them in"))?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dockerfile = root.join("docker/video/Dockerfile");
    if !dockerfile.is_file() {
        bail!("no chromium-headless-shell + ffmpeg on PATH, and no {} to build the Docker image from: install them", dockerfile.display());
    }
    let root = root.canonicalize()?;
    eprintln!("no chromium-headless-shell + ffmpeg here: filming in Docker ({IMAGE}, built from {})", root.display());
    let st = Command::new("docker").env("DOCKER_BUILDKIT", "1").args(["build", "-q", "-t", IMAGE, "-f"]).arg(&dockerfile).arg(&root).stdout(Stdio::null()).status().context("running docker build")?;
    if !st.success() {
        bail!("docker build failed ({st})");
    }

    // host directories → /mnt/N in the container; a path under one already mounted reuses it
    let mut mounts: Vec<PathBuf> = Vec::new();
    let mut inside = |p: &Path| -> Result<String> {
        let p = std::path::absolute(p)?;
        let (i, rel) = match mounts.iter().enumerate().find_map(|(i, m)| p.strip_prefix(m).ok().map(|r| (i, r.to_path_buf()))) {
            Some(hit) => hit,
            None => {
                mounts.push(p.clone());
                (mounts.len() - 1, PathBuf::new())
            }
        };
        Ok(Path::new(&format!("/mnt/{i}")).join(rel).to_string_lossy().into_owned())
    };
    let path = inside(&o.path)?;
    let script_in = inside(script)?;
    let out = o.out.as_deref().map(|d| std::fs::create_dir_all(d).map(|_| d)).transpose()?.map(&mut inside).transpose()?;

    let id = |flag: &str| Command::new("id").arg(flag).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let mut cmd = Command::new("docker");
    cmd.args(["run", "--rm", "--init", "--user", &format!("{}:{}", id("-u"), id("-g")), "-e", &format!("{IN_DOCKER}=1")]);
    for (i, m) in mounts.iter().enumerate() {
        cmd.arg("-v").arg(format!("{}:/mnt/{i}", m.display()));
    }
    // translations made on this machine (--lang): the image has no translation model
    if let Ok(tr) = crate::python::home("CODECAST_TRANSLATE", "translate") {
        if tr.is_dir() {
            cmd.arg("-v").arg(format!("{}:/tmp/.cache/codecast/translate:ro", tr.display()));
        }
    }
    cmd.args([IMAGE, "video", &script_in, "--path", &path, "--lang", &o.lang]);
    cmd.args(["--fps", &o.fps.to_string(), "--size", &format!("{}x{}", o.width, o.height), "--scale", &o.scale.to_string()]);
    cmd.args(["--crf", &o.crf.to_string(), "--tail", &o.tail.to_string()]);
    if !o.voice.is_empty() {
        cmd.args(["--voice", &o.voice]);
    }
    // explicit, so the `[video] sub` of codecast.toml is not read again in there
    cmd.args(["--sub", if o.sub.is_empty() { "none" } else { &o.sub }]);
    if !o.parts.is_empty() {
        cmd.arg("--parts").arg(o.parts.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(","));
    }
    if let Some(d) = &out {
        cmd.args(["-o", d]);
    }
    if o.force {
        cmd.arg("--force");
    }
    let st = cmd.status().context("running docker")?;
    if !st.success() {
        bail!("filming in Docker failed ({st})");
    }
    Ok(())
}

fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(bin)).find(|f| f.is_file()))
}

/// `codecast serve` of the project in a child process, on a free loopback port.
struct Server {
    child: Child,
    url: String,
}

impl Server {
    fn start(project: &Path, script: &Path) -> Result<Server> {
        let port = std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
        let exe = std::env::current_exe()?;
        let child = Command::new(exe)
            .arg("serve")
            .arg(project)
            .args(["--no-open", "-p", &port.to_string(), "--script"])
            .arg(script)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("starting codecast serve")?;
        let mut server = Server { child, url: format!("http://127.0.0.1:{port}") };
        let started = Instant::now();
        // analysis of a big project takes a while: wait for the API to answer
        while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
            if let Some(st) = server.child.try_wait()? {
                bail!("codecast serve exited ({st}) before listening");
            }
            if started.elapsed() > Duration::from_secs(120) {
                bail!("codecast serve did not listen within 2 minutes");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(server)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A headless Chromium that lets its client draw the frames (`--enable-begin-frame-control`).
struct Chrome {
    child: Child,
    /// browser DevTools websocket
    ws: String,
    profile: PathBuf,
}

impl Chrome {
    fn start(o: &Opts) -> Result<Chrome> {
        let bin = match &o.chrome {
            Some(b) => b.clone(),
            None => ["chromium-headless-shell", "chrome-headless-shell"]
                .into_iter()
                .find(|b| which(b).is_some())
                .ok_or_else(|| anyhow!("chromium-headless-shell not found on PATH: pass --chrome, or drop it to film in Docker (a regular Chrome cannot draw frames on demand)"))?
                .to_string(),
        };
        let profile = std::env::temp_dir().join(format!("codecast-video-{}", std::process::id()));
        let mut cmd = Command::new(&bin);
        cmd.args([
            "--remote-debugging-port=0",
            "--enable-begin-frame-control",
            "--run-all-compositor-stages-before-draw",
            "--disable-new-content-rendering-timeout",
            "--disable-threaded-animation",
            "--disable-threaded-scrolling",
            "--disable-checker-imaging",
            "--disable-image-animation-resync",
            "--disable-dev-shm-usage",
            "--hide-scrollbars",
            "--mute-audio",
            "--no-first-run",
        ])
        .arg(format!("--user-data-dir={}", profile.display()));
        // root, or any user inside a container, cannot use Chromium's sandbox
        if unsafe { libc::geteuid() } == 0 || Path::new("/.dockerenv").exists() {
            cmd.arg("--no-sandbox");
        }
        let mut child = cmd.arg("about:blank").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().with_context(|| format!("starting {bin}"))?;
        // the browser prints its websocket on stderr: `DevTools listening on ws://…`
        let mut err = BufReader::new(child.stderr.take().unwrap());
        let mut line = String::new();
        let ws = loop {
            line.clear();
            if err.read_line(&mut line)? == 0 {
                let _ = child.wait();
                bail!("{bin} exited before opening DevTools");
            }
            if let Some(i) = line.find("ws://") {
                break line[i..].trim().to_string();
            }
        };
        // keep draining stderr so Chromium never blocks on a full pipe
        std::thread::spawn(move || {
            let mut sink = vec![];
            let _ = err.read_to_end(&mut sink);
        });
        Ok(Chrome { child, ws, profile })
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

/// The clock Chromium's frame times (`frameTimeTicks`) are on: `CLOCK_MONOTONIC`, in ms.
fn monotonic_ms() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as f64 * 1000.0 + ts.tv_nsec as f64 / 1e6
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// A minimal DevTools client: one command at a time, events kept until asked for.
struct Cdp {
    ws: Ws,
    id: u64,
    session: Option<String>,
    events: Vec<Value>,
}

impl Cdp {
    async fn connect(url: &str) -> Result<Cdp> {
        let (ws, _) = tokio_tungstenite::connect_async(url).await.context("connecting to Chromium DevTools")?;
        Ok(Cdp { ws, id: 0, session: None, events: vec![] })
    }

    async fn next(&mut self) -> Result<Value> {
        loop {
            let m = tokio::time::timeout(Duration::from_secs(60), self.ws.next()).await.map_err(|_| anyhow!("Chromium stopped answering"))?;
            match m {
                Some(Ok(Message::Text(t))) => return Ok(serde_json::from_str(t.as_str())?),
                Some(Ok(_)) => continue,
                Some(Err(e)) => return Err(e.into()),
                None => bail!("Chromium closed the connection"),
            }
        }
    }

    /// Send a command (to the attached page, once there is one) and wait for its answer.
    async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.id += 1;
        let id = self.id;
        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = &self.session {
            msg["sessionId"] = json!(s);
        }
        self.ws.send(Message::Text(msg.to_string().into())).await?;
        loop {
            let m = self.next().await?;
            if m["id"].as_u64() == Some(id) {
                if let Some(e) = m.get("error") {
                    bail!("{method}: {e}");
                }
                return Ok(m["result"].clone());
            }
            if m.get("method").is_some() {
                self.events.push(m);
            }
        }
    }

    async fn wait_event(&mut self, method: &str) -> Result<Value> {
        if let Some(i) = self.events.iter().position(|e| e["method"] == method) {
            return Ok(self.events.remove(i));
        }
        loop {
            let m = self.next().await?;
            if m["method"] == method {
                return Ok(m);
            }
        }
    }

    async fn eval(&mut self, expr: &str) -> Result<Value> {
        let r = self.call("Runtime.evaluate", json!({ "expression": expr, "returnByValue": true })).await?;
        if let Some(e) = r.get("exceptionDetails") {
            bail!("page error: {e}");
        }
        Ok(r["result"]["value"].clone())
    }
}

/// Virtual time, one frame at a time.
struct Frames {
    dt: f64,
    base: f64,
    tick: f64,
}

impl Frames {
    /// Run the page's timers and animations for one frame, draw it, and return its JPEG.
    async fn step(&mut self, cdp: &mut Cdp, shot: bool) -> Result<Option<Vec<u8>>> {
        cdp.events.retain(|e| e["method"] != "Emulation.virtualTimeBudgetExpired");
        cdp.call("Emulation.setVirtualTimePolicy", json!({ "policy": "advance", "budget": self.dt, "maxVirtualTimeTaskStarvationCount": 500 })).await?;
        cdp.wait_event("Emulation.virtualTimeBudgetExpired").await?;
        self.tick += self.dt;
        let mut p = json!({ "frameTimeTicks": self.base + self.tick, "interval": self.dt });
        if shot {
            p["screenshot"] = json!({ "format": "jpeg", "quality": 95 });
        }
        let r = cdp.call("HeadlessExperimental.beginFrame", p).await?;
        match r["screenshotData"].as_str() {
            Some(b) => Ok(Some(base64::engine::general_purpose::STANDARD.decode(b)?)),
            None => Ok(None),
        }
    }
}

/// Film one part into `j.out` (written to a temporary name, renamed when complete).
async fn film(browser_ws: &str, url: &str, j: &Job, o: &Opts) -> Result<()> {
    // Open a tab whose frames we draw ourselves
    let mut cdp = Cdp::connect(browser_ws).await?;
    let t = cdp.call("Target.createTarget", json!({ "url": "about:blank", "enableBeginFrameControl": true })).await?;
    let target = t["targetId"].as_str().unwrap_or_default().to_string();
    let a = cdp.call("Target.attachToTarget", json!({ "targetId": target, "flatten": true })).await?;
    cdp.session = a["sessionId"].as_str().map(str::to_string);
    cdp.call("Page.enable", json!({})).await?;
    cdp.call("Runtime.enable", json!({})).await?;

    // Load the player in render mode, time frozen
    let (w, h) = ((o.width as f64 / o.scale).round(), (o.height as f64 / o.scale).round());
    cdp.call("Emulation.setDeviceMetricsOverride", json!({ "width": w, "height": h, "deviceScaleFactor": o.scale, "mobile": false })).await?;
    cdp.call("Emulation.setVirtualTimePolicy", json!({ "policy": "pause" })).await?;
    cdp.call("Page.navigate", json!({ "url": url })).await?;

    let mut fr = Frames { dt: 1000.0 / o.fps as f64, base: monotonic_ms(), tick: 0.0 };
    let mut waited = 0;
    // Wait for the player: translated, recording in, clock hook, fonts
    while cdp.eval("window.__codecastReady === true && typeof window.__codecastTime === 'function' && document.fonts.status === 'loaded'").await?.as_bool() != Some(true) {
        if let Some(e) = cdp.eval("window.__codecastError || ''").await?.as_str().filter(|e| !e.is_empty()) {
            bail!("the player says: {e}");
        }
        fr.step(&mut cdp, false).await?;
        waited += 1;
        if waited > 60 * o.fps {
            bail!("the player never got ready (no recording for this part in the page?)");
        }
    }
    cdp.eval("window.__codecastTime(0)").await?;
    // Let the first cue's camera settle for one second; the codecast's fonts (codecast.toml)
    // start loading once text uses them: wait for them too
    for _ in 0..o.fps {
        fr.step(&mut cdp, false).await?;
    }
    let mut waited = 0;
    while cdp.eval("document.fonts.status === 'loaded'").await?.as_bool() != Some(true) {
        fr.step(&mut cdp, false).await?;
        waited += 1;
        if waited > 30 * o.fps {
            bail!("the codecast's fonts never loaded");
        }
    }

    // Start ffmpeg: JPEGs in, the recording laid under them
    let tmp = j.out.with_extension("part.mp4");
    let mut ff = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-f", "image2pipe", "-c:v", "mjpeg", "-framerate", &o.fps.to_string(), "-i", "-", "-i"])
        .arg(&j.audio)
        .args(["-map", "0:v", "-map", "1:a", "-c:v", "libx264", "-preset", "medium", "-crf", &o.crf.to_string(), "-pix_fmt", "yuv420p"])
        .args(["-vf", &format!("scale={}:{}", o.width, o.height)])
        .args(["-c:a", "aac", "-b:a", "160k", "-af", "apad", "-shortest", "-movflags", "+faststart", "-f", "mp4"])
        .arg(&tmp)
        .stdin(Stdio::piped())
        .spawn()
        .context("starting ffmpeg")?;
    let mut pipe = ff.stdin.take().unwrap();
    let frames = ((j.duration + o.tail) * o.fps as f64).ceil() as usize;
    let started = Instant::now();
    let mut last_report = Instant::now();
    let mut last: Option<Vec<u8>> = None;
    // Film every frame: set the time, run one frame, draw it
    for i in 0..frames {
        let t = (i as f64 / o.fps as f64).min(j.duration);
        cdp.eval(&format!("window.__codecastTime({t:.4})")).await?;
        // a frame with nothing new to draw comes back empty: repeat the previous picture
        let img = match fr.step(&mut cdp, true).await? {
            Some(img) => img,
            None => last.clone().ok_or_else(|| anyhow!("first frame came back empty"))?,
        };
        pipe.write_all(&img).context("ffmpeg stopped reading")?;
        last = Some(img);
        if last_report.elapsed() > Duration::from_secs(5) || i + 1 == frames {
            last_report = Instant::now();
            let el = started.elapsed().as_secs_f64();
            eprint!("\r  {:>3}%  {}/{} frames  {:.1} fps   ", (i + 1) * 100 / frames, i + 1, frames, (i + 1) as f64 / el.max(0.001));
        }
    }
    eprintln!();

    // Close the pipe and keep the file only if ffmpeg succeeded
    drop(pipe);
    let st = ff.wait()?;
    let _ = cdp.call("Target.closeTarget", json!({ "targetId": target })).await;
    if !st.success() {
        let _ = std::fs::remove_file(&tmp);
        bail!("ffmpeg failed ({st})");
    }
    std::fs::rename(&tmp, &j.out)?;
    eprintln!("  ✔ {} ({:.0} s of video in {:.0} s)", j.out.display(), j.duration + o.tail, started.elapsed().as_secs_f64());
    Ok(())
}
