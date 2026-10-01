## Where errors go
@ errors
This is the error flow view. It shows every function that can fail, every error type, and how errors travel between them. Think of it as the plumbing diagram: where the water goes when something leaks.
! crate::parser::ParseError crate::storage::StorageError
The parser and the storage each have their own error enum, close to where the problems happen. Each variant carries what you would want to know: the line number of a missing field, the text of a bad priority, the name of a duplicate task.
! crate::AppError
All of them funnel into AppError, the type run returns to main. It has one variant per source of trouble, plus Empty for the case where nothing was wrong with the input except that there was none.
! crate::AppError::from
The two From implementations are what make the funnel work. The question mark does not just return the error; on the way out it calls From to convert it into the type the function returns. So a ParseError becomes AppError::Parse without a single match in run. Two tiny functions, and the conversions happen everywhere, silently.
? Why not one big error enum for everything?
  @ code:src/main.rs
  ! src/main.rs:16-32
  Because then the parser would have to know about storage errors and the storage about parse errors, and a module that knows about everyone else is a module you can never move. Each module owns its own error type, and the top level wraps them. From is the glue, and it costs two three-line impls. The design scales: add a network module, add a NetworkError, add one more From.
! crate::main
main is the only place that looks at an error: it prints it and exits with code one. Everything below it just passes errors up. That is the Rust pattern: handle errors where you can do something about them, and here only main can, by giving up with dignity.
!
That is the whole error story: three enums, two conversions, one handler, and a lot of question marks in between. To finish, let's see where all of this lives in the files.
