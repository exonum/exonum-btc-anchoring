use std::error;
use std::fmt;

use exonum::storage::Error as StorageError;
use exonum::api::ApiError;

#[derive(Debug)]
pub enum Error {
    UnknownValidatorId(u32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::UnknownValidatorId(id) => write!(f, "Unknown validator id={}", id),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::UnknownValidatorId(_) => "UnknownValidatorId",
        }
    }
}

impl Into<ApiError> for Error {
    fn into(self) -> ApiError {
        match self {
            Error::UnknownValidatorId(id) => {
                ApiError::Storage(StorageError::new(format!("Unknown validator id={}", id)))
            }
        }
    }
}
