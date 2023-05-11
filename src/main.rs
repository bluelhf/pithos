#![feature(async_fn_in_trait)]
#![feature(seek_stream_len)]
#![feature(trivial_bounds)]
#![feature(async_closure)]
#![feature(try_blocks)]

extern crate core;

mod custom_headers;
mod model;
mod error;

use std::net::SocketAddr;
use std::path::PathBuf;
use uuid::Uuid;

use axum::{http::{
    header::CONTENT_TYPE,
    method::Method,
    StatusCode,
}, routing::{get, post}, extract::{BodyStream, Path, State}, Router, body::StreamBody, TypedHeader};
use axum::http::HeaderName;
use axum::http::header::CONTENT_LENGTH;
use axum::response::{IntoResponse};

#[cfg(feature = "tls")] use axum_server::tls_rustls::RustlsConfig;
use axum::headers::HeaderMap;

use tower_http::cors::{CorsLayer, Any};
use tracing::info;

use crate::{
    error::PilviError,
    custom_headers::{X_FILE_NAME, XFileName},
    model::Model,
};
use crate::model::{GoogleCloudStorageModel, LocalFilesystemModel, SnailNoopModel};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let model_choice = std::env::var("MODEL").unwrap_or("local".into());
    let model: Box<dyn Model> = match model_choice.as_str() {
        "gcs" => initialise_gcs_model().await,
        "local" => initialise_local_model(),
        "snail" => Box::new(SnailNoopModel::default()),
        _ => panic!("MODEL environment variable must be 'gcs', 'local', or 'snail', but was '{model_choice}'"),
    };

    info!("Initialised {model} Model");

    // Model lives for the lifetime of the program — it is 'effectively static' so fine to leak
    let model: &'static dyn Model = Box::leak(model);

    let app = Router::new()
        .route("/upload", post(upload_handler))
        .route("/download/:uuid", get(download_handler))
        .layer(cors_layer())
        .with_state(model);

    let port = std::env::var("PORT")
        .map(|s| s.parse().unwrap_or_else(|_| panic!("'{s}' is not a valid port")))
        .ok();

    let addr = SocketAddr::from(([0, 0, 0, 0], port.unwrap_or(8080)));

    info!("Listening on {addr}{change_suggest}", change_suggest = if let Some(_) = port { "" } else { " (change with the PORT environment variable)" });

    #[cfg(feature = "tls")]
    {
        let config = RustlsConfig::from_pem_file(
            ["tls", "cert.pem"].iter().collect::<PathBuf>(),
            ["tls", "key.pem"].iter().collect::<PathBuf>()
        ).await.expect("tls/cert.pem and tls/key.pem should contain relevant tls certificate data");

        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            .await.expect("failed to bind to socket — is something else already there?");
    }
    #[cfg(not(feature = "tls"))]
    {
        axum_server::bind(addr)
            .serve(app.into_make_service())
            .await.expect("failed to bind to socket — is something else already there?");
    }
}

async fn initialise_gcs_model() -> Box<GoogleCloudStorageModel> {
    let bucket_name = std::env::var("BUCKET_NAME").expect("BUCKET_NAME environment variable must be set");

    Box::new(GoogleCloudStorageModel::with_bucket(
        bucket_name,
        cloud_storage::Client::new(),
    ).await.expect("failed to create Google Cloud Storage Model"))
}

fn initialise_local_model() -> Box<LocalFilesystemModel> {
    let path: PathBuf = std::env::var("STORAGE_PATH")
        .unwrap_or("files".into())
        .parse().expect("STORAGE_PATH must be a valid path");

    Box::new(LocalFilesystemModel::with_storage(
        path
    ))
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .expose_headers(vec![X_FILE_NAME.into(), CONTENT_LENGTH])
        .allow_headers(vec![X_FILE_NAME.into(), CONTENT_TYPE])
        .allow_methods([Method::HEAD, Method::GET, Method::POST])
        .allow_origin(Any)
}

#[axum::debug_handler]
async fn upload_handler(
    State(model): State<&'static dyn Model>,
    TypedHeader(x_file_name): TypedHeader<XFileName>,
    body: BodyStream
) -> Result<(StatusCode, String), PilviError> {
    info!("Receiving upload '{name}'", name = x_file_name.0);
    model.write_file(&x_file_name.0, None, body).await
        .map(|uuid| (StatusCode::CREATED, uuid.to_string()))
}

#[axum::debug_handler]
async fn download_handler(
    State(model): State<&'static dyn Model>,
    Path(uuid): Path<Uuid>
) -> Result<(HeaderMap, impl IntoResponse), PilviError> {
    let (name, length, body) = model.read_file(uuid).await?;
    info!("Receiving download request for '{name}'");

    let mut map = HeaderMap::new();
    map.insert::<HeaderName>(X_FILE_NAME.into(), name.parse()?);
    if let Some(length) = length {
        map.insert(CONTENT_LENGTH, length.to_string().parse()?);
    }
    Ok((map, StreamBody::new(body)))
}