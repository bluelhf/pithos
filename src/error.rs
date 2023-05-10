use std::error::Error;
use std::{fmt, io};
use std::fmt::{Debug, Display, Formatter};
use std::string::FromUtf8Error;
use axum::http;
use axum::response::{IntoResponse, Response};
use http::status::StatusCode;
use hyper::header::InvalidHeaderValue;
use hyper::http::uri::InvalidUri;
use tracing::error;

#[derive(Debug)]
pub enum PilviError {
    FileSystem(io::Error),
    NoSuchFile(io::Error),
    FileCorrupted(CorruptionError),
    ContentRead(axum::Error),
}

#[derive(Debug)]
pub enum CorruptionError {
    Header(InvalidHeaderValue),
    Uri(InvalidUri),
    FileName(FromUtf8Error),
}

impl Display for CorruptionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CorruptionError::Header(_) => { write!(f, "The server tried to send invalid data.") }
            CorruptionError::Uri(_) => { write!(f, "Google Cloud Storage returned an invalid object URI.") }
            CorruptionError::FileName(_) => { write!(f, "The requested file has been corrupted.") }
        }
    }
}

impl Error for CorruptionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CorruptionError::Header(e) => Some(e),
            CorruptionError::Uri(e) => Some(e),
            CorruptionError::FileName(e) => Some(e)
        }
    }
}

impl PilviError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            PilviError::FileSystem(_) => { StatusCode::INTERNAL_SERVER_ERROR }
            PilviError::FileCorrupted(_) => { StatusCode::INTERNAL_SERVER_ERROR }
            PilviError::ContentRead(_) => { StatusCode::BAD_REQUEST }
            PilviError::NoSuchFile(_) => { StatusCode::NOT_FOUND }
        }
    }
}

impl Display for PilviError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // FIXME(ilari): this is a dirty hack, figure out how to
        //               actually log properly with axum
        error!("{:?}", self);
        match self {
            PilviError::FileSystem(_) => { write!(f, "There was an unexpected file system error while storing your file in the cloud.") }
            PilviError::ContentRead(_) => { write!(f, "There was an error transmitting your file over the internet.") }
            PilviError::FileCorrupted(_) => { write!(f, "The requested file was corrupted on the server and can't be retrieved.") }
            PilviError::NoSuchFile(_) => { write!(f, "The requested file does not exist.") }
        }
    }
}

impl Error for PilviError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            PilviError::FileSystem(e) => Some(e),
            PilviError::ContentRead(e) => Some(e),
            PilviError::FileCorrupted(e) => Some(e),
            PilviError::NoSuchFile(e) => Some(e)
        }
    }
}

impl From<io::Error> for PilviError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::NotFound => PilviError::NoSuchFile(e),
            _ => PilviError::FileSystem(e)
        }
    }
}

impl From<axum::Error> for PilviError {
    fn from(e: axum::Error) -> Self {
        PilviError::ContentRead(e)
    }
}

impl From<hyper::Error> for PilviError {
    fn from(e: hyper::Error) -> Self {
        PilviError::FileSystem(io::Error::new(io::ErrorKind::Other, e))
    }
}


impl From<FromUtf8Error> for PilviError {
    fn from(e: FromUtf8Error) -> Self {
        PilviError::FileCorrupted(CorruptionError::FileName(e))
    }
}

impl From<cloud_storage::Error> for PilviError {
    fn from(e: cloud_storage::Error) -> Self {
        PilviError::FileSystem(io::Error::new(io::ErrorKind::Other, e))
    }
}

impl From<InvalidUri> for PilviError {
    fn from(e: InvalidUri) -> Self {
        PilviError::FileCorrupted(CorruptionError::Uri(e))
    }
}

impl From<InvalidHeaderValue> for PilviError {
    fn from(e: InvalidHeaderValue) -> Self {
        PilviError::FileCorrupted(CorruptionError::Header(e))
    }
}

impl IntoResponse for PilviError {
    fn into_response(self) -> Response {
        (self.status_code(), self.to_string()).into_response()
    }
}