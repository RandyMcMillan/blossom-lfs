//! Configuration loading for the blossom-lfs agent.
//!
//! Configuration is resolved in priority order:
//!
//! 1. `.lfsdalconfig` in the repository root (INI format).
//! 2. `.git/config` (INI format, under `[lfs-dal]`).
//! 3. Environment variables (`BLOSSOM_SERVER_URL`, `NOSTR_PRIVATE_KEY`).
//!
//! Private keys may be provided as either a 64-character hex string or a
//! Bech32-encoded `nsec1…` value.

use anyhow::{Context as _, Result};
use std::path::PathBuf;

/// Default chunk size: 16 MiB.
const DEFAULT_CHUNK_SIZE: usize = 16 * 1024 * 1024;
const DEFAULT_CONCURRENT_UPLOADS: usize = 8;
const DEFAULT_CONCURRENT_DOWNLOADS: usize = 8;
const DEFAULT_DAEMON_PORT: u16 = 31921;

/// Runtime configuration for the LFS agent.
#[derive(Debug, Clone)]
pub struct Config {
    /// HTTP URL of the Blossom server.
    ///
    /// Optional when `force_transport = iroh` and `iroh_endpoint` is set
    /// (iroh-only mode). Required otherwise.
    pub server_url: Option<String>,
    /// Optional iroh endpoint ID (base32-encoded). When set alongside
    /// `server_url`, the daemon uses iroh for uploads and HTTP for downloads
    /// with automatic fallback.
    pub iroh_endpoint: Option<String>,
    /// Nostr private key as a 64-character hex string.
    pub secret_key_hex: String,
    /// Maximum bytes per chunk (default 16 MiB).
    pub chunk_size: usize,
    /// Maximum number of concurrent chunk uploads.
    pub max_concurrent_uploads: usize,
    /// Maximum number of concurrent chunk downloads.
    pub max_concurrent_downloads: usize,
    /// Force all operations through a single transport.
    /// Without this, iroh is preferred for uploads and HTTP for downloads.
    pub force_transport: Option<ForceTransport>,
    /// Daemon port for lock proxy (default 31921).
    pub daemon_port: u16,
}

/// Force a specific transport for all operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForceTransport {
    /// Force all operations through HTTP.
    Http,
    /// Force all operations through iroh.
    Iroh,
}

impl Config {
    /// Load configuration from git config files or environment variables.
    ///
    /// Tries `.lfsdalconfig`, then `.git/config`, then falls back to
    /// environment variables.
    pub fn from_git_config() -> Result<Self> {
        let config_paths = vec![PathBuf::from(".lfsdalconfig"), PathBuf::from(".git/config")];

        for path in config_paths {
            if path.exists() {
                let config_content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read config file: {:?}", path))?;

                if let Ok(config) = Self::parse_config(&config_content) {
                    return Ok(config);
                }
            }
        }

        Self::from_env()
    }

    /// Load configuration from a specific repo directory path.
    ///
    /// Looks for `.lfsdalconfig` and `.git/config` in the given directory.
    /// Used by the daemon to load per-repo config.
    pub fn from_repo_path(repo_path: &std::path::Path) -> Result<Self> {
        let lfsdalconfig = repo_path.join(".lfsdalconfig");
        if lfsdalconfig.exists() {
            let content = std::fs::read_to_string(&lfsdalconfig)
                .with_context(|| format!("Failed to read config: {:?}", lfsdalconfig))?;
            if let Ok(config) = Self::parse_config(&content) {
                return Ok(config);
            }
        }

        let git_config = repo_path.join(".git/config");
        if git_config.exists() {
            let content = std::fs::read_to_string(&git_config)
                .with_context(|| format!("Failed to read config: {:?}", git_config))?;
            if let Ok(config) = Self::parse_config(&content) {
                return Ok(config);
            }
        }

        Self::from_env()
    }

