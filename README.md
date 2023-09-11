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
extern crate yaslapi;

use yaslapi::{State, Type};

fn main() {
    // Initialize test script
    let mut state = State::from_path("test.yasl");

    // Init new variable `answer` with the top of the stack (in this case, the `42`)
    state.push_int(42);
    state.init_global("answer");

    // Add Rust implemented function `rust_print` to globals.
    state.push_cfunction(rust_print, 0);

    // Check that the top of the stack is our C function.
    assert_eq!(state.peek_type(), Type::CFn);

    // Init the function as a global.
    state.init_global("rust_print");

    // Execute `test.yasl`, now that we're done setting everything up
    assert!(state.execute().is_ok());
}
```

## License
yaslapi is licensed under the [MIT License](/LICENSE).
