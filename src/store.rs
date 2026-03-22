use eyre::Result;
use rand::{Rng, RngExt, rngs::ThreadRng};
use serde::{Deserialize, Serialize};
use vfs::VfsPath;

use crate::utils::ReadWriteOnVfsPath;

#[derive(Serialize, Deserialize)]
pub struct FileMetadata {
    pub salt: String,
}

pub struct Store {
    root: VfsPath,
    salt_rng: Box<dyn Rng>,
}

impl Store {
    // r[depends store.init]
    pub fn init(root: VfsPath) -> Result<Store> {
        Self::with_salt_rng(root, ThreadRng::default())
    }

    // r[impl store.init]
    pub fn with_salt_rng(root: VfsPath, salt_rng: impl Rng + 'static) -> Result<Store> {
        root.remove_dir_all()?;
        root.create_dir_all()?;
        root.join("rainy_day")?
            .create_file()?
            .write(b"Just in case")?;

        Ok(Store {
            root,
            salt_rng: Box::new(salt_rng) as _,
        })
    }

    pub fn generate_salt(&mut self) -> [u8; 64] {
        self.salt_rng.random()
    }

    // r[impl insert.file]
    pub fn insert_file(
        &self,
        contents: Vec<u8>,
        salt: [u8; 64],
        hashed_file_path: impl AsRef<str>,
        // r[depends encryption.client]
        encrypted_file_path: Vec<u8>,
    ) -> Result<()> {
        let file_path = self.root.join(hashed_file_path)?;

        println!("Creating {:?}", file_path);
        file_path.create_dir()?;

        let metadata = FileMetadata {
            salt: hex::encode(salt),
        };

        let metadata_path = file_path.join("metadata.json")?;
        println!("Creating {metadata_path:?}");
        metadata_path.write(serde_json::to_string_pretty(&metadata)?.as_bytes())?;

        let contents_path = file_path.join("contents")?;
        println!("Creating {contents_path:?}");
        contents_path.write(&contents)?;

        let path_path = file_path.join("path")?;
        println!("Creating {path_path:?}");
        path_path.write(&encrypted_file_path)?;

        Ok(())
    }

    // r[impl retrieve.by.path]
    pub fn retrieve(&self, hashed_file_path: impl AsRef<str>) -> Result<(Vec<u8>, FileMetadata)> {
        let file_path = self.root.join(hashed_file_path)?;

        let metadata_path = file_path.join("metadata.json")?;

        println!("Reading from {metadata_path:?}");
        let metadata = serde_json::from_slice::<FileMetadata>(&metadata_path.read()?)?;

        let contents_path = file_path.join("contents")?;
        println!("Reading from {contents_path:?}");
        let encrypted_contents = contents_path.read()?;

        Ok((encrypted_contents, metadata))
    }
}
