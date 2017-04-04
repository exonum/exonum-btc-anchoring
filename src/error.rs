pub use exonum::storage::Error as StorageError;
pub use details::error::Error as InternalError;
use bitcoinrpc::Error as RpcError;

/// An anchoring btc service Error type
#[derive(Debug, Error)]
pub enum Error {
    /// Storage error
    Storage(StorageError),
    /// An internal error
    Internal(InternalError),
}

impl From<RpcError> for Error {
    fn from(err: RpcError) -> Error {
        Error::Internal(InternalError::Rpc(err))
    }
}
