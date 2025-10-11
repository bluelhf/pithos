//! Pithos is a simple file-sharing service.
#![feature(seek_stream_len)]
#![feature(trivial_bounds)]
#![feature(try_blocks)]

#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
)]
#![allow(clippy::multiple_crate_versions)]


use std::io::{Error, ErrorKind};
use std::net::SocketAddr;

use serde_with::{serde_as, DisplayFromStr};

use axum::{extract::{Path, State}, http::{method::Method, StatusCode}, Json, middleware, Router, routing::get, TypedHeader};
use axum::extract::{BodyStream, Query};
use axum::http::{HeaderMap, HeaderValue, Request};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::put;
use axum_client_ip::SecureClientIp;
use axum_signed_urls::SignedUrl;
use futures::TryStreamExt;
use google_cloud_storage::client::{Client, ClientConfig};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use uuid::Uuid;

use mime::Mime;

use crate::config::Config;
use crate::custom_headers::{X_FILE_SIZE, XFileSize};
use crate::errors::PithosError;
use crate::service::{AvailableService, DownloadHandle, GoogleCloudStorage, LocalStorage, Service, UploadHandle};
use crate::file_extensions::FileExt;

mod errors;
mod service;
mod config;
mod file_extensions;
mod custom_headers;

/// Represents the state of the application at any given time.
struct AppState {
    /// The service used to generate URLs for accessing files
    service: Box<dyn Service>,
    /// The configuration of the application
    config: Config,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let _ = dotenv::dotenv();

    let config = initialise_config().await?;

    let service: Box<dyn Service> = match config.chosen_service() {
        AvailableService::LocalStorage => { Box::new(LocalStorage::new("/signed_upload", "/signed_download")) }
        AvailableService::GoogleCloudStorage => { Box::new(initialise_gcs_service(&config).await?) }
    };

    info!("Initialised {service} Service");

    // app state lives for the lifetime of the program — it is 'effectively static' so fine to leak
    let state: &'static AppState = Box::leak(Box::new(AppState { service, config }));

    let app = Router::new()
        .route("/upload", get(upload_handler))
        .route("/download/:uuid", get(download_handler))
        .route("/signed_upload/:uuid", put(signed_upload_handler))
        .route("/signed_download/:uuid", get(signed_download_handler))
        .layer(ServiceBuilder::new()
            .layer(state.config.get_ip_source().into_extension())
            .layer(middleware::from_fn_with_state(state, filter_ips))
            .layer(cors_layer()))
        .with_state(state);

    let port = match std::env::var("PORT") {
        Ok(port_choice) => Some(port_choice.parse::<u16>()?),
        Err(_) => None,
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], port.unwrap_or(8080)));

    info!("Listening on {addr}{change_suggest}", change_suggest = if port.is_some() { "" } else { " (change with the PORT environment variable)" });


    Ok(axum_server::bind(addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?)
}

/// Parses the `Config.toml` file and returns a `Config` struct.
async fn initialise_config() -> Result<Config, Box<dyn std::error::Error>> {
    use tokio::fs;
    let config_text = fs::read_to_string("Config.toml").await?;
    Ok(toml::from_str(&config_text)?)
}

/// Initialises the Google Cloud Storage Service, using the `GOOGLE_APPLICATION_CREDENTIALS` or `GOOGLE_APPLICATION_CREDENTIALS_JSON` environment variables.
async fn initialise_gcs_service(config: &Config) -> Result<GoogleCloudStorage, Box<dyn std::error::Error>> {
    let gcs_config = config.gcs_config();

    let service = GoogleCloudStorage::with_bucket(gcs_config.bucket_name(),
        Client::new(ClientConfig::default().with_auth().await?));

    Ok(service)
}

/// Configures CORS for the application.
fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::HEAD, Method::GET, Method::PUT])
        .allow_headers(vec![X_FILE_SIZE.into(), CONTENT_TYPE])
        .allow_origin(Any)
}

/// Filters out requests from blocked IPs.
async fn filter_ips<B: Send>(State(state): State<&'static AppState>, SecureClientIp(ip): SecureClientIp, request: Request<B>, next: Next<B>) -> Result<Response, PithosError> {
    if state.config.is_blocked(&ip) {
        return Err(PithosError::Blocked);
    }

    Ok(next.run(request).await)
}

