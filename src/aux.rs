use std::ffi::CString;

use super::yaslapi_sys::YASLX_initglobal;
use super::State;

impl State {
    #[allow(clippy::missing_panics_doc)]
    pub fn init_global(&mut self, name: &str) {
        let var_name = CString::new(name).unwrap();

        // TODO: Is this needed?
        assert!(!self.global_ids.contains(&var_name));

        let var_pointer = var_name.as_ptr();
        self.global_ids.insert(var_name);
        unsafe { YASLX_initglobal(self.state, var_pointer) }
    }
}
