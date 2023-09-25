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

use std::{
    ffi::CString,
    ops::AddAssign,
    os::raw::c_void,
    ptr::NonNull,
};

use once_cell::sync::Lazy;
use yaslapi::{aux::MetatableFunction, State, StateError};
use yaslapi_sys::YASL_State;

static TABLE_NAME: Lazy<CString> = Lazy::new(|| CString::new("quaternion").unwrap());

/// Example of a user-defined data type.
#[derive(Clone, Copy, Debug)]
struct Quaternion {
    x: f64,
    y: f64,
    z: f64,
    w: f64,
}

/// Rust-defined operations on the `Quaternion` type.
impl AddAssign for Quaternion {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
        self.w += rhs.w;
    }
}

/// A helper for getting a raw pointer from a safe `Quaternion` object.
impl From<Box<Quaternion>> for NonNull<c_void> {
    fn from(ptr: Box<Quaternion>) -> Self {
        unsafe { NonNull::new_unchecked(Box::<Quaternion>::into_raw(ptr).cast()) }
    }
}

/// Implement the `__add` metatable method for the `Quaternion` type.
unsafe extern "C" fn quat_add(state: *mut YASL_State) -> i32 {
    let mut state: State = state.try_into().expect("State is null");
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

    // Return the number of values pushed to the stack.
    1
}
/// Implement the `tostr` metatable method for the `Quaternion` type.
unsafe extern "C" fn quat_tostr(state: *mut YASL_State) -> i32 {
    let mut state: State = state.try_into().expect("State is null");
    if !state.is_userdata(&TABLE_NAME) {
        state.push_str("Not a quaternion.");
        return StateError::TypeError.into();
    }

    // Pop the quaternion from the stack.
    let Quaternion { x, y, z, w } = if let Some(p) = state.peek_userdata() {
        *p.as_ptr().cast()
    } else {
        return StateError::ValueError.into();
    };

    // Push the string representation of the quaternion.
    state.push_str(&format!("quaternion({x}, {y}, {z}, {w})"));

    // Return the number of values pushed to the stack.
    1
}

/// Implement a safe destructor for the `Quaternion` type.
unsafe extern "C" fn quat_drop(_state: *mut YASL_State, data: *mut std::os::raw::c_void) {
    match NonNull::new(data) {
        Some(ptr) => {
            let _ = unsafe { Box::<Quaternion>::from_raw(ptr.as_ptr().cast()) };
        }
        None => (),
    }
}

#[test]
fn test_basic_metatable() {
    let mut state = State::from_source("echo p + q;");

    // Register an empty metatable by name and bring it to the top of the stack.
    state.push_table();
    state.register_mt(&TABLE_NAME);

    // Register the metatable functions to the table on the stack.
    state
        .load_mt(&TABLE_NAME)
        .expect("Failed to find the metatable.");
    let functions = [
        MetatableFunction::new("__add", quat_add, 2),
        MetatableFunction::new("tostr", quat_tostr, 1),
    ];
    state.table_set_functions(&functions);
    state.pop();

    // Push two test quaternions as globals.
    let p = Box::new(Quaternion {
        x: 1.0,
        y: 2.0,
        z: 3.0,
        w: 4.0,
    });
    let q = Box::new(Quaternion {
        x: -2.0,
        y: -1.0,
        z: -4.0,
        w: -3.0,
    });
    unsafe { state.push_userdata(Some(p.into()), &TABLE_NAME, Some(quat_drop)) };
    state
        .load_mt(&TABLE_NAME)
        .expect("Failed to find the metatable.");
    state
        .set_mt()
        .expect("Failed to pass correct arguments on stack.");
    state
        .init_global("p")
        .expect("Couldn't declare the new global.");

    unsafe { state.push_userdata(Some(q.into()), &TABLE_NAME, Some(quat_drop)) };
    state
        .load_mt(&TABLE_NAME)
        .expect("Failed to find the metatable.");
    state
        .set_mt()
        .expect("Failed to pass correct arguments on stack.");
    state
        .init_global("q")
        .expect("Couldn't declare the new global.");

    // Execute the script.
    state.execute().expect("Failed to execute script.");
}
