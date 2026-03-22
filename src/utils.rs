use vfs::{VfsPath, VfsResult};

pub trait ReadWriteOnVfsPath {
    fn read(&self) -> VfsResult<Vec<u8>>;
    fn write(&self, data: &[u8]) -> VfsResult<usize>;
}

impl ReadWriteOnVfsPath for VfsPath {
    fn read(&self) -> VfsResult<Vec<u8>> {
        let mut contents_vec = vec![];
        self.open_file()?.read_to_end(&mut contents_vec)?;
        Ok(contents_vec)
    }

    fn write(&self, data: &[u8]) -> VfsResult<usize> {
        Ok(self.create_file()?.write(data)?)
    }
}
