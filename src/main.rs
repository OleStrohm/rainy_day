use std::{
    error::Error,
    path::{Path, PathBuf},
};

use aes_gcm::{
    Aes256Gcm, Key, KeyInit,
    aead::{Aead, Nonce},
};
use clap::{Parser, Subcommand};
use pbkdf2::pbkdf2_hmac_array;
use rand::{Rng, RngExt, SeedableRng, rngs::ChaCha20Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vfs::{PhysicalFS, VfsPath, VfsResult};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Insert {
        relative_file_path: PathBuf,
    },
    Retrieve {
        relative_file_path: PathBuf,
        output: String,
    },
}

fn handle_command(
    command: Commands,
    store_path: VfsPath,
    salt_rng: &mut impl Rng,
) -> Result<(), Box<dyn Error>> {
    let password = b"my_super_secure_password";
    let rounds = 600_000;

    match command {
        Commands::Init => {
            store_path.remove_dir_all()?;
            store_path.create_dir_all()?;
            store_path
                .join("rainy_day")?
                .create_file()?
                .write(b"Just in case")?;
        }
        Commands::Insert { relative_file_path } => {
            let contents = store_path
                .join(relative_file_path.as_os_str().to_string_lossy())?
                .read()?;

            let salt = salt_rng.random::<[u8; 64]>();

            let key = pbkdf2_hmac_array::<Sha256, 32>(password, &salt, rounds);
            let mut rng = ChaCha20Rng::from_seed(key);

            let key = Key::<Aes256Gcm>::from_slice(&key);
            let cipher = Aes256Gcm::new(&key);

            let file_path_sha = Sha256::digest(relative_file_path.as_os_str().as_encoded_bytes());
            let file_path = store_path.join(hex::encode(file_path_sha))?;

            println!("Creating {:?}", file_path);
            file_path.create_dir()?;

            let metadata = FileMetadata {
                salt: hex::encode(salt),
            };

            let metadata_path = file_path.join("metadata.json")?;
            println!("Creating {metadata_path:?}");
            metadata_path.write(serde_json::to_string_pretty(&metadata)?.as_bytes())?;

            let contents_path = file_path.join("contents")?;
            let mut nonce = Nonce::<Aes256Gcm>::default();
            rng.fill_bytes(&mut nonce);
            let contents = cipher.encrypt(&nonce, contents.as_ref())?;
            println!("Creating {contents_path:?}");
            contents_path.write(&contents)?;

            let mut nonce = Nonce::<Aes256Gcm>::default();
            rng.fill_bytes(&mut nonce);
            let encrypted_file_path =
                cipher.encrypt(&nonce, relative_file_path.as_os_str().as_encoded_bytes())?;
            let path_path = file_path.join("path")?;
            let mut nonce = Nonce::<Aes256Gcm>::default();
            rng.fill_bytes(&mut nonce);
            println!("Creating {path_path:?}");
            path_path.write(&encrypted_file_path)?;
        }
        Commands::Retrieve {
            relative_file_path: file_path,
            output,
        } => {
            let file_path_sha = Sha256::digest(file_path.as_os_str().as_encoded_bytes());
            let file_path = store_path.join(hex::encode(file_path_sha))?;

            let metadata_path = store_path.join(file_path.as_str())?.join("metadata.json")?;

            println!("Reading from {metadata_path:?}");
            let metadata = serde_json::from_slice::<FileMetadata>(&metadata_path.read()?)?;
            let salt = hex::decode(metadata.salt)?;

            let key = pbkdf2_hmac_array::<Sha256, 32>(password, &salt, rounds);
            let mut rng = ChaCha20Rng::from_seed(key);

            let key = Key::<Aes256Gcm>::from_slice(&key);
            let cipher = Aes256Gcm::new(&key);

            let contents_path = store_path.join(file_path.as_str())?.join("contents")?;
            let mut nonce = Nonce::<Aes256Gcm>::default();
            rng.fill_bytes(&mut nonce);

            println!("Reading from {contents_path:?}");
            let contents = cipher.decrypt(&nonce, contents_path.read()?.as_ref())?;
            println!(
                "Contents of file:\n{}",
                String::from_utf8(contents.clone())?
            );

            store_path.join(output)?.write(contents.as_ref())?;
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let store_path = PhysicalFS::new("/tmp/cloud-store").into();
    let cli = Cli::parse();
    let mut rng = rand::rng();

    handle_command(cli.command, store_path, &mut rng)
}

#[derive(Serialize, Deserialize)]
struct FileMetadata {
    salt: String,
}

trait ReadWriteOnVfsPath {
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::SmallRng;
    use vfs::{MemoryFS, VfsResult};

    #[test]
    fn init() {
        let fs = MemoryFS::new();
        let root: VfsPath = fs.into();
        let store_path = root.join("store").unwrap();

        handle_command(
            Commands::Init,
            store_path.clone(),
            &mut SmallRng::from_seed([0; _]),
        )
        .unwrap();

        let mut directories = store_path
            .walk_dir()
            .unwrap()
            .collect::<VfsResult<Vec<_>>>()
            .unwrap();

        directories.sort_by_key(|path| path.as_str().to_string());
        let expected = vec!["rainy_day"]
            .iter()
            .map(|path| store_path.join(path).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(expected, directories);
    }

    #[test]
    fn insert_and_retrieve() {
        let fs = MemoryFS::new();
        let root: VfsPath = fs.into();
        let store_path = root.join("store").unwrap();
        let mut rng = SmallRng::from_seed([0; _]);

        let contents = b"Very important notes".to_vec();

        handle_command(Commands::Init, store_path.clone(), &mut rng).unwrap();
        let relative_file_path = PathBuf::from("first.txt");
        store_path
            .join(relative_file_path.to_string_lossy())
            .unwrap()
            .write(&contents)
            .unwrap();

        handle_command(
            Commands::Insert {
                relative_file_path: relative_file_path.clone(),
            },
            store_path.clone(),
            &mut rng,
        )
        .unwrap();

        let hash = "aab97556cecaa26d836e8f909d66208e47f10f98842760b68016079475cac8d7";

        let test_dir = store_path.join(hash).unwrap();

        let mut directories = test_dir
            .walk_dir()
            .unwrap()
            .collect::<VfsResult<Vec<_>>>()
            .unwrap();

        directories.sort_by_key(|path| path.as_str().to_string());
        let expected = vec!["contents", "metadata.json", "path"]
            .iter()
            .map(|path| test_dir.join(path).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(expected, directories);

        handle_command(
            Commands::Retrieve {
                relative_file_path,
                output: "retrieved_file".into(),
            },
            store_path.clone(),
            &mut rng,
        )
        .unwrap();

        let retrieved_contents = store_path.join("retrieved_file").unwrap().read().unwrap();
        assert_eq!(contents, retrieved_contents);
    }
}
