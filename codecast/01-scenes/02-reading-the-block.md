## Reading the block
```text A scene block
grid: 6x4 lines
actor icon 1,2 "🧍"
door 3,2 "door" :::off
way round 6,2 "exit" :::hidden
garden 5,3 2x2 ![Garden](img/garden-day.svg) :::hidden
actor --> door
```
@ code:iwr-core::scene::parse
| text:A scene block
! iwr-core::scene::parse
The stage we just watched is this block of text in the part file. parse reads it line by line, and like every parser of the tool it never fails: what it cannot read becomes an error for later.
! iwr-core::scene::parse/b11
Each line is one of three things. A line that starts with the word grid gives the size of the stage, and two options: draw the squares, or make them square.
! iwr-core::scene::parse/b24
A line with an arrow in it joins two things. The arrow is looked for outside quotes, so a label may contain one.
sp: And everything else is an object?
! iwr-core::scene::parse/b27
Yes. First a picture is lifted out of the line, the exclamation mark and brackets that markdown uses, so a dash in a file name is never read as an arrow.
! iwr-core::scene::parse/b28 iwr-core::scene::parse/b29
Then its name comes first, and it starts as a plain box, one square wide and one high.
! iwr-core::scene::parse/b34
Then its other words can come in any order, because each kind looks different: three colons for a state, quotes for a label, a comma for a square, a letter x for a size.
! iwr-core::scene::parse/b57
Only the square is required: a thing with no place cannot be on a stage.
! iwr-core::scene::parse/b60
A thing with a picture becomes an image, with no outline, unless a shape was given; its caption is the text in the brackets.
! iwr-core::scene::parse/b79
With no grid line, the stage is just as big as what stands on it.
! iwr-core::scene::parse/b98
The last step is verify, which looks at the whole stage at once. We will come back to it. First, the markers.
? Why can the words of an object come in any order?
  Because an author, human or model, should not have to remember a column order. Each word says what it is by its look, so the parser sorts them out, and a word it does not know is reported with the list of what it accepts.
