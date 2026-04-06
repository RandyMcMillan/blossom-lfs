//! Transport abstraction over HTTP and iroh QUIC.
//!
//! [`BlobTransport`] provides a unified async interface for blob operations
//! regardless of the underlying transport. When the `iroh` cargo feature is
//! enabled, [`IrohTransport`] connects to a Blossom peer over QUIC using the
//! iroh network. The default [`HttpTransport`] uses the standard HTTP client.
//!
//! ## Choosing a transport
//!
//! Set `transport = iroh` in your `.lfsdalconfig` (or `BLOSSOM_TRANSPORT=iroh`
//! env var) and provide the peer's iroh node ID as the `server` value. For
//! example:
//!
//! ```ini
//! [lfs-dal]
//!     server = <iroh-node-id>
//!     transport = iroh
//!     private-key = nsec1...
//! ```

use crate::error::{BlossomLfsError, Result};
use async_trait::async_trait;
use blossom_rs::protocol::BlobDescriptor;

/// Unified interface for blob storage operations.
///
/// Both HTTP and iroh transports implement this trait so the agent can
/// be written against a single API.
#[async_trait]
pub trait BlobTransport: Send + Sync {
    /// Upload `data` and return the server's blob descriptor.
    async fn upload(&self, data: &[u8], content_type: &str) -> Result<BlobDescriptor>;

    /// Download a blob by its SHA-256 hex hash.
    async fn download(&self, sha256: &str) -> Result<Vec<u8>>;

    /// Check whether a blob exists on the server (HEAD request / exists op).
    async fn exists(&self, sha256: &str) -> Result<bool>;
}

// ---------------------------------------------------------------------------
// HTTP transport (always available)
// ---------------------------------------------------------------------------

/// HTTP transport backed by [`blossom_rs::BlossomClient`].
pub struct HttpTransport {
    client: blossom_rs::BlossomClient,
}

impl HttpTransport {
    /// Create a new HTTP transport for the given server URL.
    pub fn new(
        server_url: String,
        signer: impl blossom_rs::auth::BlossomSigner + 'static,
        timeout: std::time::Duration,
    ) -> Self {
        let client = blossom_rs::BlossomClient::with_timeout(vec![server_url], signer, timeout);
        Self { client }
    }
}

#[async_trait]
impl BlobTransport for HttpTransport {
    async fn upload(&self, data: &[u8], content_type: &str) -> Result<BlobDescriptor> {
        self.client
            .upload(data, content_type)
            .await
            .map_err(BlossomLfsError::Blossom)
    }

    async fn download(&self, sha256: &str) -> Result<Vec<u8>> {
        self.client
            .download(sha256)
            .await
            .map_err(BlossomLfsError::Blossom)
    }

    async fn exists(&self, sha256: &str) -> Result<bool> {
        self.client
            .exists(sha256)
            .await
            .map_err(BlossomLfsError::Blossom)
    }
}

// ---------------------------------------------------------------------------
// Iroh transport (behind `iroh` feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "iroh")]
pub use iroh_impl::IrohTransport;

#[cfg(feature = "iroh")]
mod iroh_impl {
    use super::*;
    use blossom_rs::transport::IrohBlossomClient;
    use iroh::endpoint::Endpoint;
    use iroh::{EndpointAddr, EndpointId};

    /// QUIC transport backed by [`IrohBlossomClient`].
    ///
    /// Connects to a single Blossom peer identified by its iroh endpoint ID.
    /// Connections are cached internally by the iroh client.
    pub struct IrohTransport {
        client: IrohBlossomClient,
        peer: EndpointAddr,
    }

    impl IrohTransport {
        /// Create a new iroh transport from a local endpoint, signer, and
        /// the remote peer's endpoint ID string.
        ///
        /// The `endpoint_id` should be a base32-encoded iroh endpoint ID.
        pub fn new(
            endpoint: Endpoint,
            signer: impl blossom_rs::auth::BlossomSigner + 'static,
            endpoint_id: &str,
        ) -> std::result::Result<Self, String> {
            let eid: EndpointId = endpoint_id
                .parse()
                .map_err(|e| format!("invalid iroh endpoint ID '{}': {}", endpoint_id, e))?;

            let peer = EndpointAddr::from(eid);
            let client = IrohBlossomClient::new(endpoint, signer);

            Ok(Self { client, peer })
        }
    }

    #[async_trait]
    impl BlobTransport for IrohTransport {
        async fn upload(&self, data: &[u8], _content_type: &str) -> Result<BlobDescriptor> {
            self.client
                .upload(self.peer.clone(), data)
                .await
                .map_err(BlossomLfsError::Blossom)
        }

        async fn download(&self, sha256: &str) -> Result<Vec<u8>> {
            self.client
                .download(self.peer.clone(), sha256)
                .await
                .map_err(BlossomLfsError::Blossom)
        }

        async fn exists(&self, sha256: &str) -> Result<bool> {
            self.client
                .exists(self.peer.clone(), sha256)
                .await
                .map_err(BlossomLfsError::Blossom)
        }
    }
}
