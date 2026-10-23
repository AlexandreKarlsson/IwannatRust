## The views
@ code
+ views
At the top left, next to Code, the views button opens the drawings. Each one is built from the code, and follows it when the code changes.
@ arch
+ view
In the middle, the architecture: one box per module, and an arrow where one module uses another. Drag to move, scroll to zoom, and press F to fit everything back.
@ calls:{fn}
Still in the middle, the call tree: who calls whom, from the entry point down. Double-click a box to open it, or to fold it.
@ flow:{fn}
! {fn_b1}
+ zoom
The control flow of one function, step by step: its branches, its loops, its early returns. Here the codecast zooms in on the step it is talking about.
!
+ view
@ types
Types and traits: what holds what, and who implements what.
@ errors
The error flow: which functions return errors, where a question mark passes them up, and where they are handled.
+ source
On the right, the details of what is selected: its signature, its documentation, what it calls, who calls it, and its source.
+
