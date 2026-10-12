//! Contains the error types used by Pithos.

use std::error::Error;
use core::fmt::{self, Debug, Display, Formatter};
use axum::{http, Json};
use axum::response::{IntoResponse, Response};
use axum::extract::rejection::{PathRejection, QueryRejection, TypedHeaderRejection};
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
    /// The body of an upload exceeded the size declared when its signed URL was requested.
    BodyTooLarge(u64),
    /// The user is blocked from using this service, i.e. their IP is on the blacklist.
    Blocked,
    /// The user requested a byte range that is outside the file's current data.
    InvalidRange(u64, u64, u64),
    /// The request succeeded, but an internal error occurred when attempting to write the file.
    ServerError(Box<dyn Error>),
    /// The local file being requested doesn't exist.
    NoSuchFile,
    /// The requested query parameters were invalid
    InvalidQuery(Box<dyn Error>),
    /// A request header was missing or malformed.
    InvalidHeader(Box<dyn Error>),
    /// A parameter in the requested path was malformed.
    InvalidPath(Box<dyn Error>),
    /// The signature of the requested URL is missing or does not match.
    InvalidSignature,
    /// The IP address of the client could not be determined.
    UnknownClient(Box<dyn Error>),
}

impl PithosError {
    /// Returns the HTTP status code for this error.
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::TooLarge(_, _) | Self::BodyTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Blocked => StatusCode::FORBIDDEN,
            Self::Access(_) | Self::ServerError(_) | Self::UnknownClient(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NoSuchFile => StatusCode::NOT_FOUND,
            Self::InvalidRange(_, _, _) => StatusCode::RANGE_NOT_SATISFIABLE,
            Self::InvalidQuery(_) | Self::InvalidHeader(_) | Self::InvalidPath(_) => StatusCode::BAD_REQUEST,
            Self::InvalidSignature => StatusCode::UNAUTHORIZED,
        }
    }
}

impl Display for PithosError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge(given, max) => { write!(f, "The file you tried to upload was too large. The maximum file size is {max} bytes, but you tried to upload {given} bytes.") }
            Self::BodyTooLarge(declared) => { write!(f, "The upload exceeded the declared file size of {declared} bytes and was discarded.") }
            Self::Blocked => { write!(f, "You are blocked from using this service.") }
            Self::Access(_) => { write!(f, "The server failed to create an access URL") }
            Self::ServerError(_) => { write!(f, "The storage server failed to store the file.") }
            Self::NoSuchFile => { write!(f, "The file being requested doesn't exist. ") }
            Self::InvalidQuery(e) => {
                let mut root_ref: &(dyn Error + 'static) = &**e;
                while let Some(source) = root_ref.source() {
                    root_ref = source;
                }
                write!(f, "The requested query parameters were invalid: {root_ref}.")
            }
            Self::InvalidRange(start, end, length) => { write!(f, "The requested range, {start}-{end} bytes, is invalid, as the file is only {length} bytes in size.")}
            Self::InvalidHeader(e) => { write!(f, "The request headers were invalid: {e}.") }
            Self::InvalidPath(e) => { write!(f, "The requested path was invalid: {e}.") }
            Self::InvalidSignature => { write!(f, "The signature of the requested URL is missing or invalid.") }
            Self::UnknownClient(_) => { write!(f, "The server failed to determine your IP address.") }
        }
    }
}

impl Error for PithosError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TooLarge(_, _) | Self::BodyTooLarge(_) | Self::Blocked | Self::NoSuchFile | Self::InvalidRange(_, _, _) | Self::InvalidSignature => None,
            Self::Access(e) | Self::ServerError(e) | Self::InvalidQuery(e)
            | Self::InvalidHeader(e) | Self::InvalidPath(e) | Self::UnknownClient(e) => Some(&**e),
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
        Self::InvalidQuery(Box::new(e))
    }
}

impl From<QueryRejection> for PithosError {
    fn from(e: QueryRejection) -> Self {
        Self::InvalidQuery(Box::new(e))
    }
}

impl From<TypedHeaderRejection> for PithosError {
    fn from(e: TypedHeaderRejection) -> Self {
        Self::InvalidHeader(Box::new(e))
    }
}

impl From<PathRejection> for PithosError {
    fn from(e: PathRejection) -> Self {
        // axum reports path rejections caused by our own routes (rather than the client's input) as 5xx
        if e.status().is_client_error() {
            Self::InvalidPath(Box::new(e))
        } else {
            Self::ServerError(Box::new(e))
        }
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
