## Tabs that open themselves
@ code:iwr-core::markdown::read
| md:Order
! iwr-core::markdown::read
Tabs are not standard markdown. So read looks for them itself, line by line, before the real parser sees the text.
! iwr-core::markdown::read/b7 on-the-wire
= crates/iwr-core/src/markdown.rs:150:45-59
A tab starts with three equal signs and a name in quotes.
! iwr-core::markdown::read/b25
Its body is every indented line under it, and that body is read like a small page of its own.
sp: How do two tabs end up in the same set?
! iwr-core::markdown::read/b21 iwr-core::markdown::read/b36 exchange
A tab that comes right after another one joins it. With text in between, a new set starts.
@ code:iwr-ui::md::TabSet
! iwr-ui::md::TabSet
On screen, a set of tabs is one component. It remembers which tab the viewer clicked.
! iwr-ui::md::TabSet/b7
For each tab it lists the names inside, and asks if the voice points at one of them.
! iwr-ui::md::TabSet/b14 iwr-ui::md::TabSet/b17 m2
= crates/iwr-ui/src/md.rs:111:18-45
The first tab that says yes is opened. That is why naming an arrow was enough to bring its tab forward.
? Why is the drawing in the tab so plain?
  Because it is a small still picture, made to sit beside the code. For a drawing that moves, or that fills the view, show the diagram itself in the view.
Next, the last question: where does this note live?
