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

//! # yaslapi
//! yaslapi is a Rust library that provides a safe idiomatic wrapper for the [Yet Another Scripting Language (YASL)](https://github.com/yasl-lang/yasl) API.
//!
//! Then run cargo build to build your project.
//!
//! ## Usage
//! Here’s an example of how to use yaslapi in your Rust code:
//!
//! ```
//! use yaslapi::{State, StateSuccess, Type};
//!
//! // C-style function to print a constant string.
//! unsafe extern "C" fn rust_print(_state: *mut yaslapi_sys::YASL_State) -> i32 {
//!     println!("This is a test");
//!     StateSuccess::Generic.into()
//! }
//!
//! fn main() {
//!     // Initialize test script.
//!     let mut state = State::from_source(r#"echo "The variable 'answer' has value #{answer}", rust_print();"#);
//!
//!     // Init new variable `answer` with the top of the stack (in this case, the `42`).
//!     state.push_int(42);
//!     state.init_global("answer");
//!
//!     // Add Rust implemented function `rust_print` to globals.
//!     state.push_cfunction(rust_print, 0);
//!
//!     // Check that the top of the stack is our C function.
//!     assert_eq!(state.peek_type(), Type::CFn);
//!
//!     // Init the function as a global.
//!     state.init_global("rust_print");
//!
//!     // Execute `test.yasl`, now that we're done setting everything up.
//!     assert!(state.execute().is_ok());
//! }
//! ```

use num_derive::FromPrimitive;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    collections::HashSet,
    ffi::{CStr, CString},
    ptr::NonNull,
    sync::Mutex,
};

mod aux;

use yaslapi_sys::YASL_State;

/// Type for a C-style function that can be called from YASL.
pub type CFunction = unsafe extern "C" fn(state: *mut YASL_State) -> std::os::raw::c_int;

/// Defines the success results that a YASL operation may return from the state machine.
#[derive(Debug, FromPrimitive, PartialEq)]
#[repr(u32)]
pub enum StateSuccess {
    /// Successful execution.
    Generic = yaslapi_sys::YASL_Error_YASL_SUCCESS,
    /// Successfully executed as module.
    ModuleSuccess = yaslapi_sys::YASL_Error_YASL_MODULE_SUCCESS,
}

/// Defines the error results that a YASL operation may return from the state machine.
#[derive(Debug, FromPrimitive, PartialEq)]
#[repr(u32)]
pub enum StateError {
    /// Generic error.
    Generic = yaslapi_sys::YASL_Error_YASL_ERROR,
    /// YASL_State has not been correctly initialised.
    InitError = yaslapi_sys::YASL_Error_YASL_INIT_ERROR,
    /// Syntax error during compilation.
    SyntaxError = yaslapi_sys::YASL_Error_YASL_SYNTAX_ERROR,
    /// Type error (at runtime).
    TypeError = yaslapi_sys::YASL_Error_YASL_TYPE_ERROR,
    /// Division by zero error (at runtime).
    DivideByZeroError = yaslapi_sys::YASL_Error_YASL_DIVIDE_BY_ZERO_ERROR,
    /// Invalid items (at runtime).
    ValueError = yaslapi_sys::YASL_Error_YASL_VALUE_ERROR,
    /// Too many variables in current scope.
    TooManyVarError = yaslapi_sys::YASL_Error_YASL_TOO_MANY_VAR_ERROR,
    /// Platform specific code not supported for this platform.
    PlatformNotSupp = yaslapi_sys::YASL_Error_YASL_PLATFORM_NOT_SUPP,
    /// Assertion failed.
    AssertError = yaslapi_sys::YASL_Error_YASL_ASSERT_ERROR,
    /// Stack overflow occurred.
    StackOverflowError = yaslapi_sys::YASL_Error_YASL_STACK_OVERFLOW_ERROR,
}

/// Define the errors that a YASL operation may return.
#[allow(clippy::cast_possible_wrap)]
#[derive(Debug, FromPrimitive, PartialEq)]
#[repr(i32)]
pub enum Type {
    Undef = yaslapi_sys::YASL_Types_Y_UNDEF,
    Float = yaslapi_sys::YASL_Types_Y_FLOAT,
    Int = yaslapi_sys::YASL_Types_Y_INT,
    Bool = yaslapi_sys::YASL_Types_Y_BOOL,
    Str = yaslapi_sys::YASL_Types_Y_STR,
    List = yaslapi_sys::YASL_Types_Y_LIST,
    Table = yaslapi_sys::YASL_Types_Y_TABLE,
    Fn = yaslapi_sys::YASL_Types_Y_FN,
    Closure = yaslapi_sys::YASL_Types_Y_CLOSURE,
    CFn = yaslapi_sys::YASL_Types_Y_CFN,
    UserPtr = yaslapi_sys::YASL_Types_Y_USERPTR,
    UserData = yaslapi_sys::YASL_Types_Y_USERDATA,
}

