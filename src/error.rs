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
    FileSystemError(io::Error),
    NoSuchFileError(io::Error),
    FileCorruptedError(CorruptionError),
    ContentReadError(axum::Error),
}

#[derive(Debug)]
pub enum CorruptionError {
    InvalidHeader(InvalidHeaderValue),
    InvalidUri(InvalidUri),
    InvalidFileName(FromUtf8Error),
}

impl Display for CorruptionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CorruptionError::InvalidHeader(_) => { write!(f, "The server tried to send invalid data.") }
            CorruptionError::InvalidUri(_) => { write!(f, "Google Cloud Storage returned an invalid object URI.") }
            CorruptionError::InvalidFileName(_) => { write!(f, "The requested file has been corrupted.") }
        }
    }
}

impl Error for CorruptionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CorruptionError::InvalidHeader(e) => Some(e),
            CorruptionError::InvalidUri(e) => Some(e),
            CorruptionError::InvalidFileName(e) => Some(e)
        }
    }
}

impl PilviError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            PilviError::FileSystemError(_) => { StatusCode::INTERNAL_SERVER_ERROR }
            PilviError::FileCorruptedError(_) => { StatusCode::INTERNAL_SERVER_ERROR }
            PilviError::ContentReadError(_) => { StatusCode::BAD_REQUEST }
            PilviError::NoSuchFileError(_) => { StatusCode::NOT_FOUND }
        }
    }
}

impl Display for PilviError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // FIXME(ilari): this is a dirty hack, figure out how to
        //               actually log properly with axum
        error!("{:?}", self);
        match self {
            PilviError::FileSystemError(_) => { write!(f, "There was an unexpected file system error while storing your file in the cloud.") }
            PilviError::ContentReadError(_) => { write!(f, "There was an error transmitting your file over the internet.") }
            PilviError::FileCorruptedError(_) => { write!(f, "The requested file was corrupted on the server and can't be retrieved.") }
            PilviError::NoSuchFileError(_) => { write!(f, "The requested file does not exist.") }
        }
    }
}

impl Error for PilviError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            PilviError::FileSystemError(e) => Some(e),
            PilviError::ContentReadError(e) => Some(e),
            PilviError::FileCorruptedError(e) => Some(e),
            PilviError::NoSuchFileError(e) => Some(e)
        }
    }
}

impl From<io::Error> for PilviError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::NotFound => PilviError::NoSuchFileError(e),
            _ => PilviError::FileSystemError(e)
        }
    }
}

impl From<axum::Error> for PilviError {
    fn from(e: axum::Error) -> Self {
        PilviError::ContentReadError(e)
    }
}

impl From<hyper::Error> for PilviError {
    fn from(e: hyper::Error) -> Self {
        PilviError::FileSystemError(io::Error::new(io::ErrorKind::Other, e))
    }
}


impl From<FromUtf8Error> for PilviError {
    fn from(e: FromUtf8Error) -> Self {
        PilviError::FileCorruptedError(CorruptionError::InvalidFileName(e))
    }
}

impl From<cloud_storage::Error> for PilviError {
    fn from(e: cloud_storage::Error) -> Self {
        PilviError::FileSystemError(io::Error::new(io::ErrorKind::Other, e))
    }
}

impl From<InvalidUri> for PilviError {
    fn from(e: InvalidUri) -> Self {
        PilviError::FileCorruptedError(CorruptionError::InvalidUri(e))
    }
}

impl From<InvalidHeaderValue> for PilviError {
    fn from(e: InvalidHeaderValue) -> Self {
        PilviError::FileCorruptedError(CorruptionError::InvalidHeader(e))
    }
}

impl IntoResponse for PilviError {
    fn into_response(self) -> Response {
        (self.status_code(), self.to_string()).into_response()
    }
}