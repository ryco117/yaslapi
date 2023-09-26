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

use std::ffi::CString;

use once_cell::sync::Lazy;
use yaslapi::{aux::MetatableFunction, State, StateError};
use yaslapi_sys::YASL_State;

type Quaternion = cgmath::Quaternion<f64>;

// Use lazy evaluation to get a static `CString`.
static TABLE_NAME: Lazy<CString> = Lazy::new(|| CString::new("quaternion").unwrap());

yaslapi::new_cfn! {
    /// Implement the `__add` metatable method for the `Quaternion` type.
    QUAT_ADD, 2, state {
        if !(state.is_n_userdata(&TABLE_NAME, 0) && state.is_n_userdata(&TABLE_NAME, 1)) {
            return 0;
        }

        let (p, q): (*mut Quaternion, *const Quaternion) =
            if let (Some(q), Some(p)) = (state.pop_userdata(), state.peek_userdata()) {
                (p.as_ptr().cast(), q.as_ptr().cast())
            } else {
                return 0;
            };

        // Modify the first quaternion in place.
        *p += *q;

        // Return the number of return values pushed to the stack.
        1
    }
}
yaslapi::new_cfn! {
    /// Implement the `tostr` metatable method for the `Quaternion` type.
    QUAT_TOSTR, 1, state {
        if !state.is_userdata(&TABLE_NAME) {
            state.push_str("Not a quaternion.");
            return StateError::TypeError.into();
        }

        // Pop the quaternion from the stack.
        let quaternion: Quaternion = if let Some(p) = state.peek_userdata() {
            *p.as_ptr().cast()
        } else {
            return StateError::ValueError.into();
        };

        // Push the string representation of the quaternion.
        state.push_str(&format!("{quaternion:?}"));

        // Return the number of values pushed to the stack.
        1
    }
}

#[test]
fn test_basic_metatable() {
    let mut state = State::from_source("echo p + q;");

    // Register an empty metatable by name and bring it to the top of the stack.
    state.push_table();
    state.clone_top();
    state.register_mt(&TABLE_NAME);

    // Register the metatable functions to the table on the stack.
    let functions = [
        MetatableFunction::new("__add", QUAT_ADD.cfn, QUAT_ADD.args),
        MetatableFunction::new("tostr", QUAT_TOSTR.cfn, QUAT_TOSTR.args),
    ];
    state.table_set_functions(&functions);
    state.pop();

    // Push two test quaternions as globals.
    state.push_userdata_box(Quaternion::new(1., 2., 3., 4.), &TABLE_NAME);
    state
        .load_mt(&TABLE_NAME)
        .expect("Failed to find the metatable.");
    state
        .set_mt()
        .expect("Failed to pass correct arguments on stack.");
    state
        .init_global_slice("p")
        .expect("Couldn't declare the new global.");

    state.push_userdata_box(Quaternion::new(-2., -1., -4., -3.), &TABLE_NAME);
    state
        .load_mt(&TABLE_NAME)
        .expect("Failed to find the metatable.");
    state
        .set_mt()
        .expect("Failed to pass correct arguments on stack.");
    state
        .init_global_slice("q")
        .expect("Couldn't declare the new global.");

    // Execute the script.
    state.execute().expect("Failed to execute script.");
}
