use std::path::PathBuf;
use axum::extract::BodyStream;
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::io::ReaderStream;
use uuid::Uuid;
use crate::error::PilviError;

pub struct Model {
    storage_directory: PathBuf
}

impl Model {
    pub fn with_storage(storage_directory: PathBuf) -> Self {
        Self {
            storage_directory
        }
    }

    async fn try_create_storage(&self) -> Result<(), PilviError> {
        tokio::fs::create_dir(&self.storage_directory).await.map_err(Into::into)
    }

    pub async fn write_file(&self, file_name: &str, mut file_content: BodyStream) -> Result<Uuid, PilviError> {
        self.try_create_storage().await?;

        let id = Uuid::new_v4();
        let file_path = self.storage_directory.join(id.to_string());

        let mut file = tokio::fs::File::create(&file_path).await?;

        file.write_u64(file_name.len() as u64).await?;
        file.write(file_name.as_bytes()).await?;

        while let Some(chunk) = file_content.next().await {
            file.write_all(&chunk?).await?;
        };

        Ok(id)
    }

    pub async fn read_file(&self, file_identifier: Uuid) -> Result<(String, ReaderStream<tokio::fs::File>), PilviError> {
        let file_path = self.storage_directory.join(file_identifier.to_string());
        let mut file = tokio::fs::File::open(&file_path).await?;

        let length = file.read_u64().await?;
        let mut file_name = vec![0; length as usize];
        file.read_exact(&mut file_name).await?;

        let file_name = String::from_utf8(file_name)?;

        Ok((file_name, ReaderStream::new(file)))
    }
}