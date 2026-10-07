## The model
@ types:iwr-core::model
Everything the two passes produce lives in one module, model. This view shows its structs and enums and how they contain each other.
! iwr-core::model::Project
Project is the root: files, modules, items, calls, type relations, and the entry point. It is plain data with serde derives, so it travels as JSON to the browser.
! iwr-core::model::Item iwr-core::model::ItemExtra
An Item is any named thing: a function, a struct, a trait, an impl block. The kind-specific facts sit in ItemExtra, an enum with one variant per kind.
! iwr-core::model::FnInfo
FnInfo is the big one: parameters, return type, how many loops and branches and question marks, whether it is async, recursive, a test, plus the two pictures of the body.
! iwr-core::model::Cfg iwr-core::model::CfgNode iwr-core::model::CfgEdge
The first picture is the Cfg, a control-flow graph: statements as nodes; true, false, loop back and error exits as edges. The Control flow view draws it as it is.
! iwr-core::model::Block
The second is the Block tree: the body as nested blocks, each with what it defines, uses and calls. The Code view and the brief both read it.
? Why keep both a graph and a tree of the same body?
  They answer different questions. The tree keeps the nesting as written, which is what you want when reading code block by block. The graph keeps the paths execution can take, which is what you want to see where a function can exit. Both come cheaply from the same syn body, and each view gets the shape it needs.
! iwr-core::model::Span
Every one of them carries a Span: file, line and column, one-based. Spans are how a codecast ref becomes a glow on screen. Next, how the model becomes a picture.
