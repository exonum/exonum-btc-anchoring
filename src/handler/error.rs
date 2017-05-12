use std::fmt;
use std::error;

use details::btc::transactions::BitcoinTx;

#[derive(Debug, PartialEq)]
pub enum Error {
    IncorrectLect { reason: String, tx: BitcoinTx },
    LectNotFound { height: u64 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IncorrectLect { ref reason, ref tx } => {
                write!(f, "Incorrect lect: {}, tx={:#?}", reason, tx)
            }
            Error::LectNotFound { height } => {
                write!(f, "Suitable lect not found for height={}", height)
            }
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::IncorrectLect { .. } => "Incorrect lect",
            Error::LectNotFound { .. } => "Suitable lect not found",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}
