use async_trait::async_trait;

use std::io::ErrorKind::AlreadyExists;
use std::path::PathBuf;
use axum::BoxError;
use axum::extract::BodyStream;
use bytes::Bytes;
use futures::StreamExt;
use hyper::{Body, Uri};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::io::ReaderStream;
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
    http_client: hyper::Client<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>>,
}

impl GoogleCloudStorageModel {
    pub async fn with_bucket(bucket_name: String, client: cloud_storage::Client) -> Result<Self, cloud_storage::Error> {
        Ok(Self {
            bucket: client.bucket().read(&bucket_name).await?,
            client,
            http_client: hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots().https_or_http().enable_all_versions().build()),
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
                Ok::<_, PilviError>(bytes::Bytes::from(chunk?))
            }
        ));

        self.client.object()
            .create_streamed(
                &*self.bucket.name, stream, content_length.map(|l| l + file_name_bytes.len() as u64 + 8),
                &id.to_string(), "application/octet-stream"
            ).await?;

        Ok(id)
    }

    async fn read_file(&self, file_identifier: Uuid) -> Result<(String, Option<u64>, Body), PilviError> {
        let object = self.client.object().read(&*self.bucket.name, &file_identifier.to_string()).await?;
        let object_metadata = object.download_url(60)?;

        let res = self.http_client.get(object_metadata.parse::<Uri>()?).await?;
        let body = res.into_body();

        let mut byte_stream = body.flat_map(|chunk| {
            futures::stream::iter(chunk.unwrap().to_vec())
        });

        let bytes = byte_stream.by_ref().take(8).collect::<Vec<u8>>().await.try_into().unwrap();

        let file_name_length: u64 = u64::from_be_bytes(bytes);
        let name = byte_stream.by_ref().take(file_name_length as usize).collect::<Vec<u8>>().await.try_into().unwrap();
        let file_name = String::from_utf8(name)?;

        let chunks = byte_stream.chunks(1024).map(|chunk| {
            Ok::<_, BoxError>(Bytes::from(chunk))
        });

        Ok((file_name, None, Body::wrap_stream(chunks)))
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