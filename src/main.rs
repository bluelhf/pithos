#![feature(async_closure)]
#![feature(try_blocks)]
#![feature(once_cell)]


use std::net::SocketAddr;
use std::path::PathBuf;

use axum::{http, Router, routing::post, TypedHeader};
use axum::body::{Body, StreamBody};
use axum::extract::{Path, State};
use axum::http::{Method, Request, Response};
use axum::http::response::Builder;
use axum::routing::get;
use axum_server::tls_rustls::RustlsConfig;
use hyper::StatusCode;
use tokio::fs::File;

use tokio_util::io::ReaderStream;

use tower_http::cors;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::error::PilviError;
use crate::header::{X_FILE_NAME, XFileName};
use crate::model::Model;

mod header;
mod model;
mod error;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = RustlsConfig::from_pem_file(
        [env!("CARGO_MANIFEST_DIR"), "tls", "cert.pem"].iter().collect::<PathBuf>(),
        [env!("CARGO_MANIFEST_DIR"), "tls", "key.pem"].iter().collect::<PathBuf>()
    ).await.expect("tls/cert.pem and tls/key.pem should contain relevant tls certificate data");

    // Model lives for the lifetime of the program — it is 'effectively static' so fine to leak
    let model: &'static Model = Box::leak(Box::new(Model::with_storage("files".into())));

    let app = Router::new()
        .route("/upload", post(upload_handler))
        .route("/download/:uuid", get(download_handler))
        .route("/filename/:uuid", get(file_name_handler))
        .layer(cors_layer())
        .with_state(model);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await.expect("failed to bind to socket — is something else already there?");
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_headers(vec![X_FILE_NAME.to_owned(), http::header::CONTENT_TYPE])
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(cors::Any)
}

#[axum::debug_handler]
async fn upload_handler(
    State(model): State<&'static Model>,
    TypedHeader(x_file_name): TypedHeader<XFileName>,
    request: Request<Body>
) -> Result<Response<String>, PilviError> {
    model.write_file(&x_file_name.0, request.into_body()).await
        .map(|uuid| Response::builder()
            .status(StatusCode::CREATED)
            .body(uuid.to_string()).unwrap()) // unwrap is safe because we know the status code is valid
}

#[axum::debug_handler]
async fn file_name_handler(
    State(model): State<&'static Model>,
    Path(uuid): Path<Uuid>
) -> Result<String, PilviError> {
    Ok(model.read_file(uuid).await?.0)
}

type FileResponse = Response<StreamBody<ReaderStream<File>>>;

#[axum::debug_handler]
async fn download_handler(
    State(model): State<&'static Model>,
    Path(uuid): Path<Uuid>
) -> Result<FileResponse, PilviError> {
    let (name, body) = model.read_file(uuid).await?;

    Ok(Builder::new()
        .header("X-File-Name", name)
        .body(StreamBody::new(body)).unwrap())
}