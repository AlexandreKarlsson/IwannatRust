use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod serve;

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
        /// Narration script (text format, see docs/narration.md) to load in the player
        #[arg(long)]
        script: Option<PathBuf>,
        /// Recorded narration audio (mp3/ogg/wav) matching the script's [t] cues
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
    },
    /// Print a compact text brief of the project for writing a narration script (LLM-friendly)
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
    },
    /// Emit the built-in walkthrough as a narration script (text format)
    Guide {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Start from this function instead of main
        #[arg(long = "fn")]
        only: Option<String>,
    },
    /// Validate a narration script's refs against the project
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
        Cmd::Export { path, out, script, audio } => serve::export(path, out, script, audio),
        Cmd::Brief { path, only, no_body, no_overview } => {
            let p = iwr_core::analyze_path(&path)?;
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
        Cmd::Check { path, script } => {
            let p = iwr_core::analyze_path(&path)?;
            let s = iwr_core::script::parse(&std::fs::read_to_string(&script)?);
            let bad = iwr_core::script::check(&p, &s);
            println!("{}: {} part(s), {} cue(s)", s.title, s.parts.len(), s.cue_count());
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
