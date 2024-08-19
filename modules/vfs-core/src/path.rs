use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use async_utils::block_on;
use crate_interface::call_interface;
use systype::{SysError, SysResult};

use crate::{dentry, Dentry, InodeMode, InodeType, OpenFlags};

#[derive(Clone)]
pub struct Path {
    /// The root of the file system
    pub root: Arc<dyn Dentry>,
    /// The directory to start searching from
    pub start: Arc<dyn Dentry>,
    /// The path to search for
    pub path: String,
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

    /// Walk until path has been resolved.
    pub fn walk(&self, flags: OpenFlags) -> SysResult<Arc<dyn Dentry>> {
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
                // NOTE: lookup will only create negative dentry in non-negetive dir dentry
                name => {
                    dentry = if !flags.contains(OpenFlags::O_NOFOLLOW)
                        && dentry.inode()?.itype().is_symlink()
                    {
                        Path::resolve_dentry(dentry)?
                    } else {
                        dentry
                    };
                    match dentry.lookup(name) {
                        Ok(sub_dentry) => {
                            log::debug!("[Path::walk] sub dentry {}", sub_dentry.name());
                            dentry = sub_dentry
                        }
                        Err(e) => {
                            log::warn!("[Path::walk] {e:?} when walking in path {path}");
                            return Err(e);
                        }
                    }
                }
            }
        }
        Ok(dentry)
    }

    pub fn resolve_dentry(dentry: Arc<dyn Dentry>) -> SysResult<Arc<dyn Dentry>> {
        const MAX_RESOLVE_LINK_DEPTH: usize = 40;
        let mut dentry_it = dentry;
        for _ in 0..MAX_RESOLVE_LINK_DEPTH {
            if dentry_it.is_negetive() {
                return Ok(dentry_it);
            }
            match dentry_it.inode()?.itype() {
                InodeType::SymLink => {
                    let path = block_on(async { dentry_it.open()?.readlink_string().await })?;
                    let path = if is_absolute_path(&path) {
                        Path::new(
                            call_interface!(SysRootDentryIf::sys_root_dentry()),
                            call_interface!(SysRootDentryIf::sys_root_dentry()),
                            &path,
                        )
                    } else {
                        Path::new(
                            call_interface!(SysRootDentryIf::sys_root_dentry()),
                            dentry_it.parent().unwrap(),
                            &path,
                        )
                    };
                    let new_dentry = path.walk(OpenFlags::empty())?;
                    dentry_it = new_dentry;
                }
                _ => return Ok(dentry_it),
            }
        }
        Err(SysError::ELOOP)
    }
}

pub fn is_absolute_path(path: &str) -> bool {
    path.starts_with('/')
}

pub fn is_relative_path(path: &str) -> bool {
    !path.starts_with('/')
}

pub fn split_path(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|name| !name.is_empty() && *name != ".")
        .collect()
}

pub fn split_parent_and_name(path: &str) -> (&str, Option<&str>) {
    let trimmed_path = path.trim_start_matches('/');
    trimmed_path.find('/').map_or((trimmed_path, None), |n| {
        (&trimmed_path[..n], Some(&trimmed_path[n + 1..]))
    })
}

/// # Example
///
/// "/" -> "/"
/// "/dir/" -> "dir"
/// "/dir/file" -> "file"
pub fn get_name(path: &str) -> &str {
    path.split('/').last().unwrap_or("/")
}

#[crate_interface::def_interface]
pub trait SysRootDentryIf {
    fn sys_root_dentry() -> Arc<dyn Dentry>;
}
