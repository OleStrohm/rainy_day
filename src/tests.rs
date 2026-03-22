use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use eyre::Result;
use rand::{SeedableRng, rngs::SmallRng};
use sha2::{Digest, Sha256};
use vfs::{MemoryFS, VfsPath, VfsResult};

use crate::{client::Client, store::Store, utils::ReadWriteOnVfsPath};

#[test]
// r[verify store.init]
fn init() {
    let fs = MemoryFS::new();
    let root: VfsPath = fs.into();
    let store_path = root.join("store").unwrap();

    let _store = Store::with_salt_rng(store_path.clone(), SmallRng::from_seed([0; _])).unwrap();

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
    let fs = MemoryFS::new();
    let root: VfsPath = fs.into();
    let store_path = root.join("store").unwrap();
    let client_path = root.join("client").unwrap();

    let relative_file_path = PathBuf::from("first.txt");
    let contents = b"Very important notes";
    let test_path = client_path.join(relative_file_path.to_str().unwrap())?;

    let store = Arc::new(Mutex::new(Store::with_salt_rng(
        store_path.clone(),
        SmallRng::from_seed([0; _]),
    )?));
    let client = Client::connect(client_path, b"password".into(), store)?;

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
    let fs = MemoryFS::new();
    let root: VfsPath = fs.into();
    let store_path = root.join("store")?;
    let client_path = root.join("client")?;

    let relative_file_path = PathBuf::from("first.txt");
    let contents = b"Very important notes";
    let test_path = client_path.join(relative_file_path.to_str().unwrap())?;

    let store = Arc::new(Mutex::new(Store::with_salt_rng(
        store_path.clone(),
        SmallRng::from_seed([0; _]),
    )?));
    let client = Client::connect(client_path, b"password".into(), store)?;

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
