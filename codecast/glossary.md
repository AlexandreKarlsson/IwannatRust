# IwannatRust glossary

// Project terms, offered as questions when a cue says them. Generic Rust words are in the built-in glossary.

## codecast
aliases: codecasts
ask: What is a codecast?
A codecast is a spoken tour of a codebase: a script of cues that the IwannatRust player performs on top of its views, switching diagrams, glowing the code being talked about, and offering questions. This tour is one.

## cue
aliases: cues
ask: What is a cue?
One cue is one spoken line of a codecast, with the directive lines written above it: which view to show, what to glow, what to underline. The player applies the directives, then speaks the line.

## brief
ask: What is the brief?
The brief is the compact text print of a project that `iwr brief` produces: modules, types, and every function with its signature and its body as block lines. A writer, human or model, reads it to write a codecast without opening the source.

## ref
aliases: refs
ask: What is a ref?
A ref is how a codecast names a place in the code: an item path like `crate::run`, a block like `crate::run/b5`, a module, a file, or a file with line and columns. The resolver turns it into a span, and every view glows whatever intersects that span.

## view
aliases: views
ask: What is a view?
A view is one way of drawing the project: Code, Call tree, Control flow, Architecture, Branch tree, Structure, Types, Error flow, and Diagram. Each one is a `Graph` built by `views::build`, except Code, drawn from the block trees, and Diagram, drawn from a codecast.

## CFG
aliases: control-flow graph, control flow graph
ask: What is a control-flow graph?
A graph of one function body: statements are nodes, and the edges are the paths execution can take: true and false of an if, each arm of a match, the loop back edge, early returns and error exits. The Control flow view draws it as it is.

## Sugiyama
ask: What is the Sugiyama method?
The classic recipe for drawing a directed graph in layers: break cycles, give each node a layer, add dummy nodes for edges that skip layers, order each layer to reduce crossings, then assign coordinates. The layout engine does exactly those steps.
