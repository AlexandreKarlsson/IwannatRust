## Reading code aloud
@ flow:iwr-core::speech::split_identifier
+ view
! iwr-core::speech::split_identifier
^ iwr-core::speech::split_identifier
A robot voice reading code says silly things, like "parse underscore all". split identifier fixes that by cutting names into words.
! iwr-core::speech::split_identifier/b3
It reads the name one letter at a time, like sounding out a word.
! iwr-core::speech::split_identifier/b4 iwr-core::speech::split_identifier/b6
An underscore becomes a space. So parse all is read as two words.
! iwr-core::speech::split_identifier/b8 iwr-core::speech::split_identifier/b13
A capital letter after a small one also starts a new word: Memory Storage. And a run of capitals keeps together until a small letter follows, so HTML Parser stays readable.
? What about short names like u8 or fn?
  Those are looked up first, in a small word list: fn becomes function, u8 becomes u 8, and a codecast can add its own with a pronounce line. Only names not in the list are split letter by letter.
@ code:iwr-core::speech::spoken
! iwr-core::speech::spoken
spoken puts the sentence back together: plain words stay, code is swapped for its spoken form, and a small breath is added after a long expression.
@ arch
+ view
! iwr-core::cfg iwr-core::blocks iwr-core::resolve iwr-core::views iwr-core::layout iwr-core::diagram iwr-core::speech
That's the whole toy box. Bricks are born, tied with strings, nested like dolls, found in a phone book, tidied on a shelf, and read aloud. Thanks for playing!
