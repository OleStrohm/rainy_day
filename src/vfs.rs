use std::{
    ffi::{OsStr, OsString},
    fmt::Display,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub use error::VfsResult;

pub mod error {
    use std::sync::{MutexGuard, PoisonError};

    use crate::vfs::InnerPath;

    pub type VfsResult<T> = std::result::Result<T, VfsError>;

    #[derive(thiserror::Error, Debug)]
    pub enum VfsError {
        #[error("Path does not exist: `{0}`")]
        PathDoesNotExist(InnerPath),
        #[error("Path is a file: `{0}`")]
        IsFile(InnerPath),
        #[error("Path is a directory: `{0}`")]
        IsDirectory(InnerPath),
        #[error("Io Error: `{0:?}`")]
        Io(#[from] std::io::Error),
        #[error("Mutex poison error: `{0}`")]
        PoisonError(String),
    }

    impl<'a, T> From<PoisonError<MutexGuard<'a, T>>> for VfsError {
        fn from(e: PoisonError<MutexGuard<'a, T>>) -> Self {
            VfsError::PoisonError(e.to_string())
        }
    }
}

pub use fs::memory::MemoryFilesystem;

mod fs {
    pub mod memory {
        use std::{collections::BTreeMap, ffi::OsString};

        use crate::vfs::{Fs, InnerPath, VfsPath, VfsResult, error::VfsError};

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
                            return Err(VfsError::PathDoesNotExist(InnerPath::new(
                                full_path.into(),
                            )));
                        }
                        Entry::Dir(items) => {
                            entry = items.get_mut(path).ok_or_else(|| {
                                VfsError::PathDoesNotExist(InnerPath::new(full_path.into()))
                            })?;
                        }
                    }
                }

                match entry {
                    Entry::File(_) => {
                        return Err(VfsError::IsFile(InnerPath::new(full_path.into())));
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
                            return Err(VfsError::PathDoesNotExist(InnerPath::new(
                                full_path.into(),
                            )));
                        }
                        Entry::Dir(items) => {
                            entry = items.get_mut(path).ok_or_else(|| {
                                VfsError::PathDoesNotExist(InnerPath::new(full_path.into()))
                            })?;
                        }
                    }
                }

                match entry {
                    Entry::File(contents) => Ok(contents.clone()),
                    Entry::Dir(_) => {
                        return Err(VfsError::IsDirectory(InnerPath::new(full_path.into())));
                    }
                }
            }

            pub fn create_dir_all(&mut self, full_path: &InnerPath) -> VfsResult<()> {
                let mut entry = &mut self.internals;
                for (i, part) in full_path.parts.iter().enumerate() {
                    match entry {
                        Entry::File(_) => {
                            return Err(VfsError::IsFile(InnerPath::new(
                                full_path.parts[..i + 1].to_owned(),
                            )));
                        }
                        Entry::Dir(items) => {
                            entry = items
                                .entry(part.to_owned())
                                .or_insert(Entry::Dir(Default::default()))
                        }
                    }
                }

                if matches!(entry, Entry::File(..)) {
                    return Err(VfsError::IsFile(full_path.clone()));
                }

                Ok(())
            }
        }
    }
    pub mod physical {
        use std::{
            ffi::OsString,
            io::{Read, Write},
            path::PathBuf,
        };

        use crate::vfs::{Fs, InnerPath, VfsPath, VfsResult};

        pub struct PhysicalFilesystem;

        impl PhysicalFilesystem {
            pub fn new() -> VfsPath {
                VfsPath::create_in_fs(Fs::Physical(PhysicalFilesystem))
            }

            pub fn write(&mut self, full_path: &[OsString], contents: Vec<u8>) -> VfsResult<()> {
                let joined: OsString = full_path.join("/".as_ref());
                let path = PathBuf::from(joined);
                std::fs::File::create(path)?.write(&contents)?;
                Ok(())
            }

            pub fn read(&mut self, full_path: &[OsString]) -> VfsResult<Vec<u8>> {
                let joined: OsString = full_path.join("/".as_ref());
                let path = PathBuf::from(joined);
                let mut data = vec![];
                std::fs::File::open(path)?.read(&mut data)?;
                Ok(data)
            }

            pub fn create_dir_all(&mut self, full_path: &InnerPath) -> VfsResult<()> {
                Ok(std::fs::create_dir_all(full_path.into_path())?)
            }
        }
    }
}

