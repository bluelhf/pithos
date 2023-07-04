//! Pithos is a simple file-sharing service.
#![feature(async_fn_in_trait)]
#![feature(seek_stream_len)]
#![feature(trivial_bounds)]
#![feature(async_closure)]
#![feature(try_blocks)]

#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
)]
#![allow(clippy::multiple_crate_versions)]


use std::net::SocketAddr;

use axum::{extract::{Path, State}, http::{method::Method, StatusCode}, Json, middleware, Router, routing::get, TypedHeader};
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum_client_ip::SecureClientIp;
use google_cloud_storage::client::{Client, ClientConfig};
use tokio::fs;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use uuid::Uuid;

use crate::config::Config;
use crate::custom_headers::{X_FILE_SIZE, XFileSize};
use crate::errors::PithosError;
use crate::service::{DownloadHandle, GoogleCloudStorage, Service, UploadHandle};

mod errors;
mod service;
mod config;
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

    // app state lives for the lifetime of the program — it is 'effectively static' so fine to leak
    let state: &'static AppState = Box::leak(Box::new(AppState {
        service: initialise_gcs_service().await?,
        config: initialise_config().await?,
    }));

    let app = Router::new()
        .route("/upload", get(upload_handler))
        .route("/download/:uuid", get(download_handler))
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
    let config_text = fs::read_to_string("Config.toml").await?;
    Ok(toml::from_str(&config_text)?)
}

/// Initialises the Google Cloud Storage Service, using the `GOOGLE_CLOUD_BUCKET` and `GOOGLE_APPLICATION_CREDENTIALS` environment variables.
async fn initialise_gcs_service() -> Result<Box<GoogleCloudStorage>, Box<dyn std::error::Error>> {
    let bucket_name = std::env::var("GOOGLE_CLOUD_BUCKET")?;

    let service = Box::new(GoogleCloudStorage::with_bucket(
        bucket_name,
        Client::new(ClientConfig::default().with_auth().await?),
    ));

    info!("Initialised {service} Service");

    Ok(service)
}

/// Configures CORS for the application.
fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::HEAD, Method::GET])
        .allow_headers(vec![X_FILE_SIZE.into()])
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
    SecureClientIp(ip): SecureClientIp,
    TypedHeader(file_size): TypedHeader<XFileSize>,
) -> Result<(StatusCode, Json<UploadHandle>), PithosError> {
    let AppState { config, service } = state;

    if (file_size.0) > config.max_upload_size() {
        return Err(PithosError::TooLarge(file_size.0, config.max_upload_size()));
    }

    service.request_upload_url(ip, file_size.0).await
        .map(|handle| (StatusCode::CREATED, Json(handle)))
}

/// Handles requests to download a file, redirecting them to the service.
#[axum::debug_handler]
async fn download_handler(
    State(state): State<&'static AppState>,
    SecureClientIp(ip): SecureClientIp,
    Path(uuid): Path<Uuid>,
) -> Result<Json<DownloadHandle>, PithosError> {
    let handle = state.service.request_download_url(ip, uuid).await?;
    Ok(Json(handle))
}