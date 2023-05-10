#![feature(async_fn_in_trait)]
#![feature(seek_stream_len)]
#![feature(trivial_bounds)]
#![feature(async_closure)]
#![feature(return_position_impl_trait_in_trait)]
#![feature(try_blocks)]

extern crate core;

mod custom_headers;
mod model;
mod error;

use std::net::SocketAddr;
use uuid::Uuid;

use axum::{http::{
    header::CONTENT_TYPE,
    method::Method,
    StatusCode,
}, routing::{get, post}, extract::{BodyStream, Path, State}, Router, body::StreamBody, TypedHeader, headers};
use axum::http::HeaderName;
use axum::http::header::CONTENT_LENGTH;
use axum::response::{IntoResponse};

#[cfg(feature = "tls")] use axum_server::tls_rustls::RustlsConfig;
#[cfg(feature = "tls")] use std::path::PathBuf;
use axum::headers::HeaderMap;
use headers::ContentLength;


use tower_http::cors::{CorsLayer, Any};

use crate::{
    error::PilviError,
    custom_headers::{X_FILE_NAME, XFileName},
    model::Model,
};
use crate::model::GoogleCloudStorageModel;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let model = GoogleCloudStorageModel::with_bucket(
        std::env::var("BUCKET_NAME").expect("BUCKET_NAME environment variable must be set"),
        cloud_storage::Client::new(),
    ).await.expect("failed to create GoogleCloudStorageModel");
    //let model = LocalFilesystemModel::with_storage("files".into());

    // Model lives for the lifetime of the program — it is 'effectively static' so fine to leak
    let model: &'static dyn Model = Box::leak(Box::new(model));

    let app = Router::new()
        .route("/upload", post(upload_handler))
        .route("/download/:uuid", get(download_handler))
        .layer(cors_layer())
        .with_state(model);

    let port = std::env::var("PORT")
        .map(|s| s.parse().expect(&format!("{s} is not a valid port")))
        .unwrap_or_else(|_| 8080);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

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
    model.write_file(&x_file_name.0, None, body).await
        .map(|uuid| (StatusCode::CREATED, uuid.to_string()))
}

#[axum::debug_handler]
async fn download_handler(
    State(model): State<&'static dyn Model>,
    Path(uuid): Path<Uuid>
) -> Result<(HeaderMap, impl IntoResponse), PilviError> {
    let (name, length, body) = model.read_file(uuid).await?;

    let mut map = HeaderMap::new();
    map.insert::<HeaderName>(X_FILE_NAME.into(), name.clone().parse()?);
    if let Some(length) = length {
        map.insert(CONTENT_LENGTH, length.to_string().parse()?);
    }
    Ok((map, StreamBody::new(body)))
}