pub enum Fs {
    Memory(fs::memory::MemoryFilesystem),
    Physical(fs::physical::PhysicalFilesystem),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InnerPath {
    parts: Vec<OsString>,
}

impl InnerPath {
    fn new(parts: Vec<OsString>) -> Self {
        Self { parts }
    }

    pub fn into_path(&self) -> PathBuf {
        let joined: OsString = self.parts.join("/".as_ref());
        PathBuf::from(joined)
    }
}

impl From<InnerPath> for PathBuf {
    fn from(path: InnerPath) -> Self {
        path.into_path()
    }
}

impl Display for InnerPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut path = OsString::from("/");
        path.push(self.parts.join(&OsString::from("/")));
        f.write_str(&String::from_utf8_lossy(path.as_encoded_bytes()))
    }
}

#[derive(Clone)]
pub struct VfsPath {
    path: InnerPath,
    fs: Arc<Mutex<Fs>>,
}

impl VfsPath {
    fn create_in_fs(fs: Fs) -> VfsPath {
        VfsPath {
            path: InnerPath::default(),
            fs: Arc::new(Mutex::new(fs)),
        }
    }

    pub fn fs(&self) -> Arc<Mutex<Fs>> {
        self.fs.clone()
    }

    pub fn join(&self, p: impl AsRef<OsStr>) -> VfsPath {
        let mut vfs_path = self.clone();
        vfs_path.path.parts.push(p.as_ref().into());
        vfs_path
    }

    pub fn read(&self) -> VfsResult<Vec<u8>> {
        let mut fs = self.fs.lock()?;
        match &mut *fs {
            Fs::Memory(fs) => fs.read(&self.path.parts),
            Fs::Physical(fs) => fs.read(&self.path.parts),
        }
    }

    pub fn write(&self, contents: Vec<u8>) -> VfsResult<()> {
        let mut fs = self.fs.lock()?;
        match &mut *fs {
            Fs::Memory(fs) => fs.write(&self.path.parts, contents),
            Fs::Physical(fs) => fs.write(&self.path.parts, contents),
        }
    }

    pub fn create_dir_all(&self) -> VfsResult<()> {
        let mut fs = self.fs.lock()?;
        match &mut *fs {
            Fs::Memory(fs) => fs.create_dir_all(&self.path),
            Fs::Physical(fs) => fs.create_dir_all(&self.path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{VfsResult, fs::memory::MemoryFilesystem};

    #[test]
    fn in_memory_read_write() -> VfsResult<()> {
        let fs = MemoryFilesystem::new();
        fs.join("hi.txt").write(b"stuff".into()).unwrap();
        let contents = String::from_utf8(fs.join("hi.txt").read().unwrap()).unwrap();
        assert_eq!(contents, "stuff");
        Ok(())
    }

    #[test]
    fn create_dir_all() -> VfsResult<()> {
        let fs = MemoryFilesystem::new();
        fs.join("dir").create_dir_all().unwrap();
        fs.join("hi").write(b"stuff".into()).unwrap();
        assert_eq!(
            fs.join("hi").create_dir_all().unwrap_err().to_string(),
            "Path is a file: `/hi`"
        );
        Ok(())
    }

    #[test]
    fn error_read_root() -> VfsResult<()> {
        let fs = MemoryFilesystem::new();
        let err = fs.read().unwrap_err();
        assert_eq!(err.to_string(), "Path is a directory: `/`");
        Ok(())
    }

    #[test]
    fn create_directory_at_root() -> VfsResult<()> {
        MemoryFilesystem::new().create_dir_all()
    }
}
