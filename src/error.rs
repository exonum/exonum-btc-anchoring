pub use exonum::storage::Error as StorageError;
pub use client::Error as RpcError;

/// A Service error
#[derive(Debug, Error)]
pub enum Error {
    /// Storage error
    Storage(StorageError),
    /// Rpc error
    Rpc(RpcError),
    /// Insufficient funds to create anchoring transaction
    InsufficientFunds,
}
