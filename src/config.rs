//! A module for managing the configuration of Pithos.

use std::net::IpAddr;
use axum_client_ip::SecureClientIpSource;
use serde::Deserialize;

/// A parsed representation of the configuration file.
#[derive(Deserialize)]
pub struct Config {
    /// The table containing configuration for file uploads.
    files: Files,
    /// The table containing the IP address blacklist.
    ip_blacklist: IpBlacklist,
    /// The table containing the server configuration
    server: Server,
}

impl Config {
    /// Returns the maximum upload size in bytes.
    pub(crate) const fn max_upload_size(&self) -> u64 {
        self.files.max_upload_size
    }

    /// Returns whether the given IP address is blocked.
    pub(crate) fn is_blocked(&self, ip: &IpAddr) -> bool {
        self.ip_blacklist.blocked_ips.contains(ip)
    }

    /// Returns the client IP source.
    pub(crate) fn get_ip_source(&self) -> SecureClientIpSource {
        self.server.ip_source.clone()
    }
}

/// The table containing configuration for file uploads.
#[derive(Deserialize)]
struct Files {
    /// The maximum size of individual uploads in bytes.
    max_upload_size: u64,
}

/// The table containing the IP address blacklist.
#[derive(Deserialize)]
struct IpBlacklist {
    /// The list of IP addresses that are blocked from using Pithos.
    blocked_ips: Vec<IpAddr>,
}

/// The table containing the server configuration
#[derive(Deserialize)]
struct Server {
    /// The source for obtaining the client's IP address
    ip_source: SecureClientIpSource,
}