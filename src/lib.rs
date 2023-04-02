#![allow(clippy::must_use_candidate)]
use num_derive::FromPrimitive;
use std::{collections::HashSet, ffi::CString};

extern crate yaslapi_sys;

mod aux;

use yaslapi_sys::YASL_State;

type YaslFunc = unsafe extern "C" fn(*mut YASL_State) -> i32;

#[allow(clippy::cast_possible_wrap)]
#[derive(Debug, FromPrimitive, PartialEq)]
pub enum Error {
    Success = yaslapi_sys::YASL_Error_YASL_SUCCESS as isize,
    ModuleSuccess = yaslapi_sys::YASL_Error_YASL_MODULE_SUCCESS as isize,
    Error = yaslapi_sys::YASL_Error_YASL_ERROR as isize,
    InitError = yaslapi_sys::YASL_Error_YASL_INIT_ERROR as isize,
    SyntaxError = yaslapi_sys::YASL_Error_YASL_SYNTAX_ERROR as isize,
    TypeError = yaslapi_sys::YASL_Error_YASL_TYPE_ERROR as isize,
    DivideByZeroError = yaslapi_sys::YASL_Error_YASL_DIVIDE_BY_ZERO_ERROR as isize,
    ValueError = yaslapi_sys::YASL_Error_YASL_VALUE_ERROR as isize,
    TooManyVarError = yaslapi_sys::YASL_Error_YASL_TOO_MANY_VAR_ERROR as isize,
    PlatformNotSupp = yaslapi_sys::YASL_Error_YASL_PLATFORM_NOT_SUPP as isize,
    AssertError = yaslapi_sys::YASL_Error_YASL_ASSERT_ERROR as isize,
    StackOverflowError = yaslapi_sys::YASL_Error_YASL_STACK_OVERFLOW_ERROR as isize,
}

pub struct State {
    state: *mut YASL_State,
    global_ids: HashSet<CString>,
}

impl State {
    #[allow(clippy::missing_panics_doc)]
    pub fn new(script_location: &str) -> State {
        let script_location = CString::new(script_location).unwrap();
        let state = unsafe { yaslapi_sys::YASL_newstate(script_location.as_ptr()) };
        assert!(!state.is_null());
        State {
            state,
            global_ids: HashSet::new(),
        }
    }

    #[allow(clippy::missing_panics_doc)]
    pub fn declare_global(&mut self, name: &str) -> i32 {
        let var_name = CString::new(name).unwrap();

        // TODO: Is this needed?
        assert!(!self.global_ids.contains(&var_name));

        let name_pointer = var_name.as_ptr();
        self.global_ids.insert(var_name);

        unsafe { yaslapi_sys::YASL_declglobal(self.state, name_pointer) }
    }

    pub fn dupe_top(&self) -> i32 {
        unsafe { yaslapi_sys::YASL_duptop(self.state) }
    }

    // Type check
    pub fn is_bool(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isbool(self.state) }
    }
    pub fn is_float(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isfloat(self.state) }
    }
    pub fn is_int(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isint(self.state) }
    }
    pub fn is_list(&self) -> bool {
        unsafe { yaslapi_sys::YASL_islist(self.state) }
    }
    pub fn is_str(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isstr(self.state) }
    }
    pub fn is_table(&self) -> bool {
        unsafe { yaslapi_sys::YASL_istable(self.state) }
    }
    pub fn is_undef(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isundef(self.state) }
    }
    pub fn is_userdata(&self, tag: &CString) -> bool {
        unsafe { yaslapi_sys::YASL_isuserdata(self.state, tag.as_ptr()) }
    }
    pub fn is_userptr(&self) -> bool {
        unsafe { yaslapi_sys::YASL_isuserptr(self.state) }
    }

    // Push data
    pub fn push_bool(&self, b: bool) {
        unsafe { yaslapi_sys::YASL_pushbool(self.state, b) }
    }
    pub fn push_int(&self, i: i64) {
        unsafe { yaslapi_sys::YASL_pushint(self.state, i) }
    }
    pub fn push_cfunction(&self, f: YaslFunc, num_args: i32) {
        unsafe { yaslapi_sys::YASL_pushcfunction(self.state, Some(f), num_args) }
    }

    #[allow(clippy::missing_panics_doc)]
    pub fn execute(&self) -> Error {
        let r = unsafe { yaslapi_sys::YASL_execute(self.state) };
        match num::FromPrimitive::from_i32(r) {
            Some(r) => r,
            None => panic!("Unknown error value was returned: {r:?}"),
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        let r = unsafe { yaslapi_sys::YASL_delstate(self.state) };
        assert_eq!(Error::Success, num::FromPrimitive::from_i32(r).unwrap());
    }
}

mod tests {
    use super::*;

    #[no_mangle]
    unsafe extern "C" fn rust_print(_state: *mut YASL_State) -> i32 {
        println!("This is a test");
        0
    }

    #[test]
    fn test_basic_functionality() {
        // Initialize test script
        let mut state = State::new("test.yasl");

        // Init new variable `answer` with the top of the stack (in this case, the `42`)
        state.push_int(42);
        state.init_global("answer");

        // Add Rust implemented function `rust_print` to globals
        state.push_cfunction(rust_print, 0);
        state.init_global("rust_print");

        // Execute `test.yasl`, now that we're done setting everything up
        assert_eq!(state.execute(), Error::Success);
    }
}
