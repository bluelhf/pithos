#![feature(seek_stream_len)]
#![feature(async_closure)]
#![feature(try_blocks)]
#![feature(once_cell)]

mod custom_headers;
mod model;
mod error;

use std::net::SocketAddr;
use uuid::Uuid;

use axum::{
    http::{
        header::CONTENT_TYPE,
        method::Method,
        StatusCode,
    },
    routing::{get, post},
    extract::{BodyStream, Path, State},
    Router,
    body::StreamBody,
    TypedHeader,
};
use axum::http::{HeaderName};
use axum::http::header::CONTENT_LENGTH;
use axum::response::{IntoResponse};

#[cfg(feature = "tls")] use axum_server::tls_rustls::RustlsConfig;
#[cfg(feature = "tls")] use std::path::PathBuf;


use tower_http::cors::{CorsLayer, Any};

use crate::{
    error::PilviError,
    custom_headers::{X_FILE_NAME, XFileName},
    model::Model,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();


    // Model lives for the lifetime of the program — it is 'effectively static' so fine to leak
    let model: &'static Model = Box::leak(Box::new(Model::with_storage("files".into())));

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
    State(model): State<&'static Model>,
    TypedHeader(x_file_name): TypedHeader<XFileName>,
    body: BodyStream
) -> Result<(StatusCode, String), PilviError> {
    model.write_file(&x_file_name.0, body).await
        .map(|uuid| (StatusCode::CREATED, uuid.to_string()))
}

#[axum::debug_handler]
async fn download_handler(
    State(model): State<&'static Model>,
    Path(uuid): Path<Uuid>
) -> Result<([(HeaderName, String); 2], impl IntoResponse), PilviError> {
    let (name, length, body) = model.read_file(uuid).await?;

    Ok(([(X_FILE_NAME.into(), name), (CONTENT_LENGTH, length.to_string())], StreamBody::new(body)))
}