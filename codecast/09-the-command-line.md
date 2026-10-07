## The command line and the server
@ calls:iwr::main
The iwr binary is one match over subcommands, and the call tree shows what each one reaches: serve, analyze, export, brief, guide, speak, check, summary.
! iwr::main/b1
= crates/iwr/src/main.rs:107:15-26
clap parses the arguments into a Cli value; the derive on the enum is the whole argument parser.
! iwr::main/b20 iwr::main/b24
brief analyzes the project and prints the compact text a writer reads: types, traits, then every function with its block lines. With the guide flag, the writing guide comes first.
! iwr::main/b49 iwr::main/b53
check analyzes, parses the script, and asks script check for every ref that does not resolve. Then it looks for missing audio files and for words the voice would mispronounce.
! iwr::main/b78 iwr::main/b82
Exit code one when anything is wrong, so a codecast can be checked in CI like a test.
@ flow:iwr::serve::serve
serve is the command you use most. It analyzes the project once, loads the codecast next to it, and starts the web server.
! iwr::serve::serve/b2 iwr::serve::serve/b4
No script given? locate looks for a codecast directory, then a codecast file, next to the project. The demo works with no flags for that reason.
! iwr::serve::serve/b8 iwr::serve::serve/b9
The script is checked at startup too, and problems are printed on stderr, but the server starts anyway: a broken ref should not stop you from looking at the code.
! iwr::serve::serve/b19
= crates/iwr/src/serve.rs:78:27-53
Inside the async block, a file watcher: when a Rust file changes, the project is analyzed again and the browser picks the new version up on its next poll. Edit, save, look.
? What does the browser poll?
  A version number. The UI asks the server every second and a half for the current version; when it changes, it fetches the project JSON again and re-applies the current cue. The page never reloads, so the codecast keeps its place while you edit the code it talks about.
