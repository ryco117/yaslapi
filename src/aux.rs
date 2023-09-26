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
    collections::HashMap,
    ffi::{CStr, CString},
    ptr::NonNull,
};

use yaslapi_sys::YASL_State;

use crate::{CFunction, InvalidIdentifier, State, StateError, Type, LIFETIME_CSTRINGS};

/// Helper type for wrapping a C-style function pointer.
pub struct YaslCFn {
    pub cfn: unsafe extern "C" fn(*mut YASL_State) -> i32,
    pub args: isize,
}

#[macro_export]
macro_rules! new_cfn {
    ($(#[$attr:meta])* $fn_name:ident, $const_name:ident, $args:expr, $state:ident $func:expr) => {
        $(#[$attr])*
        unsafe extern "C" fn $fn_name(state: *mut YASL_State) -> i32 {
            let mut $state: State = state.try_into().expect("State is null");
            $func
        }
        const $const_name: YaslCFn = YaslCFn { cfn: $fn_name, args: $args };
    }
}
pub use new_cfn;

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
    pub fn declare_libs(&mut self) {
        unsafe {
            yaslapi_sys::YASLX_decllibs(self.state.as_ptr());
        }
    }

    /// Declares a global variable with the given name and initializes it with the top of the stack.
    /// The top of the stack is popped after the global is initialized.
    /// # Errors
    /// Will return an `InvalidIdentifier` if the given name is not a valid YASL identifier.
    pub fn init_global(&mut self, name: &'static CStr) -> Result<(), InvalidIdentifier> {
        // Ensure that the name is a valid YASL identifier.
        if !name.to_str().map_or(false, crate::is_valid_identifier) {
            return Err(InvalidIdentifier);
        }

        // Initialize the global variable.
        unsafe {
            yaslapi_sys::YASLX_initglobal(self.state.as_ptr(), name.as_ptr());
        }

        Ok(())
    }
    /// Declares a global variable with the given name and initializes it with the top of the stack.
    /// The top of the stack is popped after the global is initialized.
    /// The string `name` is copied as a new `CString` to a static `HashSet<_>` to provide
    /// a valid C-string pointer for the lifetime of the program, as YASL requires.
    /// # Errors
    /// Will return an `InvalidIdentifier` if the given name is not a valid YASL identifier.
    #[allow(clippy::missing_panics_doc)] // Unwrapping mutex lock should never fail.
    pub fn init_global_slice(&mut self, name: &str) -> Result<(), InvalidIdentifier> {
        // Ensure that the name is a valid YASL identifier.
        if !crate::is_valid_identifier(name) {
            return Err(InvalidIdentifier);
        }

        let var_name = CString::new(name).map_err(|_| InvalidIdentifier)?;
        let mut lifetime_strings = LIFETIME_CSTRINGS.lock().unwrap();

        // Ensure that if the C-string is already in our map that we use the original pointer.
        let existing_cstr = lifetime_strings.get(&var_name);

        // Initialize the global variable.
        unsafe {
            yaslapi_sys::YASLX_initglobal(
                self.state.as_ptr(),
                existing_cstr.unwrap_or(&var_name).as_ptr(),
            );
        }

        if existing_cstr.is_none() {
            // Prevent the C-string from being dropped.
            lifetime_strings.insert(var_name);
        }
        Ok(())
    }

    /// Inserts all functions in the array into a new table on top of the stack.
    /// # Panics
    /// The name of each function must not contain internal zero bytes.
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

            // Create a YASL function from the given data.
            yasl_fns.push(yaslapi_sys::YASLX_function {
                name: name_pointer,
                fn_: Some(f.cfn),
                args: f.args as std::os::raw::c_int,
            });

            // Prevent the C-string from being dropped.
            LIFETIME_CSTRINGS.lock().unwrap().insert(name);
        }
        // Every list must end with this entry.
        yasl_fns.push(SENTINEL_FUNCTION);

        unsafe { yaslapi_sys::YASLX_tablesetfunctions(self.state.as_ptr(), yasl_fns.as_mut_ptr()) }
    }

    /* Crate-Specific Helpers */
    /* ********************** */

    /// Return the underlying value of a global variable, optionally ensuring a type, or return an error.
    /// The string `name` is copied to a `CString` before being given to the YASL runtime.
    /// # Errors
    /// Will return a `StateError::Generic` if the given name is not a global variable.
    /// Will return a `StateError::TypeError` if the object is of a different type than what was expected.
    pub fn pop_global_slice(
        &mut self,
        name: &str,
        expected_type: Option<Type>,
    ) -> Result<Object, StateError> {
        // Load the global variable onto the stack.
        self.load_global_slice(name)?;

        // Pop the global variable off the stack and return.
        self.pop_object(expected_type)
    }

    /// Return the underlying value of the top stack object, optionally ensuring a type, or return an error.
    /// # Errors
    /// Will return a `StateError::TypeError` if the object is of a different type than what was expected.
    #[allow(clippy::missing_panics_doc)] // Getting a `HashableObject` from a `Table` key can't fail.
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
                self.clone_top();

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
            Type::Table => {
                let mut table = HashMap::new();

                // Give an empty start index to `table_next` to get the first key.
                self.push_undef();

                // Iterate over the table and insert each key-value pair into the map.
                while self.table_next() {
                    // Pop the key and value off of the stack.
                    // Similat to the note above, we don't forward the expected type
                    // to the key or value.
                    let k: HashableObject = self
                        .pop_object(None)?
                        .try_into()
                        .expect("Internal Error: Invalid key type.");
                    let v = self.pop_object(None)?;
                    table.insert(k, v);
                }
                Ok(Object::Table(table))
            }
            Type::UserData => {
                let tag = self.peek_type_name();
                Ok(Object::UserData {
                    data: self.pop_userdata(),
                    tag,
                })
            }
            Type::UserPtr => Ok(Object::UserPtr(self.pop_userptr())),
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

