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

    // Run the REPL.
    loop {
        use std::io::Write;
        // Console prompt.
        print!("yasl> ");
        std::io::stdout().flush().expect("Unable to flush to stdout.");

        //Read a line of input from stdin.
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        // Recreate the execution state from the input.
        state.reset_from_source(&input);

        // Execute the REPL.
        state.execute_repl();
    }
}
