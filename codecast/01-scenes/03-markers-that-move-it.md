## Markers that move it
```scene Fold
start text 1,1 "as written"
m1 pill 2,1 "move"
m2 pill 3,1 "state"
now round 4,1 "drawn"
tok icon 1,2 "🟡"
start --> m1 --> m2 --> now
```
@ code:iwr-core::chart::split_say
| scene:Fold
! iwr-core::chart::split_say/b13 start
A sentence of a cue can hold braces. split say takes them out before anything is spoken, so the voice, the caption and the translation only see a plain sentence.
! iwr-core::chart::split_say/b18
It remembers where each one sat, as a number of characters into the text. That is the moment the marker fires: when the voice gets there.
sp: Charts already had those braces, right?
@ code:iwr-core::chart::parse_marks
! iwr-core::chart::parse_marks/b52 iwr-core::chart::parse_marks/b44 iwr-core::chart::parse_marks/b47
Right, and scenes speak the same language with three more words: move a thing, give it a state, give it a label.
@ code:iwr-core::scene::Scene::state
! iwr-core::scene::Scene::state/b1
Nothing is ever changed in place. state starts again from the stage as written, every time.
! iwr-core::scene::Scene::state/b14 m1
Then it replays each marker fired so far, in order. Watch the token on the right: a move {>tok 2,2} puts it on a new square.
! iwr-core::scene::Scene::state/b25 m2
A state marker {>tok 3,2 =tok off} greys it, or colours it.
! iwr-core::scene::Scene::state/b44 now
And a marker about something else, a row of a chart for instance, is skipped. What comes out {>tok 4,2 =tok on} is what gets drawn.
sp: So stepping back to an earlier sentence just replays fewer markers?
Yes. No undo to write, and jumping anywhere in the part gives the right picture. Next, what happens when the block itself is wrong.
? Why does a marker sit inside the sentence and not on its own line?
  Because its place is its timing. A line above a cue can only act when the cue starts. A marker in the middle of the sentence acts on the word that carries the news, so the eye and the ear get it together.
