## The phone book
@ flow:iwr-core::resolve::build_index
! iwr-core::resolve::build_index/b22
^ iwr-core::resolve::build_index
When code says "call save", which save is it? There might be three. IwannatRust has no type checker, so it uses a phone book and some good guesses.
! iwr-core::resolve::build_index/b24 iwr-core::resolve::build_index/b26 iwr-core::resolve::build_index/b28 iwr-core::resolve::build_index/b30
The phone book has a page for types, a page for free functions and a page for methods. Each name lists everyone who has it.
! iwr-core::resolve::build_index/b15 iwr-core::resolve::build_index/b20
In a workspace with many crates, it also writes down which crate mentions which. A crate can only call friends it knows by name.
@ flow:iwr-core::resolve::resolve_method
! iwr-core::resolve::resolve_method/b1
Now a method call, like tasks dot save. First, look up save in the phone book, and keep only the ones we are allowed to reach.
! iwr-core::resolve::resolve_method/b5 iwr-core::resolve::resolve_method/b8
Prefer the ones that have a real body, not just a promise in a trait.
! iwr-core::resolve::resolve_method/b13 iwr-core::resolve::resolve_method/b16
If the call is on self, the answer is easy: the method of our own type wins.
! iwr-core::resolve::resolve_method/b28 iwr-core::resolve::resolve_method/b29
Only one left? Done. Otherwise, look for clues: if the function mentions a type by name, a method of that type is a good bet.
! iwr-core::resolve::resolve_method/b41 iwr-core::resolve::resolve_method/b44
If every candidate implements the same trait, the honest answer is the trait itself, since we cannot know which one runs.
@ flow:iwr-core::resolve::prefer_module
! iwr-core::resolve::prefer_module/b7 iwr-core::resolve::prefer_module/b8
And the last tie breaker is prefer module: the nearest neighbour wins. Same module first, like asking your own family before the neighbours.
! iwr-core::resolve::prefer_module/b11 iwr-core::resolve::prefer_module/b13 iwr-core::resolve::prefer_module/b17
Otherwise, count how much of the module path two places share, and keep the one with the longest shared start. Now all the pieces are linked. Time to draw them.
? Is the guess ever wrong?
  Sometimes, yes. Two types with a method of the same name in the same module can fool it. That is the price of not running the compiler, and in return the analysis is very fast and works on any code that parses.
