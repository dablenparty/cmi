use std::{error, fmt, io};

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    ReqwestError(reqwest::Error),
    SerdeJsonError(serde_json::Error),
    ZipError(async_zip::error::ZipError),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::IoError(err) => write!(f, "IO error: {}", err),
            Error::ReqwestError(err) => write!(f, "Reqwest error: {}", err),
            Error::ZipError(err) => write!(f, "Zip error: {}", err),
            Error::SerdeJsonError(err) => write!(f, "Serde JSON error: {}", err),
        }
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::ReqwestError(err)
    }
}

impl From<async_zip::error::ZipError> for Error {
    fn from(err: async_zip::error::ZipError) -> Self {
        Error::ZipError(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::SerdeJsonError(err)
    }
}
