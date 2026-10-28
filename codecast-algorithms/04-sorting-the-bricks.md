## Sorting the bricks
@ flow:iwr-core::cfg::Builder::plain
+ view
! iwr-core::cfg::Builder::plain/b8
^ iwr-core::cfg::Builder::plain
Most statements are plain ones, like "let x be five". plain decides what colour brick each one gets. It is like sorting toys into bins.
! iwr-core::cfg::Builder::plain/b4
First it asks a little helper to look inside the statement for special things: a question mark, an await, a return, an unwrap, a panic.
! iwr-core::cfg::Builder::plain/b9 iwr-core::cfg::Builder::plain/b11 iwr-core::cfg::Builder::plain/b13
Then a ladder of questions, from most important down. Does it return? Does it pass an error up with a question mark? Can it crash?
! iwr-core::cfg::Builder::plain/b15 iwr-core::cfg::Builder::plain/b17 iwr-core::cfg::Builder::plain/b19
Does it call one of our functions? Does it wait? Does it read or write the outside world, like printing?
! iwr-core::cfg::Builder::plain/b21 iwr-core::cfg::Builder::plain/b23 iwr-core::cfg::Builder::plain/b25
Is it a new variable? Does it change something, like pushing into a list? If nothing matches, it is a plain grey brick.
? Why does the order of the ladder matter?
  Because one line can be many things at once. A line that calls a function and ends with a question mark could fail, and failing is the bigger news, so it is checked first and wins.
! iwr-core::cfg::Builder::plain/b28 iwr-core::cfg::Builder::plain/b32
Now the clever part: gluing. Five small "let" lines in a row would be five boring bricks. If the new step has the same colour as the brick just before, it is glued onto that brick instead.
= crates/iwr-core/src/cfg.rs:316:58-95
! iwr-core::cfg::Builder::plain/b31
But only inside the same paragraph. A blank line or a new comment starts a new brick, because the programmer meant a new idea.
! iwr-core::cfg::Builder::plain/b34 iwr-core::cfg::Builder::plain/b39
= crates/iwr-core/src/cfg.rs:320:89-104
And a brick cannot grow forever: eight lines for variables and changes, four for the rest. Then a new brick starts.
! iwr-core::cfg::Builder::plain/b50 iwr-core::cfg::Builder::plain/b52 iwr-core::cfg::Builder::plain/b55
Some bricks get a second string, straight to end: a red one for errors and panics, a green one for return. A return also drops its loose string, since nothing runs after it. Next, the strings that split.
