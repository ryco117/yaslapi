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

use crate::{CFunction, State, StateError, StateSuccess, state_result, Type, yaslapi_sys};

/// Helper for specifying the functions for a metatable.
/// Each function will need an identifier, a C-style function, and the number of arguments.
/// The number of arguments is signed to allow for variadic C functions when negative.
pub struct MetatableFunction<'a> {
    pub name: &'a str,
    pub cfn: CFunction,
    pub args: isize,
}

impl State {
    /// Loads all standard libraries into the state and declares them with their default names.
    pub fn declare_libs(&mut self) -> Result<StateSuccess, StateError> {
        unsafe { state_result(yaslapi_sys::YASLX_decllibs(self.state)) }
    }

    /// Initializes a global variable with the given name and initializes it with the top of the stack.
    #[allow(clippy::missing_panics_doc)] // Converting a `&str` to a `CString` can't fail.
    pub fn init_global(&mut self, name: &str) {
        let var_name = CString::new(name).unwrap();

        // Ensure that if the C-string is already in our map that we use the original pointer.
        let cstr = self.lifetime_cstrings.get(&var_name);

        // Initialize the global variable.
        unsafe { yaslapi_sys::YASLX_initglobal(self.state, cstr.unwrap_or(&var_name).as_ptr()) }

        if cstr.is_none() {
            // Prevent the C-string from being dropped.
            self.lifetime_cstrings.insert(var_name);
        }
    }

    /// Inserts all functions in the array into a new table on top of the stack.
    #[allow(clippy::missing_panics_doc)] // Converting a `&str` to a `CString` can't fail.
    pub fn table_set_functions(&mut self, functions: &[MetatableFunction]) {
        // Create a sentinel function to mark the end of the array.
        const SENTINEL_FUNCTION: yaslapi_sys::YASLX_function = yaslapi_sys::YASLX_function {
            name: std::ptr::null(),
            fn_: None,
            args: 0,
        };

        // Allocate enough space for the functions and the sentinel.
        let mut yasl_fns = Vec::with_capacity(functions.len() + 1);

        // Create a YASL function for each function in the array.
        for f in functions {
            let name = CString::new(f.name).unwrap();
            let name_pointer = name.as_ptr();
            unsafe { yaslapi_sys::YASLX_initglobal(self.state, name_pointer) }

            // Create a YASL function from the given data.
            let fn_ = yaslapi_sys::YASLX_function {
                name: name_pointer,
                fn_: Some(f.cfn),
                args: f.args as std::os::raw::c_int,
            };
            yasl_fns.push(fn_);

            // Prevent the C-string from being dropped.
            self.lifetime_cstrings.insert(name);
        }
        // Every list must end with this entry.
        yasl_fns.push(SENTINEL_FUNCTION);

        unsafe { yaslapi_sys::YASLX_tablesetfunctions(self.state, yasl_fns.as_mut_ptr()) }
    }

    /* Crate-Specific Helpers */
    /* ********************** */

    /// Return the underlying value of a global variable, optionally ensuring a type, or return an error.
    /// # Errors
    /// Will return a `StateError::Generic` if the given name is not a global variable.
    /// Will return a `StateError::TypeError` if the object is of a different type than what was expected.
    pub fn pop_global(
        &mut self,
        name: &str,
        expected_type: Option<Type>,
    ) -> Result<Object, StateError> {
        // Load the global variable onto the stack.
        self.load_global(name)?;

        // Pop the global variable off the stack and return.
        self.pop_object(expected_type)
    }

