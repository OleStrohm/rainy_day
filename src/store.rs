use std::sync::mpsc;

use eyre::Result;
use serde::{Deserialize, Serialize};
use vfs::VfsPath;

use crate::{
    shared::{MessageToClient, MessageToStore},
    utils::ReadWriteOnVfsPath,
};

#[derive(Serialize, Deserialize)]
pub struct FileMetadata {
    pub salt: String,
}

struct ClientConnection {
    send: mpsc::Sender<MessageToClient>,
    recv: mpsc::Receiver<MessageToStore>,
}

pub enum StoreControl {
    Shutdown,
    NewClient(
        mpsc::Sender<MessageToClient>,
        mpsc::Receiver<MessageToStore>,
    ),
}

pub struct Store {
    root: VfsPath,
    clients: Vec<ClientConnection>,
    control: mpsc::Receiver<StoreControl>,
}

impl Store {
    pub fn init(root: VfsPath) -> Result<(Store, mpsc::Sender<StoreControl>)> {
        root.create_dir_all()?;
        if !root.join("rainy_day")?.exists()? {
            root.join("rainy_day")?
                .create_file()?
                .write(b"Just in case")?;
        }

        let (sender, receiver) = mpsc::channel();

        let store = Store {
            root,
            clients: vec![],
            control: receiver,
        };

        Ok((store, sender))
    }

    pub fn run(mut self) {
        loop {
            if let Ok(msg) = self.control.try_recv() {
                match msg {
                    StoreControl::Shutdown => return,
                    StoreControl::NewClient(sender, receiver) => {
                        self.clients.push(ClientConnection {
                            send: sender,
                            recv: receiver,
                        });
                    }
                }
            }

            for client in &self.clients {
                if let Ok(msg) = client.recv.try_recv() {
                    match msg {
                        MessageToStore::Insert {
                            encrypted_contents,
                            hashed_file_path,
                            encrypted_file_path,
                        } => {
                            self.insert_file(
                                encrypted_contents,
                                hashed_file_path.clone(),
                                encrypted_file_path,
                            )
                            .unwrap();
                            client
                                .send
                                .send(MessageToClient::Inserted { hashed_file_path })
                                .unwrap();
                        }
                        MessageToStore::Retrieve { hashed_file_path } => {
                            let (contents, metadata) =
                                self.retrieve(hashed_file_path.clone()).unwrap();

                            client
                                .send
                                .send(MessageToClient::Retrieved {
                                    hashed_file_path,
                                    contents,
                                    metadata,
                                })
                                .unwrap();
                        }
                        MessageToStore::RetrieveAll => {
                            let files = self
                                .root
                                .read_dir()
                                .unwrap()
                                .flat_map(|d| -> Result<_> {
                                    Ok((d.filename(), d.join("path")?.read()?))
                                })
                                .collect::<Vec<_>>();

                            client
                                .send
                                .send(MessageToClient::RetrievedAllFiles { files })
                                .unwrap();
                        }
                    }
                }
            }
        }
    }

    // r[impl insert.file]
    pub fn insert_file(
        &self,
        contents: Vec<u8>,
        hashed_file_path: impl AsRef<str>,
        // r[depends encryption.client]
        encrypted_file_path: Vec<u8>,
    ) -> Result<()> {
        let file_path = self.root.join(hashed_file_path)?;
        file_path.create_dir()?;

        let metadata = FileMetadata {
            salt: String::new(),
        };
        file_path
            .join("metadata.json")?
            .write(serde_json::to_string_pretty(&metadata)?.as_bytes())?;
        file_path.join("contents")?.write(&contents)?;
        file_path.join("path")?.write(&encrypted_file_path)?;

        Ok(())
    }

    // r[impl retrieve.by.path]
    pub fn retrieve(&self, hashed_file_path: impl AsRef<str>) -> Result<(Vec<u8>, FileMetadata)> {
        let file_path = self.root.join(hashed_file_path)?;

        let metadata =
            serde_json::from_slice::<FileMetadata>(&file_path.join("metadata.json")?.read()?)?;
        let encrypted_contents = file_path.join("contents")?.read()?;

        Ok((encrypted_contents, metadata))
    }
}
