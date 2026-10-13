//! Contains services that can be used to generate URLs for accessing files.

use core::fmt::{self, Display, Formatter};
use core::time::Duration;
use std::collections::HashMap;
use async_trait::async_trait;
use google_cloud_storage::client::Client;
use google_cloud_storage::sign::{SignedURLMethod, SignedURLOptions};
use hmac::{Hmac, KeyInit, Mac};
use mime::Mime;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
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
    /// The token that may be used for management of the file.
    pub management_token: String,
    /// The UUID of the file, for downloading.
    pub uuid: Uuid,
}

#[derive(Serialize)]
pub struct DeletionHandle {
    /// The URL from which a file may be deleted.
    pub url: String
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
    async fn request_deletion_url(&self, file_identifier: Uuid, token: &str) -> Result<DeletionHandle, PithosError>;
    async fn request_download_url(&self, type_hint: Option<Mime>, extension_hint: Option<FileExt>, file_identifier: Uuid) -> Result<DownloadHandle, PithosError>;
}

pub struct LocalStorage {
    upload_path: String,
    download_path: String,
    deletion_path: String,
    key: Box<[u8]>
}

impl LocalStorage {
    pub fn new(upload_path: &str, download_path: &str, deletion_path: &str, key: Box<[u8]>) -> Self {
        Self {
            upload_path: upload_path.to_string(),
            download_path: download_path.to_string(),
            deletion_path: deletion_path.to_string(),
            key
        }
    }
}

impl Display for LocalStorage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Local Storage")
    }
}

#[async_trait]
impl Service for LocalStorage {
    async fn request_upload_url(&self, length: u64) -> Result<UploadHandle, PithosError> {
        let uuid = Uuid::new_v4();

        // the declared length is covered by the URL's signature, so the upload handler can enforce it without trusting the client
        let length = length.to_string();
        let url = axum_signed_urls::build(&format!("{}/{}", self.upload_path, uuid), HashMap::from([("length", length.as_str())]))
            .map_err(|e| { PithosError::Access(e.into()) })?;

        let management_token = hex::encode(Hmac::<Sha256>::new_from_slice(&self.key)
            .map_err(|e| PithosError::Access(Box::new(e)))?
            .chain_update(uuid.as_bytes()).finalize().into_bytes());

        Ok(UploadHandle { url, management_token, uuid })
    }

    async fn request_deletion_url(&self, file_identifier: Uuid, token: &str) -> Result<DeletionHandle, PithosError> {
        let _ = Hmac::<Sha256>::new_from_slice(&self.key)?.chain_update(file_identifier.as_bytes()).verify_slice(hex::decode(token)?.as_slice())?;
        let url = axum_signed_urls::build(&format!("{}/{}", self.deletion_path, file_identifier), HashMap::new())
            .map_err(|e| PithosError::Access(e.into()))?;
        Ok(DeletionHandle { url })
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
    /// The HMAC key used for creating deletion tokens.
    key: Box<[u8]>
}

impl GoogleCloudStorage {
    /// Creates a new Google Cloud Storage service.
    pub fn with_bucket(bucket_name: String, client: Client, key: Box<[u8]>) -> Self {
        Self {
            bucket_name,
            client,
            key
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

        let management_token = hex::encode(Hmac::<Sha256>::new_from_slice(&self.key)
            .map_err(|e| PithosError::Access(Box::new(e)))?
            .chain_update(uuid.as_bytes()).finalize().into_bytes());

        Ok(UploadHandle { url, management_token, uuid })
    }

    async fn request_deletion_url(&self, file_identifier: Uuid, token: &str) -> Result<DeletionHandle, PithosError> {
        let _ = Hmac::<Sha256>::new_from_slice(&self.key)?.verify_slice(hex::decode(token)?.as_slice())?;
        Ok(DeletionHandle {
            url: self.client.signed_url(
                &self.bucket_name,
                &file_identifier.to_string(),
                None, None, SignedURLOptions {
                    method: SignedURLMethod::DELETE,
                    expires: Duration::from_secs(1800),
                    ..Default::default()
                }
            ).await?
        })
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
