use details::rpc::Error as RpcError;

/// Service error
#[derive(Debug, Error)]
pub enum Error {
    /// Rpc error
    Rpc(RpcError),
    /// Insufficient funds to create anchoring transaction
    InsufficientFunds,
}
