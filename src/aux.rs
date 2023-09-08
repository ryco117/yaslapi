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

use super::yaslapi_sys;
use super::{State, StateResult};

impl State {
    /// Loads all standard libraries into the state and declares them with their default names.
    pub fn declare_libs(&mut self) -> StateResult {
        unsafe { yaslapi_sys::YASLX_decllibs(self.state) }.into()
    }

    /// Initializes a global variable with the given name and initializes it with the top of the stack.
    #[allow(clippy::missing_panics_doc)]
    pub fn init_global(&mut self, name: &str) {
        let var_name = CString::new(name).unwrap();

        // Pointer `name_pointer` would be invalid if this id already exists.
        assert!(!self.global_ids.contains(&var_name));

        let name_pointer = var_name.as_ptr();
        self.global_ids.insert(var_name);
        unsafe { yaslapi_sys::YASLX_initglobal(self.state, name_pointer) }
    }
}
