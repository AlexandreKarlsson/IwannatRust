use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod codecast;
mod serve;
mod translate;
mod voice;

/// The codecast writing guide, embedded so `iwr brief --guide` works from any directory.
const WRITING_GUIDE: &str = include_str!("../../../docs/writing-codecasts.md");

#[derive(Parser)]
#[command(name = "iwr", version, about = "IwannatRust: visualize how Rust code works")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Analyze a project and serve the interactive UI on localhost
    Serve {
        /// Project directory (with Cargo.toml) or a single .rs file
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(short, long, default_value_t = 4321)]
        port: u16,
        /// Do not open the browser
        #[arg(long)]
        no_open: bool,
        /// Codecast: a script file or a `codecast/` directory (docs/codecast.md); defaults to <path>/codecast{/,.md,.txt} when present
        #[arg(long)]
        script: Option<PathBuf>,
        /// Recorded codecast audio (mp3/ogg/wav) matching the script's [t] cues
        #[arg(long)]
        audio: Option<PathBuf>,
    },
    /// Analyze a project and write the model as JSON
    Analyze {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output file (stdout when omitted)
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Pretty-print
        #[arg(long)]
        pretty: bool,
    },
    /// Write a self-contained static site (UI + project.json) that works without a server
    Export {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output directory
        #[arg(short, long, default_value = "iwr-site")]
        out: PathBuf,
        #[arg(long)]
        script: Option<PathBuf>,
        #[arg(long)]
        audio: Option<PathBuf>,
        /// Also translate the codecast into these languages (offline), for the player's language menu: --lang ja,fr
        #[arg(long, value_delimiter = ',')]
        lang: Vec<String>,
    },
    /// Print a compact text brief of the project for writing a codecast script (LLM-friendly)
    Brief {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// One function (item path) plus what it calls
        #[arg(long = "fn")]
        only: Option<String>,
        /// Signatures only, no bodies
        #[arg(long)]
        no_body: bool,
        /// Skip the modules/types overview
        #[arg(long)]
        no_overview: bool,
        /// Prepend the guide on how to write a good codecast (docs/writing-codecasts.md)
        #[arg(long)]
        guide: bool,
    },
    /// Emit the built-in walkthrough as a codecast script (text format)
    Guide {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Start from this function instead of main
        #[arg(long = "fn")]
        only: Option<String>,
    },
    /// Print a codecast as it will be spoken (code said as words), to review the TTS text
    Speak {
        /// A script file or a `codecast/` directory
        script: PathBuf,
        /// Project path, for the project glossary's `pronounce:` lines (defaults to the script's parent)
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Record a `codecast/` directory with Kokoro TTS: NN-part.ogg + NN-part.times per part file
    Voice {
        /// The `codecast/` directory
        script: PathBuf,
        /// Kokoro voice (af_heart, af_bella, am_michael, bf_emma, bm_george, …)
        #[arg(long, default_value = voice::DEFAULT_VOICE)]
        voice: String,
        /// Kokoro voice of the `sp:` lines (default: one of the other gender in the language;
        /// the same voice spoken higher when the language has only one) → NN-part.<voice>+<voice2>.ogg
        #[arg(long)]
        voice2: Option<String>,
        #[arg(long, default_value_t = 1.0)]
        speed: f32,
        /// Kokoro language (en-us, en-gb), or a codecast language to translate into and record:
        /// ja, fr, es, it, pt → NN-part.<lang>.ogg + .times (default voice of that language)
        #[arg(long, default_value = "en-us")]
        lang: String,
        /// Project path, for the names kept as code when translating (defaults to the directory's parent)
        #[arg(long)]
        path: Option<PathBuf>,
        /// Silence between cues, in seconds
        #[arg(long, default_value_t = 0.4)]
        gap: f32,
        /// Opus compression level, 0..1 (0.95 ≈ 18 kbps; lower = bigger, cleaner)
        #[arg(long, default_value_t = 0.95)]
        level: f32,
        /// Re-record parts that are up to date (each voice has its own files: recording another voice keeps this one)
        #[arg(long)]
        force: bool,
        /// Translate the parts again instead of taking the cached translations (with --lang)
        #[arg(long)]
        retranslate: bool,
        /// Synthesize every cue again instead of taking the cached clips
        #[arg(long)]
        fresh: bool,
    },
    /// Translate a codecast offline (Argos models, cached) and print it; the player translates the same way when another language is picked
    Translate {
        /// A script file or a `codecast/` directory
        script: PathBuf,
        /// Target language: ja, fr, de, es, it, pt, sv, zh, ko
        #[arg(long)]
        lang: String,
        /// Project path (for generated tours and the names kept as code)
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the pieces to translate as JSON (no model), to fill with another tool and save as
        /// `translations/<lang>.json` next to the codecast
        #[arg(long)]
        pieces: bool,
    },
    /// Validate a codecast's refs against the project (a script file or a `codecast/` directory)
    Check {
        script: PathBuf,
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Print a text summary (items, calls) for debugging
    Summary {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Serve { path, port, no_open, script, audio } => serve::serve(path, port, !no_open, script, audio),
        Cmd::Analyze { path, out, pretty } => {
            let p = iwr_core::analyze_path(&path)?;
            let s = if pretty { serde_json::to_string_pretty(&p)? } else { serde_json::to_string(&p)? };
            match out {
                Some(o) => std::fs::write(o, s)?,
                None => println!("{}", s),
            }
            Ok(())
        }
        Cmd::Export { path, out, script, audio, lang } => serve::export(path, out, script, audio, &lang),
        Cmd::Brief { path, only, no_body, no_overview, guide } => {
            let p = iwr_core::analyze_path(&path)?;
            if guide {
                println!("{}\n\n---\n", WRITING_GUIDE);
            }
            print!("{}", iwr_core::brief::build(&p, &iwr_core::brief::BriefOptions { only, bodies: !no_body, overview: !no_overview }));
            Ok(())
        }
        Cmd::Guide { path, only } => {
            let p = iwr_core::analyze_path(&path)?;
            let root = only.as_deref().and_then(|o| iwr_core::script::resolve(&p, o)).and_then(|r| r.item);
            let steps = iwr_core::guide::build(&p, root, &iwr_core::guide::GuideOptions::default());
            print!("{}", iwr_core::script::to_text(&iwr_core::script::from_guide(&p, &steps)));
            Ok(())
        }
        Cmd::Speak { script, path } => {
            let loaded = codecast::load(&script)?;
            let mut s = iwr_core::script::parse(&loaded.text);
            // generated tours read the project; without one they are left out
            let root = path.unwrap_or_else(|| script.canonicalize().ok().and_then(|f| f.parent().map(|d| d.to_path_buf())).unwrap_or_else(|| PathBuf::from(".")));
            if s.parts.iter().any(|p| p.cues.iter().any(|c| c.tour.is_some())) {
                if let Ok(p) = iwr_core::analyze_path(&root) {
                    iwr_core::script::expand_tours(&p, &mut s);
                }
            }
            let pron = codecast::pronounce(&s, loaded.glossary.as_deref());
            println!("# {}", s.title);
            for p in &s.parts {
                println!("\n## {}", p.name);
                for c in &p.cues {
                    let who = if c.second() { "sp: " } else { "" };
                    println!("{}{}", who, iwr_core::speech::spoken(&c.say, &pron));
                    for q in &c.questions {
                        println!("  ? {}", iwr_core::speech::spoken(&q.ask, &pron));
                        println!("    {}", iwr_core::speech::spoken(&q.answer, &pron));
                    }
                }
            }
            Ok(())
        }
        Cmd::Voice { script, voice, voice2, speed, lang, path, gap, level, force, retranslate, fresh } => voice::run(&script, &voice::Opts { voice, voice2, speed, lang, path, gap, level, force, only: vec![], retranslate, fresh }),
        Cmd::Translate { script, lang, path, pieces } => if pieces { translate::print_pieces(&script, &path, &lang) } else { translate::run(&script, &path, &lang) },
        Cmd::Check { path, script } => {
            let p = iwr_core::analyze_path(&path)?;
            let loaded = codecast::load(&script)?;
            let s = iwr_core::script::parse(&loaded.text);
            let mut bad = iwr_core::script::check(&p, &s);
            if let Some(g) = &loaded.glossary {
                bad.extend(codecast::check_glossary(&p, g));
            }
            for a in codecast::audio_files(&s) {
                if !loaded.dir.join(&a).is_file() {
                    bad.push(format!("audio file not found: {}", loaded.dir.join(&a).display()));
                }
            }
            bad.extend(codecast::check_config(&script));
            for stem in &loaded.stale {
                println!("  note: {}: text changed since its recording, TTS speaks it until `iwr voice {}{}`", stem, script.display(), codecast::lang_arg(stem));
            }
            for n in iwr_core::script::stage_notes(&s) {
                println!("  note: {n}");
            }
            let pron = codecast::pronounce(&s, loaded.glossary.as_deref());
            for p in &s.parts {
                for (i, c) in p.cues.iter().enumerate() {
                    for u in iwr_core::speech::unspoken(&c.say, &pron) {
                        println!("  note: {} / cue {}: `{}` will be spoken as written; add a `pronounce:` line", p.name, i + 1, u);
                    }
                    let answers = c.questions.iter().map(|q| q.answer.as_str());
                    for w in std::iter::once(c.say.as_str()).chain(answers).flat_map(iwr_core::script::hard_to_say) {
                        println!("  note: {} / cue {}: `{}` is hard to say; say it in words and show it with a `^` line", p.name, i + 1, w);
                    }
                }
            }
            let nq: usize = s.parts.iter().flat_map(|p| &p.cues).map(|c| c.questions.len()).sum();
            let nd: usize = s.parts.iter().map(|p| p.diagrams.len()).sum();
            let nb: usize = s.parts.iter().map(|p| p.layouts.len()).sum();
            let nn: usize = s.parts.iter().map(|p| p.notes.len()).sum();
            let ne: usize = s.parts.iter().map(|p| p.examples.len()).sum();
            let ng = loaded.glossary.as_deref().map(|g| iwr_core::glossary::parse(g).len()).unwrap_or(0);
            println!(
                "{}: {} part(s), {} cue(s), {} question(s){}{}{}{}{}{}",
                s.title,
                s.parts.len(),
                s.cue_count(),
                nq,
                if nd > 0 { format!(", {} diagram(s)", nd) } else { String::new() },
                if nb > 0 { format!(", {} byte layout(s)", nb) } else { String::new() },
                if nn > 0 { format!(", {} side note(s)", nn) } else { String::new() },
                if ne > 0 { format!(", {} example(s)", ne) } else { String::new() },
                if ng > 0 { format!(", {} glossary term(s)", ng) } else { String::new() },
                if s.glossary { "" } else { ", built-in glossary off" }
            );
            for b in &bad {
                println!("  {}", b);
            }
            if bad.is_empty() {
                println!("all refs resolve");
                Ok(())
            } else {
                std::process::exit(1)
            }
        }
        Cmd::Summary { path } => {
            let p = iwr_core::analyze_path(&path)?;
            println!("project {} ({} files, {} modules, {} items, {} calls, {} type rels)", p.name, p.files.len(), p.modules.len(), p.items.len(), p.calls.len(), p.type_rels.len());
            for d in &p.diagnostics {
                println!("DIAG {}", d);
            }
            for m in &p.modules {
                println!("mod {} file={:?} items={}", m.path, m.file.map(|f| p.files[f].path.clone()), m.items.len());
            }
            for it in &p.items {
                println!("[{}] {:?} {}  @{}:{}  sig=`{}`", it.id, it.kind, it.path, p.files[it.span.file].path, it.span.line_start, it.signature);
                if let Some(f) = it.fn_info() {
                    println!("      owner={:?} trait={:?} result={} opt={} ?={} unwrap={} loops={} branches={} rec={} cfg={}",
                        f.owner_type, f.trait_id, f.returns_result, f.returns_option, f.question_marks, f.unwraps, f.loops, f.branches, f.is_recursive,
                        f.cfg.as_ref().map(|c| format!("{}n/{}e", c.nodes.len(), c.edges.len())).unwrap_or("-".into()));
                }
            }
            for c in &p.calls {
                println!("call {} -> {} ({:?}) to={:?} loop={} branch={} ?={} unwrap={} rec={}", p.items[c.from].path, c.callee, c.kind, c.to.map(|t| p.items[t].path.clone()), c.in_loop, c.in_branch, c.propagated, c.unwrapped, c.is_recursive);
            }
            for r in &p.type_rels {
                println!("rel {} -{:?}-> {} {:?}", p.items[r.from].name, r.kind, p.items[r.to].name, r.label);
            }
            println!("main={:?} external={:?}", p.main, p.external_crates);
            for mode in iwr_core::views::Mode::ALL {
                let g = iwr_core::views::build(&p, mode, &iwr_core::views::ViewOptions::default());
                println!("view {:?}: {} nodes, {} edges, {}x{} note={:?}", mode, g.nodes.len(), g.edges.len(), g.width as i64, g.height as i64, g.note);
            }
            fn dump(b: &iwr_core::model::Block, d: usize) {
                println!("{}[{:?}] {}  def={:?} use={:?} calls={:?} L{}-{}", "  ".repeat(d), b.kind, b.label, b.defines, b.uses, b.calls, b.span.line_start, b.span.line_end);
                for c in &b.children { dump(c, d + 1); }
            }
            for it in p.functions().take(12) {
                if let Some(b) = it.fn_info().and_then(|f| f.blocks.as_ref()) { println!("--- blocks of {}", it.path); dump(b, 0); }
            }
            let steps = iwr_core::guide::build(&p, None, &iwr_core::guide::GuideOptions::default());
            for (i, s) in steps.iter().enumerate() {
                println!("step {:>3} [{:?}] {} (stack {:?})", i, s.kind, s.title, s.stack);
            }
            Ok(())
        }
    }
}