    fn parse_config(content: &str) -> Result<Self> {
        let mut server_url = None;
        let mut iroh_endpoint = None;
        let mut private_key_str = None;
        let mut chunk_size = DEFAULT_CHUNK_SIZE;
        let mut max_concurrent_uploads = DEFAULT_CONCURRENT_UPLOADS;
        let mut max_concurrent_downloads = DEFAULT_CONCURRENT_DOWNLOADS;
        let mut force_transport: Option<ForceTransport> = None;
        let mut daemon_port = DEFAULT_DAEMON_PORT;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() || line.starts_with('[') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                match key {
                    "server" => server_url = Some(value.to_string()),
                    "iroh-endpoint" | "irohEndpoint" => {
                        iroh_endpoint = Some(value.to_string());
                    }
                    "private-key" | "privateKey" => private_key_str = Some(value.to_string()),
                    "chunk-size" | "chunkSize" => {
                        if let Ok(v) = value.parse() {
                            chunk_size = v;
                        }
                    }
                    "max-concurrent-uploads" | "maxConcurrentUploads" => {
                        if let Ok(v) = value.parse() {
                            max_concurrent_uploads = v;
                        }
                    }
                    "max-concurrent-downloads" | "maxConcurrentDownloads" => {
                        if let Ok(v) = value.parse() {
                            max_concurrent_downloads = v;
                        }
                    }
                    "transport" => {
                        let v = value.trim().to_lowercase();
                        match v.as_str() {
                            "iroh" | "quic" => {
                                force_transport = Some(ForceTransport::Iroh);
                            }
                            "http" | "https" => {
                                force_transport = Some(ForceTransport::Http);
                            }
                            _ => {}
                        }
                    }
                    "daemon-port" | "daemonPort" => {
                        if let Ok(v) = value.parse() {
                            daemon_port = v;
                        }
                    }
                    _ => {}
                }
            }
        }

        let is_iroh_only = force_transport == Some(ForceTransport::Iroh) && iroh_endpoint.is_some();
        let server_url = if is_iroh_only {
            server_url
        } else {
            Some(server_url.ok_or_else(|| anyhow::anyhow!("Missing server URL in config"))?)
        };

        let private_key_str = private_key_str
            .or_else(|| std::env::var("NOSTR_PRIVATE_KEY").ok())
            .ok_or_else(|| anyhow::anyhow!("Missing private key"))?;

        let secret_key_hex = normalize_to_hex(&private_key_str)?;

        Ok(Config {
            server_url,
            iroh_endpoint,
            secret_key_hex,
            chunk_size,
            max_concurrent_uploads,
            max_concurrent_downloads,
            force_transport,
            daemon_port,
        })
    }

    fn from_env() -> Result<Self> {
        let server_url_env = std::env::var("BLOSSOM_SERVER_URL").ok();

        let private_key_str = std::env::var("NOSTR_PRIVATE_KEY")
            .or_else(|_| anyhow::bail!("Missing NOSTR_PRIVATE_KEY environment variable"))?;

        let secret_key_hex = normalize_to_hex(&private_key_str)?;

        let iroh_endpoint = std::env::var("BLOSSOM_IROH_ENDPOINT").ok();

        let force_transport = std::env::var("BLOSSOM_TRANSPORT").ok().and_then(|v| {
            match v.trim().to_lowercase().as_str() {
                "iroh" | "quic" => Some(ForceTransport::Iroh),
                "http" | "https" => Some(ForceTransport::Http),
                _ => None,
            }
        });

        let daemon_port = std::env::var("BLOSSOM_DAEMON_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_DAEMON_PORT);

        let is_iroh_only = force_transport == Some(ForceTransport::Iroh) && iroh_endpoint.is_some();
        let server_url = if is_iroh_only {
            server_url_env
        } else {
            Some(server_url_env.ok_or_else(|| {
                anyhow::anyhow!("Missing BLOSSOM_SERVER_URL environment variable")
            })?)
        };

        Ok(Config {
            server_url,
            iroh_endpoint,
            secret_key_hex,
            chunk_size: DEFAULT_CHUNK_SIZE,
            max_concurrent_uploads: DEFAULT_CONCURRENT_UPLOADS,
            max_concurrent_downloads: DEFAULT_CONCURRENT_DOWNLOADS,
            force_transport,
            daemon_port,
        })
    }
}

