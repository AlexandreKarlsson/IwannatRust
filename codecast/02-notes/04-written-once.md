## Written once
@ code:iwr-core::script::lend
| md:Order
! written-once
All four parts of this topic showed the same note, and none of them wrote it. It is written once, in the index of the topic.
! iwr-core::script::lend
When a script has been read, lend goes through its parts and looks at what each one names.
! iwr-core::script::lend/b26
It reads the lines that show a block, in the view or in the side panel.
! iwr-core::script::lend/b11
= crates/iwr-core/src/script.rs:692:32-58
A block named by number is skipped: a number counts the blocks of the part itself.
sp: And a block named by title?
! iwr-core::script::lend/b2
Then lend looks in a pool: first the blocks written before any part, then every part in playing order.
! iwr-core::script::lend/b40
The first block with that title is copied into the part. From there on the part plays as if it had written it.
! iwr-core::script::lend/b32
It works the same way for notes, diagrams, byte layouts, charts, scenes and examples.
! the-trip
So the rule for an author is short. Write a block once, give it a title, and point into it from anywhere.
? Why a copy, and not one shared block?
  Because a part can change what it shows: move a box, reveal a row. With its own copy, each part starts from the block as written, and stepping back in one part never disturbs another.
? Where should a shared block go?
  If one part owns it, leave it there, and the others borrow it by title. If it belongs to no part in particular, put it before the first part: at the top of a single file, or in the index of a directory.
