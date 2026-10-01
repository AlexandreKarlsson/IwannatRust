# Built-in Rust glossary

// Generic Rust vocabulary, offered as questions whenever a codecast cue says the term.
// One entry per `##` heading. `aliases:` and `ask:` are optional. Everything else is the answer.
// Answers are spoken by the browser, so: short sentences, no puns that need spelling.
// Project-specific terms belong in the project's own `codecast/glossary.md`, not here.

## crate
aliases: crates
ask: What is a crate?
A crate is Rust's unit of compilation: one library or one program, built as a whole. Your project is a crate; the packages you download from crates.io are crates too. In paths, `crate::` means "start at the root of this crate", so `crate::run` is the function `run` at the top of this one. Think of it as the box everything else ships in.

## module
aliases: modules, mod
ask: What is a module?
A module is a named box inside a crate: a file, or a `mod name { }` block, that groups functions and types and controls what is visible outside. `crate::parser::parse_all` reads "the function parse_all, in the module parser, in this crate". Modules are how a Rust program is split into rooms; `pub` is the door.

## impl
aliases: impl block, impl blocks
ask: What is an impl?
`impl` is short for implementation. An `impl Task { }` block is where the methods of the type `Task` live; the data is declared in the `struct`, the behaviour in the `impl`. `impl Describe for Task` is the second form: it implements a trait for a type, which is Rust's way of saying "Task can do what Describe promises".

## trait
aliases: traits
ask: What is a trait?
A trait is a contract: a list of methods a type promises to provide, like an interface in other languages. `trait Store { fn save(...); fn all(...); }` says "anything that calls itself a Store can save and list". Code can then be written against the trait, and any type that implements it can be swapped in. That is how the demo's in-memory storage could be replaced by a database.

## struct
aliases: structs
ask: What is a struct?
A struct is a type with named fields: `struct Task { name: String, priority: Priority }`. It is the Rust equivalent of a record or a plain object, without inheritance. The data lives in the struct; the methods live in `impl` blocks next to it.

## enum
aliases: enums, variant, variants
ask: What is an enum?
An enum is a type with a fixed list of alternatives, called variants: `enum Priority { Low, Medium, High }`. Unlike most languages, each variant can carry data, so `enum AppError { Empty, BadNumber(String) }` is one error type with payloads. A `match` on an enum must handle every variant, or the compiler complains, which is the point.

## match
aliases: pattern matching, match arms, match arm
ask: What does match do?
`match` compares a value against a list of patterns and runs the first arm that fits, like a switch that also destructures. It is exhaustive: forget a variant of an enum and the code does not compile. Combined with `Option` and `Result`, it is how Rust forces you to think about the "nothing" and "it failed" cases.

## Option
aliases: options, Some, None
ask: What is an Option?
`Option` is a type with two variants: `Some(value)` or `None`. Rust has no null; where another language would hand you a possibly-null pointer, Rust hands you an Option and makes you open it. `ok_or` turns a None into an error, `unwrap` opens it and panics if empty, and `match` or `if let` open it politely.

## Result
aliases: results, Ok, Err
ask: What is a Result?
`Result` is a type with two variants: `Ok(value)` when a function succeeded, `Err(error)` when it did not. Rust has no exceptions; a function that can fail says so in its return type, and the caller has to look inside. The question mark operator is the short way of doing that.

## question mark
aliases: question mark operator, try operator
ask: What does the question mark do?
A `?` after a call that returns a `Result` means: if it is `Ok`, unwrap it and carry on; if it is `Err`, return that error from this function right now. One character replaces a whole "if error then return error" block. It also works on `Option`, returning `None` early. The function around it must itself return a `Result` or an `Option` of the same flavour.

## ownership
aliases: owner, owns, owned
ask: What is ownership?
Every value in Rust has exactly one owner, a variable; when the owner goes out of scope, the value is freed. Passing a value to a function or assigning it moves ownership, and the old name can no longer be used. No garbage collector, no manual free: the compiler tracks who holds what and inserts the cleanup for you. The checker that enforces it is the bouncer at the door of every function.

## borrow
aliases: borrows, borrowed, borrowing, reference, references, ampersand
ask: What is a borrow?
A borrow is a reference to a value you do not own, written with an ampersand: `&tasks` lends the list without giving it away, `&mut tasks` lends it with permission to change. The rules: any number of readers, or exactly one writer, never both at once. Most of the fighting with the Rust compiler is about these two rules, and most of the bugs it prevents are too.

## clone
aliases: clones, cloned, cloning
ask: What does clone do?
`clone()` makes a deep copy of a value, so the copy has its own owner. You use it when you need the same data in two places and a borrow will not do, for example to put a task in storage while still looping over the original list. It costs an allocation, which is why Rust makes you write it out instead of copying silently.

