use std::fmt;
use std::error::Error as StdError;
use std::io::Error as IoError;
use serde_json::Error as JsonError;
use std::convert::From;

#[derive(Debug)]
pub enum Error {
    IoError(IoError),
    DeserializeError(JsonError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IoError(_) => f.write_str("Error connecting to the device"),
            Error::DeserializeError(_) => f.write_str("Could not parse the response received from the device"),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::IoError(_) => "Error connecting to the device",
            Error::DeserializeError(_) => "Could not parse the response received from the device",
        }
    }
}

impl From<IoError> for Error {
    fn from(error: IoError) -> Self {
        Error::IoError(error)
    }
}

impl From<JsonError> for Error {
    fn from(error: JsonError) -> Self {
        Error::DeserializeError(error)
    }
}
