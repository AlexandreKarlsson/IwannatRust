## A box is born
@ code:iwr-core::cfg::Builder::add
! iwr-core::cfg::Builder::add
^ iwr-core::cfg::Builder::add
Every box in a control flow picture is made by one small function, add. Think of it as the toy factory: one machine, one new brick each time.
! iwr-core::cfg::Builder::add/b1
= crates/iwr-core/src/cfg.rs:169:18-37
First, the brick gets a number. The number is simply how many bricks we already have. The first brick is zero, the next is one, and so on.
? Why a number and not a name?
  Because numbers are cheap and never clash. Strings between boxes only need to say "from brick three to brick seven". The name, the label, is kept on the brick for people to read.
! iwr-core::cfg::Builder::add/b2 iwr-core::cfg::Builder::add/b8
Next, we look inside the brick for calls to the project's own functions. If the step calls parse all, the brick remembers it, but only once, even if it is called twice.
! iwr-core::cfg::Builder::add/b11
= crates/iwr-core/src/cfg.rs:182:20-43
Then a sticky note. If the programmer wrote a short comment just above this line, the brick takes it. Take means the note is used up, so the next brick does not get it too.
! iwr-core::cfg::Builder::add/b12
The brick also remembers which loop it is inside, if any. Later, that lets us fold a whole loop into one brick.
! iwr-core::cfg::Builder::add/b15 iwr-core::cfg::Builder::add/b16
Finally the brick goes on the pile, and add hands back its number. That is the whole birth of a node: a number, a kind, a label, a sticky note, and the calls inside.
@ flow:iwr-core::cfg::build
! iwr-core::cfg::build/b3 iwr-core::cfg::build/b4
The very first two bricks are special: start and end. Every function gets them before anything else. So now we have bricks, but nothing ties them together. That is the next part.
