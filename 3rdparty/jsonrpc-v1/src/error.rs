// Rust JSON-RPC 1.0 Library
// Written in 2015 by
//     Andrew Poelstra <apoelstra@wpsoftware.net>
//
// Modified in 2016 by
//     Jean Pierre De Jesus Dudey Diaz <jeandudey@hotmail.com>
//
// Modified in 2016 by
//     Aleksey Sidorov <aleksei.sidorov@xdev.re>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the CC0 Public Domain Dedication
// along with this software.
// If not, see <http://creativecommons.org/publicdomain/zero/1.0/>.
//

//! # Error handling
//!
//! Some useful methods for creating Error objects
//!

use std::{error, fmt};

use hyper;
use serde_json;
use serde_json::Value;

/// A library error
#[derive(Debug)]
pub enum Error {
    /// Json error
    Json(serde_json::Error),
    /// Client error
    Hyper(hyper::error::Error),
    /// Rpc error,
    Rpc(Value),
    /// Response has neither error nor result
    NoErrorOrResult,
    /// Response to a request did not have the expected nonce
    NonceMismatch,
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::Json(e)
    }
}

impl From<hyper::error::Error> for Error {
    fn from(e: hyper::error::Error) -> Error {
        Error::Hyper(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Json(ref e) => write!(f, "JSON decode error: {}", e),
            Error::Hyper(ref e) => write!(f, "Hyper error: {}", e),
            _ => f.write_str(error::Error::description(self)),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Json(_) => "JSON decode error",
            Error::Hyper(_) => "Hyper error",
            Error::Rpc(_) => "Rpc error",
            Error::NoErrorOrResult => "Malformed RPC response",
            Error::NonceMismatch => "Nonce of response did not match nonce of request",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::Json(ref e) => Some(e),
            Error::Hyper(ref e) => Some(e),
            _ => None,
        }
    }
}
