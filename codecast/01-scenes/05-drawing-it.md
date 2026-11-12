## Drawing it
```scene Borders
a 1,1 "a"
b diamond 3,2 "b"
a --> b
```
@ code:iwr-core::scene::Scene::rect
| scene:Borders
! iwr-core::scene::Scene::rect
From squares to pixels there is one small function. rect multiplies a column and a row by the size of a square, and keeps a margin so neighbours never touch.
@ code:iwr-core::scene::Scene::segment
! iwr-core::scene::Scene::segment/b19 a b
A line is harder. It aims from one centre to the other, but it must stop at the borders, or the arrow head would hide under the box.
! iwr-core::scene::Scene::segment/b21 l1
So for each end we ask how far along the way its border is. A box and a diamond answer differently, which is why the arrow on the right touches the diamond on its slanted side.
! iwr-core::scene::Scene::segment/b30
When the two answers meet, the things overlap and there is no line to draw.
sp: And the gliding, who computes all the steps in between?
@ code:iwr-ui::scene::SceneView
! iwr-ui::scene::SceneView/b9
Nobody. The view asks for the state at this point of the sentence, and draws that, nothing more.
! iwr-ui::scene::SceneView/b55 iwr-ui::scene::SceneView/b74
Each thing keeps its name as a key, so the browser sees the same element at a new place and glides it there. The lines follow, since their ends are computed from where the things are now.
! iwr-ui::scene::SceneView
That is a scene: a few lines of text, a fold over markers, and a drawing with no memory. Try one the next time a place tells the story.
? How does a greyed out emoji work?
  An emoji has its own colours, so changing the text colour does nothing. The off state uses a filter of the browser that removes the colours of the whole thing and makes it paler, which works on a box and on an emoji alike.
