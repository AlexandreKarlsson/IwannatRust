# Bundled fonts

Converted to woff2 from the font folder of Club-App (`client/assets/fonts/`, same author).
The catalogue (ids, names, kinds) is `iwr_core::config::FONTS`; `src/fonts.rs` lists the files.
Most are display fonts from free font sites: check each font's licence before redistributing a
build that includes them.

To add one: convert it to `<id>.woff2` here, add a `Font` row to `FONTS` and an `asset!` line
to `FILES` in `src/fonts.rs`.
