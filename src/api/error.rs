use exonum::storage::Error as StorageError;
use blockchain_explorer::api::ApiError;

#[derive(Debug, Error)]
pub enum Error {
    Storage(StorageError),
    UnknownValidatorId,
}

impl Into<ApiError> for Error {
    fn into(self) -> ApiError {
        match self {
            Error::Storage(e) => ApiError::Storage(e),
            // FIXME add suitable error type
            Error::UnknownValidatorId => {
                ApiError::Storage(StorageError::new("Unknown validator id"))
            }
        }
    }
}
