use exonum::storage::Error as StorageError;
use client::Error as RpcError;

#[derive(Debug)]
pub enum Error {
    Storage(StorageError),
    Rpc(RpcError),
}

impl From<StorageError> for Error {
    fn from(e: StorageError) -> Error {
        Error::Storage(e)
    }
}

impl From<RpcError> for Error {
    fn from(e: RpcError) -> Error {
        Error::Rpc(e)
    }
}