## Boxes inside boxes
@ flow:iwr-core::blocks::build
+ view
! iwr-core::blocks::build
^ iwr-core::blocks::build
The flow picture is a chain. The code view needs something else: nesting dolls. A loop holds its body, an if holds its branches. blocks build makes that tree.
! iwr-core::blocks::build/b2 iwr-core::blocks::build/b7
First it collects the names of the function's inputs. Those are toys the function already has in its hands when it starts.
! iwr-core::blocks::build/b9
The biggest doll is the function itself, block number zero. Every other block will sit somewhere inside it.
@ code:iwr-core::blocks::Builder::block
! iwr-core::blocks::Builder::block/b1 iwr-core::blocks::Builder::block/b4
Each time we open a pair of curly braces, we put a fresh toy box on a stack. Names made inside go in it. When the braces close, the box is thrown away.
? Why throw the toy box away?
  Because in Rust a variable made inside braces stops existing at the closing brace. Throwing the box away copies that rule, so a name used later is not wrongly linked to a variable that is already gone.
! iwr-core::blocks::Builder::stmt
= crates/iwr-core/src/blocks.rs:190:25-55
For every statement, a block remembers two lists: the names it makes, and the names it uses. It only counts a name as used if that name is in one of the toy boxes on the stack, so outside things are ignored.
@ code:iwr-core::blocks::Builder::new_block
! iwr-core::blocks::Builder::new_block/b1 iwr-core::blocks::Builder::new_block/b2
New blocks are numbered in reading order, top to bottom. These numbers are the b one, b two names this very tour uses to point at code.
@ flow:iwr-core::blocks::build
! iwr-core::blocks::build/b13 iwr-core::blocks::build/b14
Last, every block that starts its own line gets the sticky note from the comment above it, just like the flow bricks did. Next: when a block calls a function, which function is it?
