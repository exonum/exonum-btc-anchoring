// Rust JSON-RPC 1.0 Library
// Written in 2015 by
//     Andrew Poelstra <apoelstra@wpsoftware.net>
//
// Modified in 2016 by
//     Jean Pierre De Jesus Dudey Diaz <jeandudey@hotmail.com>
//
// Modified in 2016 by
//     Aleksey Sidorov <gorthauer87@yandex.ru>
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

//! # Rust JSON-RPC Library
//!
//! Rust support for the JSON-RPC 1.0 protocol.
//!

#![crate_type = "lib"]
#![crate_type = "rlib"]
#![crate_type = "dylib"]
#![crate_name = "jsonrpc_v1"]

// Coding conventions
#![deny(missing_docs,
        trivial_casts, trivial_numeric_casts,
        unused_import_braces, unused_qualifications)]
#![deny(non_upper_case_globals)]
#![deny(non_camel_case_types)]
#![deny(non_snake_case)]
#![deny(unused_mut)]

extern crate hyper;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

pub mod client;
pub mod error;

use serde_json::Value;
use serde_json::value::from_value;
// Re-export error type
pub use error::Error;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// A JSONRPC request object
pub struct Request {
    /// A String containing the name of the method to be invoked
    pub method: String,
    /// An Array of objects to pass as arguments to the method
    pub params: Vec<Value>,
    /// The request id. This can be of any type. It is used to match the
    /// response with the request that it is replying to
    pub id: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
/// A JSONRPC response object
pub struct Response {
    /// The Object that was returned by the invoked method. This must be null
    /// in case there was a error invoking the method
    pub result: Option<Value>,
    /// An Error object if there was an error invoking the method. It must be
    /// null if there was no error
    pub error: Option<Value>,
    /// This must be the same id as the request it is responding to
    pub id: Value,
}

impl Response {
    /// Extract the result from a response
    pub fn result<T: serde::Deserialize>(&self) -> Result<T, Error> {
        if let Some(ref e) = self.error {
            return Err(Error::Rpc(e.clone()));
        }
        match self.result {
            Some(ref res) => from_value(res.clone()).map_err(Error::Json),
            None => Err(Error::NoErrorOrResult),
        }
    }

    /// Extract the result from a response, consuming the response
    pub fn into_result<T: serde::Deserialize>(self) -> Result<T, Error> {
        if let Some(e) = self.error {
            return Err(Error::Rpc(e));
        }
        match self.result {
            Some(res) => from_value(res).map_err(Error::Json),
            None => Err(Error::NoErrorOrResult),
        }
    }

    /// Return the RPC error, if there was one, but do not check the result
    pub fn check_error(self) -> Result<(), Error> {
        if let Some(e) = self.error {
            Err(Error::Rpc(e))
        } else {
            Ok(())
        }
    }

    /// Returns whether or not the `result` field is empty
    pub fn is_none(&self) -> bool {
        self.result.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::{Request, Response};
    use super::serde_json::ser;
    use super::serde_json::de;
    use super::serde_json::value::ToJson;
    use super::serde_json::Value;

    #[test]
    fn request_serialize_round_trip() {
        let original = Request {
            method: "test".to_owned(),
            params: vec![().to_json(), false.to_json(), true.to_json(), "test2".to_json()],
            id: "69".to_json(),
        };

        let s = ser::to_vec(&original).unwrap();
        let d = de::from_slice(s.as_slice()).unwrap();

        assert_eq!(original, d);
    }

    #[test]
    fn response_is_none() {
        let joanna = Response {
            result: Some(true.to_json()),
            error: None,
            id: 81.to_json(),
        };

        let bill = Response {
            result: None,
            error: None,
            id: 66.to_json(),
        };

        assert!(!joanna.is_none());
        assert!(bill.is_none());
    }

    #[test]
    fn response_extract() {
        let obj = vec!["Mary", "had", "a", "little", "lamb"];
        let response = Response {
            result: Some(obj.to_json()),
            error: None,
            id: Value::Null,
        };
        let recovered1: Vec<String> = response.result().unwrap();
        assert!(response.clone().check_error().is_ok());
        let recovered2: Vec<String> = response.into_result().unwrap();
        assert_eq!(obj, recovered1);
        assert_eq!(obj, recovered2);
    }
}
