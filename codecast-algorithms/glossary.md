# Algorithm glossary

// Words this tour keeps using. Offered as questions when a cue says them.

## open end
aliases: open ends, loose string, loose strings
ask: What is an open end?
An open end is a box whose arrow to the next step is not tied yet. The control flow builder keeps a list of them; the next box it creates ties all of them to itself, then becomes the only open end.

## node
aliases: nodes
ask: What is a node?
A node is one box in a graph: a step of a function, a function in the call tree, a module in the architecture. It has a number, a label, a kind that picks its colour, and the place in the source it came from.

## edge
aliases: edges
ask: What is an edge?
An edge is an arrow between two nodes. In a control flow graph it says what happens next, and its kind says why: true, false, a match arm, the next loop turn, a break, or an error leaving the function.

## layer
aliases: layers, rank, ranks
ask: What is a layer?
A layer is one floor of the picture. The layout puts every box on a floor one below the deepest box pointing at it, so arrows mostly go down.

## dummy node
aliases: dummy nodes, virtual node, virtual nodes
ask: What is a dummy node?
An invisible stepping stone. When an arrow jumps over several floors, the layout puts a tiny invisible box on every floor in between, so the arrow can be steered around real boxes like any other.