/// Handles requests to upload a file, redirecting them to the service.
#[axum::debug_handler]
async fn upload_handler(
    State(state): State<&'static AppState>,
    TypedHeader(file_size): TypedHeader<XFileSize>,
) -> Result<(StatusCode, Json<UploadHandle>), PithosError> {
    let AppState { config, service } = state;

    if (file_size.0) > config.max_upload_size() {
        return Err(PithosError::TooLarge(file_size.0, config.max_upload_size()));
    }

    service.request_upload_url(file_size.0).await
        .map(|handle| (StatusCode::CREATED, Json(handle)))
}

#[serde_as]
#[derive(Deserialize)]
pub struct DownloadQuery {
    #[serde_as(as = "Option<DisplayFromStr>")]
    type_hint: Option<Mime>,

    #[serde_as(as = "Option<DisplayFromStr>")]
    ext_hint: Option<FileExt>
}

/// Handles requests to download a file, redirecting them to the service.
#[axum::debug_handler]
async fn download_handler(
    State(state): State<&'static AppState>,
    Path(uuid): Path<Uuid>,
    Query(options): Query<DownloadQuery>
) -> Result<Json<DownloadHandle>, PithosError> {
    let handle = state.service.request_download_url(options.type_hint, options.ext_hint, uuid).await?;
    Ok(Json(handle))
}

/// Handles requests to upload a file to the local Pithos storage.
#[axum::debug_handler]
async fn signed_upload_handler(
    State(state): State<&'static AppState>,
    _: SignedUrl,
    Path(uuid): Path<Uuid>,
    body: BodyStream
) -> Result<StatusCode, PithosError> {
    use tokio::fs;
    use tokio_util::io::StreamReader;

    let AppState { config, .. } = state;

    let path = config.local_storage_path();
    match fs::create_dir_all(&path).await {
        Err(err) if err.kind() == ErrorKind::AlreadyExists => (),
        r => r.map_err(|e| PithosError::ServerError(Box::new(e)))?
    }

    let mut file = File::create(path.join(uuid.to_string())).await
        .map_err(|e| PithosError::ServerError(Box::new(e)))?;

    let body_with_io_error = body.map_err(|err| Error::new(ErrorKind::Other, err));
    let mut body_reader = StreamReader::new(body_with_io_error);
    tokio::io::copy_buf(&mut body_reader, &mut file).await.map_err(|e| PithosError::ServerError(Box::new(e)))?;

    Ok(StatusCode::ACCEPTED)
}

use axum::body::StreamBody;
use hyper::header::CONTENT_TYPE;
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use tokio::fs::File;

/// Handles requests to download a file from the local Pithos storage.
#[axum::debug_handler]
async fn signed_download_handler(
    State(state): State<&'static AppState>,
    _: SignedUrl,
    Path(uuid): Path<Uuid>,
    Query(options): Query<DownloadQuery>
) -> Result<(StatusCode, HeaderMap, StreamBody<ReaderStream<File>>), PithosError> {
    let AppState { config, .. } = state;

    let path = config.local_storage_path();

    let file = File::open(path.join(uuid.to_string())).await
        .map_err(|e| match e.kind() {
            ErrorKind::NotFound => PithosError::NoSuchFile,
            _ => PithosError::ServerError(Box::new(e))
        })?;

    let size = file.metadata().await.map_err(|e| PithosError::ServerError(Box::new(e)))?.len();

    let reader_stream = ReaderStream::new(file);
    let body = StreamBody::new(reader_stream);

    let mut headers = HeaderMap::new();
    headers.insert("Content-Length", HeaderValue::from(size));
    if let Some(hint) = options.type_hint {
        if let Ok(value) = HeaderValue::try_from(hint.to_string()) {
            headers.insert("Content-Type", value);
            headers.insert("Content-Disposition", HeaderValue::from_static("inline"));
        }
    }

    if let Some(ext_hint) = options.ext_hint {
        if let Ok(value) = HeaderValue::try_from(format!("attachment; filename=\"{uuid}{ext}\"", ext = ext_hint.0)) {
            headers.insert("Content-Disposition", value);
        }
    }

    Ok((StatusCode::OK, headers, body))
}