fn normalize_to_hex(key: &str) -> Result<String> {
    let key = key.trim();

    if key.starts_with("nsec1") {
        let secret_key = nostr::SecretKey::parse(key)
            .map_err(|e| anyhow::anyhow!("Failed to parse nsec: {}", e))?;
        Ok(hex::encode(secret_key.secret_bytes()))
    } else {
        // Validate it's valid hex and correct length
        let bytes = hex::decode(key).map_err(|e| anyhow::anyhow!("Failed to decode hex: {}", e))?;
        if bytes.len() != 32 {
            anyhow::bail!("Invalid secret key length: expected 32 bytes");
        }
        Ok(key.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_chunk_size() {
        assert_eq!(DEFAULT_CHUNK_SIZE, 16 * 1024 * 1024);
    }

    #[test]
    fn test_parse_config_basic() {
        let config = Config::parse_config(
            "server=https://blossom.example.com\n\
             private-key=0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        assert_eq!(
            config.server_url,
            Some("https://blossom.example.com".to_string())
        );
        assert!(config.iroh_endpoint.is_none());
        assert!(config.force_transport.is_none());
        assert_eq!(config.chunk_size, DEFAULT_CHUNK_SIZE);
        assert_eq!(config.max_concurrent_uploads, DEFAULT_CONCURRENT_UPLOADS);
        assert_eq!(
            config.max_concurrent_downloads,
            DEFAULT_CONCURRENT_DOWNLOADS
        );
        assert_eq!(config.daemon_port, DEFAULT_DAEMON_PORT);
    }

    #[test]
    fn test_parse_config_custom_values() {
        let config = Config::parse_config(
            "server=https://blossom.example.com\n\
             private-key=0000000000000000000000000000000000000000000000000000000000000001\n\
             chunk-size=4096\n\
             max-concurrent-uploads=4\n\
             max-concurrent-downloads=2\n\
             daemon-port=9999",
        )
        .unwrap();
        assert_eq!(config.chunk_size, 4096);
        assert_eq!(config.max_concurrent_uploads, 4);
        assert_eq!(config.max_concurrent_downloads, 2);
        assert_eq!(config.daemon_port, 9999);
    }

    #[test]
    fn test_parse_config_with_iroh_endpoint() {
        let config = Config::parse_config(
            "server=https://blossom.example.com\n\
             iroh-endpoint=abc123def456\n\
             private-key=0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        assert_eq!(
            config.server_url,
            Some("https://blossom.example.com".to_string())
        );
        assert_eq!(config.iroh_endpoint.as_deref(), Some("abc123def456"));
        assert!(config.force_transport.is_none());
    }

    #[test]
    fn test_parse_config_transport_iroh_legacy() {
        let config = Config::parse_config(
            "server=https://blossom.example.com\n\
             transport=iroh\n\
             private-key=0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        assert_eq!(config.force_transport, Some(ForceTransport::Iroh));
        assert_eq!(
            config.server_url,
            Some("https://blossom.example.com".to_string())
        );
    }

    #[test]
    fn test_parse_config_force_http() {
        let config = Config::parse_config(
            "server=https://blossom.example.com\n\
             iroh-endpoint=abc123\n\
             transport=http\n\
             private-key=0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        assert_eq!(config.force_transport, Some(ForceTransport::Http));
    }

    #[test]
    fn test_parse_config_iroh_only_no_server() {
        let config = Config::parse_config(
            "iroh-endpoint=abc123\n\
             transport=iroh\n\
             private-key=0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        assert_eq!(config.force_transport, Some(ForceTransport::Iroh));
        assert_eq!(config.iroh_endpoint.as_deref(), Some("abc123"));
        assert!(config.server_url.is_none());
    }

    #[test]
    fn test_parse_config_missing_server_no_iroh() {
        let result = Config::parse_config(
            "private-key=0000000000000000000000000000000000000000000000000000000000000001",
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing server URL"));
    }

    #[test]
    fn test_parse_config_missing_private_key() {
        let result = Config::parse_config("server=https://blossom.example.com");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing private key"));
    }

    #[test]
    fn test_parse_config_comments_and_sections() {
        let config = Config::parse_config(
            "# this is a comment\n\
             [lfs-dal]\n\
             \n\
             server=https://blossom.example.com\n\
             private-key=0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        assert_eq!(
            config.server_url,
            Some("https://blossom.example.com".to_string())
        );
    }

    #[test]
    fn test_parse_config_quoted_values() {
        let config = Config::parse_config(
            "server=\"https://blossom.example.com\"\n\
             private-key=0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        assert_eq!(
            config.server_url,
            Some("https://blossom.example.com".to_string())
        );
    }

    #[test]
    fn test_parse_config_unknown_keys_ignored() {
        let config = Config::parse_config(
            "server=https://blossom.example.com\n\
             private-key=0000000000000000000000000000000000000000000000000000000000000001\n\
             unknown-key=some-value",
        )
        .unwrap();
        assert_eq!(
            config.server_url,
            Some("https://blossom.example.com".to_string())
        );
    }

    #[test]
    fn test_normalize_hex_key() {
        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let result = normalize_to_hex(hex).unwrap();
        assert_eq!(result, hex);
    }

    #[test]
    fn test_normalize_hex_key_whitespace() {
        let hex = " 0000000000000000000000000000000000000000000000000000000000000001 ";
        let result = normalize_to_hex(hex).unwrap();
        assert_eq!(result, hex.trim());
    }

    #[test]
    fn test_normalize_invalid_hex() {
        let result = normalize_to_hex("not-hex-at-all-XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX");
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_short_hex() {
        let result = normalize_to_hex("abcd");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_config_camel_case_keys() {
        let config = Config::parse_config(
            "server=https://blossom.example.com\n\
             privateKey=0000000000000000000000000000000000000000000000000000000000000001\n\
             chunkSize=4096\n\
             maxConcurrentUploads=2\n\
             maxConcurrentDownloads=4\n\
             irohEndpoint=testep\n\
             daemonPort=8080",
        )
        .unwrap();
        assert_eq!(config.chunk_size, 4096);
        assert_eq!(config.max_concurrent_uploads, 2);
        assert_eq!(config.max_concurrent_downloads, 4);
        assert_eq!(config.iroh_endpoint.as_deref(), Some("testep"));
        assert_eq!(config.daemon_port, 8080);
    }

    #[test]
    fn test_parse_config_invalid_chunk_size_uses_default() {
        let config = Config::parse_config(
            "server=https://blossom.example.com\n\
             private-key=0000000000000000000000000000000000000000000000000000000000000001\n\
             chunk-size=not-a-number",
        )
        .unwrap();
        assert_eq!(config.chunk_size, DEFAULT_CHUNK_SIZE);
    }

    #[test]
    fn test_from_repo_path_lfsdalconfig() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".lfsdalconfig"),
            "server=https://example.com\nprivate-key=0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();

        let config = Config::from_repo_path(dir.path()).unwrap();
        assert_eq!(config.server_url, Some("https://example.com".to_string()));
    }

    #[test]
    fn test_from_repo_path_no_config_falls_to_env() {
        let dir = tempfile::tempdir().unwrap();
        let result = Config::from_repo_path(dir.path());
        assert!(result.is_err());
    }
}
