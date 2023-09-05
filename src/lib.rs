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

#![allow(clippy::must_use_candidate)]
use num_derive::FromPrimitive;
use std::{collections::HashSet, ffi::CString};

extern crate yaslapi_sys;

mod aux;

use yaslapi_sys::YASL_State;

/// Type for a C-style function that can be called from YASL.
pub type CFunction = unsafe extern "C" fn(*mut YASL_State) -> i32;

/// Define the success and error results that a YASL operation may return.
#[allow(clippy::cast_possible_wrap)]
#[derive(Debug, FromPrimitive, PartialEq)]
#[repr(u32)]
pub enum Error {
    /// Successful execution.
    Success = yaslapi_sys::YASL_Error_YASL_SUCCESS,
    /// Successfully executed as module.
    ModuleSuccess = yaslapi_sys::YASL_Error_YASL_MODULE_SUCCESS,
    /// Generic error.
    Error = yaslapi_sys::YASL_Error_YASL_ERROR,
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

/// Wrapper for the YASL state.
pub struct State {
    state: *mut YASL_State,
    global_ids: HashSet<CString>,
}

impl State {
    /// Initialize a new YASL `State` from a script's filepath.
    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    pub fn from_script(script_location: &str) -> Self {
        let script_location = CString::new(script_location).unwrap();
        Self::new(unsafe { yaslapi_sys::YASL_newstate(script_location.as_ptr()) })
    }

    /// Initialize a new YASL `State` from a string containing the source code.
    #[must_use]
    pub fn from_source(source: &str) -> Self {
        Self::new(unsafe { yaslapi_sys::YASL_newstate_bb(source.as_ptr().cast(), source.len()) })
    }

    /// Private helper for initializing a new YASL `State`.
    #[must_use]
    fn new(state: *mut YASL_State) -> Self {
        assert!(!state.is_null());
        Self {
            state,
            global_ids: HashSet::new(),
        }
    }

    /// Compiles the source for the given YASL `State`, but doesn't run it.
    /// Generally you should use `execute` instead.
    pub fn compile(&mut self) -> Error {
        unsafe { yaslapi_sys::YASL_compile(self.state) }.into()
    }

    /// Add a new global variable to the state with default value `undef`.
    #[allow(clippy::missing_panics_doc)]
    pub fn declare_global(&mut self, name: &str) -> i32 {
        let var_name = CString::new(name).unwrap();

        // Pointer `name_pointer` would be invalid if this id already exists.
        assert!(!self.global_ids.contains(&var_name));

        let name_pointer = var_name.as_ptr();
        self.global_ids.insert(var_name);

        unsafe { yaslapi_sys::YASL_declglobal(self.state, name_pointer) }
    }

