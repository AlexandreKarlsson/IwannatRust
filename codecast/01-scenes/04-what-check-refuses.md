## What check refuses
```text Messages
`disk` at 7,2 (2x1) is outside the grid 6x3
`a` and `b` overlap: move one, make one hidden
`a`: the label "a very long name" does not fit
line `l1` runs through `b`: move one of them
```
@ code:iwr-core::scene::Scene::verify
| text:Messages
! iwr-core::scene::Scene::verify
A model writes most scenes, and it cannot see the result. So verify looks for everything that would draw badly, and codecast check prints it before anyone presses play.
! iwr-core::scene::Scene::verify/b11
First, every thing must stand on the stage, with its whole width and height.
! iwr-core::scene::Scene::verify/b27
Its label must fit its box. The width is estimated from the letters, with emoji counting double, and the message says how to fix it: shorten, break the line, or widen the box.
sp: What if I really want one thing on top of another?
! iwr-core::scene::Scene::verify/b29
Two things may not start on the same squares, with two ways out: one of them is hidden and waits its turn, or the one behind is a zone, an area drawn under the others.
! iwr-core::scene::Scene::verify/b34
A line must end on a thing that exists, or on a square of the grid.
! iwr-core::scene::Scene::verify/b43
And a line may not run straight through a third thing, because it would read as two lines. The message suggests bending it through free squares.
The markers are checked too, by the script checker: a move off the stage, a name nobody has. Next, the last step: pixels.
? Why are these errors and not warnings?
  Because nobody reads warnings, least of all a model in a hurry. Each message names the thing, says what is wrong and how to fix it, so one more pass is usually enough to get a clean scene.
