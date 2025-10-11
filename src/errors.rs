//! Contains the error types used by Pithos.

use std::error::Error;
use core::fmt::{self, Debug, Display, Formatter};
use axum::{http, Json};
use axum::response::{IntoResponse, Response};
use google_cloud_storage::sign::SignedURLError;
use http::status::StatusCode;

use crate::file_extensions::ExtensionError;

use serde_json::json;
use tracing::error;

/// Errors that can happen when handling requests to Pithos.
#[derive(Debug)]
pub enum PithosError {
    /// An error occurred while trying to create a signed URL.
    Access(Box<dyn Error>),
    /// The file that the user wants to upload is larger than the configured maximum upload size.
    TooLarge(u64, u64),
    /// The user is blocked from using this service, i.e. their IP is on the blacklist.
    Blocked,
    /// The request succeeded, but an internal error occurred when attempting to write the file.
    ServerError(Box<dyn Error>),
    /// The local file being requested doesn't exist.
    NoSuchFile,
    /// The requested file extension wasn't valid, as it must match /(\.\p{Alnum}+)+/
    InvalidExtension(Box<dyn Error>)
}

impl PithosError {
    /// Returns the HTTP status code for this error.
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::TooLarge(_, _) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Blocked => StatusCode::FORBIDDEN,
            Self::Access(_) | Self::ServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NoSuchFile => StatusCode::NOT_FOUND,
            Self::InvalidExtension(_) => StatusCode::BAD_REQUEST
        }
    }
}

impl Display for PithosError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge(given, max) => { write!(f, "The file you tried to upload was too large. The maximum file size is {max} bytes, but you tried to upload {given} bytes.") }
            Self::Blocked => { write!(f, "You are blocked from using this service.") }
            Self::Access(_) => { write!(f, "The server failed to create an access URL") }
            Self::ServerError(_) => { write!(f, "The storage server failed to store the file.") }
            Self::NoSuchFile => { write!(f, "The file being requested doesn't exist. ") }
            Self::InvalidExtension(_) => { write!(f, "The requested file extension was invalid. It must be one or more groups of a dot followed by unicode alphanumerics.") }
        }
    }
}

impl Error for PithosError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TooLarge(_, _) | Self::Blocked | Self::NoSuchFile => None,
            Self::Access(e) | Self::ServerError(e) | Self::InvalidExtension(e) => Some(&**e),
        }
    }
}

impl From<SignedURLError> for PithosError {
    fn from(e: SignedURLError) -> Self {
        Self::Access(Box::new(e))
    }
}

impl From<ExtensionError> for PithosError {
    fn from(e: ExtensionError) -> Self {
        Self::InvalidExtension(Box::new(e))
    }
}

impl IntoResponse for PithosError {
    fn into_response(self) -> Response {
        let code = self.status_code();

        if code.is_server_error() {
            error!("{self:?}");
        }

        (code, Json(json!({"error": self.to_string()}))).into_response()
    }
}