## derive
aliases: derives, derived, derive macro
ask: What does derive mean?
`#[derive(Debug, Clone, PartialEq)]` above a type asks the compiler to write standard trait implementations for it: how to print it for debugging, how to copy it, how to compare it. It is code generation you never see, driven by the attribute on the line above the type.

## lifetime
aliases: lifetimes
ask: What is a lifetime?
A lifetime is a name for how long a reference is valid, written like `'a`. The compiler uses lifetimes to make sure a borrow never outlives the value it points to, which is how Rust prevents dangling pointers without a garbage collector. Most of the time they are inferred and you never write one.

## closure
aliases: closures, lambda
ask: What is a closure?
A closure is an anonymous function written inline, with the arguments between vertical bars: `|t| t.weight()`. It can capture variables from the surrounding scope. Iterator chains like `.map(|t| ...)` and `.filter(|t| ...)` are where you meet them most.

## iterator
aliases: iterators, iter, into_iter, iter_mut
ask: What is an iterator?
An iterator is anything that can hand out items one at a time through a `next()` method. `for t in &tasks` uses one under the hood; `.iter().map(...).filter(...).collect()` chains them. They are lazy, so nothing runs until something asks for the items, and the compiler usually turns the chain into the same loop you would have written by hand.

## Box
aliases: boxed, Box dyn, dyn
ask: What is a Box?
`Box<T>` puts a value on the heap and keeps a pointer to it. `Box<dyn Store>` means "some type that implements Store, decided at run time, behind a pointer". That `dyn` is how Rust does dynamic dispatch when generics would be overkill.

## generic
aliases: generics, type parameter, type parameters
ask: What are generics?
Generics let a function or type work for many types at once: `fn largest<T: Ord>(items: &[T])` works on any `T` that can be ordered. The `T: Ord` part is a bound, a trait the type must implement. The compiler generates a specialised copy for each type actually used, so there is no run-time cost.

## unwrap
aliases: unwraps, expect, panic, panics
ask: What does unwrap do?
`unwrap()` opens an `Option` or a `Result` and gives you the value inside; if there is none, the program panics, which means it stops with a message. Fine in tests and quick scripts, frowned upon in real code, where `?` or `match` handle the failure instead.

## String
aliases: str, string slice, strings
ask: What is the difference between String and str?
`String` is an owned, growable text buffer on the heap. `&str` is a borrowed view into text that someone else owns, a string literal for instance. Functions usually take `&str` so callers can pass either, and keep `String` for text they need to own and keep.

## Vec
aliases: vector, vectors, vec
ask: What is a Vec?
`Vec<T>` is a growable array on the heap, the workhorse collection of Rust: `push` to add, index to read, `iter()` to walk. `Vec<Task>` is a list of tasks that owns its elements.

## HashMap
aliases: hash map
ask: What is a HashMap?
`HashMap<K, V>` is a dictionary: keys of one type, values of another, fast lookup by key. `insert` adds, `get` looks up and returns an `Option`, because the key might not be there. The demo's in-memory storage is one.

## macro
aliases: macros, println, vec!
ask: What is a macro?
A macro is code that writes code at compile time; you spot one by the exclamation mark: `println!`, `format!`, `vec!`. `println!("{}", x)` expands into the calls that print `x`. They exist for things a plain function cannot do, like taking a variable number of arguments or checking a format string.

## mutable
aliases: mut, mutability, immutable
ask: What does mut mean?
Variables are read-only unless you say `let mut`. The same goes for borrows: `&x` lets you read, `&mut x` lets you change. Making mutation explicit is how Rust guarantees that a value cannot change behind your back while someone else is reading it.

## use
aliases: use statement, import, imports
ask: What does use do?
`use crate::model::Task;` brings a name into scope so you can write `Task` instead of the full path. It is an import, nothing more: the code lives where it was defined, `use` only shortens the way you refer to it.

## pub
aliases: public, private, visibility
ask: What does pub mean?
Everything in Rust is private to its module unless marked `pub`. `pub fn run` can be called from outside the module; a plain `fn parse_priority` cannot. It is how a module shows a small door and hides the furniture.

## Self
aliases: self
ask: What is self?
Inside an `impl`, `Self` is the type being implemented and `self` is the value a method was called on. `&self` borrows it to read, `&mut self` borrows it to change, and plain `self` takes ownership. A function in an impl block without `self` is an associated function, called like `Task::new(...)`.

## recursion
aliases: recursive, recurses, calls itself
ask: What is recursion?
A function that calls itself, each time on a smaller piece of the problem, until a base case stops it. Rust allows it like any language; the visualizer shows it as a call edge that loops back to its own node.

## main
aliases: entry point
ask: What is main?
`fn main()` is where a Rust program starts. It can return nothing, or a `Result` so the program can end with an error. Everything else is reached from here, which is why the tour starts at main.
