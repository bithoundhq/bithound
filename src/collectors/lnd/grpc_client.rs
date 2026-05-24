//! Thin tonic wrapper for LND's `Lightning` gRPC service. One client
//! per LND node — TLS roots and macaroons are per-node, so channels
//! are NOT shared across multiple LND endpoints the way the Bitcoin
//! `reqwest::Client` is shared.
//!
//! Contract:
//! - **TLS**: trusts ONLY the configured LND cert; native roots are
//!   deliberately off (LND's self-signed cert is the norm; trusting
//!   public CAs adds no value and adds a MITM-via-compromised-CA
//!   risk).
//! - **Macaroon**: hex-encoded at construction; attached as the
//!   `macaroon` metadata header on every request. No interceptor
//!   for v0.8.
//! - **Timeout**: each RPC wrapped in `tokio::time::timeout`
//!   (default 5s).
//! - **Startup-failure policy**: missing/malformed TLS cert at
//!   construction is a `BuildError` (sidecar aborts). LND unreachable
//!   at first poll is `ProbeResult::Failed` — `.connect_lazy()`
//!   defers the actual dial until the first RPC.

use std::sync::Once;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use tonic::metadata::MetadataValue;
use tonic::transport::{Certificate, Channel, ClientTlsConfig};

/// rustls 0.23 requires a process-global `CryptoProvider`. tonic's
/// `tls` feature doesn't install one. Install the ring provider on
/// first use; idempotent thanks to `Once`.
static INSTALL_CRYPTO_PROVIDER: Once = Once::new();

fn ensure_crypto_provider() {
    INSTALL_CRYPTO_PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

use super::lnrpc::lightning_client::LightningClient;
use super::lnrpc::{
    GetInfoRequest, GetInfoResponse, ListChannelsRequest, ListChannelsResponse, ListPeersRequest,
    ListPeersResponse,
};

#[derive(Debug)]
pub struct LndGrpcClient {
    channel: Channel,
    /// Macaroon hex-encoded at construction; stamped on every request
    /// as the `macaroon` metadata header.
    macaroon_hex: String,
    timeout: Duration,
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("cannot read TLS cert at {path}: {source}")]
    TlsCertRead {
        path: String,
        source: std::io::Error,
    },
    #[error("LND gRPC endpoint must include https:// scheme: {endpoint}")]
    MissingScheme { endpoint: String },
    #[error("invalid LND gRPC endpoint: {endpoint} ({reason})")]
    InvalidEndpoint { endpoint: String, reason: String },
    #[error("tonic TLS config failed: {0}")]
    TlsConfig(tonic::transport::Error),
    #[error("macaroon hex encoding produced invalid metadata value")]
    MacaroonInvalid,
}

#[derive(Debug, Error)]
pub enum LndRpcError {
    #[error("tonic status: code={code:?} message={message:?}")]
    Status {
        code: tonic::Code,
        message: String,
    },
    #[error("transport error: {0}")]
    Transport(#[from] tonic::transport::Error),
    #[error("response decode failed: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("timeout after {0:?}")]
    Timeout(Duration),
    #[error("macaroon metadata value invalid")]
    MacaroonInvalid,
}

impl From<tonic::Status> for LndRpcError {
    fn from(status: tonic::Status) -> Self {
        LndRpcError::Status {
            code: status.code(),
            message: status.message().to_string(),
        }
    }
}

impl LndRpcError {
    /// Maps the gRPC-side error into the collector's domain-agnostic
    /// `CollectionErrorKind`.
    pub fn collection_error_kind(&self) -> crate::collectors::CollectionErrorKind {
        use crate::collectors::CollectionErrorKind as K;
        use tonic::Code;
        match self {
            LndRpcError::Timeout(_) => K::Timeout,
            LndRpcError::Decode(_) => K::DecodeError,
            LndRpcError::MacaroonInvalid => K::Misconfigured,
            LndRpcError::Transport(_) => K::Unreachable,
            LndRpcError::Status { code, .. } => match code {
                Code::Unavailable => K::Unreachable,
                Code::DeadlineExceeded => K::Timeout,
                Code::Unauthenticated => K::AuthenticationFailed,
                Code::PermissionDenied => K::PermissionDenied,
                Code::FailedPrecondition => K::Misconfigured,
                Code::Internal => K::Internal,
                Code::Unimplemented => K::UnsupportedVersion,
                Code::ResourceExhausted => K::RateLimited,
                Code::InvalidArgument => K::InvalidResponse,
                _ => K::Internal,
            },
        }
    }
}

impl LndGrpcClient {
    /// Validates endpoint shape, reads + parses the TLS cert, and
    /// hex-encodes the macaroon. **Does not hit the network.** The
    /// channel is built with `.connect_lazy()`; the first RPC
    /// triggers the actual dial.
    pub fn new(
        endpoint: String,
        tls_cert_path: String,
        macaroon: &SecretString,
        timeout: Duration,
    ) -> Result<Self, BuildError> {
        ensure_crypto_provider();

        if !endpoint.starts_with("https://") {
            return Err(BuildError::MissingScheme { endpoint });
        }

        let pem = std::fs::read(&tls_cert_path).map_err(|source| BuildError::TlsCertRead {
            path: tls_cert_path.clone(),
            source,
        })?;
        let ca = Certificate::from_pem(&pem);
        let tls = ClientTlsConfig::new().ca_certificate(ca);

        let channel = Channel::from_shared(endpoint.clone())
            .map_err(|source| BuildError::InvalidEndpoint {
                endpoint,
                reason: source.to_string(),
            })?
            .tls_config(tls)
            .map_err(BuildError::TlsConfig)?
            .connect_lazy();

        let macaroon_hex = hex::encode(macaroon.expose_secret().as_bytes());
        // Sanity-check that the hex parses as a metadata value; the
        // failure mode is impossible in practice (hex alphabet is a
        // subset of HTTP token chars) but the explicit check makes
        // the contract obvious.
        let _: MetadataValue<_> = macaroon_hex
            .parse()
            .map_err(|_| BuildError::MacaroonInvalid)?;

        Ok(Self {
            channel,
            macaroon_hex,
            timeout,
        })
    }

