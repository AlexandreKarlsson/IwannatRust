//! `iwr`, the CLI's former name: says so and runs `codecast` (next to it, else on `PATH`) with the same arguments.

fn main() {
    eprintln!("note: `iwr` is now `codecast`; this alias will go away");
    let here = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(format!("codecast{}", std::env::consts::EXE_SUFFIX))));
    let exe = here.filter(|p| p.is_file()).unwrap_or_else(|| "codecast".into());
    let status = std::process::Command::new(exe).args(std::env::args_os().skip(1)).status();
    std::process::exit(match status {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("cannot run codecast: {}", e);
            127
        }
    });
}
