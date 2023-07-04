//! Contains the error types used by Pithos.

use std::error::Error;
use core::fmt::{self, Debug, Display, Formatter};
use axum::{http, Json};
use axum::response::{IntoResponse, Response};
use google_cloud_storage::sign::SignedURLError;
use http::status::StatusCode;

use serde_json::json;
use tracing::error;

/// Errors that can happen when handling requests to Pithos.
#[derive(Debug)]
pub enum PithosError {
    /// A Google Cloud Storage error occurred while trying to create a signed URL.
    Access(SignedURLError),
    /// The file that the user wants to upload is larger than the configured maximum upload size.
    TooLarge(u64, u64),
    /// The user is blocked from using this service, i.e. their IP is on the blacklist.
    Blocked,
}

impl PithosError {
    /// Returns the HTTP status code for this error.
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::TooLarge(_, _) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Blocked => StatusCode::FORBIDDEN,
            Self::Access(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl Display for PithosError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge(given, max) => { write!(f, "The file you tried to upload was too large. The maximum file size is {max} bytes, but you tried to upload {given} bytes.") }
            Self::Blocked => { write!(f, "You are blocked from using this service.") }
            Self::Access(_) => { write!(f, "The server failed to create an access URL") }
        }
    }
}

impl Error for PithosError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TooLarge(_, _) | Self::Blocked => None,
            Self::Access(e) => Some(e),
        }
    }
}

impl From<SignedURLError> for PithosError {
    fn from(e: SignedURLError) -> Self {
        Self::Access(e)
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