/// Lazily-initialized set of `CString`s that are allocated for the lifetime of the program.
static LIFETIME_CSTRINGS: Lazy<Mutex<HashSet<CString>>> = Lazy::new(Mutex::default);

/// Wrapper for the YASL state.
pub struct State {
    state: NonNull<YASL_State>,
}

/// Error returned when trying to initialize a global variable with an invalid name.
#[derive(Debug)]
pub struct InvalidIdentifier;

/// A helper function to determine if the given string is a valid YASL identifier.
pub fn is_valid_identifier(name: &str) -> bool {
    static IDENTIFIER_REGEX: Lazy<Regex> = Lazy::new(|| {
        regex::Regex::new(r#"[A-z_$][A-z0-9_$]*"#)
            .expect("Internal Error: Unable to compile IDENTIFIER_REGEX.")
    });
    IDENTIFIER_REGEX.is_match(name)
}

impl State {
    /// Initialize a new YASL `State` from a script's filepath. Returns `None` if the file does not exist or cannot be read.
    #[allow(clippy::missing_panics_doc)] // Building a `CString` from a `&str` can't fail.
    #[must_use]
    pub fn from_path(script_location: &str) -> Option<Self> {
        let script_location = CString::new(script_location).unwrap();
        let ptr = unsafe { yaslapi_sys::YASL_newstate(script_location.as_ptr()) };

        // Ensure that the pointer is not null before returning the final `State`.
        NonNull::new(ptr).map(|state| Self { state })
    }

    /// Initialize a new YASL `State` from a string containing the source code.
    #[must_use]
    pub fn from_source(source: &str) -> Self {
        Self {
            state: unsafe {
                NonNull::new_unchecked(yaslapi_sys::YASL_newstate_bb(
                    source.as_ptr().cast(),
                    source.len(),
                ))
            },
        }
    }

    /// Compiles the source for the given YASL `State`, but doesn't run it.
    /// Returns `StateSuccess::Generic` if the compilation was successful.
    /// Generally you should use `execute` instead.
    /// # Errors
    /// Will return `StateError::SyntaxError` if the source code contains invalid syntax.
    pub fn compile(&mut self) -> Result<StateSuccess, StateError> {
        unsafe { state_result(yaslapi_sys::YASL_compile(self.state.as_ptr())) }
    }

    /// Add a new global variable to the state with default value `undef`.
    /// The variable `name` must be a valid `YASL` identifier.
    /// # Errors
    /// Will return an `InvalidIdentifier` if the given name is not a valid YASL identifier.
    /// # Panics
    /// The argument `name` must not contain internal zero bytes.
    pub fn declare_global(&mut self, name: &str) -> Result<(), InvalidIdentifier> {
        if !is_valid_identifier(name) {
            return Err(InvalidIdentifier);
        }

        let var_name = CString::new(name).unwrap();
        let mut lifetime_strings = LIFETIME_CSTRINGS.lock().unwrap();

        // Ensure that if the C-string is already in our map that we use the original pointer.
        let cstr = lifetime_strings.get(&var_name);

        // Declare the global variable.
        unsafe {
            yaslapi_sys::YASL_declglobal(self.state.as_ptr(), cstr.unwrap_or(&var_name).as_ptr())
        };

        if cstr.is_none() {
            // Prevent the C-string from being dropped.
            lifetime_strings.insert(var_name);
        }
        Ok(())
    }

    /// Add std collections library to the global scope.
    pub fn declare_lib_collections(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_collections(self.state.as_ptr()) }
    }
    /// Add std error-handling library to the global scope.
    pub fn declare_lib_error(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_error(self.state.as_ptr()) }
    }
    /// Add std io library to the global scope.
    pub fn declare_lib_io(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_io(self.state.as_ptr()) }
    }
    /// Add std math library to the global scope.
    pub fn declare_lib_math(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_math(self.state.as_ptr()) }
    }
    /// Add std library importing YASL code to the global scope.
    pub fn declare_lib_require(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_require(self.state.as_ptr()) }
    }
    /// Add std library for importing C code to the global scope.
    pub fn declare_lib_require_c(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_require_c(self.state.as_ptr()) }
    }
    /// Add std metatable library to the global scope.
    pub fn declare_lib_mt(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_mt(self.state.as_ptr()) }
    }

    /// Duplicate the top item on the stack and push it to the stack.
    pub fn clone_top(&mut self) {
        unsafe {
            yaslapi_sys::YASL_duptop(self.state.as_ptr());
        }
    }

    /// Execute the state's bytecode.
    /// Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// Will return `StateError::SyntaxError` if the source code contains invalid syntax.
    /// May return runtime errors depending on the source code and execution state.
    pub fn execute(&mut self) -> Result<StateSuccess, StateError> {
        unsafe { state_result(yaslapi_sys::YASL_execute(self.state.as_ptr())) }
    }

    /// Execute the state's bytecode in REPL mode. The only difference
    /// between `execute_repl` and `execute` is that `execute_repl` will
    /// print the last statement passed to it if that statement is an expression.
    /// Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// Will return `StateError::SyntaxError` if the source code contains invalid syntax.
    /// May return runtime errors depending on the source code and execution state.
    pub fn execute_repl(&mut self) -> Result<StateSuccess, StateError> {
        unsafe { state_result(yaslapi_sys::YASL_execute_REPL(self.state.as_ptr())) }
    }

    /// Calls a function with `n` parameters. The function must be located below all `n`
    /// parameters it will be called with. The left-most parameter is placed directly above
    /// the function, the right-most paramter at the top of the stack.
    /// The return value is the number of objects that were returned by the function.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a non-negative C signed integer.
    pub fn function_call(&mut self, n: usize) -> usize {
        // TODO: Remove this if YASL API is updated to use unsigned values here.
        #[allow(clippy::cast_sign_loss)]
        unsafe {
            yaslapi_sys::YASL_functioncall(
                self.state.as_ptr(),
                n.try_into().expect(
                    "The input argument count cannout be safely converted to a non-negative C signed integer.",
                ),
            ) as usize
        }
    }

    /// Checks if the top of the stack is a bool.
    #[must_use]
    pub fn is_bool(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isbool(self.state.as_ptr()) }
    }
    /// Checks if the top of the stack is a float.
    #[must_use]
    pub fn is_float(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isfloat(self.state.as_ptr()) }
    }
    /// Checks if the top of the stack is an integer.
    #[must_use]
    pub fn is_int(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isint(self.state.as_ptr()) }
    }
    /// Checks if the top of the stack is a list.
    #[must_use]
    pub fn is_list(&self) -> bool {
        unsafe { yaslapi_sys::YASL_islist(self.state.as_ptr()) }
    }
    /// Checks if the top of the stack is a string.
    #[must_use]
    pub fn is_str(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isstr(self.state.as_ptr()) }
    }
    /// Checks if the top of the stack is a table.
    #[must_use]
    pub fn is_table(&self) -> bool {
        unsafe { yaslapi_sys::YASL_istable(self.state.as_ptr()) }
    }
    /// Checks if the top of the stack is undefined.
    #[must_use]
    pub fn is_undef(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isundef(self.state.as_ptr()) }
    }
    /// Checks if the top of the stack is user-data associated with a given tag.
    /// NOTE: The `tag` is currently checked by memory address instead of string content.
    #[must_use]
    pub fn is_userdata(&self, tag: &'static CStr) -> bool {
        unsafe { yaslapi_sys::YASL_isuserdata(self.state.as_ptr(), tag.as_ptr()) }
    }
    /// Checks if the top of the stack is a user-pointer.
    #[must_use]
    pub fn is_userptr(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isuserptr(self.state.as_ptr()) }
    }

    /// Checks if the object at index `n` from the top of the stack is a bool.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    pub fn is_n_bool(&mut self, n: usize) -> bool {
        unsafe {
            yaslapi_sys::YASL_isnbool(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }
    /// Checks if the object at index `n` from the top of the stack is a float.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    pub fn is_n_float(&mut self, n: usize) -> bool {
        unsafe {
            yaslapi_sys::YASL_isnfloat(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }
    /// Checks if the object at index `n` from the top of the stack is an int.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    pub fn is_n_int(&mut self, n: usize) -> bool {
        unsafe {
            yaslapi_sys::YASL_isnint(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }
    /// Checks if the object at index `n` from the top of the stack is a list.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    pub fn is_n_list(&mut self, n: usize) -> bool {
        unsafe {
            yaslapi_sys::YASL_isnlist(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }
    /// Checks if the object at index `n` from the top of the stack is a string.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    pub fn is_n_str(&mut self, n: usize) -> bool {
        unsafe {
            yaslapi_sys::YASL_isnstr(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }
    /// Checks if the object at index `n` from the top of the stack is a table.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    pub fn is_n_table(&mut self, n: usize) -> bool {
        unsafe {
            yaslapi_sys::YASL_isntable(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }
    /// Checks if the object at index `n` from the top of the stack is `undef`.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    pub fn is_n_undef(&mut self, n: usize) -> bool {
        unsafe {
            yaslapi_sys::YASL_isnundef(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }
    /// Checks if the object at index `n` from the top of the stack is userdata of a given tag.
    /// NOTE: The `tag` is currently checked by memory address instead of string content.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    pub fn is_n_userdata(&mut self, tag: &'static CStr, n: usize) -> bool {
        unsafe {
            yaslapi_sys::YASL_isnuserdata(
                self.state.as_ptr(),
                tag.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }
    /// Checks if the object at index `n` from the top of the stack is userpointer.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    pub fn is_n_userptr(&mut self, n: usize) -> bool {
        unsafe {
            yaslapi_sys::YASL_isnuserptr(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }

    /// Pops the top of the stack, then evaluates `len` on the popped value. The result is pushed to the stack.
    pub fn len(&mut self) {
        unsafe { yaslapi_sys::YASL_len(self.state.as_ptr()) }
    }

    /// Indexes the list on top of the stack and pushes the result to the stack.
    /// If `n` is negative it indexes from the end of the list.
    /// Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// If the object on the stack is not a list then it will return `StateError::TypeError`.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a 64-bit signed integer.
    pub fn list_get(&mut self, n: isize) -> Result<StateSuccess, StateError> {
        unsafe {
            state_result(yaslapi_sys::YASL_listget(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a 64-bit signed integer."),
            ))
        }
    }

    /// Pops the top of the stack and appends it to a list (which should be located directly below the top of the stack).
    /// Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// If the object on the stack is not a list then it will return `StateError::TypeError`.
    pub fn list_push(&mut self) -> Result<StateSuccess, StateError> {
        unsafe { state_result(yaslapi_sys::YASL_listpush(self.state.as_ptr())) }
    }

    /// Loads the specified global from state and pushes it to the stack.
    /// Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// If the global does not exist then it will return `StateError::Generic`.
    pub fn load_global(&mut self, name: &CStr) -> Result<StateSuccess, StateError> {
        unsafe {
            state_result(yaslapi_sys::YASL_loadglobal(
                self.state.as_ptr(),
                name.as_ptr(),
            ))
        }
    }
    /// Loads the specified global from state and pushes it to the stack.
    /// Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// If the global does not exist then it will return `StateError::Generic`.
    /// # Panics
    /// The string slice `name` must not contain internal zero bytes.
    pub fn load_global_slice(&mut self, name: &str) -> Result<StateSuccess, StateError> {
        let name = CString::new(name).unwrap();
        unsafe {
            state_result(yaslapi_sys::YASL_loadglobal(
                self.state.as_ptr(),
                name.as_ptr(),
            ))
        }
    }

    /// Loads a metatable by name. Returns `StateSuccess::Generic` if successful.
    /// # Panics
    /// The string slice `name` must not contain internal zero bytes.
    /// # Errors
    /// If the metatable `name` does not exist then it will return `StateError::Generic`.
    pub fn load_mt(&mut self, name: &str) -> Result<StateSuccess, StateError> {
        let name = CString::new(name).unwrap();
        unsafe { state_result(yaslapi_sys::YASL_loadmt(self.state.as_ptr(), name.as_ptr())) }
    }
    /// Loads a metatable by name. Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// If the metatable `name` does not exist then it will return `StateError::Generic`.
    pub fn load_mt_cstr(&mut self, name: &CStr) -> Result<StateSuccess, StateError> {
        unsafe { state_result(yaslapi_sys::YASL_loadmt(self.state.as_ptr(), name.as_ptr())) }
    }

    // TODO: Determine if these should be added.
    // YASL_loadprintout(struct YASL_State *S);
    // YASL_loadprinterr(struct YASL_State *S);

    /// Returns the boolean value of the top of the stack, if it is a bool.
    /// Otherwise, returns false.
    #[must_use]
    pub fn peek_bool(&self) -> bool {
        unsafe { yaslapi_sys::YASL_peekbool(self.state.as_ptr()) }
    }
    /// Returns the string value of the top of the stack, if the top of the stack is a string.
    /// Otherwise, returns `None`.
    /// # Panics
    /// The viewed string must contain valid UTF-8.
    #[must_use]
    pub fn peek_str(&self) -> Option<String> {
        unsafe {
            let ptr = yaslapi_sys::YASL_peekcstr(self.state.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(
                    CString::from_raw(ptr)
                        .into_string()
                        .expect("Peeked string is not valid UTF-8"),
                )
            }
        }
    }
    /// Returns the float value of the top of the stack, if the top of the stack is a float.
    /// Otherwise, returns 0.0.
    #[must_use]
    pub fn peek_float(&self) -> f64 {
        unsafe { yaslapi_sys::YASL_peekfloat(self.state.as_ptr()) }
    }
    /// Returns the int value of the top of the stack, if the top of the stack is an int.
    /// Otherwise, returns 0.
    #[must_use]
    pub fn peek_int(&self) -> i64 {
        unsafe { yaslapi_sys::YASL_peekint(self.state.as_ptr()) }
    }
    /// Returns the userdata value of the top of the stack, if the top of the stack is a userdata.
    #[must_use]
    pub fn peek_userdata(&self) -> Option<*mut std::os::raw::c_void> {
        let ptr = unsafe { yaslapi_sys::YASL_peekuserdata(self.state.as_ptr()) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    /// Returns the userptr value of the top of the stack, if the top of the stack is a userptr.
    #[must_use]
    pub fn peek_userptr(&self) -> Option<*mut std::os::raw::c_void> {
        let ptr = unsafe { yaslapi_sys::YASL_peekuserptr(self.state.as_ptr()) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    /// Returns the type of the top of the stack.
    #[must_use]
    pub fn peek_type(&self) -> Type {
        unsafe { yaslapi_sys::YASL_peektype(self.state.as_ptr()) }.into()
    }
    /// Returns the type of the top of the stack as a string, or `None` if no string exists.
    #[must_use]
    pub fn peek_type_name(&self) -> Option<&'static CStr> {
        unsafe {
            let ptr = yaslapi_sys::YASL_peektypename(self.state.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr))
            }
        }
    }
    /// Returns the type of the top of the stack as a string, or `None` if no string exists.
    /// # Panics
    /// The type name must contain valid UTF-8. This includes the tags of `UserData` objects.
    #[must_use]
    pub fn peek_type_name_slice(&self) -> Option<&'static str> {
        unsafe {
            let ptr = yaslapi_sys::YASL_peektypename(self.state.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr))
            }
        }
        .map(|s| {
            s.to_str()
                .expect("YASL_peektypename returned invalid UTF-8")
        })
    }

    /// Returns the bool value at index `n` from the top of the stack, if it is a boolean.
    /// Otherwise returns false.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    #[must_use]
    pub fn peek_n_bool(&self, n: usize) -> bool {
        unsafe {
            yaslapi_sys::YASL_peeknbool(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }
    /// Returns the float value at index `n` from the top of the stack, if it is a float.
    /// Otherwise returns 0.0.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    #[must_use]
    pub fn peek_n_float(&self, n: usize) -> f64 {
        unsafe {
            yaslapi_sys::YASL_peeknfloat(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }
    /// Returns the int value at index `n` from the top of the stack, if it is an int.
    /// Otherwise returns 0.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    #[must_use]
    pub fn peek_n_int(&self, n: usize) -> i64 {
        unsafe {
            yaslapi_sys::YASL_peeknint(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        }
    }
    /// Returns the userdata value at index `n` from the top of the stack, if it is a userdata.
    /// Otherwise returns `None`.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    #[must_use]
    pub fn peek_n_userdata(&self, n: usize) -> Option<*mut std::os::raw::c_void> {
        let ptr = unsafe {
            yaslapi_sys::YASL_peeknuserdata(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            )
        };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    /// Returns the type of index `n` from the top of the stack as a string, or `None` if no string exists.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    #[must_use]
    pub fn peek_n_typename(&self, n: usize) -> Option<&'static CStr> {
        unsafe {
            let ptr = yaslapi_sys::YASL_peekntypename(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            );
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr))
            }
        }
    }
    /// Returns the type name of index `n` from the top of the stack as a string, or `None` if no string exists.
    /// # Panics
    /// The type name must contain valid UTF-8. This includes the tags of `UserData` objects.
    /// The argument count `n` must be able to safely convert into a C unsigned integer.
    #[must_use]
    pub fn peek_n_typename_slice(&self, n: usize) -> Option<&'static str> {
        unsafe {
            let ptr = yaslapi_sys::YASL_peekntypename(
                self.state.as_ptr(),
                n.try_into()
                    .expect("Index must be able to safely convert into a C unsigned integer."),
            );
            if ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(ptr))
            }
        }
        .map(|s| {
            s.to_str()
                .expect("YASL_peekntypename returned invalid UTF-8")
        })
    }

    /// TODO: Document.
    #[must_use]
    pub fn peek_vargs_count(&self) -> i64 {
        unsafe { yaslapi_sys::YASL_peekvargscount(self.state.as_ptr()) }
    }

    /// Removes the top of the stack.
    pub fn pop(&mut self) {
        unsafe { yaslapi_sys::YASL_pop(self.state.as_ptr()) }
    }
    /// Returns the bool value at the top of the stack, if the top of the stack is a boolean. Otherwise returns false. Removes the top element of the stack.
    pub fn pop_bool(&mut self) -> bool {
        unsafe { yaslapi_sys::YASL_popbool(self.state.as_ptr()) }
    }
    /// Returns the string value of the top of the stack, if the top of the stack is a string. Otherwise returns `None`. Removes the top of the stack.
    /// # Panics
    /// The popped string must contain valid UTF-8.
    pub fn pop_str(&mut self) -> Option<String> {
        unsafe {
            let ptr = yaslapi_sys::YASL_popcstr(self.state.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(
                    // TODO: Determine if it is possible to create invalid UTF-8 strings from within YASL which would crash here.
                    CString::from_raw(ptr)
                        .into_string()
                        .expect("Popped string is not valid UTF-8"),
                )
            }
        }
    }
    /// Returns the float value at the top of the stack, if the top of the stack is a float. Otherwise returns 0.0. Removes the top of the stack.
    pub fn pop_float(&mut self) -> f64 {
        unsafe { yaslapi_sys::YASL_popfloat(self.state.as_ptr()) }
    }
    /// Returns the int value of the top of the stack, if the top of the stack is an int. Otherwise returns 0. Removes the top of the stack.
    pub fn pop_int(&mut self) -> i64 {
        unsafe { yaslapi_sys::YASL_popint(self.state.as_ptr()) }
    }
    /// Returns the `UserData` value of the top of the stack, if the top of the stack is a `UserData`. Otherwise returns `None`. Removes the top of the stack.
    pub fn pop_userdata(&mut self) -> Option<NonNull<std::os::raw::c_void>> {
        if self.peek_type() == Type::UserData {
            NonNull::new(unsafe { yaslapi_sys::YASL_popuserdata(self.state.as_ptr()) })
        } else {
            // Ensure that we still pop the value off the stack for the caller's sake.
            self.pop();
            None
        }
    }
    /// Returns the `UserPtr` value of the top of the stack, if the top of the stack is a `UserPtr`. Otherwise returns `None`. Removes the top of the stack.
    pub fn pop_userptr(&mut self) -> Option<NonNull<std::os::raw::c_void>> {
        if self.peek_type() == Type::UserPtr {
            NonNull::new(unsafe { yaslapi_sys::YASL_popuserptr(self.state.as_ptr()) })
        } else {
            // Ensure that we still pop the value off the stack for the caller's sake.
            self.pop();
            None
        }
    }

    // TODO: Rust doesn't really support variadic argument lists; more reading required.
    // Prints a runtime error. @param S the YASL_State in which the error occurred. @param fmt a format string, taking the same parameters as printf.
    // pub fn print_err(S: *mut YASL_State, fmt: *const std::os::raw::c_char, ...) {
    //     unsafe { yaslapi_sys::YASL_print_err(S, fmt) }
    // }

    /// Pushes a boolean value onto the stack.
    pub fn push_bool(&mut self, b: bool) {
        unsafe { yaslapi_sys::YASL_pushbool(self.state.as_ptr(), b) }
    }
    /// Pushes a C-style function onto the stack.
    pub fn push_cfunction(&mut self, f: CFunction, num_args: i32) {
        unsafe { yaslapi_sys::YASL_pushcfunction(self.state.as_ptr(), Some(f), num_args) }
    }
    /// Pushes a double value onto the stack.
    pub fn push_float(&mut self, f: f64) {
        unsafe { yaslapi_sys::YASL_pushfloat(self.state.as_ptr(), f) }
    }
    /// Pushes an integer value onto the stack.
    pub fn push_int(&mut self, i: i64) {
        unsafe { yaslapi_sys::YASL_pushint(self.state.as_ptr(), i) }
    }
    /// Pushes an empty list onto the stack.
    pub fn push_list(&mut self) {
        unsafe { yaslapi_sys::YASL_pushlist(self.state.as_ptr()) }
    }
    /// Pushes a nul-terminated string onto the stack. This memory will not be managed by YASL and must outlive the state.
    pub fn push_literal(&mut self, string: &'static CStr) {
        unsafe { yaslapi_sys::YASL_pushlit(self.state.as_ptr(), string.as_ptr().cast()) }
    }
    /// Pushes an empty table onto the stack.
    pub fn push_table(&mut self) {
        unsafe { yaslapi_sys::YASL_pushtable(self.state.as_ptr()) }
    }
    /// Pushes a string onto the stack. YASL makes a copy of the given string, and manages the memory for it.
    pub fn push_str(&mut self, string: &str) {
        unsafe {
            yaslapi_sys::YASL_pushlstr(self.state.as_ptr(), string.as_ptr().cast(), string.len());
        }
    }
    /// Pushes an `undef` value onto the stack.
    pub fn push_undef(&mut self) {
        unsafe { yaslapi_sys::YASL_pushundef(self.state.as_ptr()) }
    }
    /// Pushes user-data onto the stack, along with a unique tag and destructor for this type.
    /// # Safety
    /// Rust cannot make safety guarantees about data that is being pointed to in YASL.
    pub unsafe fn push_userdata(
        &mut self,
        data: *mut std::os::raw::c_void,
        tag: &'static CStr,
        destructor: std::option::Option<
            unsafe extern "C" fn(state: *mut YASL_State, data: *mut std::os::raw::c_void),
        >,
    ) {
        unsafe {
            yaslapi_sys::YASL_pushuserdata(self.state.as_ptr(), data, tag.as_ptr(), destructor);
        }
    }
    /// Pushes a user-pointer onto the stack.
    /// # Safety
    /// Rust cannot make safety guarantees about data that is being pointed to in YASL.
    pub unsafe fn push_userptr(&mut self, userptr: Option<NonNull<std::os::raw::c_void>>) {
        unsafe {
            yaslapi_sys::YASL_pushuserptr(
                self.state.as_ptr(),
                userptr.map_or(std::ptr::null_mut(), NonNull::as_ptr),
            );
        }
    }
    /// Pushes a nul-terminated string onto the stack. YASL makes a copy of the given string, and manages the memory for it.
    pub fn push_zstr(&mut self, cstring: &CStr) {
        unsafe { yaslapi_sys::YASL_pushzstr(self.state.as_ptr(), cstring.as_ptr()) }
    }

    /// Registers a metatable with name `name`. After this returns, the
    /// metatable can be referred to by `name` in other functions dealing
    /// with metatables, e.g. `set_mt(..)` and `load_mt(..)`.
    /// # Panics
    /// The string slice `name` must not contain internal zero bytes.
    pub fn register_mt(&mut self, name: &str) {
        let name = CString::new(name).unwrap();
        unsafe { yaslapi_sys::YASL_registermt(self.state.as_ptr(), name.as_ptr()) };
    }

    /// Recreate the state machine from the given script path.
    /// Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// If the script does not exist or cannot be read then it will return `StateError::Generic`.
    /// # Panics
    /// The string slice `script_location` must not contain internal zero bytes.
    pub fn reset_from_script(&mut self, script_location: &str) -> Result<StateSuccess, StateError> {
        let script_location = CString::new(script_location).unwrap();
        unsafe {
            state_result(yaslapi_sys::YASL_resetstate(
                self.state.as_ptr(),
                script_location.as_ptr(),
            ))
        }
    }
    /// Recreate the state machine from the given source code.
    pub fn reset_from_source(&mut self, source: &str) {
        unsafe {
            yaslapi_sys::YASL_resetstate_bb(
                self.state.as_ptr(),
                source.as_ptr().cast(),
                source.len(),
            );
        }
    }

    /// Pops the top of the YASL stack and stores it in the given global.
    /// Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// If the global does not exist or is `const` then it will return `StateError::Generic`.
    pub fn set_global(&mut self, name: &CStr) -> Result<StateSuccess, StateError> {
        unsafe {
            state_result(yaslapi_sys::YASL_setglobal(
                self.state.as_ptr(),
                name.as_ptr(),
            ))
        }
    }
    /// Pops the top of the YASL stack and stores it in the given global.
    /// Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// If the global does not exist or is `const` then it will return `StateError::Generic`.
    #[allow(clippy::missing_panics_doc)] // Building a `CString` from a `&str` can't fail.
    pub fn set_global_slice(&mut self, name: &str) -> Result<StateSuccess, StateError> {
        let name = CString::new(name).unwrap();
        unsafe {
            state_result(yaslapi_sys::YASL_setglobal(
                self.state.as_ptr(),
                name.as_ptr(),
            ))
        }
    }

    // TODO: Learn what the exact API here is.
    /// Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// The top object on the stack must be either a `Table` or `Undef` or it will return `StateError::TypeError`.
    /// The next object on the stack must be either a `UserData`, `Table`, and `List`
    /// or it will return `StateError::TypeError`.
    pub fn set_mt(&mut self) -> Result<StateSuccess, StateError> {
        unsafe { state_result(yaslapi_sys::YASL_setmt(self.state.as_ptr())) }
    }

    // TODO: Learn if these should be added.
    // void YASL_setprintout_tostr(struct YASL_State *S);
    // void YASL_setprinterr_tostr(struct YASL_State *S);

    // TODO: Learn what the exact API here is.
    pub fn stringify_top(&mut self) {
        unsafe { yaslapi_sys::YASL_stringifytop(self.state.as_ptr()) }
    }

    /// Iterates over a table. The top-most item of the stack should be the previous index in
    /// the table, followed by the table itself. The index is popped, and then if there are
    /// more elements in the table, the next index and value are pushed. No values are pushed
    /// if we are already at the end of the table.
    /// Returns `true` if the next index and value were pushed, `false` otherwise.
    pub fn table_next(&mut self) -> bool {
        unsafe { yaslapi_sys::YASL_tablenext(self.state.as_ptr()) }
    }

    /// Inserts a key-value pair into the table. The top-most items are the value, then key,
    /// then table. The key and value are popped from the stack.
    /// Returns `StateSuccess::Generic` if successful.
    /// # Errors
    /// If the object third from the top of the stack is not a table then it will return `StateError::TypeError`.
    /// If the key is of a type that cannot be hashed (e.g., `List`, `Table`, and `UserData`) then it will return `StateError::TypeError`.
    pub fn table_set(&mut self) -> Result<StateSuccess, StateError> {
        unsafe { state_result(yaslapi_sys::YASL_tableset(self.state.as_ptr())) }
    }

    /// Causes a fatal error.
    /// # Panics
    /// The argument `error` must be able to safely convert into a C signed integer.
    pub fn throw_err(&self, error: isize) -> ! {
        unsafe {
            yaslapi_sys::YASL_throw_err(
                self.state.as_ptr(),
                error
                    .try_into()
                    .expect("Error ID must be able to safely convert into a C signed integer."),
            )
        }
    }
}

/// Implement a default empty state.
impl Default for State {
    fn default() -> Self {
        Self::from_source("")
    }
}

/// Automatically perform proper cleanup of the YASL `State`.
impl Drop for State {
    fn drop(&mut self) {
        unsafe { yaslapi_sys::YASL_delstate(self.state.as_ptr()) };
    }
}

// Unsafe helper for converting from an integer to a safe YASL `Result`.
unsafe fn state_result(r: i32) -> Result<StateSuccess, StateError> {
    match num::FromPrimitive::from_i32(r) {
        Some(s) => Ok(s),
        None => match num::FromPrimitive::from_i32(r) {
            Some(e) => Err(e),
            None => panic!("Unknown error was returned: {r:?}"),
        },
    }
}

/// Convert from a YASL `StateSuccess` enum to the underlying integer.
impl From<StateSuccess> for i32 {
    fn from(s: StateSuccess) -> Self {
        s as Self
    }
}

/// Convert from a YASL `StateError` enum to the underlying integer.
impl From<StateError> for i32 {
    fn from(s: StateError) -> Self {
        s as Self
    }
}

/// Safely convert from an integer to a YASL `Type`.
impl From<i32> for Type {
    fn from(t: i32) -> Self {
        match num::FromPrimitive::from_i32(t) {
            Some(r) => r,
            None => panic!("Unknown type was returned: {t:?}"),
        }
    }
}

/// Convert from a YASL `Type` to the underlying integer.
impl From<Type> for i32 {
    fn from(t: Type) -> Self {
        t as Self
    }
}
