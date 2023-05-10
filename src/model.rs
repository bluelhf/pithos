use std::collections::VecDeque;
use std::io;
use std::io::ErrorKind;
use async_trait::async_trait;

use std::io::ErrorKind::AlreadyExists;
use std::path::PathBuf;
use axum::extract::BodyStream;
use bytes::Bytes;
use futures::{StreamExt, TryStreamExt};
use hyper::Body;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::io::{ReaderStream, StreamReader};
use uuid::Uuid;
use crate::error::PilviError;


#[async_trait]
pub(crate) trait Model: Sync + Send {
    async fn write_file(&self, file_name: &str, length: Option<u64>, file_content: BodyStream) -> Result<Uuid, PilviError>;
    async fn read_file(&self, file_identifier: Uuid) -> Result<(String, Option<u64>, Body), PilviError>;
}


pub struct GoogleCloudStorageModel {
    client: cloud_storage::Client,
    bucket: cloud_storage::bucket::Bucket,
}

impl GoogleCloudStorageModel {
    pub async fn with_bucket(bucket_name: String, client: cloud_storage::Client) -> Result<Self, cloud_storage::Error> {
        Ok(Self {
            bucket: client.bucket().read(&bucket_name).await?,
            client,
        })
    }
}

#[async_trait]
impl Model for GoogleCloudStorageModel {
    async fn write_file(&self, file_name: &str, content_length: Option<u64>, file_content: BodyStream) -> Result<Uuid, PilviError> {
        let id = Uuid::new_v4();

        let file_name_bytes = Bytes::from(file_name.to_string());
        let length = Bytes::from(file_name.len().to_be_bytes().to_vec());

        let stream = futures::stream::iter(vec![Ok(length), Ok(file_name_bytes.clone())].into_iter()).chain(file_content.map(
            |chunk| {
                Ok::<_, PilviError>(chunk?)
            }
        ));

        self.client.object()
            .create_streamed(
                &self.bucket.name, stream, content_length.map(|l| l + file_name_bytes.len() as u64 + 8),
                &id.to_string(), "application/octet-stream"
            ).await?;

        Ok(id)
    }

    async fn read_file(&self, file_identifier: Uuid) -> Result<(String, Option<u64>, Body), PilviError> {
        let mut body = StreamReader::new(self.client.object()
            .download_streamed(&self.bucket.name, &file_identifier.to_string())
            .await?.chunks(1024)
            .map(|c| c.into_iter().collect::<Result<VecDeque<u8>, cloud_storage::Error>>())
            .map(|e| e.map_err(|err| {
                io::Error::new(ErrorKind::Other, err)
            }))
        );


        let mut length_bytes = [0u8; 8];
        body.read_exact(&mut length_bytes).await?;
        let file_name_length = u64::from_be_bytes(length_bytes);

        let mut name_bytes = vec![0u8; file_name_length as usize];
        body.read_exact(&mut name_bytes).await?;
        let file_name = String::from_utf8(name_bytes)?;

        Ok((file_name, None, Body::wrap_stream(body.into_inner().map_ok(Vec::<u8>::from))))
    }
}

pub struct LocalFilesystemModel {
    storage_directory: PathBuf
}

impl LocalFilesystemModel {
    pub fn with_storage(storage_directory: PathBuf) -> Self {
        Self {
            storage_directory
        }
    }

    async fn try_create_storage(&self) -> Result<(), PilviError> {
        match tokio::fs::create_dir(&self.storage_directory).await {
            Ok(_) => Ok(()),
            Err(error) => {
                if error.kind() == AlreadyExists { Ok(()) } else { Err(error.into()) }
            }
        }
    }
}

#[async_trait]
impl Model for LocalFilesystemModel {
    async fn write_file(&self, file_name: &str, _: Option<u64>, mut file_content: BodyStream) -> Result<Uuid, PilviError> {
        self.try_create_storage().await?;

        let id = Uuid::new_v4();
        let file_path = self.storage_directory.join(id.to_string());

        let mut file = File::create(&file_path).await?;

        file.write_u64(file_name.len() as u64).await?;
        file.write(file_name.as_bytes()).await?;

        while let Some(chunk) = file_content.next().await {
            file.write_all(&chunk?).await?;
        };

        Ok(id)
    }

    async fn read_file(&self, file_identifier: Uuid) -> Result<(String, Option<u64>, Body), PilviError> {
        let file_path = self.storage_directory.join(file_identifier.to_string());
        let mut file = File::open(&file_path).await?;
        file.sync_all().await?;

        let length = file.read_u64().await?;
        let mut file_name = vec![0; length as usize];
        file.read_exact(&mut file_name).await?;

        let file_name = String::from_utf8(file_name)?;

        Ok((file_name, Some(file.metadata().await?.len() - 8 - length), Body::wrap_stream(ReaderStream::new(file))))
    }
}