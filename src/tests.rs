use std::{
    path::PathBuf,
    sync::mpsc,
    thread::{self, JoinHandle},
};

use eyre::Result;
use sha2::{Digest, Sha256};
use vfs::{MemoryFS, VfsPath, VfsResult};

use crate::{
    client::Client,
    store::{Store, StoreControl},
    utils::ReadWriteOnVfsPath,
};

pub fn store() -> (mpsc::Sender<StoreControl>, JoinHandle<()>) {
    let store_path: VfsPath = MemoryFS::new().into();

    let (store, sender) = Store::init(store_path.clone()).unwrap();
    let handle = thread::spawn(move || store.run());
    (sender, handle)
}

pub fn client() -> (
    mpsc::Sender<MessageToStore>,
    mpsc::Receiver<MessageToClient>,
) {
    let (store_sender, client_receiver) = mpsc::channel();
    let (client_sender, store_receiver) = mpsc::channel();

    (client_sender, client_receiver)
}

#[test]
// r[verify store.init]
fn init() {
    let store_path: VfsPath = MemoryFS::new().into();

    let _store = Store::init(store_path.clone()).unwrap();

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
// r[verify retrieve.by.path]
// r[verify insert.file]
fn insert_and_retrieve() -> Result<()> {
    let store_path: VfsPath = MemoryFS::new().into();
    let client_path: VfsPath = MemoryFS::new().into();

    let relative_file_path = PathBuf::from("first.txt");
    let contents = b"Very important notes";
    let test_path = client_path.join(relative_file_path.to_str().unwrap())?;

    let store = Store::init(store_path.clone())?;
    let client = Client::new(client_path, b"password".into(), store.connect())?;

    test_path.write(contents)?;

    client.insert_to_store(&relative_file_path)?;
    test_path.remove_file()?;
    client.retrieve_from_store(&relative_file_path)?;

    let retrieved_contents = test_path.read()?;

    assert_eq!(contents.as_slice(), retrieved_contents);

    Ok(())
}

#[test]
fn insert_check_store() -> Result<()> {
    let store_path: VfsPath = MemoryFS::new().into();
    let client_path: VfsPath = MemoryFS::new().into();

    let relative_file_path = PathBuf::from("first.txt");
    let contents = b"Very important notes";
    let test_path = client_path.join(relative_file_path.to_str().unwrap())?;

    let store = Store::init(store_path.clone())?;
    let client = Client::new(client_path, b"password".into(), store.connect())?;

    test_path.write(contents)?;

    client.insert_to_store(&relative_file_path)?;
    let hash = hex::encode(Sha256::digest(
        relative_file_path.as_os_str().as_encoded_bytes(),
    ));

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

    Ok(())
}

#[test]
fn sync_to_client() -> Result<()> {
    let store_path: VfsPath = MemoryFS::new().into();

    let relative_file_path = PathBuf::from("first.txt");
    let contents = b"Very important notes";

    let store = Store::init(store_path.clone())?;

    {
        // First client
        let client_path: VfsPath = MemoryFS::new().into();
        let test_path = client_path.join(relative_file_path.to_str().unwrap())?;

        test_path.write(contents)?;

        let client = Client::new(client_path, b"password".into(), store.connect())?;
        client.insert_to_store(&relative_file_path)?;
    }

    // Second client
    let client_path: VfsPath = MemoryFS::new().into();
    let test_path = client_path.join(relative_file_path.to_str().unwrap())?;
    let client = Client::new(client_path.clone(), b"password".into(), store.connect())?;
    client.sync_from_store()?;

    assert_eq!(test_path.read()?, contents);

    Ok(())
}

#[test]
// r[verify retrieve.by.path]
// r[verify insert.file]
fn insert_and_retrieve_in_directory() -> Result<()> {
    let store_path: VfsPath = MemoryFS::new().into();
    let client_path: VfsPath = MemoryFS::new().into();

    let relative_file_path = PathBuf::from("path/to/file.txt");
    let contents = b"Very important notes";
    let test_path = client_path.join(relative_file_path.to_str().unwrap())?;

    let store = Store::init(store_path.clone())?;
    let client = Client::new(client_path, b"password".into(), store.connect())?;

    test_path.write(contents)?;

    client.insert_to_store(&relative_file_path)?;
    test_path.remove_file()?;
    client.retrieve_from_store(&relative_file_path)?;

    let retrieved_contents = test_path.read()?;

    assert_eq!(contents.as_slice(), retrieved_contents);

    Ok(())
}

#[test]
fn sync_both_ways() -> Result<()> {
    let store_path: VfsPath = MemoryFS::new().into();

    let paths = ["first.txt", "hello/second.txt", "hello/third.txt"];
    let contents = b"Very important notes";

    let store = Store::init(store_path.clone())?;

    {
        // First client
        let client_path: VfsPath = MemoryFS::new().into();
        let client = Client::new(client_path.clone(), b"password".into(), store.connect())?;
        for path in &paths {
            client_path.join(path)?.write(contents)?;
        }
        client.sync_to_store()?;
    }

    // Second client
    let client_path: VfsPath = MemoryFS::new().into();
    let client = Client::new(client_path.clone(), b"password".into(), store.connect())?;
    client.sync_from_store()?;

    for path in &paths {
        assert_eq!(client_path.join(path)?.read()?, contents);
    }

    Ok(())
}
