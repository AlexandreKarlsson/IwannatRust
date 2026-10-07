## Views and layout
@ flow:iwr-core::views::build
build is the only door into the views module. The UI calls it with a mode and options, and gets a Graph with positioned nodes back.
! iwr-core::views::build/b2
One match, one builder per mode: call tree, control flow, architecture, branch tree, structure, types, error flow.
! iwr-core::views::build/b3 iwr-core::views::build/b4
Code is the exception: the Code view is drawn by the UI from the block trees, so it gets an empty graph.
! iwr-core::views::build/b19 iwr-core::views::build/b20
And Diagram is the other exception: a diagram does not come from the project at all, it comes from the codecast. We get to that in two parts.
@ flow:iwr-core::layout::layered
Every builder ends in the layout engine. layered takes boxes with sizes and edges, and returns coordinates and edge routes. No external crate, so it runs in wasm too.
! iwr-core::layout::layered/b9 iwr-core::layout::layered/b14
First, connected components. Nodes that nothing links to should not wander into the middle of a graph that is about something else.
! iwr-core::layout::layered/b25 iwr-core::layout::layered/b27 iwr-core::layout::layered/b39
With more than one component, each is laid out alone, biggest first, and the results are packed into rows so a wide graph stays roughly square.
! iwr-core::layout::layered/b69
With one component, the real work is layered_component: the classic Sugiyama recipe, in five steps.
@ code:iwr-core::layout::layered_component
! crates/iwr-core/src/layout.rs:168-185
Reading it as code. Step one, break cycles: a depth-first search finds back edges and reverses them, so the graph becomes a DAG.
! crates/iwr-core/src/layout.rs:199-216
Step two, longest-path layering: every node goes one layer below its deepest predecessor.
! crates/iwr-core/src/layout.rs:242-279
Step three, virtual nodes: an edge that skips layers is split, so the ordering step can route it around real boxes.
! crates/iwr-core/src/layout.rs:280-305
Step four, barycenter sweeps: each layer is sorted by the average position of its neighbours, downwards then upwards, a few times, to reduce crossings.
! crates/iwr-core/src/layout.rs:306-369
Step five, coordinates: positions along the cross axis with a relaxation pass, then layers along the main axis. Horizontal mode just swaps the axes.
? Why not use graphviz?
  Because the layout has to run in the browser, in WebAssembly, with no system binaries and no JavaScript dependency. A few hundred lines of Rust do the job for graphs of this size, and the same code gives the command line and the UI identical pictures.
