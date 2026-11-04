// The interface texts: `i18n/<lang>/*.toml` → typed functions in `$OUT_DIR/i18n.rs` (see iwr-i18n-gen).
fn main() {
    iwr_i18n_gen::build("i18n", iwr_i18n_gen::LangArg::Global("crate::i18n::lang()"));
}
