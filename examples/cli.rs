use clap::{arg, command, Parser};
use rustyline::{error::ReadlineError, DefaultEditor};
use yaslapi::State;

// C-style function to quit from the REPL.
unsafe extern "C" fn repl_quit(_: *mut yaslapi_sys::YASL_State) -> i32 {
    std::process::exit(0);
}

// Constants that are better defined manually.
const ABOUT: &str =
    "A reference implementation command line interface for Yet Another Scripting Language (YASL).";
const AUTHORS: &str = "Thiabaud Engelbrecht, Ryan Andersen";

// Use crate `clap` to parse command line arguments.
#[derive(Parser)]
#[command(about = ABOUT, author = AUTHORS, version, long_about = None)]
// Default template does not contain an authors section.
#[command(
    help_template = "Authors: {author-with-newline}Version: {version}\n{about-section}\n{usage-heading} {usage}\n\n{all-args}{tab}"
)]
struct Arguments {
    /// Compiles the given `input` source or script instead of executing it.
    #[arg(short, long, default_value_t = false)]
    compile: bool,

    /// Executes `input` as code and prints result of last statement.
    #[arg(short, long, default_value_t = false)]
    execute_print: bool,

    /// Executes `input` as code.
    #[arg(short = 'E', long, default_value_t = false)]
    execute: bool,

    /// Path to an optional script (or literal source with -e or -E) to execute.
    #[arg(trailing_var_arg = true)]
    input: Option<String>,
}

fn main() {
    // Parse the command line arguments.
    let args = Arguments::parse();

    // Helper function to execute source code.
    let execute_helper = |src: &str, args_compile, args_execute_print| {
        let mut state = State::from_source(&src);
        state.declare_libs();

        let _ = if args_compile {
            state.compile()
        } else {
            if args_execute_print {
                state.execute_repl()
            } else {
                state.execute()
            }
        };
    };

    // Check if we were given source expressions from the arguments.
    if args.execute_print || args.execute {
        if let Some(input) = args.input {
            execute_helper(&input, args.compile, args.execute_print);
        }
        return;
    }

    // Check if we were given a script location from the arguments.
    if let Some(input) = args.input {
        let mut state = State::from_path(&input).expect("Could not read file.");
        state.declare_libs();

        let _ = if args.compile {
            state.compile()
        } else {
            state.execute()
        };
        return;
    }

    // Create a new state.
    let mut state = State::default();

    // Declare the standard library.
    state.declare_libs();

    // Add a global `quit` function.
    state.push_cfunction(repl_quit, 0);
    state.init_global_slice("quit").unwrap();

    // Create a new single line editor.
    let mut reader = DefaultEditor::new().expect("Could not allocate a default line editor.");

    // Run the REPL.
    loop {
        // Console prompt.
        match reader.readline("yasl> ") {
            Ok(mut line) => {
                // Append to the history.
                let _ = reader.add_history_entry(line.as_str());

                // Append a newline character.
                line.push('\n');

                // Recreate the execution state from the input.
                state.reset_from_source(&line);

                let _ = if args.compile {
                    // Compile the source.
                    state.compile()
                } else {
                    // Execute the REPL.
                    state.execute_repl()
                };
            }
            Err(ReadlineError::Eof) | Err(ReadlineError::Interrupted) => {
                println!("Quit signal received.");
                // Exit the REPL.
                break;
            }
            Err(err) => {
                // An unexpected error occurred.
                panic!("Error: {err:?}");
            }
        }
    }
}
