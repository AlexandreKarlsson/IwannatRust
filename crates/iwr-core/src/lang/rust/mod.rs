//! Rust sources, read with `syn`: modules from the file layout and inline `mod`s, items, then the
//! calls, flow graph and block tree of every function body.

pub mod blocks;
pub(crate) mod body;
pub mod cfg;
mod items;

pub(crate) use body::analyze;
pub(crate) use items::collect;
pub use items::module_path;