    /// Return the underlying value of the top stack object, optionally ensuring a type, or return an error.
    /// # Errors
    /// Will return a `StateError::TypeError` if the object is of a different type than what was expected.
    pub fn pop_object(&mut self, expected_type: Option<Type>) -> Result<Object, StateError> {
        // Check the type on the stack.
        let stack_type = self.peek_type();
        if let Some(object_type) = expected_type {
            // If the caller expected a certain type which wasn't found, return an error.
            if stack_type != object_type {
                return Err(StateError::TypeError);
            }
        }

        // Get the underlying value.
        match stack_type {
            Type::Bool => Ok(Object::Bool(self.pop_bool())),
            Type::Int => Ok(Object::Int(self.pop_int())),
            Type::Float => Ok(Object::Float(self.pop_float())),
            Type::Str => Ok(Object::Str(self.pop_str().unwrap_or_default())),
            Type::List => {
                // Clone the top of the stack so it isn't consumed by `len`.
                self.clone_top()?;

                // Get the length of the list.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let n = {
                    self.len();
                    self.pop_int() as usize
                };

                // Create a vector to hold the list.
                let mut list = Vec::with_capacity(n);

                // Iterate over the list and push each object onto the vector.
                for i in 0..n {
                    // Get the object at index `i` and push it onto the stack.
                    #[allow(clippy::cast_possible_wrap)]
                    self.list_get(i as isize)?;

                    // Pop the object off of the stack and push it onto the vector.
                    // NOTE: We don't forward the expected type since if the original
                    // caller expected a list, they didn't expect a list of lists.
                    list.push(self.pop_object(None)?);
                }
                Ok(Object::List(list))
            }
            //Type::Table => Ok(Object::Table(self.pop_table()?)),
            //Type::Userdata => Ok(Object::Userdata(self.pop_userdata()?)),
            //Type::Userptr => Ok(Object::Userptr(self.pop_userptr()?)),
            t => {
                // Temporary warning for unhandled types.
                if !matches!(t, Type::Undef) {
                    println!("Warning: Unhandled type: {t:?}");
                }

                // Pop the object off of the stack and return `Undef`.
                self.pop();
                Ok(Object::Undef)
            }
        }
    }
}

/// Helper enum for wrapping a YASL object
pub enum Object {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Object>),
    //Table(Vec<(Object, Object)>),
    //Userdata(*mut yaslapi_sys::),
    //Userptr(*mut yaslapi_sys::),
    Undef,
}

/// Get the type of a YASL `Object` enum.
impl From<Object> for Type {
    fn from(value: Object) -> Self {
        match value {
            Object::Bool(_) => Type::Bool,
            Object::Int(_) => Type::Int,
            Object::Float(_) => Type::Float,
            Object::Str(_) => Type::Str,
            Object::List(_) => Type::List,
            //Object::Table(_) => Type::Table,
            //Object::Userdata(_) => Type::Userdata,
            //Object::Userptr(_) => Type::Userptr,
            Object::Undef => Type::Undef,
        }
    }
}

/// Helper for getting an underlying bool from the `Object` enum.
impl TryFrom<Object> for bool {
    type Error = Type;
    fn try_from(value: Object) -> Result<Self, Self::Error> {
        match value {
            Object::Bool(b) => Ok(b),
            o => Err(o.into()),
        }
    }
}
/// Helper for getting an underlying float from the `Object` enum.
impl TryFrom<Object> for f64 {
    type Error = Type;
    fn try_from(value: Object) -> Result<Self, Self::Error> {
        match value {
            Object::Float(f) => Ok(f),
            o => Err(o.into()),
        }
    }
}
/// Helper for getting an underlying integer from the `Object` enum.
impl TryFrom<Object> for i64 {
    type Error = Type;
    fn try_from(value: Object) -> Result<Self, Self::Error> {
        match value {
            Object::Int(i) => Ok(i),
            o => Err(o.into()),
        }
    }
}
/// Helper for getting an underlying string from the `Object` enum.
impl TryFrom<Object> for String {
    type Error = Type;
    fn try_from(value: Object) -> Result<Self, Self::Error> {
        match value {
            Object::Str(str) => Ok(str),
            o => Err(o.into()),
        }
    }
}
/// Helper for getting an object-list from an `Object` enum of type list.
impl TryFrom<Object> for Vec<Object> {
    type Error = Type;
    fn try_from(value: Object) -> Result<Self, Self::Error> {
        match value {
            Object::List(list) => Ok(list),
            o => Err(o.into()),
        }
    }
}
