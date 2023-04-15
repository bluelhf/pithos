use std::error::Error;
use std::{fmt, io};
use std::fmt::Formatter;
use std::string::FromUtf8Error;
use axum::http;
use axum::response::{IntoResponse, Response};
use http::status::StatusCode;
use tracing::error;

#[derive(Debug)]
pub enum PilviError {
    FileSystemError(io::Error),
    NoSuchFileError(io::Error),
    FileCorruptedError(FromUtf8Error),
    ContentReadError(axum::Error),
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

impl fmt::Display for PilviError {
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


impl From<FromUtf8Error> for PilviError {
    fn from(e: FromUtf8Error) -> Self {
        PilviError::FileCorruptedError(e)
    }
}

impl IntoResponse for PilviError {
    fn into_response(self) -> Response {
        (self.status_code(), self.to_string()).into_response()
    }
}