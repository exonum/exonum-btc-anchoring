use std::fmt;
use std::error;

use details::btc::transactions::BitcoinTx;

#[derive(Debug)]
pub enum Error {
    IncorrectLect { reason: String, tx: BitcoinTx },
    LectNotFound,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IncorrectLect { ref reason, ref tx } => {
                write!(f, "Incorrect lect: {} tx={:#?}", reason, tx)
            }
            Error::LectNotFound => write!(f, "Lect not found"),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::IncorrectLect { .. } => "Incorrect lect",
            Error::LectNotFound => "Lect not found",
        }
    }

    fn cause(&self) -> Option<&::std::error::Error> {
        None
    }
}
