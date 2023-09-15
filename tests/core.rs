// MIT License

// Copyright (c) 2023 Ryan Andersen

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use yaslapi::{State, StateSuccess, Type};
use yaslapi_sys::YASL_State;

// C-style function to print a constant string.
unsafe extern "C" fn rust_print(_state: *mut YASL_State) -> i32 {
    println!("This is a test");
    StateSuccess::Generic.into()
}

// Given a new YASL `State`, compile and execute immediately.
fn execute_state(state: &mut State) {
    // Execute the state machine.
    assert!(state.execute().is_ok());
}

// Given a new YASL `State`, only compile.
fn compile_state(state: &mut State) {
    // Execute the state machine.
    assert!(state.compile().is_ok());
}

// Given a new YASL `State`, do some basic tests.
fn test_core_functionality(mut state: State, test_fn: &dyn Fn(&mut State) -> ()) {
    // Init new variable `answer` with the top of the stack (in this case, the `42`).
    state.push_int(42);
    state.init_global("answer").unwrap();

    // Add Rust implemented function `rust_print` to globals.
    state.push_cfunction(rust_print, 0);

    // Check that the top of the stack is our C function.
    assert_eq!(state.peek_type(), Type::CFn);

    // Init the function as a global.
    state.init_global("rust_print").unwrap();

    // Now that we're done setting things up, test the state machine.
    test_fn(&mut state);
}

// Test core functionality from script.
#[test]
fn test_core_functionality_from_script() {
    test_core_functionality(
        State::from_path("tests/test.yasl").expect("Could not read the test file."),
        &compile_state,
    );
    test_core_functionality(
        State::from_path("tests/test.yasl").expect("Could not read the test file."),
        &execute_state,
    );
}

// Test core functionality from source string.
#[test]
fn test_core_functionality_from_source() {
    let source_str = include_str!("test.yasl");
    test_core_functionality(State::from_source(source_str), &compile_state);
    test_core_functionality(State::from_source(source_str), &execute_state);
}

// Test mutability of the globals' state.
#[test]
fn test_global_mutability() {
    const NAME: &str = "x";
    const DEFAULT: i64 = 0;

    // Create a new state machine to increment a glabal variable `x`.
    let mut state = State::from_source("x += 1;");

    // Declare and initialize the global variable.
    state.push_int(DEFAULT);
    state.init_global(NAME).unwrap();

    // Ensure that the initial value is correct.
    state
        .load_global_slice(NAME)
        .expect("Failed to load the global");
    assert_eq!(state.pop_int(), DEFAULT);

    // Execute the state machine a single time.
    state
        .execute()
        .expect("Failed to execute the state machine");

    // Ensure the global has increased.
    state.load_global_slice(NAME).expect("");
    let new_value = state.pop_int();
    assert_eq!(new_value, DEFAULT + 1);

    // Execute the state machine three more times.
    state
        .execute()
        .expect("Failed to execute the state machine");
    state
        .execute()
        .expect("Failed to execute the state machine");
    state
        .execute()
        .expect("Failed to execute the state machine");

    // Ensure the global has accrued the correct value.
    state
        .load_global_slice(NAME)
        .expect("Failed to load the global");
    assert_eq!(state.pop_int(), new_value + 3);
}
