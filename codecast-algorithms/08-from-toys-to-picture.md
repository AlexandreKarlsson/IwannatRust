## From toys to picture
@ flow:iwr-core::views::cfg_into
+ view
! iwr-core::views::cfg_into
^ iwr-core::views::cfg_into
The bricks from the builder are not drawn as they are. cfg into turns them into picture boxes, and it hides some to keep things calm.
! iwr-core::views::cfg_into/b4 iwr-core::views::cfg_into/b9
Loops start folded, like a closed box of toys. Every brick inside a folded loop is told: you are represented by that loop brick.
! iwr-core::views::cfg_into/b13 iwr-core::views::cfg_into/b16
Then each brick that stands for itself becomes a picture box. The bricks hidden in a loop are skipped.
! iwr-core::views::cfg_into/b45 iwr-core::views::cfg_into/b49
A folded loop says what it hides: how many steps are inside, and which functions they call. Its badge shows the count with a little circle arrow.
! iwr-core::views::cfg_into/b59 iwr-core::views::cfg_into/b68
A call can open in place, like opening a door to another room. The callee's own bricks are added right there, with names that start with the call's name, so they never clash.
! iwr-core::views::cfg_into/b59
But only three rooms deep, and never into a function that is already open above. Otherwise recursion would open doors forever.
! iwr-core::views::cfg_into/b80 iwr-core::views::cfg_into/b82
Strings are copied last. A string whose two ends are inside the same folded loop vanishes with it, and duplicates are dropped.
@ code:iwr-core::views::node_size
= crates/iwr-core/src/views.rs:451:13-50
Every box needs a size before it can be placed: as wide as its text plus a little padding, and one row taller per line. Now we have boxes with sizes. Let's put them on the shelf.
