use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::mpsc,
};

use aes_gcm::{
    Aes256Gcm, Key, KeyInit,
    aead::{Aead, Nonce},
};
use eyre::Result;
use pbkdf2::pbkdf2_hmac_array;
use rand::{Rng, SeedableRng, rngs::ChaCha20Rng};
use sha2::{Digest, Sha256};
use vfs::VfsPath;

use crate::{
    shared::{MessageToClient, MessageToStore},
    utils::ReadWriteOnVfsPath,
};

pub struct Client {
    root: VfsPath,
    send: mpsc::Sender<MessageToStore>,
    recv: mpsc::Receiver<MessageToClient>,
    password: Vec<u8>,
    rounds: u32,
}

impl Client {
    /// Creates a client connected to a specific store
    pub fn new(
        root: VfsPath,
        password: Vec<u8>,
        (send, recv): (
            mpsc::Sender<MessageToStore>,
            mpsc::Receiver<MessageToClient>,
        ),
    ) -> Result<Client> {
        root.create_dir_all()?;
        Ok(Client {
            root,
            send,
            recv,
            password,
            #[cfg(test)]
            rounds: 6000,
            #[cfg(not(test))]
            rounds: 6000_000,
        })
    }

    /// Inserts a file to the store
    pub(crate) fn insert_to_store(&self, relative_file_path: &Path) -> Result<()> {
        let contents = self
            .root
            .join(relative_file_path.as_os_str().to_str().unwrap())?
            .read()?;

        //let salt = store.generate_salt();
        let hashed_file_path = Sha256::digest(relative_file_path.as_os_str().as_encoded_bytes());

        let key = pbkdf2_hmac_array::<Sha256, 32>(&self.password, &hashed_file_path, self.rounds);
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

        self.send.send(MessageToStore::Insert {
            encrypted_contents,
            hashed_file_path: hex::encode(hashed_file_path),
            encrypted_file_path,
        })?;

        match self.recv.recv()? {
            MessageToClient::Inserted { .. } => Ok(()),
            _ => unimplemented!("Cannot handle out of order messages"),
        }
    }

    /// Inserts a file to the store
    pub(crate) fn retrieve_from_store(&self, relative_file_path: &Path) -> Result<()> {
        let hashed_file_path = Sha256::digest(relative_file_path.as_os_str().as_encoded_bytes());
        self.send.send(MessageToStore::Retrieve {
            hashed_file_path: hex::encode(hashed_file_path),
        })?;
        let (encrypted_contents, _metadata) = match self.recv.recv()? {
            MessageToClient::Retrieved {
                contents, metadata, ..
            } => (contents, metadata),
            _ => unimplemented!("Cannot handle out of order messages"),
        };

        let key = pbkdf2_hmac_array::<Sha256, 32>(&self.password, &hashed_file_path, self.rounds);
        let mut rng = ChaCha20Rng::from_seed(key);

        let key = Key::<Aes256Gcm>::from_slice(&key);
        // r[impl encrypt.aes]
        let cipher = Aes256Gcm::new(&key);

        let mut nonce = Nonce::<Aes256Gcm>::default();
        rng.fill_bytes(&mut nonce);

        // r[impl decrypt.contents]
        let contents = cipher.decrypt(&nonce, encrypted_contents.as_ref())?;

        self.root
            .join(relative_file_path.to_str().unwrap())?
            .write(&contents)?;

        Ok(())
    }

    fn decrypt_file_path(
        &self,
        hashed_file_path: String,
        encrypted_file_path: &[u8],
    ) -> Result<PathBuf> {
        let hashed_file_path = hex::decode(hashed_file_path)?;

        let key = pbkdf2_hmac_array::<Sha256, 32>(&self.password, &hashed_file_path, self.rounds);
        let mut rng = ChaCha20Rng::from_seed(key);

        let key = Key::<Aes256Gcm>::from_slice(&key);
        let cipher = Aes256Gcm::new(&key);

        let mut nonce = Nonce::<Aes256Gcm>::default();
        rng.fill_bytes(&mut nonce);
        let mut nonce = Nonce::<Aes256Gcm>::default();
        rng.fill_bytes(&mut nonce);

        let decrypted_file_path = cipher.decrypt(&nonce, encrypted_file_path)?;
        Ok(PathBuf::from(unsafe {
            OsString::from_encoded_bytes_unchecked(decrypted_file_path)
        }))
    }

    pub fn sync_from_store(&self) -> Result<()> {
        self.send.send(MessageToStore::RetrieveAll)?;
        loop {
            match self.recv.recv()? {
                MessageToClient::Retrieved { .. } => unimplemented!(),
                MessageToClient::Inserted { .. } => unimplemented!(),
                MessageToClient::RetrievedAllFiles { files } => {
                    for (hashed_file_path, encrypted_file_path) in files {
                        let relative_file_path =
                            self.decrypt_file_path(hashed_file_path, &encrypted_file_path)?;
                        self.retrieve_from_store(&relative_file_path)?;
                    }
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn sync_to_store(&self) -> Result<()> {
        let file_paths = self
            .root
            .walk_dir()?
            .filter(|p| {
                p.as_ref()
                    .ok()
                    .and_then(|p| p.is_file().ok())
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();

        for path in file_paths {
            self.insert_to_store(Path::new(path?.as_str()))?;
        }

        Ok(())
    }
}
