use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::fmt::Error;

use systype::{SysError, SysResult};

use crate::{dentry, Dentry, InodeMode, OpenFlags};

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

    pub fn walk(&self, flags: OpenFlags, mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        let path = self.path.as_str();
        let mut dentry = if is_absolute_path(path) {
            self.root.clone()
        } else {
            self.start.clone()
        };
        log::debug!("[Path::walk] {:?}", split_path(path));
        let name = split_path(path).last().unwrap().to_string();
        for p in split_path(path) {
            match p {
                ".." => {
                    dentry = dentry.parent().ok_or(SysError::ENOENT)?;
                }
                name => match dentry.lookup(name) {
                    Ok(sub_dentry) => {
                        log::debug!("[Path::walk] sub dentry {}", sub_dentry.name());
                        dentry = sub_dentry
                    }
                    Err(e) => {
                        log::error!("[Path::walk] error {e:?}");
                        return Err(e);
                    }
                },
            }
        }
        if flags.contains(OpenFlags::O_CREAT) {
            // If pathname does not exist, create it as a regular file.
            log::debug!("[Path::walk] create {name}");
            dentry = dentry
                .parent()
                .expect("can not be root dentry")
                .create(&name, InodeMode::FILE)?
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
