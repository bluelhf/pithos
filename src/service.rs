//! Contains services that can be used to generate URLs for accessing files.

use core::fmt::{self, Display, Formatter};
use core::time::Duration;
use std::collections::HashMap;
use async_trait::async_trait;
use google_cloud_storage::client::Client;
use google_cloud_storage::sign::{SignedURLMethod, SignedURLOptions};
use mime::Mime;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use crate::errors::PithosError;
use crate::file_extensions::FileExt;

#[derive(Deserialize, Copy, Clone)]
pub enum AvailableService {
    LocalStorage,
    GoogleCloudStorage
}

/// Represents a response to a file upload request.
#[derive(Serialize)]
pub struct UploadHandle {
    /// The URL to which the file should be uploaded.
    pub url: String,
    /// The UUID of the file, for downloading.
    pub uuid: Uuid,
}

/// Represents a response to a file download request.
#[derive(Serialize)]
pub struct DownloadHandle {
    /// The URL from which the file can be downloaded.
    pub url: String,
}

/// A service that can be used to generate URLs for accessing files.
#[async_trait]
pub trait Service: Display + Sync + Send {
    async fn request_upload_url(&self, length: u64) -> Result<UploadHandle, PithosError>;
    async fn request_download_url(&self, type_hint: Option<Mime>, extension_hint: Option<FileExt>, file_identifier: Uuid) -> Result<DownloadHandle, PithosError>;
}

pub struct LocalStorage {
    upload_path: String,
    download_path: String,
}

impl LocalStorage {
    pub fn new(upload_path: &str, download_path: &str) -> Self {
        Self { upload_path: upload_path.to_string(), download_path: download_path.to_string() }
    }
}

impl Display for LocalStorage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Local Storage")
    }
}

#[async_trait]
impl Service for LocalStorage {
    async fn request_upload_url(&self, _: u64) -> Result<UploadHandle, PithosError> {
        let uuid = Uuid::new_v4();

        let url = axum_signed_urls::build(&format!("{}/{}", self.upload_path, uuid), HashMap::new())
            .map_err(|e| { PithosError::Access(e.into()) })?;
        Ok(UploadHandle { url, uuid })
    }

    async fn request_download_url(&self, hint: Option<Mime>, ext_hint: Option<FileExt>, file_identifier: Uuid) -> Result<DownloadHandle, PithosError> {
        let mut query = HashMap::new();

        if let Some(hint) = hint {
            query.insert("type_hint", hint.to_string());
        }

        if let Some(ext_hint) = ext_hint {
            query.insert("ext_hint", ext_hint.0);
        }

        let url = axum_signed_urls::build(&format!("{}/{}", self.download_path, file_identifier), query.iter().map(|(k, v)| (*k, v.as_str())).collect())
            .map_err(|e| { PithosError::Access(e.into()) })?;

        Ok(DownloadHandle { url })
    }
}

/// A service that uses Google Cloud Storage to store files.
pub struct GoogleCloudStorage {
    /// The name of the bucket in which files are stored.
    bucket_name: String,
    /// The client used to communicate with Google Cloud Storage.
    client: Client,
}

impl GoogleCloudStorage {
    /// Creates a new Google Cloud Storage service.
    pub const fn with_bucket(bucket_name: String, client: Client) -> Self {
        Self {
            bucket_name,
            client,
        }
    }
}

impl Display for GoogleCloudStorage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Google Cloud Storage")
    }
}

#[async_trait]
impl Service for GoogleCloudStorage {
    async fn request_upload_url(&self, length: u64) -> Result<UploadHandle, PithosError> {
        let uuid = Uuid::new_v4();

        let url = self.client.signed_url(
            &self.bucket_name,
            &uuid.to_string(),
            None, None, SignedURLOptions {
                method: SignedURLMethod::PUT,
                headers: vec![format!("Content-Length: {length}")],
                expires: Duration::from_secs(1800),
                ..Default::default()
            }
        ).await?;

        Ok(UploadHandle { url, uuid })
    }

    async fn request_download_url(&self, _: Option<Mime>, _: Option<FileExt>, file_identifier: Uuid) -> Result<DownloadHandle, PithosError> {
        Ok(DownloadHandle {
            url: self.client.signed_url(
            &self.bucket_name,
            &file_identifier.to_string(),
            None, None, SignedURLOptions {
                method: SignedURLMethod::GET,
                expires: Duration::from_secs(1800),
                ..Default::default()
            }).await?
        })
    }
}
