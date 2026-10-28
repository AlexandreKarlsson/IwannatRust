## Groups in groups
@ flow:iwr-core::diagram::Ctx::place
+ view
! iwr-core::diagram::Ctx::place
^ iwr-core::diagram::Ctx::place
Diagrams like the ones in this tour can have groups inside groups, like boxes in a moving van. place lays them out with a simple trick: inside first.
! iwr-core::diagram::Ctx::place/b5 iwr-core::diagram::Ctx::place/b8
For each child that is itself a group, place calls itself on that group first. That is recursion: the function asks itself to solve the smaller box.
! iwr-core::diagram::Ctx::place/b10
Once the inside is done, its width and height, plus a margin and room for the title, become the size of one big box.
! iwr-core::diagram::Ctx::place/b18 iwr-core::diagram::Ctx::place/b24
Arrows are lifted to this level. If an arrow goes to a toy inside a group, at this level it goes to the group itself.
! iwr-core::diagram::Ctx::place/b31
Then the very same shelf tidier from the last part lays out this level. One engine, used again and again, at every depth.
! iwr-core::diagram::Ctx::place/b32 iwr-core::diagram::Ctx::place/b34
Each child remembers where it sits inside its parent. Adding the parents' positions later gives every box its place on the page. One job left: saying it all out loud.
