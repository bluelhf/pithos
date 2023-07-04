//! Contains services that can be used to generate URLs for accessing files.

use core::fmt::{self, Display, Formatter};
use std::net::IpAddr;
use core::time::Duration;
use async_trait::async_trait;
use google_cloud_storage::client::Client;
use google_cloud_storage::sign::{SignedURLMethod, SignedURLOptions};
use uuid::Uuid;
use serde::Serialize;
use crate::errors::PithosError;

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
    async fn request_upload_url(&self, ip: IpAddr, length: u64) -> Result<UploadHandle, PithosError>;
    async fn request_download_url(&self, ip: IpAddr, file_identifier: Uuid) -> Result<DownloadHandle, PithosError>;
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
    async fn request_upload_url(&self, _: IpAddr, length: u64) -> Result<UploadHandle, PithosError> {
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

    async fn request_download_url(&self, _: IpAddr, file_identifier: Uuid) -> Result<DownloadHandle, PithosError> {
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