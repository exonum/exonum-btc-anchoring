pub use exonum::storage::Error as StorageError;
pub use client::Error as RpcError;

#[derive(Debug, Error)]
pub enum Error {
    Storage(StorageError),
    Rpc(RpcError),
    InsufficientFunds,
}