/// Helper enum for wrapping a YASL `Object`.
#[derive(Clone, Debug)]
pub enum Object {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Object>),
    Table(HashMap<HashableObject, Object>),
    UserData {
        data: Option<NonNull<std::os::raw::c_void>>,
        tag: Option<&'static CStr>,
    },
    UserPtr(Option<NonNull<std::os::raw::c_void>>),
    Undef,
}

/// YASL `Object`s which are capable of being used as keys to a table.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HashableObject {
    Bool(bool),
    Int(i64),
    Float(HashableF64),
    Str(String),
    UserPtr(Option<NonNull<std::os::raw::c_void>>),
    Undef,
}

/// Helper struct for making the `Object` type usable for indexing tables.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HashableF64(f64);
/// Ensure that this type is hashable.
impl std::hash::Hash for HashableF64 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}
/// Ensure that this type is usable as a key in a hash map.
impl Eq for HashableF64 {}
impl From<HashableF64> for f64 {
    /// Helper to get the underlying f64.
    fn from(value: HashableF64) -> Self {
        value.0
    }
}
impl TryFrom<Object> for HashableObject {
    type Error = Type;
    /// Helper to convert a YASL `Object` into a `HashableObject`, or return the error
    /// value if the type cannot be used as a key.
    fn try_from(value: Object) -> Result<Self, Self::Error> {
        match value {
            Object::Bool(b) => Ok(Self::Bool(b)),
            Object::Int(i) => Ok(Self::Int(i)),
            Object::Float(f) => Ok(Self::Float(HashableF64(f))),
            Object::Str(s) => Ok(Self::Str(s)),
            Object::UserPtr(p) => Ok(Self::UserPtr(p)),
            Object::Undef => Ok(Self::Undef),
            v => Err(v.into()),
        }
    }
}
impl From<HashableObject> for Object {
    /// Helper to convert a `HashableObject` into a YASL `Object`.
    fn from(value: HashableObject) -> Self {
        match value {
            HashableObject::Bool(b) => Self::Bool(b),
            HashableObject::Int(i) => Self::Int(i),
            HashableObject::Float(f) => Self::Float(f.into()),
            HashableObject::Str(s) => Self::Str(s),
            HashableObject::UserPtr(p) => Self::UserPtr(p),
            HashableObject::Undef => Self::Undef,
        }
    }
}

/// Get the type of a YASL `Object` enum.
impl From<&Object> for Type {
    fn from(value: &Object) -> Self {
        match value {
            Object::Bool(_) => Type::Bool,
            Object::Int(_) => Type::Int,
            Object::Float(_) => Type::Float,
            Object::Str(_) => Type::Str,
            Object::List(_) => Type::List,
            Object::Table(_) => Type::Table,
            Object::UserData { .. } => Type::UserData,
            Object::UserPtr(_) => Type::UserPtr,
            Object::Undef => Type::Undef,
        }
    }
}
/// Get the type of a YASL `Object` enum.
impl From<Object> for Type {
    fn from(value: Object) -> Self {
        Self::from(&value)
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

impl<'a> MetatableFunction<'a> {
    /// Create a new `MetatableFunction` from the given data.
    pub fn new(name: &'a str, cfn: CFunction, args: isize) -> Self {
        Self { name, cfn, args }
    }
}