    /// Add std collections library to the global scope.
    pub fn declare_lib_collections(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_collections(self.state) }
    }
    /// Add std error-handling library to the global scope.
    pub fn declare_lib_error(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_error(self.state) }
    }
    /// Add std io library to the global scope.
    pub fn declare_lib_io(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_io(self.state) }
    }
    /// Add std math library to the global scope.
    pub fn declare_lib_math(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_math(self.state) }
    }
    /// Add std library importing YASL code to the global scope.
    pub fn declare_lib_require(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_require(self.state) }
    }
    /// Add std library for importing C code to the global scope.
    pub fn declare_lib_require_c(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_require_c(self.state) }
    }
    /// Add std metatable library to the global scope.
    pub fn declare_lib_mt(&mut self) -> i32 {
        unsafe { yaslapi_sys::YASL_decllib_mt(self.state) }
    }

    /// Duplicate the top of the stack.
    pub fn clone_top(&mut self) -> Error {
        unsafe { yaslapi_sys::YASL_duptop(self.state) }.into()
    }

    /// Checks if the top of the stack is a bool.
    pub fn is_bool(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isbool(self.state) }
    }
    /// Checks if the top of the stack is a float.
    pub fn is_float(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isfloat(self.state) }
    }
    /// Checks if the top of the stack is an integer.
    pub fn is_int(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isint(self.state) }
    }
    /// Checks if the top of the stack is a list.
    pub fn is_list(&self) -> bool {
        unsafe { yaslapi_sys::YASL_islist(self.state) }
    }
    /// Checks if the top of the stack is a string.
    pub fn is_str(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isstr(self.state) }
    }
    /// Checks if the top of the stack is a table.
    pub fn is_table(&self) -> bool {
        unsafe { yaslapi_sys::YASL_istable(self.state) }
    }
    /// Checks if the top of the stack is undefined.
    pub fn is_undef(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isundef(self.state) }
    }
    /// Checks if the top of the stack is user-data associated with a given tag.
    pub fn is_userdata(&self, tag: &CString) -> bool {
        unsafe { yaslapi_sys::YASL_isuserdata(self.state, tag.as_ptr()) }
    }
    /// Checks if the top of the stack is a user-pointer.
    pub fn is_userptr(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isuserptr(self.state) }
    }

    /// Returns the type of the top of the stack.
    pub fn peek_type(&self) -> Type {
        unsafe { yaslapi_sys::YASL_peektype(self.state) }.into()
    }
    /// Returns the type of the top of the stack as a string.
    #[allow(clippy::missing_panics_doc)]
    pub fn peek_type_name(&self) -> &str {
        unsafe { std::ffi::CStr::from_ptr(yaslapi_sys::YASL_peektypename(self.state)) }
            .to_str()
            .unwrap()
    }
    /// Returns the userdata value of the top of the stack, if the top of the stack is a userdata.
    pub fn peek_userdata(&self) -> Option<*mut ::std::os::raw::c_void> {
        let ptr = unsafe { yaslapi_sys::YASL_peekuserdata(self.state) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    /// Returns the userptr value of the top of the stack, if the top of the stack is a userptr.
    pub fn peek_userptr(&self) -> Option<*mut ::std::os::raw::c_void> {
        let ptr = unsafe { yaslapi_sys::YASL_peekuserptr(self.state) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    /// TODO: Document.
    pub fn peek_vargs_count(&self) -> i64 {
        unsafe { yaslapi_sys::YASL_peekvargscount(self.state) }
    }

    /// Removes the top of the stack.
    pub fn pop(&mut self) {
        unsafe { yaslapi_sys::YASL_pop(self.state) }
    }

    /// Returns the bool value at the top of the stack, if the top of the stack is a boolean. Otherwise returns false. Removes the top element of the stack.
    pub fn pop_bool(&mut self) -> bool {
        unsafe { yaslapi_sys::YASL_popbool(self.state) }
    }

    /// Returns the str value (nul-terminated) of the top of the stack, if the top of the stack is a str.
    pub fn pop_cstr(&mut self) -> Option<*mut ::std::os::raw::c_char> {
        let ptr = unsafe { yaslapi_sys::YASL_popcstr(self.state) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    /// Returns the float value at the top of the stack, if the top of the stack is a float. Otherwise returns 0.0. Removes the top of the stack.
    pub fn pop_float(&mut self) -> f64 {
        unsafe { yaslapi_sys::YASL_popfloat(self.state) }
    }
    /// Returns the int value of the top of the stack, if the top of the stack is an int. Otherwise returns 0. Removes the top of the stack.
    pub fn pop_int(&mut self) -> i64 {
        unsafe { yaslapi_sys::YASL_popint(self.state) }
    }
    /// TODO
    pub fn pop_userdata(&mut self) -> Option<*mut ::std::os::raw::c_void> {
        let ptr = unsafe { yaslapi_sys::YASL_popuserdata(self.state) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    /// TODO
    pub fn pop_userptr(&mut self) -> Option<*mut ::std::os::raw::c_void> {
        let ptr = unsafe { yaslapi_sys::YASL_popuserptr(self.state) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }
    // Prints a runtime error. @param S the YASL_State in which the error occurred. @param fmt a format string, taking the same parameters as printf.
    // pub fn print_err(S: *mut YASL_State, fmt: *const ::std::os::raw::c_char, ...) {
    //     unsafe { yaslapi_sys::YASL_print_err(S, fmt) }
    // }

    /// Pushes a boolean value onto the stack.
    pub fn push_bool(&mut self, b: bool) {
        unsafe { yaslapi_sys::YASL_pushbool(self.state, b) }
    }
    /// Pushes a C-style function onto the stack.
    pub fn push_cfunction(&mut self, f: CFunction, num_args: i32) {
        unsafe { yaslapi_sys::YASL_pushcfunction(self.state, Some(f), num_args) }
    }
    /// Pushes a double value onto the stack.
    pub fn push_float(&mut self, f: f64) {
        unsafe { yaslapi_sys::YASL_pushfloat(self.state, f) }
    }
    /// Pushes an integer value onto the stack.
    pub fn push_int(&mut self, i: i64) {
        unsafe { yaslapi_sys::YASL_pushint(self.state, i) }
    }
    /// Pushes a length-specified string onto the stack. YASL makes a copy of the given string, and manages the memory for it. The string may have embedded zeroes.
    pub fn push_lstr(&mut self, string: &[i8]) {
        unsafe { yaslapi_sys::YASL_pushlstr(self.state, string.as_ptr(), string.len()) }
    }
    /// Pushes an empty list onto the stack.
    pub fn push_list(&mut self) {
        unsafe { yaslapi_sys::YASL_pushlist(self.state) }
    }
    /// Pushes a nul-terminated string onto the stack. This memory will not be managed by YASL and must outlive the state.
    pub fn push_lit(&mut self, string: &'static str) {
        unsafe { yaslapi_sys::YASL_pushlit(self.state, string.as_ptr().cast()) }
    }
    /// Pushes an empty table onto the stack.
    pub fn push_table(&mut self) {
        unsafe { yaslapi_sys::YASL_pushtable(self.state) }
    }
    /// Pushes an undef value onto the stack.
    pub fn push_undef(&mut self) {
        unsafe { yaslapi_sys::YASL_pushundef(self.state) }
    }
    /// Pushes user-data onto the stack, along with a unique tag and destructor for this type.
    /// # Safety
    /// Rust cannot make safety guarantees about data that is being pointed to in YASL.
    pub unsafe fn push_userdata(
        &mut self,
        data: *mut ::std::os::raw::c_void,
        tag: &str,
        destructor: ::std::option::Option<
            unsafe extern "C" fn(arg1: *mut YASL_State, arg2: *mut ::std::os::raw::c_void),
        >,
    ) {
        unsafe { yaslapi_sys::YASL_pushuserdata(self.state, data, tag.as_ptr().cast(), destructor) }
    }
    /// Pushes a user-pointer onto the stack.
    /// # Safety
    /// Rust cannot make safety guarantees about data that is being pointed to in YASL.
    pub unsafe fn push_userptr(&mut self, userpointer: *mut ::std::os::raw::c_void) {
        unsafe { yaslapi_sys::YASL_pushuserptr(self.state, userpointer) }
    }
    /// Pushes a nul-terminated string onto the stack. YASL makes a copy of the given string, and manages the memory for it. The string may not have embedded 0s; it is assumed to end at the first 0.
    pub fn push_zstr(&mut self, cstring: &CString) {
        unsafe { yaslapi_sys::YASL_pushzstr(self.state, cstring.as_ptr()) }
    }

    /// Execute the state's bytecode.
    //#[allow(clippy::missing_panics_doc)]
    pub fn execute(&mut self) -> Error {
        unsafe { yaslapi_sys::YASL_execute(self.state) }.into()
    }

    /// Execute the state's bytecode in REPL mode. The only difference
    /// between `execute_repl` and `execute` is that `execute_repl` will
    /// print the last statement passed to it if that statement is an expression.
    pub fn execute_repl(&mut self) -> Error {
        unsafe { yaslapi_sys::YASL_execute_REPL(self.state) }.into()
    }

    /// Calls a function with `n` parameters. The function must be located below all `n`
    /// parameters it will be called with. The left-most parameter is placed directly above
    /// the function, the right-most paramter at the top of the stack.
    /// # Panics
    /// The argument count `n` must be able to safely convert into a non-negative 32-bit signed integer.
    pub fn function_call(&mut self, n: usize) -> Error {
        unsafe { yaslapi_sys::YASL_functioncall(
            self.state,
            n.try_into().expect("The input argument count cannout be safely converted to a 32-bit signed integer.")
        ) }.into()
    }

    /// Recreate the state machine from the given source code.
    pub fn reset_from_source(&mut self, source: &str) -> Error {
        unsafe { yaslapi_sys::YASL_resetstate_bb(self.state, source.as_ptr().cast(), source.len()) }
            .into()
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
        let r = unsafe { yaslapi_sys::YASL_delstate(self.state) };
        assert_eq!(Error::Success, num::FromPrimitive::from_i32(r).unwrap());
    }
}

/// Safely convert from an integer to a YASL `Error`.
impl From<i32> for Error {
    fn from(e: i32) -> Self {
        match num::FromPrimitive::from_i32(e) {
            Some(r) => r,
            None => panic!("Unknown error was returned: {e:?}"),
        }
    }
}

/// Convert from a YASL `Error` to the underlying integer.
impl From<Error> for i32 {
    fn from(e: Error) -> Self {
        e as Self
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
