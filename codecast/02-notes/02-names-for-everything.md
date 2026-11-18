## Names for everything
@ code:iwr-core::markdown::slug
| md:Order
! iwr-core::markdown::slug
A name starts in slug. It takes a title and keeps what an author can type in a script.
! iwr-core::markdown::slug/b3 iwr-core::markdown::slug/b4
= crates/iwr-core/src/markdown.rs:257:12-31
Letters and digits stay, in lower case. Japanese letters count as letters, so a Japanese title has a name too.
! iwr-core::markdown::slug/b5 iwr-core::markdown::slug/b6
Anything else becomes one dash, never two in a row.
@ code:iwr-core::markdown::collect
! iwr-core::markdown::collect
Then collect walks the page from top to bottom, and gives each kind of block its name.
! iwr-core::markdown::collect/b7 the-trip
A heading gives its slug, and it opens a section.
! iwr-core::markdown::collect/b9 the-trip/1
A list asks for the name of its section, then counts its items.
sp: What if a section has two lists?
! iwr-core::markdown::collect/b10
= crates/iwr-core/src/markdown.rs:313:27-39
Then only the first one gets names. Two steps answering to the same number would be worse than one list without names.
! iwr-core::markdown::collect/b15 len
A table names each row by its first cell, then each column by its header.
! iwr-core::markdown::collect/b22 m1
A drawing inside the page brings the names it already had: its boxes and its messages.
! iwr-core::markdown::collect/b31 exchange
And a tab gives its own name before the names of what it holds.
! iwr-core::markdown::collect
The page on screen asks the same code for its names. So a name that check accepts is a name that glows.
? Why not let the author choose the names?
  Because a note should stay plain markdown, readable anywhere. Names taken from the words need no extra syntax, and the author already knows them: they are what the listener reads.
? What happens to the names when the note is translated?
  The words on screen change, the names in the script do not. The translation keeps a small table from each new name to the one that was written, so the same line glows in every language.
Next, the tabs: how one line of text becomes something that opens.
