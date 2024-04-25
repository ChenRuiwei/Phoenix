use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use systype::{SysError, SysResult};

use crate::Dentry;

#[derive(Clone)]
pub struct Path {
    /// The root of the file system
    root: Arc<dyn Dentry>,
    /// The directory to start searching from
    start: Arc<dyn Dentry>,
    /// The path to search for
    path: String,
}

impl Eq for Path {}

impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && Arc::ptr_eq(&self.start, &other.start)
    }
}

impl Path {
    pub fn new(root: Arc<dyn Dentry>, start: Arc<dyn Dentry>, path: &str) -> Self {
        Self {
            root,
            start,
            path: path.to_string(),
        }
    }

    pub fn walk(&self) -> SysResult<Arc<dyn Dentry>> {
        let path = self.path.as_str();
        let mut dentry = if is_absolute_path(path) {
            self.root.clone()
        } else {
            self.start.clone()
        };
        log::debug!("[Path::walk] {:?}", split_path(path));
        for p in split_path(path) {
            match p {
                ".." => {
                    dentry = dentry.parent().ok_or(SysError::ENOENT)?;
                }
                name => {
                    dentry = dentry.lookup(name)?;
                }
            }
        }
        Ok(dentry)
    }
}

pub fn is_absolute_path(path: &str) -> bool {
    path.starts_with('/')
}

pub fn is_relative_path(path: &str) -> bool {
    !path.starts_with('/')
}

pub fn split_path(path_name: &str) -> Vec<&str> {
    path_name
        .split('/')
        .filter(|name| !name.is_empty() && *name != ".")
        .collect()
}
