## A scene explains itself
```scene The stage
grid: 6x3 lines
actor icon 1,2 "🧍"
door 3,2 "door" :::off
lamp icon 5,1 "💡" :::off
way round 6,2 "exit" :::hidden
actor --> door
```
@ scene
! actor
A diagram decides by itself where its boxes go. Sometimes the place is the story: a slot in a buffer, a machine on a network. So a part can set a stage, a grid, and put things on its squares.
! actor door
Every thing has a name, a square and a shape. Our actor stands in the first column, and the door waits two squares away, greyed out because it is closed.
sp: And they just stand there while you talk?
!
No. A marker inside the sentence moves them: the actor walks {>actor 2,2} one square, right as I say it.
States change the same way: the lamp {=lamp on !lamp} turns on, and the door {=door ok !door} opens.
Things can also wait hidden. Here comes {+way !way} the exit, while the door {=door "open"} gets a new label.
sp: So the whole story is written once, and the sentences reveal it?
Exactly. And for the last step the actor {-l1 >actor way "bye"@way} leaves, with a note over the exit.
? Why a grid and not exact pixels?
  Because squares can be checked and pixels cannot. With squares, the tool knows when two things collide, when one falls off the stage, and when a label is too long for its box. The author thinks in columns and rows, and the player turns them into pixels for any window size.
? When should I draw a diagram instead?
  When only the links matter. A diagram lays the boxes out by itself, so nobody has to choose a place for them. Take a scene when where a thing sits means something, or when things have to move from place to place.
Next, let's see how these few lines of text become a stage.
