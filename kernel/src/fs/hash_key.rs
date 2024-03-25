use alloc::string::String;

use crate::stack_trace;
#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct HashKey {
    pub parent_ino: usize,
    pub child_name: String,
}

impl HashKey {
    pub fn new(parent_ino: usize, child_name: String) -> Self {
        stack_trace!();
        Self {
            parent_ino,
            child_name,
        }
    }
}