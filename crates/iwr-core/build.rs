// The texts core says to people (view names, notes, generated tours): `i18n/<lang>/*.toml` →
// typed functions taking the language, in `$OUT_DIR/i18n.rs` (see iwr-i18n-gen).
fn main() {
    iwr_i18n_gen::build("i18n", iwr_i18n_gen::LangArg::Param);
}
