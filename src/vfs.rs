use std::{
    error::Error,
    ffi::{OsStr, OsString},
    fmt::Display,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

pub type VfsResult<T> = std::result::Result<T, VfsError>;

#[derive(Debug)]
pub enum VfsError {
    PathDoesNotExist(Vec<OsString>),
    IsFile(Vec<OsString>),
    IsDirectory(Vec<OsString>),
    PoisonError,
}

impl<'a> From<PoisonError<MutexGuard<'a, Fs>>> for VfsError {
    fn from(_: PoisonError<MutexGuard<'a, Fs>>) -> Self {
        VfsError::PoisonError
    }
}

impl Display for VfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VfsError::PathDoesNotExist(os_strings) => f.write_str(&format!(
                "Path does not exist: {}",
                os_strings
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            )),
            VfsError::IsFile(os_strings) => f.write_str(&format!(
                "Path is a file: {}",
                os_strings
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            )),
            VfsError::IsDirectory(os_strings) => f.write_str(&format!(
                "Path is a file: {}",
                os_strings
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            )),
            VfsError::PoisonError => f.write_str("Poison error"),
        }
    }
}
impl Error for VfsError {}

mod fs {
    pub mod memory {
        use std::{collections::BTreeMap, ffi::OsString};

        use super::super::{Fs, VfsError, VfsPath, VfsResult};

        enum Entry {
            File(Vec<u8>),
            Dir(BTreeMap<OsString, Entry>),
        }

        pub struct MemoryFilesystem {
            internals: Entry,
        }

        impl MemoryFilesystem {
            pub fn new() -> VfsPath {
                VfsPath::create_in_fs(Fs::Memory(MemoryFilesystem {
                    internals: Entry::Dir(BTreeMap::new()),
                }))
            }

            pub fn write(&mut self, full_path: &[OsString], contents: Vec<u8>) -> VfsResult<()> {
                let mut entry = &mut self.internals;
                let (filename, paths) = full_path
                    .split_last()
                    .expect("path contains at least one element");
                for path in paths {
                    match entry {
                        Entry::File(_) => {
                            return Err(VfsError::PathDoesNotExist(full_path.into()));
                        }
                        Entry::Dir(items) => {
                            entry = items
                                .get_mut(path)
                                .ok_or_else(|| VfsError::PathDoesNotExist(full_path.into()))?;
                        }
                    }
                }

                match entry {
                    Entry::File(_) => {
                        return Err(VfsError::IsFile(full_path.into()));
                    }
                    Entry::Dir(items) => {
                        items.insert(filename.clone(), Entry::File(contents));
                    }
                }

                Ok(())
            }

            pub fn read(&mut self, full_path: &[OsString]) -> VfsResult<Vec<u8>> {
                let mut entry = &mut self.internals;
                for path in full_path {
                    match entry {
                        Entry::File(_) => {
                            return Err(VfsError::PathDoesNotExist(full_path.into()));
                        }
                        Entry::Dir(items) => {
                            entry = items
                                .get_mut(path)
                                .ok_or_else(|| VfsError::PathDoesNotExist(full_path.into()))?;
                        }
                    }
                }

                match entry {
                    Entry::File(contents) => Ok(contents.clone()),
                    Entry::Dir(_) => {
                        return Err(VfsError::IsDirectory(full_path.into()));
                    }
                }
            }
        }
    }
}

enum Fs {
    Memory(fs::memory::MemoryFilesystem),
}

#[derive(Clone)]
pub struct VfsPath {
    path: Vec<OsString>,
    fs: Arc<Mutex<Fs>>,
}

impl VfsPath {
    fn create_in_fs(fs: Fs) -> VfsPath {
        VfsPath {
            path: vec![],
            fs: Arc::new(Mutex::new(fs)),
        }
    }

    pub fn join(&self, p: &OsStr) -> VfsPath {
        let mut vfs_path = self.clone();
        vfs_path.path.push(p.into());
        vfs_path
    }

    pub fn read(&self) -> VfsResult<Vec<u8>> {
        let mut fs = self.fs.lock()?;
        match &mut *fs {
            Fs::Memory(fs) => fs.read(&self.path),
        }
    }

    pub fn write(&self, contents: Vec<u8>) -> VfsResult<()> {
        let mut fs = self.fs.lock()?;
        match &mut *fs {
            Fs::Memory(fs) => fs.write(&self.path, contents),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{VfsResult, fs::memory::MemoryFilesystem};

    #[test]
    fn in_memory_read_write() -> VfsResult<()> {
        let fs = MemoryFilesystem::new();
        fs.join("hi.txt".as_ref()).write(b"stuff".into()).unwrap();
        let contents = String::from_utf8(fs.join("hi.txt".as_ref()).read().unwrap()).unwrap();
        assert_eq!(contents, "stuff");
        Ok(())
    }
}
