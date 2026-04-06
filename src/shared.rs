use crate::store::FileMetadata;


pub enum MessageToStore {
    Insert {
        encrypted_contents: Vec<u8>,
        hashed_file_path: String,
        encrypted_file_path: Vec<u8>,
    },
    Retrieve {
        hashed_file_path: String,
    },
    RetrieveAll,
}
pub enum MessageToClient {
    Retrieved {
        hashed_file_path: String,
        contents: Vec<u8>,
        metadata: FileMetadata,
    },
    Inserted {
        hashed_file_path: String,
    },
    RetrievedAllFiles {
        files: Vec<(String, Vec<u8>)>,
    }
}