    /// Calls `Lightning.GetInfo`. Per-request macaroon header,
    /// `timeout` window via `tokio::time::timeout`.
    pub async fn get_info(&self) -> Result<GetInfoResponse, LndRpcError> {
        let mut client = LightningClient::new(self.channel.clone());
        let mut request = tonic::Request::new(GetInfoRequest {});
        self.stamp_macaroon(&mut request)?;
        let outcome = tokio::time::timeout(self.timeout, client.get_info(request)).await;
        match outcome {
            Err(_) => Err(LndRpcError::Timeout(self.timeout)),
            Ok(Err(status)) => Err(status.into()),
            Ok(Ok(response)) => Ok(response.into_inner()),
        }
    }

    /// Calls `Lightning.ListChannels`. Returns all channels (not
    /// filtering by `active_only`, `inactive_only`, etc.) — the B1
    /// rule needs both active and inactive channels.
    pub async fn list_channels(&self) -> Result<ListChannelsResponse, LndRpcError> {
        let mut client = LightningClient::new(self.channel.clone());
        let mut request = tonic::Request::new(ListChannelsRequest {
            active_only: false,
            inactive_only: false,
            public_only: false,
            private_only: false,
            peer: Vec::new(),
            peer_alias_lookup: false,
        });
        self.stamp_macaroon(&mut request)?;
        let outcome = tokio::time::timeout(self.timeout, client.list_channels(request)).await;
        match outcome {
            Err(_) => Err(LndRpcError::Timeout(self.timeout)),
            Ok(Err(status)) => Err(status.into()),
            Ok(Ok(response)) => Ok(response.into_inner()),
        }
    }

    /// Calls `Lightning.ListPeers`. Used by the polling collector
    /// (BTH-63) to cross-reference channel `remote_pubkey` against
    /// connected peers for the `peer_online` derived field on
    /// `LndChannelState`.
    pub async fn list_peers(&self) -> Result<ListPeersResponse, LndRpcError> {
        let mut client = LightningClient::new(self.channel.clone());
        let mut request = tonic::Request::new(ListPeersRequest {
            latest_error: false,
        });
        self.stamp_macaroon(&mut request)?;
        let outcome = tokio::time::timeout(self.timeout, client.list_peers(request)).await;
        match outcome {
            Err(_) => Err(LndRpcError::Timeout(self.timeout)),
            Ok(Err(status)) => Err(status.into()),
            Ok(Ok(response)) => Ok(response.into_inner()),
        }
    }

    fn stamp_macaroon<T>(&self, request: &mut tonic::Request<T>) -> Result<(), LndRpcError> {
        let value: MetadataValue<_> = self
            .macaroon_hex
            .parse()
            .map_err(|_| LndRpcError::MacaroonInvalid)?;
        request.metadata_mut().insert("macaroon", value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_macaroon() -> SecretString {
        SecretString::new("some-macaroon-bytes".into())
    }

    /// Validates that the scheme check runs BEFORE the file read, so
    /// operators dialing without `https://` get a clear error even
    /// when the cert path is also wrong.
    #[test]
    fn new_rejects_endpoint_without_https_scheme() {
        let err = LndGrpcClient::new(
            "127.0.0.1:10009".into(),
            "/nonexistent/path/tls.cert".into(),
            &dummy_macaroon(),
            Duration::from_secs(5),
        )
        .expect_err("should reject missing scheme");
        assert!(matches!(err, BuildError::MissingScheme { .. }));
    }

    /// Validates that a missing TLS cert file aborts construction
    /// with `TlsCertRead`. Startup-failure policy: config bugs are
    /// loud at startup, not silent runtime failures.
    #[test]
    fn new_rejects_missing_tls_cert_file() {
        let err = LndGrpcClient::new(
            "https://127.0.0.1:10009".into(),
            "/nonexistent/path/tls.cert".into(),
            &dummy_macaroon(),
            Duration::from_secs(5),
        )
        .expect_err("should fail on missing cert");
        assert!(matches!(err, BuildError::TlsCertRead { .. }));
    }

    // NOTE: a happy-path construction test would need a real DER-encoded
    // self-signed certificate to satisfy tonic 0.12's
    // ClientTlsConfig::ca_certificate parsing. Generating one inside
    // a unit test is overkill for v0.8; the success path is exercised
    // end-to-end by the polling collector tests (BTH-63) and the Polar
    // e2e harness (BTH-67), both of which provide real LND TLS material.
}
