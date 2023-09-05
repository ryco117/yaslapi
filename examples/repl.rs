use rustyline::{error::ReadlineError, DefaultEditor};
use yaslapi::State;

// C-style function to quit from the REPL.
unsafe extern "C" fn repl_quit(_: *mut yaslapi_sys::YASL_State) -> i32 {
    std::process::exit(0);
}

fn main() {
    // Create a new state.
    let mut state = State::default();

    // Declare the standard library.
    state.declare_libs();

    // Add a global `quit` function.
    state.push_cfunction(repl_quit, 0);
    state.init_global("quit");

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

                // Execute the REPL.
                state.execute_repl();
            }
            Err(ReadlineError::Eof) | Err(ReadlineError::Interrupted) => {
                println!("Quit signal received.");

                // Exit the REPL.
                break;
            }
            Err(err) => {
                // Print the error.
                println!("Error: {err:?}");
            }
        }
    }
}
