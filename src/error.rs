//! Error types
use serde_json;
use std::{convert::From, error, fmt, io, result};

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    Serde(serde_json::Error),
    TPLink(SectionError),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::IO(_) => f.write_str("Error connecting to the device"),
            Error::Serde(_) => f.write_str("Could not parse the response received from the device"),
            Error::TPLink(err) => f.write_str(&format!(
                "Response data error: ({}) {}",
                err.err_code, err.err_msg
            )),
            Error::Other(err) => f.write_str(&err),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::IO(_) => "Error connecting to the device",
            Error::Serde(_) => "Could not parse the response received from the device",
            Error::TPLink(_) => "Response data error",
            Error::Other(err) => err.as_str(),
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IO(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Serde(error)
    }
}

impl From<String> for Error {
    fn from(error: String) -> Self {
        Error::Other(error)
    }
}

impl From<SectionError> for Error {
    fn from(error: SectionError) -> Self {
        Error::TPLink(error)
    }
}

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SectionError {
    pub err_code: i16,
    pub err_msg: String,
}

impl fmt::Display for SectionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}: {}", self.err_code, self.err_msg))
    }
}

impl error::Error for SectionError {
    fn description(&self) -> &str {
        "TPLink section error"
    }
}
