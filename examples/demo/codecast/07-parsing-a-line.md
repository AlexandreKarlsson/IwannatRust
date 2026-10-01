## Parsing a line
@ code:crate::parser::parse_all
The parser is three small functions. parse_all is the entry: it walks the lines, skips the empty ones, and collects a task per line. Nothing clever, which is what you want from a parser.
! crate::parser::parse_all/b2 crate::parser::parse_all/b3 crate::parser::parse_all/b4
Blank lines are skipped with a continue, so a trailing newline is not an error. Small mercies.
! crate::parser::parse_all/b5
Every other line goes through parse_line, and again the question mark means the first bad line stops everything. The parser does not collect errors; it reports the first one and goes home.
@ code:crate::parser::parse_line
! crate::parser::parse_line/b1
parse_line does the real work. It splits the line on colons and takes the pieces one at a time, with an iterator: each call to next gives the next piece, or None when the line runs out.
! crate::parser::parse_line/b2 crate::parser::parse_line/b3 crate::parser::parse_line/b4
= src/parser.rs:28:29-64
Each piece is an Option, because the line may be too short. ok_or turns a None into a MissingField error, and the question mark sends it up. Three fields, three chances to fail, three identical lines. Repetition is a feature here: you can see at a glance that all three are handled the same way.
? What is the difference between Option and Result?
  Option says "there might be nothing here": Some of a value, or None. Result says "this might have failed": Ok of a value, or Err of an error. ok_or is the bridge between them: it takes an Option and an error to use when it is None, and gives back a Result. The parser needs that bridge because a missing field is not merely nothing, it is a mistake with a line number.
! crate::parser::parse_line/b5
The priority text goes through parse_priority, a match on the three allowed words that returns BadPriority for anything else. A match must cover every case, so the catch-all arm at the bottom is not optional; the compiler would refuse the function without it.
! crate::parser::parse_line/b6
= src/parser.rs:32:43-95
And the step count is parsed as a number. map_err swaps the standard library's parse error for the parser's own BadNumber, carrying the offending text. The closure in the middle, the thing between vertical bars, is the swap: it ignores the original error and builds ours.
! crate::parser::parse_line/b9
Only when all of that succeeded does a Task get built. Five question marks in nine lines: this is what error propagation looks like in Rust. No try, no catch, no nesting, just a trail of little question marks saying "not my problem" all the way up to main. Next, the other half of the program: how tasks get picked.
