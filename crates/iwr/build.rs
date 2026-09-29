// Make sure the UI dist folder exists so `rust-embed` compiles even before the
// web UI has been built (the server then reports that the UI is missing).
fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/dx/iwr-ui/release/web/public");
    let _ = std::fs::create_dir_all(&dir);
    println!("cargo:rerun-if-changed={}", dir.display());
}
