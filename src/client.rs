use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use aes_gcm::{
    Aes256Gcm, Key, KeyInit,
    aead::{Aead, Nonce},
};
use eyre::{Result, eyre};
use pbkdf2::pbkdf2_hmac_array;
use rand::{Rng, SeedableRng, rngs::ChaCha20Rng};
use sha2::{Digest, Sha256};
use vfs::VfsPath;

use crate::{store::Store, utils::ReadWriteOnVfsPath};

pub struct Client {
    root: VfsPath,
    store: Arc<Mutex<Store>>,
    password: Vec<u8>,
    rounds: u32,
}

impl Client {
    /// Creates a client connected to a specific store
    pub fn connect(root: VfsPath, password: Vec<u8>, store: Arc<Mutex<Store>>) -> Result<Client> {
        root.create_dir_all()?;
        Ok(Client {
            root,
            store,
            password,
            #[cfg(test)]
            rounds: 6000,
            #[cfg(not(test))]
            rounds: 6000_000,
        })
    }

    /// Inserts a file to the store
    pub fn insert_to_store(&self, relative_file_path: &Path) -> Result<()> {
        let contents = self
            .root
            .join(relative_file_path.as_os_str().to_str().unwrap())?
            .read()?;
        let mut store = self
            .store
            .lock()
            .map_err(|e| eyre!("Mutex Error: {}", e.to_string()))?;

        let salt = store.generate_salt();

        let key = pbkdf2_hmac_array::<Sha256, 32>(&self.password, &salt, self.rounds);
        let mut rng = ChaCha20Rng::from_seed(key);

        let key = Key::<Aes256Gcm>::from_slice(&key);
        let cipher = Aes256Gcm::new(&key);

        let mut nonce = Nonce::<Aes256Gcm>::default();
        rng.fill_bytes(&mut nonce);
        // r[impl encrypt.contents]
        let encrypted_contents = cipher.encrypt(&nonce, contents.as_ref())?;
        let mut nonce = Nonce::<Aes256Gcm>::default();
        rng.fill_bytes(&mut nonce);

        // r[impl encrypt.path]
        let encrypted_file_path =
            cipher.encrypt(&nonce, relative_file_path.as_os_str().as_encoded_bytes())?;

        let hashed_file_path = hex::encode(Sha256::digest(
            relative_file_path.as_os_str().as_encoded_bytes(),
        ));

        store.insert_file(
            encrypted_contents,
            salt,
            hashed_file_path,
            encrypted_file_path,
        )
    }

    /// Inserts a file to the store
    pub fn retrieve_from_store(&self, relative_file_path: &Path) -> Result<()> {
        let store = self
            .store
            .lock()
            .map_err(|e| eyre!("Mutex Error: {}", e.to_string()))?;
        let hashed_file_path = hex::encode(Sha256::digest(
            relative_file_path.as_os_str().as_encoded_bytes(),
        ));
        let (encrypted_contents, metadata) = store.retrieve(hashed_file_path)?;
        let salt = hex::decode(metadata.salt)?;

        let key = pbkdf2_hmac_array::<Sha256, 32>(&self.password, &salt, self.rounds);
        let mut rng = ChaCha20Rng::from_seed(key);

        let key = Key::<Aes256Gcm>::from_slice(&key);
        // r[impl encrypt.aes]
        let cipher = Aes256Gcm::new(&key);

        let mut nonce = Nonce::<Aes256Gcm>::default();
        rng.fill_bytes(&mut nonce);

        // r[impl decrypt.contents]
        let contents = cipher.decrypt(&nonce, encrypted_contents.as_ref())?;
        println!(
            "Contents of file:\n{}",
            String::from_utf8(contents.clone())?
        );

        self.root
            .join(relative_file_path.to_str().unwrap())?
            .write(&contents)?;

        Ok(())
    }
}
