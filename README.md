# yaslapi
yaslapi is a Rust library that provides safe idiomatic wrapper for the [Yet Another Scripting Language (YASL)](https://github.com/yasl-lang/yasl) API.

## Installation
First, you must have CMake and a C compiler installed so that YASL can be compiled locally.
To install yaslapi, add the following to your `Cargo.toml` file:

```toml
[dependencies]
yaslapi = "0.2.0"
```

Then run cargo build to build your project.

## Usage
Here’s an example of how to use yaslapi in your Rust code:

```rust
use yaslapi::{State, StateSuccess, Type};

// C-style function to print a constant string.
unsafe extern "C" fn rust_print(_state: *mut yaslapi_sys::YASL_State) -> i32 {
    println!("This is a test");

    // Return the number of values pushed to the stack.
    0
}

fn main() {
    // Initialize test script.
    let mut state = State::from_source(r#"echo "The variable 'answer' has value #{answer}", rust_print();"#);

    // Init new variable `answer` with the top of the stack (in this case, the `42`).
    state.push_int(42);
    state.init_global_slice("answer");

    // Add Rust implemented function `rust_print` to globals.
    state.push_cfunction(rust_print, 0);

    // Check that the top of the stack is our C function.
    assert_eq!(state.peek_type(), Type::CFn);

    // Init the function as a global.
    state.init_global_slice("rust_print");

    // Execute `test.yasl`, now that we're done setting everything up.
    assert!(state.execute().is_ok());
}
```

## License
yaslapi is licensed under the [MIT License](/LICENSE).
